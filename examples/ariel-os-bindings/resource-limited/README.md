# Resource-limiting example

This example demonstrates using Wasmtime's resource limiting capabilities with treVM for async capsules.

Limiting memory usage is done with [Wasmtime's `ResourceLimiter`](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html). We limit to 40,000 bytes of memory used, further memory growing is rejected.
This especially requires dynamic memory allocation, which is not provided by the allocator Talc in combination with Wasm's custom page sizes proposal, so this payload uses a custom [fork of Talc](https://github.com/COLORADIO-Project/talc) which supports this.

Limiting CPU usage is done via [fuel consumption](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html#method.consume_fuel), as described in the documentation for [`ResourceLimiter`](https://docs.rs/wasmtime/latest/wasmtime/trait.ResourceLimiter.html).

CPU usage is limited to 10,000,000 units of Wasmtime fuel and the runtime is configured so capsule execution yields every 5,000 units of fuel consumed. This can be observed with the feature `monitor-heap` (which adds an async Ariel OS task with regular logs). With regular yields turned on, heap usage is regularly logged during capsule execution. Without regular yields, this is not the case.

The [`resource-limited` payload](../../payloads/resource-limited/) used for this example attempts to indefinitely fill a `Vec` with random integers and logs this to the console. When further allocation fails due to the memory growing being rejected by the ResourceLimiter, the capsule crashes.
