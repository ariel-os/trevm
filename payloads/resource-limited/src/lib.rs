#![no_std]

#[cfg(all(not(target_feature = "atomics"), target_family = "wasm"))]
#[global_allocator]
static TALC: talc::wasm::WasmDynamicTalc = talc::wasm::new_wasm_dynamic_allocator();

use wit_bindgen::generate;

extern crate alloc;
use alloc::format;
use alloc::vec::Vec;

generate!({
    world: "example-resource-limited",
    path: "../../wit",
    generate_all,
});

use ariel::wasm_bindings::log_api::info;
use ariel::wasm_bindings::rng_api::RNG;

pub struct MyComponent;

impl Guest for MyComponent {
    fn run() {
        info("Hello from the capsule. Going to create a Vec.");

        let mut vec = Vec::new();

        info("Starting loop.");
        for i in 0.. {
            let r = RNG::next_u32();
            vec.push(r);
            info(&format!("Inserted {} at index {}.", r, i));
        }
    }
}

export!(MyComponent);

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}
