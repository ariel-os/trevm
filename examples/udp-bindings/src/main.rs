#![no_main]
#![no_std]

use ariel_os::debug::{
    ExitCode, exit,
    log::{defmt, info},
};
use ariel_os::time::Timer;

use ariel_os::net;
use ariel_os::reexports::embassy_net::udp::PacketMetadata;
use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

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

    let component =
        unsafe { Component::deserialize_raw(&engine, component_bytes.as_slice().into()) }?;

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
            &mut tx_buffer,
        );
    }

    let mut store = Store::new(&engine, host);

    store.set_fuel(1_000_000)?;

    let mut linker = Linker::new(&engine);

    ExampleUdp::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    let bindings = ExampleUdp::instantiate(&mut store, &component, &linker)?;
    bindings.call_bind_socket(&mut store, 1234)?;
    loop {
        // This function might never return but it will stop because of fuel exhaustation
        bindings.call_run(&mut store)?;
        Timer::after_millis(10).await;
    }
}
