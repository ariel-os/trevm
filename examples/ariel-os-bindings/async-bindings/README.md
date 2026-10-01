# Async-bindings

An example showing how ariel can asynchronously run wasm components that yield regularly and that call asynchronous host functions.

## Details

The wit files defining the interface of the bindings and the contents of the component as in [the wit directory](../../wit/). The rust code used to create the component is in [payloads/async-bindings](../../payloads/async-bindings/). It can be modified and then recompiled using the provided script `precompiled_wasm.rs`. See [here](../../README.md#compiling-or-recompiling-payloads) for more information.


## How to run

In this directory, run
```
laze build -b nrf52840dk run
```

This example has also been tested on the `rpi-pico2-w` board. This example will not work on RISCV 32 bits MCUs (such as the ESP32-C6) because wasmtime, the WebAssembly runtime that this example use, doesn't support asynchronous execution of components on such architectures.