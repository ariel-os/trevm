#!/bin/sh

set -e

# ariel-os-bindings-example
for p in ble-scanner gpio udp-bindings
do
    cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/${p}/Cargo.toml -o ./examples/ariel-os-bindings/${p}/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28
done

cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sensors/Cargo.toml -o examples/ariel-os-bindings/fake-sensor/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28

# These need fuel, and are usable also with 64bit native
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/ariel-os-bindings/async-bindings/payload.cwasm --config payloads/.cargo/config.toml --fuel --toolchain +nightly-2026-04-28
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/ariel-os-bindings/async-bindings/payload.pulley64f.cwasm --config payloads/.cargo/config.toml --fuel --target pulley64 --toolchain +nightly-2026-04-28

cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/simple-updates-1/Cargo.toml -o ./examples/ariel-os-bindings/simple-updates/payload1.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/simple-updates-2/Cargo.toml -o ./examples/ariel-os-bindings/simple-updates/payload2.cwasm --config payloads/.cargo/config.toml --toolchain nightly-2026-04-28

cp examples/ariel-os-bindings/simple-updates/*.cwasm examples/ariel-os-bindings/insecure-updates/
cp examples/ariel-os-bindings/async-bindings/payload.cwasm examples/ariel-os-bindings/updatable-async/async-payload.cwasm


# trevm examples
for p in ephemeral-no-bindings ephemeral-with-bindings persistent-no-bindings persistent-with-bindings
do
    cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/${p}/Cargo.toml -o ./examples/trevm/${p}/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28
done

cd payloads/sandbox-no-bindings
FIB_NUM=10 cargo +nightly-2026-04-28 build --release --config ../.cargo/config.toml
mv target/wasm32v1-none/release/sandbox_no_bindings.wasm fib-10.wasm
FIB_NUM=30 cargo +nightly-2026-04-28 build --release --config ../.cargo/config.toml
mv target/wasm32v1-none/release/sandbox_no_bindings.wasm fib-30.wasm
cd ../..
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sandbox-no-bindings/fib-10.wasm -o examples/trevm/sandbox-no-bindings/fib-10.cwasm
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sandbox-no-bindings/fib-30.wasm -o examples/trevm/sandbox-no-bindings/fib-30.cwasm
rm payloads/sandbox-no-bindings/fib-10.wasm payloads/sandbox-no-bindings/fib-30.wasm

# sure-vm example
cp examples/ariel-os-bindings/async-bindings/payload.cwasm examples/sure-vm/suit-updatable/payload.cwasm
