#![no_std]

#[global_allocator]
static ALLOCATOR: talc::Talck<talc::locking::AssumeUnlockable, talc::ClaimOnOom> = {
    static mut MEMORY: [u8; 0x4000] = [0; 0x4000]; // 16KiB of memory
    let span = talc::Span::from_array((&raw const MEMORY).cast_mut());
    talc::Talc::new(unsafe { talc::ClaimOnOom::new(span) }).lock()
};

use wit_bindgen::generate;

extern crate alloc;

generate!({
    world: "example-ble-scanner",
    path: "../../wit",
    generate_all,
});

use alloc::format;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;

use ariel::wasm_bindings::log_api::info;
use exports::ariel::wasm_bindings::ble_api::{BdAddr, Guest};
struct MyComponent;

/// SAFETY: WASM is single threaded
pub struct SendCell<T>(RefCell<T>);
unsafe impl<T> Send for SendCell<T> {}
unsafe impl<T> Sync for SendCell<T> {}

static SEEN: SendCell<BTreeMap<[u8; 6], u64>> = SendCell(RefCell::new(BTreeMap::new()));


impl core::fmt::Display for BdAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.f,
            self.e,
            self.d,
            self.c,
            self.b,
            self.a,
        )
    }
}

impl BdAddr {
    fn into_inner(&self) -> [u8; 6] {
        let mut res = [0; 6];
        res[0] = self.a;
        res[1] = self.b;
        res[2] = self.c;
        res[3] = self.d;
        res[4] = self.e;
        res[5] = self.f;
        res
    }

    fn new(val: [u8; 6]) -> Self {
        let mut res = BdAddr::default();
        res.a = val[0];
        res.b = val[1];
        res.c = val[2];
        res.d = val[3];
        res.e = val[4];
        res.f = val[5];
        res
    }
}

impl Default for BdAddr {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            f: 0,
        }
    }
}


impl Guest for MyComponent {
    fn on_single_report(addr: BdAddr) -> Result<(), ()> {
        let mut addr_collection = SEEN.0.borrow_mut();
        let count = addr_collection.entry(addr.into_inner()).or_default();
        if *count == 0 {
            let discovered = format!("discovered: {}", addr);
            info(&discovered);
        }
        *count += 1;
        Ok(())
    }

    fn return_stats() -> Vec<(BdAddr, u64)> {
        SEEN.0.borrow().iter().map(|(addr, count)| { (BdAddr::new(*addr), *count) }).collect()
    }
}

export!(MyComponent);

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable();
}
