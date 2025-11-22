#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
clap = { version = "4.5.40", features = ["derive"]}
# Dependency version is locked to get reproducible build output.
wasmtime = { version = "=38.0", default-features = false, features = ["component-model", "async", "cranelift", "pulley"] }
miette = { version = "7.2.0", features = ["fancy"] }
thiserror = { version = "2.0.12" }

---
#![feature(trim_prefix_suffix)]
use std::{fs, io, path::{PathBuf, Path}};
use std::io::BufRead as _;
use clap::{Parser, ValueEnum, builder::PossibleValue};
use miette::Diagnostic;

use wasmtime::{Config, Engine, OptLevel};

#[derive(Clone, Copy, Debug)]
enum CLIOptLevel {
    Three,
    S,
    Z,
}

impl ValueEnum for CLIOptLevel {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::S, Self::Three, Self::Z]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Three => Some(PossibleValue::new("3")),
            Self::S => Some(PossibleValue::new("s")),
            Self::Z => Some(PossibleValue::new("z")),
        }
    }
}


/// Simple CLI that takes a compiled .wasm file and turns into a precompiled-component
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to the compiled Wasm file or to a manifest for a lib that will be compiled
    #[arg(short, long)]
    path: PathBuf,

    /// Path to an optional `config.toml` file when `path` points to a manifest
    #[arg(long)]
    config: Option<PathBuf>,

    /// Toolchain for the compilation when `path` points to a manifest
    #[arg(short, long)]
    toolchain: Option<String>,

    /// Additionnal compilation options propagated to cargo
    #[arg(long = "compile_options", short = 'C')]
    additional: Vec<String>,

    #[arg(long, default_value = "pulley32")]
    target: String,

    /// Turn fuel instrumentation on
    #[arg(short, long)]
    fuel: bool,

    /// Override default opt-level
    #[arg(short = 'O', long = "opt-level", value_enum, default_value_t = CLIOptLevel::S)]
    opt_level: CLIOptLevel,

    /// Path of the output file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Path to wasm-tools if isn't not in $PATH
    #[arg(short, long)]
    wasm_tools: Option<PathBuf>,

    /// Outputs a Module instead of a component
    #[arg(short, long)]
    module: bool,

    /// Conserve the intermediate files under `ouput.wasm` and `output.comp.wasm`
    #[arg(long = "conserve-artifacts", short = 'a')]
    conserve: bool
}

#[derive(Debug, thiserror::Error, Diagnostic)]
enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Precompilation Error: {0}")]
    Precomp(#[from] wasmtime::Error),
}


fn main() -> miette::Result<()> {
    let args = Args::parse();

    let Args { path, config, toolchain, additional, fuel, output, wasm_tools, module, opt_level, target, conserve } = args;

    // Check that the path exists
    assert!(fs::exists(&path).map_err(Error::from)?);

    let new_path = match path.extension().map(std::ffi::OsStr::to_str).flatten() {
        // If it's a already a compiled wasm do nothing
        Some("wasm") => {
            path
        },
        // Means that it's a directory
        Some("toml") => {
            std::println!("Compiling Library");
            let manifest = io::BufReader::new(fs::File::open(&path).map_err(Error::from)?);
            let name = manifest.lines().into_iter().filter_map(
                |line| {
                    match line {
                        Ok(l) => {
                            if l.starts_with("name =") {
                                Some(l.split("=").nth(1).unwrap().split("\"").nth(1).unwrap().to_owned().replace('-', "_"))
                            }
                            else {
                                None
                            }
                        }
                        Err(_) => None
                    }
                }
            ).next().unwrap();

            let mut args: Vec<&str> =  match toolchain  {
                Some(toolchain) => {
                    if toolchain.starts_with("+") {
                        vec![toolchain.leak()]
                    }
                    else {
                        vec!["+", toolchain.leak()]
                    }
                }
                None => {
                    // Use nightly by default since it's needed for --page-size=1
                    vec!["+nightly"]
                }
            };

            args.extend(["rustc", "--release", "--manifest-path", path.as_path().to_str().unwrap()]);

            if let Some(config_path) = config {
                    match config_path.extension().map(std::ffi::OsStr::to_str).flatten() {
                    Some("toml") => {
                        args.push("--config");
                        args.push(config_path.to_string_lossy().into_owned().leak());
                    }
                    _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "--config should point to a cargo config file")).map_err(Error::from)?,
                }
            }


            args.extend(additional.iter().map(String::as_str));
            args.extend(["--target-dir", "temp"]);
            let opt_string = match opt_level {
                CLIOptLevel::Three => "-Copt-level=3",
                CLIOptLevel::S => "-Copt-level=s",
                CLIOptLevel::Z => "-Copt-level=z",
            };

            args.extend(["--", opt_string]);

            std::println!("{:?}", args);

            let output = std::process::Command::new("cargo")
                .args(&args)
                .output()
                .map_err(Error::from)?;

            let std::process::Output {status, stdout: _, stderr} = output;

            if !status.success() {
                std::println!(
                    "{}", String::from_utf8_lossy(&stderr)
                );

                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Module compilation failed")).map_err(Error::from)?;
            }


            format!("temp/wasm32v1-none/release/{name}.wasm").into()
        },
        _ => {
            Err(io::Error::new(io::ErrorKind::InvalidInput, "--path should be wasm file or a path to a Cargo.toml manifest")).map_err(Error::from)?
        }
    };

    std::println!("{}", new_path.display());

    if !module {
        // Turn into a component
        std::println!("Turning the Module into a component");
        let wasm_tools_path = if wasm_tools.is_some() {
            wasm_tools.unwrap()
        } else {
            "wasm-tools".into()
        };
        // Note: On Unix, non UTF8 strings are invalid Paths so the unwrap() is infallible
        let output = std::process::Command::new(wasm_tools_path)
            .args(["component", "new", new_path.as_path().to_str().unwrap(), "-o", "temp.wasm"])
            .output()
            .map_err(Error::from)?;

        let std::process::Output {status, stdout: _, stderr} = output;

        if !status.success() {
            std::println!(
                "{}", String::from_utf8_lossy(&stderr)
            );
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Componentization failed")).map_err(Error::from)?;
        }
    }

    let out = if output.is_some() {
        output.unwrap()
    } else {
        String::from("payload.cwasm").into()
    };

    if conserve {
        // Derive the artifacts names from output
        let mut art_path = PathBuf::from(out.clone());
        // initial.wasm
        art_path.set_extension("wasm");
        std::fs::copy(&new_path, &art_path).map_err(Error::from)?;
        if !module {
            art_path.set_extension("comp.wasm");
            std::fs::copy("temp.wasm", &art_path).map_err(Error::from)?;
        }
    }

    if !module {
        precompile("temp.wasm", &target, fuel, out, module)?;
        std::fs::remove_file("temp.wasm").map_err(Error::from)?;
    } else {
        precompile(&new_path, &target, fuel, out, module)?;
    }
    if std::fs::exists("temp").map_err(Error::from)? {
        std::fs::remove_dir_all("temp").map_err(Error::from)?;
    }

    Ok(())
}

fn precompile<P: AsRef<Path>>(path: P, target: &str, fuel: bool, out: PathBuf, module: bool) -> miette::Result<()> {
    std::println!("Precompiling Wasm Module/Component");
    let mut config = Config::new();

    // Options found to reduce the output code size the most at least for components
    config.memory_init_cow(false);
    config.generate_address_map(false);
    config.table_lazy_init(false);
    config.cranelift_opt_level(OptLevel::Speed);

    config.wasm_custom_page_sizes(true);
    config.target(target).map_err(Error::from)?;

    // 0 means limiting ourselves to what the module asked
    // This needs to be set at pre-compile time and at runtime in the engine
    config.memory_reservation(0);

    // Disabling this allows runtime optimizations but means that the maximum memory
    // that the module can have is
    // S = min(initial_memory, memory_reservation) + memory_reserver_for_growth
    // since it can grow by reallocating.
    config.memory_may_move(false);

    // Enable fuel intstrumentation to prevent malevolent code from running indefinitely in the VM
    config.consume_fuel(fuel);

    // Create an `Engine` with that configuration.
    let engine = Engine::new(&config).map_err(Error::from)?;

    std::println!("Reading Module/Component File");
    let wasm = fs::read(path).map_err(Error::from)?;
    let precompiled = if !module {
        engine.precompile_component(&wasm).map_err(Error::from)?
    } else {
        engine.precompile_module(&wasm).map_err(Error::from)?
    };

    std::println!("Writing the precompiled file");
    fs::write(out, &precompiled).map_err(Error::from)?;

    Ok(())
}
