#![no_main]
#![no_std]

use ariel_os::debug::{ExitCode, exit};
use ariel_os::log::{Debug2Format, info};

use ariel_os::time::Timer;

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

// extern crate alloc;

use ariel_os_bindings::wasm::ArielOSHost;

bindgen!({
    world: "mvp",
    path: "../../wit",
    with: {
        "wasm-card:crypto/crypto-extension": ariel_os_bindings::wasm::wasm_card_crypto::common,
    },
});

impl MvpImports for ArielOSHost {
    fn log_array(&mut self, data: wasmtime::component::__internal::Vec<u8>,) -> () {
        info!("{:?}", Debug2Format(&data));
    }
}

#[ariel_os::task(autostart)]
async fn main() {

    let r = run_wasm().await;
    info!("{:?}", Debug2Format(&r));
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


    let engine = Engine::new(&config)?;
    let component_bytes = if cfg!(target_pointer_width = "64") {
        include_bytes!("../payload-native.cwasm").as_slice()
    } else {
        include_bytes!("../payload.cwasm").as_slice()
    };

    let component = unsafe { Component::deserialize_raw(&engine, component_bytes.into()) }?;

    let host = ArielOSHost::default();

    let mut store = Store::new(&engine, host);

    let mut linker = Linker::new(&engine);

    Mvp::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    let bindings = Mvp::instantiate(&mut store, &component, &linker)?;

    bindings.call_run(&mut store)?;

    Ok(())
}
