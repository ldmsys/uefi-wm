//! A `no_std` UEFI GUI toolkit.
//!
//! # Modules
//! - [`gfx`] — software framebuffer and antialiased drawing primitives.
//! - [`input`] — keyboard and pointer input from UEFI protocols and direct PS/2 I/O.
//! - [`wm`] — themed floating window manager with widgets.
//!
//! # Minimal usage
//! ```ignore
//! // Font bytes are provided by the caller so the library itself stays
//! // font-agnostic.  Embed a TTF with include_bytes! in your application.
//! static FONT: &[u8] = include_bytes!("path/to/font.ttf");
//!
//! let mut fb  = gfx::Framebuffer::new(width, height, pixel_format).unwrap();
//! let mut wm  = wm::WindowManager::new(width, height, FONT, 16.0);
//! wm.set_theme(wm::Theme::Dark); // Light is the default; Classic is also available.
//! let mut drv = input::InputDriver::new(width, height);
//! // Build the UI, then enter the library-owned event loop.
//! wm.run(&mut fb, &mut drv, gop.frame_buffer(), gop_stride);
//! ```
//!
//! The scoped GOP protocol remains open while its [`uefi::proto::console::gop::FrameBuffer`]
//! is consumed by the event loop. `gop_stride` is measured in pixels.

#![no_std]
#![deny(warnings)]
extern crate alloc;

pub mod gfx;
pub mod input;
mod qr;
pub mod wm;
