# SUIT updatable Capsule

## About

This example shows how to update an async WebAssembly capsule over CoAP using a signed SUIT manifest.

The update is kept in memory only. It replaces the currently running capsule for the lifetime of the process, but it is not written to flash and does not survive a reboot.

For lower peak memory usage, the example drops the currently running capsule after the SUIT envelope has been authenticated, but before the new `payload.cwasm` is fetched and validated. If the fetch or validation fails, the runner will be left without a loaded capsule until another valid update is sent.

## Requirements

The commands in this example require:

- Arm's `suit-tool`: https://gitlab.arm.com/research/ietf-suit/suit-tool
- aiocoap's command-line tools: `aiocoap-fileserver`, `aiocoap-client` 

## How to run

All commands below are intended to be run from the example root directory.

Run the ESP:

```sh
CONFIG_WIFI_NETWORK=... CONFIG_WIFI_PASSWORD=... laze build -b espressif-esp32-c6-devkitc-1 -s coap-server-config-unprotected run
```

Build the update payload:

```sh
cargo +nightly -Z script ../../precompile_wasm.rs --path ../../payloads/async-bindings/Cargo.toml --config ../../payloads/.cargo/config.toml -o payload.cwasm --fuel
```

Edit `suit/manifest.json` and set the `uri` field to the host serving `payload.cwasm`, for example:

```json
"uri": "coap://192.168.1.100:5683/payload.cwasm"
```

For local testing, the example accepts the same `manifest-sequence-number` more than once and logs a warning. Lower sequence numbers are rejected while the board is running. 

Regenerate and sign the SUIT manifest:

```sh
suit-tool create -i suit/manifest.json -o suit/manifest.suit

suit-tool sign -m suit/manifest.suit -k suit/demo-private-key.pem -o suit/manifest.signed.suit
```

Ensure that the host firewall allows inbound UDP traffic on port 5683. 
Then serve the update payload:

```sh
aiocoap-fileserver .
```

Send the signed manifest to the board:

```sh
aiocoap-client -m PUT --payload @suit/manifest.signed.suit coap://<board-address>/vm-control
```

The key pair in `suit/` is for this example only. Firmware verification uses `suit/demo-public-key-p256.bin`.

## Testing

This example has been tested on the following boards:

- `espressif-esp32-c6-devkitc-1`
- `dfrobot-firebeetle2-esp32-c6`
- `rpi-pico2-w`
