//! Keyboard and pointer input for the window manager.
//!
//! [`InputDriver`] exclusively opens EFI Simple Text Input, then probes EFI
//! Absolute Pointer, EFI Simple Pointer, and direct i8042 PS/2. Each call to
//! [`InputDriver::read_ptr`] polls every available source in that order; the
//! PS/2 path is not conditional on the EFI sources being idle. All sources
//! update the same cached button state, so edges are relative to the preceding
//! observation even when it came from another source.
//!
//! On x86 and x86_64, the direct PS/2 path uses port `0x64` for
//! status/commands and `0x60` for data. It is disabled when initialization does
//! not receive a `0xFA` acknowledgement from the mouse. On other architectures
//! the direct PS/2 backend is a disabled no-op; EFI protocols remain available.

extern crate alloc;

use alloc::vec::Vec;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::{Input, Key};
use uefi::proto::unsafe_protocol;
use uefi::{Event, Status};
use uefi_raw::protocol::console::{
    AbsolutePointerMode, AbsolutePointerProtocol, AbsolutePointerState,
};

// ---------------------------------------------------------------------------
// Thin safe wrapper for EFI_ABSOLUTE_POINTER_PROTOCOL
// (not yet in uefi 0.33 as a first-class type)
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[repr(transparent)]
#[unsafe_protocol(AbsolutePointerProtocol::GUID)]
/// Thin wrapper for the raw EFI Absolute Pointer protocol used by UEFI 0.33.
pub struct AbsolutePointer(AbsolutePointerProtocol);

impl AbsolutePointer {
    /// Reads one state, returning `None` for every EFI status except success.
    pub fn read_state(&mut self) -> Option<AbsolutePointerState> {
        let mut s = AbsolutePointerState::default();
        let ok = unsafe { (self.0.get_state)(&self.0, &mut s) };
        if ok == Status::SUCCESS { Some(s) } else { None }
    }

    /// Returns the protocol's mode structure.
    pub fn mode(&self) -> &AbsolutePointerMode {
        unsafe { &*self.0.mode }
    }

    /// Wraps the protocol's wait event, or returns `None` for a null event.
    pub fn wait_for_input_event(&self) -> Option<Event> {
        unsafe { Event::from_ptr(self.0.wait_for_input) }
    }
}

// ---------------------------------------------------------------------------
// Direct PS/2 mouse driver (bypasses UEFI protocol layer)
// ---------------------------------------------------------------------------

/// Reads PS/2 mouse data directly from the i8042 ports.
///
/// The i8042 has two channels: keyboard (IRQ 1) and mouse/auxiliary (IRQ 12).
/// Relevant status-register (`0x64`) bits are 0 (output full), 1 (input full),
/// and 5 (output came from the auxiliary channel).
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
struct Ps2Mouse {
    /// Partial packet accumulator; PS/2 mouse sends 3-byte packets.
    buf:       [u8; 3],
    buf_idx:   usize,
    /// Set to false if init fails (e.g. i8042 not present).
    pub ok:    bool,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
impl Ps2Mouse {
    const DATA:   u16 = 0x60;
    const STATUS: u16 = 0x64;

    unsafe fn inb(port: u16) -> u8 {
        let v: u8;
        core::arch::asm!("in al, dx", in("dx") port, out("al") v, options(nostack, nomem));
        v
    }
    unsafe fn outb(port: u16, v: u8) {
        core::arch::asm!("out dx, al", in("dx") port, in("al") v, options(nostack, nomem));
    }

    // Wait until i8042 input buffer is empty (safe to write a command).
    unsafe fn wait_wr() {
        for _ in 0u32..100_000 {
            if Self::inb(Self::STATUS) & 0x02 == 0 { return; }
        }
    }

    // Wait until i8042 output buffer has data (safe to read).
    unsafe fn wait_rd() {
        for _ in 0u32..100_000 {
            if Self::inb(Self::STATUS) & 0x01 != 0 { return; }
        }
    }

    // Send one byte to the PS/2 mouse auxiliary channel.
    unsafe fn send_mouse(byte: u8) {
        Self::wait_wr(); Self::outb(Self::STATUS, 0xD4); // route next byte to mouse
        Self::wait_wr(); Self::outb(Self::DATA,   byte);
    }

    pub fn new() -> Self {
        let ok = unsafe { Self::init_hw() };
        Self { buf: [0; 3], buf_idx: 0, ok }
    }

    unsafe fn init_hw() -> bool {
        // Flush any stale data.
        for _ in 0..16 {
            if Self::inb(Self::STATUS) & 0x01 == 0 { break; }
            let _ = Self::inb(Self::DATA);
        }

        // Enable auxiliary (mouse) port.
        Self::wait_wr();
        Self::outb(Self::STATUS, 0xA8);

        // Enable mouse data reporting (command 0xF4 → mouse should ACK with 0xFA).
        Self::send_mouse(0xF4);
        Self::wait_rd();
        let ack = Self::inb(Self::DATA);
        ack == 0xFA
    }

    /// Drains complete three-byte PS/2 packets as `(dx, dy, button_mask)`.
    ///
    /// Overflowed packets are discarded and Y is converted to screen direction.
    pub fn poll(&mut self) -> alloc::vec::Vec<(i32, i32, u8)> {
        let mut out = alloc::vec::Vec::new();
        unsafe {
            loop {
                let st = Self::inb(Self::STATUS);
                // Require both OBF (bit 0) and AUX (bit 5).
                if st & 0x21 != 0x21 { break; }
                let byte = Self::inb(Self::DATA);

                // Sync: the first byte of every packet always has bit 3 = 1.
                if self.buf_idx == 0 && byte & 0x08 == 0 { continue; }

                self.buf[self.buf_idx] = byte;
                self.buf_idx += 1;

                if self.buf_idx == 3 {
                    self.buf_idx = 0;
                    let flags = self.buf[0];
                    let rx    = self.buf[1];
                    let ry    = self.buf[2];
                    // Overflow bits set → discard (spurious data during init).
                    if flags & 0xC0 != 0 { continue; }
                    let dx = if flags & 0x10 != 0 { rx as i32 - 256 } else { rx as i32 };
                    // PS/2 Y is inverted relative to screen coordinates.
                    let dy = if flags & 0x20 != 0 { ry as i32 - 256 } else { ry as i32 };
                    out.push((dx, -dy, flags & 0x07));
                }
            }
        }
        out
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
struct Ps2Mouse {
    pub ok: bool,
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
impl Ps2Mouse {
    fn new() -> Self {
        Self { ok: false }
    }

    fn poll(&mut self) -> Vec<(i32, i32, u8)> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Public event type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
/// A normalized keyboard or pointer event.
pub enum InputEvent {
    /// A UEFI keyboard event.
    Key(Key),
    /// Relative movement from EFI Simple Pointer or direct PS/2.
    MouseMove  {
        /// Horizontal delta; positive values move right.
        dx: i64,
        /// Vertical delta; positive values move down.
        dy: i64,
    },
    /// Absolute cursor position scaled to screen pixels.
    MouseAbs   {
        /// Horizontal screen coordinate.
        x: i32,
        /// Vertical screen coordinate.
        y: i32,
    },
    /// Left-button state transition (`true` means pressed).
    LeftButton (bool),
    /// Right-button state transition (`true` means pressed).
    ///
    /// [`crate::wm::WindowManager`] currently ignores this variant.
    RightButton(bool),
}

// ---------------------------------------------------------------------------
// InputDriver
// ---------------------------------------------------------------------------

/// Owns keyboard and pointer protocol handles for the application lifetime.
pub struct InputDriver {
    kbd:        boot::ScopedProtocol<Input>,
    abs:        Option<boot::ScopedProtocol<AbsolutePointer>>,
    rel:        Option<boot::ScopedProtocol<Pointer>>,
    ps2:        Ps2Mouse,

    last_left:  bool,
    last_right: bool,
    sw:         u32,
    sh:         u32,

    /// Whether any EFI pointer protocol opened or direct PS/2 initialized.
    pub ptr_found: bool,
    /// Whether an accepted Absolute Pointer handle was opened.
    ///
    /// This does not indicate exclusive or most-recent pointer use.
    pub use_abs:   bool,
    /// Whether EFI Simple Pointer was opened.
    pub rel_found: bool,

    // Cached coordinate range from AbsolutePointerMode (static after init).
    abs_rx: u64,
    abs_ry: u64,
    abs_min_x: u64,
    abs_min_y: u64,

    // Cached UEFI wait events (built once at init, cloned each wait_events call).
    kbd_wait: Option<Event>,
    abs_wait: Option<Event>,
    rel_wait: Option<Event>,

    // ---- diagnostics (updated every read_ptr call) ----
    /// Number of successful Absolute Pointer reads, with wrapping addition.
    pub abs_reads: u32,
    /// Absolute Pointer X range captured during construction.
    pub abs_rx_init: u64,
    /// X range copied after the most recent successful absolute read.
    pub mode_rx:   u64,
    /// Y range copied after the most recent successful absolute read.
    pub mode_ry:   u64,
    /// Raw X coordinate from the most recent successful absolute read.
    pub raw_x:     u64,
    /// Raw Y coordinate from the most recent successful absolute read.
    pub raw_y:     u64,
}

impl InputDriver {
    /// Opens the keyboard and all available pointer paths.
    ///
    /// # Panics
    ///
    /// Panics when EFI Simple Text Input cannot be located or opened
    /// exclusively. Pointer discovery failures are tolerated.
    pub fn new(sw: u32, sh: u32) -> Self {
        let kbd_h = boot::get_handle_for_protocol::<Input>()
            .expect("no keyboard handle");
        let kbd = boot::open_protocol_exclusive::<Input>(kbd_h)
            .expect("cannot open keyboard");

        // Prefer any live absolute pointer published by the firmware.
        //
        // OVMF registers a ConSplitter stub AbsolutePointer with mode range = 0
        // before any USB device is enumerated.  get_handle_for_protocol returns
        // that dummy first.  We must iterate ALL handles and pick the first one
        // whose coordinate range is non-zero (i.e. a real device is behind it).
        let abs = Self::find_live_abs();

        // Also open SimplePointer (PS/2 / usb-mouse) alongside AbsolutePointer.
        // Both are tried every frame so whichever QEMU routes events to works.
        let rel = boot::get_handle_for_protocol::<Pointer>()
            .and_then(|h| unsafe {
                boot::open_protocol::<Pointer>(
                    OpenProtocolParams {
                        handle:     h,
                        agent:      boot::image_handle(),
                        controller: None,
                    },
                    OpenProtocolAttributes::GetProtocol,
                )
            })
            .ok();

        let ptr_found = abs.is_some() || rel.is_some();
        let use_abs   = abs.is_some();

        // Cache mode range once (static hardware property).
        let (abs_rx, abs_ry, abs_min_x, abs_min_y) = abs.as_ref().map(|a| {
            let m = a.mode();
            (
                m.absolute_max_x.saturating_sub(m.absolute_min_x),
                m.absolute_max_y.saturating_sub(m.absolute_min_y),
                m.absolute_min_x,
                m.absolute_min_y,
            )
        }).unwrap_or((0, 0, 0, 0));

        // Cache wait events — cloning per-frame is cheaper than re-deriving.
        let kbd_wait = kbd.wait_for_key_event().map(|e| unsafe { e.unsafe_clone() });
        let abs_wait = abs.as_ref().and_then(|a| a.wait_for_input_event())
            .map(|e| unsafe { e.unsafe_clone() });
        let rel_wait = rel.as_ref().and_then(|r| r.wait_for_input_event())
            .map(|e| unsafe { e.unsafe_clone() });

        let rel_found = rel.is_some();
        let abs_rx_init = abs_rx;

        // Direct PS/2 mouse — fallback when firmware has no mouse drivers.
        let ps2 = Ps2Mouse::new();
        let ps2_found = ps2.ok;

        let ptr_found = ptr_found || ps2_found;

        Self {
            kbd, abs, rel, ps2,
            last_left:  false,
            last_right: false,
            sw, sh,
            ptr_found,
            use_abs,
            rel_found,
            abs_rx, abs_ry, abs_min_x, abs_min_y,
            kbd_wait, abs_wait, rel_wait,
            abs_reads: 0,
            abs_rx_init,
            mode_rx:   0,
            mode_ry:   0,
            raw_x:     0,
            raw_y:     0,
        }
    }

    // Scan all Absolute Pointer handles and take the first with nonzero X/Y
    // ranges. Z is an optional axis and is not a reliable device discriminator:
    // valid pointer drivers may advertise it even though this crate uses only X/Y.
    fn find_live_abs() -> Option<boot::ScopedProtocol<AbsolutePointer>> {
        use uefi::boot::SearchType;
        use uefi_raw::protocol::console::AbsolutePointerProtocol;

        let handles = boot::locate_handle_buffer(
            SearchType::ByProtocol(&AbsolutePointerProtocol::GUID),
        )
        .ok()?;

        for &h in handles.iter() {
            let Ok(proto) = (unsafe {
                boot::open_protocol::<AbsolutePointer>(
                    OpenProtocolParams {
                        handle:     h,
                        agent:      boot::image_handle(),
                        controller: None,
                    },
                    OpenProtocolAttributes::GetProtocol,
                )
            }) else {
                continue;
            };
            let mode = proto.mode();
            let rx = mode.absolute_max_x.saturating_sub(mode.absolute_min_x);
            let ry = mode.absolute_max_y.saturating_sub(mode.absolute_min_y);
            // Skip zero-range ConSplitter stubs, but accept optional Z axes.
            if rx > 0 && ry > 0 {
                return Some(proto);
            }
        }
        None
    }

    /// Returns cached UEFI wait-event clones in keyboard, absolute, relative order.
    ///
    /// Unavailable events are omitted. The standard window-manager loop assumes
    /// the normally present keyboard event occupies slot 0 and appends its frame
    /// timer after this list.
    pub fn wait_events(&self) -> Vec<Event> {
        let mut v = Vec::with_capacity(3);
        if let Some(ref e) = self.kbd_wait {
            v.push(unsafe { e.unsafe_clone() });
        }
        if let Some(ref e) = self.abs_wait {
            v.push(unsafe { e.unsafe_clone() });
        }
        if let Some(ref e) = self.rel_wait {
            v.push(unsafe { e.unsafe_clone() });
        }
        v
    }

    /// Drain all pending keyboard events.
    pub fn read_keys(&mut self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        loop {
            match self.kbd.read_key() {
                Ok(Some(k)) => out.push(InputEvent::Key(k)),
                _           => break,
            }
        }
        out
    }

    /// Polls every available pointer source and returns normalized changes.
    ///
    /// Sources are checked in Absolute Pointer, Simple Pointer, direct PS/2
    /// order. The PS/2 byte stream is drained; each EFI protocol is read once.
    /// A single result may contain movement from more than one source.
    pub fn read_ptr(&mut self) -> Vec<InputEvent> {
        let mut out = Vec::new();

        // Prefer AbsolutePointer (USB tablet); also check SimplePointer so that
        // PS/2 events work when QEMU routes VNC to the PS/2 mouse instead.
        // push_buttons uses shared last_left/last_right, so button edges from
        // both sources are naturally deduplicated.
        if let Some(abs) = &mut self.abs {
            if let Some(state) = abs.read_state() {
                self.abs_reads = self.abs_reads.wrapping_add(1);
                self.raw_x    = state.current_x;
                self.raw_y    = state.current_y;
                self.mode_rx  = self.abs_rx;
                self.mode_ry  = self.abs_ry;

                if self.abs_rx > 0 && self.abs_ry > 0 {
                    let x = (state.current_x.saturating_sub(self.abs_min_x)
                        * self.sw as u64 / self.abs_rx) as i32;
                    let y = (state.current_y.saturating_sub(self.abs_min_y)
                        * self.sh as u64 / self.abs_ry) as i32;
                    out.push(InputEvent::MouseAbs { x, y });
                }
                self.push_buttons(&mut out,
                    state.active_buttons & 0x01 != 0,
                    state.active_buttons & 0x02 != 0);
            }
        }

        if let Some(rel) = &mut self.rel {
            if let Ok(Some(state)) = rel.read_state() {
                let dx = state.relative_movement[0] as i64;
                let dy = state.relative_movement[1] as i64;
                if dx != 0 || dy != 0 {
                    out.push(InputEvent::MouseMove { dx, dy });
                }
                self.push_buttons(&mut out, state.button[0], state.button[1]);
            }
        }

        // Direct PS/2 fallback: works even when OVMF has no mouse drivers.
        if self.ps2.ok {
            for (dx, dy, btn) in self.ps2.poll() {
                if dx != 0 || dy != 0 {
                    out.push(InputEvent::MouseMove { dx: dx as i64, dy: dy as i64 });
                }
                self.push_buttons(&mut out, btn & 0x01 != 0, btn & 0x02 != 0);
            }
        }

        out
    }

    fn push_buttons(&mut self, out: &mut Vec<InputEvent>, left: bool, right: bool) {
        if left != self.last_left {
            self.last_left = left;
            out.push(InputEvent::LeftButton(left));
        }
        if right != self.last_right {
            self.last_right = right;
            out.push(InputEvent::RightButton(right));
        }
    }
}
