extern crate alloc;
use alloc::{string::String, vec::Vec};

use wasmtime::component::{Component, Linker};
use wasmtime::{Result as wasm_result, Store};

pub use coap_message_utils::Error as CoAPError;

pub trait EphemeralCapsule<T, R>: CanInstantiate<T> {
    /// Runs a function and returns a result
    fn run(&mut self, store: &mut Store<T>) -> wasm_result<R>;
}

pub trait PersistentCapsule<T>: CanInstantiate<T> {
    type E: Into<CoAPError>;
    fn coap_run(
        &mut self,
        store: &mut Store<T>,
        code: u8,
        observed_len: u32,
        message: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), Self::E>;

    fn initialize_handler(&mut self, store: &mut Store<T>) -> wasm_result<()>;

    fn report_resources(&mut self, store: &mut Store<T>) -> Result<Vec<String>, Self::E>;
}

/// Glue layer that allows a generic backend to operate on any concrete bindgen type.
///
/// Open questions:
/// * Could this be part of (or interdependent with) PersistentCapsule?
/// * Do we need it to be a trait in the first place? (Maybe all sensible applications that can use
///   this module have to use the single bindgen output anyway, and thus all the bindgen could move
///   into this module.)
pub trait CanInstantiate<T> {
    /// Runs Self::add_to_linker and Self::instantiate (which are bindgen generated methods without
    /// a type)
    fn instantiate(
        linker: &mut Linker<T>,
        store: &mut Store<T>,
        component: Component,
    ) -> wasm_result<Self>
    where
        Self: Sized;
}
