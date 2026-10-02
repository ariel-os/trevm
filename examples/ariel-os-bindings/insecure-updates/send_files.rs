#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
clap = { version = "4.5.40", features = ["derive"] }

---

use clap::Parser;
use std::net::{UdpSocket, Ipv4Addr};
use std::io::Read;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(short = 'z', long, help = "path to wasm payload")]
    payload: std::path::PathBuf,

    #[clap(short='s', long, help = "packetization level", default_value_t = 128)]
    packet_size: usize,

    #[clap(short='i', long, help = "Destination IP", default_value_t = String::from("10.42.0.61"))]
    dest_ip: String,

    #[clap(short='p', long, help = "Destination Port", default_value_t = 1234)]
    dest_port: u16,
}



fn main() -> std::io::Result<()> {
    let args = Args::parse();

    println!("{:?}", args);
    let Args {payload, packet_size, dest_ip, dest_port} = args;
    let ip_addr = dest_ip.parse::<Ipv4Addr>().unwrap();

    let out_addr: (Ipv4Addr, u16) = (ip_addr, dest_port);

    println!("Binding Socket");
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    let mut file_buf = Vec::new();
    let mut payload_f = std::fs::File::open(payload)?;
    payload_f.read_to_end(&mut file_buf)?;


    let payload_size = file_buf.len() as u32;
    println!("Payload size: {payload_size}");
    // Send file size
    socket.send_to(
        &payload_size.to_be_bytes(), out_addr
    )?;

    std::thread::sleep(std::time::Duration::from_millis(10));

    for chunk in file_buf.chunks(packet_size) {
        println!("Sending chunk of size {}", chunk.len());
        socket.send_to(
            chunk, out_addr
        )?;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
}