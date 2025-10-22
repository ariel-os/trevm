use ariel_os_random::{FastRngSend, fast_rng_send};

extern crate alloc;

use alloc::vec::Vec;
use rand_core::RngCore as _;

use wasmtime::component::{bindgen, Resource};

bindgen!({
    world: "ariel:wasm-bindings/rng",
    path: "../../wit/",
});

pub use ariel::wasm_bindings::rng_api::{Host, HostWithStore, add_to_linker, HostRNG, RNG};

pub struct ArielRNGHost {
    rng: FastRngSend
}

impl Default for ArielRNGHost {
    fn default() -> Self {
        Self { rng: fast_rng_send() }
    }
}

impl HostRNG for ArielRNGHost {

    fn next_u32(&mut self,) -> u32 {
        self.rng.next_u32()
    }
    fn next_u64(&mut self,) -> u64 {
        self.rng.next_u64()
    }
    fn random_bytes(&mut self,len:u32,) -> Vec<u8> {
        let mut dest: Vec<u8> = core::iter::repeat_n(0, len as usize).collect();
        self.rng.fill_bytes(&mut dest);
        dest
    }
    fn drop(&mut self, _: Resource<RNG>) -> wasmtime::Result<()> {
        unreachable!("Should never be dropped since it's never instantiated")
    }
}

impl Host for ArielRNGHost {}
