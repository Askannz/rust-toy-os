mod device;

use alloc::vec;

use crate::time::SystemClock;
use crate::virtio::network::VirtioNetwork;

use device::SmolTcpVirtio;
use lazy_static::lazy_static;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, Medium};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

lazy_static! {
    static ref IFACE_ADDR: IpCidr = IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24);
    static ref GATEWAY_ADDR: Ipv4Address = Ipv4Address([10, 0, 2, 2]);
}

const BUF_SIZE: usize = 4096;

pub struct TcpStack {
    device: SmolTcpVirtio,
    interface: Interface,
    sockets: SocketSet<'static>,
    next_port: u16,
}

impl TcpStack {
    pub fn new<'a>(clock: &SystemClock, virtio_dev: VirtioNetwork) -> Self {
        let mut device = SmolTcpVirtio::new(virtio_dev);
        let mac_addr = device.virtio_dev.mac_addr;

        let config = match device.capabilities().medium {
            Medium::Ethernet => Config::new(EthernetAddress(mac_addr).into()),
        };

        let timestamp = clock.time();

        let mut interface =
            Interface::new(config, &mut device, Instant::from_millis(timestamp as i64));
        interface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(*IFACE_ADDR).unwrap();
        });

        interface
            .routes_mut()
            .add_default_ipv4_route(*GATEWAY_ADDR)
            .unwrap();

        let sockets_storage: [_; 1] = Default::default();
        let sockets = SocketSet::new(sockets_storage);

        TcpStack {
            device,
            interface,
            sockets,
            next_port: 65000,
        }
    }

    pub fn connect(&mut self, addr: Ipv4Address, port: u16) -> anyhow::Result<SocketHandle> {
        let mut socket = {
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
        };

        let cx = self.interface.context();

        socket
            .connect(cx, (addr, port), self.next_port)
            .map_err(anyhow::Error::msg)?;
        self.next_port += 1;

        let socket_handle = self.sockets.add(socket);

        log::debug!("Connected to port {} ({:?})", port, socket_handle);

        Ok(socket_handle)
    }

    pub fn get_socket_state(&self, handle: SocketHandle) -> tcp::State {
        self.sockets.get::<tcp::Socket>(handle).state()
    }

    pub fn may_send(&self, handle: SocketHandle) -> bool {
        self.sockets.get::<tcp::Socket>(handle).may_send()
    }

    pub fn may_recv(&self, handle: SocketHandle) -> bool {
        self.sockets.get::<tcp::Socket>(handle).may_recv()
    }

    pub fn write(&mut self, handle: SocketHandle, buf: &[u8]) -> anyhow::Result<usize> {
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        log::debug!("Writing {}B to socket {:?}", buf.len(), handle);
        let sent_len = socket.send_slice(buf).map_err(anyhow::Error::msg)?;
        log::debug!("{}B sent", sent_len);
        Ok(sent_len)
    }

    pub fn read(&mut self, handle: SocketHandle, buf: &mut [u8]) -> anyhow::Result<usize> {
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);

        let recv_len = socket
            .recv(|recv_buffer| {
                let src_len = recv_buffer.len();
                let dst_len = buf.len();
                let cpy_len = usize::min(src_len, dst_len);

                buf[..cpy_len].copy_from_slice(&recv_buffer[..cpy_len]);
                (cpy_len, cpy_len)
            })
            .map_err(anyhow::Error::msg)?;

        log::debug!("Received {}B from socket {:?}", recv_len, handle);

        Ok(recv_len)
    }

    pub fn close(&mut self, handle: SocketHandle) {
        log::debug!("Closing socket {:?}", handle);
        let socket = self.sockets.get_mut::<tcp::Socket>(handle);
        socket.close();
        self.sockets.remove(handle);
    }

    pub fn poll_interface(&mut self, clock: &SystemClock) {
        let timestamp = clock.time();
        let elapsed = Instant::from_millis(timestamp as i64);
        self.interface
            .poll(elapsed, &mut self.device, &mut self.sockets);
    }

    pub fn pop_counters(&mut self) -> (usize, usize) {
        self.device.virtio_dev.get_counters()
    }
}
