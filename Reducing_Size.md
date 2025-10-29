# Reducing the size of the payload

In the following sections we consider how different (pre-)compilation options affect the final size of the payload that we have to either embed in our firmware at built or send at runtime over the wire or over the air. The payload considered will be the one from [this crate](https://github.com/ariel-os/trevm/payloads/coap-server-bindings) which implements a CoAP server handler in a Wasm capsule.

## Rust to .wasm compilation

Everything was tested with
```shell
$ rustc +nightly -vV
rustc 1.92.0-nightly (7c275d09e 2025-09-18)
binary: rustc
commit-hash: 7c275d09ea6b953d2cca169667184a7214bd14c7
commit-date: 2025-09-18
host: x86_64-unknown-linux-gnu
release: 1.92.0-nightly
LLVM version: 21.1.1
```
Here is the initial `Cargo.toml` and `.cargo/config.toml` configuration
```toml
[profile.release]
opt-level = 3
codegen-units = 16
lto = "thin"
debug = false
```

```toml
[build]
rustflags = [
    "-C", "link-arg=--page-size=1",
    "-C", "link-arg=--initial-memory=32768",
    "-C", "link-arg=-zstack-size=4096",
]
target = "wasm32v1-none"
```

The following variables will be tested in isolation and then when all combined
- `opt-level` with `"s"` and `"z"`
- `lto = "fat"`
- `codegen-units = 1`
- `strip = true`
- `-Z build-std-features` with `panic_immediate_abort` and `optimize_for_size`
- `-Zlocation-detail=none`

In the following tables, the size is measured with `wc -c` and reported in bytes. This means that the reported size includes debug information when they exist. The difference between the default and the explored options is also reported as a percentage of the default. So for example if the default is 100 B and an option reduced it to 75B the difference would be -25%.

| Default Settings | `opt-level = 's'` | `opt-level = 'z'` | `lto = "fat"`   | `codegen-units = 1` | `strip = symbols` |
| ---------------- | ----------------- | ----------------- | --------------- | ------------------- | ----------------- |
| 53,095 B         | 41,630 B (-22%)   | 45,757 B (-14%)   | 20,872 B (-61%) | 49,073 B (-8%)      | 47,274 B (-11%)   |



| Default Settings | `panic_immediate_abort` | `optimize_for_size` | `-Zlocation-detail=none` |
| ---------------- | ----------------------- | ------------------- | ------------------------ |
| 53,095 B         | 44,331 B (-17%)         | 52,375 B (-1%)      | 52,159 B (-2%)           |


Using `opt-level = 's'` and every other option enabled we go down to `14415` bytes or a difference of -73%. Using `opt-level = 'z'` instead yields `13660` bytes or a difference of -74%. As shown here using there isn't a clear best optimization level between `'s'` and `'z'`. Case by case testing needs to be done to get the smallest code size.


## Precompilation using Wasmtime
Wasmtime does not interpret raw wasm modules and instead compiles the incoming module either to native code through the Cranelift or Winch compilers or to Pulley bytecode, Pulley being an interpreter that allows wasmtime to work on architectures where native code compilation is not available which is the case for our 32 bit MCUs. See [`Precompiling Wasm`](https://docs.wasmtime.dev/examples-pre-compiling-wasm.html) from the Wasmtime book for more general information. Since this pre-compilation step can't be done in the firmware it is fairly easy to measure the impact of this step directly through the size of the resulting ELF file.

In this section we will look at the final size of the ELF produced and how it's influence by changing configuration options through the [`Config`](https://docs.rs/wasmtime/latest/wasmtime/struct.Config.html) while using as inputs two modules obtained from the previous section, the 53,095 bytes baseline and the 13,660 bytes optimized module. They were turned into components using [`wasm-tools`](github.com/bytecodealliance/wasm-tools) and the `wasm-tools component new` command without any attempt at optimizations. This added roughly 2,100 bytes to the inputs (the baseline went up 2,117 bytes total 55,212 bytes; the optimized module went up 2,142 bytes total 15,802 bytes).

The following options, changed from their default value through mutating the `Config` struct will be considered:
- `memory_may_move`
- `memory_init_cow`
- `cranelift_opt_level` with `None` (default), `Speed` and `SpeedAndSize`
- `compiler_inlining`
- `debug_adapter_modules`
- `generate_adress_map`
- `table_lazy_init`
- `native_unwind_info`
- `signal_based_traps`
- `wasm_backtrace`
- `wasm_bulk_memory`

Setup:
```sh
$ wasm-tools --version
wasm-tools 1.239.0 (a64ae8dd0 2025-09-20)
```
```toml
[dependencies]
wasmtime = {version = "38.0.3",  default-features = false, features = ["runtime", "component-model", "pulley", "cranelift"] }
```

Default Precompilation options
```rust
use wasmtime::{Config, Engine, Result, OptLevel};

fn main() -> wasmtime::Result<()> {
  let mut config = Config::new();

  // The inputs were compiled with `--pagesize=1`
  config.wasm_custom_page_sizes(true);

  // Our target
  config.target("pulley32")?;

  // No Optimizations by default
  config.cranelift_opt_level(OptLevel::None);
  let engine = Engine::new(&config)?;

  let before_precompile = include_bytes!("..."); // Path to the input file
  let precompiled = engine.precompile_component(before_precompile)?;

  std::fs::write("precompiled.cwasm", &precompiled)?;
  Ok(())
}
```

Sizes are measured with `wc -c` as they were in the previous sections and the size differences are reported in the same way

|                   | Default Settings | `memory_may_move` | `memory_init_cow` | `cranelift_opt_level(Speed)` | `cranelift_opt_level(SpeedAndSize)` |
| ----------------- | ---------------- | ----------------- | ----------------- | ---------------------------- | ----------------------------------- |
| Unoptimized Input | 281,160 B        | 281,160 B (-0%)   | 182,136 B (-35%)  | 215,624 B (-23%)             | 215,624 B (-23%)                    |
| Optimized Input   | 138,408 B        | 138,408 B (-0%)   | 50,392 B (-63%)   | 138,392 B (-0%)              | 138,392 B (-0%)                     |

|                   | Default Settings | `compiler_inlining` | `debug_adapter_modules` | `generate_adress_map` | `table_lazy_init` |
| ----------------- | ---------------- | ------------------- | ----------------------- | --------------------- | ----------------- |
| Unoptimized Input | 281,160 B        | 281,160 B (-0%)     | 281,160 B (-0%)         | 215,544 B (-23%)      | 281,088 B (-0%)   |
| Optimized Input   | 138,408 B        | 138,408 B (-0%)     | 138,408 B (-0%)         | 138,328 B (-0%)       | 138,328 B (-0%)   |

|                   | Default Settings | `native_unwind_info` | `signal_based_traps` | `wasm_backtrace` | `wasm_bulk_memory` |
| ----------------- | ---------------- | -------------------- | -------------------- | ---------------- | ------------------ |
| Unoptimized Input | 281,160 B        | 281,160 B (-0%)      | 281,160 B (-0%)      | 281,160 B (-0%)  | 281,160 B (-0%)    |
| Optimized Input   | 138,408 B        | 138,408 B (-0%)      | 138,408 B (-0%)      | 138,408 B (-0%)  | 138,408 B (-0%)    |

Combining all of the options that reduces the code size requires adding these lines to the precompilation workflow
```rust
  config.memory_init_cow(false);
  config.generate_address_map(false);
  config.table_lazy_init(false);
  config.cranelift_opt_level(OptLevel::Speed);
```
The results of combining the options gives the follow results with size factors

|                   | Original | Component | Default Precompilation | Precomilation with Optimizations |
| ----------------- | -------- | --------- | ---------------------- | -------------------------------- |
| Unoptimized Input | 53,095 B | 55,212 B  | 281,160 B  (1)         | 84232 B (-70%) (2)               |
| Optimized Input   | 13,660 B | 15,802 B  | 138,408 B  (3)         | 28168 B (-80%) (4)               |

For reference, below is by section breakdown of the 4 precompiled files mentionned in the above table using `llvm-size`


```shell
# File (1)
$ llvm-size -A unopt_unopt.cwasm
unopt_unopt.cwasm  :
section                   size   addr
.wasmtime.engine           791      0
.wasmtime.bti                1      0
.text                    91996      0
.wasmtime.addrmap        67004      0
.wasmtime.traps            454      0
.wasmtime.exceptions       320      0
.rodata.wasm             65536      0
.name.wasm                5471      0
.wasmtime.info            3009      0
Total                   234582
```
```sh
# File(2)
$ llvm-size -A unopt_opt.cwasm
unopt_opt.cwasm  :
section                   size   addr
.wasmtime.engine           792      0
.wasmtime.bti                1      0
.text                    61271      0
.wasmtime.traps            449      0
.wasmtime.exceptions       320      0
.rodata.wasm              2484      0
.name.wasm                5471      0
.wasmtime.info            2995      0
Total                    73783
```
```sh
# File(3)
$ llvm-size -A opt_unopt.cwasm
opt_unopt.cwasm  :
section                   size   addr
.wasmtime.engine           791      0
.wasmtime.bti                1      0
.text                    22820      0
.wasmtime.addrmap        18564      0
.wasmtime.traps            244      0
.wasmtime.exceptions       296      0
.rodata.wasm             65536      0
.wasmtime.info            2338      0
Total                   110590
```
```sh
# File(4)
$ llvm-size -A opt_opt.cwasm
opt_opt.cwasm  :
section                   size   addr
.wasmtime.engine           792      0
.wasmtime.bti                1      0
.text                    19338      0
.wasmtime.traps            239      0
.wasmtime.exceptions       296      0
.rodata.wasm               280      0
.wasmtime.info            2312      0
Total                    23258
```

## Conclusion
In this section we highlighted that compilations options can have a huge influence on the final size of the capsule. We did not try to evaluate the impact of reducing code size on the performance of the capsule payload. The options that were found to reduce code size have been put in the [payload config file](./payloads/.cargo/config.toml) and in the [precompilation_script](./precompile_wasm.rs).