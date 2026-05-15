//! Software framebuffer and drawing primitives.
//!
//! Text is rendered with fontdue (grayscale antialiasing).  Each glyph's
//! coverage bitmap is alpha-blended over whatever is already in the buffer,
//! so the caller fills regions with `fill()` before drawing text on top.

extern crate alloc;

use alloc::vec::Vec;
use uefi::proto::console::gop::PixelFormat;

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b } }

    pub fn lerp(a: Color, b: Color, t: u32, total: u32) -> Color {
        if total == 0 { return a; }
        let inv = total - t;
        Color::rgb(
            ((a.r as u32 * inv + b.r as u32 * t) / total) as u8,
            ((a.g as u32 * inv + b.g as u32 * t) / total) as u8,
            ((a.b as u32 * inv + b.b as u32 * t) / total) as u8,
        )
    }

    pub const BLACK:       Color = Color::rgb(0x00, 0x00, 0x00);
    pub const WHITE:       Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const DESKTOP:     Color = Color::rgb(0x00, 0x80, 0x80);
    pub const FACE:        Color = Color::rgb(0xD4, 0xD0, 0xC8);
    pub const HIGHLIGHT:   Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const SHADOW:      Color = Color::rgb(0x80, 0x80, 0x80);
    pub const DARK_SHADOW: Color = Color::rgb(0x40, 0x40, 0x40);
    pub const FRAME:       Color = Color::rgb(0x00, 0x00, 0x00);
    pub const ACT_L:       Color = Color::rgb(0x00, 0x00, 0x80);
    pub const ACT_R:       Color = Color::rgb(0x10, 0x84, 0xD0);
    pub const INACT_L:     Color = Color::rgb(0x7B, 0x7B, 0x7B);
    pub const INACT_R:     Color = Color::rgb(0xB5, 0xB5, 0xB5);
    pub const ACT_TEXT:    Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const INACT_TEXT:  Color = Color::rgb(0xD4, 0xD0, 0xC8);
    pub const WINDOW:      Color = Color::rgb(0xFF, 0xFF, 0xFF);
    pub const WINDOW_TEXT: Color = Color::rgb(0x00, 0x00, 0x00);
    pub const TASKBAR:     Color = Color::rgb(0xD4, 0xD0, 0xC8);
}

// ---------------------------------------------------------------------------
// Framebuffer
// ---------------------------------------------------------------------------

pub struct Framebuffer {
    pub buf:    Vec<u32>,
    pub width:  u32,
    pub height: u32,
    fmt:        PixelFormat,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32, fmt: PixelFormat) -> Self {
        let n = (width * height) as usize;
        let mut buf = Vec::with_capacity(n);
        buf.resize(n, 0);
        Self { buf, width, height, fmt }
    }

    // -----------------------------------------------------------------------
    // Pixel encoding
    // -----------------------------------------------------------------------

    /// RGB → GOP-native u32.
    #[inline]
    pub fn pack(&self, c: Color) -> u32 {
        match self.fmt {
            PixelFormat::Rgb => ((c.b as u32) << 16) | ((c.g as u32) << 8) | (c.r as u32),
            _                => ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32),
        }
    }

    /// GOP-native u32 → RGB.
    #[inline]
    pub fn unpack(&self, p: u32) -> Color {
        match self.fmt {
            PixelFormat::Rgb => Color::rgb(p as u8, (p >> 8) as u8, (p >> 16) as u8),
            _                => Color::rgb((p >> 16) as u8, (p >> 8) as u8, p as u8),
        }
    }

    // -----------------------------------------------------------------------
    // Pixel writes
    // -----------------------------------------------------------------------

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, p: u32) {
        if x < self.width && y < self.height {
            self.buf[(y * self.width + x) as usize] = p;
        }
    }

    #[inline]
    pub fn set_i(&mut self, x: i32, y: i32, p: u32) {
        if x >= 0 && y >= 0 { self.set(x as u32, y as u32, p); }
    }

    /// Alpha-blend `fg` over the existing pixel at (x, y).
    /// `coverage` 0 = transparent, 255 = opaque.
    #[inline]
    pub fn blend_pixel(&mut self, x: u32, y: u32, fg: Color, coverage: u8) {
        if x >= self.width || y >= self.height || coverage == 0 { return; }
        let idx = (y * self.width + x) as usize;
        if coverage == 255 {
            self.buf[idx] = self.pack(fg);
            return;
        }
        let bg  = self.unpack(self.buf[idx]);
        let a   = coverage as u32;
        let inv = 255 - a;
        let r   = ((fg.r as u32 * a + bg.r as u32 * inv) / 255) as u8;
        let g   = ((fg.g as u32 * a + bg.g as u32 * inv) / 255) as u8;
        let b   = ((fg.b as u32 * a + bg.b as u32 * inv) / 255) as u8;
        self.buf[idx] = self.pack(Color::rgb(r, g, b));
    }

    // -----------------------------------------------------------------------
    // Fills and outlines
    // -----------------------------------------------------------------------

    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, p: u32) {
        let x0 = x.max(0) as u32;
        let y0 = y.max(0) as u32;
        let xe = x.saturating_add(w as i32);
        let ye = y.saturating_add(h as i32);
        if xe <= 0 || ye <= 0 { return; }
        let x1 = (xe as u32).min(self.width);
        let y1 = (ye as u32).min(self.height);
        if x0 >= x1 || y0 >= y1 { return; }
        let row_w = (x1 - x0) as usize;
        for row in y0..y1 {
            let base = (row * self.width + x0) as usize;
            self.buf[base..base + row_w].fill(p);
        }
    }

    pub fn rect_outline(&mut self, x: i32, y: i32, w: u32, h: u32, p: u32) {
        self.fill(x,                y,                w, 1, p);
        self.fill(x,                y + h as i32 - 1, w, 1, p);
        self.fill(x,                y,                1, h, p);
        self.fill(x + w as i32 - 1, y,                1, h, p);
    }

    // -----------------------------------------------------------------------
    // Gradient
    // -----------------------------------------------------------------------

    pub fn gradient_h(&mut self, x: i32, y: i32, w: u32, h: u32,
                      left: Color, right: Color) {
        for col in 0..w {
            let c = Color::lerp(left, right, col, w.saturating_sub(1).max(1));
            let p = self.pack(c);
            self.fill(x + col as i32, y, 1, h, p);
        }
    }

    // -----------------------------------------------------------------------
    // 3-D borders
    // -----------------------------------------------------------------------

    pub fn border_raised(&mut self, x: i32, y: i32, w: u32, h: u32) {
        let fr = self.pack(Color::FRAME);
        let hi = self.pack(Color::HIGHLIGHT);
        let sh = self.pack(Color::SHADOW);
        let dk = self.pack(Color::DARK_SHADOW);
        self.fill(x,                y,                w, 1, fr);
        self.fill(x,                y,                1, h, fr);
        self.fill(x + w as i32 - 1, y,                1, h, dk);
        self.fill(x,                y + h as i32 - 1, w, 1, dk);
        if w > 2 && h > 2 {
            self.fill(x + 1,            y + 1,            w - 2, 1, hi);
            self.fill(x + 1,            y + 1,            1, h - 2, hi);
            self.fill(x + w as i32 - 2, y + 1,            1, h - 2, sh);
            self.fill(x + 1,            y + h as i32 - 2, w - 2, 1, sh);
        }
    }

    pub fn border_sunken(&mut self, x: i32, y: i32, w: u32, h: u32) {
        let hi = self.pack(Color::HIGHLIGHT);
        let sh = self.pack(Color::SHADOW);
        let dk = self.pack(Color::DARK_SHADOW);
        self.fill(x,                y,                w, 1, sh);
        self.fill(x,                y,                1, h, sh);
        self.fill(x + w as i32 - 1, y,                1, h, hi);
        self.fill(x,                y + h as i32 - 1, w, 1, hi);
        if w > 2 && h > 2 {
            self.fill(x + 1,            y + 1,            w - 2, 1, dk);
            self.fill(x + 1,            y + 1,            1, h - 2, dk);
            self.fill(x + w as i32 - 2, y + 1,            1, h - 2, hi);
            self.fill(x + 1,            y + h as i32 - 2, w - 2, 1, hi);
        }
    }

    // -----------------------------------------------------------------------
    // Antialiased text  (fontdue)
    // -----------------------------------------------------------------------
    //
    // Coordinate convention
    // ---------------------
    //   pen_x      — left edge of the glyph advance box
    //   baseline_y — the typographic baseline in screen pixels (Y increases ↓)
    //
    // fontdue Metrics
    //   xmin            — px from pen to left edge of bitmap (may be negative)
    //   ymin            — px from baseline to bottom of bitmap (negative = descender)
    //   width / height  — bitmap dimensions
    //
    // So the top-left of the bitmap in screen coords is:
    //   (pen_x + xmin,  baseline_y - ymin - height)

    /// Blit one antialiased glyph over the current buffer contents.
    pub fn glyph_aa(&mut self, pen_x: i32, baseline_y: i32,
                    m: &fontdue::Metrics, bitmap: &[u8], fg: Color) {
        let blit_x = pen_x + m.xmin;
        let blit_y = baseline_y - m.ymin - m.height as i32;
        for row in 0..m.height {
            for col in 0..m.width {
                let cov = bitmap[row * m.width + col];
                if cov == 0 { continue; }
                let sx = blit_x + col as i32;
                let sy = blit_y + row as i32;
                if sx >= 0 && sy >= 0 {
                    self.blend_pixel(sx as u32, sy as u32, fg, cov);
                }
            }
        }
    }

    /// Draw a UTF-8 string; returns the pen x after the last glyph.
    pub fn text_aa(&mut self, pen_x: i32, baseline_y: i32,
                   s: &str, fg: Color,
                   font: &fontdue::Font, px: f32) -> i32 {
        let mut x = pen_x;
        for ch in s.chars() {
            let (m, bmp) = font.rasterize(ch, px);
            self.glyph_aa(x, baseline_y, &m, &bmp, fg);
            x += m.advance_width as i32;
        }
        x
    }

    /// Pixel-width of a string (sum of rounded advance widths).
    pub fn text_width(s: &str, font: &fontdue::Font, px: f32) -> i32 {
        s.chars()
            .map(|c| font.metrics(c, px).advance_width as i32)
            .sum()
    }

    /// Draw a signed integer; returns pen x after the last digit.
    pub fn int_aa(&mut self, pen_x: i32, baseline_y: i32, n: i32,
                  fg: Color, font: &fontdue::Font, px: f32) -> i32 {
        let mut buf = [0u8; 12];
        let mut pos = buf.len();
        let neg = n < 0;
        let mut v = if neg { -(n as i64) } else { n as i64 };
        loop {
            pos -= 1; buf[pos] = b'0' + (v % 10) as u8; v /= 10;
            if v == 0 { break; }
        }
        if neg { pos -= 1; buf[pos] = b'-'; }
        let s = core::str::from_utf8(&buf[pos..]).unwrap_or("?");
        self.text_aa(pen_x, baseline_y, s, fg, font, px)
    }

    /// Draw text centred in a horizontal span.
    pub fn text_centered_aa(&mut self, area_x: i32, area_w: u32, baseline_y: i32,
                             s: &str, fg: Color,
                             font: &fontdue::Font, px: f32) {
        let tw  = Self::text_width(s, font, px);
        let off = if tw < area_w as i32 { (area_w as i32 - tw) / 2 } else { 0 };
        self.text_aa(area_x + off, baseline_y, s, fg, font, px);
    }

    // -----------------------------------------------------------------------
    // Present
    // -----------------------------------------------------------------------

    /// # Safety
    /// `gop_ptr` must point to a valid GOP framebuffer; `gop_stride` is
    /// pixels-per-scan-line as reported by GOP.
    pub unsafe fn present_to(&self, gop_ptr: *mut u8, gop_stride: usize) {
        for row in 0..self.height as usize {
            let src_base = row * self.width as usize;
            let dst_base = row * gop_stride;
            let dst = gop_ptr.add(dst_base * 4) as *mut u32;
            core::ptr::copy_nonoverlapping(
                self.buf.as_ptr().add(src_base),
                dst,
                self.width as usize,
            );
        }
    }
}
