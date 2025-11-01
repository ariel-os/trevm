#![no_main]
#![no_std]

use ariel_os::debug::{
    ExitCode, exit,
    log::{Debug2Format, info},
};
use ariel_os::time::{Duration, Instant, Timer, with_timeout};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

// extern crate alloc;

use ariel_os_bindings::wasm::ArielOSHost;

bindgen!({
    world: "example-async",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/rng-api": ariel_os_bindings::wasm::rng,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
    },
    imports: { default: async },
    exports: { default: async },
});

#[ariel_os::task(autostart)]
async fn main() {
    let now = Instant::now();

    // Timeout of 9 seconds to showcase that the runtime yields regularly
    let r = with_timeout(Duration::from_secs(9), run_wasm()).await;
    let new_now = Instant::now();
    info!("{:?}", Debug2Format(r));
    info!("This took {:?} ms", (new_now - now).as_millis());
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

/// # Errors
/// Misconfiguration of Wasmtime or of the component
async fn run_wasm() -> wasmtime::Result<()> {
    let mut config = Config::default();

    // Options that must conform with the precompilation step
    config.wasm_custom_page_sizes(true);
    config
        .target(
            // Even if it is interpreted, pointer width and endianness have to match the host -- but we
            // currently don't have any "be" systems that we could branch on further. (If things
            // don't align, the unwrap will complain anyway.)
            if cfg!(target_pointer_width = "64") {
                "pulley64"
            } else {
                "pulley32"
            },
        )
        .unwrap();

    config.table_lazy_init(false);
    config.memory_reservation(0);
    config.memory_init_cow(false);
    config.memory_may_move(false);

    // Options that can be changed without changing the payload
    config.max_wasm_stack(2048);
    config.memory_reservation_for_growth(0);

    // Async support
    config.async_support(true);
    config.async_stack_size(4096);

    // Fuel
    config.consume_fuel(true);

    let engine = Engine::new(&config)?;
    let component_bytes = if cfg!(target_pointer_width = "64") {
        include_bytes!("../payload.pulley64f.cwasm").as_slice()
    } else {
        include_bytes!("../payload.cwasm").as_slice()
    };

    let component =
        unsafe { Component::deserialize_raw(&engine, component_bytes.into()) }?;

    let host = ArielOSHost::default();

    let mut store = Store::new(&engine, host);

    // Enough fuel to never run out before the timeout
    store.set_fuel(1_000_000_000)?;

    // Yield every 10_000 fuel expanded to allow for timeouts
    store.fuel_async_yield_interval(Some(10_000))?;

    let mut linker = Linker::new(&engine);

    ExampleAsync::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    let bindings = ExampleAsync::instantiate_async(&mut store, &component, &linker).await?;

    bindings.call_run(&mut store).await?;

    Ok(())
}

// Same as https://github.com/bytecodealliance/wasmtime/blob/main/examples/min-platform/embedding/wasmtime-platform.c
// I have no idea whether this is safe or not.
// https://github.com/bytecodealliance/wasmtime/blob/aec935f2e746d71934c8a131be15bbbb4392138c/crates/wasmtime/src/runtime/vm/traphandlers.rs#L888
static mut TLS_PTR: usize = 0;
#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_get() -> *mut u8 {
    unsafe { TLS_PTR as *mut u8 }
}

#[unsafe(no_mangle)]
extern "C" fn wasmtime_tls_set(val: *const u8) {
    unsafe { TLS_PTR = val as usize };
}
