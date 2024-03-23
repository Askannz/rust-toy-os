extern crate alloc;

use std::io::{Result, Read, Write};

use core::cell::OnceCell;
use alloc::format;
use guestlib::{tcp_read, tcp_write, FramebufferHandle};
use applib::{Color, Rect};
use applib::drawing::text::{draw_str, DEFAULT_FONT};
use applib::drawing::text::{RichText, HACK_15, Font};
use applib::ui::button::{Button, ButtonConfig};
use applib::ui::text::{ScrollableText, TextConfig};

mod tls;

use tls::TlsClient;

struct AppState {
    fb_handle: FramebufferHandle,
    button: Button,
    text_area: ScrollableText,

    tls_client: Option<TlsClient<Socket>>,

    first_frame: bool,

    recv_buffer: Vec<u8>,
    request_state: RequestState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RequestState {
    Idle,
    Connecting,
    Sending { total_sent: usize },
    Receiving { total_recv: usize },
    Done,
}

static mut APP_STATE: OnceCell<AppState> = OnceCell::new();

const SERVER_IP: [u8; 4] = [209, 216, 230, 207];
const SERVER_NAME: &str = "news.ycombinator.com";

const REQUEST_DATA: &[u8] = concat!(
    "GET / HTTP/1.1\r\n",
    "Host: news.ycombinator.com\r\n",
    "Connection: close\r\n",
    "Accept-Encoding: identity\r\n",
    "\r\n"
).as_bytes();

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

    let rect_button = Rect { x0: 0, y0: 0, w: 100, h: 25 };
    let rect_console = Rect { x0: 0, y0: rect_button.h.into(), w: win_w, h: win_h };

    let state = AppState { 
        fb_handle,
        button: Button::new(&ButtonConfig {
            rect: rect_button,
            ..Default::default()
        }),
        text_area: ScrollableText::new(&TextConfig { 
            rect: rect_console,
            ..Default::default()
        }),

        tls_client: None,

        first_frame: true,
        recv_buffer: Vec::new(),
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

    let mut b = 0;
    if let Some(tls_client) = state.tls_client.as_mut() {
        b = tls_client.update();
    }

    let redraw_button = state.button.update(&win_input_state);

    let mut text_update = None;

    let new_state = match state.request_state {
    
        RequestState::Idle if state.button.is_fired() => {
            guestlib::tcp_connect(SERVER_IP, 443);
            RequestState::Connecting
        },

        RequestState::Connecting => {
            let socket_ready = guestlib::tcp_may_send() && guestlib::tcp_may_recv();
            if socket_ready {
                state.tls_client = Some(TlsClient::new(Socket, SERVER_NAME));
                RequestState::Sending { total_sent: 0 } 
            } else {
                RequestState::Connecting
            }
        },

        RequestState::Sending { mut total_sent } => {
            let sent = state.tls_client.as_mut().unwrap().write(&REQUEST_DATA[total_sent..]).unwrap();
            total_sent += sent;
            println!("TLS: sent {}/{}B", total_sent, REQUEST_DATA.len());
            if total_sent < REQUEST_DATA.len() {
                RequestState::Sending { total_sent }
            } else {
                RequestState::Receiving { total_recv: 0 }
                //RequestState::Done
            }
        },    

        RequestState::Receiving { mut total_recv } => {

            if b == 0 && !guestlib::tcp_may_recv() {

                let resp_str = core::str::from_utf8(&state.recv_buffer).expect("Not UTF-8");

                let i1 = resp_str.find("<html").expect("No <html> tag");
                let i2 = resp_str.find("</html>").expect("No </html> tag");
                let html = &resp_str[i1..i2+7];

                let rich_text = render_html(html);

                text_update = Some(rich_text);

                RequestState::Done

            } else {
                let mut buf = vec![0u8; b];
                state.tls_client.as_mut().unwrap().read_exact(&mut buf).unwrap();
                let read_len = b;
                state.recv_buffer.extend_from_slice(&buf[..read_len]);
                total_recv += read_len;
                println!("Total recv {}", total_recv);

                RequestState::Receiving { total_recv }
            }
        }

        req_state => req_state
    };

    if new_state != state.request_state {
        println!("Request state change: {:?} => {:?}", state.request_state, new_state);
        state.request_state = new_state
    }

    let redraw_text = state.text_area.update(&win_input_state, text_update);

    let redraw = redraw_button || redraw_text || state.first_frame;

    if !redraw { return; }

    let mut framebuffer = guestlib::get_framebuffer(&mut state.fb_handle);
    framebuffer.fill(Color::WHITE);
    state.text_area.draw(&mut framebuffer);
    state.button.draw(&mut framebuffer);
    state.first_frame = false;
}


fn render_html(html: &str) -> RichText {

    use html_parser::{Dom, Node, Result};

    let dom = Dom::parse(html).expect("Invalid HTML");

    let mut rich_text = RichText::new();

    fn walk(rich_text: &mut RichText, node: &Node, is_link: bool, mut depth: usize, mut bg_color: Option<Color>) {
        match node {
            Node::Element(element) if element.name == "head" => (),
            Node::Element(element) => {

                if let Some(Some(hex_str)) = element.attributes.get("bgcolor") {
                    let mut color_bytes = hex::decode(hex_str.replace("#", "")).expect("Invalid color");

                    match color_bytes.len() {
                        3 => color_bytes.push(255),
                        4 => (),
                        _ => panic!("Invalid color: {:?}", color_bytes)
                    };

                    let color_bytes: [u8; 4] = color_bytes.try_into().unwrap();

                    bg_color = Some(Color::from_u32(u32::from_le_bytes(color_bytes)));
                }

                let is_link = element.name == "a";
                if element.name == "tr" { depth += 1; }

                for child in &element.children {
                    walk(rich_text, &child, is_link, depth, bg_color);
                }

                if element.name == "tr" {
                    rich_text.add_part("\n-\n", Color::BLACK, &HACK_15, bg_color);
                }

                if element.name == "table" {
                    rich_text.add_part("-\n", Color::BLACK, &HACK_15, bg_color);
                }
            },
            Node::Text(ref text) if is_link => {
                let line_text = format!("{}{}", " ".repeat(depth), text);
                rich_text.add_part(&line_text, Color::BLACK, &HACK_15, bg_color);
            }
            _ => ()
        };
    }

    let root = dom.children.get(0).unwrap();
    walk(&mut rich_text, &root, false, 0, None);

    rich_text
}
