#!/bin/sh

# set -e


for p in ble-scanner ephemeral-no-bindings ephemeral-with-bindings gpio persistent-no-bindings persistent-with-bindings udp-bindings
do
    cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/${p}/Cargo.toml -o ./examples/${p}/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28
done

# These need fuel, and are usable also with 64bit native
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.cwasm --config payloads/.cargo/config.toml --fuel --toolchain +nightly-2026-04-28
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/async-bindings/Cargo.toml -o ./examples/async-bindings/payload.pulley64f.cwasm --config payloads/.cargo/config.toml --fuel --target pulley64 --toolchain +nightly-2026-04-28

cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/simple-updates-1/Cargo.toml -o ./examples/simple-updates/payload1.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/simple-updates-2/Cargo.toml -o ./examples/simple-updates/payload2.cwasm --config payloads/.cargo/config.toml --toolchain nightly-2026-04-28

cp examples/simple-updates/*.cwasm examples/insecure-updates/
cp examples/async-bindings/payload.cwasm examples/updatable-async/async-payload.cwasm
cp examples/async-bindings/payload.cwasm examples/suit-updatable/payload.cwasm

cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sensors/Cargo.toml -o examples/fake-sensor/payload.cwasm --config payloads/.cargo/config.toml --toolchain +nightly-2026-04-28

cd payloads/sandbox-no-bindings
FIB_NUM=10 cargo +nightly-2026-04-28 build --release --config ../.cargo/config.toml
mv target/wasm32v1-none/release/sandbox_no_bindings.wasm fib-10.wasm
FIB_NUM=30 cargo +nightly-2026-04-28 build --release --config ../.cargo/config.toml
mv target/wasm32v1-none/release/sandbox_no_bindings.wasm fib-30.wasm
cd ../..
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sandbox-no-bindings/fib-10.wasm -o examples/sandbox-no-bindings/fib-10.cwasm
cargo +nightly-2026-04-28 -Z script precompile_wasm.rs --path payloads/sandbox-no-bindings/fib-30.wasm -o examples/sandbox-no-bindings/fib-30.cwasm
rm payloads/sandbox-no-bindings/fib-10.wasm payloads/sandbox-no-bindings/fib-30.wasm
