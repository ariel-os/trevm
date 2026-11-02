# Wasm Card example

An example of running crypto operations in a wasm capsule.

## Details

The API that is made available to the wasm capsule is found [here](../../wit/deps/wasm-card/crypto-common.wit). The payload contained in the capsule is found [here](../../payloads/wasm-card/). The implementation of the API is found [here](../../src/ariel-os-bindings/src/wasm/wasm_card_crypto/common.rs).

See [#Workflow](../../README.md#workflow) to learn how to compile and precompile the payloads. To run on native, the payload needs to be pre-compiled with `--target pulley64`.

## How to run

In this directory, run
```
laze build -b native run
```
This example has also been tested on the `nrf52840dk` board.
