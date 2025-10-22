#[cfg(feature = "log")]
pub mod log;

#[cfg(feature = "rng")]
pub mod rng;

#[cfg(feature = "time")]
pub mod time;

#[cfg(feature = "udp")]
pub mod udp;

#[cfg(feature = "coap-server-guest")]
pub mod coap_server_guest;


#[derive(Default)]
pub struct ArielOSHost {
    #[cfg(feature = "log")]
    log_host: crate::wasm::log::ArielLogHost,

    #[cfg(feature = "time")]
    time_host: crate::wasm::time::ArielTimeHost,

    #[cfg(feature = "rng")]
    rng_host: crate::wasm::rng::ArielRNGHost,

    #[cfg(feature = "udp")]
    udp_host: crate::wasm::udp::ArielUDPHost,
}

#[cfg(feature = "log")]
impl crate::wasm::log::Host for ArielOSHost {
    fn info(&mut self,input:wasmtime::component::__internal::String) -> () {
        self.log_host.info(input);
    }
}

#[cfg(feature = "time")]
impl crate::wasm::time::Host for ArielOSHost {
    async fn sleep(&mut self, millis: u64) {
        self.time_host.sleep(millis).await
    }

    fn now_as_millis(&mut self) -> u64 {
        self.time_host.now_as_millis()
    }
}

#[cfg(feature = "rng")]
impl crate::wasm::rng::HostRNG for ArielOSHost {
    fn next_u32(&mut self,) -> u32 {
        self.rng_host.next_u32()
    }
    fn next_u64(&mut self,) -> u64 {
        self.rng_host.next_u64()
    }
    fn random_bytes(&mut self,len: u32) -> wasmtime::component::__internal::Vec<u8> {
        self.rng_host.random_bytes(len)
    }
    fn drop(&mut self, rep: wasmtime::component::Resource<crate::wasm::rng::RNG>) -> wasmtime::Result<()> {
        self.rng_host.drop(rep)
    }
}

#[cfg(feature = "rng")]
impl crate::wasm::rng::Host for ArielOSHost {}

#[cfg(feature = "udp")]
impl crate::wasm::udp::Host for ArielOSHost {}

#[cfg(feature = "udp")]
impl crate::wasm::udp::HostUdpSocket for ArielOSHost {
    fn bind(&mut self,port:u16,) -> Result<(),()> {
        self.udp_host.bind(port)
    }

    fn send(&mut self,data:wasmtime::component::__internal::Vec<u8>,endpoint:udp::gen_udp::UdpMetadata,) -> Result<(),()> {
        self.udp_host.send(data, endpoint)
    }

    fn recv(&mut self,) -> Result<Option<(wasmtime::component::__internal::Vec<u8>,udp::gen_udp::UdpMetadata,)>,()> {
        self.udp_host.recv()
    }

    fn drop(&mut self,rep:wasmtime::component::Resource<udp::gen_udp::UdpSocket>) -> wasmtime::Result<()> {
        self.udp_host.drop(rep)
    }
}

#[cfg(feature = "udp")]
impl ArielOSHost {
    pub unsafe fn initialize_socket(&mut self,
        stack: ariel_os_embassy::NetworkStack,
        rx_meta: &mut [ariel_os_embassy::reexports::embassy_net::udp::PacketMetadata],
        rx_buffer: &mut [u8],
        tx_meta: &mut [ariel_os_embassy::reexports::embassy_net::udp::PacketMetadata],
        tx_buffer: &mut [u8],
    ) {
        unsafe { self.udp_host.initialize_socket(stack, rx_meta, rx_buffer, tx_meta, tx_buffer) };
    }
}
