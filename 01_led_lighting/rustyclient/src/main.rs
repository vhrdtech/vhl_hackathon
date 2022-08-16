use std::env;
use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::net::{TcpStream};
use tokio::io::AsyncWriteExt;
use vhl_stdlib::serdes::NibbleBufMut;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let addr = "192.168.0.199:7777";
    let addr = addr.parse::<SocketAddr>()
        .context(format!("unable to parse socket address: '{}'", addr))?;

    let num: u8 = args[1].parse()?;

    // let stream = tcp_socket.connect(addr).await?;
    let mut stream = TcpStream::connect(addr).await?;
    let (rx, mut tx) = stream.split();

    // let mut buf = [0u8; 128];
    // let mut wgr = NibbleBufMut::new(&mut buf);
    // wgr.put_nibble(2);
    // wgr.put_u8(0xaa);
    // wgr.put_vlu4_u32(1234567);

    let buf = [
        0b000_100_11, 0b1_00_00000, 0b001_101010, 0b1_0101010,
        0b0011_1100, 0b0001_0010, num, 0xbb, 0b000_11011
    ];

    println!("Sending: {:02x?}", buf);

    // tx.write_all(wgr.finish()).await?;
    tx.write_all(&buf).await?;

    Ok(())
}
