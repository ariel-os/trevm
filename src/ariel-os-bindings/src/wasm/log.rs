use ariel_os_debug::log::info;

extern crate alloc;
use alloc::string::String;

use wasmtime::component::bindgen;

bindgen!({
    world: "ariel:wasm-bindings/log@0.0.1",
    path: "../../wit/",
});


pub use ariel::wasm_bindings::log_api::{Host, HostWithStore, add_to_linker};


#[derive(Default)]
pub(crate) struct ArielLogHost;

impl Host for ArielLogHost {
    fn info(&mut self, input: String) {
        info!("[WASM] {}", input.as_str());
    }
}
