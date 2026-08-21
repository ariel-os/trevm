#![no_main]
#![no_std]
extern crate alloc;

use core::ptr::NonNull;

use alloc::vec::Vec;
use ariel_os::coap::coap_run;
use ariel_os::debug::log::{Debug2Format, error, info, warn};

use coap_handler_implementations::{HandlerBuilder, ReportingHandlerBuilder, new_dispatcher};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;

use embassy_futures::select::{Either, select};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Error as WasmtimeError, Store};

use ariel_os_bindings::wasm::ArielOSHost;

use crate::suit::{UpdateError, build_and_authenticate_manifest, fetch_and_verify_update};
use crate::vm_control::{VmControl, VmEvent, wait_for_update_request};

mod coap_fetch;
mod suit;
mod vm_control;

bindgen!({
    world: "example-async",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
        "ariel:wasm-bindings/rng-api": ariel_os_bindings::wasm::rng,

    },
    require_store_data_send: true,
});

static VM_DROP_REQUESTS: Channel<CriticalSectionRawMutex, (), 1> = Channel::new();
static VM_STATUS_SIGNAL: Channel<CriticalSectionRawMutex, VmEvent, 1> = Channel::new();
static UPDATE_RESULTS: Channel<CriticalSectionRawMutex, Result<Vec<u8>, ()>, 1> = Channel::new();

#[ariel_os::task(autostart)]
async fn coap_task() {
    let control = VmControl::new();

    let handler = new_dispatcher()
        .at_with_attributes(&["vm-control"], &[], control)
        .with_wkc();

    info!("Starting CoAP handler");
    coap_run(handler).await;
}

#[ariel_os::task(autostart)]
async fn suit_update_task() {
    let mut accepted_sequence_number = None;
    loop {
        let envelope = wait_for_update_request().await;
        info!("[SUIT] Received update request");

        let (manifest, sequence_number) = match build_and_authenticate_manifest(&envelope) {
            Ok(manifest) => manifest,
            Err(e) => {
                info!("[SUIT] Update rejected: {:?}", Debug2Format(&e));
                continue;
            }
        };

        if let Some(current) = accepted_sequence_number {
            if sequence_number < current {
                warn!(
                    "[SUIT] Update rejected: {:?}",
                    Debug2Format(&UpdateError::RollbackDetected {
                        current,
                        attempted: sequence_number,
                    })
                );
                continue;
            }

            if sequence_number == current {
                warn!(
                    "[SUIT] accepting repeated manifest sequence number {:?} for testing",
                    sequence_number
                );
            }
        }

        info!("[SUIT] Update authenticated. Requesting drop of old capsule...");
        VM_DROP_REQUESTS.send(()).await;
        match VM_STATUS_SIGNAL.receive().await {
            VmEvent::Dropped => {
                info!("[SUIT] Capsule dropped. Fetching new capsule...");
            }
            other => {
                info!("[SUIT] Unexpected VM event {:?}", Debug2Format(&other));
                continue;
            }
        }

        match fetch_and_verify_update(manifest).await {
            Ok(capsule) => {
                accepted_sequence_number = Some(
                    accepted_sequence_number
                        .map_or(sequence_number, |current| current.max(sequence_number)),
                );

                info!(
                    "[SUIT] Successfully fetched capsule with a length of {} bytes. Requesting install...",
                    capsule.len()
                );
                UPDATE_RESULTS.send(Ok(capsule)).await
            }
            Err(e) => {
                warn!("[SUIT] Failed to retrieve capsule: {:?}", Debug2Format(&e));
                UPDATE_RESULTS.send(Err(())).await
            }
        }
    }
}

#[ariel_os::task(autostart)]
async fn runner_task() {
    let engine = make_engine();
    let initial_capsule = include_bytes!("../payload.cwasm").as_slice();
    let mut capsule: Vec<u8> = Vec::from(initial_capsule);

    let mut linker = Linker::new(&engine);
    ExampleAsync::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state).unwrap();

    let mut host = ArielOSHost::default();

    loop {
        let (returned_host, result) = run_capsule(&engine, &linker, capsule, host).await;
        match result {
            Ok(VmEvent::Dropped) => {
                info!("Capsule stopped externally");
            }
            Ok(VmEvent::Finished) => {
                info!("Capsule finished on its own");
            }
            Err(e) => {
                error!("run_capsule crashed: {:?}", Debug2Format(&e));
            }
        }

        host = returned_host;

        info!("Waiting for new capsule...");
        capsule = wait_for_capsule().await;
    }
}

fn make_engine() -> Engine {
    let mut cfg = Config::default();
    cfg.wasm_custom_page_sizes(true);
    cfg.target("pulley32").unwrap();

    // Must match precompilation
    cfg.table_lazy_init(false);
    cfg.memory_reservation(0);
    cfg.memory_init_cow(false);
    cfg.memory_may_move(false);

    // Runtime-only tuning
    cfg.max_wasm_stack(2048);
    cfg.memory_reservation_for_growth(0);
    cfg.async_stack_size(4096);

    cfg.consume_fuel(true);

    Engine::new(&cfg).unwrap()
}

async fn wait_for_capsule() -> Vec<u8> {
    loop {
        let cmd_fut = VM_DROP_REQUESTS.receive();
        let update_fut = UPDATE_RESULTS.receive();

        match select(cmd_fut, update_fut).await {
            Either::First(()) => {
                info!("No capsule loaded; acknowledging drop request");
                VM_STATUS_SIGNAL.send(VmEvent::Dropped).await;
            }
            Either::Second(Ok(capsule)) => {
                info!("Received new capsule");
                return capsule;
            }
            Either::Second(Err(())) => {
                info!("Update failed; still waiting for capsule");
            }
        }
    }
}

async fn run_capsule(
    engine: &Engine,
    linker: &Linker<ArielOSHost>,
    mut capsule: Vec<u8>,
    host: ArielOSHost,
) -> (ArielOSHost, Result<VmEvent, WasmtimeError>) {
    let component =
        match unsafe { Component::deserialize_raw(&engine, NonNull::from(capsule.as_mut())) } {
            Ok(component) => component,
            Err(e) => {
                error!("Failed to deserialize component: {:?}", Debug2Format(&e));

                return (host, Err(e));
            }
        };

    let mut store = Store::new(&engine, host);

    store.set_fuel(u64::MAX).expect("failed to set fuel");

    store
        .fuel_async_yield_interval(Some(1_000))
        .expect("failed to set fuel async yield interval");

    let bindings = match ExampleAsync::instantiate_async(&mut store, &component, linker).await {
        Ok(bindings) => bindings,
        Err(e) => {
            let host = store.into_data();
            return (host, Err(e));
        }
    };

    let run_fut = bindings.run.call_async(&mut store, &[], &mut []);
    let drop_requested_fut = VM_DROP_REQUESTS.receive();

    let result = match select(drop_requested_fut, run_fut).await {
        Either::First(_) => Ok(VmEvent::Dropped),
        Either::Second(Ok(_)) => Ok(VmEvent::Finished),
        Either::Second(Err(e)) => Err(e),
    };

    let host = store.into_data();

    if matches!(result, Ok(VmEvent::Dropped)) {
        VM_STATUS_SIGNAL.send(VmEvent::Dropped).await;
    }

    (host, result)
}
