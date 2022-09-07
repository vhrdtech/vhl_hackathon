#![allow(unused_imports)]
#![allow(unused_variables)]

use std::env;
use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::net::{TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vhl_cg::point::Point;
use vhl_stdlib::discrete::{U2Sp1, U4};
use vhl_stdlib::serdes::{NibbleBuf, NibbleBufMut};
use vhl_stdlib::serdes::buf::BufMut;
use vhl_stdlib::serdes::traits::SerializeBytes;
use vhl_stdlib::serdes::xpi_vlu4::addressing::{NodeSet, RequestId, XpiResourceSet};
use vhl_stdlib::serdes::xpi_vlu4::{MultiUri, NodeId, Uri};
use vhl_stdlib::serdes::xpi_vlu4::priority::Priority;
use vhl_stdlib::serdes::xpi_vlu4::request::{XpiRequest, XpiRequestBuilder, XpiRequestKind, XpiRequestKindKind};

use vhl_stdlib::serdes::nibble_buf::Error as NibbleBufError;
use vhl_stdlib::serdes::buf::Error as BufError;
use vhl_stdlib::serdes::bit_buf::Error as BitBufError;
use vhl_stdlib::serdes::vlu4::{Vlu4Vec, Vlu4VecBuilder};
use vhl_stdlib::serdes::xpi_vlu4::reply::XpiReply;

#[derive(Debug)]
enum MyError {
    NibbleBufError(NibbleBufError),
    BufError(BufError),
    BitBufError(BitBufError),
}

impl From<NibbleBufError> for MyError {
    fn from(e: NibbleBufError) -> Self {
        MyError::NibbleBufError(e)
    }
}

impl From<BufError> for MyError {
    fn from(e: BufError) -> Self {
        MyError::BufError(e)
    }
}

impl From<BitBufError> for MyError {
    fn from(e: BitBufError) -> Self {
        MyError::BitBufError(e)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let addr = "192.168.0.199:7777";
    let addr = addr.parse::<SocketAddr>()
        .context(format!("unable to parse socket address: '{}'", addr))?;

    let num: u8 = args[1].parse()?;

    // let stream = tcp_socket.connect(addr).await?;
    let mut stream = TcpStream::connect(addr).await?;
    let (mut rx, mut tx) = stream.split();

    let multi_uri: MultiUri = NibbleBuf::new_all(&[0x10, 0x52, 0x55]).des_vlu4().unwrap();
    let resource_set = XpiResourceSet::MultiUri(multi_uri);
    println!("{}", resource_set);
    // let resource_set = XpiResourceSet::Uri(Uri::OnePart4(U4::new(5).unwrap())); // /sync

    let mut buf = [0u8; 32];
    let request_builder = XpiRequestBuilder::new(
        NibbleBufMut::new_all(&mut buf),
        NodeId::new(33).unwrap(),
        NodeSet::Unicast(NodeId::new(44).unwrap()),
        resource_set,
        RequestId::new(27).unwrap(),
        Priority::Lossy(U2Sp1::new(1).unwrap())
    ).unwrap();
    let nwr = request_builder.build_kind_with(|nwr| {
        let mut vb = nwr.put_vec::<&[u8]>();

        vb.put_aligned_with::<BufError, _>(8, |slice| {
            let mut wgr = BufMut::new(slice);
            wgr.put(&Point { x: 10, y: 20 })?;
            wgr.put(&Point { x: 5, y: 7 })?;
            Ok(())
        })?;
        // vb.put_aligned_with::<BufError, _>(8, |slice| {
        //     let mut wgr = BufMut::new(slice);
        //     wgr.put(&Point { x: 5, y: 3 })?;
        //     wgr.put(&Point { x: 6, y: 4 })?;
        //     Ok(())
        // })?;

        let nwr = vb.finish()?;
        Ok((XpiRequestKindKind::Call, nwr))
    }).unwrap();

    let (buf, byte_pos, _) = nwr.finish();

    // let mut nrd = NibbleBuf::new_all(&buf[0..byte_pos]);
    // println!("{}", nrd);
    // let req: XpiRequest = nrd.des_vlu4().unwrap();
    // println!("{}", req);

    // let ecbridge_node = local_node.by_id(85);
    // let p = ecbridge_node.sync_3(p1, p2).await; -> Result<Point, Error>

    // tx.write_all(wgr.finish()).await?;
    println!("Send: {:2x?}", &buf[0..byte_pos]);
    tx.write_all(&buf[0..byte_pos]).await?;

    let mut buf = Vec::new();
    buf.resize(15, 0);
    let reply_size = rx.read_exact(&mut buf).await?;
    println!("Read {}: {:2x?}", reply_size, buf);
    let mut rdr = NibbleBuf::new_all(&buf[0..reply_size]);
    let reply: XpiReply = rdr.des_vlu4().unwrap();
    println!("{:?}", reply);

    Ok(())
}
