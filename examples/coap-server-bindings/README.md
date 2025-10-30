# wasm-coap-interop

## About

This example shows how to run a coap-server through a wasm capsule by deferring the processing of selects message (such as all those asking for a resource under `/vm`) to the capsule.

## Security

Secure connections can be enabled by using the `-s coap-server-config-storage`. This will read and apply the permissions set in [`peers.yml`](./peers.yml). At startup, you should see a log looking like
```sh
[INFO ] CoAP server identity: {8: ... }
```
<p style="color:red"> <b>Note :</b> </p>

This will not work on the Pico boards because of a peripheral conflict between the wifi and storage modules. To still use them together, change `DMA_CH0` to `DMA_CH1` in [this file](../../build/imports/ariel-os/src/ariel-os-rp/src/storage.rs). Note that this link only works after `laze` has download the ariel-os repository in `build/imports/ariel-os`.

You should replace `{2: "", 8: ...}` by the server identity in the `peer_creed` field of [`client.diag`](./client.diag) before using it in client calls.

Note: OSCORE requires CoAP options to be sorted which is not currently guaranteed by our handler implementations. This could case the server to crash in some scenarios. In our testing, the worse we could achieve was to simply get a message rejected when it shouldn't have. In the current state, achieving this requires contrived examples which is why we still choose to showcase security options.

## How to run

Look [here](../README.md#networking) for information about network configuration in Ariel OS.

```sh
# Example for running on the RP Pico 2 W using wifi
CONFIG_WIFI_NETWORK=... CONFIG_WIFI_PASSWORD=... laze build -b rpi-pico-2-w -s wifi-cyw43 -s coap-server-config-unprotected run
```

Once the server is set-up, in another terminal, you can send request by using
```sh
# optionally add --credential client.diag to use secure connections
pipx run --spec 'aiocoap[oscore, prettyprint]' aiocoap-client coap://<Address of the server>/vm/example
```

It's possible to get the resource that are provided by the server by `GET`ting `.well-known/core`

This example has been tested on the following boards:
- NRF52840DK using the `usb-ethernet` and `network-config-ipv4-static` modules.
- RPI Pico 2 W using the `wifi-cyw43` and `network-config-ipv4-dhcp` modules.
- DFRobot FireBeetle 2 using the `espressif-esp32-c6-devkitc-1` builder and the `wifi-esp` and `network-config-ipv4-dhcp` modules.
