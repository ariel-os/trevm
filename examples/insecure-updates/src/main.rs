#![no_main]
#![no_std]

use ariel_os::time::Timer;
use ariel_os::{
    debug::{
        ExitCode, exit,
        log::{defmt, info},
    },
    reexports::embassy_net::udp::{RecvError, UdpMetadata},
};

use ariel_os::{net, reexports::embassy_net};
use embassy_net::udp::{PacketMetadata, UdpSocket};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};
use wasmtime::{Config, Engine, Store};

use embassy_futures::select::{Either, select};

use ariel_os_bindings::wasm::ArielOSHost;

bindgen!({
    world: "example-updates",
    path: "../../wit/",
    with: {
        "ariel:wasm-bindings/log-api": ariel_os_bindings::wasm::log,
        "ariel:wasm-bindings/time-api": ariel_os_bindings::wasm::time,
    },
    exports: {
        default: async,
    },
    imports: {
        default: async,
    }
});

const BUFFER_SIZE: usize = 128;
const WASM_BUFFER_SIZE: usize = 32 * 1024; // 32 KiB maximum wasm component for now
const USIZE_BYTES: usize = (usize::BITS / 8) as usize;

#[ariel_os::task(autostart)]
async fn main() {
    let r = run_wasm().await;
    info!("{:?}", defmt::Debug2Format(&r));
    Timer::after_millis(100).await;
    exit(ExitCode::SUCCESS);
}

fn configure_engine() -> wasmtime::Result<Engine> {
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

    // async support
    config.async_support(true);
    config.async_stack_size(4096);

    Ok(Engine::new(&config)?)
}

/// # Errors
/// Misconfiguration of Wasmtime or of the component
async fn run_wasm() -> wasmtime::Result<()> {
    // Set Things Up for capsule instantiation
    let engine = configure_engine()?;
    let mut linker = Linker::<ArielOSHost>::new(&engine);
    ExampleUpdates::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;

    // UDP Stack setup
    let stack = net::network_stack().await.unwrap();

    let mut rx_meta = [PacketMetadata::EMPTY; 1];
    let mut rx_buffer = [0; BUFFER_SIZE];
    let mut tx_meta = [PacketMetadata::EMPTY; 1];
    let mut tx_buffer = [0; BUFFER_SIZE];
    let mut buf = [0; BUFFER_SIZE];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    info!("Listening on UDP:1234...");
    if let Err(e) = socket.bind(1234) {
        info!("bind error: {:?}", e);
        exit(ExitCode::FAILURE);
    }
    info!(
        "Ready to receive WASM Payload of up to {} bytes",
        WASM_BUFFER_SIZE
    );
    let mut wasm_buffer = [0_u8; WASM_BUFFER_SIZE];

    let mut current_instance: Option<ExampleUpdates> = None;
    let mut current_component: Component;
    let mut current_store: Store<ArielOSHost> = Store::new(&engine, ArielOSHost::default());
    'outer: loop {
        // Ugly code for now, who cares
        if current_instance.is_none() {
            let mut state = CommunicationState::NotStarted;
            let packet = socket.recv_from(&mut buf).await;
            match process_packet(packet, &mut buf, &mut wasm_buffer, &mut state) {
                Ok(PacketKind::DeclareSize) => {
                    info!("Someone is trying to send a file");
                }
                Ok(PacketKind::Useless) => {
                    // Restart
                    continue 'outer;
                }
                Err(PacketProcessingError::HadntStartedYet) => {
                    // Restart
                    continue 'outer;
                }
                Err(PacketProcessingError::SizeTooBig) => {
                    // Restart
                    continue 'outer;
                }
                Err(PacketProcessingError::ReceiveError) => {
                    panic!("Socket has an issue, panicking")
                }
                // ProgressCan'tStop if it didn't start yet
                Err(PacketProcessingError::ProgressStopped) => unreachable!(),
                // Other Oks require a different CommunicationState
                Ok(_) => unreachable!(),
            }
            // Reaching this means that we starting communicating and the state reflects this
            assert!(matches!(state, CommunicationState::InProgress(_, _)));
            match state {
                CommunicationState::InProgress(0, rem) => {
                    info!("Trying to receive file of size {}", rem)
                }
                _ => unreachable!(),
            }
            // Receiving the actual file
            'inner: loop {
                let packet = socket.recv_from(&mut buf).await;
                match process_packet(packet, &buf, &mut wasm_buffer, &mut state) {
                    Ok(PacketKind::PartialFill) => {}
                    Ok(PacketKind::BufferFiled) => break 'inner,
                    Ok(PacketKind::Useless) => {}
                    Ok(PacketKind::DeclareSize) => unreachable!(),
                    Err(PacketProcessingError::HadntStartedYet) => unreachable!(),
                    Err(PacketProcessingError::ProgressStopped) => continue 'outer,
                    Err(PacketProcessingError::SizeTooBig) => continue 'outer,
                    Err(PacketProcessingError::ReceiveError) => {
                        panic!("Socket has an issue, panicking")
                    }
                }
            }
            info!("The File was completely received");
            // Now the transfer is done, make a component, a store and an instance
            current_component =
                unsafe { Component::deserialize_raw(&engine, wasm_buffer.as_slice().into())? };
            current_store = Store::new(&engine, ArielOSHost::default());
            let instance =
                ExampleUpdates::instantiate_async(&mut current_store, &current_component, &linker)
                    .await?;
            current_instance = Some(instance);
        }
        // Guaranteed to be a Some(_) and everything to be initialized

        let stop_reason = {
            let mut capsule_work = core::pin::pin!(
                current_instance
                    .as_mut()
                    .unwrap()
                    .call_run(&mut current_store)
            );
            'polling: loop {
                info!("Running a component and waiting for updates");
                match select(socket.recv_from(&mut buf), &mut capsule_work).await {
                    Either::First(packet) => {
                        let mut state = CommunicationState::NotStarted;
                        match process_packet(packet, &buf, &mut [], &mut state) {
                            Ok(PacketKind::DeclareSize) => {
                                // Stop the capsule
                                info!(
                                    "Received the start of a new wasm capsule, cancelling the underlying capsule immediatly"
                                );
                                break 'polling StopReason::NewCapsule(state);
                            }
                            _ => {
                                info!("Useless Package gotten");
                                continue 'polling;
                            }
                        }
                    }
                    Either::Second(_) => {
                        info!("The capsule finished running, you love to see it");
                        break 'polling StopReason::CapsuleFinished;
                    }
                }
            }
        };
        match stop_reason {
            StopReason::CapsuleFinished => {
                current_instance = None;
            }
            StopReason::NewCapsule(mut state) => {
                // The capsule was stopped because a new capsule is incoming
                match state {
                    CommunicationState::InProgress(0, rem) => {
                        info!("Trying to receive file of size {}", rem);
                    }
                    _ => unreachable!(),
                }
                // Receiving the actual file
                'inner: loop {
                    let packet = socket.recv_from(&mut buf).await;
                    match process_packet(packet, &buf, &mut wasm_buffer, &mut state) {
                        Ok(PacketKind::PartialFill) => {}
                        Ok(PacketKind::BufferFiled) => break 'inner,
                        Ok(PacketKind::Useless) => {}
                        Ok(PacketKind::DeclareSize) => unreachable!(),
                        Err(PacketProcessingError::HadntStartedYet) => unreachable!(),
                        Err(PacketProcessingError::ProgressStopped) => {
                            current_instance = None;
                            continue 'outer;
                        }
                        Err(PacketProcessingError::SizeTooBig) => {
                            current_instance = None;
                            continue 'outer;
                        }
                        Err(PacketProcessingError::ReceiveError) => {
                            panic!("Socket has an issue, panicking")
                        }
                    }
                }
                info!("The File was completely received");
                // Now the transfer is done, make a component, a store and an instance
                current_component =
                    unsafe { Component::deserialize_raw(&engine, wasm_buffer.as_slice().into())? };
                current_store = Store::new(&engine, ArielOSHost::default());
                let instance = ExampleUpdates::instantiate_async(
                    &mut current_store,
                    &current_component,
                    &linker,
                )
                .await?;
                current_instance = Some(instance);
            }
        }
    }
}

#[derive(Debug)]
enum StopReason {
    NewCapsule(CommunicationState),
    CapsuleFinished,
}

#[derive(Debug)]
enum PacketKind {
    DeclareSize,
    PartialFill,
    BufferFiled,
    Useless,
}

#[derive(Debug, Clone, Copy)]
enum CommunicationState {
    NotStarted,
    InProgress(usize, usize),
}

impl CommunicationState {
    fn in_progress(&self) -> bool {
        match &self {
            CommunicationState::InProgress(_, _) => true,
            CommunicationState::NotStarted => false,
        }
    }
}

#[derive(Debug)]
enum PacketProcessingError {
    ProgressStopped,
    ReceiveError,
    SizeTooBig,
    HadntStartedYet,
}

use PacketProcessingError::*;

fn process_packet(
    packet: Result<(usize, UdpMetadata), RecvError>,
    recv_buffer: &[u8],
    wasm_buffer: &mut [u8],
    state: &mut CommunicationState,
) -> Result<PacketKind, PacketProcessingError> {
    match packet {
        Ok((0, _)) => {
            if state.in_progress() {
                return Err(ProgressStopped);
            } else {
                return Ok(PacketKind::Useless);
            }
        }
        Ok((n, _)) => match state {
            CommunicationState::InProgress(offset, remaining) => {
                if *remaining < n {
                    return Err(SizeTooBig);
                } else {
                    wasm_buffer[*offset..*offset + n].copy_from_slice(&recv_buffer[..n]);
                    *offset += n;
                    *remaining -= n;
                }
                if *remaining == 0 {
                    return Ok(PacketKind::BufferFiled);
                } else {
                    return Ok(PacketKind::PartialFill);
                }
            }
            CommunicationState::NotStarted => {
                if n != USIZE_BYTES {
                    return Err(HadntStartedYet);
                }
                let remaining = usize::from_be_bytes(recv_buffer[..n].try_into().unwrap());
                if remaining > WASM_BUFFER_SIZE {
                    return Err(SizeTooBig);
                }
                let offset = 0;
                *state = CommunicationState::InProgress(offset, remaining);
                return Ok(PacketKind::DeclareSize);
            }
        },
        Err(_) => return Err(ReceiveError),
    }
}
