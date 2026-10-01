# gpio-wasm

## About

This example runs a capsule that implements a simple blinky by letting the capsule manage select GPIOs.

## How to run

```
laze build -b nrf52840dk run
```

This example relies on the structured board generation for Ariel OS to defined the peripherals corresponding to the LED and the button. As such, it will work on any board known by Ariel OS to provides a least an LED and a Button, assuming they have sufficient flash (at least 512KiB). This has been tested on the `nrf52840dk` and on the `nrf9160dk`.