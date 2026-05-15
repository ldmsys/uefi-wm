//! `uefi-wm` — a no_std UEFI GUI toolkit.
//!
//! # Modules
//! - [`gfx`] — software framebuffer and antialiased drawing primitives.
//! - [`input`] — keyboard and mouse input drivers (UEFI protocols + PS/2 fallback).
//! - [`wm`] — floating window manager with widgets (Windows 2000 visual style).
//!
//! # Minimal usage
//! ```no_run
//! // Font bytes are provided by the caller so the library itself stays
//! // font-agnostic.  Embed a TTF with include_bytes! in your application.
//! static FONT: &[u8] = include_bytes!("path/to/font.ttf");
//!
//! let mut fb  = gfx::Framebuffer::new(width, height, pixel_format);
//! let mut wm  = wm::WindowManager::new(width, height, FONT, 16.0);
//! let mut drv = input::InputDriver::new(width, height);
//! // Build UI: wm.open() / wm.add_*() / wm.set_on_click() / wm.set_on_change(), then:
//! wm.run(&mut fb, &mut drv, gop_ptr, gop_stride);
//! ```

#![no_std]
#![deny(warnings)]
extern crate alloc;

pub mod gfx;
pub mod input;
pub mod wm;
