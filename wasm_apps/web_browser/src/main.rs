extern crate alloc;

use std::io::{Read, Write};

use std::borrow::Cow;
use std::fmt;
use std::fmt::Debug;

use alloc::collections::BTreeMap;
use alloc::format;
use anyhow::Context;
use applib::{Color, Rect};
use core::cell::OnceCell;
use guestlib::{PixelData, WasmLogger};

use applib::content::TrackedContent;
use applib::input::Keycode;
use applib::input::{InputEvent, InputState};
use applib::uitk::{self, UuidProvider};
use applib::FbViewMut;

mod dns;
mod html;
mod socket;
mod tls;

use html::canvas::html_canvas;
use html::{
    layout::{compute_layout, LayoutNode},
    parsing::parse_html,
};
use socket::Socket;
use tls::TlsClient;

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

struct AppState {
    pixel_data: PixelData,

    url_text: TrackedContent<String>,
    url_cursor: usize,

    buffer: Vec<u8>,

    uuid_provider: UuidProvider,
    ui_store: uitk::UiStore,

    webview_scroll_offsets: (i64, i64),
    webview_scroll_dragging: (bool, bool),

    request_state: RequestState,
}

struct UiLayout {
    url_bar_rect: Rect,
    button_rect: Rect,
    progress_bar_rect: Rect,
    canvas_rect: Rect,
}

enum RequestState {
    Idle {
        domain: Option<String>,
        layout: TrackedContent<LayoutNode>,
    },
    Dns {
        domain: String,
        path: String,
        dns_socket: Socket,
        dns_state: DnsState,
    },
    Https {
        domain: String,
        path: String,
        tls_client: TlsClient,
        https_state: HttpsState,
    },
    Render {
        domain: Option<String>,
        html: String,
    },
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
            RequestState::Dns {
                domain, dns_state, ..
            } => write!(f, "DNS {} {:?}", domain, dns_state),
            RequestState::Https { https_state, .. } => write!(f, "HTTPS {:?}", https_state),
            RequestState::Render { .. } => write!(f, "Render"),
        }
    }
}

fn get_progress_repr(request_state: &RequestState) -> (u64, Cow<str>) {
    match request_state {
        RequestState::Dns { dns_state, .. } => match dns_state {
            DnsState::Connecting => (0, Cow::Borrowed("DNS: connecting")),
            DnsState::Sending { out_count } => {
                (1, Cow::Owned(format!("DNS: sent {} bytes", out_count)))
            }
            DnsState::ReceivingLen { .. } => (2, Cow::Borrowed("DNS: receiving response length")),
            DnsState::ReceivingResp { in_count } => {
                (3, Cow::Owned(format!("DNS: received {} bytes", in_count)))
            }
        },
        RequestState::Https { https_state, .. } => match https_state {
            HttpsState::Connecting => (4, Cow::Borrowed("HTTPS: connecting")),
            HttpsState::Sending { out_count } => {
                (5, Cow::Owned(format!("HTTPS: sent {} bytes", out_count)))
            }
            HttpsState::Receiving { in_count } => {
                (6, Cow::Owned(format!("HTTPS: received {} bytes", in_count)))
            }
        },
        RequestState::Render { .. } => (7, Cow::Borrowed("Rendering")),
        RequestState::Idle { .. } => (8, Cow::Borrowed("")),
    }
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const SCHEME: &str = "https://";
const DNS_SERVER_IP: [u8; 4] = [1, 1, 1, 1];
const BUFFER_SIZE: usize = 100_000;

fn main() {}

#[no_mangle]
pub fn init() -> () {
    log::set_max_level(LOGGING_LEVEL);
    log::set_logger(&LOGGER).unwrap();

    let win_rect = guestlib::get_win_rect();

    let url_text = String::from("https://news.ycombinator.com/");
    let url_len = url_text.len();

    let mut uuid_provider = uitk::UuidProvider::new();

    let state = AppState {
        pixel_data: PixelData::new(),
        url_text: TrackedContent::new(url_text, &mut uuid_provider),
        url_cursor: url_len,

        buffer: vec![0u8; BUFFER_SIZE],
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),
        webview_scroll_offsets: (0, 0),
        webview_scroll_dragging: (false, false),
        request_state: RequestState::Render {
            domain: None,
            html: format!(
                "<html>\n\
                <p bgcolor=\"#0000ff\">WELCOME</p>\n\
                </html>\n",
            ),
        },
    };
    unsafe {
        APP_STATE
            .set(state)
            .unwrap_or_else(|_| panic!("App already initialized"));
    }
}

#[no_mangle]
pub fn step() {
    let state = unsafe { APP_STATE.get_mut().expect("App not initialized") };

    let input_state = guestlib::get_input_state();
    let win_rect = guestlib::get_win_rect();
    let win_input_state = input_state.change_origin(&win_rect);

    let AppState {
        ui_store,
        uuid_provider,
        ..
    } = state;

    let time = guestlib::get_time();

    let mut framebuffer = state.pixel_data.get_framebuffer();
    framebuffer.fill(Color::BLACK);

    let mut uitk_context = ui_store.get_context(&mut framebuffer, &win_input_state, uuid_provider, time);

    let ui_layout = compute_ui_layout(&win_rect);

    let is_button_fired = uitk_context.button(&uitk::ButtonConfig {
        rect: ui_layout.button_rect.clone(),
        text: "GO".into(),
        ..Default::default()
    });

    uitk_context.editable_text(
        &uitk::EditableTextConfig {
            rect: ui_layout.url_bar_rect.clone(),
            color: Color::WHITE,
            bg_color: Some(Color::rgb(128, 128, 128)),
            ..Default::default()
        },
        &mut state.url_text,
        &mut state.url_cursor,
    );

    let (progress_val, progress_str) = get_progress_repr(&state.request_state);

    uitk_context.progress_bar(
        &uitk::ProgressBarConfig {
            rect: ui_layout.progress_bar_rect.clone(),
            max_val: 8,
            bg_color: Color::rgb(128, 128, 128),
            bar_color: Color::rgb(128, 128, 255),
            text_color: Color::WHITE,
            ..Default::default()
        },
        progress_val,
        &progress_str,
    );

    let url_go = is_button_fired || check_enter_pressed(&win_input_state);

    let prev_state_debug = format!("{:?}", state.request_state);
    try_update_request_state(state, url_go, &ui_layout, &win_input_state, time);
    let new_state_debug = format!("{:?}", state.request_state);

    if new_state_debug != prev_state_debug {
        log::info!(
            "Request state change: {} => {}",
            prev_state_debug,
            new_state_debug
        );
    }
}

fn compute_ui_layout(win_rect: &Rect) -> UiLayout {
    const BUTTON_W: u32 = 100;
    const BAR_H: u32 = 25;

    UiLayout {
        url_bar_rect: Rect {
            x0: 0,
            y0: 0,
            w: win_rect.w - BUTTON_W,
            h: BAR_H,
        },
        button_rect: Rect {
            x0: (win_rect.w - BUTTON_W).into(),
            y0: 0,
            w: BUTTON_W,
            h: BAR_H,
        },
        progress_bar_rect: Rect {
            x0: 0,
            y0: BAR_H.into(),
            w: win_rect.w,
            h: BAR_H,
        },
        canvas_rect: Rect {
            x0: 0,
            y0: (2 * BAR_H).into(),
            w: win_rect.w,
            h: win_rect.h - 2 * BAR_H,
        },
    }
}

fn check_enter_pressed(input_state: &InputState) -> bool {
    input_state.events.iter().any(|event| {
        if let Some(InputEvent::KeyPress {
            keycode: Keycode::KEY_ENTER,
        }) = event
        {
            true
        } else {
            false
        }
    })
}

fn try_update_request_state(
    state: &mut AppState,
    url_go: bool,
    ui_layout: &UiLayout,
    input_state: &InputState,
    time: f64,
) {
    match update_request_state(state, url_go, ui_layout, input_state, time) {
        Ok(_) => (),
        Err(err) => {
            log::error!("{}", err);
            state.request_state = RequestState::Render {
                domain: None,
                html: make_error_html(err),
            }
        }
    }
}

fn make_error_html(error: anyhow::Error) -> String {
    let errors: Vec<String> = error
        .chain()
        .enumerate()
        .map(|(i, sub_err)| format!("<p>{}: {}</p>", i, sub_err))
        .collect();

    format!(
        "<html>\n\
        <p bgcolor=\"#ff0000\">ERROR</p>\n\
        {}
        </html>\n",
        errors.join("\n")
    )
}

fn update_request_state(
    state: &mut AppState,
    url_go: bool,
    ui_layout: &UiLayout,
    input_state: &InputState,
    time: f64,
) -> anyhow::Result<()> {
    match &mut state.request_state {
        RequestState::Idle {
            domain: current_domain,
            layout,
        } => {
            let mut framebuffer = state.pixel_data.get_framebuffer();
            let mut uitk_context =
                state
                    .ui_store
                    .get_context(&mut framebuffer, input_state, &mut state.uuid_provider, time);

            let link_hover = html_canvas(
                &mut uitk_context,
                &layout,
                &ui_layout.canvas_rect,
                &mut state.webview_scroll_offsets,
                &mut state.webview_scroll_dragging,
            );

            let mut url_data: Option<(String, String)> = None;

            if url_go {
                let (domain, path) = parse_url(state.url_text.as_ref())?;
                url_data = Some((domain.to_string(), path.to_string()));
            } else if input_state.pointer.left_click_trigger {
                if let Some(href) = link_hover {
                    if let Some(current_domain) = current_domain {
                        if !href.starts_with(SCHEME) {
                            let path = format!("/{}", href);
                            url_data = Some((current_domain.clone(), path));
                        }
                    }
                }
            }

            if let Some((domain, path)) = url_data {
                let s_ref = state.url_text.mutate(&mut state.uuid_provider);
                let _ = core::mem::replace(s_ref, format!("{}{}{}", SCHEME, domain, path));
                let dns_socket = Socket::new(DNS_SERVER_IP, 53)?;
                state.request_state = RequestState::Dns {
                    domain,
                    path,
                    dns_socket,
                    dns_state: DnsState::Connecting,
                };
            }
        }

        RequestState::Dns {
            domain,
            path,
            dns_state,
            dns_socket,
        } => match dns_state {
            DnsState::Connecting => {
                let socket_ready = dns_socket.may_send() && dns_socket.may_recv();
                if socket_ready {
                    let tcp_bytes = dns::make_tcp_dns_request(domain);
                    state.buffer.clear();
                    state.buffer.write(&tcp_bytes)?;
                    *dns_state = DnsState::Sending { out_count: 0 };
                }
            }

            DnsState::Sending { out_count } => {
                let n = dns_socket
                    .write(&state.buffer[*out_count..])
                    .context("Could not write to DNS socket")?;
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.resize(2, 0u8);
                    *dns_state = DnsState::ReceivingLen { in_count: 0 };
                }
            }

            DnsState::ReceivingLen { in_count } => {
                let n = dns_socket
                    .read(&mut state.buffer[*in_count..])
                    .context("Could not read from DNS socket")?;
                *in_count += n;

                if *in_count >= 2 {
                    let len_bytes: [u8; 2] = state.buffer.as_slice().try_into()?;
                    let dns_len: usize = u16::from_be_bytes(len_bytes)
                        .try_into()
                        .context("Invalid DNS response data")?;

                    state.buffer.resize(dns_len, 0u8);

                    *dns_state = DnsState::ReceivingResp { in_count: 0 };
                }
            }

            DnsState::ReceivingResp { in_count } => {
                let n = dns_socket
                    .read(&mut state.buffer[*in_count..])
                    .context("Could not read from DNS socket")?;
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

        RequestState::Https {
            domain,
            path,
            tls_client,
            https_state,
        } => match https_state {
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
                        path, domain
                    )?;
                    *https_state = HttpsState::Sending { out_count: 0 };
                }
            }

            HttpsState::Sending { out_count } => {
                tls_client.update();
                let n = tls_client.write(&state.buffer[*out_count..])?;
                *out_count += n;

                if *out_count >= state.buffer.len() {
                    state.buffer.clear();
                    *https_state = HttpsState::Receiving { in_count: 0 };
                }
            }

            HttpsState::Receiving { in_count } => {
                let n_plaintext = tls_client.update();
                *in_count += n_plaintext;

                if n_plaintext > 0 {
                    let len = state.buffer.len();
                    state.buffer.resize(len + n_plaintext, 0u8);
                    tls_client.read_exact(&mut state.buffer[len..len + n_plaintext])?;
                } else if tls_client.tls_closed() {
                    let http_string = core::str::from_utf8(&state.buffer)?;
                    //guestlib::qemu_dump(http_string.as_bytes());
                    let (_header, body) = parse_http(http_string)?;
                    //guestlib::qemu_dump(body.as_bytes());
                    state.request_state = RequestState::Render {
                        domain: Some(domain.clone()),
                        html: body,
                    };
                }
            }
        },

        RequestState::Render { domain, html } => {
            let html_tree = parse_html(html)?;
            let layout = compute_layout(&html_tree)?;

            //log::debug!("Layout: {:?}", layout.rect);
            state.request_state = RequestState::Idle {
                domain: domain.clone(),
                layout: TrackedContent::new(layout, &mut state.uuid_provider),
            };
        }
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
        }
        _ => body.to_owned(),
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
            }
        };
    }

    chunks.join("")
}

type HttpHeader = BTreeMap<String, String>;

fn parse_header(http_response: &str) -> anyhow::Result<(HttpHeader, &str)> {
    let i = http_response
        .find("\r\n\r\n")
        .ok_or(anyhow::anyhow!("Could not locate header"))?;

    let (header_str, body) = http_response.split_at(i);

    log::debug!("HTTP response header:\n{}", header_str);

    let header = header_str
        .split("\r\n")
        .filter_map(|line| {
            let (key, val) = line.split_once(":")?;
            let val = val.trim().to_owned();
            let key = key.to_lowercase();
            Some((key, val))
        })
        .collect();

    Ok((header, body))
}

fn parse_url(url: &str) -> anyhow::Result<(&str, &str)> {
    if !url.starts_with(SCHEME) {
        return Err(anyhow::anyhow!("Invalid URL (no https://)"));
    }

    let (_scheme, s) = url.split_at(SCHEME.len());

    let (domain, path) = match s.find("/") {
        Some(i) => s.split_at(i),
        None => (s, "/"),
    };

    Ok((domain, path))
}
