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
    world: "example-updates",
    path: "../../wit",
    generate_all,
});

use ariel::wasm_bindings::time_api::{sleep, now_as_millis};
use ariel::wasm_bindings::log_api::info;
struct MyComponent;

impl Guest for MyComponent {
    fn run() -> () {
        info("Hello from payload B");


        let mut prefix = "It has been ".to_string();
        prefix.push_str(now_as_millis().to_string().as_str());
        prefix.push_str(" ms since boot");
        info(&prefix);
        info("Now sleeping for 10_000 seconds then returning, please cancel me");
        sleep(10_000_000);
    }
}

export!(MyComponent);


#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}