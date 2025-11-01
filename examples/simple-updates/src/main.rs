#![no_main]
#![no_std]

use ariel_os::debug::{
    ExitCode, exit,
    log::{defmt, info},
};
use ariel_os::time::Timer;
use ariel_os_boards::pins;

use ariel_os::gpio::{Input, Pull};
use ariel_os::hal::group_peripherals;

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

use embassy_futures::select::{Either, select};

use ariel_os_bindings::wasm::ArielOSHost;

pub enum Enumerate {
    One,
    Two,
}

pub use Enumerate::*;

bindgen!({
    world: "example-updates",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
    },
    exports: {
        default: async,
    },
    imports: {
        default: async,
    }
});

group_peripherals!(Peripherals {
    buttons: pins::ButtonPeripherals,
});

#[ariel_os::task(autostart, peripherals)]
async fn main(p: Peripherals) {
    let r = run_wasm(p).await;
    info!("{:?}", defmt::Debug2Format(&r));
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

fn configure_engine() -> wasmtime::Result<Engine> {
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

    // async support
    config.async_support(true);
    config.async_stack_size(4096);

    Ok(Engine::new(&config)?)
}

/// # Errors
/// Misconfiguration of Wasmtime or of the component
async fn run_wasm(peris: Peripherals) -> wasmtime::Result<()> {
    let engine = configure_engine()?;
    let mut current_store: Store<ArielOSHost>;
    let mut linker = Linker::<ArielOSHost>::new(&engine);
    ExampleUpdates::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

    let mut btn = Input::builder(peris.buttons.button0, Pull::Up)
        .build_with_interrupt()
        .unwrap();

    let comp1 = include_bytes!("../payload1.cwasm");
    let comp2 = include_bytes!("../payload2.cwasm");
    // This can only be done once per payload bytes because we are using Deserialize Raw (or is it ? How does it know)
    let component2 = unsafe { Component::deserialize_raw(&engine, comp2.as_slice().into())? };
    let component1 = unsafe { Component::deserialize_raw(&engine, comp1.as_slice().into())? };
    let mut current = One;
    let mut current_instance: Option<ExampleUpdates>;
    loop {
        // Change the component code rebuild the store because it otherwise won't free memory.
        match current {
            Two => {
                current_store = Store::new(&engine, ArielOSHost::default());
                let instance =
                    ExampleUpdates::instantiate_async(&mut current_store, &component1, &mut linker)
                        .await?;
                current_instance = Some(instance);
                current = One;
            }
            One => {
                current_store = Store::new(&engine, ArielOSHost::default());
                let instance =
                    ExampleUpdates::instantiate_async(&mut current_store, &component2, &mut linker)
                        .await?;
                current_instance = Some(instance);
                current = Two;
            }
        }
        // Main loop
        // Switch between two instances when a button is pushed
        match current_instance.as_ref() {
            None => unreachable!(),
            Some(instance) => {
                match select(
                    instance.call_run(&mut current_store),
                    btn.wait_for_falling_edge(),
                )
                .await
                {
                    Either::First(_) => {
                        info!(
                            "The capsule is done working, now waiting on a button press to start the other capsule"
                        );
                        btn.wait_for_falling_edge().await;
                    }
                    Either::Second(_) => {
                        info!("A button was pressed, now changing capsule");
                    }
                }
            }
        }
    }
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
