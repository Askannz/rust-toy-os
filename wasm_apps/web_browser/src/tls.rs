use std::io::{self, Read, Write};
use std::sync::Arc;

//use mio::net::TcpStream;
use rustls::pki_types::ServerName;
use rustls::RootCertStore;

pub struct TlsClient<T> {
    socket: T,
    closing: bool,
    clean_closure: bool,
    tls_conn: rustls::ClientConnection,
}

impl<T: Read + Write> TlsClient<T> {
    pub fn new(
        sock: T,
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
            closing: false,
            clean_closure: false,
            tls_conn: rustls::ClientConnection::new(Arc::new(config), server_name).unwrap(),
        }
    }

    pub fn update(&mut self) -> usize {

        let mut b = 0;

        if self.is_closed() {
            //println!("TLS is closed");
            return b;
        }

        if self.tls_conn.wants_read() {
            //println!("TLS wants read");
            b = self.do_read();
        }

        if self.tls_conn.wants_write() {
            //println!("TLS wants write");
            self.do_write();
        }

        b
    }

    /// We're ready to do a read.
    fn do_read(&mut self) -> usize {
        // Read TLS data.  This fails if the underlying TCP connection
        // is broken.
        match self.tls_conn.read_tls(&mut self.socket) {
            Err(error) => {
                if error.kind() == io::ErrorKind::WouldBlock {
                    return 0;
                }
                println!("TLS read error: {:?}", error);
                self.closing = true;
                return 0;
            }

            // If we're ready but there's no data: EOF.
            Ok(0) => {
                //println!("Read EOF");
                //self.closing = true;
                //self.clean_closure = true;
                //return;
            }

            Ok(_) => {}
        };

        // Reading some TLS data might have yielded new TLS
        // messages to process.  Errors from this indicate
        // TLS protocol problems and are fatal.
        let io_state = match self.tls_conn.process_new_packets() {
            Ok(io_state) => io_state,
            Err(err) => {
                println!("TLS error: {:?}", err);
                self.closing = true;
                return 0;
            }
        };

        // Having read some TLS data, and processed any new messages,
        // we might have new plaintext as a result.
        //
        // Read it and then write it to stdout.
        // if io_state.plaintext_bytes_to_read() > 0 {
        //     let mut plaintext = vec![0u8; io_state.plaintext_bytes_to_read()];
        //     self.tls_conn
        //         .reader()
        //         .read_exact(&mut plaintext)
        //         .unwrap();
        //     io::stdout()
        //         .write_all(&plaintext)
        //         .unwrap();
        // }

        // If that fails, the peer might have started a clean TLS-level
        // session closure.
        if io_state.peer_has_closed() {
            self.clean_closure = true;
            self.closing = true;
        }

        io_state.plaintext_bytes_to_read()
    }

    fn do_write(&mut self) {
        match self.tls_conn.write_tls(&mut self.socket) {
            Err(error) => {
                if error.kind() == io::ErrorKind::WouldBlock {
                    return;
                }
                println!("TLS write error: {:?}", error);
                self.closing = true;
                return;
            }

            // If we're ready but there's no data: EOF.
            Ok(0) => {
                //println!("Write EOF");
                //self.closing = true;
                //self.clean_closure = true;
                return;
            }

            Ok(_) => {}
        };
    }

    pub fn is_closed(&self) -> bool {
        self.closing
    }
}
impl<T> io::Write for TlsClient<T> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.tls_conn.writer().write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tls_conn.writer().flush()
    }
}

impl<T> io::Read for TlsClient<T> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self.tls_conn.reader().read(bytes) {
            Err(error) => {
                if error.kind() == io::ErrorKind::UnexpectedEof {
                    //println!("Unexpected EOF");
                    return Ok(0)
                }
                Err(error)
            },

            Ok(n) => Ok(n)
        }
    }
}

