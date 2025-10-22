#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

generate!({
    world: "example-udp",
    path: "../../wit",
    generate_all,
});

use ariel::wasm_bindings::udp_api::UdpSocket;
use ariel::wasm_bindings::log_api::info;
struct MyComponent;

impl Guest for MyComponent {
    fn bind_socket(port: u16) {
        info("Hello from inside the capsule");
        UdpSocket::bind(port).unwrap();
    }

    fn run() -> () {
        match UdpSocket::try_recv() {
            Ok(Some((data, endpoint))) => {
                info("Received a packet, echoing it back");
                UdpSocket::send(&data, endpoint).unwrap();
            }
            Ok(None) => {
                // Not packet were ready to be received
                // info("N");

            }
            Err(_) => {
                info("Something's wrong with the network configuration");
                panic!()
            }
        }
    }
}

export!(MyComponent);


#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}