//! A `no_std` UEFI GUI toolkit.
//!
//! # Warning
//! This project is still experimental and not yet mature enough for production
//! use. The API may change in breaking ways in future versions, and the software
//! is provided without any warranty, express or implied, including warranties of
//! merchantability, fitness for a particular purpose, or non-infringement.
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
//! let mode_info = gop.current_mode_info();
//! let mut fb  = gfx::Framebuffer::from_mode_info(&mode_info).unwrap();
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
