# Insecure Updates

This example runs payloads received over UDP and runs them while checking for potential updates over UDP.

## Details

The wit files defining the interface of the bindings and the contents of the component as in [the wit directory](../../wit/).
The payloads have all comply to the wit file that is loaded at compile time.
The "protocol" for sending the files is very simple and not secured at all. A simple `send_file.rs` script is provided with the default IP and port already configured.

## How to run

In this directory, run
```
laze build -b nrf52840dk -s network-config-ipv4-static -d network-config-ipv4-dhcp -s usb-ethernet run
```
After setting up the networking (See [here](https://ariel-os.github.io/ariel-os/dev/docs/book/networking.html) for more info) you can see a payload with
```
./send_files.rs -z payload1.cwasm
```
