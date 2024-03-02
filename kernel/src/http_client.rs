use alloc::vec;
use alloc::string::String;
use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle};
use smoltcp::phy::{Device, Medium};
use smoltcp::socket::tcp;
use smoltcp::time:: Instant;
use smoltcp::wire::{EthernetAddress, IpCidr, IpAddress, Ipv4Address};

use crate::smoltcp_virtio::SmolTcpVirtio;
use crate::virtio::network::VirtioNetwork;


const BUF_SIZE: usize = 4096 * 1024;

pub struct HttpClient<'a> {
    device: SmolTcpVirtio,
    iface: Interface,
    sockets: SocketSet<'a>,
    client_handle: SocketHandle,
    state: State,
}

enum State {
    Sending,
    Receiving,
    Done,
}

impl<'a> HttpClient<'a> {

    pub fn new(virtio_dev: VirtioNetwork) -> Self {

        let ip_cidr = IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24);

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

        iface.routes_mut().add_default_ipv4_route(Ipv4Address([10, 0, 2, 2])).unwrap();

        let mut client_socket = {
            let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0u8; BUF_SIZE]);
            tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
        };

        let cx = iface.context();

        //93.184.216.34

        client_socket
            //.connect(cx, (IpAddress::v4(10, 0, 2, 2), 1235), 65000)
            //.connect(cx, (IpAddress::v4(93, 184, 216, 34), 80), 65000)
            .connect(cx, (IpAddress::v4(1, 1, 1, 1), 80), 65000)
            .unwrap();
    
        let sockets_storage: [_; 1] = Default::default();
        let mut sockets = SocketSet::new(sockets_storage);
        let client_handle = sockets.add(client_socket);
        
        HttpClient {
            device,
            iface,
            sockets,
            client_handle,
            state: State::Sending,
        }

    }

    pub fn update(&mut self) {

        let timestamp = read_time_ms();
        let elapsed = Instant::from_millis(timestamp as i64);

        self.iface.poll(elapsed, &mut self.device, &mut self.sockets);

        let socket = self.sockets.get_mut::<tcp::Socket>(self.client_handle);

        match self.state {
            State::Sending => {
                if socket.can_send() {
                    log::debug!("Sending data");
                    socket.send_slice(String::from("GET / HTTP/1.1\r\nAccept: text/html\r\n\r\n").as_bytes()).unwrap();
                    self.state = State::Receiving;
                }
            },
            State::Receiving => {
                if socket.can_recv() {
                    log::debug!(
                        "got {:?}",
                        socket.recv(|buffer| { (buffer.len(), core::str::from_utf8(buffer).unwrap()) })
                    );
                    socket.close();
                    self.state = State::Done;
                }
            },
            State::Done => {}
        }
    }

}

fn read_time_ms() -> u64 {
    let cycles_count = unsafe { core::arch::x86_64::_rdtsc()};
    cycles_count / 5_000_000
}
