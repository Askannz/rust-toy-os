/*
    Loosely adapted from
    https://github.com/smoltcp-rs/smoltcp/blob/533f103a9544fa0de7d75383b13fc021f7b0642b/examples/loopback.rs
*/

use alloc::vec;
use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};
use smoltcp::phy::{Device, Medium};
use smoltcp::socket::tcp;
use smoltcp::time:: Instant;
use smoltcp::wire::{EthernetAddress, IpCidr};

use crate::serial_println;
use crate::smoltcp_virtio::SmolTcpVirtio;
use crate::virtio::network::VirtioNetwork;

const BUF_SIZE: usize = 4096;
const HTML_DATA: &'static [u8] = include_bytes!("../../html_data.txt");

pub struct HttpServer<'a> {
    device: SmolTcpVirtio,
    iface: Interface,
    port: u16,
    sockets: SocketSet<'a>,
    server_handle: SocketHandle,
}

impl<'a> HttpServer<'a> {

    pub fn new(virtio_dev: VirtioNetwork, ip_cidr: IpCidr, port: u16) -> Self {

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
            ip_addrs.push(ip_cidr).unwrap();
        });

        let server_socket = {
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
        };
    
        let sockets_storage: [_; 1] = Default::default();
        let mut sockets = SocketSet::new(sockets_storage);
        let server_handle = sockets.add(server_socket);
        
        HttpServer {
            device,
            iface,
            port,
            sockets,
            server_handle,
        }

    }

    pub fn update(&mut self) {

        let timestamp = read_time_ms();
        let elapsed = Instant::from_millis(timestamp as i64);

        self.iface.poll(elapsed, &mut self.device, &mut self.sockets);

        let socket = self.sockets.get_mut::<tcp::Socket>(self.server_handle);

        if !socket.is_active() && !socket.is_listening() {
            socket.listen(self.port).unwrap();
            serial_println!("Listening is enabled");
        }

        if socket.can_send() {
            serial_println!("Sending data");
            socket.send_slice(HTML_DATA).unwrap();
            serial_println!("Closing socket");
            socket.close();
        }
    }

}

fn read_time_ms() -> u64 {
    let cycles_count = unsafe { core::arch::x86_64::_rdtsc()};
    cycles_count / 5_000_000
}
