use core::slice::Iter;

use wasmtime::Store;

use coap_handler::{Attribute, Handler, Record, Reporting};
use coap_handler_implementations::{
    HandlerBuilder, ReportingHandlerBuilder, SimpleRendered, new_dispatcher,
};
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

pub struct WasmHandler<T: 'static, G: CoapServerGuest> {
    store: Store<T>,
    instance: G,
    paths: Vec<StringRecord>,
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
    pub fn new(mut store: wasmtime::Store<T>, mut instance: G) -> Self {
        instance.initialize_handler(&mut store).unwrap();
        let paths = instance
            .report_resources(&mut store)
            .map_err(|e| e.into())
            .unwrap()
            .into_iter()
            .map(|s| StringRecord(s))
            .collect();
        WasmHandler {
            store,
            instance,
            paths,
        }
    }
}

impl<'w, T: 'static, G: CoapServerGuest> WasmHandlerWrapped<'w, T, G> {
    pub fn to_handler(self) -> impl Handler + Reporting {
        let handler = new_dispatcher()
            .below(&["vm"], self.clone())
            .at(&["hello"], SimpleRendered("Hello from the host"))
            .with_wkc();

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

        let mut incoming_code: u8 = request.code().into();
        // info!("HOST incoming request with payload {:?}", request.payload());
        // for o in request.options() {
        //     info!("HOST Option {} {:?}", o.number(), o.value());
        // };

        let mut buffer = core::iter::repeat_n(0, 1280).collect::<Vec<u8>>();

        let mut reencoded = GenericMessage::new(&mut incoming_code, &mut buffer);
        reencoded.set_from_message2(request).unwrap();
        let incoming_len = reencoded.finish();

        return s
            .instance
            .coap_run(&mut s.store, incoming_code, incoming_len as u32, buffer)
            .map_err(|e| e.into());
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

pub struct StringRecord(String);

impl<'a> Record for &'a StringRecord {
    type PathElement = &'a String;
    type PathElements = core::iter::Once<&'a String>;
    type Attributes = core::iter::Empty<Attribute>;

    fn attributes(&self) -> Self::Attributes {
        core::iter::empty()
    }

    fn rel(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> Self::PathElements {
        core::iter::once(&self.0)
    }
}

impl<'w, T: 'static, G: CoapServerGuest> Reporting for WasmHandlerWrapped<'w, T, G> {
    type Record<'a>
        = &'a StringRecord
    where
        Self: 'a;

    type Reporter<'a>
        = Iter<'a, StringRecord>
    where
        Self: 'a;

    fn report(&self) -> Self::Reporter<'_> {
        // Using a ConstantSliceRecord instead would be tempting, but that'd need a const return
        // value from self.0.content_format()

        let s = self.0.borrow();

        // FIXME: Broken temporarily during work on RefCell wrapping
        /*s.paths*/
        [].iter()
    }
}
