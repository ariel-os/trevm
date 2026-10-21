#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

generate!({
    world: "example-crypto",
    path: "../../wit",
    generate_all,
});

extern crate alloc;

use alloc::vec;

use wasm_card::crypto::ecdsa_api::{
    generate, hash
};



struct MyComponent;

impl Guest for MyComponent {
    fn run() -> () {
        let key_pair = generate();

        key_pair.0.print();
        key_pair.1.print();

        let bytes = vec![0, 1, 3, 4];

        let hashed = hash(&bytes);

        log_array(&hashed);
    }
}

export!(MyComponent);

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}
