use core::mem::size_of;
use dwt_systick_monotonic::fugit;
use smoltcp::iface::{
    Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes,
    SocketStorage,
    SocketHandle,
};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, IpEndpoint, Ipv6Cidr};
use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
use stm32h7xx_hal::{ethernet as ethernet_h7, stm32};
use stm32h7xx_hal::ethernet::PinsRMII;
use stm32h7xx_hal::rcc::{CoreClocks, rec};
use serde::{Serialize, Deserialize};
use crate::{log_debug, log_error, log_info, log_trace, log_warn};
use rtic::Mutex;

const T: u8 = 0;

/// Locally administered MAC address
pub const MAC_ADDRESS: [u8; 6] = [0x02, 0x00, 0x11, 0x22, 0x33, 0x44];

/// Ethernet descriptor rings are a global singleton
#[link_section = ".sram3.eth"]
pub static mut DES_RING: ethernet_h7::DesRing<4, 4> = ethernet_h7::DesRing::new();

/// Net storage with static initialisation - another global singleton
pub struct NetStorageStatic<'a> {
    ip_addrs: [IpCidr; 1],
    socket_storage: [SocketStorage<'a>; 8],
    neighbor_cache_storage: [Option<(IpAddress, Neighbor)>; 8],
    routes_storage: [Option<(IpCidr, Route)>; 1],
}
pub static mut STORE: NetStorageStatic = NetStorageStatic {
    // Garbage
    ip_addrs: [IpCidr::Ipv6(Ipv6Cidr::SOLICITED_NODE_PREFIX)],
    socket_storage: [SocketStorage::EMPTY; 8],
    neighbor_cache_storage: [None; 8],
    routes_storage: [None; 1],
};

pub type Lan8742A = ethernet_h7::phy::LAN8742A<ethernet_h7::EthernetMAC>;

pub struct Net<'a> {
    iface: Interface<'a, ethernet_h7::EthernetDMA<'a, 4, 4>>,
    tcp_handle: SocketHandle,
}

impl<'a> Net<'a> {
    pub fn new(
        store: &'static mut NetStorageStatic<'a>,
        ethdev: ethernet_h7::EthernetDMA<'a, 4, 4>,
        ethernet_addr: HardwareAddress,
    ) -> Self {
        // Set IP address
        store.ip_addrs =
            [IpCidr::new(IpAddress::v4(192, 168, 0, 199).into(), 0)];

        let neighbor_cache =
            NeighborCache::new(&mut store.neighbor_cache_storage[..]);
        let routes = Routes::new(&mut store.routes_storage[..]);

        let mut iface =
            InterfaceBuilder::new(ethdev, &mut store.socket_storage[..])
                .hardware_addr(ethernet_addr)
                .neighbor_cache(neighbor_cache)
                .ip_addrs(&mut store.ip_addrs[..])
                .routes(routes)
                .finalize();

        let tcp_socket = {
            static mut TCP_SERVER_RX_DATA: [u8; 128] = [0; 128];
            static mut TCP_SERVER_TX_DATA: [u8; 128] = [0; 128];
            let tcp_rx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
            let tcp_tx_buffer = TcpSocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
            TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer)
        };

        let tcp_handle = iface.add_socket(tcp_socket);

        return Net { iface, tcp_handle };
    }

    fn now() -> Instant {
        let now: u64 = crate::app::monotonics::now().duration_since_epoch().to_millis();
        // rprintln!("now(): {}ms", now);
        Instant::from_millis(now as i64)
    }

    /// Polls on the ethernet interface.
    pub fn poll(&mut self) -> bool {
        self.iface
            .poll(Self::now())
            .unwrap_or_else(|e|  {
                if e != smoltcp::Error::Unrecognized {
                    log_warn!(=>T, "Poll err: {:?}", e);
                }
                false
            })
    }

    pub fn poll_at(&mut self) -> Option<smoltcp::time::Instant> {
        self.iface.poll_at(Self::now())
    }
}

pub struct PollAtHandle {
    pub originally_scheduled_at: crate::Instant,
    pub handle: crate::app::smoltcp_poll_at::SpawnHandle
}

pub fn init(
    eth_mac: stm32::ETHERNET_MAC,
    eth_mtl: stm32::ETHERNET_MTL,
    eth_dma: stm32::ETHERNET_DMA,
    pins: impl PinsRMII,
    prec: rec::Eth1Mac,
    clocks: &CoreClocks
) -> (Net<'static>, Lan8742A) {
    let mac_addr = EthernetAddress::from_bytes(&MAC_ADDRESS);
    let (eth_dma, eth_mac) = unsafe {
        ethernet_h7::new(
            eth_mac, eth_mtl, eth_dma,
            pins,
            &mut DES_RING,
            mac_addr.clone(),
            prec,
            clocks
        )
    };

    // Initialise ethernet PHY...
    log_info!(=>T, "PHY init...");
    let mut lan8742a = ethernet_h7::phy::LAN8742A::new(eth_mac);
    use stm32h7xx_hal::ethernet::PHY;
    lan8742a.phy_reset();
    lan8742a.phy_init();
    log_info!(=>T, "PHY init done.");
    // The eth_dma should not be used until the PHY reports the link is up

    unsafe { ethernet_h7::enable_interrupt() };

    // unsafe: mutable reference to static storage, we only do this once
    let store = unsafe { &mut STORE };
    let net = Net::new(store, eth_dma, mac_addr.into());
    (net, lan8742a)
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct IpEndpointL {
    pub addr: IpAddressL,
    pub port: u16,
}

impl TryFrom<IpEndpoint> for IpEndpointL {
    type Error = ();

    fn try_from(value: IpEndpoint) -> Result<Self, Self::Error> {
        Ok(IpEndpointL {
            addr: value.addr.try_into()?,
            port: value.port
        })
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum IpAddressL {
    Ipv4([u8; 4]),
    #[cfg(feature = "proto-ipv6")]
    Ipv6([u8; 16]),
}

impl TryFrom<smoltcp::wire::IpAddress> for IpAddressL {
    type Error = ();

    fn try_from(value: IpAddress) -> Result<Self, Self::Error> {
        match value {
            IpAddress::Unspecified => Err(()),
            IpAddress::Ipv4(v4) => Ok(IpAddressL::Ipv4(v4.0)),
            #[cfg(feature = "proto-ipv6")]
            IpAddress::Ipv6(v6) => Ok(IpAddressL::Ipv6(v6.0)),
            _ => Err(())
        }
    }
}

pub fn ethernet_event(mut ctx: crate::app::ethernet_event::Context) {
    let time = crate::app::monotonics::now().duration_since_epoch().to_micros();
    log_trace!(=>T, "\nethernet_event: {}us", time);
    let mut useful_work = false;

    unsafe { ethernet_h7::interrupt_handler() }
    ctx.local.led_act.toggle();

    let tcp_handle = ctx.local.net.tcp_handle;
    let eth_out_prod: &mut bbqueue::Producer<512> = ctx.local.eth_out_prod;
    let eth_in_cons: &mut bbqueue::Consumer<512> = ctx.local.eth_in_cons;
    let net: &mut Net = ctx.local.net;

    // for i in 0..2 {
        // rprintln!("poll time: {}us", crate::app::monotonics::now().duration_since_epoch().to_micros());

    let poll1 = net.poll();

    let tcp_socket: &mut TcpSocket = net.iface.get_socket(
        tcp_handle
    );
    // rprintln!("{:?}", tcp_socket.state());

    if tcp_socket.state() == smoltcp::socket::TcpState::CloseWait {
        tcp_socket.close(); useful_work = true;
    }
    if !tcp_socket.is_open() {
        let r = tcp_socket.listen(7777);
        log_info!(=>T, "tcp_socket: listen(): {:?}", r);
        useful_work = true;
    }

    handle_tcp_rx(tcp_socket, eth_out_prod, &mut useful_work);
    handle_tcp_tx(tcp_socket, eth_in_cons, &mut useful_work);

    let time = crate::app::monotonics::now().duration_since_epoch().to_micros();
    // rprintln!("poll time: {}us", crate::app::monotonics::now().duration_since_epoch().to_micros());

    let mut poll2: Option<bool> = None;
    match net.poll_at() {
        Some(advised_instant) => {
            if advised_instant.total_micros() == 0 {
                poll2 = Some(net.poll());
                log_debug!(=>T, "Should try to cancel?");
            } else {
                let poll_at_handle: Option<PollAtHandle> = ctx.shared.poll_at_handle.lock(|h| h.take());
                log_debug!(=>T, "now: {}us advised to run at: {}us", time, advised_instant.total_micros());
                let advised_instant = crate::Instant::from_ticks(
                    advised_instant.total_micros() as u64 * (crate::CORE_FREQ as u64 / 1_000_000)
                );
                let poll_at_handle = match poll_at_handle {
                    Some(poll_at_handle) => {
                        log_debug!(=>T, "handle before exists, t={}us", poll_at_handle.originally_scheduled_at.duration_since_epoch().to_micros());
                        if advised_instant < poll_at_handle.originally_scheduled_at {
                            log_debug!(=>T, "rescheduling smoltcp_poll_at at an earlier time");
                            poll_at_handle.handle.reschedule_at(advised_instant).map(|handle| {
                                log_debug!(=>T, "reschedule success");
                                PollAtHandle {
                                    originally_scheduled_at: advised_instant,
                                    handle
                                }
                            }).ok()
                        } else {
                            log_debug!(=>T, "no need to reschedule");
                            Some(poll_at_handle)
                        }
                    }
                    None => {
                        log_debug!(=>T, "handle before is None");
                        match crate::app::smoltcp_poll_at::spawn_at(advised_instant) {
                            Ok(handle) => {
                                Some(PollAtHandle {
                                    originally_scheduled_at: advised_instant,
                                    handle
                                })
                            }
                            Err(_) => {
                                log_warn!(=>T, "Scheduling smoltcp_poll_at failed!");
                                None
                            }
                        }
                    }
                };
                ctx.shared.poll_at_handle.lock(|h| *h = poll_at_handle);
            }
        }
        None => {
            log_debug!(=>T, "Should try to cancel on None?");
        }
    }

    log_warn!(=>T, "definitely_useful={} poll1={} poll2={:?}", useful_work, poll1, poll2);
}

fn handle_tcp_rx(tcp_socket: &mut TcpSocket, eth_out_prod: &mut bbqueue::Producer<512>, useful_work: &mut bool) {
    let remote_endpoint =  tcp_socket.remote_endpoint();
    if tcp_socket.can_recv() {
        match tcp_socket.recv(|buffer| {
            // dequeue the amount returned
            (buffer.len(), buffer)
        }) {
            Ok(buf) => {
                // done_smth_useful = true;
                // rprintln!("tcp_socket: recv: {} {:02x?}", buf.len(), buf);
                let endpoint: IpEndpointL = match remote_endpoint.try_into() {
                    Ok(endpoint) => endpoint,
                    Err(_) => {
                        log_error!(=>T, "wrong endpoint address");
                        return;
                    }
                };

                // do not remove +1 from IpEndpointL size, because when ipv6 is disabled
                // enum have only one variant and is optimized to be 0 size, but
                // serializer still use 1 byte for the discriminant
                match eth_out_prod.grant_exact(buf.len() + size_of::<IpEndpointL>() + 1) {
                    Ok(mut wgr) => {
                        let endpoint_ser_len = ssmarshal::serialize(&mut wgr, &endpoint).unwrap();
                        wgr[endpoint_ser_len .. buf.len() + endpoint_ser_len].copy_from_slice(buf);
                        wgr.commit(buf.len() + endpoint_ser_len); *useful_work = true;
                        let r = crate::app::link_process::spawn();
                        if r.is_err() {
                            log_error!(=>T, "link_process: spawn failed");
                        }
                    }
                    Err(_) => {
                        log_error!(=>T, "grant failed");
                    }
                }
            }
            Err(e) => {
                log_warn!(=>T, "tcp_socket: recv: {:?}", e);
            }
        }
    }
}

fn handle_tcp_tx(tcp_socket: &mut TcpSocket, eth_in_cons: &mut bbqueue::Consumer<512>, useful_work: &mut bool) {
    if tcp_socket.can_send() {
        match eth_in_cons.read() {
            Ok(rgr) => {
                match tcp_socket.send_slice(&rgr) {
                    Ok(written) => {
                        rgr.release(written); *useful_work = true;
                        // done_smth_useful = true;
                        // log_trace!("Written {} to tcp_socket", written);
                    }
                    Err(e) => {
                        log_warn!(=>T, "tcp_socket write err: {:?}", e);
                    }
                }
            }
            Err(_) => {}
        }
    }
}

pub fn smoltcp_poll_at(mut cx: crate::app::smoltcp_poll_at::Context) {
    let time = crate::app::monotonics::now().duration_since_epoch().to_micros();
    log_trace!("smoltcp_poll_at: {}us", time);

    cx.shared.poll_at_handle.lock(|h| *h = None);
    // if ETH is same or higher priority, it will run immediately, and schedule_at will fail,
    // since this task is not yet finished
    rtic::pend(stm32h7xx_hal::pac::Interrupt::ETH);
}