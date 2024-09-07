use alloc::vec::Vec;
use lazy_static::lazy_static;
use crate::app::AppDescriptor;
use applib::{decode_png, Framebuffer, OwnedPixels, Rect};

lazy_static! {

    //
    // Wallpaper

    pub static ref WALLPAPER: Vec<u8> = decode_png(include_bytes!("../../wallpaper.png"));


    //
    // App icons

    static ref CUBE_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/cube.png"));
    static ref CHRONO_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/chronometer.png"));
    static ref TERMINAL_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/terminal.png"));

    //
    // WASM apps

    pub static ref APPLICATIONS: [AppDescriptor; 4] = [
        AppDescriptor {
            data: include_bytes!("../../embedded_data/cube_3d.wasm"),
            launch_rect: Rect {
                x0: 100,
                y0: 100,
                w: 200,
                h: 40
            },
            name: "3D Cube",
            init_win_rect: Rect {
                x0: 200,
                y0: 200,
                w: 200,
                h: 200
            },
            icon: Some(&CUBE_ICON),
        },
        AppDescriptor {
            data: include_bytes!("../../embedded_data/chronometer.wasm"),
            launch_rect: Rect {
                x0: 100,
                y0: 150,
                w: 200,
                h: 40
            },
            name: "Chronometer",
            init_win_rect: Rect {
                x0: 600,
                y0: 200,
                w: 200,
                h: 200
            },
            icon: Some(&CHRONO_ICON),
        },
        AppDescriptor {
            data: include_bytes!("../../embedded_data/terminal.wasm"),
            launch_rect: Rect {
                x0: 100,
                y0: 200,
                w: 200,
                h: 40
            },
            name: "Terminal",
            init_win_rect: Rect {
                x0: 400,
                y0: 300,
                w: 600,
                h: 300
            },
            icon: Some(&TERMINAL_ICON),
        },
        AppDescriptor {
            data: include_bytes!("../../embedded_data/web_browser.wasm"),
            launch_rect: Rect {
                x0: 100,
                y0: 250,
                w: 200,
                h: 40
            },
            name: "Web Browser",
            init_win_rect: Rect {
                x0: 400,
                y0: 300,
                w: 800,
                h: 600
            },
            icon: None,
        },
    ];
}