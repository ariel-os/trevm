#![no_main]
#![no_std]

use ariel_os::debug::{exit, log::{defmt, info}, ExitCode};
use ariel_os::time::Timer;

use ariel_os::net;
use ariel_os::reexports::embassy_net::udp::PacketMetadata;
use wasmtime::{Config, Engine, Store};
use wasmtime::component::{bindgen, Component, HasSelf, Linker};

use ariel_os_bindings::wasm::ArielOSHost;

bindgen!({
    world: "example-udp",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/udp-api": ariel_os_bindings::wasm::udp,
    },
});

static BUFFER_SIZE: usize = 128;

#[ariel_os::task(autostart)]
async fn main() {
    let r = run_wasm().await;
    info!("{:?}", defmt::Debug2Format(&r));
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}


/// # Errors
/// Misconfiguration of Wasmtime or of the component
async fn run_wasm() -> wasmtime::Result<()> {
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

    // Use fuel instrumentation to prevent indefinite execution
    config.consume_fuel(true);

    let engine = Engine::new(&config)?;
    let component_bytes = include_bytes!("../payload.cwasm");

    let component = unsafe { Component::deserialize_raw(&engine, component_bytes.as_slice().into()) }?;

    let mut host = ArielOSHost::default();

    let stack = net::network_stack().await.unwrap();

    let mut rx_buffer = [0; BUFFER_SIZE];
    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0; BUFFER_SIZE];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];

    unsafe {
        host.initialize_socket(
            stack,
            &mut rx_meta,
            &mut rx_buffer,
            &mut tx_meta,
            &mut tx_buffer
        );
    }

    let mut store = Store::new(&engine, host);

    store.set_fuel(1_000_000)?;

    let mut linker = Linker::new(&engine);

    ExampleUdp::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| {state})?;
    let bindings = ExampleUdp::instantiate(&mut store, &component, &linker)?;
    bindings.call_bind_socket(&mut store, 1234)?;
    loop {
    // This function might never return but it will stop because of fuel exhaustation
        bindings.call_run(&mut store)?;
        Timer::after_millis(10).await;
    }
}



// Same as https://github.com/bytecodealliance/wasmtime/blob/main/examples/min-platform/embedding/wasmtime-platform.c
// I have no idea whether this is safe or not.
// https://github.com/bytecodealliance/wasmtime/blob/aec935f2e746d71934c8a131be15bbbb4392138c/crates/wasmtime/src/runtime/vm/traphandlers.rs#L888
static mut TLS_PTR: u32 = 0;
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get() -> *mut u8 {
    unsafe { TLS_PTR as *mut u8 }
}

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(val: *const u8) {
   unsafe { TLS_PTR = val as u32 };
}
