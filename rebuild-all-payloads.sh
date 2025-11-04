#!/bin/sh

set -e


for p in coap-server-bindings gpio udp-bindings
do
    ./precompile_wasm.rs --path payloads/${p}/Cargo.toml -o ./examples/${p}/payload.cwasm --config payloads/.cargo/config.toml
done

# These need fuel, and are usable also with 64bit native
./precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.cwasm --config payloads/.cargo/config.toml --fuel
./precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.pulley64f.cwasm --config payloads/.cargo/config.toml --fuel --target pulley64

./precompile_wasm.rs --path payloads/simple-updates-1/Cargo.toml -o ./examples/simple-updates/payload1.cwasm --config payloads/.cargo/config.toml
./precompile_wasm.rs --path payloads/simple-updates-2/Cargo.toml -o ./examples/simple-updates/payload2.cwasm --config payloads/.cargo/config.toml

cp examples/simple-updates/*.cwasm examples/insecure-updates/
