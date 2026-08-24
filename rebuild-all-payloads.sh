#!/bin/sh

set -e


for p in ble-scanner ephemeral-no-bindings ephemeral-with-bindings gpio persistent-no-bindings persistent-with-bindings udp-bindings
do
    cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/${p}/Cargo.toml -o ./examples/${p}/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-01-20
done

# These need fuel, and are usable also with 64bit native
cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.cwasm --config payloads/.cargo/config.toml --fuel --toolchain +nightly-2026-01-20
cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.pulley64f.cwasm --config payloads/.cargo/config.toml --fuel --target pulley64 --toolchain +nightly-2026-01-20

cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/simple-updates-1/Cargo.toml -o ./examples/simple-updates/payload1.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-01-20
cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/simple-updates-2/Cargo.toml -o ./examples/simple-updates/payload2.cwasm --config payloads/.cargo/config.toml --toolchain nightly-2026-01-20

cp examples/simple-updates/*.cwasm examples/insecure-updates/
cp examples/async-bindings/payload.cwasm examples/updatable-async/async-payload.cwasm
cp examples/async-bindings/payload.cwasm examples/suit-updatable/payload.cwasm

cargo +nightly-2026-01-20 -Z script precompile_wasm.rs --path payloads/sensors/Cargo.toml -o examples/fake-sensor/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-01-20
