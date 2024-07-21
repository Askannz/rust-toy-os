use std::io;
pub struct Socket { handle_id: i32 }


impl io::Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        guestlib::tcp_read(buf, self.handle_id)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
    }
}

impl io::Write for Socket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        guestlib::tcp_write(buf, self.handle_id)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Socket {

    pub fn new(ip_addr: [u8; 4], port: u16) -> anyhow::Result<Self> {
        let handle_id = guestlib::tcp_connect(ip_addr, port)?;
        Ok(Socket { handle_id })
    }

    pub fn may_recv(&self) -> bool {
        guestlib::tcp_may_recv(self.handle_id)
    }

    pub fn may_send(&self) -> bool {
        guestlib::tcp_may_send(self.handle_id)
    }
    
    pub fn close(&mut self) {
        guestlib::tcp_close(self.handle_id)
    }
}
