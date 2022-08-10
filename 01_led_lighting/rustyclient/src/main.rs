use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::net::{TcpStream};
use tokio::io::AsyncWriteExt;
use vhl_stdlib::nibble_buf::NibbleBufMut;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "192.168.0.199:7777";
    let addr = addr.parse::<SocketAddr>()
        .context(format!("unable to parse socket address: '{}'", addr))?;

    // let stream = tcp_socket.connect(addr).await?;
    let mut stream = TcpStream::connect(addr).await?;
    let (rx, mut tx) = stream.split();

    let mut buf = [0u8; 128];
    let mut wgr = NibbleBufMut::new(&mut buf);
    wgr.put_nibble(2);
    wgr.put_u8(0xaa);
    wgr.put_vlu4_u32(1234567);

    tx.write_all(wgr.finish()).await?;

    Ok(())
}
