#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

generate!({
    world: "example-coap-server",
    path: "../../wit",
    generate_all,
});

use coap_message_implementations::inmemory::Message;
use coap_message_implementations::inmemory_write::GenericMessage;
use coap_handler_implementations::{new_dispatcher, SimpleRendered, HandlerBuilder};
use coap_handler::{Handler, Reporting};

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::ToString as _;

#[allow(unused_imports)]
use ariel::wasm_bindings::log_api::info;

use exports::ariel::wasm_bindings::coap_server_guest::{Guest, CoapErr};
struct MyComponent;

impl Guest for MyComponent {
    fn coap_run(code: u8, observed_len: u32, message: Vec<u8>) -> Result<(u8, Vec<u8>), CoapErr> {
        coap_run(code, observed_len, message)
    }
}


fn coap_run(mut code: u8, observed_len: u32,  mut message: Vec<u8>) -> Result<(u8, Vec<u8>), CoapErr> {
    let mut handler = build_handler();
    // info("A request was received, you love to see it");

    let reencoded = Message::new(code, &message[..observed_len as usize]);
    // for o in reencoded.options() {
    //     let formated = format!("Option {} {:?}", o.number(), o.value());
    //     info(&formated);
    // };


    let extracted = match handler.extract_request_data(&reencoded) {
        Ok(ex) => ex,
        Err(_) => {
            return Err(CoapErr::NotFound);
        }
    };
    drop(reencoded);
    message.as_mut_slice().fill(0);

    let mut response = GenericMessage::new(&mut code, message.as_mut_slice());

    match handler.build_response(&mut response, extracted) {
        Err(_) => {
            return Err(CoapErr::InternalServerError);
        }
        _ => {}
    }

    let outgoing_len = response.finish();
    message.truncate(outgoing_len);
    return Ok((code, message));
}


fn build_handler() -> impl Handler + Reporting {
    new_dispatcher()
        .at(&["example"], SimpleRendered("This resource exists inside a capsule"))
        .at(&["other_example"], SimpleRendered("Another ressource"))
}



export!(MyComponent);


#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}