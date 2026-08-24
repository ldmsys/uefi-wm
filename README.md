# uefi-wm

`uefi-wm` is a `#![no_std]` GUI toolkit for `x86_64-unknown-uefi`. It renders a
themed floating desktop into a software back-buffer and presents it through the
UEFI Graphics Output Protocol (GOP).

The repository root is both the publishable library package and a Cargo
workspace containing the demo:

- workspace root: reusable `uefi-wm` library crate (`uefi_wm`)
- `demo/`: the non-publishable `uefi-gui` BIOS/UEFI setup demo

The demo is excluded from the library's crates.io package, so demo-only source,
fonts, and runtime artifacts do not affect published crate contents or versioning.

The library owns no font data. Applications provide TTF bytes when constructing
the window manager.

## Requirements

The repository pins nightly Rust and configures Cargo to build the UEFI target,
including `core`, `compiler_builtins`, and `alloc`:

```bash
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
rustup target add x86_64-unknown-uefi --toolchain nightly
sudo apt install qemu-system-x86 ovmf
# Optional ISO export: sudo apt install xorriso dosfstools mtools
```

## Build and run

```bash
cargo check
cargo build
cargo build --release
cargo doc -p uefi-wm --no-deps

bash demo/qemu.sh          # GTK display
bash demo/qemu.sh --vnc    # loopback-only VNC display :1, TCP port 5901
bash demo/qemu.sh --ps2    # use q35's built-in i8042 instead of a tablet
bash demo/qemu.sh --usb-tablet # test a USB tablet with compatible firmware
bash demo/qemu.sh --iso    # create demo/uefi-gui.iso without launching QEMU
bash demo/qemu.sh --iso=out/uefi-gui.iso # choose the ISO output path
UEFI_TARGET=aarch64-unknown-uefi bash demo/qemu.sh --iso
```

VNC binds to `127.0.0.1`; use an authenticated tunnel if access from another
machine is required.

The binaries are written to:

- debug: `target/x86_64-unknown-uefi/debug/uefi-gui.efi`
- release: `target/x86_64-unknown-uefi/release/uefi-gui.efi`

`demo/qemu.sh` builds the release binary, copies it to
`demo/esp/EFI/BOOT/BOOTX64.EFI`, finds either `OVMF_CODE.fd` (with
`OVMF_VARS.fd`) or the single-file `OVMF.fd`, and launches a q35 VM. It uses a
USB keyboard on xHCI and, by default, a virtio tablet. OVMF builds with
tablet-capable `VirtioInputDxe` expose that device through EFI Absolute Pointer.
`--ps2` instead uses q35's built-in i8042 (which remains present under
`-nodefaults`); it does not add a duplicate controller. `--usb-tablet` attaches
QEMU's USB HID tablet explicitly to xHCI. A USB tablet becomes usable only when
the firmware contains a matching HID tablet driver; QEMU's absolute host
coordinates alone do not create an EFI Absolute Pointer protocol.

`--iso` stops after producing a UEFI-bootable ISO. The ISO contains a FAT El
Torito EFI system image plus a loose executable in the ISO filesystem. Set
`UEFI_TARGET` to `x86_64-unknown-uefi` (the default) or
`aarch64-unknown-uefi`; the generator selects `BOOTX64.EFI` or
`BOOTAA64.EFI` respectively. Set `ISO_OUT` or use `--iso=PATH` to choose the
output location. ISO generation uses `xorriso` as the ISO authoring tool,
`mkfs.vfat` from `dosfstools`, and the `mtools` commands `mmd` and `mcopy`; it
does not require QEMU or OVMF. QEMU launch mode remains x86_64-only.

There are no tests in the workspace. Runtime behavior must be verified by
booting the EFI image; `cargo check` and `cargo doc` cover compilation and
documentation links.

## Using the library

Keep UEFI's allocator, panic-handler, and logger features in the final binary.
The library itself enables only `uefi/alloc` to avoid duplicate runtime symbols.

```toml
[dependencies]
uefi-wm = "0.1"
uefi = { version = "0.33", features = [
    "alloc", "global_allocator", "panic_handler", "logger"
] }
```

A typical application keeps GOP open, captures its mode properties, builds the
UI, and passes the GOP framebuffer guard into the library-owned event loop:

```rust,ignore
use uefi::boot;
use uefi::proto::console::gop::GraphicsOutput;
use uefi_wm::gfx::Framebuffer;
use uefi_wm::input::InputDriver;
use uefi_wm::wm::WindowManager;

static FONT: &[u8] = include_bytes!("../fonts/MyFont.ttf");

let handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle).unwrap();
let info = gop.current_mode_info();
let (mode_width, mode_height) = info.resolution();
let (width, height) = (mode_width as u32, mode_height as u32);
let stride = info.stride();
let format = info.pixel_format();

let mut fb = Framebuffer::new(width, height, format).unwrap();
let mut wm = WindowManager::new(width, height, FONT, 16.0);
let mut input = InputDriver::new(width, height);

let window = wm.open("Settings", 100, 80, 500, 300);
let slider = wm.add_slider(window, 0, 0, 220, "Volume: ", 0, 100, 50);
let progress = wm.add_progressbar(window, 0, 32, 220, "");
wm.set_on_change(slider, alloc::boxed::Box::new(move |wm, _ctx| {
    wm.set_progressbar(progress, wm.slider_value(slider) as u32, 100);
}));

wm.run(&mut fb, &mut input, gop.frame_buffer(), stride);
```

`WindowManager::run` consumes the GOP framebuffer guard, which keeps its GOP
borrow valid for the loop. The stride is measured in pixels, not bytes.

## Themes

The window manager includes `Theme::Classic`, `Theme::Light`, and
`Theme::Dark`. `WindowManager::new` and `Theme::default` use Light. Pick
an initial theme with `new_with_theme`, or switch at runtime without rebuilding
the UI:

```rust,ignore
use uefi_wm::wm::{Theme, WindowManager};

let mut wm = WindowManager::new_with_theme(
    width, height, FONT, 16.0, Theme::Light,
);
wm.set_theme(Theme::Dark);
assert_eq!(wm.theme(), Theme::Dark);
```

Theme changes affect the desktop, window chrome, widget geometry, disabled
states, selection accents, modal message boxes, and pointer on the next render.
Light and Dark use a 32 px title bar, roomier client padding and controls, flat
borders, modern caption icons, and hover feedback for caption buttons and
interactive widgets. Dark uses a dark-grey title bar while retaining blue
selection and focus accents. Classic retains the original compact beveled
geometry and cursor. Switching themes does not change focus, widget values,
callbacks, or window z-order. The demo's OS Settings panel includes a Theme
combo box for live selection.

## Widgets and layout

Widget coordinates are relative to the client origin. Under Classic it is:

```text
(window.x + 11, window.y + 34)
```

That is `BORDER + PAD` horizontally and
`BORDER + TITLE_H + 1 + PAD` vertically. The client width is
`window.w - 2 * (BORDER + PAD)`. Light and Dark use a 1 px border, 32 px title
bar, and 12 px padding, making their client origin `(window.x + 13,
window.y + 46)`; control hit boxes expand with their modern heights.

| Constructor | Handle | Initial state / behavior |
|---|---|---|
| `add_label(win, x, y, width, text)` | `LabelId` | Owned text; decorative |
| `add_textbox(win, x, y, width, label)` | `TextBoxId` | Empty, editable, may receive initial focus |
| `add_textarea(win, x, y, width, height)` | `TextAreaId` | Empty, editable, vertically scrollable |
| `add_checkbox(win, x, y, label)` | `CheckBoxId` | Unchecked |
| `add_radiobutton(win, x, y, label, group)` | `RadioButtonId` | First button in each `(window, group)` is selected |
| `add_combobox(win, x, y, width, label, options)` | `ComboBoxId` | Index 0 selected, including for an empty options list |
| `add_listbox(win, x, y, width, height, items)` | `ListBoxId` | No selection |
| `add_button(win, x, y, width, label)` | `ButtonId` | Push button |
| `add_progressbar(win, x, y, width, label)` | `ProgressBarId` | Value 0, maximum 100; decorative |
| `add_slider(win, x, y, width, label, min, max, initial)` | `SliderId` | Initial value clamped to the range |
| `add_numeric_updown(win, x, y, width, label, min, max, initial)` | `NumericUpDownId` | Initial value clamped; step 1 |
| `add_groupbox(win, x, y, width, height, label)` | `GroupBoxId` | Decorative |
| `add_separator(win, x, y, width)` | `SeparatorId` | Decorative |
| `add_qrcode(win, x, y, size, data)` | `Result<QrCodeId, QrCodeError>` | Owned UTF-8 payload; decorative |

All label/option/item arguments except `add_label` text and window titles are
stored as `&'static str`. Slider and numeric ranges must satisfy `min <= max`.

QR Codes are encoded entirely inside `uefi-wm`; no QR or image dependency is
used. The widget selects versions 1 through 10 automatically, encodes the UTF-8
string in byte mode with Low error correction, and renders a standards-required
four-module white quiet zone. A payload may contain at most 271 UTF-8 bytes.
The pixel `size` must fit the selected symbol and quiet zone at a minimum of one
pixel per module; larger squares use the largest integral scale and center the
result. Construction reports `QrCodeError::DataTooLong` or
`QrCodeError::SizeTooSmall` instead of creating an unreadable symbol.

```rust,ignore
use uefi_wm::wm::QrCodeError;

let qr = wm.add_qrcode(window, 280, 0, 148, "https://example.com/setup")?;
assert_eq!(wm.qrcode_data(qr), "https://example.com/setup");
wm.set_qrcode_data(qr, "WIFI:T:WPA;S:Firmware;P:secret;;")?;
# Ok::<(), QrCodeError>(())
```

### QR encoder attribution

The dependency-free QR encoding implementation is adapted from
[Project Nayuki's QR Code generator](https://github.com/nayuki/QR-Code-generator),
copyright Project Nayuki and distributed under the MIT License. The complete
upstream copyright and permission notice is retained at the top of
[`src/qr.rs`](src/qr.rs).

## Callbacks and state

`set_on_click` accepts every typed widget handle and fires after a left-button
press and release over the same widget. This includes decorative labels,
progress bars, group boxes, separators, and QR Codes; disabled interactive
widgets do not receive clicks. `set_on_change` installs callbacks only for text
boxes, text areas, check boxes, radio buttons, combo boxes, list boxes, sliders,
and numeric up-down controls. Passing another `WidgetId` is a no-op.

Change callbacks run only when input handling reports a mutation or activation.
Most controls report only actual value changes, but selecting an already-selected
radio button and committing a successfully parsed numeric edit can report even
when the stored value is unchanged. State setter methods do not invoke callbacks.
`event_source` is assigned before callback lookup and retains the most recently
triggered widget; it is not reset after the callback. Inspect it inside the
callback when one callback body serves several widgets.

State getters return a neutral fallback for a stale or wrong typed handle, and
setters silently do nothing. Important setter details:

- `set_textbox_text` moves the cursor to the end.
- `set_textarea_text` moves the cursor and scroll position to the top.
- `append_textarea_text` moves the cursor to the end and scrolls to the bottom.
- `set_combobox_selected`, `set_slider_value`, and `set_numeric_value` clamp.
- `set_listbox_items` clears selection and resets scrolling.
- `set_progressbar` clamps `value` to `max`; `max == 0` is supported.
- `set_numeric_step` clamps the step to at least 1.
- `set_qrcode_data` re-encodes the owned UTF-8 payload. On an encoding or size
  error, it preserves the previous payload and symbol and returns the error.
- `widget_set_enabled` updates interactive widgets only. Disabled widgets are
  skipped by mouse hit testing and focus cycling, but disabling the currently
  focused widget does not clear focus and keyboard paths do not consistently
  re-check the enabled flag.
- `widget_set_visible` affects every widget type.
- read-only text controls remain focusable; text areas also remain scrollable.

`EventCtx::quit` requests loop termination. `EventCtx::fb` exposes the current
back-buffer, and `EventCtx::present` immediately blits it. A later
`WindowManager::render` redraws the entire scene.

## Event loop and exit behavior

`WindowManager::run` creates a periodic timer with a 166,670 × 100 ns period
(approximately 60 Hz). `InputDriver::wait_events` returns cached clones in this
order when present: keyboard, AbsolutePointer, SimplePointer. The timer is
appended last. Index 0 is treated as keyboard; every other index—including the
timer—causes all pointer sources to be polled.

`handle` returns `true` when Escape is pressed, when `q`/`Q` is pressed while no
widget is focused, or when a callback sets `ctx.quit`. Closing all windows does
not itself end the loop; close buttons only mark their windows invisible.

For a custom loop, reproduce the dispatch rule used by `run`:

```rust,ignore
let mut gop_fb = gop.frame_buffer();
let timer_slot = input.wait_events().len();
loop {
    let mut wait = input.wait_events();
    wait.push(unsafe { frame_timer.unsafe_clone() });
    let fired = uefi::boot::wait_for_event(&mut wait).unwrap_or(timer_slot);
    let events = if fired == 0 {
        input.read_keys()
    } else {
        input.read_ptr()
    };
    if wm.handle(&events, &mut fb, &mut input, &mut gop_fb, stride) {
        break;
    }
    wm.render(&mut fb);
    fb.present_to(&mut gop_fb, stride);
}
```

## Input behavior

`InputDriver::new` requires and exclusively opens EFI Simple Text Input. It
then probes three pointer paths:

1. the first usable EFI Absolute Pointer handle;
2. one EFI Simple Pointer handle;
3. direct i8042 PS/2 port I/O on x86/x86_64.

Absolute Pointer handles with a zero X/Y range are rejected. An optional Z axis
is accepted but ignored because cursor positioning uses only X and Y. On every
`read_ptr` call, all successfully initialized sources are polled in that order
(one read per EFI protocol, a full drain for PS/2). Movement events from multiple
sources may therefore appear in one batch. All sources update one pair of cached
button states, so an edge is relative to the preceding observation even when
that observation came from another source.

On x86/x86_64, the direct PS/2 driver accepts three-byte packets, discards
packets with either overflow bit, converts PS/2 Y to screen Y, and is disabled
if initialization does not return the `0xFA` acknowledgement. Other
architectures use a disabled no-op backend and retain the architecture-neutral
EFI pointer paths. The `RightButton` event is emitted but the window manager
currently ignores it.

## Framebuffer behavior

`Framebuffer::new` uses checked dimensions and fallible capacity reservation.
It stores one `u32` per visible pixel; dimensions and vector ownership remain
private so safe callers cannot invalidate that relationship. `pixels` and
`pixels_mut` expose fixed-length slices for direct access. Primitive writes clip
to the buffer. `present_to` writes each visible row through UEFI's volatile GOP
framebuffer API, allowing GOP stride to exceed the visible width. It validates
the stride and destination size before writing.

Encoding follows the implementation exactly:

| GOP format | Packed `u32` |
|---|---|
| `PixelFormat::Rgb` | `(B << 16) | (G << 8) | R` |
| every other format | `(R << 16) | (G << 8) | B` |

The second row includes `Bgr`, `Bitmask`, and `BltOnly`; the library does not
special-case or reject the latter two.

## Modal dialogs

`message_box` is available from a widget callback because it needs
`EventCtx`. It snapshots the rendered scene and busy-polls keyboard and pointer
input while redrawing the dialog. Buttons return on mouse press, Enter selects
OK/Yes, and Escape returns `No` even for an OK-only dialog. The title and body
must be `&'static str`; the dialog is fixed at 360 × 140 pixels and does not
wrap its body text.
