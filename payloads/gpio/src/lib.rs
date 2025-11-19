#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

extern crate alloc;

use alloc::string::ToString as _;

generate!({
    world: "example-gpio",
    path: "../../wit",
    generate_all,
});

use ariel::wasm_bindings::gpio_api::{toggle_led, wait_for_button_low};
use ariel::wasm_bindings::log_api::info;
use ariel::wasm_bindings::time_api::sleep;
struct MyComponent;

impl Guest for MyComponent {
    fn blinky() -> () {
        info("In the capsule");
        loop {
            wait_for_button_low().unwrap();
            toggle_led().unwrap();
            sleep(300);
        }
    }
}

export!(MyComponent);

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}
