use wasmtime::component::bindgen;
use wasmtime::component::{ResourceTable, Resource};

extern crate alloc;

use alloc::vec::Vec;

pub use k256::{
    sha2::{Digest, Sha256},
    PublicKey as E256pKey, Secp256k1, Scalar, ProjectivePoint, SecretKey as E256sKey,
};

// use crypto_common::Generate as _;

bindgen!({
    world: "wasm-card:crypto/ecdsa",
    path: "../../wit/",
    with: {
        "wasm-card:crypto/ecdsa-api.public-key":E256pKey,
        "wasm-card:crypto/ecdsa-api.private-key":E256sKey,
    }
});


pub use wasm_card::crypto::ecdsa_api::{
    Host, HostPrivateKey, HostPublicKey, add_to_linker, HostWithStore,
    PublicKey, PrivateKey
};

#[derive(Default)]
pub(crate) struct WasmCardHost {
    pub(crate) keys: ResourceTable,
}

impl Host for WasmCardHost {
    fn hash(&mut self, data: Vec<u8>,) -> Vec<u8> {
        let hasher = Sha256::default();
        Vec::from_iter(hasher.chain_update(data).finalize())
    }

    fn generate(&mut self) -> (Resource<PublicKey>, Resource<PrivateKey>) {
        let mut crypto_rng = ariel_os_random::crypto_rng();
        let private_key = E256sKey::random(&mut crypto_rng);
        let public_key = E256pKey::from_secret_scalar(&private_key.to_nonzero_scalar());

        let private_handle = self.keys.push(private_key).unwrap();
        let public_handle = self.keys.push(public_key).unwrap();

        (public_handle, private_handle)
    }
}

impl HostPrivateKey for WasmCardHost {
    fn drop(&mut self, rep: Resource<E256sKey>) -> wasmtime::Result<()> {
        self.keys.delete(rep).unwrap();
        Ok(())
    }

    fn print(&mut self, self_: Resource<PrivateKey>) {
        let key = self.keys.get(&self_);
        ariel_os_log::info!("{:?}", ariel_os_log::Debug2Format(&key));
    }
}

impl HostPublicKey for WasmCardHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256pKey>) -> wasmtime::Result<()> {
        self.keys.delete(rep).unwrap();
        Ok(())
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256pKey>,) -> () {
        let key = self.keys.get(&self_);
        ariel_os_log::info!("{:?}", ariel_os_log::Debug2Format(&key));
    }
}

use super::super::ArielOSHost;

impl Host for ArielOSHost {
    fn generate(&mut self,) -> (wasmtime::component::Resource<E256pKey>, wasmtime::component::Resource<E256sKey>,) {
        self.wasm_card_host.generate()
    }

    fn hash(&mut self, data: wasmtime::component::__internal::Vec<u8>,) -> wasmtime::component::__internal::Vec<u8> {
        self.wasm_card_host.hash(data)
    }
}

impl HostPrivateKey for ArielOSHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256sKey>) -> wasmtime::Result<()> {
        HostPrivateKey::drop(&mut self.wasm_card_host, rep)
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256sKey>,) -> () {
        HostPrivateKey::print(&mut self.wasm_card_host, self_);
    }
}

impl HostPublicKey for ArielOSHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256pKey>) -> wasmtime::Result<()> {
        HostPublicKey::drop(&mut self.wasm_card_host, rep)
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256pKey>,) -> () {
        HostPublicKey::print(&mut self.wasm_card_host, self_);
    }
}
