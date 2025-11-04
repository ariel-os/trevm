#![no_main]
#![no_std]

use ariel_os::coap::coap_run;
use ariel_os::debug::log::{defmt, error, info};
use ariel_os::debug::{ExitCode, exit};

use ariel_os::time::Timer;
use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use ariel_os_bindings::wasm::coap_server_guest::{
    CanInstantiate, CoAPError, CoapServerGuest, WasmHandler, WasmHandlerWrapped,
};

use ariel_os_bindings::wasm::ArielOSHost;

use coap_handler::Handler;
use coap_handler_implementations::{HandlerBuilder, ReportingHandlerBuilder};

bindgen!({
    world: "example-coap-server",
    path: "../../wit",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
    }
});

use crate::exports::ariel::wasm_bindings::coap_server_guest::CoapErr;

impl Into<CoAPError> for CoapErr {
    fn into(self) -> CoAPError {
        match self {
            CoapErr::NotFound => CoAPError::not_found(),
            CoapErr::InternalServerError => CoAPError::internal_server_error(),
            CoapErr::HandlerNotBuilt => CoAPError::internal_server_error(),
            // _ => CoAPError::internal_server_error(),
        }
    }
}

impl CoapServerGuest for ExampleCoapServer {
    type E = CoapErr;
    fn coap_run<T: 'static>(
        &mut self,
        store: &mut Store<T>,
        code: u8,
        observed_len: u32,
        buffer: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), Self::E> {
        match self.ariel_wasm_bindings_coap_server_guest().call_coap_run(
            store,
            code,
            observed_len,
            &buffer,
        ) {
            Ok(coap_rep) => coap_rep,
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed, CoAP requests to it will return 5.00 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                return Err(CoapErr::InternalServerError);
            }
        }
    }

    fn initialize_handler<T: 'static>(&mut self, store: &mut Store<T>) -> Result<(), ()> {
        match self
            .ariel_wasm_bindings_coap_server_guest()
            .call_initialize_handler(store)
        {
            Ok(handler_init_rep) => handler_init_rep,
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed at startup, CoAP requests to it will return 5.00 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                Err(())
            }
        }
    }

    fn report_resources<T: 'static>(
        &mut self,
        store: &mut Store<T>,
    ) -> Result<Vec<String>, Self::E> {
        match self
            .ariel_wasm_bindings_coap_server_guest()
            .call_report(store)
        {
            Ok(handler_init_rep) => handler_init_rep,
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed at startup, CoAP requests to it will return 5.03 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                Err(CoapErr::HandlerNotBuilt)
            }
        }
    }
}

impl ExampleCoapServerImports for ArielOSHost {
    fn uppercase(&mut self, s: String) -> String {
        info!(
            "WASM asked us to uppercase {:?}. Maybe we can use hardware acceleration for it?",
            s.as_str()
        );
        s.to_uppercase()
    }
}

impl CanInstantiate<ArielOSHost> for ExampleCoapServer {
    fn instantiate(
        mut linker: &mut Linker<ArielOSHost>,
        mut store: &mut Store<ArielOSHost>,
        component: Component,
    ) -> wasmtime::Result<Self> {
        ExampleCoapServer::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
        ExampleCoapServer::instantiate(&mut store, &component, &linker)
    }
}

#[ariel_os::task(autostart)]
async fn main() {
    let res = run_wasm_coap_server().await;
    info!("{:?}", defmt::Debug2Format(&res));
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

async fn run_wasm_coap_server() -> wasmtime::Result<()> {
    let mut config = Config::default();

    // Options that must conform with the precompilation step
    config.wasm_custom_page_sizes(true);
    config.target("pulley32").unwrap();

    config.table_lazy_init(false);
    config.memory_reservation(0);
    config.memory_init_cow(false);
    config.memory_may_move(false);

    // Options that can be changed without changing the payload
    config.max_wasm_stack(2048);
    config.memory_reservation_for_growth(0);

    let engine = Engine::new(&config).unwrap();

    let host = ArielOSHost::default();

    let wasm = include_bytes!("../payload.cwasm").as_slice();

    let mut wasmhandler = WasmHandler::new(host);
    // SAFETY: Data in that file was produced by ./precompile_wasm.rs
    unsafe {
        wasmhandler.start_from_static(wasm, &engine)?;
    }
    let wrapped = WasmHandlerWrapped(&core::cell::RefCell::new(wasmhandler));
    let handler = wrapped
        .clone()
        .to_handler()
        .at_with_attributes(
            &["vm-control"],
            &[],
            Control {
                wrapped,
                engine: &engine,
            },
        )
        .with_wkc();
    info!("Starting Handler");
    coap_run(handler).await;
}

struct Control<'w> {
    wrapped: WasmHandlerWrapped<'w, ArielOSHost, ExampleCoapServer>,
    // FIXME: I'd rather just carry around the wrapped, but apparently there are some pieces we
    // can't just extract from the instance inside again easily (but maybe this should work).
    engine: &'w Engine,
}

impl<'w> Handler for Control<'w> {
    // Block option to respond with, and code
    type RequestData = (Option<u32>, u8);

    type ExtractRequestError = coap_message_utils::Error;

    type BuildResponseError<M: coap_message::MinimalWritableMessage> = coap_message_utils::Error;

    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        use coap_message::MessageOption;
        use coap_message_utils::OptionsExt;

        let s = &mut *self.wrapped.0.borrow_mut();

        match request.code().into() {
            coap_numbers::code::DELETE => {
                // FIXME: Handle If-Match
                request.options().ignore_elective_others()?;

                s.stop();

                Ok((None, coap_numbers::code::DELETED))
            }
            coap_numbers::code::PUT => {
                // FIXME: There's probably a ToDo around
                // coap_message_utils::option_value::Block2RequestData to also make block1 options
                // usable more easily
                let mut block1: Option<u32> = None;
                // FIXME: Handle If-None-Match, If-Match
                request
                    .options()
                    .filter(|o| {
                        if o.number() == coap_numbers::option::BLOCK1
                            && let Some(n) = o.value_uint()
                            && block1.is_none()
                        {
                            block1 = Some(n);
                            false
                        } else {
                            true
                        }
                    })
                    .ignore_elective_others()?;

                // This is a bit of a simplification, but ignoring the block size and just
                // appending is really kind'a fine IMO.
                let block1 = block1.unwrap_or(0);

                // FIXME there's probably a Size1 option; if so, reallocate to fail early.

                let szx = block1 & 0x7;
                let blocksize = 1 << (4 + szx);
                let offset = (block1 >> 4) * blocksize;

                if offset == 0 {
                    s.stop();
                    s.mutate_program().unwrap().truncate(0);
                }

                let Ok(program) = s.mutate_program() else {
                    // FIXME: CoAPError should have such a constructor too (but there's no harm in
                    // returning an error through the Ok path).
                    return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
                };

                // If we had any of the content signed, we'd have to take care not to let any of
                // the calculations truncate / overflow, lest someone might send a wrappingly large
                // file that only after wrapping is malicious, but as long as all trust is in a
                // single authenticated peer, this does not matter yet.
                if program.len() != offset as usize {
                    return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
                }

                let payload = request.payload();
                program
                    .try_reserve(payload.len())
                    // FIXME: Request Entity Too Big?
                    .map_err(|_| CoAPError::internal_server_error())?;
                program.extend_from_slice(payload);

                if block1 & 0x8 == 0x8 {
                    // More to say you have?
                    Ok((Some(block1), coap_numbers::code::CONTINUE))
                } else {
                    info!(
                        "Re-instantiating based on program of {} bytes.",
                        program.len()
                    );

                    // SAFETY: We trust the user to provide us with checked data
                    unsafe {
                        s.start_from_dynamic(self.engine)
                            // FIXME: relay more details?
                            .map_err(|_| CoAPError::bad_request())
                    }?;

                    // FIXME if there was no Block1 option at all, can we still send some?
                    Ok((Some(block1), coap_numbers::code::CHANGED))
                }
            }
            _ => Err(CoAPError::method_not_allowed()),
        }
    }

    fn estimate_length(&mut self, _request: &Self::RequestData) -> usize {
        1
    }

    fn build_response<M: coap_message::MutableWritableMessage>(
        &mut self,
        response: &mut M,
        request: Self::RequestData,
    ) -> Result<(), Self::BuildResponseError<M>> {
        let (block1, code) = request;
        use coap_message::{Code, OptionNumber};

        response.set_code(M::Code::new(code).map_err(CoAPError::from_unionerror)?);
        if let Some(block1) = block1 {
            response
                .add_option_uint(
                    M::OptionNumber::new(coap_numbers::option::BLOCK1)
                        .map_err(CoAPError::from_unionerror)?,
                    block1,
                )
                .map_err(CoAPError::from_unionerror)?;
        }
        Ok(())
    }
}

// Same as https://github.com/bytecodealliance/wasmtime/blob/main/examples/min-platform/embedding/wasmtime-platform.c
// I have no idea whether this is safe or not.
// https://github.com/bytecodealliance/wasmtime/blob/aec935f2e746d71934c8a131be15bbbb4392138c/crates/wasmtime/src/runtime/vm/traphandlers.rs#L888
static mut TLS_PTR: usize = 0;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get() -> *mut u8 {
    #[allow(unsafe_code)]
    unsafe {
        TLS_PTR as *mut u8
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(val: *const u8) {
    #[allow(unsafe_code)]
    unsafe {
        TLS_PTR = val as usize
    };
}
