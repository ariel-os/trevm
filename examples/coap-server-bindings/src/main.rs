#![no_main]
#![no_std]

use ariel_os::coap::coap_run;
use ariel_os::debug::{exit, ExitCode};
use ariel_os::debug::{log::{info, defmt}};

use ariel_os::time::Timer;
use wasmtime::{Config, Engine, Store};
use wasmtime::component::{bindgen, Component, HasSelf, Linker};

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

use ariel_os_bindings::wasm::coap_server_guest::{
    CoapServerGuest,
    CoAPError,
    build_wasm_handler,
};

use ariel_os_bindings::wasm::ArielOSHost;

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
    fn coap_run<T: 'static>(&mut self, store: &mut Store::<T>, code: u8, observed_len: u32, buffer: Vec<u8>) -> Result<(u8, Vec<u8>), Self::E> {
        self.ariel_wasm_bindings_coap_server_guest().call_coap_run(store, code, observed_len, &buffer).unwrap()
    }

    fn initialize_handler<T: 'static>(&mut self, store: &mut Store<T>) -> Result<(), ()> {
        self.ariel_wasm_bindings_coap_server_guest().call_initialize_handler(store).unwrap()
    }

    fn report_resources<T: 'static>(&mut self, store: &mut Store<T>) -> Result<Vec<String>, Self::E> {
        self.ariel_wasm_bindings_coap_server_guest().call_report(store).unwrap()
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

    let mut store = Store::new(&engine, host);

    let wasm = include_bytes!("../payload.cwasm");

    let component = unsafe { Component::deserialize_raw(&engine, wasm.as_slice().into())? };

    let mut linker = Linker::new(&engine);

    ExampleCoapServer::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| { state })?;
    let instance = ExampleCoapServer::instantiate(&mut store, &component, &linker)?;

    let handler = build_wasm_handler(store, instance);
    info!("Starting Handler");
    coap_run(handler).await;
}

// Same as https://github.com/bytecodealliance/wasmtime/blob/main/examples/min-platform/embedding/wasmtime-platform.c
// I have no idea whether this is safe or not.
// https://github.com/bytecodealliance/wasmtime/blob/aec935f2e746d71934c8a131be15bbbb4392138c/crates/wasmtime/src/runtime/vm/traphandlers.rs#L888
static mut TLS_PTR: u32 = 0;

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get() -> *mut u8 {
    #[allow(unsafe_code)]
    unsafe { TLS_PTR as *mut u8 }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(val: *const u8) {
    #[allow(unsafe_code)]
    unsafe { TLS_PTR = val as u32 };
}
