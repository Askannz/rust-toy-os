use core::str;

use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::phy::{Device, Medium};
use smoltcp::socket::tcp;
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

use crate::serial_println;
//use crate::loopback::Loopback;
use crate::smoltcp_virtio::SmolTcpVirtio;
use crate::virtio::network::VirtioNetwork;

const LOCAL_IP: IpAddress = IpAddress::v4(10, 0, 0, 1);
const REMOTE_IP: IpAddress = IpAddress::v4(10, 0, 0, 2);

mod mock {
    use core::cell::Cell;
    use smoltcp::time::{Duration, Instant};

    #[derive(Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct Clock(Cell<Instant>);

    impl Clock {
        pub fn new() -> Clock {
            Clock(Cell::new(Instant::from_millis(0)))
        }

        pub fn advance(&self, duration: Duration) {
            self.0.set(self.0.get() + duration)
        }

        pub fn elapsed(&self) -> Instant {
            self.0.get()
        }
    }
}

pub fn test_smolltcp(virtio_dev: VirtioNetwork) {

    let clock = mock::Clock::new();
    let mut device = SmolTcpVirtio::new(virtio_dev);
    let mac_addr = device.virtio_dev.mac_addr;

    let config = match device.capabilities().medium {
        Medium::Ethernet => {
            Config::new(EthernetAddress(mac_addr).into())
        }
    };

    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(LOCAL_IP, 24))
            .unwrap();
    });

    let server_socket = {
        static mut TCP_SERVER_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_SERVER_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let client_socket = {
        static mut TCP_CLIENT_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_CLIENT_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let mut sockets: [_; 2] = Default::default();
    let mut sockets = SocketSet::new(&mut sockets[..]);
    let server_handle = sockets.add(server_socket);
    let client_handle = sockets.add(client_socket);

    let mut did_listen = false;
    let mut did_connect = false;
    let mut done = false;
    while !done {
        iface.poll(clock.elapsed(), &mut device, &mut sockets);

        let mut socket = sockets.get_mut::<tcp::Socket>(client_handle);
        let cx = iface.context();
        if !socket.is_open() {
            if !did_connect {
                serial_println!("connecting");
                socket
                    .connect(cx, (REMOTE_IP, 1234), 65000)
                    .unwrap();
                did_connect = true;
            }
        }

        if socket.can_send() {
            serial_println!("sending");
            socket.send_slice(&[0xBA; 16]).unwrap();
            socket.close();
        }

        match iface.poll_delay(clock.elapsed(), &sockets) {
            Some(Duration::ZERO) => serial_println!("resuming"),
            Some(delay) => {
                serial_println!("sleeping for {} ms", delay);
                clock.advance(delay)
            }
            None => clock.advance(Duration::from_millis(1)),
        }
    }
}
