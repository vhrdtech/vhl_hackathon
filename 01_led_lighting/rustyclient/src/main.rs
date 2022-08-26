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
use vhl_stdlib::serdes::xpi_vlu4::request::{XpiRequest, XpiRequestKind};

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

    let mut buf = [0u8; 32];
    let mut wrr = NibbleBufMut::new_all(&mut buf);

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
        let mut arr = Vlu4VecBuilder::<&[u8]>::new(&mut args_set);
        arr.put_aligned_with::<MyError, _>(p1.len_bytes() + p2.len_bytes(), |slice| {
            let mut wgr = BufMut::new(slice);
            wgr.put(&p1)?;
            wgr.put(&p2)?;
            Ok(())
        }).unwrap();
        arr.put_aligned_with::<MyError, _>(8, |slice| {
            let mut wgr = BufMut::new(slice);
            wgr.put(&Point { x: 5, y: 3 })?;
            wgr.put(&Point { x: 6, y: 4 })?;
            Ok(())
        }).unwrap();
        arr.finish_as_vec().unwrap()
    };
    println!("{:?}", args_set);
    let request_kind = XpiRequestKind::Call { args_set };

    let multi_uri: MultiUri = NibbleBuf::new_all(&[0x10, 0x52, 0x55]).des_vlu4().unwrap();
    let resource_set = XpiResourceSet::MultiUri(multi_uri);
    println!("{}", resource_set);
    // let resource_set = XpiResourceSet::Uri(Uri::OnePart4(U4::new(5).unwrap())); // /sync


    wrr.skip(8).unwrap();
    wrr.put(&resource_set).unwrap();
    wrr.put(&args_set).unwrap();
    wrr.put(&RequestId::new(27).unwrap()).unwrap();
    wrr.rewind::<_, MyError>(0, |wrr| {
        wrr.as_bit_buf::<MyError, _>(|wrr| {
            wrr.put_up_to_8(3, 0b000)?; // unused 31:29
            wrr.put(&Priority::Lossless(U2Sp1::new(1).unwrap()))?; // bits 28:26
            wrr.put_bit(true)?; // bit 25, is_unicast
            wrr.put_bit(true)?; // bit 24, is_request
            wrr.put_bit(true)?; // bit 23, reserved
            wrr.put(&NodeId::new(33).unwrap())?; // bits 22:16
            wrr.put_up_to_8(2, 0b00)?; // bits 15:7 - discriminant of NodeSet (2b) + 7b for NodeId or other
            wrr.put(&NodeId::new(44).unwrap())?; // unicast dest NodeId
            wrr.put(&resource_set)?; // bits 6:4 - discriminant of ResourceSet+Uri
            wrr.put_up_to_8(4, 0b0000)?; // bits 3:0 - discriminant of XpiRequestKind
            Ok(())
        })?;
        Ok(())
    }).unwrap();


    let (buf, byte_pos, _) = wrr.finish();

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
    buf.resize(22, 0);
    let reply_size = rx.read_exact(&mut buf).await?;
    println!("Read {}: {:2x?}", reply_size, buf);
    let mut rdr = NibbleBuf::new_all(&buf[0..reply_size]);
    let reply: XpiReply = rdr.des_vlu4().unwrap();
    println!("{:?}", reply);

    Ok(())
}
