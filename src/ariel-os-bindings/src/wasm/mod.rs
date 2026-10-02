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

#[derive(Default)]
pub struct ArielOSHost {
    #[cfg(feature = "rng")]
    rng_host: crate::wasm::rng::ArielRNGHost,

    #[cfg(feature = "udp")]
    udp_host: crate::wasm::udp::ArielUDPHost,

    #[cfg(feature = "gpio")]
    gpio_host: crate::wasm::gpio::ArielGpioHost,
}
