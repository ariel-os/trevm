#![no_std]

extern crate alloc;

pub mod handler;

pub mod suit;

pub mod capsule_traits {
    pub use trevm::{CanInstantiate, EphemeralCapsule, PersistentCapsule};
}
