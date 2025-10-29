#![no_main]
#![no_std]

use ariel_os_boards::pins;

use ariel_os::{debug::{exit, log::{defmt, info}, ExitCode}};
use ariel_os::gpio::{Input, Level, Output, Pull};

use wasmtime::{Config, Engine, Store};
use wasmtime::component::{bindgen, Component, HasSelf, Linker};

use ariel_os_bindings::wasm::ArielOSHost;


bindgen!({
    world: "example-gpio",
    path: "../../wit",
    with: {
        "ariel:wasm-bindings/gpio-api": ariel_os_bindings::wasm::gpio,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
    },
    imports: { default: async },
    exports: { default: async },
});

ariel_os::hal::group_peripherals!(Peripherals {
    leds: pins::LedPeripherals,
    buttons: pins::ButtonPeripherals,
});


#[ariel_os::task(autostart, peripherals)]
async fn main(peris: Peripherals) {
    let r = run_wasm(peris).await;
    info!("{:?}", defmt::Debug2Format(&r));
    exit(ExitCode::SUCCESS);
}

async fn run_wasm(peris: Peripherals) -> wasmtime::Result<()> {
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

    // Options relating to async
    config.async_support(true);
    config.async_stack_size(4096);


    let led1 = Output::new(peris.leds.led0, Level::Low);
    let pull = Pull::Up;

    let btn1 = Input::builder(peris.buttons.button0, pull)
        .build_with_interrupt()
        .unwrap();

    let mut host = ArielOSHost::default();
    host.bind_peris(led1, btn1);

    let engine = Engine::new(&config)?;

    let component_bytes = include_bytes!("../payload.cwasm");

    let component = unsafe { Component::deserialize_raw(&engine, component_bytes.as_slice().into()) }?;

    let mut store = Store::new(&engine, host);

    let mut linker = Linker::new(&engine);

    ExampleGpio::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| {state})?;
    let bindings = ExampleGpio::instantiate_async(&mut store, &component, &mut linker).await?;

    bindings.call_blinky(&mut store).await?;

    Ok(())
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