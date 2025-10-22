# wasm-bindings-async

An example showing how ariel can bind asynchronous functions to a wasm component

## Details

The wit files defining the interface of the bindings and the contents of the component as in [the wit directory](../../wit/). The rust code used to create the component is in [payloads/wasm-bindings-async](../../payloads/wasm-bindings-async/). It can be modified and then recompiled using the provided script `precompiled_wasm.rs`. See [here](../../README.md#compiling-or-recompiling-payloads) for more information.

## Shortcomings


## How to run

In this directory, run

    laze build -b nrf52840dk run

This example has only been tested using `network-config-static` which uses the hardcoded `10.41.0.61/24` IP adress.
Look [here](../README.md#networking) for more information about network configuration.

This example has also been tested on the `rpi-pico2-w` board.