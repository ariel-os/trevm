#![no_main]
#![no_std]

extern crate alloc;

use alloc::{vec::Vec, string::String};
use alloc::boxed::Box;


use ariel_os::coap::coap_run;
use ariel_os::log::{Debug2Format, error, info, warn};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use coap_message_utils::Error as CoapError;

use wasmtime::component::{Component, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

use ariel_os_bindings::wasm::ArielOSHost;

use crate::suit::{build_and_authenticate_manifest, fetch_and_verify_update};
use crate::vm_control::SuitUpdatablePersistentCapsule;

use ariel_os_bindings::wasm::coap::{PersistentCapsule, CanInstantiate};

mod coap_fetch;
mod suit;
mod vm_control;

bindgen!({
    world: "example-persistent-no-bindings",
    path: "../../wit",
});

static SUIT_VERIFY_SIGNAL: Signal<CriticalSectionRawMutex, (u64, Box<[u8]>)> = Signal::new();
static UPDATE_RESULTS: Signal<CriticalSectionRawMutex, Result<(u64, Vec<u8>), ()>> = Signal::new();


#[ariel_os::task(autostart)]
async fn main() {
    let mut config = Config::default();

    // Options that must conform with the precompilation step
    config.wasm_custom_page_sizes(true);
    config.target("pulley32").unwrap();

    config.table_lazy_init(false);
    config.memory_reservation(0);
    config.memory_init_cow(false);
    config.memory_may_move(false);

    // Options that can be changed without changing the payload
    config.max_wasm_stack(2048);
    config.memory_reservation_for_growth(0);

    let engine = Engine::new(&config).unwrap();

    let control: SuitUpdatablePersistentCapsule<'_, ArielOSHost, ExamplePersistentNoBindings> = SuitUpdatablePersistentCapsule::new(&engine);

    let handler = control.to_handler();
    info!("Starting CoAP handler");
    coap_run(handler).await;
}



impl CanInstantiate<ArielOSHost> for ExamplePersistentNoBindings {
    fn instantiate(
        linker: &mut Linker<ArielOSHost>,
        store: &mut Store<ArielOSHost>,
        component: Component,
    ) -> wasmtime::Result<Self> {
        ExamplePersistentNoBindings::instantiate(store, &component, &linker)
    }
}

impl PersistentCapsule<ArielOSHost> for ExamplePersistentNoBindings {
    type E = CoapError;
    fn coap_run(
        &mut self,
        store: &mut Store<ArielOSHost>,
        code: u8,
        observed_len: u32,
        buffer: Vec<u8>,
    ) -> Result<(u8, Vec<u8>), Self::E> {
        match self.ariel_wasm_bindings_coap_server_guest().call_coap_run(
            store,
            code,
            observed_len,
            &buffer,
        ) {
            Ok(coap_rep) => coap_rep.map_err(|_| CoapError::internal_server_error()),
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed, CoAP requests to it will return 5.00 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                return Err(CoapError::internal_server_error());
            }
        }
    }

    fn initialize_handler(&mut self, store: &mut Store<ArielOSHost>) -> wasmtime::Result<()> {
        match self
            .ariel_wasm_bindings_coap_server_guest()
            .call_initialize_handler(store)
        {
            Ok(handler_init_rep) => match handler_init_rep {
                Ok(()) => Ok(()),
                Err(()) => Err(wasmtime::Error::msg("Error when initializing the handler")),
            },
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed at startup, CoAP requests to it will return 5.00 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                Err(wasm_error)
            }
        }
    }

    fn report_resources(&mut self, store: &mut Store<ArielOSHost>) -> Result<Vec<String>, Self::E> {
        match self
            .ariel_wasm_bindings_coap_server_guest()
            .call_report(store)
        {
            Ok(handler_init_rep) => handler_init_rep.map_err(|_| CoapError::internal_server_error()),
            Err(wasm_error) => {
                error!(
                    "The capsule has crashed at startup, CoAP requests to it will return 5.00 \n{}",
                    defmt::Display2Format(&wasm_error)
                );
                Err(CoapError::internal_server_error())
            }
        }
    }
}

#[ariel_os::task(autostart)]
async fn coap_fetching() {
    info!("Waiting for a Manifest ");
    loop {
        let (last_accepted_sequence_number, manifest) = SUIT_VERIFY_SIGNAL.wait().await;

        let (manifest, sequence_number) = match build_and_authenticate_manifest(&manifest) {
            Ok(m) => m,
            Err(e) => {
                info!("SUIT Update rejected: {:?}", Debug2Format(&e));
                UPDATE_RESULTS.signal(Err(()));
                continue;
            }
        };
        info!("Received a SUIT manifest with sequence number: {:?}", sequence_number);

        if last_accepted_sequence_number >= sequence_number {
            info!("SUIT Update rejected");
            continue;
        }


        match fetch_and_verify_update(manifest).await {
            Ok(capsule) => {
                info!(
                    "[SUIT] Successfully fetched capsule with a length of {} bytes. Requesting install...",
                    capsule.len()
                );
                UPDATE_RESULTS.signal(Ok((sequence_number, capsule)));
            }
            Err(e) => {
                warn!("[SUIT] Failed to retrieve capsule: {:?}", Debug2Format(&e));
                UPDATE_RESULTS.signal(Err(()));
            }
        }
    }
}
