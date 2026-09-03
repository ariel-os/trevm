extern crate alloc;

use super::ArielOSHost;
use ariel_os_hal::gpio::{IntEnabledInput, Output};

use wasmtime::component::bindgen;

bindgen!({
    world: "ariel:wasm-bindings/gpio",
    path: "../../wit/",
    imports: {
        "ariel:wasm-bindings/gpio-api.wait-for-button-low": async
    }
});

pub use ariel::wasm_bindings::gpio_api::{Host, HostWithStore, add_to_linker};

#[derive(Default)]
pub(crate) struct ArielGpioHost {
    pub(crate) led: Option<Output<'static>>,
    pub(crate) button: Option<IntEnabledInput<'static>>,
}

impl Host for ArielGpioHost {
    fn toggle_led(&mut self) -> Result<(), ()> {
        self.led.as_mut().map(|led| led.toggle()).ok_or(())
    }

    async fn wait_for_button_low(&mut self) -> Result<(), ()> {
        match self.button.as_mut() {
            Some(b) => Ok(b.wait_for_low().await),
            None => Err(()),
        }
    }
}

impl Host for ArielOSHost {
    fn toggle_led(&mut self) -> Result<(), ()> {
        self.gpio_host.toggle_led()
    }

    async fn wait_for_button_low(&mut self) -> Result<(), ()> {
        self.gpio_host.wait_for_button_low().await
    }
}

impl ArielOSHost {
    pub fn bind_led(&mut self, led: Output<'static>) {
        self.gpio_host.led = Some(led);
    }
    pub fn bind_button(&mut self, button: IntEnabledInput<'static>) {
        self.gpio_host.button = Some(button);
    }
}
