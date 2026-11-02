#[cfg(feature = "log")]
pub mod log;

#[cfg(feature = "rng")]
pub mod rng;

#[cfg(feature = "time")]
pub mod time;

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "gpio")]
pub mod gpio;

#[cfg(feature = "sensors")]
pub mod sensors;


pub mod wasm_card_crypto;

// pub mod wasi_crypto;

#[derive(Default)]
pub struct ArielOSHost {
    #[cfg(feature = "rng")]
    rng_host: crate::wasm::rng::ArielRNGHost,

    #[cfg(feature = "udp")]
    udp_host: crate::wasm::udp::ArielUDPHost,

    #[cfg(feature = "gpio")]
    gpio_host: crate::wasm::gpio::ArielGpioHost,

    #[cfg(feature = "wasm-card")]
    wasm_card_host: crate::wasm::wasm_card_crypto::common::WasmCardHost,
}
