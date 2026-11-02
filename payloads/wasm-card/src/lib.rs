#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

generate!({
    world: "mvp",
    path: "../../wit",
    generate_all,
});

extern crate alloc;

use alloc::vec;

use wasm_card::crypto::crypto_extension::{
    generate, hash, into_scalar, curve_mult, scalar_mult, scalar_sub
};



struct MyComponent;

impl Guest for MyComponent {
    fn run() -> () {
        let key_pair1 = generate();

        key_pair1.0.print();
        key_pair1.1.print();

        let key_pair2 = generate();


        let bytes = vec![0, 1, 3, 4];

        let hashed = hash(&bytes);

        log_array(&hashed);

        let scalar_1 = into_scalar(key_pair1.1);
        scalar_1.print();

        let scalar_2 = into_scalar(key_pair2.1);
        scalar_2.print();

        let mult_by_gen = curve_mult(&scalar_1);

        mult_by_gen.print();

        let mul_scalar = scalar_mult(&scalar_2, &mult_by_gen);

        mul_scalar.print();

        let sub_scalar = scalar_sub(&mul_scalar, &scalar_1);

        sub_scalar.print();
    }
}

export!(MyComponent);

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}
