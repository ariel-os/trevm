#![no_main]
#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use ariel_os::coap::coap_run;
use ariel_os::debug::log::{Debug2Format, info};

use coap_handler::Handler;
use coap_handler_implementations::{HandlerBuilder, ReportingHandlerBuilder, new_dispatcher};

use coap_message::{Code, OptionNumber};

use coap_message_utils::Error as CoapError;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use embassy_futures::select::{Either, select};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

use ariel_os_bindings::wasm::ArielOSHost;

#[derive(Debug)]
enum UpdateMsg {
    Install(Vec<u8>),
    Stop,
}

static UPDATE: Signal<CriticalSectionRawMutex, UpdateMsg> = Signal::new();

bindgen!({
    world: "example-async",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
        "ariel:wasm-bindings/rng-api": ariel_os_bindings::wasm::rng,

    },
    require_store_data_send: true,
});

enum Payload {
    Static(&'static [u8]),
    Owned(Box<[u8]>),
}

impl Payload {
    fn as_bytes(&self) -> &[u8] {
        match self {
            Payload::Static(bytes) => bytes,
            Payload::Owned(bytes) => bytes,
        }
    }

    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

struct VmControl {
    program: Vec<u8>,
}

impl VmControl {
    fn new() -> Self {
        Self {
            program: Vec::new(),
        }
    }
}

impl Handler for VmControl {
    type RequestData = (Option<u32>, u8);

    type ExtractRequestError = coap_message_utils::Error;
    type BuildResponseError<M: coap_message::MinimalWritableMessage> = coap_message_utils::Error;

    fn extract_request_data<M: coap_message::ReadableMessage>(
        &mut self,
        request: &M,
    ) -> Result<Self::RequestData, Self::ExtractRequestError> {
        use coap_message::MessageOption;
        use coap_message_utils::OptionsExt;

        match request.code().into() {
            coap_numbers::code::DELETE => {
                info!("Received DELETE request for program ");
                request.options().ignore_elective_others()?;

                self.program.clear();
                UPDATE.signal(UpdateMsg::Stop);

                Ok((None, coap_numbers::code::DELETED))
            }

            coap_numbers::code::PUT => {
                info!("Received PUT request for program ");
                let mut block1: Option<u32> = None;

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
                let blocksize = 1usize << (4 + szx);
                let offset = (block1 >> 4) as usize * blocksize;

                if offset == 0 {
                    self.program.clear();
                }
                if self.program.len() != offset {
                    return Ok((None, coap_numbers::code::REQUEST_ENTITY_INCOMPLETE));
                }

                let payload = request.payload();
                self.program.try_reserve_exact(payload.len()).map_err(|e| {
                    info!(
                        "Failed to reserve memory for program: {:?}",
                        Debug2Format(&e)
                    );
                    CoapError::internal_server_error()
                })?;
                self.program.extend_from_slice(payload);

                if (block1 & 0x8) == 0x8 {
                    Ok((Some(block1), coap_numbers::code::CONTINUE))
                } else {
                    let image = core::mem::take(&mut self.program);
                    UPDATE.signal(UpdateMsg::Install(image));
                    Ok((Some(block1), coap_numbers::code::CHANGED))
                }
            }

            _ => Err(CoapError::method_not_allowed()),
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
        response.set_code(M::Code::new(code).map_err(CoapError::from_unionerror)?);

        if let Some(block1) = block1 {
            response
                .add_option_uint(
                    M::OptionNumber::new(coap_numbers::option::BLOCK1)
                        .map_err(CoapError::from_unionerror)?,
                    block1 as u32,
                )
                .map_err(CoapError::from_unionerror)?;
        }
        Ok(())
    }
}

#[ariel_os::task(autostart)]
async fn coap_task() {
    let control = VmControl::new();

    let handler = new_dispatcher()
        .at_with_attributes(&["vm-control"], &[], control)
        .with_wkc();

    info!("Starting CoAP handler");
    coap_run(handler).await;
}

#[ariel_os::task(autostart)]
async fn runner_task() {
    let engine = make_engine();
    let initial_payload = include_bytes!("../async-payload.cwasm").as_slice();
    let mut payload = Payload::Static(initial_payload);

    info!("Initial payload size: {} bytes", initial_payload.len());

    let mut linker = Linker::new(&engine);
    ExampleAsync::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state).unwrap();

    loop {
        payload = wait_for_payload(payload).await;
        info!("Current payload size: {} bytes", payload.as_bytes().len());

        if let Some(next_payload) = run_payload(&engine, &linker, &payload).await {
            payload = next_payload;
        }
    }
}

fn make_engine() -> Engine {
    let mut cfg = Config::default();
    cfg.wasm_custom_page_sizes(true);
    cfg.target("pulley32").unwrap();

    // Must match precompilation
    cfg.table_lazy_init(false);
    cfg.memory_reservation(0);
    cfg.memory_init_cow(false);
    cfg.memory_may_move(false);

    // Runtime-only tuning
    cfg.max_wasm_stack(2048);
    cfg.memory_reservation_for_growth(0);
    cfg.async_stack_size(4096);

    Engine::new(&cfg).unwrap()
}

async fn wait_for_payload(mut payload: Payload) -> Payload {
    while payload.is_empty() {
        match UPDATE.wait().await {
            UpdateMsg::Install(new_img) => {
                info!("Accepted new capsule image ({} bytes)", new_img.len());
                payload = Payload::Owned(new_img.into_boxed_slice());
            }
            UpdateMsg::Stop => {}
        }
    }

    payload
}

async fn run_payload(
    engine: &Engine,
    linker: &Linker<ArielOSHost>,
    payload: &Payload,
) -> Option<Payload> {
    let bytes = payload.as_bytes();
    let mem = core::ptr::NonNull::from(bytes);
    let component = unsafe { Component::deserialize_raw(engine, mem.into()) }.unwrap();

    let host = ArielOSHost::default();
    let mut store = Store::new(engine, host);

    let bindings = ExampleAsync::instantiate_async(&mut store, &component, linker)
        .await
        .unwrap();

    info!("Running payload");

    let run_fut = bindings.run.call_async(&mut store, &[], &mut []);
    let update_fut = UPDATE.wait();

    let next = match select(update_fut, run_fut).await {
        Either::First(UpdateMsg::Install(new_img)) => {
            Some(Payload::Owned(new_img.into_boxed_slice()))
        }
        Either::First(UpdateMsg::Stop) => Some(Payload::Static(&[])),
        Either::Second(_) => None,
    };

    info!("Payload done!");
    next
}
