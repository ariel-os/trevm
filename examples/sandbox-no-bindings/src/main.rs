#![no_main]
#![no_std]

extern crate alloc;
use alloc::string::String;

use ariel_os::coap::coap_run;
use ariel_os::debug::{ExitCode, exit};
use ariel_os::log::info;

use ariel_os::time::Timer;
use wasmtime::component::{Component, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

use coap_handler_implementations::{ReportingHandlerBuilder, new_dispatcher};

use ariel_os_bindings::wasm::coap::{CanInstantiate, EphemeralCapsule};

use ariel_os_bindings::wasm::ArielOSHost;

use ariel_os_bindings::wasm::coap::sanbdox::Sandbox;
bindgen!({
    world: "example-sandbox-no-bindings",
    path: "../../wit",
});

impl CanInstantiate<ArielOSHost> for ExampleSandboxNoBindings {
    fn instantiate(
        linker: &mut Linker<ArielOSHost>,
        store: &mut Store<ArielOSHost>,
        component: Component,
    ) -> wasmtime::Result<Self> {
        ExampleSandboxNoBindings::instantiate(store, &component, &linker)
    }
}

impl EphemeralCapsule<ArielOSHost, String> for ExampleSandboxNoBindings {
    fn run(&mut self, store: &mut Store<ArielOSHost>) -> wasmtime::Result<String> {
        self.call_run(store)
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

    let sandbox: Sandbox<'_, ArielOSHost, String, ExampleSandboxNoBindings> = Sandbox::new(&engine);

    let handler = sandbox.to_handler(new_dispatcher()).with_wkc();

    info!("Starting Handler");
    coap_run(handler).await;
    #[allow(unreachable_code)]
    Ok(())
}
