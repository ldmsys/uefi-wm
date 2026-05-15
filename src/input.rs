//! Input drivers: keyboard via EFI_SIMPLE_TEXT_INPUT, mouse via i8042 PS/2.
//!
//! Mouse strategy
//! --------------
//! OVMF (the Ubuntu package) ships without Ps2MouseDxe or
//! UsbMouseAbsolutePointerDxe, so EFI_SIMPLE_POINTER_PROTOCOL and
//! EFI_ABSOLUTE_POINTER_PROTOCOL never receive hardware events.  We fall
//! back to direct PS/2 port I/O:
//!
//!   - Port 0x64: i8042 status/command register.
//!   - Port 0x60: i8042 data register.
//!
//! The q35 machine always has an i8042 with PS/2 mouse (#2 in QEMU's mouse
//! list).  Because our keyboard is on xHCI USB, i8042 is unused by OVMF and
//! safe for us to own.  We initialize the PS/2 mouse (enable reporting) and
//! poll the data port each timer tick.
//!
//! We still try EFI_ABSOLUTE_POINTER_PROTOCOL first (for real USB tablets on
//! physical hardware) and fall through to PS/2 only when that yields nothing.

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
pub struct AbsolutePointer(AbsolutePointerProtocol);

impl AbsolutePointer {
    pub fn read_state(&mut self) -> Option<AbsolutePointerState> {
        let mut s = AbsolutePointerState::default();
        let ok = unsafe { (self.0.get_state)(&self.0, &mut s) };
        if ok == Status::SUCCESS { Some(s) } else { None }
    }

    pub fn mode(&self) -> &AbsolutePointerMode {
        unsafe { &*self.0.mode }
    }

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
/// Status register (0x64) bits:
///   0  OBF  – output-buffer full; data ready to read from 0x60
///   1  IBF  – input-buffer full; do not write until clear
///   5  AUX  – OBF data came from mouse channel (not keyboard)
struct Ps2Mouse {
    /// Partial packet accumulator; PS/2 mouse sends 3-byte packets.
    buf:       [u8; 3],
    buf_idx:   usize,
    /// Set to false if init fails (e.g. i8042 not present).
    pub ok:    bool,
}

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

    /// Drain all available PS/2 mouse bytes; collect complete 3-byte packets.
    /// Returns a list of (dx, dy, btn_mask) tuples (one per complete packet).
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

// ---------------------------------------------------------------------------
// Public event type
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum InputEvent {
    Key(Key),
    /// Relative movement (SimplePointerProtocol / usb-mouse).
    MouseMove  { dx: i64, dy: i64 },
    /// Absolute cursor position in screen pixels (AbsolutePointer / usb-tablet).
    MouseAbs   { x: i32, y: i32 },
    LeftButton (bool),
    RightButton(bool),
}

// ---------------------------------------------------------------------------
// InputDriver
// ---------------------------------------------------------------------------

/// Holds open protocol handles for the lifetime of the application.
pub struct InputDriver {
    kbd:        boot::ScopedProtocol<Input>,
    abs:        Option<boot::ScopedProtocol<AbsolutePointer>>,
    rel:        Option<boot::ScopedProtocol<Pointer>>,
    ps2:        Ps2Mouse,

    last_left:  bool,
    last_right: bool,
    sw:         u32,
    sh:         u32,

    pub ptr_found: bool,
    /// true = using AbsolutePointer (tablet); false = SimplePointer (mouse)
    pub use_abs:   bool,
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
    pub abs_reads: u32,
    pub abs_rx_init: u64,  // initial abs range at init time (shows which device was found)
    pub mode_rx:   u64,
    pub mode_ry:   u64,
    pub raw_x:     u64,
    pub raw_y:     u64,
}

impl InputDriver {
    pub fn new(sw: u32, sh: u32) -> Self {
        let kbd_h = boot::get_handle_for_protocol::<Input>()
            .expect("no keyboard handle");
        let kbd = boot::open_protocol_exclusive::<Input>(kbd_h)
            .expect("cannot open keyboard");

        // Prefer absolute pointer (usb-tablet, no QEMU mouse-grab required).
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

        // Direct PS/2 mouse — fallback when OVMF has no mouse drivers.
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

    /// Scan all EFI_ABSOLUTE_POINTER_PROTOCOL handles and return the first one
    /// whose mode reports a non-zero coordinate range (i.e. a real device).
    /// Scan all EFI_ABSOLUTE_POINTER_PROTOCOL handles and return the first real
    /// pointing device.  Two spurious handles must be skipped:
    ///   - ConSplitter stub: reports rx = 0 before any real device is attached.
    ///   - VMMouse (OVMF VmmouseDxe on ISA bus): reports a non-zero Z-axis range
    ///     and requires VMware guest initialisation that never happens in UEFI.
    ///     If we accidentally use it every GetState returns EFI_NOT_READY forever.
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
            let rz = mode.absolute_max_z.saturating_sub(mode.absolute_min_z);
            // Skip ConSplitter stub (rx = 0) and VMMouse (rz > 0, ISA-based).
            if rx > 0 && ry > 0 && rz == 0 {
                return Some(proto);
            }
        }
        None
    }

    /// UEFI wait events for `boot::wait_for_event`.
    /// Slot 0: keyboard; slot 1+: abs/rel pointer (whichever were found).
    /// Caller appends the frame timer before calling wait_for_event.
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

    /// Read current pointer state and return any changes as events.
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
