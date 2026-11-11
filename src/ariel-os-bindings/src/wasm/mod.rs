#[cfg(feature = "resource-limiting")]
use alloc::boxed::Box;

#[cfg(feature = "resource-limiting")]
use wasmtime::ResourceLimiter;

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

#[cfg(feature = "resource-limiting")]
mod resource_limiting;

pub struct ArielOSHost {
    #[cfg(feature = "rng")]
    rng_host: crate::wasm::rng::ArielRNGHost,

    #[cfg(feature = "udp")]
    udp_host: crate::wasm::udp::ArielUDPHost,

    #[cfg(feature = "gpio")]
    gpio_host: crate::wasm::gpio::ArielGpioHost,

    #[cfg(feature = "resource-limiting")]
    pub limiter: Box<dyn ResourceLimiter>,
}

impl ArielOSHost {
    #[cfg(feature = "resource-limiting")]
    pub fn new(limiter: Box<dyn ResourceLimiter>) -> Self {
        Self {
            limiter: limiter,
            ..Self::default()
        }
    }
}

impl Default for ArielOSHost {
    fn default() -> Self {
        Self {
            #[cfg(feature = "rng")]
            rng_host: crate::wasm::rng::ArielRNGHost::default(),

            #[cfg(feature = "udp")]
            udp_host: crate::wasm::udp::ArielUDPHost::default(),

            #[cfg(feature = "gpio")]
            gpio_host: crate::wasm::gpio::ArielGpioHost::default(),

            #[cfg(feature = "resource-limiting")]
            limiter: Box::new(crate::wasm::resource_limiting::DefaultResourceLimiter),
        }
    }
}
