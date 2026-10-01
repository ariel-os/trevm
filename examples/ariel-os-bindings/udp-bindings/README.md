# UDP

An example showcase the UDP bindings provided by Ariel-OS-bindings.
Any component run through this example will be able to receive and send UDP packets by using the approriate functions.

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
