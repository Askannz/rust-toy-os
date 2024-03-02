use alloc::vec;
use alloc::string::String;
use anyhow::Ok;
use lazy_static::lazy_static;
use log::debug;
use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};
use smoltcp::phy::{Device, Medium};
use smoltcp::socket::tcp;
use smoltcp::time:: Instant;
use smoltcp::wire::{EthernetAddress, IpCidr, IpAddress, Ipv4Address};

use crate::time::SystemClock;
use crate::smoltcp_virtio::SmolTcpVirtio;
use crate::virtio::network::VirtioNetwork;

//use crate::tls::TcpSocket;

pub trait TcpSocket {
    fn write_all(&mut self, buf: &[u8]) -> anyhow::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize>;
}

const BUF_SIZE: usize = 4096 * 1024;

struct SingleSocketInterface<'a> {
    device: SmolTcpVirtio,
    iface: Interface,
    sockets: SocketSet<'a>,
    socket_handle: SocketHandle,
}

lazy_static! {
    static ref IFACE_ADDR: IpCidr = IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24);
    static ref GATEWAY_ADDR: Ipv4Address = Ipv4Address([10, 0, 2, 2]);
}

impl<'a> SingleSocketInterface<'a> {

    fn new(virtio_dev: VirtioNetwork, addr: Ipv4Address, port: u16) -> Self {

        let mut device = SmolTcpVirtio::new(virtio_dev);
        let mac_addr = device.virtio_dev.mac_addr;
    
        let config = match device.capabilities().medium {
            Medium::Ethernet => {
                Config::new(EthernetAddress(mac_addr).into())
            }
        };

        let timestamp = read_time_ms();
    
        let mut iface = Interface::new(config, &mut device, Instant::from_millis(timestamp as i64));
        iface.update_ip_addrs(|ip_addrs| {
            ip_addrs.push(*IFACE_ADDR).unwrap();
        });

        iface.routes_mut().add_default_ipv4_route(*GATEWAY_ADDR).unwrap();

        let mut socket = {
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
        };

        let cx = iface.context();

        socket
            .connect(cx, (addr, port), 65000)
            .unwrap();
    
        let sockets_storage: [_; 1] = Default::default();
        let mut sockets = SocketSet::new(sockets_storage);
        let socket_handle = sockets.add(socket);
        
        SingleSocketInterface {
            device,
            iface,
            sockets,
            socket_handle
        }
    }

    fn update_interface(&mut self) {
        let timestamp = read_time_ms();
        let elapsed = Instant::from_millis(timestamp as i64);
        loop {
            let updated = self.iface.poll(elapsed, &mut self.device, &mut self.sockets);
            if !updated { break; }
        }
    }
}

impl<'a> TcpSocket for SingleSocketInterface<'a> {

    fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {

        let recv_len = loop {

            self.update_interface();
    
            let socket = self.sockets.get_mut::<tcp::Socket>(self.socket_handle);

            if socket.can_recv() {

                log::debug!("socket.can_recv()");
                //loop {}

                let recv_len = socket.recv(|recv_buffer| {

                    log::debug!("socket.recv()");
                    loop {}

                    let src_len = recv_buffer.len();
                    let dst_len = buf.len();
                    let cpy_len = usize::min(src_len, dst_len);

                    log::debug!("{} {}", src_len, dst_len);
                    loop {}

                    buf.copy_from_slice(&recv_buffer[..cpy_len]);
                    (cpy_len, cpy_len)
                }).map_err(anyhow::Error::msg)?;

                log::debug!("Received {}B", recv_len);

                break recv_len;
            }

        };

        Ok(recv_len)
    }

    fn write_all(&mut self, buf: &[u8]) -> anyhow::Result<()> {

        loop {

            self.update_interface();

            let socket = self.sockets.get_mut::<tcp::Socket>(self.socket_handle);

            if socket.can_send() {
                let sent_len = socket.send_slice(buf).map_err(anyhow::Error::msg)?;
                self.update_interface();
                log::debug!("Sent {}B out of {}B", sent_len, buf.len());
                break;
            }
        }

        Ok(())
    }

}

pub fn test_http(virtio_dev: VirtioNetwork, clock: &SystemClock) {

    log::debug!("Creating TCP socket");
    let mut socket = SingleSocketInterface::new(virtio_dev, Ipv4Address([1, 1, 1, 1]), 80);

    log::debug!("Sending HTTP GET request");
    socket.write_all(String::from("GET / HTTP/1.1\r\nAccept: text/html\r\n\r\n").as_bytes()).unwrap();

    log::debug!("Waiting for HTTP response");
    let mut recv_buf = [0u8; 10_000_000];

    log::debug!("Allocated recv buffer");

    log::debug!("Delay");
    clock.spin_delay(60_000.0);
    log::debug!("End delay");
    //loop {}

    log::debug!("Reading response");

    let recv_len = socket.read(&mut recv_buf).unwrap();

    log::debug!("Got HTTP response");

    let s = core::str::from_utf8(&recv_buf[..recv_len]).unwrap();
    log::debug!("got {:?}", s);

}

fn read_time_ms() -> u64 {
    let cycles_count = unsafe { core::arch::x86_64::_rdtsc()};
    cycles_count / 5_000_000
}
