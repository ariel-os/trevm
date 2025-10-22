# wasm-coap-interop

## About

This Examples shows how to run a coap-server through a wasm capsule by deferring the processing of selects message (such as all those asking for a resource under `/vm`) to the capsule.

## Security

As of today (22.10.25), security using the laze module `coap-server-config-demokeys` is untested. Testing and stabilization of this is planned and expcted to take some minor changes in `ariel-os-bindings`.

## How to run

Look [here](../README.md#networking) for information about network configurationin Ariel OS.

```sh
# Example for running on the RP Pico 2 W using wifi
CONFIG_WIFI_NETWORK=... CONFIG_WIFI_PASSWORD=... laze build -b rpi-pico-2-w -s wifi-cyw43 -s coap-server-config-unprotected run
```

This example has been tested on the following boards:
- NRF52840DK using the `usb-ethernet` and `network-config-ipv4-static` modules.
- RPI Pico 2 W using the `wifi-cyw43` and `network-config-ipv4-dhcp` modules.
- DFRobot FireBeetle 2 using the `espressif-esp32-c6-devkitc-1` builder and the `wifi-esp` and `network-config-ipv4-dhcp` modules.
