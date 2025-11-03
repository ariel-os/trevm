use wasmtime::{
    Store,
    component::{Component, Linker},
};

use coap_handler::{Attribute, Handler, Record, Reporting};
use coap_handler_implementations::{HandlerBuilder, SimpleRendered, new_dispatcher};
use coap_message_implementations::inmemory_write::GenericMessage;

pub use coap_message_utils::Error as CoAPError;

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

// Completely Useless because exported interfaces don't create traits
// bindgen!({
//     world: "ariel:wasm-bindings/coap-server",
//     path: "../../wit",
// });

pub trait CoapServerGuest {
    type E: Into<CoAPError>;
    fn coap_run<T: 'static>(
        &mut self,
        store: &mut Store<T>,
        code: u8,
        observed_len: u32,
        message: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), Self::E>;

    fn initialize_handler<T: 'static>(&mut self, store: &mut Store<T>) -> Result<(), ()>;

    fn report_resources<T: 'static>(
        &mut self,
        store: &mut Store<T>,
    ) -> Result<Vec<String>, Self::E>;
}

/// Glue layer that allows a generic backend to operate on any concrete bindgen type.
///
/// Open questions:
/// * Could this be part of (or interdependent with) CoapServerGuest?
/// * Do we need it to be a trait in the first place? (Maybe all sensible applications that can use
///   this module have to use the singel bindgen output anyway, and thus all the bindgen could move
///   into this module.)
pub trait CanInstantiate<T>: Sized {
    /// Runs Self::add_to_linker and Self::instantiate (which are bindgen generated methods without
    /// a type)
    fn instantiate(
        linker: &mut Linker<T>,
        store: &mut Store<T>,
        component: Component,
    ) -> wasmtime::Result<Self>;
}

// FIXME: pub as with all other WasmHandler fields
pub enum WasmHandlerState<T: 'static, G: CoapServerGuest> {
    Running { store: Store<T>, instance: G },
    NotRunning { store_data: T },
    // This is mainly used so we don't have to resort to take_mut tricks, and can process data from
    // one state into the next one.
    //
    // Ideally this never hits RAM because it's living short enough that the compiler won't even
    // use it because it sees that it's overwritten in code that can't panic.
    //
    // If this is seen in any place outside of where it's expected (i.e., when it's not just
    // replaced immediately), the library may panic.
    Taken,
}

impl<T: 'static, G: CoapServerGuest> WasmHandlerState<T, G> {
    pub fn stop(&mut self) {
        *self = match core::mem::replace(self, WasmHandlerState::Taken) {
            WasmHandlerState::Running { store, .. } => WasmHandlerState::NotRunning {
                store_data: store.into_data(),
            },
            // of take, but then we just leave it that way
            stopped => stopped,
        }
    }
}

pub struct WasmHandler<T: 'static, G: CoapServerGuest> {
    // All fields are pub mainly while we figure out which pieces of any new-program logic best go
    // where.
    pub state: WasmHandlerState<T, G>,
    pub paths: Vec<StringRecord>,
    /// Backing data of the instance.
    ///
    /// This is empty initially when the program is loaded from flash.
    ///
    /// # Safety invariants
    ///
    /// This needs to stay unchanged as long as an instance is `Some`; this is a guarantee used to
    /// satisfy the `Component::deserialize_raw` requirements.
    // FIXME This should be `unsafe pub`, but that's not stable, and we should "just" re-evaluate
    // what our boundaries are
    pub program: Vec<u8>,
}

pub struct WasmHandlerWrapped<'w, T: 'static, G: CoapServerGuest>(
    pub &'w core::cell::RefCell<WasmHandler<T, G>>,
);

impl<'w, T: 'static, G: CoapServerGuest> Clone for WasmHandlerWrapped<'w, T, G> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<T: 'static, G: CoapServerGuest> WasmHandler<T, G> {
    pub fn new(store_data: T) -> Self {
        WasmHandler {
            state: WasmHandlerState::NotRunning { store_data },
            program: Vec::new(),
            paths: Vec::new(),
        }
    }

    /// Start running a CoAP server from 'static code (which is typically shipped with the firmware
    /// and resides in flash)
    ///
    /// # Safety
    ///
    /// The requirements of [`wasmtime::Component::deserialize`] apply. (Paraphrasing: This needs
    /// to be wasmtime prepared code; arbitrary data may execute arbitrary code).
    pub unsafe fn start_from_static(
        &mut self,
        wasm: &'static [u8],
        engine: &wasmtime::Engine,
    ) -> wasmtime::Result<()>
    where
        G: CanInstantiate<T>,
    {
        // SAFETY:
        // * The requirement on code content is forwarded.
        // * The requirement on code lifetime is satisfied by the 'static.
        unsafe { self.start_raw(wasm.into(), engine) }
    }

    /// Start running a CoAP server from data that has been prepared in `.program`.
    ///
    /// # Safety
    ///
    /// The requirements of [`wasmtime::Component::deserialize`] apply. (Paraphrasing: This needs
    /// to be wasmtime prepared code; arbitrary data may execute arbitrary code).
    pub unsafe fn start_from_dynamic(&mut self, engine: &wasmtime::Engine) -> wasmtime::Result<()>
    where
        G: CanInstantiate<T>,
    {
        // SAFETY:
        // * The requirement on code content is forwarded.
        // * The requirement on code lifetime is satisfied by the type's unsafe invariant that
        //   program is not mutated while running.
        unsafe { self.start_raw(self.program.as_slice().into(), engine) }
    }

    /// Starts running a CoAP server from a provided instance.
    ///
    /// This expects `self.state` to be currently taken.
    ///
    /// # Safety
    ///
    /// The requirements of [`wasmtime::Component::deserialize_raw`] apply. (Paraphrasing: This
    /// needs to be wasmtime prepared code; arbitrary data may execute arbitrary code, and the
    /// program code must outlive any use of the returned instance).
    pub unsafe fn start_raw(
        &mut self,
        wasm: core::ptr::NonNull<[u8]>,
        engine: &wasmtime::Engine,
    ) -> wasmtime::Result<()>
    where
        G: CanInstantiate<T>,
    {
        let WasmHandlerState::NotRunning { store_data } =
            core::mem::replace(&mut self.state, WasmHandlerState::Taken)
        else {
            // FIXME: Just provide a .stop() here and run that before.
            panic!("Starting from non-stopped state.");
        };

        let mut store = Store::new(&engine, store_data);
        let component = unsafe { Component::deserialize_raw(&engine, wasm)? };
        let mut linker = Linker::<T>::new(&engine);
        let mut instance = G::instantiate(&mut linker, &mut store, component)?;

        instance.initialize_handler(&mut store).unwrap();
        self.paths = instance
            .report_resources(&mut store)
            .map_err(|e| e.into())
            .unwrap()
            .into_iter()
            .map(|s| StringRecord(s))
            .collect();
        self.state = WasmHandlerState::Running { store, instance };

        Ok(())
    }
}

impl<'w, T: 'static, G: CoapServerGuest> WasmHandlerWrapped<'w, T, G> {
    pub fn to_handler(self) -> impl Handler + Reporting {
        let handler = new_dispatcher()
            .below(&["vm"], self.clone())
            .at(&["hello"], SimpleRendered("Hello from the host"));

        return handler;
    }
}

/// FIXME: use trait function when the WithSortedOptions bound
mod disable_sort_options_bound {
    use coap_message::MessageOption;
    use coap_message::{Code, OptionNumber};

    pub trait AbleToBeSetFromMessage: coap_message::MinimalWritableMessage {
        fn set_from_message2<M>(&mut self, msg: &M) -> Result<(), Self::UnionError>
        where
            M: coap_message::ReadableMessage,
        {
            self.set_code(Self::Code::new(msg.code().into())?);

            for opt in msg.options() {
                self.add_option(Self::OptionNumber::new(opt.number())?, opt.value())?;
            }
            self.set_payload(msg.payload())?;
            Ok(())
        }
    }

    impl<T: coap_message::MinimalWritableMessage> AbleToBeSetFromMessage for T {}
}

use disable_sort_options_bound::AbleToBeSetFromMessage;

impl<'w, T: 'static, G: CoapServerGuest> Handler for WasmHandlerWrapped<'w, T, G> {
    // request data is the message replied by the inner handler along it's code
    type RequestData = (u8, Vec<u8>);

    type ExtractRequestError = CoAPError;

    type BuildResponseError<M: coap_message::MinimalWritableMessage> = M::UnionError;

    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        // The handler is exclusive, we don't have to worry about simultaneous access until we
        // allow the WasmHandler to also perform tasks outside CoAP.
        let s = &mut *self.0.borrow_mut();

        match &mut s.state {
            WasmHandlerState::Running { store, instance } => {
                let mut incoming_code: u8 = request.code().into();
                // info!("HOST incoming request with payload {:?}", request.payload());
                // for o in request.options() {
                //     info!("HOST Option {} {:?}", o.number(), o.value());
                // };

                let mut buffer = core::iter::repeat_n(0, 1280).collect::<Vec<u8>>();

                let mut reencoded = GenericMessage::new(&mut incoming_code, &mut buffer);
                reencoded.set_from_message2(request).unwrap();
                let incoming_len = reencoded.finish();

                instance
                    .coap_run(store, incoming_code, incoming_len as u32, buffer)
                    .map_err(|e| e.into())
            }
            _other => Err(CoAPError::service_unavailable()),
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        // Good enough I guess ...
        1280
    }

    fn build_response<M: coap_message::MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {
        response.set_from_message2(&coap_message_implementations::inmemory::Message::new(
            request.0, &request.1,
        ))
    }
}

// FIXME this is quite alloc'y

#[derive(Clone)]
pub struct StringRecord(pub String);

impl Record for StringRecord {
    type PathElement = String;
    type PathElements = core::iter::Once<String>;
    type Attributes = core::iter::Empty<Attribute>;

    fn attributes(&self) -> Self::Attributes {
        core::iter::empty()
    }

    fn rel(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> Self::PathElements {
        core::iter::once(self.0.clone())
    }
}

impl<'w, T: 'static, G: CoapServerGuest> Reporting for WasmHandlerWrapped<'w, T, G> {
    type Record<'a>
        = StringRecord
    where
        Self: 'a;

    type Reporter<'a>
        = alloc::vec::IntoIter<StringRecord>
    where
        Self: 'a;

    fn report(&self) -> Self::Reporter<'_> {
        // Using a ConstantSliceRecord instead would be tempting, but that'd need a const return
        // value from self.0.content_format()

        self.0.borrow().paths.clone().into_iter()
    }
}
