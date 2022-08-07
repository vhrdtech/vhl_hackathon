use core::sync::atomic::Ordering;
use smoltcp::iface::{
    Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes,
    SocketStorage,
    SocketHandle,
};
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv6Cidr};
use smoltcp::socket::{TcpSocket, TcpSocketBuffer};
use rtt_target::rprintln;
use stm32h7xx_hal::{ethernet as ethernet_h7, stm32};
use stm32h7xx_hal::ethernet::PinsRMII;
use stm32h7xx_hal::rcc::{CoreClocks, rec};

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

    /// Polls on the ethernet interface. You should refer to the smoltcp
    /// documentation for poll() to understand how to call poll efficiently
    pub fn poll(&mut self, now: i64) -> bool {
        let timestamp = Instant::from_millis(now);

        self.iface
            .poll(timestamp)
            .unwrap_or_else(|e|  {
                if e != smoltcp::Error::Unrecognized {
                    rprintln!("Poll err: {:?}", e)
                }
                false
            })
    }
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
    rprintln!("PHY init...");
    let mut lan8742a = ethernet_h7::phy::LAN8742A::new(eth_mac);
    use stm32h7xx_hal::ethernet::PHY;
    lan8742a.phy_reset();
    lan8742a.phy_init();
    rprintln!("PHY init done.");
    // The eth_dma should not be used until the PHY reports the link is up

    unsafe { ethernet_h7::enable_interrupt() };

    // unsafe: mutable reference to static storage, we only do this once
    let store = unsafe { &mut STORE };
    let net = Net::new(store, eth_dma, mac_addr.into());
    (net, lan8742a)
}

pub fn ethernet_event(ctx: crate::app::ethernet_event::Context) {
    unsafe { ethernet_h7::interrupt_handler() }
    ctx.local.led_act.toggle();
    let time = crate::TIME.load(Ordering::Relaxed);

    for i in 0..10 {
        let might_be_new_data = ctx.local.net.poll(time as i64);
        if !might_be_new_data {
            break;
        }
        rprintln!("ethernet_event it: {}", i);
        let tcp_socket: &mut TcpSocket = ctx.local.net.iface.get_socket(
            ctx.local.net.tcp_handle
        );
        rprintln!("{:?}", tcp_socket.state());

        if tcp_socket.state() == smoltcp::socket::TcpState::CloseWait {
            tcp_socket.close();
        }
        if !tcp_socket.is_open() {
            let r = tcp_socket.listen(7777);
            rprintln!("tcp_socket: listen(): {:?}", r);
        }

        if tcp_socket.can_recv() {
            match tcp_socket.recv(|buffer| {
                // dequeue the amount returned
                (buffer.len(), buffer)
            }) {
                Ok(buf) => {
                    rprintln!("tcp_socket: recv: {} {:02x?}", buf.len(), buf);
                    //link_process(buf);
                }
                Err(e) => {
                    rprintln!("tcp_socket: recv: {:?}", e);
                }
            }
        }
    }
}