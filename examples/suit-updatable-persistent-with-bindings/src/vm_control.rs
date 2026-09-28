use alloc::vec::Vec;
use alloc::vec;
use alloc::string::String;

use ariel_os::log::{Debug2Format, info};

use ariel_os_bindings::wasm::coap::{CanInstantiate, PersistentCapsule};
use coap_handler::{Handler, Reporting, Record, Attribute};
use coap_message::{Code, MessageOption, OptionNumber};
use coap_message_utils::{Error as CoapError, OptionsExt};

use coap_handler_implementations::{HandlerBuilder, ReportingHandlerBuilder, new_dispatcher};
use coap_message_implementations::inmemory_write::GenericMessage;


use coap_numbers::option::{URI_PATH, BLOCK1};

use wasmtime::{Engine, Store, component::{Linker, Component}};

pub enum Slot {
    Main,
    RollBack,
}

/// A persistent that can also be update by sending PUT request with no path
pub struct SuitUpdatablePersistentCapsule<'w, T: 'static + Default, G> {
    instance: Option<(Store<T>, G)>,
    main_payload: Vec<u8>,
    rollback_payload: Vec<u8>,
    staging_manifest: Vec<u8>,
    last_accepted_sequence_number: u64,
    paths: Vec<StringRecord>,
    engine: &'w Engine,
    running_slot: Slot,
}

impl<'w, T:'static + Default, G> SuitUpdatablePersistentCapsule<'w, T, G>{
    pub fn new(engine: &'w Engine) -> Self {
        Self {
            engine,
            instance: None,
            rollback_payload: Vec::new(),
            main_payload: Vec::new(),
            staging_manifest: Vec::new(),
            last_accepted_sequence_number: 0,
            paths: Vec::new(),
            running_slot: Slot::RollBack,
        }
    }
}

impl<T: 'static + Default, G: CanInstantiate<T> + PersistentCapsule<T>> SuitUpdatablePersistentCapsule<'_, T, G> {

    fn update_with_suit(&mut self, capsule: Vec<u8>) -> Result<(), CoapError> {
        let (new_slot, new_payload, rollback_payload) = match self.running_slot {
            Slot::Main => (Slot::RollBack, &mut self.main_payload, &mut self.rollback_payload),
            Slot::RollBack => (Slot::Main, &mut self.rollback_payload, &mut self.main_payload),
        };

        *new_payload = capsule;

        info!("Fetched a payload of size: {:?}", &new_payload.len());

        // Try to instantiate the new capsule and initialized the handler
        // We assume that errors in `coap_run` are "normal". Whereas errors in `initialize_handler` are
        // rollback_worthy.
        let mut new_store =  Store::new(self.engine, T::default());

        let wasm = new_payload;
        let component = unsafe {
            Component::deserialize_raw(&self.engine, wasm.as_slice().into())
        }
                .map_err(|e| {
                    info!("Could not instantiate capsule: {:?}", Debug2Format(&e));
                    CoapError::bad_request()
                }
            )?;


        let mut linker = Linker::<T>::new(&self.engine);
        let mut new_instance = G::instantiate(&mut linker, &mut new_store, component).map_err(
            |e| {
                info!("Could not instantiate capsule: {:?}", Debug2Format(&e));
                CoapError::not_acceptable()
            }
        )?;

        new_instance.initialize_handler(&mut new_store).map_err(
            |e| {
                info!("Could not initialized the handler: {:?}", Debug2Format(&e));
                CoapError::bad_request()
            }
        )?;

        self.paths = new_instance
            .report_resources(&mut new_store)
            .map_err(|e| e.into())?
            .into_iter()
            .map(|s| StringRecord(s))
            .collect();

        self.instance = Some((new_store, new_instance));
        // Clear the rollback_payload
        rollback_payload.clear();

        self.running_slot = new_slot;

        Ok(())
    }

    fn process_put_request<M: coap_message::ReadableMessage>(&mut self, request: &M, block1: Option<u32>) -> Result<(Option<u32>, u8), CoapError> {
        info!("Received PUT request for program ");
        // This is a bit of a simplification, but ignoring the block size and just
        // appending is really kind'a fine IMO.
        let block1_value = block1.unwrap_or(0);

        // FIXME there's probably a Size1 option; if so, reallocate to fail early.

        let szx = block1_value & 0x7;
        let blocksize = 1usize << (4 + szx);
        let offset = (block1_value >> 4) as usize * blocksize;

        if offset == 0 {
            self.staging_manifest.clear();
        }
        if self.staging_manifest.len() != offset {
            return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
        }

        let payload = request.payload();
        self.staging_manifest.try_reserve_exact(payload.len()).map_err(|e| {
            info!(
                "Failed to reserve memory for program: {:?}",
                Debug2Format(&e)
            );
            CoapError::internal_server_error()
        })?;
        self.staging_manifest.extend_from_slice(payload);

        if (block1_value & 0x8) == 0x8 {
            Ok((block1, coap_numbers::code::CONTINUE))
        } else {
            let image = core::mem::take(&mut self.staging_manifest);
            info!("Sending MAnifest for verification");
            crate::SUIT_VERIFY_SIGNAL.signal((self.last_accepted_sequence_number, image.into_boxed_slice()));
            Ok((block1, coap_numbers::code::CHANGED))
        }
    }
}


impl<T: 'static + Default, G: PersistentCapsule<T> + CanInstantiate<T>> Handler for SuitUpdatablePersistentCapsule<'_, T, G> {
    /// When receiving SUIT manifest, you need the block1 option and the code,
    /// When receiving other request, the capsule returns Vec<u8> and the code directly
    type RequestData = (Option<u32>, Option<Vec<u8>>, u8);


    type ExtractRequestError = coap_message_utils::Error;
    type BuildResponseError<M: coap_message::MinimalWritableMessage> = M::UnionError;


    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError>
    {
        info!("First checking if update were present beforehand");
        match crate::UPDATE_RESULTS.try_take() {
            Some(Ok((seq_num, capsule))) => {
                self.last_accepted_sequence_number = seq_num;

                match self.update_with_suit(capsule) {
                    Ok(()) => { },
                    Err(_e) => {
                        info!("Error during installation");
                    }
                }
            }
            Some(Err(())) => {
                info!("Error during fetching Suit auth");
            }
            None => {
                info!("No updates since last request")
            }
        }

        let mut block1: Option<u32> = None;
        let mut no_path: bool = true;

        request.options().filter(|o| {
            if o.number() == URI_PATH
            && no_path && o.value_str().is_some() {
                no_path = false;
                false
            } else if o.number() == BLOCK1 && let Some(bk1) = o.value_uint(){
                block1 = Some(bk1);
                false
            } else {
                true
            }
        }).ignore_elective_others()?;

        if no_path {
            match request.code().into() {
                coap_numbers::code::DELETE => {
                    info!("Received DELETE request. Stopping the VM and dropping it from the active slot. Also dropping the backup slot if there was any");
                    if self.instance.is_none() {
                        return Err(CoapError::not_found());
                    } else {
                        let _ = self.instance.take();
                        self.main_payload.clear();
                        self.rollback_payload.clear();
                        self.running_slot = Slot::RollBack;
                        return Ok((None, None, coap_numbers::code::DELETED))
                    }
                }
                coap_numbers::code::PUT => {
                    self.process_put_request(request, block1).map(|(block1, code)| {(block1, None, code) })
                }
                _ => return Err(CoapError::method_not_allowed())
            }
        } else {
            match &mut self.instance {
                Some((store, instance))=> {
                    let mut incoming_code: u8 = request.code().into();

                    let mut buffer = core::iter::repeat_n(0, 1280).collect::<Vec<u8>>();

                    let mut reencoded = GenericMessage::new(&mut incoming_code, &mut buffer);
                    reencoded.set_from_message2(request).unwrap();
                    let incoming_len = reencoded.finish();

                  instance
                    .coap_run(store, incoming_code, incoming_len as u32, buffer)
                    .map(|(code, message)| (None, Some(message), code))
                    .map_err(|e| {
                        e.into()
                    })
                }
                None => Err(CoapError::service_unavailable()),
            }
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        1280
    }

    fn build_response<M: coap_message::MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {

        match request {
            (block1, None, code) => {
                response.set_code(M::Code::new(code)?);

                if let Some(block1) = block1 {
                    response
                        .add_option_uint(
                            M::OptionNumber::new(coap_numbers::option::BLOCK1)?,
                            block1 as u32,
                        )?;
                }
            },
            (None, Some(message), code) => {
                response.set_from_message2(&coap_message_implementations::inmemory::Message::new(
                    code, &message,
                ))?;
            }
            _ => unreachable!("Shouldn't be possible...")
        }
        Ok(())
    }
}


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

impl<T: 'static + Default, G: PersistentCapsule<T> + CanInstantiate<T>> Reporting for SuitUpdatablePersistentCapsule<'_, T, G> {
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

        let mut resources = vec![StringRecord(String::new())];
        resources.extend_from_slice(&self.paths);
        resources.into_iter()
    }
}




impl<T: 'static + Default, G: PersistentCapsule<T> + CanInstantiate<T>> SuitUpdatablePersistentCapsule<'_, T, G> {
    pub fn to_handler(self) -> impl Handler + Reporting {
        new_dispatcher().below(&["vm"], self).with_wkc()
    }
}

/// FIXME: use trait function when the WithSortedOptions bound situation is fixed
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
