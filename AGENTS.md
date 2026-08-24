# Repository guide

The Rust source and Cargo/shell configuration are authoritative. Keep this file,
`README.md`, and rustdoc synchronized with implementation changes.

## Workspace

- Target: `x86_64-unknown-uefi`, `#![no_std]`, nightly Rust.
- Root package: `uefi-wm`; Rust crate name `uefi_wm`.
- Workspace demo package: `demo/`; package `uefi-gui-demo`, binary `uefi-gui`.
- `.cargo/config.toml` selects the target and builds `core`,
  `compiler_builtins`, and `alloc`.
- The demo binary enables UEFI allocator/panic/logger features. The library
  enables only `uefi/alloc`; duplicating binary runtime features in the library
  causes duplicate runtime symbols.

```text
Cargo.toml                 library package plus workspace configuration
src/lib.rs                 no_std library root
src/gfx.rs                 software framebuffer and drawing
src/input.rs               keyboard, EFI pointers, direct PS/2
src/wm.rs                  windows, widgets, callbacks, event loops
demo/Cargo.toml            non-publishable demo package
demo/src/main.rs           GOP setup and BIOS/UEFI setup demo
demo/fonts/                fonts embedded by the demo
demo/qemu.sh               release build, ISO generator, and QEMU launcher
```

## Commands

```bash
cargo check
cargo build
cargo build --release
cargo doc -p uefi-wm --no-deps

bash demo/qemu.sh
bash demo/qemu.sh --vnc
bash demo/qemu.sh --ps2
bash demo/qemu.sh --usb-tablet
bash demo/qemu.sh --iso
bash demo/qemu.sh --iso=out/uefi-gui.iso
UEFI_TARGET=aarch64-unknown-uefi bash demo/qemu.sh --iso
```

There are no automated tests. Boot the EFI binary for runtime validation.
`demo/qemu.sh` recognizes `--vnc`, `--ps2`, and `--usb-tablet` in either of its
first two arguments. Its default pointer is `virtio-tablet-pci`; tablet-capable
OVMF VirtioInput firmware exposes it through EFI Absolute Pointer. The
`--usb-tablet` mode pins the USB HID tablet to `xhci.0` and requires a firmware
USB tablet driver; QEMU does not itself publish UEFI protocols. The `--ps2`
mode uses q35's built-in i8042 and must not add a duplicate `-device i8042`.
`--iso` and `--iso=PATH` build a UEFI El Torito ISO and exit without locating
OVMF or launching QEMU. `ISO_OUT` provides another output-path override.
`UEFI_TARGET` accepts `x86_64-unknown-uefi` or `aarch64-unknown-uefi` and maps
them to `BOOTX64.EFI` or `BOOTAA64.EFI`. ISO creation requires `xorriso`,
`mkfs.vfat`, `mmd`, `mcopy`,
and `truncate`. QEMU launch mode rejects non-x86_64 targets.
VNC listens only on `127.0.0.1:5901`; remote access requires an explicit tunnel.
It searches for `OVMF_CODE.fd` plus optional `OVMF_VARS.fd`, then for
single-file `OVMF.fd`; it does not search for the `*_4M.fd` names.

## Demo startup

`demo/src/main.rs` initializes UEFI helpers, exclusively opens GOP, and captures
the dimensions, pixel format, and pixel stride while keeping the scoped
protocol open. It then constructs:

1. `Framebuffer`
2. `WindowManager` with caller-owned TTF bytes
3. `InputDriver`

It builds three demo windows and calls `WindowManager::run` with
`gop.frame_buffer()`. `run` consumes that guard, preserving its borrow of the
scoped GOP protocol throughout the loop.

The demo starts with the default Light theme. Its OS Settings group contains
a Theme combo box whose change callback selects Classic, Light, or Dark with
`WindowManager::set_theme`. The final, topmost demo window contains a 148 px QR
Code linked to the `uefi-wm` GitHub project and is positioned near the lower
right of the detected display with a 20 px minimum inset.

## Event loop

`WindowManager::run` creates a periodic 166,670 × 100 ns timer. Each iteration:

```text
wait_events() + frame timer
    index 0 -> read_keys()
    any other index or wait error -> read_ptr()
handle(events, framebuffer, input, GOP framebuffer, stride)
render(framebuffer)
present_to(GOP framebuffer, stride)
```

`wait_events` pushes cached event clones in keyboard, AbsolutePointer,
SimplePointer order, omitting any unavailable event. The constructor requires a
keyboard protocol, and normal firmware supplies its wait event, so keyboard is
normally slot 0. Custom loops must preserve the same dispatch assumption.

`handle` returns `true` for Escape, for `q`/`Q` when no widget is focused, or
when a callback sets `EventCtx::quit`. Hiding the final window via its close
button does not terminate the loop.

Right-button transitions are produced by `InputDriver` but ignored by
`WindowManager::handle`.

## Input implementation

`InputDriver::new` exclusively opens EFI Simple Text Input. It also attempts to
open AbsolutePointer and SimplePointer with `GetProtocol`. On x86/x86_64 it
also initializes a direct i8042 PS/2 reader; other architectures compile a
disabled no-op PS/2 backend.

`find_live_abs` scans all AbsolutePointer handles and accepts the first with
non-zero X/Y ranges. This rejects zero-range ConSplitter stubs. An optional Z
axis is not used as a discriminator because valid drivers may advertise one;
cursor positioning ignores Z.

`read_ptr` does not select a single active source. Every call tries, in order:

1. AbsolutePointer
2. SimplePointer
3. direct PS/2, when initialization succeeded

Each EFI source is read once and PS/2 is drained. A batch may contain motion
from several sources. All sources update
one cached left/right state pair, so edges are relative to the preceding source
observation. Absolute coordinates are scaled from their advertised
minimum/range to screen dimensions; final cursor clamping happens in
`WindowManager::handle`.

On x86/x86_64, the direct PS/2 path owns ports `0x60` and `0x64`, expects the
`0xFA` response to enable-reporting command `0xF4`, drains three-byte packets,
ignores overflowed packets, and reverses the packet Y delta. Failed
initialization disables polling. Other architectures use a disabled no-op
backend and rely on EFI pointer protocols.

The public diagnostics mean:

- `ptr_found`: at least one EFI pointer protocol or the direct PS/2 path exists.
- `use_abs`: an accepted AbsolutePointer handle was opened; this does not mean
  it was the only or most recent source.
- `rel_found`: SimplePointer was opened.
- `abs_reads`: successful AbsolutePointer reads, wrapping at `u32::MAX`.
- `abs_rx_init`: accepted device's X range at construction.
- `mode_rx` / `mode_ry`: cached ranges copied after a successful absolute read.
- `raw_x` / `raw_y`: coordinates from the most recent successful absolute read.

These fields are not currently rendered by the window manager; there is no
diagnostic taskbar in the implementation.

## Framebuffer

`Framebuffer::new` returns `Result`, using checked dimension multiplication and
fallible reservation. Its vector and dimensions are private, preserving a
tightly packed `width * height` invariant. `width`, `height`, `pixels`, and
`pixels_mut` provide access without permitting vector resizing. Drawing methods
write packed GOP pixels. `fill` clips signed rectangles; `set` and `blend_pixel`
ignore out-of-bounds coordinates. `present_to` validates the GOP stride and
framebuffer size, then writes `width` pixels per row through UEFI's volatile
framebuffer API. The destination stride is measured in pixels.

Pixel packing is:

- `PixelFormat::Rgb`: `(B << 16) | (G << 8) | R`
- every other variant: `(R << 16) | (G << 8) | B`

Do not describe `Bitmask` or `BltOnly` as supported specially: they follow the
non-`Rgb` branch.

## Windows, layout, and rendering

`Theme` provides Classic, Light, and Dark renderers. `WindowManager::new` and
`Theme::default` use Light; `new_with_theme` selects the initial theme, while
`theme` and `set_theme` query or change it at runtime. Theme changes preserve
window and widget state and apply to subsequent full renders and modal message
boxes.
Classic retains 22 px title bars, compact controls, beveled borders, centered
titles, classic caption glyphs, and the original cursor. Light and Dark use 32
px title bars, 1 px window borders, 12 px client padding, taller controls, flat
borders, left-aligned titles, modern caption glyphs, a compact cursor, and
pointer-hover feedback. Dark uses dark-grey active and inactive title bars,
with blue retained as the widget accent. Public `Framebuffer::border_raised`
and `border_sunken` primitives retain classic colors, while their `_with`
variants accept explicit colors.

Windows live in a `Vec`; the last entry is topmost/focused. Clicking a body
removes and pushes that window to raise it. A close-button press sets `vis =
false`; minimize toggles `minimized`; title dragging can move windows partly off
screen. Rendering redraws the full desktop, visible windows, widgets, open combo
drop-downs, then cursor.

Classic widget positions are relative to the client origin:

```text
x = window.x + BORDER + PAD              = window.x + 11
y = window.y + BORDER + TITLE_H + 1 + PAD = window.y + 34
```

The usable client width is `window.w - 2 * (BORDER + PAD)`. Standalone labels
use `rel_y + ascent` as their baseline. The 22 px controls center label text at
`rel_y + ascent + (22 - ascent) / 2`; align adjacent standalone labels by adding
that same offset (about 4 px for the demo font).

Light and Dark use client origin `(window.x + 13, window.y + 46)`, derived from
a 1 px border, 32 px title bar, and 12 px padding. Their text boxes, combo boxes,
and numeric controls are 26 px high; buttons are 30 px; check boxes are 16 px.
Hit testing uses the same active theme metrics as rendering.

## Widget contracts

There are 14 widget types and corresponding typed handles: Label, TextBox,
TextArea, CheckBox, RadioButton, ComboBox, ListBox, Button, ProgressBar, Slider,
NumericUpDown, GroupBox, Separator, QR Code.

- Window titles and label widget text are copied into `String`.
- Control labels, combo options, and list items are stored as `&'static str`.
- The first radio button added to each `(window ID, group ID)` is selected.
- Combo boxes start at selected index 0, even with no options.
- List boxes start with no selection.
- Progress bars start at 0/100.
- Slider and numeric ranges require `min <= max`; initial/set values clamp.
- Numeric steps clamp to at least 1.
- QR Codes own their UTF-8 payload and are decorative. The internal,
  dependency-free byte-mode encoder automatically selects versions 1 through
  10 at Low error correction, so capacity is 271 UTF-8 bytes. Rendering uses a
  four-module white quiet zone, integral module scaling, and centering within
  the requested square. Construction fails with `DataTooLong` or `SizeTooSmall`
  rather than storing an unreadable symbol.

Handles wrap the global widget-vector index. Typed accessors still check the
runtime variant: stale/wrong handles produce neutral getter values or no-op
setters. Removing widgets is not implemented, so valid handles remain stable.

`widget_set_enabled` affects only interactive widgets; decorative widgets are a
no-op. Disabled widgets are skipped by hit testing and focus cycling, but the
method does not clear current focus and several keyboard-routing branches do not
re-check `enabled`. `widget_set_visible` covers all widget variants. Read-only
text controls remain normally styled and focusable; a read-only text area can
still scroll.

## Callback model

The internal callback type is:

```rust
Box<dyn for<'ctx, 'gop> FnMut(&mut WindowManager, &mut EventCtx<'ctx, 'gop>)>
```

- `set_on_click` accepts any `Into<WidgetId>` and is available for all 14 widget
  types. It fires after a left press and release over the same widget. Disabled
  interactive widgets are not clickable; decorative widgets remain clickable
  because `widget_set_enabled` does not apply to them.
- `set_on_change` accepts any `Into<WidgetId>`, but installs callbacks only for
  TextBox, TextArea, CheckBox, RadioButton, ComboBox, ListBox, Slider, and
  NumericUpDown. Other variants are silently ignored.
- Programmatic state setters never fire callbacks. Input generally reports only
  value changes, except radio selection reports the selected widget even when it
  was already selected, and a successfully parsed numeric commit reports even
  when clamping leaves the value unchanged.
- `event_source` is assigned before callback lookup. It retains the most recent
  triggering widget after callback completion; it is not reset to `None`.
- Callbacks are temporarily taken out of their widget, called, then restored.
  A callback cannot replace its own stored callback permanently during that
  invocation because restoration overwrites the slot afterward.

`EventCtx` contains `quit`, the mutable back-buffer, input driver, borrowed GOP
framebuffer guard, and stride. Only `quit` and `fb` are public. `present`
immediately blits the back-buffer; the next normal render overwrites custom
pixels.

## State setter details

- `set_textbox_text`: replaces text and moves cursor to the end.
- `set_textarea_text`: replaces text and resets cursor/scroll to zero.
- `append_textarea_text`: appends, moves cursor to end, scrolls to the bottom.
- `set_textarea_scroll`: clamps only to the final logical line, not to the last
  full viewport position.
- `set_combobox_selected`: clamps to `len - 1`, with empty lists yielding 0.
- `set_listbox_items`: replaces items, clears selection, resets scroll.
- `set_progressbar`: stores `min(value, max)` and stores `max` unchanged.
- `set_numeric_step`: stores `max(step, 1)`.
- `set_qrcode_data`: encodes before mutation and preserves the old payload and
  symbol when the new content is too long or does not fit the original square.

## Modal dialog

`WindowManager::message_box` requires callback-owned `EventCtx` and static title
and body strings. It snapshots `render_base` without the normal cursor, then
busy-polls `read_keys` and `read_ptr`, redraws a fixed 360 × 140 dialog, draws the
cursor, and presents continuously. Mouse selection occurs on left-button press;
Enter chooses OK/Yes; Escape returns `MsgBoxResult::No` for both button layouts.
The body is drawn on one line without wrapping.

## Documentation maintenance

When changing behavior, update all three documentation layers:

1. public rustdoc at the defining item/module;
2. `README.md` for users;
3. this file for repository-specific implementation guidance.

Prefer `ignore` for rustdoc fragments that depend on UEFI runtime state. Run
`cargo doc -p uefi-wm --no-deps` to catch broken intra-doc links, then
`cargo check`.
