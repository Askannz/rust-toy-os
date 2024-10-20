use crate::app::AppDescriptor;
use alloc::vec::Vec;
use applib::{decode_png, Framebuffer, OwnedPixels, Rect};
use lazy_static::lazy_static;

lazy_static! {

    //
    // Wallpaper

    pub static ref WALLPAPER: Vec<u8> = decode_png(include_bytes!("../../wallpaper.png"));


    //
    // App icons

    pub static ref CUBE_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/cube.png"));
    pub static ref CHRONO_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/chronometer.png"));
    pub static ref TERMINAL_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/terminal.png"));
    pub static ref CLOSE_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/close.png"));
    pub static ref RELOAD_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/reload.png"));
    pub static ref BLANK_ICON: Framebuffer<OwnedPixels> = Framebuffer::new_owned(32, 32);

    //
    // WASM apps

    pub static ref APPLICATIONS: [AppDescriptor; 4] = [
        AppDescriptor {
            data: include_bytes!("../wasm/cube_3d.wasm"),
            name: "3D Cube",
            init_win_rect: Rect {
                x0: 200,
                y0: 200,
                w: 200,
                h: 200
            },
            icon: &CUBE_ICON,
        },
        AppDescriptor {
            data: include_bytes!("../wasm/chronometer.wasm"),
            name: "Chronometer",
            init_win_rect: Rect {
                x0: 600,
                y0: 200,
                w: 200,
                h: 200
            },
            icon: &CHRONO_ICON,
        },
        AppDescriptor {
            data: include_bytes!("../wasm/terminal.wasm"),
            name: "Terminal",
            init_win_rect: Rect {
                x0: 400,
                y0: 300,
                w: 600,
                h: 300
            },
            icon: &TERMINAL_ICON,
        },
        AppDescriptor {
            data: include_bytes!("../wasm/web_browser.wasm"),
            name: "Web Browser",
            init_win_rect: Rect {
                x0: 400,
                y0: 300,
                w: 800,
                h: 600
            },
            icon: &TERMINAL_ICON,
        },
    ];
}
