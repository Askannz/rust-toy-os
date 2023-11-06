#![no_std]

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
        // TODO: bounds check
        FrameBufRegion { fb: self, rect: rect.clone() }
    }

    pub fn as_region<'b>(&'b mut self) -> FrameBufRegion<'a, 'b> {
        let rect = Rect { x0: 0, y0: 0, w: self.w, h: self.h };
        FrameBufRegion { fb: self, rect }
    }

    fn get_pixel_mut(&mut self, x: i32, y: i32) -> &mut [u8] {
        // TODO: bounds check
        let i = (y * self.w + x) as usize * 4;
        &mut self.data[i..i+4]
    }

    fn get_pixel(&self, x: i32, y: i32) -> &[u8] {
        // TODO: bounds check
        let i = (y * self.w + x) as usize * 4;
        &self.data[i..i+4]
    }
}

impl<'a, 'b> FrameBufRegion<'a, 'b> {


    pub fn get_pixel_mut(&mut self, x: i32, y: i32) -> &mut [u8] {
        // TODO: bounds check
        let x_fb = x + self.rect.x0;
        let y_fb = y + self.rect.y0;
        self.fb.get_pixel_mut(x_fb, y_fb)
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> &[u8] {
        // TODO: bounds check
        let x_fb = x + self.rect.x0;
        let y_fb = y + self.rect.y0;
        self.fb.get_pixel(x_fb, y_fb)
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: &Color) {
        let &Color(r, g, b) = color;
        self.get_pixel_mut(x, y).copy_from_slice(&[r, g, b, 0xff]);
    }

    pub fn copy_from(&mut self, src: &FrameBufRegion) {

        let w = i32::min(self.rect.w, src.rect.w);
        let h = i32::min(self.rect.h, src.rect.h);

        for x in 0..w {
            for y in 0..h {
                let px_src = src.get_pixel(x, y);
                self.get_pixel_mut(x, y).copy_from_slice(px_src);
            }
        }
    }

    pub fn fill(&mut self, color: &Color) {
    
        let Rect { x0, y0, w, h } = self.rect;
    
        for x in x0..x0+w {
            for y in y0..y0+h {
                self.set_pixel(x, y, color);
            }
        }
    }
}
