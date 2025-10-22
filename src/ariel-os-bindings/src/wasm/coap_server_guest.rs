// use wasmtime::component::bindgen;
use wasmtime::Store;

use coap_handler::{Handler, Reporting};
use coap_handler_implementations::{new_dispatcher, HandlerBuilder, ReportingHandlerBuilder, SimpleRendered};
use coap_message_implementations::inmemory_write::GenericMessage;
use coap_handler_implementations::wkc;

pub use coap_message_utils::Error as CoAPError;

// use ariel_os_debug::log::{debug, info};

extern crate alloc;
use alloc::vec::Vec;

// Completely Useless because exported interfaces don't create traits
// bindgen!({
//     world: "ariel:wasm-bindings/coap-server",
//     path: "../../wit",
// });

pub trait CoapServerGuest {
    type E: Into<CoAPError>;
    fn coap_run<T: 'static>(&mut self, store: &mut Store<T>, code:u8, observed_len: u32, message: Vec<u8>) -> Result<(u8, Vec<u8>), Self::E>;
}


pub struct WasmHandler<T: 'static, G: CoapServerGuest> {
    store: Store<T>,
    instance: G,
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

    impl<T: coap_message::MinimalWritableMessage> AbleToBeSetFromMessage for T { }
}

use disable_sort_options_bound::AbleToBeSetFromMessage;


impl<T:'static, G: CoapServerGuest> Handler for WasmHandler<T, G> {
    // request data is the message replied by the inner handler along it's code
    type RequestData = (u8, Vec<u8>);

    type ExtractRequestError = CoAPError;

    type BuildResponseError<M: coap_message::MinimalWritableMessage> = M::UnionError;


    fn extract_request_data<M: coap_message::ReadableMessage>(
            &mut self,
            request: &M,
        ) -> Result<Self::RequestData, Self::ExtractRequestError> {

        let mut incoming_code: u8 = request.code().into();
        // info!("HOST incoming request with payload {:?}", request.payload());
        // for o in request.options() {
        //     info!("HOST Option {} {:?}", o.number(), o.value());
        // };

        let mut buffer = core::iter::repeat_n(0, 1280).collect::<Vec<u8>>();

        let mut reencoded = GenericMessage::new(&mut incoming_code, &mut buffer);
        reencoded.set_from_message2(request).unwrap();
        let incoming_len = reencoded.finish();

        // info!("HOST len: {}\n {:?}", incoming_len, defmt::Debug2Format(&buffer));

        return self.instance.coap_run(&mut self.store, incoming_code, incoming_len as u32, buffer).map_err(|e| e.into());
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
        response.set_from_message2(&coap_message_implementations::inmemory::Message::new(request.0, &request.1))
    }
}


impl<T: 'static, G: CoapServerGuest> Reporting for WasmHandler<T, G> {
    type Record<'a>
        = wkc::EmptyRecord
    where
        Self: 'a;
    type Reporter<'a>
        = core::iter::Once<wkc::EmptyRecord>
    where
        Self: 'a;

    fn report(&self) -> Self::Reporter<'_> {
        // Using a ConstantSliceRecord instead would be tempting, but that'd need a const return
        // value from self.0.content_format()
        core::iter::once(wkc::EmptyRecord {})
    }
}


pub fn build_wasm_handler<T:'static, G: CoapServerGuest>(store: wasmtime::Store<T>, instance: G) -> impl Handler + Reporting {
    let handler = new_dispatcher()
        .below(&["vm"], WasmHandler { store, instance })
        .at(&["hello"], SimpleRendered("Hello from the host"))
        .with_wkc();

    return handler;
}