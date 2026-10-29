#![no_main]
#![no_std]

extern crate alloc;
use alloc::boxed::Box;

use ariel_os::{
    debug::{ExitCode, exit},
    log::info,
    time::Timer,
};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, ResourceLimiter, Store};

use ariel_os_bindings::wasm::ArielOSHost;

bindgen!({
    world: "example-resource-limited",
    path: "../../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
        "ariel:wasm-bindings/rng-api": ariel_os_bindings::wasm::rng,
    },
    require_store_data_send: true,
});

struct Limiter;

impl ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        info!(
            "memory_growing called with current {}, desired {} and maximum {:?}",
            current, desired, maximum
        );

        Ok(desired < 40_000)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        info!(
            "table_growing called with current {}, desired {} and maximum {:?}",
            current, desired, maximum
        );

        Ok(true)
    }
}

#[ariel_os::task(autostart)]
async fn main() {
    match run_wasm().await {
        Ok(()) => info!("Wasm run completed"),
        Err(e) => info!("Wasm failed:\n{:#}", defmt::Display2Format(&e)),
    }

    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

async fn run_wasm() -> wasmtime::Result<()> {
    let mut config = Config::new();

    // Options that must conform with the precompilation step
    config.wasm_custom_page_sizes(true);
    config.target("pulley32").unwrap();

    config.table_lazy_init(false);
    config.memory_reservation(0); // Must be zero for Talc to use malloc memory
    config.memory_init_cow(false);
    config.memory_may_move(true);

    // Options that can be changed without changing the payload
    config.max_wasm_stack(2048);
    config.memory_reservation_for_growth(0);

    // Options relating to async
    config.async_stack_size(4096);

    // Enable fuel consumption
    config.consume_fuel(true);

    let engine = Engine::new(&config)?;

    let component_bytes = include_bytes!("../payload.cwasm");

    let component =
        unsafe { Component::deserialize_raw(&engine, component_bytes.as_slice().into()) }?;

    let host = ArielOSHost::new(Box::new(Limiter {}));

    let mut store = Store::new(&engine, host);

    store.set_fuel(10_000_000)?; // Set the initial fuel for the Store
    store.fuel_async_yield_interval(Some(5_000))?; // Async yield every 5_000 used fuel
    store.limiter(|host| host.limiter.as_mut()); // Install the limiter for memory usage

    let mut linker = Linker::new(&engine);

    ExampleResourceLimited::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    let bindings =
        ExampleResourceLimited::instantiate_async(&mut store, &component, &linker).await?;

    bindings.run.call_async(&mut store, &[], &mut []).await?;

    Ok(())
}

#[cfg(feature = "monitor-heap")]
#[ariel_os::task(autostart)]
async fn monitor_heap() {
    loop {
        let free = esp_alloc::HEAP.free();
        info!("Free Heap: {} bytes", free);
        Timer::after_millis(10).await;
    }
}
