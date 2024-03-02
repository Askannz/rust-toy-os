
extern crate alloc;
use alloc::sync::Arc;

use rustls::client::{ClientConnectionData, UnbufferedClientConnection};
use rustls::unbuffered::{
    AppDataRecord, ConnectionState,
    UnbufferedStatus, WriteTraffic,
};
use rustls::version::TLS12;
use rustls::{ClientConfig, RootCertStore};

pub use rustls::pki_types::UnixTime;
pub use rustls::time_provider::TimeProvider;

pub trait TcpSocket {
    fn write_all(&mut self, buf: &[u8]) -> anyhow::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize>;
}

pub fn https_get<T: TcpSocket>(
    time_provider: Arc<dyn TimeProvider>,
    sock: &mut T,
    host: &str
) -> anyhow::Result<()> {

    let root_store = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.into(),
    };

    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let config = ClientConfig::builder_with_details(provider, time_provider)
        .with_protocol_versions(&[&TLS12]).unwrap()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let config = Arc::new(config);

    let mut incoming_tls = [0; INCOMING_TLS_BUFSIZE];
    let mut outgoing_tls = vec![0; OUTGOING_TLS_INITIAL_BUFSIZE];

    converse(&config, sock, host, &mut incoming_tls, &mut outgoing_tls)?;

    Ok(())
}

fn converse<T: TcpSocket>(
    config: &Arc<ClientConfig>,
    sock: &mut T,
    host: &str,
    incoming_tls: &mut [u8],
    outgoing_tls: &mut Vec<u8>,
) -> Result<(), anyhow::Error> {

    let mut conn: UnbufferedClientConnection = UnbufferedClientConnection::new(
        Arc::clone(config),
        host.to_owned().try_into().map_err(anyhow::Error::msg)?
    ).map_err(anyhow::Error::msg)?;

    let mut incoming_used = 0;
    let mut outgoing_used = 0;

    let mut open_connection = true;
    let mut sent_request = false;
    let mut received_response = false;

    let mut iter_count = 0;
    while open_connection {
        let UnbufferedStatus { mut discard, state } =
            conn.process_tls_records(&mut incoming_tls[..incoming_used]);

        match dbg!(state.unwrap()) {
            ConnectionState::ReadTraffic(mut state) => {
                while let Some(res) = state.next_record() {
                    let AppDataRecord {
                        discard: new_discard,
                        payload,
                    } = res.map_err(anyhow::Error::msg)?;
                    discard += new_discard;

                    if payload.starts_with(b"HTTP") {
                        let response = core::str::from_utf8(payload).map_err(anyhow::Error::msg)?;
                        println!("{response}");
                        let header = response
                            .lines()
                            .next()
                            .unwrap_or(response);

                        println!("{header}");
                    } else {
                        println!("(.. continued HTTP response ..)");
                    }

                    received_response = true;
                }
            }

            ConnectionState::EncodeTlsData(mut state) => {
                try_write(
                    |out_buffer| state.encode(out_buffer).map_err(anyhow::Error::msg),
                    outgoing_tls,
                    &mut outgoing_used,
                )?;
            }

            ConnectionState::TransmitTlsData(mut state) => {

                if let Some(mut may_encrypt) = state.may_encrypt_app_data() {
                    encrypt_http_request(
                        host,
                        &mut sent_request,
                        &mut may_encrypt,
                        outgoing_tls,
                        &mut outgoing_used,
                    );
                }

                send_tls(sock, outgoing_tls, &mut outgoing_used)?;
                state.done();
            }

            ConnectionState::BlockedHandshake { .. } => {
                recv_tls(sock, incoming_tls, &mut incoming_used)?;
            }

            ConnectionState::WriteTraffic(mut may_encrypt) => {
                if encrypt_http_request(
                    host,
                    &mut sent_request,
                    &mut may_encrypt,
                    outgoing_tls,
                    &mut outgoing_used,
                ) {
                    send_tls(sock, outgoing_tls, &mut outgoing_used)?;
                    recv_tls(sock, incoming_tls, &mut incoming_used)?;
                } else if !received_response {
                    // this happens in the TLS 1.3 case. the app-data was sent in the preceding
                    // `TransmitTlsData` state. the server should have already written a
                    // response which we can read out from the socket
                    recv_tls(sock, incoming_tls, &mut incoming_used)?;
                } else {
                    try_write(
                        |out_buffer| may_encrypt.queue_close_notify(out_buffer).map_err(anyhow::Error::msg),
                        outgoing_tls,
                        &mut outgoing_used,
                    )?;
                    send_tls(sock, outgoing_tls, &mut outgoing_used)?;
                    open_connection = false;
                }
            }

            ConnectionState::Closed => {
                open_connection = false;
            }

            // other states are not expected in this example
            _ => unreachable!(),
        }

        if discard != 0 {
            assert!(discard <= incoming_used);

            incoming_tls.copy_within(discard..incoming_used, 0);
            incoming_used -= discard;

            eprintln!("discarded {discard}B from `incoming_tls`");
        }

        iter_count += 1;
        assert!(
            iter_count < MAX_ITERATIONS,
            "did not get a HTTP response within {MAX_ITERATIONS} iterations"
        );
    }

    assert!(sent_request);
    assert!(received_response);
    assert_eq!(0, incoming_used);
    assert_eq!(0, outgoing_used);

    Ok(())
}

fn try_write(
    mut f: impl FnMut(&mut [u8]) -> Result<usize, anyhow::Error>,
    outgoing_tls: &mut Vec<u8>,
    outgoing_used: &mut usize,
) -> Result<usize, anyhow::Error>
{
    let written = f(&mut outgoing_tls[*outgoing_used..])?;

    *outgoing_used += written;

    Ok(written)
}

fn recv_tls<T: TcpSocket>(
    sock: &mut T,
    incoming_tls: &mut [u8],
    incoming_used: &mut usize,
) -> Result<(), anyhow::Error> {
    let read = sock.read(&mut incoming_tls[*incoming_used..]).map_err(anyhow::Error::msg)?;
    eprintln!("received {read}B of data");
    *incoming_used += read;
    Ok(())
}

fn send_tls<T: TcpSocket>(
    sock: &mut T,
    outgoing_tls: &[u8],
    outgoing_used: &mut usize,
) -> Result<(), anyhow::Error> {
    sock.write_all(&outgoing_tls[..*outgoing_used]).map_err(anyhow::Error::msg)?;
    eprintln!("sent {outgoing_used}B of data");
    *outgoing_used = 0;
    Ok(())
}

fn encrypt_http_request(
    host: &str,
    sent_request: &mut bool,
    may_encrypt: &mut WriteTraffic<'_, ClientConnectionData>,
    outgoing_tls: &mut [u8],
    outgoing_used: &mut usize,
) -> bool {
    if !*sent_request {
        let request = format!(
            "GET / HTTP/1.1\r\n\
            Host: {host}\r\n\
            Connection: close\r\n\
            Accept-Encoding: identity\r\n\
            \r\n"
        ).into_bytes();
        let written = may_encrypt
            .encrypt(&request, &mut outgoing_tls[*outgoing_used..])
            .expect("encrypted request does not fit in `outgoing_tls`");
        *outgoing_used += written;
        *sent_request = true;
        eprintln!("queued HTTP request");
        true
    } else {
        false
    }
}

const KB: usize = 1024;
const INCOMING_TLS_BUFSIZE: usize = 1024 * KB;
const OUTGOING_TLS_INITIAL_BUFSIZE: usize = KB;

const MAX_ITERATIONS: usize = 20;
