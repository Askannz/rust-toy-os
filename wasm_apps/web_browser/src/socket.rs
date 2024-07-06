use std::io::{Result, Read, Write};

pub struct Socket { handle_id: i32 }


impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(guestlib::tcp_read(buf, self.handle_id))
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(guestlib::tcp_write(buf, self.handle_id))
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Socket {

    pub fn new(ip_addr: [u8; 4], port: u16) -> Self {
        let handle_id = guestlib::tcp_connect(ip_addr, port);
        Socket { handle_id }
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
