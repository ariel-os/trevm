# Updatable async Capsule

## About

This example shows how to run and update a WebAssembly capsule which uses async function calls internally using treVM.

## How to run

Look [here](https://ariel-os.github.io/ariel-os/dev/docs/book/networking.html) for information about network configuration in Ariel OS.

```sh
# Example for running on the ESP32-C6-DevKitC-1 using wifi
CONFIG_WIFI_NETWORK=... CONFIG_WIFI_PASSWORD=... laze build -b espressif-esp32-c6-devkitc-1 -s coap-server-config-unprotected run
```

Once the server is set up, in another terminal, you can send a request to `PUT` another capsule by using
```sh
# add --credential client.diag to use secure connections
pipx run --spec 'aiocoap[oscore, prettyprint]' aiocoap-client coap://<Address of the server>/path/to/resource
```

For most resources, you will need to add `--credentials ./client.diag` to authorize the access.

It's possible to get the resources that are provided by the server by `GET`ting `.well-known/core`

This example has been tested on the following boards:
- ESP32-C6-DevKitC-1
