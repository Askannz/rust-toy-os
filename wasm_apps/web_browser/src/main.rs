extern crate alloc;

use std::io::{Read, Write};

use std::borrow::Cow;
use std::fmt;
use std::fmt::Debug;

use alloc::collections::BTreeMap;
use alloc::format;
use anyhow::Context;
use html::render_list::RenderItem;
use lazy_static::lazy_static;
use applib::{Color, FbView, FbViewMut, Rect, StyleSheet};
use core::cell::OnceCell;
use guestlib::{PixelData, WasmLogger};

use applib::content::TrackedContent;
use applib::input::Keycode;
use applib::input::{InputEvent, InputState};
use applib::uitk::{self, ButtonConfig, UuidProvider, TextBoxState};
use applib::{Framebuffer, OwnedPixels};

mod dns;
mod html;
mod socket;
mod tls;

use html::canvas::html_canvas;
use html::{
    layout::{compute_layout, LayoutNode},
    parsing::parse_html,
    block_layout::{compute_block_layout},
    render_list::compute_render_list,
    render::render_html2,
};
use socket::Socket;
use tls::TlsClient;

static LOGGER: WasmLogger = WasmLogger;
const LOGGING_LEVEL: log::LevelFilter = log::LevelFilter::Debug;

lazy_static! {
    pub static ref HN_ICON: Framebuffer<OwnedPixels> = 
        Framebuffer::from_png(include_bytes!("../icons/websites/hackernews.png"));
    pub static ref MF_WEBSITE_ICON: Framebuffer<OwnedPixels> = 
        Framebuffer::from_png(include_bytes!("../icons/websites/mfwebsite.png"));
    pub static ref EX_WEBSITE_ICON: Framebuffer<OwnedPixels> = 
        Framebuffer::from_png(include_bytes!("../icons/websites/example.png"));
}

struct AppState {
    pixel_data: PixelData,

    url_text: TrackedContent<String>,
    url_textbox_state: TextBoxState,

    buffer: Vec<u8>,

    uuid_provider: UuidProvider,
    ui_store: uitk::UiStore,

    webview_scroll_offsets: (i64, i64),
    webview_scroll_dragging: (bool, bool),

    request_state: RequestState,
}

struct UiLayout {
    url_bar_rect: Rect,
    go_button_rect: Rect,
    reload_button_rect: Rect,
    progress_bar_rect: Rect,
    canvas_rect: Rect,
}

#[derive(Debug, Clone)]
struct HttpTarget {
    host: String,
    path: String,
}

enum RequestState {
    Home,
    Idle {
        http_target: Option<HttpTarget>,
        layout: TrackedContent<LayoutNode>,
        render_list: TrackedContent<Vec<RenderItem>>,
    },
    Dns {
        http_target: HttpTarget,
        dns_socket: Socket,
        dns_state: DnsState,
    },
    Https {
        http_target: HttpTarget,
        tls_client: TlsClient,
        https_state: HttpsState,
    },
    Render {
        http_target: Option<HttpTarget>,
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
            RequestState::Home => write!(f, "Home"),
            RequestState::Idle { .. } => write!(f, "Idle"),
            RequestState::Dns {
                http_target, dns_state, ..
            } => write!(f, "DNS {:?} {:?}", http_target, dns_state),
            RequestState::Https { https_state, .. } => write!(f, "HTTPS {:?}", https_state),
            RequestState::Render { .. } => write!(f, "Render"),
        }
    }
}

fn get_progress_repr(request_state: &RequestState) -> (u64, Cow<str>) {
    match request_state {
        RequestState::Home => (0, Cow::Borrowed("Home")),
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

    let url_text = String::from("https://example.com/");
    let url_len = url_text.len();

    let mut uuid_provider = uitk::UuidProvider::new();

    let state = AppState {
        pixel_data: PixelData::new(),
        url_text: TrackedContent::new(url_text, &mut uuid_provider),
        url_textbox_state: TextBoxState::new(),

        buffer: vec![0u8; BUFFER_SIZE],
        ui_store: uitk::UiStore::new(),
        uuid_provider: UuidProvider::new(),
        webview_scroll_offsets: (0, 0),
        webview_scroll_dragging: (false, false),
        request_state: RequestState::Home,
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

    let AppState {
        ui_store,
        uuid_provider,
        ..
    } = state;

    let time = guestlib::get_time();
    let stylesheet = guestlib::get_stylesheet();

    let mut framebuffer = state.pixel_data.get_framebuffer();

    let mut uitk_context = ui_store.get_context(
        &mut framebuffer,
        &stylesheet,
        &input_state,
        uuid_provider,
        time
    );

    let ui_layout = compute_ui_layout(&win_rect);

    let is_go_button_fired = uitk_context.button(&uitk::ButtonConfig {
        rect: ui_layout.go_button_rect.clone(),
        text: "GO".into(),
        ..Default::default()
    });

    let is_reload_button_fired = uitk_context.button(&uitk::ButtonConfig {
        rect: ui_layout.reload_button_rect.clone(),
        text: "Reload".into(),
        ..Default::default()
    });

    uitk_context.editable_text_box(
        &ui_layout.url_bar_rect,
        &mut state.url_text,
        &mut state.url_textbox_state,
        false,
        false,
        None::<&TrackedContent<String>>,
    );

    let (progress_val, progress_str) = get_progress_repr(&state.request_state);

    uitk_context.progress_bar(
        &uitk::ProgressBarConfig {
            rect: ui_layout.progress_bar_rect.clone(),
            max_val: 8,
            ..Default::default()
        },
        progress_val,
        &progress_str,
    );

    let url_bar_go = match is_go_button_fired || check_enter_pressed(&input_state) {
        false => None,
        true => Some(state.url_text.as_ref().to_owned())
    };

    let prev_state_debug = format!("{:?}", state.request_state);
    try_update_request_state(state, &stylesheet, url_bar_go, is_reload_button_fired, &ui_layout, &input_state, time);
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
    const RELOAD_BUTTON_W: u32 = 100;
    const GO_BUTTON_W: u32 = 40;
    const BAR_H: u32 = 25;

    UiLayout {
        url_bar_rect: Rect {
            x0: 0,
            y0: 0,
            w: win_rect.w - GO_BUTTON_W - RELOAD_BUTTON_W,
            h: BAR_H,
        },
        go_button_rect: Rect {
            x0: (win_rect.w - GO_BUTTON_W - RELOAD_BUTTON_W).into(),
            y0: 0,
            w: GO_BUTTON_W,
            h: BAR_H,
        },
        reload_button_rect: Rect {
            x0: (win_rect.w - RELOAD_BUTTON_W).into(),
            y0: 0,
            w: RELOAD_BUTTON_W,
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
    stylesheet: &StyleSheet,
    url_bar_go: Option<String>,
    is_reload_button_fired: bool,
    ui_layout: &UiLayout,
    input_state: &InputState,
    time: f64,
) {
    match update_request_state(state, stylesheet, url_bar_go, is_reload_button_fired, ui_layout, input_state, time) {
        Ok(_) => (),
        Err(err) => {
            log::error!("{}", err);
            state.request_state = RequestState::Render {
                http_target: None,
                html: make_error_html(err),
            }
        }
    }
}

fn make_error_html(error: anyhow::Error) -> String {

    let traceback: Vec<String> = error
        .chain()
        .enumerate()
        .map(|(i, sub_err)| format!(
            "<tr><td>{}: {}</td></tr>\n",
            i, sub_err
        ))
        .collect();

    format!(
        r##"<html>
            <table>
                <tr><td>Error:</td></tr>
                {}
            </table>
        </html>"##,
        traceback.join("\n"),
    )
}

fn update_request_state(
    state: &mut AppState,
    stylesheet: &StyleSheet,
    url_bar_go: Option<String>,
    is_reload_button_fired: bool,
    ui_layout: &UiLayout,
    input_state: &InputState,
    time: f64,
) -> anyhow::Result<()> {
    match &mut state.request_state {

        RequestState::Home => {

            const BUTTON_H: u32 = 50;
            const BUTTON_W: u32 = 400;

            struct Favorite {
                link: &'static str,
                icon: &'static Framebuffer<OwnedPixels>,
            }

            let favorites = [
                Favorite {
                    link: "https://motherfuckingwebsite.com",
                    icon: &MF_WEBSITE_ICON
                },
                Favorite {
                    //link: "https://news.ycombinator.com/item?id=43535688",
                    link: "https://news.ycombinator.com",
                    icon: &HN_ICON
                },
                Favorite {
                    link: "https://example.com",
                    icon: &EX_WEBSITE_ICON
                },
            ];
            let canvas_rect = &ui_layout.canvas_rect;

            let row_h = canvas_rect.h / (2 * favorites.len() + 1) as u32;

            let x0 = {
                let r = Rect { x0: 0, y0: 0, w: BUTTON_W, h: BUTTON_H };
                r.align_to_rect_horiz(&ui_layout.canvas_rect).x0
            };
    
            let mut framebuffer = state.pixel_data.get_framebuffer();
            let mut uitk_context = state.ui_store.get_context(
                &mut framebuffer,
                &stylesheet,
                input_state,
                &mut state.uuid_provider,
                time
            );

            let mut y = canvas_rect.y0 + row_h as i64;
            let mut clicked_url = None;

            for fav in favorites {

                let row_rect = Rect { x0, y0: y, w: BUTTON_W, h: row_h };
                let button_rect = Rect { x0, y0: y, w: BUTTON_W, h: BUTTON_H }
                    .align_to_rect_vert(&row_rect);
                let clicked = uitk_context.button(&ButtonConfig { 
                    rect: button_rect,
                    text: fav.link.to_string(),
                    icon: Some(fav.icon),
                    ..Default::default()
                });
                y += (2 * row_h) as i64;

                if clicked {
                    clicked_url = Some(fav.link);
                }
            }

            let url = {
                if let Some(url) = clicked_url { Some(url.to_owned()) }
                else if let Some(url) = url_bar_go { Some(url) }
                else { None }
            };

            if let Some(url) = url {
                let http_target = parse_url(&url)?;
                initiate_redirect(state, http_target)?;
            }
        },

        RequestState::Idle {
            http_target,
            layout,
            render_list,
        } => {
            let mut framebuffer = state.pixel_data.get_framebuffer();

            // let link_hover: Option<&str> = None;
            // let mut dst_fb = framebuffer.subregion_mut(&ui_layout.canvas_rect);
            // let r = dst_fb.shape_as_rect();
            // dst_fb.fill(Color::WHITE);
            // render_html2(&mut dst_fb, render_list, &r);

            let mut uitk_context = state.ui_store.get_context(
                &mut framebuffer,
                &stylesheet,
                input_state,
                &mut state.uuid_provider,
                time
            );

            let link_hover = html_canvas(
                &mut uitk_context,
                &render_list,
                &ui_layout.canvas_rect,
                &mut state.webview_scroll_offsets,
                &mut state.webview_scroll_dragging,
            );

            let new_http_target = {
                if let Some(url_text) = url_bar_go {
                    let new_http_target = parse_url(&url_text)?;
                    Some(new_http_target)
                } else if is_reload_button_fired {
                    http_target.clone()
                } else if input_state.pointer.left_click_trigger {

                    let mut new_http_target = None;
                    if let Some(href) = link_hover {
                        if let Some(http_target) = http_target {
                            if !href.starts_with(SCHEME) {
                                new_http_target = Some(HttpTarget { 
                                    host:  http_target.host.clone(),
                                    path: format!("/{}", href),
                                });
                            }
                        }
                    }

                    new_http_target
                } else  {
                    None
                }
            };

            if let Some(new_http_target) = new_http_target {
                initiate_redirect(state, new_http_target)?;
            }
        }

        RequestState::Dns {
            http_target,
            dns_state,
            dns_socket,
        } => match dns_state {
            DnsState::Connecting => {
                let socket_ready = dns_socket.may_send() && dns_socket.may_recv();
                if socket_ready {
                    let tcp_bytes = dns::make_tcp_dns_request(&http_target.host);
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
                        http_target: http_target.clone(),
                        tls_client: TlsClient::new(https_socket, &http_target.host),
                        https_state: HttpsState::Connecting,
                    }
                }
            }
        },

        RequestState::Https {
            http_target,
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
                        http_target.path, http_target.host,
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
                        http_target: Some(http_target.clone()),
                        html: body,
                    };
                }
            }
        },

        RequestState::Render { http_target, html } => {

            let html_tree = parse_html(html)?;
            //log::debug!("{}", html_tree.plot());

            let block_layout_tree = compute_block_layout(&html_tree);
            //log::debug!("{}", block_layout_tree.plot());

            let page_max_w = ui_layout.canvas_rect.w;

            let render_list = compute_render_list(&block_layout_tree, page_max_w);
            // for render_item in render_list.iter() {
            //     log::debug!("{:?}", render_item);
            // }

            
            let layout = compute_layout(&html_tree, page_max_w)?;

            //log::debug!("Layout: {:?}", layout.rect);
            state.request_state = RequestState::Idle {
                http_target: http_target.clone(),
                layout: TrackedContent::new(layout, &mut state.uuid_provider),
                render_list: TrackedContent::new(render_list, &mut state.uuid_provider),
            };
        }
    };

    Ok(())
}

fn initiate_redirect(state: &mut AppState, http_target: HttpTarget)  -> anyhow::Result<()> {

    let s_ref = state.url_text.mutate(&mut state.uuid_provider);
    let _ = core::mem::replace(s_ref, format!("{}{}{}", SCHEME, http_target.host, http_target.path));
    let dns_socket = Socket::new(DNS_SERVER_IP, 53)?;
    state.request_state = RequestState::Dns {
        http_target: http_target,
        dns_socket,
        dns_state: DnsState::Connecting,
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

fn parse_url(url: &str) -> anyhow::Result<HttpTarget> {
    if !url.starts_with(SCHEME) {
        return Err(anyhow::anyhow!("Invalid URL (no https://)"));
    }

    let (_scheme, s) = url.split_at(SCHEME.len());

    let (host, path) = match s.find("/") {
        Some(i) => s.split_at(i),
        None => (s, "/"),
    };

    Ok(HttpTarget { host: host.to_owned(), path: path.to_owned() })
}
