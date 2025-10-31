# Comparing different Web Assembly Runtimes with Ariel OS

In this section we will look at the code size of the binary produced by compiling several WebAssembly runtimes on the `nrf52840dk` board using Ariel OS.

## Setup
### Wasm Module
The runtimes will be tested with this minimal WebAssembly Module
```rust
#![no_std]

#[link(wasm_import_module = "host")]
unsafe extern "C" {
    fn extra() -> u32;
}


#[unsafe(no_mangle)]
extern "C" fn add_with_extra(a: u32, b: u32) -> u32 {
    a + b + unsafe { extra() }
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo<'_>) -> ! {
    core::arch::wasm32::unreachable();
}
```
It will be compiled for the `wasm32v1-none` target using the following `.cargo/config.toml`
```toml
[unstable]
build-std = ["core", "alloc", "panic_abort"]
build-std-features = ["optimize_for_size", "panic_immediate_abort"]

[build]
rustflags = [
    "-Z", "location-detail=none",
    "-C", "link-arg=--initial-memory=65356",
    "-C", "link-arg=-zstack-size=4096"
]
target = "wasm32v1-none"

[profile.release]
opt-level= "z"
codegen-units = 1
lto = "fat"
debug = false
strip = "symbols"
```

For wasmtime, since it doesn't support raw wasm bytecode in `#![no_std]` contexts, it will need to be precompiled. The following configuration is used

```rust
use wasmtime::{Config, Engine, OptLevel};

fn main() -> wasmtime::Result<()> {
    let mut config = Config::new();

    // Options that were found to reduce code size
    config.memory_init_cow(false);
    config.generate_address_map(false);
    config.table_lazy_init(false);
    config.cranelift_opt_level(OptLevel::Speed);

    // 0 means limiting ourselves to what the module asked
    // This needs to be set at pre-compile time to use it at runtime
    config.memory_reservation(0);

    // Disabling this allows runtime optimizations but means that the maximum memory
    // that the module can have is
    // S = min(initial_memory, memory_reservation) + memory_reserver_for_growth
    // since it can grow by reallocating.
    config.memory_may_move(false);

    let engine = Engine::new(&config)?;

    let wasm = include_bytes!("/path/to/input.wasm");

    let precompiled = engine.precompile_module(wasm)?;

    std::fs::write("input.cwasm", &precompiled).unwrap();

    Ok(())
}
```

### Runtimes tested

- [Wasmtime](github.com/bytecodealliance/wasmtime)
- [Wasmi](https://github.com/wasmi-labs/wasmi)
- [Wasm-interpreter](https://github.com/DLR-FT/wasm-interpreter)

We originally wanted to also evaluate [`wasm3`](https://github.com/wasm3/wasm3) and [`wamr`](github.com/bytecodealliance/wasm-micro-runtime) through their rust bindings but were unable to compile the underlying C code.

### Boards considered

This was tested on the `nrf52840dk`.

## Size Comparisons

The exact code that was compiled is present in [this repository](https://github.com/anlavandier/ariel-runtime-size-comparisons). Below are the results in bytes of the sizes of different sections of the compiled ELF, the size of the crates that are pulled by runtime, evaluated using `cargo-bloat` and the size that is flashed to the device as reported by `probe-rs`.

|            | wasmi     | wasmtime  | wasm-interpreter |
| ---------- | --------- | --------- | ---------------- |
| `.text`    | 511,300 B | 264,548 B | 162,272 B        |
| `.bss`     | 5,676 B   | 5,764 B   | 5,668 B          |
| `.rodata`  | 79,544 B  | 54,916 B  | 16,940 B         |
| `--crates` | 427,300 B | 175,300 B | 121,400 B        |
| `probe-rs` | 580,000 B | 316,000 B | 176,000 B        |

## Features Comparisons

[Wasmtime](github.com/bytecodealliance/wasmtime) is the flagship runtime in the rust ecosystem. It is developped by the Bytecode Alliance which is consortium that is involved with the development of new Web Assembly standards and proposals. As such, it supports by far the most features including the [WebAssembly Component Model](https://component-model.bytecodealliance.org/introduction.html) and asynchronous execution.

[Wasmi](https://github.com/wasmi-labs/wasmi) tries to comply with the Wasmtime API but only supports synchronous execution of regular Wasm modules.

[Wasm-interpreter](https://github.com/DLR-FT/wasm-interpreter) is still not published on [crates.io](crates.io) and only supports the bare-minimum and is barely documented.

## Conclusion

[Wasmi](https://github.com/wasmi-labs/wasmi) seems to be objectively worse as it has a bigger code size and less features. [Wasm-interpreter](https://github.com/DLR-FT/wasm-interpreter) is very promising but lacks too many features and it's unclear if something like the Component Model is planned at all. In the meantime, it seems like [Wasmtime](github.com/bytecodealliance/wasmtime) is the runtime that is best suited for us.

The as-of-yet closed source [Myrmic runtime](https://myrmic.org/) could be interesting and will need to be tested once it becomes open-source in early 2026.
