use std::io;
use std::sync::Arc;

use rustls::pki_types::ServerName;
use rustls::RootCertStore;

use crate::socket::Socket;

pub struct TlsClient {
    socket: Socket,
    closed: bool,
    tls_conn: rustls::ClientConnection,
}

impl TlsClient {
    pub fn new(
        sock: Socket,
        server_name: &str,
    ) -> Self {

        let server_name = ServerName::try_from(server_name)
            .expect("Invalid server name")
            .to_owned();

        let root_store = RootCertStore::from_iter(
            webpki_roots::TLS_SERVER_ROOTS
                .iter()
                .cloned(),
        );

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Self {
            socket: sock,
            closed: false,
            tls_conn: rustls::ClientConnection::new(Arc::new(config), server_name).unwrap(),
        }
    }

    pub fn update(&mut self) -> usize {

        if self.closed {
            return 0;
        }

        let mut n_plaintext = 0;

        if self.tls_conn.wants_read() {
            n_plaintext = self.do_read();
        }

        if self.tls_conn.wants_write() {
            self.do_write();
        }

        n_plaintext
    }

    fn do_read(&mut self) -> usize {

        match self.tls_conn.read_tls(&mut self.socket) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return 0,
            Err(error) => {
                log::error!("TLS read error: {:?}", error);
                self.closed = true;
                return 0;
            },
            Ok(_) => {},
        };

        let io_state = match self.tls_conn.process_new_packets() {
            Ok(io_state) => io_state,
            Err(err) => {
                log::error!("TLS error: {:?}", err);
                self.closed = true;
                return 0;
            }
        };

        if io_state.peer_has_closed() {
            self.closed = true;
        }

        io_state.plaintext_bytes_to_read()
    }

    fn do_write(&mut self) {
        match self.tls_conn.write_tls(&mut self.socket) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return,
            Err(error) => {
                log::error!("TLS write error: {:?}", error);
                self.closed = true;
                return;
            },
            Ok(_) => {},
        };
    }

    pub fn socket_ready(&self) -> bool {
        self.socket.may_send() && self.socket.may_recv()
    }

    pub fn tls_closed(&self) -> bool {
        self.closed
    }

}
impl io::Write for TlsClient {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.tls_conn.writer().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tls_conn.writer().flush()
    }
}

impl io::Read for TlsClient {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self.tls_conn.reader().read(bytes) {
            //Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(0),
            Err(error) => Err(error),
            Ok(n) => Ok(n),
        }
    }
}

