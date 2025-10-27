#![no_std]
#![feature(type_alias_impl_trait)]

use core::cell::RefCell;

use coap_message_implementations::inmemory::Message;
use coap_message_implementations::inmemory_write::GenericMessage;
use coap_handler_implementations::{new_dispatcher, SimpleRendered, HandlerBuilder};
use coap_handler::{Handler, Reporting};
use coap_handler::Record as _;

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

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

struct SendCell<T>(RefCell<T>);

/// Safety :
/// Wasm is single threaded
unsafe impl<T> Send for SendCell<T> {}
unsafe impl<T> Sync for SendCell<T> {}


use exports::ariel::wasm_bindings::coap_server_guest::{Guest, CoapErr};
use ariel::wasm_bindings::log_api::info;

struct MyComponent;

impl Guest for MyComponent {
    fn coap_run(code: u8, observed_len: u32, message: Vec<u8>) -> Result<(u8, Vec<u8>), CoapErr> {
        coap_run(code, observed_len, message)
    }

    fn initialize_handler() -> Result<(), ()> {
        initialize_handler()
    }

    fn report() -> Result<Vec<String>, CoapErr> {
        report_resource()
    }
}

type HandlerType = impl Handler + Reporting;
static HANDLER: SendCell<Option<HandlerType>> = SendCell(RefCell::new(None));


fn coap_run(mut code: u8, observed_len: u32,  mut message: Vec<u8>) -> Result<(u8, Vec<u8>), CoapErr> {
    let mut handler = HANDLER.0.borrow_mut();
    if handler.is_none() {
        return Err(CoapErr::HandlerNotBuilt);
    }
    let handler = handler.as_mut().unwrap();


    let reencoded = Message::new(code, &message[..observed_len as usize]);

    let extracted = match handler.extract_request_data(&reencoded) {
        Ok(ex) => ex,
        // Assume that if it failed it's because it wasn't found
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

#[define_opaque(HandlerType)]
fn initialize_handler() -> Result<(), ()> {
    match HANDLER.0.borrow_mut() {
        mut h if h.is_none() => {
            *h = Some(build_handler());
        },
        _ => {
            info("WARNING! The handler can only be built once");
        }
    }
    Ok(())
}

fn build_handler() -> impl Handler + Reporting {
    new_dispatcher()
                .at(&["example"], SimpleRendered("This resource exists inside a capsule"))
                .at(&["other_example"], SimpleRendered("Another ressource"))
                .at(&["inner", "third_example"], SimpleRendered("A deeper resource"))
}

fn report_resource() -> Result<Vec<String>, CoapErr> {
    let mut handler = HANDLER.0.borrow_mut();
    if handler.is_none() {
        return Err(CoapErr::HandlerNotBuilt);
    }
    let handler = handler.as_mut().unwrap();


    let mut resources = Vec::new();

    for record in handler.report() {
        // intersperse with "/";
        let mut complete_path = record.path().fold(String::new(), |a, b| a + b.as_ref() + "/");
        // remove the trailing "/";
        complete_path.truncate(complete_path.len() - 1);
        resources.push(complete_path);
    }


    Ok(resources)
}

export!(MyComponent);


#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}