#![no_std]

#[repr(C)]
pub struct AppHandle<'a, 'b> {
    pub system_state: SystemState,
    pub app_rect: Rect,
    pub app_framebuffer: FrameBufRegion<'a, 'b>,
} 


#[derive(Debug, Clone)]
#[repr(C)]
pub struct SystemState {
    pub pointer: PointerState,
    pub time: u64,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct PointerState {
    pub x: i32,
    pub y: i32,
    pub clicked: bool
}



#[derive(Clone)]
pub struct Color(pub u8, pub u8, pub u8);
#[derive(Clone)]
pub struct Rect { pub x0: i32, pub y0: i32, pub w: i32, pub h: i32 }

impl Rect {
    pub fn check_in(&self, x: i32, y: i32) -> bool {
        return 
            x >= self.x0 && x < self.x0 + self.w &&
            y >= self.y0 && y < self.y0 + self.h
    }
}

pub struct Framebuffer<'a> {
    pub data: &'a mut [u8],
    pub w: i32,
    pub h: i32,
}

pub struct FrameBufRegion<'a, 'b> {
    pub fb: &'b mut Framebuffer<'a>,
    pub rect: Rect
}

impl<'a> Framebuffer<'a> {
    pub fn get_region<'b>(&'b mut self, rect: &Rect) -> FrameBufRegion<'a, 'b> {
        FrameBufRegion { fb: self, rect: rect.clone() }
    }
}

impl<'a, 'b> FrameBufRegion<'a, 'b> {
    pub fn set_pixel(&mut self, x: i32, y: i32, color: &Color) {
        let Color(r, g, b) = *color;
        let i = (((y+self.rect.y0) * self.fb.w + x + self.rect.x0) * 4) as usize;
        self.fb.data[i] = r;
        self.fb.data[i+1] = g;
        self.fb.data[i+2] = b;
        self.fb.data[i+3] = 0xff;
    }
}
