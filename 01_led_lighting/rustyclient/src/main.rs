#![allow(unused_imports)]
// #![allow(unused_variables)]

use std::collections::HashMap;
use std::env;
use std::net::{AddrParseError, SocketAddr};
use std::sync::{Arc, RwLock, TryLockResult};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures::channel::mpsc;
use futures::channel::mpsc::Receiver;
use futures::{SinkExt, StreamExt};
use tracing::{debug, info, Level, trace};
use tracing_subscriber::FmtSubscriber;

use vhl_cg::point::Point;

use vhl_stdlib::discrete::{U2, U4};
use vhl_stdlib::serdes::{Buf, NibbleBuf, NibbleBufMut};
use vhl_stdlib::serdes::buf::BufMut;
use vhl_stdlib::serdes::traits::SerializeBytes;
use vhl_stdlib::serdes::nibble_buf::Error as NibbleBufError;
use vhl_stdlib::serdes::buf::Error as BufError;
use vhl_stdlib::serdes::bit_buf::Error as BitBufError;
use vhl_stdlib::serdes::vlu4::{Vlu32N, Vlu4Vec, Vlu4VecBuilder};
use xpi::error::XpiError;
use xpi::event_kind::XpiEventDiscriminant;

use xpi::owned::{NodeSet, RequestId, ResourceSet, Priority, NodeId, Event, EventKind, UriOwned, ResourceInfo, MultiUriOwned};
use xpi::xwfd::UriMask;
use xpi_node::node::async_std::{VhNode, NodeError};
use xpi_node::node::addressing::RemoteNodeAddr;
use xpi_node::node::filter::{EventFilter, EventKindFilter, NodeSetFilter, ResourceSetFilter, SourceFilter};

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

// to be cg-d
struct ECBridgeClient {
    node: VhNode
}

impl ECBridgeClient {
    pub async fn new(local_id: NodeId) -> Self {
        Self {
            node: VhNode::new_client(local_id).await
        }
    }

    pub async fn connect_remote(&mut self, addr: RemoteNodeAddr) -> Result<(), NodeError> {
        self.node.connect_remote(addr, vec![NodeId(0)]).await
    }

    #[allow(dead_code)]
    pub async fn call_sync(&mut self, p1: Point, p2: Point) -> Result<Point> {
        let mut args = Vec::new();
        args.resize(8, 0);
        let mut nwr = NibbleBufMut::new_all(&mut args);
        nwr.put(&p1)?;
        nwr.put(&p2)?;

        let request_id = RequestId(3);
        let dst_node_id = NodeId(0);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::Uri(UriOwned::new(&[0, 11, 2, 0])),
            EventKind::Call {
                args_set: vec![nwr.to_nibble_buf_owned()]
            },
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let reply = self.node.filter_one(
            EventFilter::new_with_timeout(Duration::from_millis(100))
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::One(XpiEventDiscriminant::CallResults))
                .request_id(request_id)
        ).await?;
        trace!("filter_one returned: {}", reply);
        match reply.kind {
            EventKind::CallResults(results) => {
                if results.len() != 1 {
                    return Err(NodeError::ExpectedDifferentAmountOf("CallComplete results".to_owned()).into());
                }
                match &results[0] {
                    Ok(result) => {
                        let mut nrd = result.to_nibble_buf_ref();
                        let p: Point = nrd.des_vlu4().context("Deserializing reply")?;
                        Ok(p)
                    }
                    Err(e) => {
                        Err(e.clone().into())
                    }
                }
            }
            u => {
                Err(NodeError::ExpectedReplyKind("CallComplete".to_owned(), format!("{:?}", u.discriminant())).into())
            }
        }
    }

    #[allow(dead_code)]
    pub async fn call1_unit(&mut self, uri: UriOwned) -> Result<()> {
        let mut args = Vec::new();
        let mut nwr = NibbleBufMut::new_all(&mut args);

        let request_id = RequestId(3);
        let dst_node_id = NodeId(0);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::Uri(uri),
            EventKind::Call {
                args_set: vec![nwr.to_nibble_buf_owned()]
            },
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let reply = self.node.filter_one(
            EventFilter::new_with_timeout(Duration::from_millis(100))
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::One(XpiEventDiscriminant::CallResults))
                .request_id(request_id)
        ).await?;
        trace!("filter_one returned: {}", reply);
        match reply.kind {
            EventKind::CallResults(results) => {
                if results.len() != 1 {
                    return Err(NodeError::ExpectedDifferentAmountOf("CallComplete results".to_owned()).into());
                }
                match &results[0] {
                    Ok(result) => {
                        Ok(())
                    }
                    Err(e) => {
                        Err(e.clone().into())
                    }
                }
            }
            u => {
                Err(NodeError::ExpectedReplyKind("CallComplete".to_owned(), format!("{:?}", u.discriminant())).into())
            }
        }
    }

    #[allow(dead_code)]
    pub async fn call_many_unit(&mut self, multi_uri: MultiUriOwned) -> Result<()> {
        let mut args = Vec::new();
        let mut nwr = NibbleBufMut::new_all(&mut args);

        let request_id = RequestId(3);
        let dst_node_id = NodeId(0);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::MultiUri(multi_uri),
            EventKind::Call {
                args_set: vec![nwr.to_nibble_buf_owned(), nwr.to_nibble_buf_owned()]
            },
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let reply = self.node.filter_one(
            EventFilter::new_with_timeout(Duration::from_millis(100))
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::One(XpiEventDiscriminant::CallResults))
                .request_id(request_id)
        ).await?;
        trace!("filter_one returned: {}", reply);
        match reply.kind {
            EventKind::CallResults(results) => {
                // if results.len() != 1 {
                //     return Err(NodeError::ExpectedDifferentAmountOf("CallComplete results".to_owned()).into());
                // }
                // match &results[0] {
                //     Ok(result) => {
                //         Ok(())
                //     }
                //     Err(e) => {
                //         Err(e.clone().into())
                //     }
                // }
                Ok(())
            }
            u => {
                Err(NodeError::ExpectedReplyKind("CallComplete".to_owned(), format!("{:?}", u.discriminant())).into())
            }
        }
    }

    pub async fn observe_one(&mut self) -> Result<Receiver<u8>, NodeError> {
        let request_id = RequestId(3);
        let dst_node_id = NodeId(0);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::Uri(UriOwned::new(&[2, ])),
            EventKind::Subscribe {
                rates: Vec::new()
            },
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let mut updates = self.node.filter_many(
            EventFilter::new()
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::Two(XpiEventDiscriminant::SubscribeResults, XpiEventDiscriminant::StreamUpdates))
                .resource_set(ResourceSetFilter::ContainsUri(UriOwned::new(&[2])))
                .drop_on_remote_disconnect(true)
                .request_id(request_id)
        ).await?;
        let (mut tx, rx) = mpsc::channel(1);
        tokio::spawn(async move {
            while let Some(event) = updates.next().await {
                debug!("event in observe: {:?}", event);
                if tx.send(1).await.is_err() {
                    break
                }
            }
        });
        Ok(rx)
    }

    #[allow(dead_code)]
    pub async fn write_digit(&mut self, digit: u8) -> Result<()> {
        let mut args = Vec::new();
        args.resize(8, 0);
        let mut nwr = NibbleBufMut::new_all(&mut args);
        nwr.put(&digit)?;

        let request_id = RequestId(3);
        let dst_node_id = NodeId(1);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::Uri(UriOwned::new(&[1])),
            EventKind::Write {
                values: vec![nwr.to_nibble_buf_owned()]
            },
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let reply = self.node.filter_one(
            EventFilter::new_with_timeout(Duration::from_millis(100))
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::One(XpiEventDiscriminant::WriteResults))
                .request_id(request_id)
        ).await?;
        trace!("filter_one returned: {}", reply);
        match reply.kind {
            EventKind::WriteResults(results) => {
                if results.len() != 1 {
                    return Err(NodeError::ExpectedDifferentAmountOf("WriteResults results".to_owned()).into());
                }
                match &results[0] {
                    Ok(_) => {
                        Ok(())
                    }
                    Err(e) => {
                        Err(e.clone().into())
                    }
                }
            }
            u => {
                Err(NodeError::ExpectedReplyKind("WriteResults".to_owned(), format!("{:?}", u.discriminant())).into())
            }
        }
    }


    #[allow(dead_code)]
    pub async fn read_digit(&mut self) -> Result<u8> {
        let request_id = RequestId(3);
        let dst_node_id = NodeId(0);
        let ev = Event::new_with_default_ttl(
            self.node.node_id(),
            NodeSet::Unicast(dst_node_id),
            ResourceSet::Uri(UriOwned::new(&[1])),
            EventKind::Read,
            request_id,
            Priority::Lossy(0)
        );
        self.node.submit_one(ev).await?;
        let reply = self.node.filter_one(
            EventFilter::new_with_timeout(Duration::from_millis(100))
                .src(SourceFilter::NodeId(dst_node_id))
                .dst(NodeSetFilter::NodeId(self.node.node_id()))
                .kind(EventKindFilter::One(XpiEventDiscriminant::ReadResults))
                .request_id(request_id)
        ).await?;
        trace!("filter_one returned: {}", reply);
        match reply.kind {
            EventKind::ReadResults(results) => {
                if results.len() != 1 {
                    return Err(NodeError::ExpectedDifferentAmountOf("WriteResults results".to_owned()).into());
                }
                match &results[0] {
                    Ok(result) => {
                        let mut nrd = result.to_nibble_buf_ref();
                        let digit: u32 = nrd.des_vlu4().context("Deserializing reply")?;
                        Ok(digit as u8)
                    }
                    Err(e) => {
                        Err(e.clone().into())
                    }
                }
            }
            u => {
                Err(NodeError::ExpectedReplyKind("WriteResults".to_owned(), format!("{:?}", u.discriminant())).into())
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_env_filter("rustyclient=trace,xpi_node=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // let addr = "tcp://192.168.0.199:7777";
    let addr = "tcp://127.0.0.1:7777";
    let addr = RemoteNodeAddr::parse(addr)
        .context(format!("unable to parse socket address: '{}'", addr))?;

    // // Establish connection to another node with statically generated xPI
    // // SemVer compatibility checks must pass before any requests can be sent
    // let ecbridge_client = ECBridgeClient::connect(&mut client_node, ecbridge_node_id).await?;
    let mut ecbridge_client = ECBridgeClient::new(NodeId(1)).await;

    // let mut local11 = VhNode::new_client(NodeId(11)).await;
    // VhNode::connect_instances(&mut local10, &mut local11).await?;

    // let smth = local10.filter_one( () ).await;
    // println!("filter one: {:?}", smth);

    ecbridge_client.connect_remote(addr).await?;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut updates = ecbridge_client.observe_one().await?;
    while let Some(value) = updates.next().await {
        info!("new value: {value}");
    }

    // debug!("call_sync_unit: {:?}", ecbridge_client.call1_unit(UriOwned::new(&[0, 11, 2, 0])).await?);
    // debug!("call_sync_unit: {:?}", ecbridge_client.call1_unit(UriOwned::new(&[0, 11, 2, 1])).await?);

    // let mut mu = MultiUriOwned::new();
    // mu.push(UriOwned::new(&[0, 11, 2]), UriMask::ByBitfield8(0b1100_0000));
    // let before = Instant::now();
    // debug!("call_many_unit: {:?}", ecbridge_client.call_many_unit(mu).await);
    // debug!("dt: {:?}", before.elapsed());

    // let write_result = ecbridge_client.write_digit(7).await;
    // debug!("Write digit: {:?}", write_result);
    //
    // let digit = ecbridge_client.read_digit().await;
    // debug!("Read digit: {:?}", digit);



    // let (mut rx, mut tx) = stream.split();
    // //
    // let multi_uri: MultiUri = NibbleBuf::new_all(&[0x10, 0x52, 0x55]).des_vlu4().unwrap();
    // let resource_set = XpiResourceSet::MultiUri(multi_uri);
    // println!("{}", resource_set);
    //
    // let mut buf = [0u8; 32];
    // let request_builder = XpiRequestBuilder::new(
    //     NibbleBufMut::new_all(&mut buf),
    //     NodeId::new(33).unwrap(),
    //     NodeSet::Unicast(NodeId::new(44).unwrap()),
    //     resource_set,
    //     RequestId::new(27).unwrap(),
    //     Priority::Lossy(U2Sp1::new(1).unwrap())
    // ).unwrap();
    // let nwr = request_builder.build_kind_with(|nwr| {
    //     let mut vb = nwr.put_vec::<&[u8]>();
    //
    //     vb.put_aligned_with::<BufError, _>(8, |slice| {
    //         let mut wgr = BufMut::new(slice);
    //         wgr.put(&Point { x: 10, y: 20 })?;
    //         wgr.put(&Point { x: 5, y: 7 })?;
    //         Ok(())
    //     })?;
    //
    //     let nwr = vb.finish()?;
    //     Ok((XpiRequestKindKind::Call, nwr))
    // }).unwrap();
    //
    // let (buf, byte_pos, _) = nwr.finish();
    //
    // println!("Send: {:2x?}", &buf[0..byte_pos]);
    // tx.write_all(&buf[0..byte_pos]).await?;
    //
    // let mut buf = Vec::new();
    // buf.resize(15, 0);
    // let reply_size = rx.read_exact(&mut buf).await?;
    // println!("Read {}: {:2x?}", reply_size, buf);
    // let mut rdr = NibbleBuf::new_all(&buf[0..reply_size]);
    // let reply: XpiReply = rdr.des_vlu4().unwrap();
    // println!("{:?}", reply);

    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}
