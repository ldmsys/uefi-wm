//! Software framebuffer and drawing primitives.
//!
//! Text is rendered with `fontdue` grayscale antialiasing. Each glyph's
//! coverage bitmap is alpha-blended over whatever is already in the buffer,
//! so the caller fills regions with `fill()` before drawing text on top.

extern crate alloc;

use alloc::vec::Vec;
use uefi::proto::console::gop::{FrameBuffer as GopFrameBuffer, PixelFormat};

// ---------------------------------------------------------------------------
// Colour
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
/// An eight-bit RGB color independent of GOP's packed pixel order.
pub struct Color {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

impl Color {
    /// Constructs an RGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b } }

    /// Linearly interpolates from `a` to `b` at `t / total`.
    ///
    /// Returns `a` when `total` is zero. Callers must keep `t <= total`; a
    /// larger `t` underflows `total - t` in debug builds.
    pub fn lerp(a: Color, b: Color, t: u32, total: u32) -> Color {
        if total == 0 { return a; }
        let inv = total - t;
        Color::rgb(
            ((a.r as u32 * inv + b.r as u32 * t) / total) as u8,
            ((a.g as u32 * inv + b.g as u32 * t) / total) as u8,
            ((a.b as u32 * inv + b.b as u32 * t) / total) as u8,
        )
    }

    /// Black.
    pub const BLACK:       Color = Color::rgb(0x00, 0x00, 0x00);
    /// White.
    pub const WHITE:       Color = Color::rgb(0xFF, 0xFF, 0xFF);
    /// Desktop teal.
    pub const DESKTOP:     Color = Color::rgb(0x00, 0x80, 0x80);
    /// Standard control face.
    pub const FACE:        Color = Color::rgb(0xD4, 0xD0, 0xC8);
    /// Light 3-D border edge.
    pub const HIGHLIGHT:   Color = Color::rgb(0xFF, 0xFF, 0xFF);
    /// Dark 3-D border edge.
    pub const SHADOW:      Color = Color::rgb(0x80, 0x80, 0x80);
    /// Darkest 3-D border edge.
    pub const DARK_SHADOW: Color = Color::rgb(0x40, 0x40, 0x40);
    /// Black outer frame.
    pub const FRAME:       Color = Color::rgb(0x00, 0x00, 0x00);
    /// Active-title gradient start.
    pub const ACT_L:       Color = Color::rgb(0x00, 0x00, 0x80);
    /// Active-title gradient end.
    pub const ACT_R:       Color = Color::rgb(0x10, 0x84, 0xD0);
    /// Inactive-title gradient start.
    pub const INACT_L:     Color = Color::rgb(0x7B, 0x7B, 0x7B);
    /// Inactive-title gradient end.
    pub const INACT_R:     Color = Color::rgb(0xB5, 0xB5, 0xB5);
    /// Active-title text.
    pub const ACT_TEXT:    Color = Color::rgb(0xFF, 0xFF, 0xFF);
    /// Inactive-title text.
    pub const INACT_TEXT:  Color = Color::rgb(0xD4, 0xD0, 0xC8);
    /// Window background.
    pub const WINDOW:      Color = Color::rgb(0xFF, 0xFF, 0xFF);
    /// Window foreground text.
    pub const WINDOW_TEXT: Color = Color::rgb(0x00, 0x00, 0x00);
    /// Taskbar color retained by the visual palette.
    pub const TASKBAR:     Color = Color::rgb(0xD4, 0xD0, 0xC8);
}

// ---------------------------------------------------------------------------
// Framebuffer
// ---------------------------------------------------------------------------

/// A tightly packed software back-buffer using GOP-native `u32` pixels.
pub struct Framebuffer {
    buf:        Vec<u32>,
    width:      u32,
    height:     u32,
    fmt:        PixelFormat,
}

/// Error returned when a software framebuffer cannot be allocated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FramebufferError {
    /// `width * height` cannot be represented as a `usize` pixel count.
    DimensionsOverflow,
    /// The allocator could not reserve storage for every pixel.
    AllocationFailed,
}

impl Framebuffer {
    /// Allocates a zero-filled, tightly packed `width * height` back-buffer.
    ///
    /// Returns an error rather than wrapping the pixel count or aborting during
    /// the initial capacity reservation.
    pub fn new(width: u32, height: u32, fmt: PixelFormat) -> Result<Self, FramebufferError> {
        let width_usize = usize::try_from(width)
            .map_err(|_| FramebufferError::DimensionsOverflow)?;
        let height_usize = usize::try_from(height)
            .map_err(|_| FramebufferError::DimensionsOverflow)?;
        let n = width_usize.checked_mul(height_usize)
            .ok_or(FramebufferError::DimensionsOverflow)?;
        let mut buf = Vec::new();
        buf.try_reserve_exact(n).map_err(|_| FramebufferError::AllocationFailed)?;
        buf.resize(n, 0);
        Ok(Self { buf, width, height, fmt })
    }

    /// Returns the visible width in pixels.
    #[inline]
    pub const fn width(&self) -> u32 { self.width }

    /// Returns the visible height in pixels.
    #[inline]
    pub const fn height(&self) -> u32 { self.height }

    /// Returns the packed pixels in row-major order with no row padding.
    #[inline]
    pub fn pixels(&self) -> &[u32] { &self.buf }

    /// Returns mutable packed pixels without allowing the buffer to be resized.
    #[inline]
    pub fn pixels_mut(&mut self) -> &mut [u32] { &mut self.buf }

    // -----------------------------------------------------------------------
    // Pixel encoding
    // -----------------------------------------------------------------------

    /// Packs RGB into GOP's native `u32` byte order.
    ///
    /// [`PixelFormat::Rgb`] receives the RGB branch; every other variant uses
    /// the BGR branch, including `Bitmask` and `BltOnly`.
    #[inline]
    pub fn pack(&self, c: Color) -> u32 {
        match self.fmt {
            PixelFormat::Rgb => ((c.b as u32) << 16) | ((c.g as u32) << 8) | (c.r as u32),
            _                => ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32),
        }
    }

    /// Unpacks a GOP-native `u32` using the same format rule as [`Self::pack`].
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
    /// Writes a packed pixel, ignoring coordinates outside the back-buffer.
    pub fn set(&mut self, x: u32, y: u32, p: u32) {
        if x < self.width && y < self.height {
            self.buf[(y * self.width + x) as usize] = p;
        }
    }

    #[inline]
    /// Signed-coordinate form of [`Self::set`].
    pub fn set_i(&mut self, x: i32, y: i32, p: u32) {
        if x >= 0 && y >= 0 { self.set(x as u32, y as u32, p); }
    }

    /// Alpha-blends `fg` over the existing pixel at `(x, y)`.
    ///
    /// Coverage 0 is transparent and 255 is opaque. Out-of-bounds writes are
    /// ignored.
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

    /// Fills the portion of a signed rectangle that intersects the back-buffer.
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

    /// Draws a one-pixel outline using four clipped fills.
    pub fn rect_outline(&mut self, x: i32, y: i32, w: u32, h: u32, p: u32) {
        self.fill(x,                y,                w, 1, p);
        self.fill(x,                y + h as i32 - 1, w, 1, p);
        self.fill(x,                y,                1, h, p);
        self.fill(x + w as i32 - 1, y,                1, h, p);
    }

    // -----------------------------------------------------------------------
    // Gradient
    // -----------------------------------------------------------------------

    /// Fills a horizontal gradient from `left` to `right`.
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

    /// Draws the toolkit's two-pixel raised border.
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

    /// Draws the toolkit's two-pixel sunken border.
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

    /// Pixel width of a string (sum of individually truncated advance widths).
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

    /// Copies the visible back-buffer into a GOP framebuffer row by row.
    ///
    /// `gop_stride` is pixels per scan line, so destination padding is skipped.
    ///
    /// # Panics
    ///
    /// Panics if `gop_stride` is smaller than the visible width, if the visible
    /// rows do not fit in `gop_fb`, or if the size calculation overflows.
    pub fn present_to(&self, gop_fb: &mut GopFrameBuffer<'_>, gop_stride: usize) {
        let width = self.width as usize;
        let height = self.height as usize;
        assert!(gop_stride >= width, "GOP stride is smaller than the visible width");

        let required_pixels = if height == 0 {
            0
        } else {
            (height - 1)
                .checked_mul(gop_stride)
                .and_then(|last_row| last_row.checked_add(width))
                .expect("GOP framebuffer dimensions overflow")
        };
        let required_bytes = required_pixels.checked_mul(core::mem::size_of::<u32>())
            .expect("GOP framebuffer byte size overflows");
        assert!(required_bytes <= gop_fb.size(), "GOP framebuffer is too small");

        for row in 0..height {
            let src_base = row * width;
            let dst_base = row * gop_stride;
            for col in 0..width {
                let offset = (dst_base + col) * core::mem::size_of::<u32>();
                let bytes = self.buf[src_base + col].to_ne_bytes();
                // The bounds, pixel layout, and stride were validated above.
                unsafe { gop_fb.write_value(offset, bytes) };
            }
        }
    }
}
