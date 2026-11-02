use wasmtime::component::bindgen;
use wasmtime::component::{ResourceTable, Resource};

extern crate alloc;

use alloc::vec::Vec;

pub use k256::{
    sha2::{Digest, Sha256},
    PublicKey as E256pKey, Secp256k1, Scalar as E256Scalar, ProjectivePoint, SecretKey as E256sKey,
    elliptic_curve::ff::PrimeField,
};


// use crypto_common::Generate as _;

bindgen!({
    world: "wasm-card:crypto/crypto-extension-impl",
    path: "../../wit/",
    with: {
        "wasm-card:crypto/crypto-extension.public-key":E256pKey,
        "wasm-card:crypto/crypto-extension.secret-key":E256sKey,
        "wasm-card:crypto/crypto-extension.scalar": E256Scalar,
    }
});


pub use wasm_card::crypto::crypto_extension::{
    Host,
    HostPublicKey,
    HostSecretKey,
    HostScalar,
    HostWithStore,
    add_to_linker,
};

#[derive(Default)]
pub(crate) struct WasmCardHost {
    pub(crate) keys: ResourceTable,
    pub(crate) scalars: ResourceTable,
}

impl Host for WasmCardHost {
    fn hash(&mut self, data: Vec<u8>,) -> Vec<u8> {
        let hasher = Sha256::default();
        Vec::from_iter(hasher.chain_update(data).finalize())
    }

    fn generate(&mut self) -> (Resource<E256pKey>, Resource<E256sKey>) {
        let mut crypto_rng = ariel_os_random::crypto_rng();
        let private_key = E256sKey::random(&mut crypto_rng);
        let public_key = E256pKey::from_secret_scalar(&private_key.to_nonzero_scalar());

        let private_handle = self.keys.push(private_key).unwrap();
        let public_handle = self.keys.push(public_key).unwrap();

        (public_handle, private_handle)
    }

    fn into_scalar(&mut self, secret_key: Resource<E256sKey>) -> Resource<E256Scalar> {
        let key = self.keys.get(&secret_key).unwrap();
        let scalar = key.to_nonzero_scalar();
        self.scalars.push(*scalar).unwrap()
    }

    fn curve_mult(&mut self, scalar: Resource<E256Scalar>) -> Resource<E256Scalar> {
        let scalar = self.scalars.get(&scalar).unwrap();
        let generator = <E256Scalar as PrimeField>::MULTIPLICATIVE_GENERATOR;
        let new_scalar = *scalar * generator;
        self.scalars.push(new_scalar).unwrap()
    }

    fn scalar_mult(&mut self, a: Resource<E256Scalar>, b: Resource<E256Scalar>) -> Resource<E256Scalar> {
        let a = self.scalars.get(&a).unwrap();
        let b = self.scalars.get(&b).unwrap();
        self.scalars.push(*a * *b).unwrap()
    }

    fn scalar_sub(&mut self, a: Resource<E256Scalar>, b: Resource<E256Scalar>) -> Resource<E256Scalar> {
        let a = self.scalars.get(&a).unwrap();
        let b = self.scalars.get(&b).unwrap();
        self.scalars.push(*a - *b).unwrap()
    }

}

impl HostSecretKey for WasmCardHost {
    fn drop(&mut self, rep: Resource<E256sKey>) -> wasmtime::Result<()> {
        self.keys.delete(rep).unwrap();
        Ok(())
    }

    fn print(&mut self, self_: Resource<E256sKey>) {
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

impl HostScalar for WasmCardHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256Scalar>) -> wasmtime::Result<()> {
        self.scalars.delete(rep).unwrap();
        Ok(())
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256Scalar>) {
        let scalar = self.scalars.get(&self_);
        ariel_os_log::info!("{:?}", ariel_os_log::Debug2Format(&scalar));
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

    fn curve_mult(&mut self, scalar: wasmtime::component::Resource<E256Scalar>,) -> wasmtime::component::Resource<E256Scalar> {
        self.wasm_card_host.curve_mult(scalar)
    }

    fn into_scalar(&mut self, secret_key: wasmtime::component::Resource<E256sKey>,) -> wasmtime::component::Resource<E256Scalar> {
        self.wasm_card_host.into_scalar(secret_key)
    }

    fn scalar_mult(&mut self, a: wasmtime::component::Resource<E256Scalar>, b: wasmtime::component::Resource<E256Scalar>,) -> wasmtime::component::Resource<E256Scalar> {
        self.wasm_card_host.scalar_mult(a, b)
    }

    fn scalar_sub(&mut self, a: wasmtime::component::Resource<E256Scalar>, b: wasmtime::component::Resource<E256Scalar>,) -> wasmtime::component::Resource<E256Scalar> {
        self.wasm_card_host.scalar_sub(a, b)
    }
}

impl HostSecretKey for ArielOSHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256sKey>) -> wasmtime::Result<()> {
        HostSecretKey::drop(&mut self.wasm_card_host, rep)
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256sKey>,) -> () {
        HostSecretKey::print(&mut self.wasm_card_host, self_);
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

impl HostScalar for ArielOSHost {
    fn drop(&mut self, rep: wasmtime::component::Resource<E256Scalar>) -> wasmtime::Result<()> {
        HostScalar::drop(&mut self.wasm_card_host, rep)
    }

    fn print(&mut self, self_: wasmtime::component::Resource<E256Scalar>,) -> () {
        HostScalar::print(&mut self.wasm_card_host, self_);
    }
}
