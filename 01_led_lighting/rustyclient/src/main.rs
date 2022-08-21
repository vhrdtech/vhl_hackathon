#![allow(unused_imports)]
#![allow(unused_variables)]

use std::env;
use std::net::SocketAddr;
use anyhow::{Context, Result};
use tokio::net::{TcpStream};
use tokio::io::AsyncWriteExt;
use vhl_cg::point::Point;
use vhl_stdlib::discrete::{U2Sp1, U4};
use vhl_stdlib::serdes::{NibbleBuf, NibbleBufMut};
use vhl_stdlib::serdes::buf::BufMut;
use vhl_stdlib::serdes::traits::SerializeBytes;
use vhl_stdlib::serdes::vlu4::Vlu4SliceArray;
use vhl_stdlib::serdes::xpi_vlu4::addressing::{NodeSet, RequestId, XpiResourceSet};
use vhl_stdlib::serdes::xpi_vlu4::{MultiUri, NodeId, Uri};
use vhl_stdlib::serdes::xpi_vlu4::priority::Priority;
use vhl_stdlib::serdes::xpi_vlu4::request::{XpiRequest, XpiRequestKind};

use vhl_stdlib::serdes::nibble_buf::Error as NibbleBufError;
use vhl_stdlib::serdes::buf::Error as BufError;

#[derive(Debug)]
enum MyError {
    NibbleBufError(NibbleBufError),
    BufError(BufError),
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

    // let buf = [
    //     0b000_100_11, 0b1_00_00000, 0b001_101010, 0b1_0101010,
    //     0b0011_1100, 0b0001_0010, num, 0xbb, 0b000_11011
    // ];
    //
    // println!("Sending: {:02x?}", buf);

    let mut buf = [0u8; 32];
    let mut wgr = NibbleBufMut::new_all(&mut buf);

    // let request_data = [0x22, 3, 4, 6, 5];
    // let request_kind = XpiRequestKind::Call { args_set: Vlu4SliceArray::new(
    //     2,
    //     NibbleBuf::new(&request_data[0..1], 2).unwrap(),
    //     NibbleBuf::new(&request_data[1..=4], 8).unwrap()
    // ) };

    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 5, y: 7 };

    let mut args_set = [0u8; 128];
    let args_set = {
        let wgr = NibbleBufMut::new_all(&mut args_set);
        let mut wgr = wgr.put_slice_array();
        wgr.put_exact::<MyError, _>(p1.len_bytes() + p2.len_bytes(), |slice| {
            let mut wgr = BufMut::new(slice);
            wgr.put(&p1)?;
            wgr.put(&p2)?;
            Ok(())
        }).unwrap();
        wgr.finish_as_slice_array().unwrap()
    };
    println!("{}", args_set);
    let request_kind = XpiRequestKind::Call { args_set };

    // let multi_uri: MultiUri = NibbleBuf::new_all(&[0x10, 0x52, 0x34]).des_vlu4().unwrap();
    // let resource_set = XpiResourceSet::MultiUri(multi_uri);
    let resource_set = XpiResourceSet::Uri(Uri::OnePart4(U4::new(5).unwrap())); // /sync

    let request = XpiRequest {
        source: NodeId::new(42).unwrap(),
        destination: NodeSet::Unicast(NodeId::new(85).unwrap()),
        resource_set,
        kind: request_kind,
        request_id: RequestId::new(27).unwrap(),
        priority: Priority::Lossless(U2Sp1::new(1).unwrap())
    };
    wgr.put(request).unwrap();
    let (buf, byte_pos, _) = wgr.finish();

    // let ecbridge_node = local_node.by_id(85);
    // let p = ecbridge_node.sync_3(p1, p2).await; -> Result<Point, Error>

    // tx.write_all(wgr.finish()).await?;
    tx.write_all(&buf[0..byte_pos]).await?;

    Ok(())
}
