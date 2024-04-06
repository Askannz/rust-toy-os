extern crate alloc;

use std::io::{Result, Read, Write};

use std::fmt::Debug;
use std::fmt;

use core::cell::OnceCell;
use alloc::format;
use guestlib::{FramebufferHandle};
use applib::{Color, Rect};


use applib::ui::button::{Button, ButtonConfig};
use applib::ui::text::{EditableText, EditableTextConfig};


mod tls;
mod render;
mod dns;

use render::Webview;
use tls::TlsClient;

struct AppState<'a> {
    fb_handle: FramebufferHandle,
    button: Button,
    url_bar: EditableText,

    webview: render::Webview<'a>,

    first_frame: bool,

    buffer: Vec<u8>,

    request_state: RequestState,

}

enum RequestState {
    Idle,
    Dns { domain: String, dns_state: DnsState },
    Https { domain: String, tls_client: TlsClient<Socket>, https_state: HttpsState },

}

#[derive(Debug, Clone, PartialEq)]
enum DnsState {
    Connecting,
    Sending { out_count: usize },
    ReceivingLen { in_count: usize },
    ReceivingResp { in_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
enum HttpsState {
    Connecting,
    Sending { out_count: usize },
    Receiving,
}

impl Debug for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestState::Idle => write!(f, "Idle"),
            RequestState::Dns { dns_state, .. } => write!(f, "DNS {:?}", dns_state),
            RequestState::Https { https_state, .. } => write!(f, "HTTPS {:?}", https_state),
        }
    }
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const DNS_SERVER_IP: [u8; 4] = [1, 1, 1, 1];

struct Socket;


impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Ok(guestlib::tcp_read(buf))
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        Ok(guestlib::tcp_write(buf))
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}


fn main() {}

#[no_mangle]
pub fn init() -> () {

    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);

    let Rect { w: win_w, h: win_h, .. } = win_rect;

    let button_w = 100;
    let bar_h = 25;

    let rect_button = Rect { x0: (win_w - button_w).into(), y0: 0, w: button_w, h: bar_h };
    let rect_url_bar = Rect { x0: 0, y0: 0, w: win_w - button_w, h: bar_h };
    let rect_webview = Rect  { x0: 0, y0: rect_button.h.into(), w: win_w, h: win_h };

    let state = AppState { 
        fb_handle,
        button: Button::new(&ButtonConfig {
            rect: rect_button,
            text: "GO".into(),
            ..Default::default()
        }),
        url_bar: EditableText::new(&EditableTextConfig {
            rect: rect_url_bar,
            color: Color::WHITE,
            bg_color: Some(Color::rgb(128, 128, 128)),
            ..Default::default()
        }),

        webview: Webview::new(&rect_webview),

        first_frame: true,
        buffer: vec![0u8; 100_000],
        request_state: RequestState::Idle,
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let win_rect = guestlib::get_win_rect();

    let win_input_state = system_state.input.change_origin(&win_rect);

    let redraw_button = state.button.update(&win_input_state);
    let redraw_url_bar = state.url_bar.update(&win_input_state);

    let mut html_update = None;
    let prev_state_debug = format!("{:?}", state.request_state);

    match &mut state.request_state {
    
        RequestState::Idle => {
            if state.button.is_fired() || state.url_bar.is_flushed() {
                guestlib::tcp_connect(DNS_SERVER_IP, 53);
                let domain = state.url_bar.text().to_string();
                state.request_state = RequestState::Dns { domain, dns_state: DnsState::Connecting } ;
            }
        },

        RequestState::Dns { domain, dns_state } => match dns_state {

            DnsState::Connecting => {
                let socket_ready = guestlib::tcp_may_send() && guestlib::tcp_may_recv();
                if socket_ready {
                    let tcp_bytes = dns::make_tcp_dns_request(domain);
                    state.buffer.resize(tcp_bytes.len(), 0u8);
                    state.buffer.copy_from_slice(&tcp_bytes);
                    *dns_state = DnsState::Sending { out_count: 0 };
                }
            },

            DnsState::Sending { out_count } => {
                let n = guestlib::tcp_write(&state.buffer[*out_count..]);
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.resize(2, 0u8);
                    *dns_state = DnsState::ReceivingLen { in_count: 0 };
                }
            }

            DnsState::ReceivingLen { in_count } => {
                let n = guestlib::tcp_read(&mut state.buffer[*in_count..]);
                *in_count += n;

                if *in_count >= 2 {
                    let len_bytes: [u8; 2] = state.buffer.as_slice().try_into().unwrap();
                    let dns_len: usize = u16::from_be_bytes(len_bytes).try_into().expect("Invalid DNS response data");

                    state.buffer.resize(dns_len, 0u8);

                    *dns_state = DnsState::ReceivingResp { in_count: 0 };
                }
            }

            DnsState::ReceivingResp { in_count } => {
                let n = guestlib::tcp_read(&mut state.buffer[*in_count..]);
                *in_count += n;

                if *in_count >= state.buffer.len() {
                    let ip_addr = dns::parse_tcp_dns_response(&state.buffer);
                    guestlib::tcp_connect(ip_addr, 443);
                    state.request_state = RequestState::Https { 
                        domain: domain.clone(),
                        tls_client: TlsClient::new(Socket, domain),
                        https_state: HttpsState::Connecting,
                    }
                }
            }
        },

        RequestState::Https { domain, tls_client, https_state } => match https_state {

            HttpsState::Connecting => {
                let socket_ready = guestlib::tcp_may_send() && guestlib::tcp_may_recv();
                if socket_ready {

                    let http_string = format!(
                        "GET / HTTP/1.1\r\n\
                        Host: {}\r\n\
                        Connection: close\r\n\
                        Accept-Encoding: identity\r\n\
                        \r\n",
                        domain
                    );
                    let http_bytes = http_string.as_bytes();

                    state.buffer.resize(http_bytes.len(), 0u8);
                    state.buffer.copy_from_slice(&http_bytes);
                    *https_state = HttpsState::Sending { out_count: 0 };
                }
            },

            HttpsState::Sending { out_count } => {

                tls_client.update();
                let n = tls_client.write(&state.buffer[*out_count..]).unwrap();
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.resize(0, 0u8);
                    *https_state = HttpsState::Receiving;
                }
            },

            HttpsState::Receiving => {

                let n_plaintext = tls_client.update();

                if n_plaintext > 0 {

                    let len = state.buffer.len();
                    state.buffer.resize(len+n_plaintext, 0u8);
                    tls_client.read_exact(&mut state.buffer[len..len+n_plaintext]).unwrap();


                } else if n_plaintext == 0 && !guestlib::tcp_may_recv() {

                    let http_string = core::str::from_utf8(&state.buffer).expect("Not UTF-8");
                    let html_string = get_html_string(http_string);
                    html_update = Some(html_string);
                    state.request_state = RequestState::Idle;
                }
            }

        },
    };

    let new_state_debug = format!("{:?}", state.request_state);

    if new_state_debug != prev_state_debug {
        println!("Request state change: {} => {}", prev_state_debug, new_state_debug);
    }

    let redraw_view = state.webview.update(&system_state.input, html_update.as_deref());

    let redraw = redraw_button || redraw_url_bar || redraw_view || state.first_frame;

    if !redraw { return; }

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);
    framebuffer.fill(Color::WHITE);

    state.webview.draw(&mut framebuffer);
    state.button.draw(&mut framebuffer);
    state.url_bar.draw(&mut framebuffer);
    state.first_frame = false;
}

fn get_html_string(http_string: &str) -> String {

    let s = http_string;

    let i1 = s.find("<html").expect("No <html> tag");
    let (_, s) = s.split_at(i1);

    let i2 = s.find("</html>").expect("No </html> tag");
    let (s, _) = s.split_at(i2);

    s.to_string()
}
