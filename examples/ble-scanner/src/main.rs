#![no_main]
#![no_std]

use ariel_os::debug::log::{defmt, info};
use ariel_os::debug::{ExitCode, exit};

use ariel_os::time::{Timer, Duration};
use embassy_futures::select::{select3, Either3};
use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{AsContextMut, Config, Engine, Store};

extern crate alloc;
use core::cell::RefCell;

use trouble_host::{
    Host,
    connection::{PhySet, ScanConfig},
    prelude::{EventHandler, LeAdvReportsIter},
    scan::Scanner,
};

use ariel_os_bindings::wasm::ArielOSHost;


bindgen!({
    world: "example-ble-scanner",
    path: "../../wit",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
    }
});

use exports::ariel::wasm_bindings::ble_api::BdAddr;

#[ariel_os::task(autostart)]
async fn main() {
    let res = run_wasm_coap_server().await;
    info!("{:?}", defmt::Debug2Format(&res));
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

async fn run_wasm_coap_server() -> wasmtime::Result<()> {
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

    let host = ArielOSHost::default();
    let mut store = Store::new(&engine, host);

    let wasm = include_bytes!("../payload.cwasm").as_slice();

    let component = unsafe { Component::deserialize_raw(&engine, wasm.into()).unwrap() };

    info!("Instantatiating component");

    let mut linker = Linker::new(&engine);

    ExampleBleScanner::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
    let comp = ExampleBleScanner::instantiate(&mut store, &component, &linker)?;
    let printer = ComponentScanner((comp, RefCell::new(store)));

    info!("starting ble stack");
    let stack = ariel_os::ble::ble_stack().await;

    let Host {
        central,
        mut runner,
        ..
    } = stack.build();

    let mut scanner = Scanner::new(central);

    loop {
        // Scan for 5 seconds then print the stats
        match select3(
            runner.run_with_handler(&printer),
            async {
                    let config = ScanConfig::<'_> {
                        active: true,
                        phys: PhySet::M1,
                        interval: Duration::from_secs(1),
                        window: Duration::from_secs(1),
                        ..Default::default()
                    };
                    let mut _session = scanner.scan(&config).await.unwrap();
                    // Scan forever
                    info!("scanning...");
                    loop {
                        Timer::after_secs(1).await;
                    }
                },
                Timer::after_secs(5)
        ).await {
            Either3::First(_) => unreachable!(),
            Either3::Second(_) => unreachable!(),
            Either3::Third(_) => { } // Leave the match to drop the &printer ref
        }

            let comp_instance = &printer.0.0;
            let mut store_handle = printer.0.1.borrow_mut();
            let stats = comp_instance.interface0.call_return_stats(store_handle.as_context_mut()).unwrap();
            let different_addr = stats.len();
            let total_count: u64 = stats.iter().map(|(_, c)| { *c }).sum();
            info!("Discovered {} different adress over {} packets", different_addr, total_count);
    }
}

pub struct ComponentScanner((ExampleBleScanner, RefCell<Store<ArielOSHost>>));

impl EventHandler for ComponentScanner {
    fn on_adv_reports(&self, mut it: LeAdvReportsIter<'_>) {
        let comp_instance = &self.0.0;
        let mut store_handle = self.0.1.borrow_mut();
        while let Some(Ok(report)) = it.next() {
            comp_instance.interface0
                .call_on_single_report(store_handle.as_context_mut(), BdAddr::new(report.addr.into_inner()))
                    .unwrap()
                    .unwrap();
        }
    }
}

impl Default for BdAddr {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            f: 0,
        }
    }
}

impl BdAddr {
    fn new(val: [u8; 6]) -> Self {
        let mut res = BdAddr::default();
        res.a = val[0];
        res.b = val[1];
        res.c = val[2];
        res.d = val[3];
        res.e = val[4];
        res.f = val[5];
        res
    }
}