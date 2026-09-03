use core::fmt::{Debug, Write};
use core::marker::PhantomData;

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use ariel_os_log::info;

use coap_handler::{Handler, Reporting};

use coap_handler_implementations::helpers::block2_write;
use coap_handler_implementations::{HandlerBuilder, SimpleRendered};

use coap_message::MessageOption;

use coap_message_utils::Error as CoAPError;
use coap_message_utils::OptionsExt;
use coap_message_utils::option_value::Block2RequestData;

use wasmtime::component::{Component, Linker};
use wasmtime::{Engine, Store};

use super::coap_traits::EphemeralCapsule;

enum SandboxError {
    WebAssembly,
    NotFound,
}

/// A Sandbox that instantiates, runs and then deletes simple wasm capsules
pub struct Sandbox<'a, T: 'static + Default, R: Debug, G: EphemeralCapsule<T, R>> {
    instances: BTreeMap<String, (Store<T>, G)>,
    engine: &'a Engine,
    _marker: PhantomData<R>,
    last_received_vector: Vec<u8>,
}

impl<'a, T: 'static + Default, R: Debug, G: EphemeralCapsule<T, R>> Sandbox<'a, T, R, G> {
    /// Creates a Sandbox using the provided engine
    pub fn new(engine: &'a Engine) -> Self {
        Self {
            engine,
            instances: BTreeMap::new(),
            _marker: PhantomData,
            last_received_vector: Vec::new(),
        }
    }

    /// Looks up a capsule and executes it and returns the result
    fn execute_capsule(&mut self, uri_path: &str) -> Result<R, SandboxError> {
        if let Some((store, instance)) = self.instances.get_mut(uri_path) {
            instance.run(store).map_err(|_| SandboxError::WebAssembly)
        } else {
            Err(SandboxError::NotFound)
        }
    }

    /// Instantiates a capsule at the given path from the already present bytecode
    ///
    /// # Safety
    ///
    /// The requirements of [`wasmtime::Component::deserialize`] apply. (Paraphrasing: This needs
    /// to be wasmtime prepared code; arbitrary data may execute arbitrary code).
    unsafe fn instantiate_capsule(&mut self, uri_path: String) -> Result<(), SandboxError> {
        // SAFETY:
        // * The requirement on code content is forwarded.
        let mut store = Store::new(self.engine, T::default());
        let comp = unsafe {
            Component::deserialize(self.engine, self.last_received_vector.as_slice())
                .map_err(|_| SandboxError::WebAssembly)?
        };
        let mut linker = Linker::new(self.engine);
        let instance =
            G::instantiate(&mut linker, &mut store, comp).map_err(|_| SandboxError::WebAssembly)?;
        self.instances.insert(uri_path, (store, instance));
        Ok(())
    }

    fn process_put_request(
        &mut self,
        uri_path: String,
        block1: Option<u32>,
        payload: &[u8],
    ) -> Result<(Option<u32>, u8), CoAPError> {
        // This is a bit of a simplification, but ignoring the block size and just
        // appending is really kind'a fine IMO.
        let block1 = block1.unwrap_or_default();

        // FIXME there's probably a Size1 option; if so, reallocate to fail early.

        let szx = block1 & 0x7;
        if szx == 7 {
            return Err(CoAPError::bad_request());
        }

        let blocksize = 1 << (4 + szx);
        let m = block1 & 0x8 == 0x8;
        let offset = (block1 >> 4) * blocksize;

        // Means that this is the first block of the body of a new capsule
        if offset == 0 {
            // Remove the instance if there is one to avoid unecessary RAM usage
            let _ = self.instances.remove(&uri_path);
            self.last_received_vector.truncate(0);
        }

        // If we had any of the content signed, we'd have to take care not to let any of
        // the calculations truncate / overflow, lest someone might send a wrappingly large
        // file that only after wrapping is malicious, but as long as all trust is in a
        // single authenticated peer, this does not matter yet.
        if self.last_received_vector.len() != offset as usize {
            // FIXME: CoAPError should have such a constructor too (but there's no harm in
            // returning an error through the Ok path).
            return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
        }

        // If this isn't the last block, the implied block size and received
        // block size must be the same
        if m && blocksize as usize != payload.len() {
            return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
        }

        if self
            .last_received_vector
            .try_reserve(payload.len())
            .is_err()
        {
            self.last_received_vector.truncate(0);
            // FIXME: CoAPError should have such a constructor too (but there's no harm in
            // returning an error through the Ok path).
            return Ok((None, coap_numbers::code::REQUEST_ENTITY_TOO_LARGE));
        }

        // Add the received bytes to the payload
        self.last_received_vector.extend_from_slice(payload);

        if m {
            // Transfer isn't complete yet
            Ok((Some(block1), coap_numbers::code::CONTINUE))
        } else {
            // Transfer is done, instantiate the capsule and return
            // SAFETY:
            // * We trust our authenticated users
            match unsafe { self.instantiate_capsule(uri_path) } {
                Err(SandboxError::WebAssembly) => return Err(CoAPError::bad_request()),
                Err(_) => unreachable!(),
                Ok(_) => {
                    info!(
                        "Instantiated capsule based on program of {} bytes.",
                        self.last_received_vector.len()
                    );
                    self.last_received_vector.truncate(0);
                }
            }
            Ok((Some(block1), coap_numbers::code::CREATED))
        }
    }

    pub fn to_handler(self, base: impl Handler + Reporting) -> impl Handler + Reporting {
        base.below(&["sandbox"], self).at(
            &["sandbox-instructions"],
            SimpleRendered(
                "PUT your wasm code as /sandbox/path/ and later GET the same URI to run the code",
            ),
        )
    }
}

impl<T: 'static + Default, R: Debug, G: EphemeralCapsule<T, R>> Handler for Sandbox<'_, T, R, G> {
    // Block1 option to respond with, code and block2 option to respond with;
    type RequestData = (Option<u32>, u8, Option<(Block2RequestData, String)>);

    type ExtractRequestError = CoAPError;

    type BuildResponseError<M: coap_message::MinimalWritableMessage> = CoAPError;

    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        use coap_numbers::option::{BLOCK1, BLOCK2, URI_PATH};

        // Process options once
        let mut block1: Option<u32> = None;
        let mut path: Option<String> = None;
        let mut block2: Option<Block2RequestData> = None;

        request
            .options()
            .filter(|o| {
                if o.number() == URI_PATH
                    && path.is_none()
                    && let Some(uri_path) = o.value_str()
                {
                    path = Some(String::from(uri_path));
                    false
                } else if o.number() == BLOCK1
                    && block1.is_none()
                    && let Some(n) = o.value_uint()
                {
                    block1 = Some(n);
                    false
                } else if o.number() == BLOCK2
                    && block2.is_none()
                    && let Ok(n) = Block2RequestData::from_option(o)
                {
                    block2 = Some(n);
                    false
                } else {
                    true
                }
            })
            .ignore_elective_others()?;

        let Some(path) = path else {
            return Err(CoAPError::not_found());
        };
        // I don't want to deal with empty path
        if path.is_empty() {
            return Err(CoAPError::forbidden());
        }

        match request.code().into() {
            // Request to instantiate a new capsule
            coap_numbers::code::PUT => {
                let (b1opt, code) = self.process_put_request(path, block1, request.payload())?;
                Ok((b1opt, code, None))
            }
            coap_numbers::code::GET => Ok((
                None,
                coap_numbers::code::CONTENT,
                Some((block2.unwrap_or_default(), path)),
            )),
            coap_numbers::code::DELETE => {
                self.instances.remove(&path);
                Ok((None, coap_numbers::code::DELETED, None))
            }
            _ => Err(CoAPError::method_not_allowed()),
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        1280 - 40 - 4 // does this correclty calculate the IPv6 minimum MTU?
    }

    fn build_response<M: coap_message::MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {
        use coap_message::{Code, OptionNumber};

        let (block1, code, block2_and_path) = request;

        response.set_code(M::Code::new(code).map_err(CoAPError::from_unionerror)?);

        if let Some(block1) = block1 {
            response
                .add_option_uint(
                    M::OptionNumber::new(coap_numbers::option::BLOCK1)
                        .map_err(CoAPError::from_unionerror)?,
                    block1,
                )
                .map_err(CoAPError::from_unionerror)?;
        } else if let Some((block2, path)) = block2_and_path {
            // SAFETY
            // We trust the user to have provided us with safe data
            let result = match self.execute_capsule(&path) {
                Err(SandboxError::NotFound) => Err(CoAPError::not_found()),
                Err(SandboxError::WebAssembly) => Err(CoAPError::internal_server_error()),
                Ok(r) => Ok(r),
            }?;
            block2_write(block2, response, |w| {
                write!(w, "{:?}", result).map_err(|_| CoAPError::internal_server_error())
            })?;
        }
        Ok(())
    }
}

impl<T: 'static + Default, R: Debug, G: EphemeralCapsule<T, R>> Reporting for Sandbox<'_, T, R, G> {
    type Record<'res>
        = StringRef<'res>
    where
        Self: 'res;

    type Reporter<'res>
        = core::iter::Map<
        alloc::collections::btree_map::Keys<'res, String, (Store<T>, G)>,
        for<'a> fn(&'a String) -> StringRef<'a>,
    >
    where
        Self: 'res;

    fn report(&self) -> Self::Reporter<'_> {
        self.instances.keys().map(|path| StringRef(path.as_str()))
    }
}

use coap_handler::{Attribute, Record};
pub struct StringRef<'a>(pub &'a str);

impl<'a> Record for StringRef<'a> {
    type PathElement = &'a str;
    type PathElements = core::iter::Once<&'a str>;
    type Attributes = core::iter::Empty<Attribute>;

    fn attributes(&self) -> Self::Attributes {
        core::iter::empty()
    }

    fn rel(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> Self::PathElements {
        core::iter::once(self.0)
    }
}
