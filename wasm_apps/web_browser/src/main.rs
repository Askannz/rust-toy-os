extern crate alloc;

use std::io::{Read, Write};

use std::fmt::Debug;
use std::{any, fmt};
use std::borrow::Cow;

use core::cell::OnceCell;
use alloc::format;
use alloc::collections::BTreeMap;
use anyhow::Context;
use guestlib::{FramebufferHandle, WasmLogger};
use applib::{Color, Rect};


use applib::ui::button::{Button, ButtonState};
use applib::ui::progress_bar::{ProgressBar, ProgressBarConfig};
use applib::ui::text::{EditableText, EditableTextConfig};

mod tls;
mod render;
mod dns;
mod socket;
mod html_parsing;

use render::Webview;
use tls::TlsClient;
use socket::Socket;

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState<'a> {
    fb_handle: FramebufferHandle,
    url_bar: EditableText,
    progress_bar: ProgressBar,

    webview: render::Webview<'a>,

    first_frame: bool,

    buffer: Vec<u8>,

    request_state: RequestState,
}

enum RequestState {
    Idle { domain: Option<String> },
    Dns { domain: String, path: String, dns_socket: Socket, dns_state: DnsState },
    Https { domain: String, path: String, tls_client: TlsClient, https_state: HttpsState },
    Render { domain: String, html: String },
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
    Receiving { in_count: usize },
}

impl Debug for RequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestState::Idle { .. } => write!(f, "Idle"),
            RequestState::Dns { domain, dns_state, .. } => write!(f, "DNS {} {:?}", domain, dns_state),
            RequestState::Https { https_state, .. } => write!(f, "HTTPS {:?}", https_state),
            RequestState::Render { .. } => write!(f, "Render"),
        }
    }
}

fn get_progress_repr(request_state: &RequestState) -> (u64, Cow<str>) {
    match request_state {
        RequestState::Dns { dns_state, .. } => match dns_state {
            DnsState::Connecting => (0, Cow::Borrowed("DNS: connecting")),
            DnsState::Sending { out_count } => (1, Cow::Owned(format!("DNS: sent {} bytes", out_count))),
            DnsState::ReceivingLen { .. } => (2, Cow::Borrowed("DNS: receiving response length")),
            DnsState::ReceivingResp { in_count } => (3, Cow::Owned(format!("DNS: received {} bytes", in_count))),
        },
        RequestState::Https { https_state, .. } => match https_state {
            HttpsState::Connecting => (4, Cow::Borrowed("HTTPS: connecting")),
            HttpsState::Sending { out_count } => (5, Cow::Owned(format!("HTTPS: sent {} bytes", out_count))),
            HttpsState::Receiving { in_count } => (6, Cow::Owned(format!("HTTPS: received {} bytes", in_count))),
        },
        RequestState::Render { .. } => (7, Cow::Borrowed("Rendering")),
        RequestState::Idle { .. } => (8, Cow::Borrowed("")),
    }
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const SCHEME: &str = "https://";
const DNS_SERVER_IP: [u8; 4] = [1, 1, 1, 1];
const BUFFER_SIZE: usize = 100_000;

const button_w: u32 = 100;
const bar_h: u32 = 25;


fn main() {}

#[no_mangle]
pub fn init() -> () {

    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let win_rect = guestlib::get_win_rect();
    let fb_handle = guestlib::create_framebuffer(win_rect.w, win_rect.h);

    let Rect { w: win_w, h: win_h, .. } = win_rect;

    let rect_url_bar = Rect { x0: 0, y0: 0, w: win_w - button_w, h: bar_h };
    let rect_progress_bar = Rect { x0: 0, y0: bar_h.into(), w: win_w, h: bar_h };
    let rect_webview = Rect  { x0: 0, y0: (2 * bar_h).into(), w: win_w, h: win_h - 2 * bar_h};

    let state = AppState { 
        fb_handle,
        url_bar: EditableText::new(&EditableTextConfig {
            rect: rect_url_bar,
            color: Color::WHITE,
            bg_color: Some(Color::rgb(128, 128, 128)),
            ..Default::default()
        }),
        progress_bar: ProgressBar::new(&ProgressBarConfig {
            rect: rect_progress_bar,
            max_val: 8,
            bg_color: Color::rgb(128, 128, 128),
            bar_color: Color::rgb(128, 128, 255),
            text_color: Color::WHITE,
            ..Default::default()
        }),

        webview: Webview::new(&rect_webview),

        first_frame: true,
        buffer: vec![0u8; BUFFER_SIZE],
        request_state: RequestState::Idle { domain: None },
    };
    unsafe { APP_STATE.set(state).unwrap_or_else(|_| panic!("App already initialized")); }
}

#[no_mangle]
pub fn step() {

    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let system_state = guestlib::get_system_state();
    let win_rect = guestlib::get_win_rect();
    let win_input_state = system_state.input.change_origin(&win_rect);
    
    let is_button_fired = {

        // TODO: this should only be needed once...
        let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);
        framebuffer.fill(Color::WHITE);

        let rect_button = Rect { x0: (win_rect.w - button_w).into(), y0: 0, w: button_w, h: bar_h };
        let button_state = ButtonState {
            rect: rect_button,
            text: "GO".into(),
            ..Default::default()
        };
        Button::update(&mut framebuffer, &win_input_state, &button_state)
    };

    let url_override = match state.first_frame {
        true => Some("https://news.ycombinator.com".to_owned()),
        //true => Some("https://en.wikipedia.org/wiki/Rust".to_owned()),
        false => match &state.request_state {
            RequestState::Dns { domain, path, .. } => Some(format!("{}{}{}", SCHEME, domain, path)),
            _ => None,
        }
    };

    let html_update = match &state.request_state {
        RequestState::Render { html, .. } => Some(html.as_str()),
        _ => None
    };

    let (prog_val, prog_str) = get_progress_repr(&state.request_state);
    
    let redraw_progress_bar = state.progress_bar.update(prog_val, prog_str.as_ref());
    let redraw_url_bar = state.url_bar.update(&win_input_state, url_override.as_deref());
    let redraw_view = state.webview.update(&win_input_state, html_update);

    let prev_state_debug = format!("{:?}", state.request_state);
    try_update_request_state(state, is_button_fired);
    let new_state_debug = format!("{:?}", state.request_state);

    if new_state_debug != prev_state_debug {
        log::info!("Request state change: {} => {}", prev_state_debug, new_state_debug);
    }

    //let redraw = redraw_progress_bar || redraw_button || redraw_url_bar || redraw_view || state.first_frame;

    //if !redraw { return; }

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);

    state.progress_bar.draw(&mut framebuffer);
    state.webview.draw(&mut framebuffer);
    //state.button.draw(&mut framebuffer);
    state.url_bar.draw(&mut framebuffer);
    state.first_frame = false;
}

fn try_update_request_state(state: &mut AppState, is_button_fired: bool) {

    match update_request_state(state, is_button_fired) {
        Ok(_) => (),
        Err(err) => {
            log::error!("{}", err);
            state.request_state = RequestState::Render { 
                domain: "ERROR".to_owned(),
                html: make_error_html(err),
            }
        }
    }

}

fn make_error_html(error: anyhow::Error) -> String {

    let errors: Vec<String> = error.chain().enumerate()
    .map(|(i, sub_err)| format!(
        "<p>{}: {}</p>", i, sub_err
    ))
    .collect();

    format!(
        "<html>\n\
        <p bgcolor=\"#ff0000\">ERROR</p>\n\
        {}
        </html>\n",
        errors.join("\n")
    )
}

fn update_request_state(state: &mut AppState, is_button_fired: bool) -> anyhow::Result<()>{
    
    match &mut state.request_state {
    
        RequestState::Idle { domain: current_domain } => {

            let mut url_data: Option<(String, String)> = None;

            if is_button_fired || state.url_bar.is_flushed() {
                let (domain, path) = parse_url(state.url_bar.text());
                url_data = Some((domain.to_string(), path.to_string()));
            } else {
                if let Some(href) = state.webview.check_redirect() {
                    if let Some(current_domain) = current_domain {
                        if !href.starts_with(SCHEME) {
                            let path = format!("/{}", href);
                            url_data = Some((current_domain.clone(), path));
                        }
                    }
                }
            }

            if let Some((domain, path)) = url_data {
                let dns_socket = Socket::new(DNS_SERVER_IP, 53)?;
                state.request_state = RequestState::Dns { 
                    domain,
                    path,
                    dns_socket,
                    dns_state: DnsState::Connecting
                };
            }
        },

        RequestState::Dns { domain, path, dns_state, dns_socket } => match dns_state {

            DnsState::Connecting => {
                let socket_ready = dns_socket.may_send() && dns_socket.may_recv();
                if socket_ready {
                    let tcp_bytes = dns::make_tcp_dns_request(domain);
                    state.buffer.clear();
                    state.buffer.write(&tcp_bytes)?;
                    *dns_state = DnsState::Sending { out_count: 0 };
                }
            },

            DnsState::Sending { out_count } => {
                let n = dns_socket.write(&state.buffer[*out_count..]).context("Could not write to DNS socket")?;
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.resize(2, 0u8);
                    *dns_state = DnsState::ReceivingLen { in_count: 0 };
                }
            }

            DnsState::ReceivingLen { in_count } => {
                let n = dns_socket.read(&mut state.buffer[*in_count..]).context("Could not read from DNS socket")?;
                *in_count += n;

                if *in_count >= 2 {
                    let len_bytes: [u8; 2] = state.buffer.as_slice().try_into()?;
                    let dns_len: usize = u16::from_be_bytes(len_bytes).try_into().context("Invalid DNS response data")?;

                    state.buffer.resize(dns_len, 0u8);

                    *dns_state = DnsState::ReceivingResp { in_count: 0 };
                }
            }

            DnsState::ReceivingResp { in_count } => {
                let n = dns_socket.read(&mut state.buffer[*in_count..]).context("Could not read from DNS socket")?;
                *in_count += n;

                if *in_count >= state.buffer.len() {
                    let ip_addr = dns::parse_tcp_dns_response(&state.buffer)?;

                    dns_socket.close();

                    let https_socket = Socket::new(ip_addr, 443)?;
                    state.request_state = RequestState::Https { 
                        domain: domain.clone(),
                        path: path.clone(),
                        tls_client: TlsClient::new(https_socket, domain),
                        https_state: HttpsState::Connecting,
                    }
                }
            }
        },

        RequestState::Https { domain, path, tls_client, https_state } => match https_state {

            HttpsState::Connecting => {
                if tls_client.socket_ready() {
                    state.buffer.clear();
                    write!(
                        &mut state.buffer,
                        "GET {} HTTP/1.1\r\n\
                        Host: {}\r\n\
                        Connection: close\r\n\
                        Accept-Encoding: identity\r\n\
                        \r\n",
                        path,
                        domain
                    )?;
                    *https_state = HttpsState::Sending { out_count: 0 };
                }
            },

            HttpsState::Sending { out_count } => {

                tls_client.update();
                let n = tls_client.write(&state.buffer[*out_count..])?;
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.clear();
                    *https_state = HttpsState::Receiving { in_count: 0 };
                }
            },

            HttpsState::Receiving { in_count } => {

                let n_plaintext = tls_client.update();
                *in_count += n_plaintext;

                if n_plaintext > 0 {

                    let len = state.buffer.len();
                    state.buffer.resize(len+n_plaintext, 0u8);
                    tls_client.read_exact(&mut state.buffer[len..len+n_plaintext])?;

                } else if tls_client.tls_closed() {

                    let http_string = core::str::from_utf8(&state.buffer)?;
                    //guestlib::qemu_dump(http_string.as_bytes());
                    let (_header, body) = parse_http(http_string)?;
                    //guestlib::qemu_dump(body.as_bytes());
                    state.request_state = RequestState::Render { domain: domain.clone(), html: body };
                }
            }

        },

        RequestState::Render {domain, .. } => { 
            state.request_state = RequestState::Idle { 
                domain: Some(domain.clone())
            };
        },
    };

    Ok(())
}

fn parse_http(http_response: &str) -> anyhow::Result<(HttpHeader, String)> {

    let (header, body) = parse_header(http_response)?;
    let transfer_encoding = header.get("transfer-encoding");

    let body = match transfer_encoding.map(|s| s.as_str()) {
        Some("chunked") => {
            log::info!("De-chunking response body");
            dechunk_body(body)
        },
        _ => body.to_owned()
    };

    Ok((header, body))

}

fn dechunk_body(body: &str) -> String {

    #[derive(Debug, PartialEq, Clone, Copy)]
    enum ParsingState {
        ReadingChunkSize,
        ReadingChunk,
    }

    let mut state = ParsingState::ReadingChunkSize;

    let mut chunks = Vec::new();

    for line in body.split("\r\n") {
        state = match state {
            ParsingState::ReadingChunkSize => ParsingState::ReadingChunk,
            ParsingState::ReadingChunk => {
                chunks.push(line);
                ParsingState::ReadingChunkSize
            },
        };
    }

    chunks.join("")
}

type HttpHeader = BTreeMap<String, String>;

fn parse_header(http_response: &str) ->  anyhow::Result<(HttpHeader, &str)>{

    let i = http_response.find("\r\n\r\n").ok_or(anyhow::anyhow!("Could not locate header"))?;

    let (header_str, body) = http_response.split_at(i);

    log::debug!("HTTP response header:\n{}", header_str);

    let header = header_str.split("\r\n").filter_map(|line| {
        let (key, val) = line.split_once(":")?;
        let val = val.trim().to_owned();
        let key = key.to_lowercase();
        Some((key, val))
    })
    .collect();

    Ok((header, body))
}

fn parse_url(url: &str) -> (&str, &str) {

    if !url.starts_with(SCHEME) {
        panic!("Invalid URL (no https://)");
    }

    let (_scheme, s) = url.split_at(SCHEME.len());

   let (domain, path) = match s.find("/") {
        Some(i) => s.split_at(i),
        None => (s, "/")
    };

    (domain, path)
}
