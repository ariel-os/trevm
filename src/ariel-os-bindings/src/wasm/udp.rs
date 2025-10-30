use ariel_os_debug::log::info;
use ariel_os_embassy::reexports::embassy_net;

use embassy_net::IpAddress;
use embassy_net::udp::{PacketMetadata, UdpMetadata, UdpSocket};

use wasmtime::component::Resource;

use core::mem;

extern crate alloc;
use alloc::vec::Vec;

use wasmtime::component::bindgen;

bindgen!({
    world: "ariel:wasm-bindings/udp",
    path: "../../wit/",
});

pub use ariel::wasm_bindings::udp_api::add_to_linker;
pub use ariel::wasm_bindings::udp_api::{self as gen_udp, Host, HostUdpSocket, HostWithStore};

use embassy_futures::block_on;

#[derive(Default)]
pub struct ArielUDPHost {
    socket: Option<UdpSocket<'static>>,
    buffer_size: usize,
}

impl ArielUDPHost {
    pub unsafe fn initialize_socket(
        &mut self,
        stack: ariel_os_embassy::NetworkStack,
        rx_meta: &mut [PacketMetadata],
        rx_buffer: &mut [u8],
        tx_meta: &mut [PacketMetadata],
        tx_buffer: &mut [u8],
    ) {
        let buff_size = tx_buffer.len();
        let rx_meta: &'static mut [PacketMetadata] = unsafe { mem::transmute(rx_meta) };
        let rx_buffer: &'static mut [u8] = unsafe { mem::transmute(rx_buffer) };
        let tx_meta: &'static mut [PacketMetadata] = unsafe { mem::transmute(tx_meta) };
        let tx_buffer: &'static mut [u8] = unsafe { mem::transmute(tx_buffer) };

        let socket = UdpSocket::new(stack, rx_meta, rx_buffer, tx_meta, tx_buffer);

        self.socket = Some(socket);
        self.buffer_size = buff_size;
    }
}

impl gen_udp::Ipv4Addr {
    fn from_octets(octets: [u8; 4]) -> Self {
        Self {
            a: octets[0],
            b: octets[1],
            c: octets[2],
            d: octets[3],
        }
    }
}

// #[cfg(feature = "ipv6")]
// impl udp::Ipv6Addr {
//     fn from_segments(segments: [u16; 8]) -> Self {
//         Self {
//             a: segments[0],
//             b: segments[1],
//             c: segments[2],
//             d: segments[3],
//             e: segments[4],
//             f: segments[5],
//             g: segments[6],
//             h: segments[7],
//         }
//     }
// }

impl From<IpAddress> for gen_udp::IpAddr {
    fn from(t: IpAddress) -> Self {
        match t {
            IpAddress::Ipv4(ipaddr) => {
                let octs = ipaddr.octets();
                gen_udp::IpAddr::V4(gen_udp::Ipv4Addr::from_octets(octs))
            }
            // #[cfg(feature = "ipv6")]
            // IpAddress::Ipv6(ipaddr) => {
            //     let segments = ipaddr.segments();
            //     udp::IpAddr::V6(
            //         udp::Ipv6Addr::from_segments(segments)
            //     )
            // },
            #[allow(unreachable_patterns, reason = "Conditional compilation")]
            _ => unreachable!(),
        }
    }
}

impl From<gen_udp::IpAddr> for IpAddress {
    fn from(t: gen_udp::IpAddr) -> Self {
        match t {
            gen_udp::IpAddr::V4(ipaddr) => {
                let gen_udp::Ipv4Addr { a, b, c, d } = ipaddr;
                Self::v4(a, b, c, d)
            }
            // #[cfg(feature = "ipv6")]
            // udp::IpAddr::V6(ipaddr) => {
            //     let udp::Ipv6Addr { a, b, c, d, e, f, g, h } = ipaddr;
            //     Self::v6(a, b, c, d, e, f, g, h)
            // },
            #[allow(unreachable_patterns, reason = "Conditional compilation")]
            _ => unreachable!(),
        }
    }
}

impl From<UdpMetadata> for gen_udp::UdpMetadata {
    fn from(t: UdpMetadata) -> Self {
        let UdpMetadata {
            endpoint,
            local_address,
            meta: _,
        } = t;
        let e_addr = endpoint.addr.into();
        let e_port = endpoint.port;

        Self {
            endpoint: gen_udp::Endpoint {
                addr: e_addr,
                port: e_port,
            },
            local_addr: local_address.map(gen_udp::IpAddr::from),
        }
    }
}

impl From<gen_udp::UdpMetadata> for UdpMetadata {
    fn from(t: gen_udp::UdpMetadata) -> Self {
        let gen_udp::UdpMetadata {
            endpoint,
            local_addr: _,
        } = t;
        let e_addr: IpAddress = endpoint.addr.into();
        let e_port = endpoint.port;

        (e_addr, e_port).into()
    }
}

impl HostUdpSocket for ArielUDPHost {
    fn bind(&mut self, port: u16) -> Result<(), ()> {
        match self.socket.as_mut() {
            Some(socket) => socket.bind(port).map_err(|_| ()),
            None => {
                info!("Unintialized Socket");
                Err(())
            }
        }
    }

    fn send(&mut self, data: Vec<u8>, endpoint: gen_udp::UdpMetadata) -> Result<(), ()> {
        match self.socket.as_ref() {
            Some(socket) => {
                let endpoint = UdpMetadata::from(endpoint);
                info!("Sending some data to {:?}", endpoint);
                block_on(socket.send_to(&data, endpoint)).map_err(|_| ())
            }
            None => {
                info!("Unintialized Socket");
                Err(())
            }
        }
    }

    fn try_recv(&mut self) -> Result<Option<(Vec<u8>, gen_udp::UdpMetadata)>, ()> {
        match self.socket.as_ref() {
            Some(socket) => {
                if !socket.may_recv() {
                    return Ok(None);
                }
                let mut buf: Vec<u8> = core::iter::repeat_n(0, self.buffer_size).collect();
                match block_on(socket.recv_from(&mut buf)) {
                    Err(_) => Err(()),
                    Ok((0, _)) => Ok(None),
                    Ok((n, endpoint)) => {
                        info!("Received some data from {:?}", endpoint);
                        buf.truncate(n);
                        Ok(Some((buf, endpoint.into())))
                    }
                }
            }
            None => {
                info!("Unintialized Socket");
                Err(())
            }
        }
    }

    fn drop(&mut self, _: Resource<gen_udp::UdpSocket>) -> wasmtime::Result<()> {
        unreachable!()
    }
}

impl Host for ArielUDPHost {}
