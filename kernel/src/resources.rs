use crate::app::AppDescriptor;
use alloc::vec::Vec;
use applib::{decode_png, Framebuffer, OwnedPixels, Rect, Color};
use applib::{StyleSheet, StyleSheetColors};
use lazy_static::lazy_static;

lazy_static! {

    //
    // Wallpaper

    pub static ref WALLPAPER: Framebuffer<OwnedPixels> = 
        Framebuffer::from_png(include_bytes!("../../wallpaper.png"));


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
    pub static ref MOVE_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/move.png"));
    pub static ref PLAY_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/play.png"));
    pub static ref PAUSE_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/pause.png"));
    pub static ref INSPECT_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/inspect.png"));
    pub static ref SPEEDOMETER_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/speedometer.png"));
    pub static ref CHIP_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/chip.png"));
    pub static ref NETWORK_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/network.png"));
    pub static ref WEB_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/web.png"));
    pub static ref PYTHON_ICON: Framebuffer<OwnedPixels> =
        Framebuffer::from_png(include_bytes!("../icons/python.png"));
    pub static ref BLANK_ICON: Framebuffer<OwnedPixels> = Framebuffer::new_owned(32, 32);

    //
    // Stylesheet

    pub static ref STYLESHEET: StyleSheet = StyleSheet {
        colors: StyleSheetColors {
            background: Color::rgb(0x44, 0x44, 0x44),
            blue: Color::rgb(0, 0, 150),
            purple: Color::rgb(100, 10, 210),
            element: Color::rgb(100, 100, 100),
            green: Color::rgb(0, 180, 0),
            hover_overlay: Color::rgba(150, 150, 150, 100),
            selected_overlay: Color::rgba(255, 255, 255, 100),
            red: Color::rgb(180, 0, 0),
            yellow: Color::rgb(180, 180, 0),
            text: Color::WHITE,
            accent: Color::rgb(122, 0, 255),
        },
        margin: 2,
    };

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
            name: "Python terminal",
            init_win_rect: Rect {
                x0: 400,
                y0: 300,
                w: 600,
                h: 300
            },
            icon: &PYTHON_ICON,
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
            icon: &WEB_ICON,
        },
    ];
}
