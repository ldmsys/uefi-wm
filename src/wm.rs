//! Floating window manager with Classic, Light, and Dark themes.
//!
//! # Widget catalogue
//!
//! | Widget | Handle | Constructor |
//! |--------|--------|-------------|
//! | Label | [`LabelId`] | [`WindowManager::add_label`] |
//! | TextBox | [`TextBoxId`] | [`WindowManager::add_textbox`] |
//! | TextArea | [`TextAreaId`] | [`WindowManager::add_textarea`] |
//! | CheckBox | [`CheckBoxId`] | [`WindowManager::add_checkbox`] |
//! | RadioButton | [`RadioButtonId`] | [`WindowManager::add_radiobutton`] |
//! | ComboBox | [`ComboBoxId`] | [`WindowManager::add_combobox`] |
//! | ListBox | [`ListBoxId`] | [`WindowManager::add_listbox`] |
//! | Button | [`ButtonId`] | [`WindowManager::add_button`] |
//! | ProgressBar | [`ProgressBarId`] | [`WindowManager::add_progressbar`] |
//! | Slider | [`SliderId`] | [`WindowManager::add_slider`] |
//! | NumericUpDown | [`NumericUpDownId`] | [`WindowManager::add_numeric_updown`] |
//! | GroupBox | [`GroupBoxId`] | [`WindowManager::add_groupbox`] |
//! | Separator | [`SeparatorId`] | [`WindowManager::add_separator`] |
//!
//! # Callback model
//!
//! There are two callback types, both with the same signature
//! `FnMut(&mut WindowManager, &mut EventCtx)`:
//!
//! | Type | Setter | Fires when |
//! |------|--------|------------|
//! | `on_click` | [`WindowManager::set_on_click`] | Mouse released over the same button it was pressed on |
//! | `on_change` | [`WindowManager::set_on_change`] | Input handling reports a mutation or activation for TextBox, TextArea, CheckBox, RadioButton, ComboBox, ListBox, Slider, or NumericUpDown |
//!
//! **Write accessors never fire `on_change`.** Some also update related cursor,
//! selection, or scroll state as documented by the individual method. Only
//! mutation/activation reports routed from input invoke the callback.
//!
//! [`WindowManager::event_source`] is assigned before callback lookup and
//! retains the most recently triggered widget after the callback returns.
//!
//! ## Quick example
//!
//! ```ignore
//! use uefi_wm::wm::{EventCtx, MsgBoxButtons, MsgBoxResult, WindowManager};
//!
//! // (wm: WindowManager already created)
//! let win    = wm.open("Settings", 100, 80, 500, 400);
//! let slider = wm.add_slider(win, 0,  0, 220, "Volume: ", 0, 100, 50);
//! let pbar   = wm.add_progressbar(win, 0, 32, 220, "");
//! let enable = wm.add_checkbox(win, 0, 64, "Enable audio");
//! let ok_btn = wm.add_button(win, 160, 100, 80, "OK");
//!
//! // Mirror slider position into the progress bar.
//! wm.set_on_change(slider, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
//!     wm.set_progressbar(pbar, wm.slider_value(slider) as u32, 100);
//! }));
//!
//! // Checkbox gates whether the slider is interactive.
//! wm.set_on_change(enable, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
//!     wm.widget_set_enabled(slider, wm.checkbox_checked(enable));
//! }));
//!
//! // OK button: confirm then exit.
//! wm.set_on_click(ok_btn, Box::new(|wm: &mut WindowManager, ctx: &mut EventCtx| {
//!     if wm.message_box(ctx, "Confirm", "Save and exit?", MsgBoxButtons::YesNo)
//!         == MsgBoxResult::Yes
//!     {
//!         ctx.quit = true;
//!     }
//! }));
//! ```
//!
//! # Typed widget handles
//!
//! Every `add_*` constructor returns a typed handle (`SliderId`, `ButtonId`, …).
//! Pass that handle to the matching accessor — the compiler rejects mismatched types.
//! All typed handles also implement `Into<WidgetId>` for cross-widget operations
//! (`set_on_change`, `widget_set_enabled`, `widget_set_visible`).
//!
//! # State accessors
//!
//! **Read** (never fire `on_change`):
//! [`WindowManager::textbox_text`], [`WindowManager::textarea_text`],
//! [`WindowManager::textarea_scroll`],
//! [`WindowManager::checkbox_checked`], [`WindowManager::combobox_selected`],
//! [`WindowManager::listbox_selected`], [`WindowManager::radio_selected_in_group`],
//! [`WindowManager::slider_value`], [`WindowManager::numeric_value`].
//!
//! **Write** (also never fire `on_change`):
//! [`WindowManager::label_set_text`],
//! [`WindowManager::set_textbox_text`], [`WindowManager::set_textarea_text`],
//! [`WindowManager::append_textarea_text`], [`WindowManager::set_textarea_scroll`],
//! [`WindowManager::set_checkbox_checked`], [`WindowManager::set_combobox_selected`],
//! [`WindowManager::set_listbox_items`], [`WindowManager::set_slider_value`],
//! [`WindowManager::set_progressbar`],
//! [`WindowManager::set_numeric_value`], [`WindowManager::set_numeric_step`].
//!
//! **Visibility / interactivity**:
//! [`WindowManager::widget_set_enabled`], [`WindowManager::widget_set_visible`],
//! [`WindowManager::set_textbox_readonly`], [`WindowManager::set_textarea_readonly`].
//!
//! # Custom event loop
//!
//! [`WindowManager::run`] is the standard entry point. For extra event sources,
//! a variable frame rate, or per-frame application logic, drive the loop yourself:
//!
//! ```ignore
//! use uefi::boot::{self, EventType, TimerTrigger, Tpl};
//! // fb: Framebuffer, wm: WindowManager, drv: InputDriver, gop, gop_stride already created.
//! let mut gop_fb = gop.frame_buffer();
//!
//! let frame_timer = unsafe {
//!     boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap()
//! };
//! boot::set_timer(&frame_timer, TimerTrigger::Periodic(166_670)).unwrap();
//! let timer_slot = drv.wait_events().len(); // standard loop assumes keyboard is slot 0
//!
//! loop {
//!     let mut wait_buf = drv.wait_events();
//!     wait_buf.push(unsafe { frame_timer.unsafe_clone() });
//!     let fired = boot::wait_for_event(&mut wait_buf).unwrap_or(timer_slot);
//!
//!     let events = if fired == 0 { drv.read_keys() } else { drv.read_ptr() };
//!
//!     if wm.handle(&events, &mut fb, &mut drv, &mut gop_fb, gop_stride) { break; }
//!     wm.render(&mut fb);
//!     fb.present_to(&mut gop_fb, gop_stride);
//! }
//! ```

extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};
use uefi::boot::{self, EventType, TimerTrigger, Tpl};
use uefi::proto::console::gop::FrameBuffer as GopFrameBuffer;
use uefi::proto::console::text::{Key, ScanCode};

use crate::gfx::{Color, Framebuffer};
use crate::input::{InputDriver, InputEvent};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

const TITLE_H:    i32 = 22;
const BORDER:     i32 = 3;
const PAD:        i32 = 8;
const CAPBTN_W:   i32 = 16;
const CAPBTN_H:   i32 = 14;
const CAPBTN_GAP: i32 = 2;
const TEXTBOX_H:  u32 = 22;
const CB_SZ:      u32 = 14;
const COMBO_H:    u32 = 22;
const COMBO_ARW:  u32 = 20;
const WBTN_H:     u32 = 26;
const PTR_SCALE:  i64 = 1;

const PBAR_H:     u32 = 18;
const SLIDER_H:   u32 = 22;
const SLIDER_THW: i32 = 12;   // thumb width in px
const NUD_H:      u32 = 22;
const NUD_BTN_W:  i32 = 18;   // width of the up+down column
const RADIO_R:    i32 = 5;    // radius of the radio circle
const LIST_ITEM:  u32 = 20;   // listbox row height
const GROUPBOX_T: i32 = 8;    // y-offset of groupbox title baseline above border

const FRAME_PERIOD_100NS: u64 = 166_670;

// ---------------------------------------------------------------------------
// Public dialog types
// ---------------------------------------------------------------------------

/// Built-in visual theme used by [`WindowManager`].
///
/// [`Theme::Classic`] preserves the original Windows 9x/2000-style palette.
/// [`Theme::Light`] and [`Theme::Dark`] use modern neutral surfaces, roomier
/// geometry, flat controls, hover feedback, modern caption icons, and a compact
/// pointer while preserving application state during runtime switches.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    /// Original raised-edge, teal-desktop appearance.
    Classic,
    /// Modern light appearance.
    #[default]
    Light,
    /// Modern dark appearance.
    Dark,
}

#[derive(Clone, Copy)]
struct Palette {
    desktop: Color,
    face: Color,
    highlight: Color,
    shadow: Color,
    dark_shadow: Color,
    frame: Color,
    active_l: Color,
    active_r: Color,
    inactive_l: Color,
    inactive_r: Color,
    active_text: Color,
    inactive_text: Color,
    accent: Color,
    window: Color,
    window_text: Color,
    label_text: Color,
    disabled_text: Color,
}

#[derive(Clone, Copy)]
struct Metrics {
    title_h: i32,
    border: i32,
    pad: i32,
    capbtn_w: i32,
    capbtn_h: i32,
    capbtn_gap: i32,
    textbox_h: u32,
    checkbox_sz: u32,
    combo_h: u32,
    button_h: u32,
    slider_h: u32,
    nud_h: u32,
}

impl Theme {
    const fn palette(self) -> Palette {
        match self {
            Self::Classic => Palette {
                desktop: Color::rgb(0x00, 0x80, 0x80),
                face: Color::rgb(0xD4, 0xD0, 0xC8),
                highlight: Color::rgb(0xFF, 0xFF, 0xFF),
                shadow: Color::rgb(0x80, 0x80, 0x80),
                dark_shadow: Color::rgb(0x40, 0x40, 0x40),
                frame: Color::rgb(0x00, 0x00, 0x00),
                active_l: Color::rgb(0x00, 0x00, 0x80),
                active_r: Color::rgb(0x10, 0x84, 0xD0),
                inactive_l: Color::rgb(0x7B, 0x7B, 0x7B),
                inactive_r: Color::rgb(0xB5, 0xB5, 0xB5),
                active_text: Color::rgb(0xFF, 0xFF, 0xFF),
                inactive_text: Color::rgb(0xD4, 0xD0, 0xC8),
                accent: Color::rgb(0x00, 0x00, 0x80),
                window: Color::rgb(0xFF, 0xFF, 0xFF),
                window_text: Color::rgb(0x00, 0x00, 0x00),
                label_text: Color::rgb(0x80, 0x80, 0x80),
                disabled_text: Color::rgb(0x80, 0x80, 0x80),
            },
            Self::Light => Palette {
                desktop: Color::rgb(0xE8, 0xED, 0xF5),
                face: Color::rgb(0xF5, 0xF7, 0xFA),
                highlight: Color::rgb(0xFF, 0xFF, 0xFF),
                shadow: Color::rgb(0xA8, 0xB2, 0xC1),
                dark_shadow: Color::rgb(0x69, 0x73, 0x86),
                frame: Color::rgb(0xC1, 0xC9, 0xD4),
                active_l: Color::rgb(0x25, 0x63, 0xEB),
                active_r: Color::rgb(0x3B, 0x82, 0xF6),
                inactive_l: Color::rgb(0x94, 0xA3, 0xB8),
                inactive_r: Color::rgb(0xCB, 0xD5, 0xE1),
                active_text: Color::rgb(0xFF, 0xFF, 0xFF),
                inactive_text: Color::rgb(0x33, 0x41, 0x55),
                accent: Color::rgb(0x25, 0x63, 0xEB),
                window: Color::rgb(0xFF, 0xFF, 0xFF),
                window_text: Color::rgb(0x17, 0x20, 0x33),
                label_text: Color::rgb(0x17, 0x20, 0x33),
                disabled_text: Color::rgb(0x7A, 0x86, 0x98),
            },
            Self::Dark => Palette {
                desktop: Color::rgb(0x0B, 0x11, 0x20),
                face: Color::rgb(0x1F, 0x29, 0x37),
                highlight: Color::rgb(0x47, 0x55, 0x69),
                shadow: Color::rgb(0x0F, 0x17, 0x2A),
                dark_shadow: Color::rgb(0x02, 0x06, 0x17),
                frame: Color::rgb(0x02, 0x06, 0x17),
                active_l: Color::rgb(0x2B, 0x30, 0x38),
                active_r: Color::rgb(0x2B, 0x30, 0x38),
                inactive_l: Color::rgb(0x20, 0x24, 0x2B),
                inactive_r: Color::rgb(0x20, 0x24, 0x2B),
                active_text: Color::rgb(0xFF, 0xFF, 0xFF),
                inactive_text: Color::rgb(0xCB, 0xD5, 0xE1),
                accent: Color::rgb(0x3B, 0x82, 0xF6),
                window: Color::rgb(0x11, 0x18, 0x27),
                window_text: Color::rgb(0xF1, 0xF5, 0xF9),
                label_text: Color::rgb(0xF1, 0xF5, 0xF9),
                disabled_text: Color::rgb(0x94, 0xA3, 0xB8),
            },
        }
    }

    const fn metrics(self) -> Metrics {
        match self {
            Self::Classic => Metrics {
                title_h: TITLE_H, border: BORDER, pad: PAD,
                capbtn_w: CAPBTN_W, capbtn_h: CAPBTN_H, capbtn_gap: CAPBTN_GAP,
                textbox_h: TEXTBOX_H, checkbox_sz: CB_SZ, combo_h: COMBO_H,
                button_h: WBTN_H, slider_h: SLIDER_H, nud_h: NUD_H,
            },
            Self::Light | Self::Dark => Metrics {
                title_h: 32, border: 1, pad: 12,
                capbtn_w: 32, capbtn_h: 30, capbtn_gap: 0,
                textbox_h: 26, checkbox_sz: 16, combo_h: 26,
                button_h: 30, slider_h: 26, nud_h: 26,
            },
        }
    }

    const fn is_modern(self) -> bool { !matches!(self, Self::Classic) }
}

/// Button layout for [`WindowManager::message_box`].
pub enum MsgBoxButtons {
    /// A single OK button.
    Ok,
    /// Yes and No buttons.
    YesNo,
}

#[derive(PartialEq)]
/// Result returned by [`WindowManager::message_box`].
pub enum MsgBoxResult {
    /// The OK button or Enter in an OK-only dialog.
    Ok,
    /// The Yes button or Enter in a Yes/No dialog.
    Yes,
    /// The No button, or Escape in either dialog layout.
    No,
}

// ---------------------------------------------------------------------------
// Widget callback type alias
// ---------------------------------------------------------------------------

type Cb = Box<dyn for<'ctx, 'gop> FnMut(&mut WindowManager, &mut EventCtx<'ctx, 'gop>)>;

// ---------------------------------------------------------------------------
// Typed widget handles
// ---------------------------------------------------------------------------

/// Typed global widget index for a label.
#[derive(Clone, Copy, Debug, PartialEq)] pub struct LabelId(pub usize);
/// Typed global widget index for a text box.
///
/// Read value with [`WindowManager::textbox_text`]; write with [`WindowManager::set_textbox_text`];
/// make read-only with [`WindowManager::set_textbox_readonly`];
/// react to edits with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct TextBoxId(pub usize);
/// Typed global widget index for a text area.
///
/// Read with [`WindowManager::textarea_text`]; write with [`WindowManager::set_textarea_text`]
/// or [`WindowManager::append_textarea_text`]; scroll with [`WindowManager::textarea_scroll`] /
/// [`WindowManager::set_textarea_scroll`]; make read-only with [`WindowManager::set_textarea_readonly`];
/// react to edits with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct TextAreaId(pub usize);
/// Typed global widget index for a checkbox.
///
/// Read with [`WindowManager::checkbox_checked`]; write with [`WindowManager::set_checkbox_checked`];
/// react to toggles with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct CheckBoxId(pub usize);
/// Typed global widget index for a radio button.
///
/// Query the selected button in a group with [`WindowManager::radio_selected_in_group`].
/// React to selection changes with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct RadioButtonId(pub usize);
/// Typed global widget index for a combo box.
///
/// Read selected index with [`WindowManager::combobox_selected`];
/// write with [`WindowManager::set_combobox_selected`];
/// react to selection changes with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct ComboBoxId(pub usize);
/// Typed global widget index for a list box.
///
/// Read selected index with [`WindowManager::listbox_selected`];
/// replace items with [`WindowManager::set_listbox_items`];
/// react to selection changes with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct ListBoxId(pub usize);
/// Typed global widget index for a button.
///
/// React to clicks with [`WindowManager::set_on_click`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct ButtonId(pub usize);
/// Typed global widget index for a progress bar.
///
/// Update with [`WindowManager::set_progressbar`]. Not interactive.
#[derive(Clone, Copy, Debug, PartialEq)] pub struct ProgressBarId(pub usize);
/// Typed global widget index for a slider.
///
/// Read value with [`WindowManager::slider_value`]; write with [`WindowManager::set_slider_value`];
/// react to drags and key presses with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct SliderId(pub usize);
/// Typed global widget index for a numeric up-down control.
///
/// Read value with [`WindowManager::numeric_value`]; write with [`WindowManager::set_numeric_value`];
/// change step size with [`WindowManager::set_numeric_step`];
/// react to changes with [`WindowManager::set_on_change`].
#[derive(Clone, Copy, Debug, PartialEq)] pub struct NumericUpDownId(pub usize);
/// Typed global widget index for a group box.
/// Decorative only — no value accessors.
#[derive(Clone, Copy, Debug, PartialEq)] pub struct GroupBoxId(pub usize);
/// Typed global widget index for a separator.
/// Decorative only — no value accessors.
#[derive(Clone, Copy, Debug, PartialEq)] pub struct SeparatorId(pub usize);

/// Type-erased widget handle.
///
/// Used by [`WindowManager::widget_set_enabled`], [`WindowManager::widget_set_visible`],
/// [`WindowManager::set_on_change`], and the [`WindowManager::event_source`] field.
/// Every typed handle (`SliderId`, `ButtonId`, …) converts into `WidgetId` via `From`/`Into`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WidgetId {
    /// Label handle.
    Label(LabelId),
    /// Text-box handle.
    TextBox(TextBoxId),
    /// Text-area handle.
    TextArea(TextAreaId),
    /// Checkbox handle.
    CheckBox(CheckBoxId),
    /// Radio-button handle.
    RadioButton(RadioButtonId),
    /// Combo-box handle.
    ComboBox(ComboBoxId),
    /// List-box handle.
    ListBox(ListBoxId),
    /// Button handle.
    Button(ButtonId),
    /// Progress-bar handle.
    ProgressBar(ProgressBarId),
    /// Slider handle.
    Slider(SliderId),
    /// Numeric up-down handle.
    NumericUpDown(NumericUpDownId),
    /// Group-box handle.
    GroupBox(GroupBoxId),
    /// Separator handle.
    Separator(SeparatorId),
}

impl WidgetId {
    fn idx(self) -> usize {
        match self {
            WidgetId::Label(id)         => id.0,
            WidgetId::TextBox(id)       => id.0,
            WidgetId::TextArea(id)      => id.0,
            WidgetId::CheckBox(id)      => id.0,
            WidgetId::RadioButton(id)   => id.0,
            WidgetId::ComboBox(id)      => id.0,
            WidgetId::ListBox(id)       => id.0,
            WidgetId::Button(id)        => id.0,
            WidgetId::ProgressBar(id)   => id.0,
            WidgetId::Slider(id)        => id.0,
            WidgetId::NumericUpDown(id) => id.0,
            WidgetId::GroupBox(id)      => id.0,
            WidgetId::Separator(id)     => id.0,
        }
    }
}

impl From<LabelId>         for WidgetId { fn from(id: LabelId)         -> Self { WidgetId::Label(id) } }
impl From<TextBoxId>       for WidgetId { fn from(id: TextBoxId)       -> Self { WidgetId::TextBox(id) } }
impl From<TextAreaId>      for WidgetId { fn from(id: TextAreaId)      -> Self { WidgetId::TextArea(id) } }
impl From<CheckBoxId>      for WidgetId { fn from(id: CheckBoxId)      -> Self { WidgetId::CheckBox(id) } }
impl From<RadioButtonId>   for WidgetId { fn from(id: RadioButtonId)   -> Self { WidgetId::RadioButton(id) } }
impl From<ComboBoxId>      for WidgetId { fn from(id: ComboBoxId)      -> Self { WidgetId::ComboBox(id) } }
impl From<ListBoxId>       for WidgetId { fn from(id: ListBoxId)       -> Self { WidgetId::ListBox(id) } }
impl From<ButtonId>        for WidgetId { fn from(id: ButtonId)        -> Self { WidgetId::Button(id) } }
impl From<ProgressBarId>   for WidgetId { fn from(id: ProgressBarId)   -> Self { WidgetId::ProgressBar(id) } }
impl From<SliderId>        for WidgetId { fn from(id: SliderId)        -> Self { WidgetId::Slider(id) } }
impl From<NumericUpDownId> for WidgetId { fn from(id: NumericUpDownId) -> Self { WidgetId::NumericUpDown(id) } }
impl From<GroupBoxId>      for WidgetId { fn from(id: GroupBoxId)      -> Self { WidgetId::GroupBox(id) } }
impl From<SeparatorId>     for WidgetId { fn from(id: SeparatorId)     -> Self { WidgetId::Separator(id) } }

// ---------------------------------------------------------------------------
// EventCtx
// ---------------------------------------------------------------------------

/// Runtime context passed to every widget callback alongside `&mut WindowManager`.
///
/// Both `on_click` and `on_change` callbacks receive:
///
/// ```text
/// |wm: &mut WindowManager, ctx: &mut EventCtx| { … }
/// ```
///
/// Use `ctx` to signal exit, draw onto the back-buffer, or force an immediate
/// screen blit.  Use `wm` to read or write widget state, open new windows, or
/// show a [`WindowManager::message_box`].
pub struct EventCtx<'a, 'gop> {
    /// Set to `true` to stop [`WindowManager::run`] after this callback returns.
    ///
    /// ```ignore
    /// |_wm: &mut WindowManager, ctx: &mut EventCtx| {
    ///     ctx.quit = true; // event loop exits after returning from this closure
    /// }
    /// # ;
    /// ```
    pub quit:       bool,

    /// The current back-buffer.
    ///
    /// You can draw custom graphics here with [`Framebuffer::fill`],
    /// [`Framebuffer::text_aa`], etc.  The next call to [`WindowManager::render`]
    /// redraws the full scene and will overwrite any pixels you set, so call
    /// [`EventCtx::present`] immediately if you need the user to see the change
    /// before the next frame.
    pub fb:         &'a mut Framebuffer,
    /// Input and GOP access remain private; use [`Self::present`] to blit.
    drv:            &'a mut InputDriver,
    gop_fb:         &'a mut GopFrameBuffer<'gop>,
    gop_stride:     usize,
}

impl EventCtx<'_, '_> {
    /// Blit the back-buffer to the GOP framebuffer immediately.
    ///
    /// [`WindowManager::run`] already calls this once per frame after
    /// [`WindowManager::render`].  Call `present()` inside a callback only when
    /// you need the user to see an interim state — for example, a "working…"
    /// overlay drawn to [`EventCtx::fb`] before a long-running operation starts.
    pub fn present(&mut self) {
        self.fb.present_to(self.gop_fb, self.gop_stride)
    }
}

// ---------------------------------------------------------------------------
// Widget structs (crate-private implementation details)
// ---------------------------------------------------------------------------

struct Label {
    win_id:  u32,
    rel_x:   i32,
    rel_y:   i32,
    width:   u32,    // 0 = auto (no clip)
    text:    String,
    visible: bool,
}

struct TextBox {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,
    label:     &'static str,
    text:      String,
    cursor:    usize,
    enabled:   bool,
    read_only: bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct TextArea {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,
    height:    u32,
    text:      String,
    cursor:    usize,   // byte offset into text
    scroll_y:  usize,   // first visible line index
    enabled:   bool,
    read_only: bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct CheckBox {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    label:     &'static str,
    checked:   bool,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct RadioButton {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    label:     &'static str,
    group_id:  u32,
    selected:  bool,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct ComboBox {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,
    label:     &'static str,
    options:   Vec<&'static str>,
    selected:  usize,
    open:      bool,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct ListBox {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,
    height:    u32,
    items:     Vec<&'static str>,
    selected:  Option<usize>,
    scroll:    usize,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct Button {
    win_id:   u32,
    rel_x:    i32,
    rel_y:    i32,
    width:    u32,
    label:    &'static str,
    pressed:  bool,
    enabled:  bool,
    visible:  bool,
    on_click: Option<Cb>,
}

struct ProgressBar {
    win_id:  u32,
    rel_x:   i32,
    rel_y:   i32,
    width:   u32,
    label:   &'static str,
    value:   u32,
    max:     u32,
    visible: bool,
}

struct Slider {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,
    label:     &'static str,
    min:       i32,
    max:       i32,
    value:     i32,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
}

struct NumericUpDown {
    win_id:    u32,
    rel_x:     i32,
    rel_y:     i32,
    width:     u32,   // width of the number box, not counting label or buttons
    label:     &'static str,
    value:     i32,
    min:       i32,
    max:       i32,
    step:      i32,
    enabled:   bool,
    visible:   bool,
    on_change: Option<Cb>,
    edit_buf:  Option<String>,  // Some(_) while user is typing a value directly
}

struct GroupBox {
    win_id:  u32,
    rel_x:   i32,
    rel_y:   i32,
    width:   u32,
    height:  u32,
    label:   &'static str,
    visible: bool,
}

struct Separator {
    win_id:  u32,
    rel_x:   i32,
    rel_y:   i32,
    width:   u32,
    visible: bool,
}

// ---------------------------------------------------------------------------
// Widget enum
// ---------------------------------------------------------------------------

enum Widget {
    Label(Label),
    TextBox(TextBox),
    TextArea(TextArea),
    CheckBox(CheckBox),
    RadioButton(RadioButton),
    ComboBox(ComboBox),
    ListBox(ListBox),
    Button(Button),
    ProgressBar(ProgressBar),
    Slider(Slider),
    NumericUpDown(NumericUpDown),
    GroupBox(GroupBox),
    Separator(Separator),
}

impl Widget {
    fn win_id(&self) -> u32 {
        match self {
            Widget::Label(w)         => w.win_id,
            Widget::TextBox(w)       => w.win_id,
            Widget::TextArea(w)      => w.win_id,
            Widget::CheckBox(w)      => w.win_id,
            Widget::RadioButton(w)   => w.win_id,
            Widget::ComboBox(w)      => w.win_id,
            Widget::ListBox(w)       => w.win_id,
            Widget::Button(w)        => w.win_id,
            Widget::ProgressBar(w)   => w.win_id,
            Widget::Slider(w)        => w.win_id,
            Widget::NumericUpDown(w) => w.win_id,
            Widget::GroupBox(w)      => w.win_id,
            Widget::Separator(w)     => w.win_id,
        }
    }

    fn rel_pos(&self) -> (i32, i32) {
        match self {
            Widget::Label(w)         => (w.rel_x, w.rel_y),
            Widget::TextBox(w)       => (w.rel_x, w.rel_y),
            Widget::TextArea(w)      => (w.rel_x, w.rel_y),
            Widget::CheckBox(w)      => (w.rel_x, w.rel_y),
            Widget::RadioButton(w)   => (w.rel_x, w.rel_y),
            Widget::ComboBox(w)      => (w.rel_x, w.rel_y),
            Widget::ListBox(w)       => (w.rel_x, w.rel_y),
            Widget::Button(w)        => (w.rel_x, w.rel_y),
            Widget::ProgressBar(w)   => (w.rel_x, w.rel_y),
            Widget::Slider(w)        => (w.rel_x, w.rel_y),
            Widget::NumericUpDown(w) => (w.rel_x, w.rel_y),
            Widget::GroupBox(w)      => (w.rel_x, w.rel_y),
            Widget::Separator(w)     => (w.rel_x, w.rel_y),
        }
    }

    fn is_focusable(&self) -> bool {
        matches!(self,
            Widget::TextBox(_)
            | Widget::TextArea(_)
            | Widget::ComboBox(_)
            | Widget::ListBox(_)
            | Widget::Slider(_)
            | Widget::NumericUpDown(_)
            | Widget::RadioButton(_))
    }

    fn is_visible(&self) -> bool {
        match self {
            Widget::Label(w)         => w.visible,
            Widget::TextBox(w)       => w.visible,
            Widget::TextArea(w)      => w.visible,
            Widget::CheckBox(w)      => w.visible,
            Widget::RadioButton(w)   => w.visible,
            Widget::ComboBox(w)      => w.visible,
            Widget::ListBox(w)       => w.visible,
            Widget::Button(w)        => w.visible,
            Widget::ProgressBar(w)   => w.visible,
            Widget::Slider(w)        => w.visible,
            Widget::NumericUpDown(w) => w.visible,
            Widget::GroupBox(w)      => w.visible,
            Widget::Separator(w)     => w.visible,
        }
    }

    fn is_enabled(&self) -> bool {
        match self {
            Widget::TextBox(w)       => w.enabled,
            Widget::TextArea(w)      => w.enabled,
            Widget::CheckBox(w)      => w.enabled,
            Widget::RadioButton(w)   => w.enabled,
            Widget::ComboBox(w)      => w.enabled,
            Widget::ListBox(w)       => w.enabled,
            Widget::Button(w)        => w.enabled,
            Widget::Slider(w)        => w.enabled,
            Widget::NumericUpDown(w) => w.enabled,
            _                        => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Window
// ---------------------------------------------------------------------------

struct Window {
    id:        u32,
    title:     String,
    x:         i32,
    y:         i32,
    w:         u32,
    h:         u32,
    vis:       bool,
    minimized: bool,
    dragging:  bool,
    drag_ox:   i32,
    drag_oy:   i32,
}

impl Window {
    fn title_rect(&self, m: Metrics) -> (i32, i32, u32, u32) {
        (self.x + m.border, self.y + m.border,
         self.w - (m.border * 2) as u32, m.title_h as u32)
    }
    fn btn_rect(&self, idx: i32, m: Metrics) -> (i32, i32, u32, u32) {
        let (tx, ty, tw, _) = self.title_rect(m);
        let classic_inset = if m.capbtn_gap > 0 { 2 } else { 0 };
        let bx = tx + tw as i32 - classic_inset
            - (idx + 1) * (m.capbtn_w + m.capbtn_gap) + m.capbtn_gap;
        let by = ty + (m.title_h - m.capbtn_h) / 2;
        (bx, by, m.capbtn_w as u32, m.capbtn_h as u32)
    }
    fn close_rect(&self, m: Metrics)    -> (i32, i32, u32, u32) { self.btn_rect(0, m) }
    fn maximize_rect(&self, m: Metrics) -> (i32, i32, u32, u32) { self.btn_rect(1, m) }
    fn minimize_rect(&self, m: Metrics) -> (i32, i32, u32, u32) { self.btn_rect(2, m) }

    fn hit(px: i32, py: i32, r: (i32, i32, u32, u32)) -> bool {
        let (x, y, w, h) = r;
        px >= x && px < x + w as i32 && py >= y && py < y + h as i32
    }
    fn hit_close(&self, px: i32, py: i32, m: Metrics) -> bool { Self::hit(px, py, self.close_rect(m)) }
    fn hit_minimize(&self, px: i32, py: i32, m: Metrics) -> bool { Self::hit(px, py, self.minimize_rect(m)) }
    fn hit_title(&self, px: i32, py: i32, m: Metrics) -> bool {
        let (tx, ty, tw, th) = self.title_rect(m);
        px >= tx && px < tx + tw as i32 && py >= ty && py < ty + th as i32
    }
    fn hit_body(&self, px: i32, py: i32, m: Metrics) -> bool {
        let h = self.visible_height(m);
        px >= self.x && px < self.x + self.w as i32
            && py >= self.y && py < self.y + h as i32
    }
    fn visible_height(&self, m: Metrics) -> u32 {
        if self.minimized { (m.border * 2 + m.title_h) as u32 } else { self.h }
    }
    fn client_origin(&self, m: Metrics) -> (i32, i32) {
        (self.x + m.border + m.pad, self.y + m.border + m.title_h + 1 + m.pad)
    }
}

// ---------------------------------------------------------------------------
// WindowManager
// ---------------------------------------------------------------------------

/// Owns the desktop's windows, widgets, focus, cursor, and font.
pub struct WindowManager {
    windows:          Vec<Window>,
    next_id:          u32,
    /// Current cursor X coordinate in screen pixels.
    pub cx:           i32,
    /// Current cursor Y coordinate in screen pixels.
    pub cy:           i32,
    sw:               u32,
    sh:               u32,
    lbtn:             bool,
    theme:            Theme,

    widgets:          Vec<Widget>,
    focused_widget:   Option<usize>,
    armed_btn:        Option<usize>,
    slider_drag:      Option<usize>,
    /// (widget_idx, mouse_y_at_start, scroll_at_start)
    scroll_drag:      Option<(usize, i32, usize)>,

    /// The most recently triggered widget, or `None` before the first trigger.
    ///
    /// The field is assigned before the manager looks up a callback. It remains
    /// set after callback completion and can therefore be stale outside a
    /// callback.
    ///
    /// For a single-widget callback, the simplest approach is to capture the
    /// typed handle directly in the closure — `event_source` is not needed:
    ///
    /// ```ignore
    /// // slider: SliderId — captured from the enclosing scope
    /// wm.set_on_change(slider, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     let v = wm.slider_value(slider); // no need to consult event_source
    ///     // …
    /// }));
    /// ```
    ///
    /// When the same closure body is registered on several widgets, pattern-match
    /// `event_source` to identify the caller:
    ///
    /// ```ignore
    /// // s1, s2: SliderId — both drive the same label
    /// let handler = move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     let id = match wm.event_source {
    ///         Some(WidgetId::Slider(id)) => id,
    ///         _ => return,
    ///     };
    ///     let v = wm.slider_value(id);
    ///     // update label with v …
    ///     let _ = (v, lbl);
    /// };
    /// wm.set_on_change(s1, Box::new(handler));
    /// // handler has been moved; register s2 with a separate closure:
    /// wm.set_on_change(s2, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     if let Some(WidgetId::Slider(id)) = wm.event_source {
    ///         let _ = wm.slider_value(id); // …
    ///     }
    /// }));
    /// ```
    pub event_source: Option<WidgetId>,

    font:             fontdue::Font,
    ascent:           i32,
    font_px:          f32,
}

impl WindowManager {
    /// Constructs a Light-themed empty window manager and parses the
    /// caller-supplied font.
    ///
    /// The cursor starts at the center of the screen. `fontdue` copies the font
    /// data, so `font_bytes` need not have a static lifetime.
    ///
    /// # Panics
    ///
    /// Panics if `font_bytes` is not a font accepted by `fontdue`.
    pub fn new(sw: u32, sh: u32, font_bytes: &[u8], font_px: f32) -> Self {
        Self::new_with_theme(sw, sh, font_bytes, font_px, Theme::Light)
    }

    /// Constructs an empty window manager with the selected built-in theme.
    ///
    /// The theme can later be changed with [`Self::set_theme`].
    pub fn new_with_theme(sw: u32, sh: u32, font_bytes: &[u8], font_px: f32,
                          theme: Theme) -> Self {
        let font = fontdue::Font::from_bytes(
            font_bytes, fontdue::FontSettings::default(),
        ).unwrap();
        let ascent = font.horizontal_line_metrics(font_px)
            .map(|m| m.ascent as i32).unwrap_or(13);
        Self {
            windows: Vec::new(), next_id: 0,
            cx: sw as i32 / 2, cy: sh as i32 / 2,
            sw, sh, lbtn: false, theme,
            widgets: Vec::new(), focused_widget: None,
            armed_btn: None, slider_drag: None, scroll_drag: None, event_source: None,
            font, ascent, font_px,
        }
    }

    /// Returns the currently selected visual theme.
    pub const fn theme(&self) -> Theme { self.theme }

    /// Selects the visual theme used by subsequent renders and message boxes.
    ///
    /// Window positions, focus, widget values, and callbacks are unchanged.
    pub fn set_theme(&mut self, theme: Theme) { self.theme = theme; }

    // -----------------------------------------------------------------------
    // Font helpers
    // -----------------------------------------------------------------------

    fn text_w(&self, s: &str) -> i32 {
        Framebuffer::text_width(s, &self.font, self.font_px)
    }

    fn label_px(&self, label: &str) -> i32 {
        self.text_w(label) + self.text_w(" ")
    }

    fn fit<'s>(&self, s: &'s str, max_px: i32) -> &'s str {
        let mut w = 0i32;
        let mut end = 0usize;
        for ch in s.chars() {
            let cw = self.font.metrics(ch, self.font_px).advance_width as i32;
            if w + cw > max_px { break; }
            w += cw; end += ch.len_utf8();
        }
        &s[..end]
    }

    fn line_h(&self) -> i32 { self.font_px as i32 + 3 }

    /// Returns the byte offset within `s` that corresponds to a click at `click_x` pixels
    /// from the left of the rendered string, snapping to the nearest character boundary.
    fn cursor_from_x(&self, s: &str, click_x: i32) -> usize {
        let mut x = 0i32;
        let mut pos = 0usize;
        for ch in s.chars() {
            let cw = self.font.metrics(ch, self.font_px).advance_width as i32;
            if x + cw / 2 >= click_x { break; }
            x += cw;
            pos += ch.len_utf8();
        }
        pos
    }

    fn textbox_cursor_at(&self, idx: usize, ox: i32, click_x: i32) -> usize {
        let Widget::TextBox(tb) = &self.widgets[idx] else { return 0; };
        let inner_x = ox + tb.rel_x + self.label_px(tb.label) + 4;
        let max_px  = tb.width as i32 - 8;
        let cursor  = tb.cursor.min(tb.text.len());
        let left    = self.fit_right(&tb.text[..cursor], max_px);
        let view_start = cursor - left.len();
        let rel_x = (click_x - inner_x).max(0);
        view_start + self.cursor_from_x(&tb.text[view_start..], rel_x)
    }

    fn textarea_cursor_at(&self, idx: usize, ox: i32, oy: i32, click_x: i32, click_y: i32) -> usize {
        let Widget::TextArea(ta) = &self.widgets[idx] else { return 0; };
        let bx = ox + ta.rel_x;
        let by = oy + ta.rel_y;
        let lh = self.line_h();
        let clicked_row  = ((click_y - (by + 2)).max(0) / lh) as usize;
        let clicked_line = ta.scroll_y + clicked_row;
        let mut byte_pos = 0usize;
        for (line_i, chunk) in ta.text.split('\n').enumerate() {
            if line_i == clicked_line {
                let rel_x = (click_x - (bx + 4)).max(0);
                return byte_pos + self.cursor_from_x(chunk, rel_x);
            }
            byte_pos += chunk.len() + 1;
        }
        ta.text.len()
    }

    // -----------------------------------------------------------------------
    // Window
    // -----------------------------------------------------------------------

    /// Opens a visible, non-minimized window and returns its numeric ID.
    ///
    /// The title is copied. New windows are appended at the top of z-order.
    pub fn open(&mut self, title: &str, x: i32, y: i32, w: u32, h: u32) -> u32 {
        let id = self.next_id; self.next_id += 1;
        self.windows.push(Window {
            id, title: String::from(title), x, y, w, h,
            vis: true, minimized: false, dragging: false, drag_ox: 0, drag_oy: 0,
        });
        id
    }

    // -----------------------------------------------------------------------
    // Widget constructors
    // -----------------------------------------------------------------------

    /// Adds a mutable text label. A width of zero disables horizontal clipping.
    pub fn add_label(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                     width: u32, text: &str) -> LabelId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::Label(Label {
            win_id, rel_x, rel_y, width, text: String::from(text), visible: true,
        }));
        LabelId(idx)
    }

    /// Adds an empty single-line text box with a static label.
    ///
    /// The first text box added for a window may become the focused widget.
    pub fn add_textbox(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                       width: u32, label: &'static str) -> TextBoxId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::TextBox(TextBox {
            win_id, rel_x, rel_y, width, label, text: String::new(), cursor: 0,
            enabled: true, read_only: false, visible: true, on_change: None,
        }));
        if self.focused_widget.is_none()
            || self.widgets[self.focused_widget.unwrap()].win_id() != win_id
        {
            self.focused_widget = Some(idx);
        }
        TextBoxId(idx)
    }

    /// Adds an empty multiline text area.
    pub fn add_textarea(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                        width: u32, height: u32) -> TextAreaId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::TextArea(TextArea {
            win_id, rel_x, rel_y, width, height,
            text: String::new(), cursor: 0, scroll_y: 0,
            enabled: true, read_only: false, visible: true, on_change: None,
        }));
        TextAreaId(idx)
    }

    /// Adds an initially unchecked checkbox with a static label.
    pub fn add_checkbox(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                        label: &'static str) -> CheckBoxId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::CheckBox(CheckBox {
            win_id, rel_x, rel_y, label, checked: false,
            enabled: true, visible: true, on_change: None,
        }));
        CheckBoxId(idx)
    }

    /// Adds a radio button to a window-local group.
    ///
    /// The first button added to each `(win_id, group_id)` is selected.
    pub fn add_radiobutton(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                           label: &'static str, group_id: u32) -> RadioButtonId {
        let idx = self.widgets.len();
        // First radio in group is auto-selected
        let is_first = !self.widgets.iter().any(|w| {
            w.win_id() == win_id
                && matches!(w, Widget::RadioButton(r) if r.group_id == group_id)
        });
        self.widgets.push(Widget::RadioButton(RadioButton {
            win_id, rel_x, rel_y, label, group_id,
            selected: is_first, enabled: true, visible: true, on_change: None,
        }));
        RadioButtonId(idx)
    }

    /// Adds a combo box whose initial selected index is zero.
    ///
    /// The label and options are retained as static string references.
    pub fn add_combobox(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                        width: u32, label: &'static str,
                        options: &[&'static str]) -> ComboBoxId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::ComboBox(ComboBox {
            win_id, rel_x, rel_y, width, label,
            options: options.iter().copied().collect(),
            selected: 0, open: false,
            enabled: true, visible: true, on_change: None,
        }));
        ComboBoxId(idx)
    }

    /// Adds a scrollable list box with no initial selection.
    ///
    /// Items are retained as static string references.
    pub fn add_listbox(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                       width: u32, height: u32, items: &[&'static str]) -> ListBoxId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::ListBox(ListBox {
            win_id, rel_x, rel_y, width, height,
            items: items.iter().copied().collect(),
            selected: None, scroll: 0,
            enabled: true, visible: true, on_change: None,
        }));
        ListBoxId(idx)
    }

    /// Adds a push button with a static label.
    pub fn add_button(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                      width: u32, label: &'static str) -> ButtonId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::Button(Button {
            win_id, rel_x, rel_y, width, label, pressed: false,
            enabled: true, visible: true, on_click: None,
        }));
        ButtonId(idx)
    }

    /// Adds a noninteractive progress bar initialized to `0 / 100`.
    pub fn add_progressbar(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                           width: u32, label: &'static str) -> ProgressBarId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::ProgressBar(ProgressBar {
            win_id, rel_x, rel_y, width, label,
            value: 0, max: 100, visible: true,
        }));
        ProgressBarId(idx)
    }

    /// Adds a horizontal integer slider and clamps `initial` to its range.
    ///
    /// # Panics
    ///
    /// Panics when `min > max`.
    pub fn add_slider(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                      width: u32, label: &'static str,
                      min: i32, max: i32, initial: i32) -> SliderId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::Slider(Slider {
            win_id, rel_x, rel_y, width, label,
            min, max, value: initial.clamp(min, max),
            enabled: true, visible: true, on_change: None,
        }));
        SliderId(idx)
    }

    /// Adds an integer editor with up/down buttons and an initial step of one.
    ///
    /// `initial` is clamped to the inclusive range.
    ///
    /// # Panics
    ///
    /// Panics when `min > max`.
    pub fn add_numeric_updown(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                              width: u32, label: &'static str,
                              min: i32, max: i32, initial: i32) -> NumericUpDownId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::NumericUpDown(NumericUpDown {
            win_id, rel_x, rel_y, width, label,
            value: initial.clamp(min, max), min, max, step: 1,
            enabled: true, visible: true, on_change: None, edit_buf: None,
        }));
        NumericUpDownId(idx)
    }

    /// Adds a decorative labeled group border.
    pub fn add_groupbox(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                        width: u32, height: u32, label: &'static str) -> GroupBoxId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::GroupBox(GroupBox {
            win_id, rel_x, rel_y, width, height, label, visible: true,
        }));
        GroupBoxId(idx)
    }

    /// Adds a decorative two-pixel horizontal separator.
    pub fn add_separator(&mut self, win_id: u32, rel_x: i32, rel_y: i32,
                         width: u32) -> SeparatorId {
        let idx = self.widgets.len();
        self.widgets.push(Widget::Separator(Separator {
            win_id, rel_x, rel_y, width, visible: true,
        }));
        SeparatorId(idx)
    }

    // -----------------------------------------------------------------------
    // Callback setters
    // -----------------------------------------------------------------------

    /// Attach a click callback to a button widget.
    ///
    /// The closure fires when the left mouse button is **released** over the same
    /// button it was pressed on.  If the user presses and then drags off the button
    /// before releasing, the click is cancelled and the callback does not fire.
    ///
    /// Inside the callback you have full access to [`WindowManager`] — you can read
    /// and write widget state, open new windows, or show a modal
    /// [`WindowManager::message_box`].  Set [`EventCtx::quit`] to `true` to stop
    /// the event loop after the callback returns.
    ///
    /// # Example — exit after a confirmation dialog
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, MsgBoxButtons, MsgBoxResult, WindowManager};
    ///
    /// let win    = wm.open("App", 100, 80, 400, 300);
    /// let ok_btn = wm.add_button(win, 0, 0, 80, "OK");
    ///
    /// wm.set_on_click(ok_btn, Box::new(|wm: &mut WindowManager, ctx: &mut EventCtx| {
    ///     let answer = wm.message_box(ctx, "Confirm", "Exit?", MsgBoxButtons::YesNo);
    ///     if answer == MsgBoxResult::Yes {
    ///         ctx.quit = true; // wm.run() returns after this callback
    ///     }
    /// }));
    /// ```
    ///
    /// # Example — open a second window on click
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, WindowManager};
    ///
    /// let win  = wm.open("Main", 100, 80, 400, 300);
    /// let btn  = wm.add_button(win, 0, 0, 120, "Show details");
    ///
    /// wm.set_on_click(btn, Box::new(|wm: &mut WindowManager, _: &mut EventCtx| {
    ///     let detail_win = wm.open("Details", 200, 150, 360, 260);
    ///     wm.add_label(detail_win, 0, 0, 340, "Version 1.0 — built with uefi-wm.");
    /// }));
    /// ```
    pub fn set_on_click(&mut self, id: ButtonId,
                        cb: Box<dyn for<'ctx, 'gop> FnMut(
                            &mut WindowManager, &mut EventCtx<'ctx, 'gop>)>) {
        if let Some(Widget::Button(b)) = self.widgets.get_mut(id.0) {
            b.on_click = Some(cb);
        }
    }

    /// Attach a value-change callback to any interactive widget.
    ///
    /// The closure fires when input handling reports a mutation or activation by
    /// typing, clicking, dragging, or using arrow keys. Most controls report only
    /// actual value changes. Radio-button selection reports even when the selected
    /// button was already selected, and committing a successfully parsed numeric
    /// edit reports even when the clamped value is unchanged. Programmatic setters
    /// such as [`WindowManager::set_slider_value`] never fire it.
    ///
    /// Accepted widget types: [`TextBoxId`], [`TextAreaId`], [`CheckBoxId`],
    /// [`RadioButtonId`], [`ComboBoxId`], [`ListBoxId`], [`SliderId`],
    /// [`NumericUpDownId`].  Every typed handle converts to [`WidgetId`] via
    /// `Into`, so you can pass a `SliderId` (or any other handle) directly.
    ///
    /// # Example — slider controlling a progress bar
    ///
    /// The most common pattern: capture the typed handles in a `move` closure so
    /// you never need to inspect [`WindowManager::event_source`].
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, WindowManager};
    ///
    /// let win    = wm.open("Audio", 100, 80, 400, 300);
    /// let slider = wm.add_slider(win, 0, 0, 220, "Volume: ", 0, 100, 50);
    /// let pbar   = wm.add_progressbar(win, 0, 32, 220, "");
    /// wm.set_progressbar(pbar, 50, 100); // initial state
    ///
    /// wm.set_on_change(slider, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     wm.set_progressbar(pbar, wm.slider_value(slider) as u32, 100);
    /// }));
    /// ```
    ///
    /// # Example — checkbox enabling another widget
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, WindowManager};
    ///
    /// let win     = wm.open("Network", 100, 80, 400, 300);
    /// let enabled = wm.add_checkbox(win, 0, 0, "Custom DNS");
    /// let dns_box = wm.add_textbox(win, 0, 24, 260, "DNS server: ");
    /// wm.widget_set_enabled(dns_box, false); // grayed out until checked
    ///
    /// wm.set_on_change(enabled, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     wm.widget_set_enabled(dns_box, wm.checkbox_checked(enabled));
    /// }));
    /// ```
    ///
    /// # Example — combo box switching visible panels
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, WindowManager};
    ///
    /// let win   = wm.open("Settings", 100, 80, 500, 400);
    /// let combo = wm.add_combobox(win, 0, 0, 200, "Mode: ", &["Simple", "Advanced"]);
    /// let lbl_a = wm.add_label(win, 0, 32, 460, "Simple mode options…");
    /// let lbl_b = wm.add_label(win, 0, 32, 460, "Advanced mode options…");
    /// wm.widget_set_visible(lbl_b, false);
    ///
    /// wm.set_on_change(combo, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///     let advanced = wm.combobox_selected(combo) == 1;
    ///     wm.widget_set_visible(lbl_a, !advanced);
    ///     wm.widget_set_visible(lbl_b,  advanced);
    /// }));
    /// ```
    ///
    /// # Example — one closure handling multiple widgets via `event_source`
    ///
    /// When you need to register the same logic on several widgets, write a
    /// separate `Box::new(…)` per call (closures cannot be cloned) and match on
    /// [`WindowManager::event_source`] inside:
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, WidgetId, WindowManager};
    ///
    /// let win = wm.open("Color", 100, 80, 400, 300);
    /// let r   = wm.add_slider(win, 0,  0, 200, "R: ", 0, 255, 128);
    /// let g   = wm.add_slider(win, 0, 30, 200, "G: ", 0, 255, 128);
    /// let b   = wm.add_slider(win, 0, 60, 200, "B: ", 0, 255, 128);
    /// let lbl = wm.add_label(win, 220, 0, 160, "");
    ///
    /// // Shared handler — registered independently for each slider.
    /// for &sid in &[r, g, b] {
    ///     wm.set_on_change(sid, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
    ///         // All three handles are captured; read whichever changed via event_source.
    ///         let _r = wm.slider_value(r);
    ///         let _g = wm.slider_value(g);
    ///         let _b = wm.slider_value(b);
    ///         // (integer-to-string formatting omitted; update lbl here)
    ///         let _ = lbl;
    ///     }));
    /// }
    /// ```
    pub fn set_on_change(&mut self, id: impl Into<WidgetId>,
                         cb: Box<dyn for<'ctx, 'gop> FnMut(
                             &mut WindowManager, &mut EventCtx<'ctx, 'gop>)>) {
        let idx = id.into().idx();
        match self.widgets.get_mut(idx) {
            Some(Widget::TextBox(w))       => w.on_change = Some(cb),
            Some(Widget::TextArea(w))      => w.on_change = Some(cb),
            Some(Widget::CheckBox(w))      => w.on_change = Some(cb),
            Some(Widget::RadioButton(w))   => w.on_change = Some(cb),
            Some(Widget::ComboBox(w))      => w.on_change = Some(cb),
            Some(Widget::ListBox(w))       => w.on_change = Some(cb),
            Some(Widget::Slider(w))        => w.on_change = Some(cb),
            Some(Widget::NumericUpDown(w)) => w.on_change = Some(cb),
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // State accessors
    // -----------------------------------------------------------------------

    /// Replaces a label's owned text.
    pub fn label_set_text(&mut self, id: LabelId, text: &str) {
        if let Some(Widget::Label(w)) = self.widgets.get_mut(id.0) {
            w.text.clear(); w.text.push_str(text);
        }
    }

    /// Returns a text box's contents, or `""` for a wrong or stale handle.
    pub fn textbox_text(&self, id: TextBoxId) -> &str {
        match self.widgets.get(id.0) {
            Some(Widget::TextBox(w)) => &w.text, _ => "",
        }
    }

    /// Replaces a text box's contents and moves its cursor to the end.
    pub fn set_textbox_text(&mut self, id: TextBoxId, text: &str) {
        if let Some(Widget::TextBox(w)) = self.widgets.get_mut(id.0) {
            w.text.clear(); w.text.push_str(text);
            w.cursor = w.text.len();
        }
    }

    /// Returns a text area's contents, or `""` for a wrong or stale handle.
    pub fn textarea_text(&self, id: TextAreaId) -> &str {
        match self.widgets.get(id.0) {
            Some(Widget::TextArea(w)) => &w.text, _ => "",
        }
    }

    /// Replaces a text area's contents and resets its cursor and scroll to zero.
    pub fn set_textarea_text(&mut self, id: TextAreaId, text: &str) {
        if let Some(Widget::TextArea(w)) = self.widgets.get_mut(id.0) {
            w.text.clear(); w.text.push_str(text);
            w.cursor = 0; w.scroll_y = 0;
        }
    }

    /// Appends `text`, moves the cursor to the end, and scrolls to the bottom.
    pub fn append_textarea_text(&mut self, id: TextAreaId, text: &str) {
        let lh = self.line_h();
        if let Some(Widget::TextArea(w)) = self.widgets.get_mut(id.0) {
            w.text.push_str(text);
            w.cursor = w.text.len();
            let n_lines = w.text.split('\n').count();
            let n_vis = ((w.height as i32 - 4) / lh).max(1) as usize;
            w.scroll_y = n_lines.saturating_sub(n_vis);
        }
    }

    /// Returns the first visible logical line, or zero for an invalid handle.
    pub fn textarea_scroll(&self, id: TextAreaId) -> usize {
        match self.widgets.get(id.0) {
            Some(Widget::TextArea(w)) => w.scroll_y, _ => 0,
        }
    }

    /// Sets the first visible logical line, clamped to the final line.
    ///
    /// This clamps to the content rather than to the last full viewport.
    pub fn set_textarea_scroll(&mut self, id: TextAreaId, line: usize) {
        if let Some(Widget::TextArea(w)) = self.widgets.get_mut(id.0) {
            let n_lines = w.text.split('\n').count();
            w.scroll_y = line.min(n_lines.saturating_sub(1));
        }
    }

    /// Returns whether a checkbox is checked, or `false` for an invalid handle.
    pub fn checkbox_checked(&self, id: CheckBoxId) -> bool {
        matches!(self.widgets.get(id.0), Some(Widget::CheckBox(w)) if w.checked)
    }

    /// Sets a checkbox without firing its change callback.
    pub fn set_checkbox_checked(&mut self, id: CheckBoxId, checked: bool) {
        if let Some(Widget::CheckBox(w)) = self.widgets.get_mut(id.0) { w.checked = checked; }
    }

    /// Returns a combo box's selected index, or zero for an invalid handle.
    pub fn combobox_selected(&self, id: ComboBoxId) -> usize {
        match self.widgets.get(id.0) {
            Some(Widget::ComboBox(w)) => w.selected, _ => 0,
        }
    }

    /// Selects an index, clamped to the final option.
    ///
    /// An empty option list stores index zero. No change callback is fired.
    pub fn set_combobox_selected(&mut self, id: ComboBoxId, sel: usize) {
        if let Some(Widget::ComboBox(w)) = self.widgets.get_mut(id.0) {
            w.selected = sel.min(w.options.len().saturating_sub(1));
        }
    }

    /// Returns a list box's selected index, or `None` if unset or invalid.
    pub fn listbox_selected(&self, id: ListBoxId) -> Option<usize> {
        match self.widgets.get(id.0) {
            Some(Widget::ListBox(w)) => w.selected, _ => None,
        }
    }

    /// Replaces static list items, clears selection, and resets scrolling.
    pub fn set_listbox_items(&mut self, id: ListBoxId, items: &[&'static str]) {
        if let Some(Widget::ListBox(w)) = self.widgets.get_mut(id.0) {
            w.items = items.iter().copied().collect();
            w.selected = None; w.scroll = 0;
        }
    }

    /// Returns the handle of the selected radio button in `(win_id, group_id)`,
    /// or `None` if none is selected.
    pub fn radio_selected_in_group(&self, win_id: u32, group_id: u32) -> Option<RadioButtonId> {
        self.widgets.iter().position(|w| {
            matches!(w, Widget::RadioButton(r)
                if r.win_id == win_id && r.group_id == group_id && r.selected)
        }).map(RadioButtonId)
    }

    /// Returns a slider's value, or zero for an invalid handle.
    pub fn slider_value(&self, id: SliderId) -> i32 {
        match self.widgets.get(id.0) {
            Some(Widget::Slider(w)) => w.value, _ => 0,
        }
    }

    /// Sets and range-clamps a slider without firing its change callback.
    pub fn set_slider_value(&mut self, id: SliderId, val: i32) {
        if let Some(Widget::Slider(w)) = self.widgets.get_mut(id.0) {
            w.value = val.clamp(w.min, w.max);
        }
    }

    /// Stores `min(value, max)` and the supplied maximum.
    ///
    /// A maximum of zero is accepted. No change callback is fired.
    pub fn set_progressbar(&mut self, id: ProgressBarId, value: u32, max: u32) {
        if let Some(Widget::ProgressBar(w)) = self.widgets.get_mut(id.0) {
            w.value = value.min(max); w.max = max;
        }
    }

    /// Returns a numeric up-down value, or zero for an invalid handle.
    pub fn numeric_value(&self, id: NumericUpDownId) -> i32 {
        match self.widgets.get(id.0) {
            Some(Widget::NumericUpDown(w)) => w.value, _ => 0,
        }
    }

    /// Sets and range-clamps a numeric value without firing its callback.
    pub fn set_numeric_value(&mut self, id: NumericUpDownId, val: i32) {
        if let Some(Widget::NumericUpDown(w)) = self.widgets.get_mut(id.0) {
            w.value = val.clamp(w.min, w.max);
        }
    }

    /// Sets the increment step, clamping it to at least one.
    pub fn set_numeric_step(&mut self, id: NumericUpDownId, step: i32) {
        if let Some(Widget::NumericUpDown(w)) = self.widgets.get_mut(id.0) {
            w.step = step.max(1);
        }
    }

    /// Sets the enabled flag on an interactive widget.
    ///
    /// Disabled widgets are grayed, skipped by hit testing, and skipped by focus
    /// cycling. This method does not clear existing focus; keyboard routing for
    /// an already-focused widget does not consistently consult the enabled flag.
    /// Decorative widget handles are accepted but ignored.
    pub fn widget_set_enabled(&mut self, id: impl Into<WidgetId>, enabled: bool) {
        let idx = id.into().idx();
        match self.widgets.get_mut(idx) {
            Some(Widget::TextBox(w))       => w.enabled = enabled,
            Some(Widget::TextArea(w))      => w.enabled = enabled,
            Some(Widget::CheckBox(w))      => w.enabled = enabled,
            Some(Widget::RadioButton(w))   => w.enabled = enabled,
            Some(Widget::ComboBox(w))      => w.enabled = enabled,
            Some(Widget::ListBox(w))       => w.enabled = enabled,
            Some(Widget::Button(w))        => w.enabled = enabled,
            Some(Widget::Slider(w))        => w.enabled = enabled,
            Some(Widget::NumericUpDown(w)) => w.enabled = enabled,
            _ => {}
        }
    }

    /// Prevents text edits without changing appearance or focusability.
    ///
    /// Cursor-navigation keys continue to work.
    pub fn set_textbox_readonly(&mut self, id: TextBoxId, read_only: bool) {
        if let Some(Widget::TextBox(w)) = self.widgets.get_mut(id.0) {
            w.read_only = read_only;
        }
    }

    /// Prevents text edits without changing appearance or focusability.
    ///
    /// Cursor navigation and scrolling continue to work.
    pub fn set_textarea_readonly(&mut self, id: TextAreaId, read_only: bool) {
        if let Some(Widget::TextArea(w)) = self.widgets.get_mut(id.0) {
            w.read_only = read_only;
        }
    }

    /// Shows or hides any widget type.
    pub fn widget_set_visible(&mut self, id: impl Into<WidgetId>, visible: bool) {
        let idx = id.into().idx();
        match self.widgets.get_mut(idx) {
            Some(Widget::Label(w))         => w.visible = visible,
            Some(Widget::TextBox(w))       => w.visible = visible,
            Some(Widget::TextArea(w))      => w.visible = visible,
            Some(Widget::CheckBox(w))      => w.visible = visible,
            Some(Widget::RadioButton(w))   => w.visible = visible,
            Some(Widget::ComboBox(w))      => w.visible = visible,
            Some(Widget::ListBox(w))       => w.visible = visible,
            Some(Widget::Button(w))        => w.visible = visible,
            Some(Widget::ProgressBar(w))   => w.visible = visible,
            Some(Widget::Slider(w))        => w.visible = visible,
            Some(Widget::NumericUpDown(w)) => w.visible = visible,
            Some(Widget::GroupBox(w))      => w.visible = visible,
            Some(Widget::Separator(w))     => w.visible = visible,
            None => {}
        }
    }

    // -----------------------------------------------------------------------
    // Event loop
    // -----------------------------------------------------------------------

    /// Drives the standard event loop until input or a callback requests exit.
    ///
    /// Creates an approximately 60 Hz timer, polls input via [`InputDriver`], and calls
    /// [`WindowManager::handle`] then [`WindowManager::render`] each tick. Blocks until exit.
    /// Escape always exits. `q` or `Q` exits only when no widget is focused, and
    /// a callback can exit by setting [`EventCtx::quit`]. Closing the final
    /// visible window does not end the loop.
    ///
    /// Consumes `gop_fb`, keeping its borrow of the scoped GOP protocol valid
    /// for the entire loop. `gop_stride` is measured in pixels.
    ///
    /// For custom event sources or per-frame logic, see the module-level *Custom event loop*
    /// example and call [`WindowManager::handle`] / [`WindowManager::render`] yourself.
    pub fn run(&mut self, fb: &mut Framebuffer, drv: &mut InputDriver,
               mut gop_fb: GopFrameBuffer<'_>, gop_stride: usize) {
        let frame_timer = unsafe {
            boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None)
                .expect("Cannot create timer event")
        };
        boot::set_timer(&frame_timer, TimerTrigger::Periodic(FRAME_PERIOD_100NS))
            .expect("Cannot set timer");

        let timer_slot = drv.wait_events().len();

        loop {
            let mut wait_buf = drv.wait_events();
            wait_buf.push(unsafe { frame_timer.unsafe_clone() });
            let fired = boot::wait_for_event(&mut wait_buf).unwrap_or(timer_slot);

            let events: Vec<InputEvent> = if fired == 0 {
                drv.read_keys()
            } else {
                drv.read_ptr()
            };

            if self.handle(&events, fb, drv, &mut gop_fb, gop_stride) { break; }

            self.render(fb);
            fb.present_to(&mut gop_fb, gop_stride);
        }
    }

    /// Processes one batch of input events and reports whether the app should exit.
    ///
    /// Call once per frame after [`InputDriver::read_keys`] or [`InputDriver::read_ptr`].
    /// Right-button events are ignored. The GOP framebuffer and stride are retained
    /// only in callback contexts, where [`EventCtx::present`] or
    /// [`WindowManager::message_box`] may use them.
    ///
    /// See the module-level *Custom event loop* example for the dispatch pattern.
    pub fn handle(&mut self, events: &[InputEvent], fb: &mut Framebuffer,
                  drv: &mut InputDriver, gop_fb: &mut GopFrameBuffer<'_>, gop_stride: usize) -> bool {
        for ev in events {
            match ev {
                InputEvent::MouseMove { dx, dy } => {
                    self.cx = (self.cx + (*dx * PTR_SCALE) as i32).clamp(0, self.sw as i32 - 1);
                    self.cy = (self.cy + (*dy * PTR_SCALE) as i32).clamp(0, self.sh as i32 - 1);
                    if let Some(idx) = self.update_drag() {
                        if self.fire_on_change(idx, fb, drv, gop_fb, gop_stride) { return true; }
                    }
                }
                InputEvent::MouseAbs { x, y } => {
                    self.cx = (*x).clamp(0, self.sw as i32 - 1);
                    self.cy = (*y).clamp(0, self.sh as i32 - 1);
                    if let Some(idx) = self.update_drag() {
                        if self.fire_on_change(idx, fb, drv, gop_fb, gop_stride) { return true; }
                    }
                }
                InputEvent::LeftButton(true) => {
                    self.lbtn = true;
                    let prev_focus = self.focused_widget;
                    if let Some(idx) = self.on_press() {
                        if self.fire_on_change(idx, fb, drv, gop_fb, gop_stride) { return true; }
                    }
                    // If clicking elsewhere moved focus away from a NUD in edit mode, commit it.
                    if let Some(old) = prev_focus {
                        if self.focused_widget != prev_focus && self.commit_nud_edit(old) {
                            if self.fire_on_change(old, fb, drv, gop_fb, gop_stride) { return true; }
                        }
                    }
                }
                InputEvent::LeftButton(false) => {
                    self.lbtn = false;
                    self.slider_drag = None;
                    self.scroll_drag = None;
                    for w in &mut self.windows { w.dragging = false; }
                    for widget in &mut self.widgets {
                        if let Widget::Button(b) = widget { b.pressed = false; }
                    }
                    if let Some(armed) = self.armed_btn.take() {
                        if self.is_over_button(armed) {
                            if self.fire_btn_click(armed, fb, drv, gop_fb, gop_stride) {
                                return true;
                            }
                        }
                    }
                }
                InputEvent::Key(k) => {
                    let (cont, changed) = self.route_key(k);
                    if let Some(idx) = changed {
                        if self.fire_on_change(idx, fb, drv, gop_fb, gop_stride) { return true; }
                    }
                    if !cont { return true; }
                }
                _ => {}
            }
        }
        false
    }

    /// Displays a modal dialog and blocks until mouse or keyboard dismissal.
    ///
    /// Can only be called from inside a callback (it requires `&mut EventCtx`).
    /// The dialog busy-polls keyboard and all pointer sources while the rest of
    /// the UI is frozen. A button returns on left-button press; Enter chooses
    /// OK/Yes, and Escape returns [`MsgBoxResult::No`] for either layout.
    ///
    /// `title` and `text` must have static lifetimes. The fixed 360 × 140 dialog
    /// draws `text` as one unwrapped line.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use uefi_wm::wm::{EventCtx, MsgBoxButtons, MsgBoxResult, WindowManager};
    ///
    /// // Inside an on_click or on_change callback:
    /// |wm: &mut WindowManager, ctx: &mut EventCtx| {
    ///     match wm.message_box(ctx, "Save?", "Unsaved changes will be lost.", MsgBoxButtons::YesNo) {
    ///         MsgBoxResult::Yes => { /* save … */ ctx.quit = true; }
    ///         MsgBoxResult::No  => { /* discard */ ctx.quit = true; }
    ///         MsgBoxResult::Ok  => {}
    ///     }
    /// }
    /// # ;
    /// ```
    pub fn message_box(&mut self, ctx: &mut EventCtx,
                       title: &'static str, text: &'static str,
                       buttons: MsgBoxButtons) -> MsgBoxResult {
        self.render_base(ctx.fb);
        let background = ctx.fb.pixels().to_vec();
        msgbox_loop(
            &background, ctx.fb, ctx.drv, ctx.gop_fb, ctx.gop_stride,
            &self.font, self.ascent, self.font_px,
            title, text, &buttons, self.sw, self.sh,
            &mut self.cx, &mut self.cy, self.theme.palette(), self.theme.is_modern(),
        )
    }

    // -----------------------------------------------------------------------
    // Internal: callback firing
    // -----------------------------------------------------------------------

    fn widget_id_for(&self, idx: usize) -> Option<WidgetId> {
        match self.widgets.get(idx) {
            Some(Widget::Label(_))         => Some(WidgetId::Label(LabelId(idx))),
            Some(Widget::TextBox(_))       => Some(WidgetId::TextBox(TextBoxId(idx))),
            Some(Widget::TextArea(_))      => Some(WidgetId::TextArea(TextAreaId(idx))),
            Some(Widget::CheckBox(_))      => Some(WidgetId::CheckBox(CheckBoxId(idx))),
            Some(Widget::RadioButton(_))   => Some(WidgetId::RadioButton(RadioButtonId(idx))),
            Some(Widget::ComboBox(_))      => Some(WidgetId::ComboBox(ComboBoxId(idx))),
            Some(Widget::ListBox(_))       => Some(WidgetId::ListBox(ListBoxId(idx))),
            Some(Widget::Button(_))        => Some(WidgetId::Button(ButtonId(idx))),
            Some(Widget::ProgressBar(_))   => Some(WidgetId::ProgressBar(ProgressBarId(idx))),
            Some(Widget::Slider(_))        => Some(WidgetId::Slider(SliderId(idx))),
            Some(Widget::NumericUpDown(_)) => Some(WidgetId::NumericUpDown(NumericUpDownId(idx))),
            Some(Widget::GroupBox(_))      => Some(WidgetId::GroupBox(GroupBoxId(idx))),
            Some(Widget::Separator(_))     => Some(WidgetId::Separator(SeparatorId(idx))),
            None => None,
        }
    }

    // Commit a NUD's edit_buf in-place; returns true if the stored value changed.
    fn commit_nud_edit(&mut self, idx: usize) -> bool {
        if let Some(Widget::NumericUpDown(n)) = self.widgets.get_mut(idx) {
            if let Some(buf) = n.edit_buf.take() {
                if let Ok(v) = buf.parse::<i32>() {
                    let old = n.value;
                    n.value = v.clamp(n.min, n.max);
                    return n.value != old;
                }
            }
        }
        false
    }

    fn fire_on_change(&mut self, idx: usize, fb: &mut Framebuffer,
                      drv: &mut InputDriver, gop_fb: &mut GopFrameBuffer<'_>, gop_stride: usize) -> bool {
        self.event_source = self.widget_id_for(idx);
        let cb = self.take_on_change(idx);
        let Some(mut cb) = cb else { return false };
        let mut ctx = EventCtx { quit: false, fb, drv, gop_fb, gop_stride };
        cb(self, &mut ctx);
        let quit = ctx.quit;
        self.put_on_change(idx, cb);
        quit
    }

    fn take_on_change(&mut self, idx: usize) -> Option<Cb> {
        match self.widgets.get_mut(idx) {
            Some(Widget::TextBox(w))       => w.on_change.take(),
            Some(Widget::TextArea(w))      => w.on_change.take(),
            Some(Widget::CheckBox(w))      => w.on_change.take(),
            Some(Widget::RadioButton(w))   => w.on_change.take(),
            Some(Widget::ComboBox(w))      => w.on_change.take(),
            Some(Widget::ListBox(w))       => w.on_change.take(),
            Some(Widget::Slider(w))        => w.on_change.take(),
            Some(Widget::NumericUpDown(w)) => w.on_change.take(),
            _ => None,
        }
    }

    fn put_on_change(&mut self, idx: usize, cb: Cb) {
        match self.widgets.get_mut(idx) {
            Some(Widget::TextBox(w))       => w.on_change = Some(cb),
            Some(Widget::TextArea(w))      => w.on_change = Some(cb),
            Some(Widget::CheckBox(w))      => w.on_change = Some(cb),
            Some(Widget::RadioButton(w))   => w.on_change = Some(cb),
            Some(Widget::ComboBox(w))      => w.on_change = Some(cb),
            Some(Widget::ListBox(w))       => w.on_change = Some(cb),
            Some(Widget::Slider(w))        => w.on_change = Some(cb),
            Some(Widget::NumericUpDown(w)) => w.on_change = Some(cb),
            _ => {}
        }
    }

    fn fire_btn_click(&mut self, idx: usize, fb: &mut Framebuffer,
                      drv: &mut InputDriver, gop_fb: &mut GopFrameBuffer<'_>, gop_stride: usize) -> bool {
        self.event_source = Some(WidgetId::Button(ButtonId(idx)));
        let cb = if let Some(Widget::Button(b)) = self.widgets.get_mut(idx) {
            b.on_click.take()
        } else { None };
        let Some(mut cb) = cb else { return false };
        let mut ctx = EventCtx { quit: false, fb, drv, gop_fb, gop_stride };
        cb(self, &mut ctx);
        let quit = ctx.quit;
        if let Some(Widget::Button(b)) = self.widgets.get_mut(idx) {
            b.on_click = Some(cb);
        }
        quit
    }

    // -----------------------------------------------------------------------
    // Internal: keyboard
    // -----------------------------------------------------------------------

    fn route_key(&mut self, k: &Key) -> (bool, Option<usize>) {
        if let Key::Special(ScanCode::ESCAPE) = k { return (false, None); }

        if let Key::Printable(c) = k {
            if u16::from(*c) == 0x0009 {
                let prev_focus = self.focused_widget;
                self.cycle_focus();
                if let Some(old) = prev_focus {
                    if self.focused_widget != prev_focus && self.commit_nud_edit(old) {
                        return (true, Some(old));
                    }
                }
                return (true, None);
            }
        }

        let Some(idx) = self.focused_widget else {
            if let Key::Printable(c) = k {
                let code = u16::from(*c);
                if code == b'q' as u16 || code == b'Q' as u16 { return (false, None); }
            }
            return (true, None);
        };

        let changed = self.route_key_to_widget(idx, k);
        // After key, scroll TextArea cursor into view
        self.textarea_scroll_to_cursor(idx);
        // After key, scroll ListBox selected into view
        self.listbox_scroll_to_selected(idx);

        (true, if changed { Some(idx) } else { None })
    }

    fn route_key_to_widget(&mut self, idx: usize, k: &Key) -> bool {
        let lh = self.line_h();
        match k {
            Key::Printable(c) => {
                let code = u16::from(*c);
                match &mut self.widgets[idx] {
                    Widget::TextBox(tb) if !tb.read_only => {
                        if code == 0x0008 {
                            if tb.cursor > 0 {
                                let prev = prev_char_boundary(&tb.text, tb.cursor);
                                tb.text.drain(prev..tb.cursor);
                                tb.cursor = prev;
                                return true;
                            }
                        } else if code >= 0x0020 {
                            if let Some(ch) = char::from_u32(code as u32) {
                                let len = ch.len_utf8();
                                tb.text.insert(tb.cursor, ch);
                                tb.cursor += len;
                                return true;
                            }
                        }
                    }
                    Widget::TextArea(ta) if !ta.read_only => {
                        if code == 0x0008 {
                            if ta.cursor > 0 {
                                let prev = prev_char_boundary(&ta.text, ta.cursor);
                                ta.text.drain(prev..ta.cursor);
                                ta.cursor = prev;
                                return true;
                            }
                        } else if code == 0x000D {
                            ta.text.insert(ta.cursor, '\n');
                            ta.cursor += 1;
                            return true;
                        } else if code >= 0x0020 {
                            if let Some(ch) = char::from_u32(code as u32) {
                                let len = ch.len_utf8();
                                ta.text.insert(ta.cursor, ch);
                                ta.cursor += len;
                                return true;
                            }
                        }
                    }
                    Widget::NumericUpDown(n) if n.enabled => {
                        if code == 0x0008 {
                            // Backspace: remove last char from edit buffer
                            if let Some(buf) = &mut n.edit_buf {
                                buf.pop();
                                if buf.is_empty() { n.edit_buf = None; }
                            }
                        } else if code == 0x000D {
                            // Enter: commit edit buffer
                            if let Some(buf) = n.edit_buf.take() {
                                if let Ok(v) = buf.parse::<i32>() {
                                    n.value = v.clamp(n.min, n.max);
                                    return true;
                                }
                            }
                        } else if let Some(ch) = char::from_u32(code as u32) {
                            let is_minus = ch == '-'
                                && n.edit_buf.as_deref().unwrap_or("").is_empty()
                                && n.min < 0;
                            if ch.is_ascii_digit() || is_minus {
                                n.edit_buf.get_or_insert_with(String::new).push(ch);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Key::Special(sc) => {
                if *sc == ScanCode::DELETE {
                    match &mut self.widgets[idx] {
                        Widget::TextBox(tb) if !tb.read_only => {
                            if tb.cursor < tb.text.len() {
                                let next = next_char_boundary(&tb.text, tb.cursor);
                                tb.text.drain(tb.cursor..next);
                                return true;
                            }
                        }
                        Widget::TextArea(ta) if !ta.read_only => {
                            if ta.cursor < ta.text.len() {
                                let next = next_char_boundary(&ta.text, ta.cursor);
                                ta.text.drain(ta.cursor..next);
                                return true;
                            }
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::LEFT {
                    match &mut self.widgets[idx] {
                        Widget::TextBox(tb) => {
                            if tb.cursor > 0 {
                                tb.cursor = prev_char_boundary(&tb.text, tb.cursor);
                            }
                        }
                        Widget::TextArea(ta) => {
                            if ta.cursor > 0 {
                                ta.cursor = prev_char_boundary(&ta.text, ta.cursor);
                            }
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::RIGHT {
                    match &mut self.widgets[idx] {
                        Widget::TextBox(tb) => {
                            if tb.cursor < tb.text.len() {
                                tb.cursor = next_char_boundary(&tb.text, tb.cursor);
                            }
                        }
                        Widget::TextArea(ta) => {
                            if ta.cursor < ta.text.len() {
                                ta.cursor = next_char_boundary(&ta.text, ta.cursor);
                            }
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::HOME {
                    match &mut self.widgets[idx] {
                        Widget::TextBox(tb) => { tb.cursor = 0; }
                        Widget::TextArea(ta) => {
                            ta.cursor = ta.text[..ta.cursor].rfind('\n')
                                .map(|i| i + 1).unwrap_or(0);
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::END {
                    match &mut self.widgets[idx] {
                        Widget::TextBox(tb) => { tb.cursor = tb.text.len(); }
                        Widget::TextArea(ta) => {
                            let pos = ta.cursor;
                            let nl = ta.text[pos..].find('\n').unwrap_or(ta.text.len() - pos);
                            ta.cursor = pos + nl;
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::UP {
                    match &mut self.widgets[idx] {
                        Widget::ComboBox(cb) => {
                            if cb.selected > 0 { cb.selected -= 1; return true; }
                        }
                        Widget::ListBox(lb) => {
                            let old = lb.selected;
                            lb.selected = match lb.selected {
                                Some(s) if s > 0 => Some(s - 1),
                                None if !lb.items.is_empty() => Some(0),
                                other => other,
                            };
                            return lb.selected != old;
                        }
                        Widget::Slider(s) => {
                            if s.value > s.min { s.value -= 1; return true; }
                        }
                        Widget::NumericUpDown(n) => {
                            if let Some(buf) = n.edit_buf.take() {
                                if let Ok(v) = buf.parse::<i32>() { n.value = v.clamp(n.min, n.max); }
                            }
                            if n.value + n.step <= n.max { n.value += n.step; return true; }
                        }
                        Widget::TextArea(ta) => {
                            let pos = ta.cursor;
                            let (ln, col) = offset_to_lc(&ta.text, pos);
                            if ln > 0 { ta.cursor = lc_to_offset(&ta.text, ln - 1, col); }
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::DOWN {
                    match &mut self.widgets[idx] {
                        Widget::ComboBox(cb) => {
                            if cb.selected + 1 < cb.options.len() {
                                cb.selected += 1; return true;
                            }
                        }
                        Widget::ListBox(lb) => {
                            let old = lb.selected;
                            let n = lb.items.len();
                            lb.selected = match lb.selected {
                                Some(s) if s + 1 < n => Some(s + 1),
                                None if !lb.items.is_empty() => Some(0),
                                other => other,
                            };
                            return lb.selected != old;
                        }
                        Widget::Slider(s) => {
                            if s.value < s.max { s.value += 1; return true; }
                        }
                        Widget::NumericUpDown(n) => {
                            if let Some(buf) = n.edit_buf.take() {
                                if let Ok(v) = buf.parse::<i32>() { n.value = v.clamp(n.min, n.max); }
                            }
                            if n.value - n.step >= n.min { n.value -= n.step; return true; }
                        }
                        Widget::TextArea(ta) => {
                            let pos = ta.cursor;
                            let (ln, col) = offset_to_lc(&ta.text, pos);
                            ta.cursor = lc_to_offset(&ta.text, ln + 1, col);
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::PAGE_UP {
                    match &mut self.widgets[idx] {
                        Widget::ListBox(lb) => {
                            let n_vis = (lb.height / LIST_ITEM) as usize;
                            lb.scroll = lb.scroll.saturating_sub(n_vis);
                            lb.selected = lb.selected.map(|s| s.saturating_sub(n_vis));
                            return true;
                        }
                        Widget::TextArea(ta) => {
                            let n_vis = ((ta.height as i32 - 4) / lh).max(1) as usize;
                            ta.scroll_y = ta.scroll_y.saturating_sub(n_vis);
                            let (ln, col) = offset_to_lc(&ta.text, ta.cursor);
                            ta.cursor = lc_to_offset(&ta.text, ln.saturating_sub(n_vis), col);
                        }
                        _ => {}
                    }
                } else if *sc == ScanCode::PAGE_DOWN {
                    match &mut self.widgets[idx] {
                        Widget::ListBox(lb) => {
                            let n = lb.items.len();
                            let n_vis = (lb.height / LIST_ITEM) as usize;
                            lb.scroll = (lb.scroll + n_vis).min(n.saturating_sub(n_vis));
                            lb.selected = lb.selected
                                .map(|s| (s + n_vis).min(n.saturating_sub(1)));
                            return true;
                        }
                        Widget::TextArea(ta) => {
                            let n_lines = ta.text.split('\n').count();
                            let n_vis = ((ta.height as i32 - 4) / lh).max(1) as usize;
                            ta.scroll_y = (ta.scroll_y + n_vis)
                                .min(n_lines.saturating_sub(n_vis));
                            let (ln, col) = offset_to_lc(&ta.text, ta.cursor);
                            ta.cursor = lc_to_offset(
                                &ta.text,
                                (ln + n_vis).min(n_lines.saturating_sub(1)),
                                col,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        false
    }

    fn textarea_scroll_to_cursor(&mut self, idx: usize) {
        let (cursor, height, scroll_y) = match self.widgets.get(idx) {
            Some(Widget::TextArea(ta)) => (ta.cursor, ta.height, ta.scroll_y),
            _ => return,
        };
        let lh = self.line_h();
        let n_vis = ((height as i32 - 4) / lh).max(1) as usize;
        let (ln, _) = offset_to_lc(
            match &self.widgets[idx] { Widget::TextArea(ta) => &ta.text, _ => return },
            cursor,
        );
        let new_scroll = if ln < scroll_y {
            ln
        } else if ln >= scroll_y + n_vis {
            ln + 1 - n_vis
        } else {
            scroll_y
        };
        if new_scroll != scroll_y {
            if let Widget::TextArea(ta) = &mut self.widgets[idx] {
                ta.scroll_y = new_scroll;
            }
        }
        let _ = height; // suppress unused warning
    }

    fn listbox_scroll_to_selected(&mut self, idx: usize) {
        let (selected, height, scroll) = match self.widgets.get(idx) {
            Some(Widget::ListBox(lb)) => (lb.selected, lb.height, lb.scroll),
            _ => return,
        };
        let Some(sel) = selected else { return };
        let n_vis = (height / LIST_ITEM) as usize;
        let new_scroll = if sel < scroll {
            sel
        } else if sel >= scroll + n_vis {
            sel + 1 - n_vis
        } else {
            scroll
        };
        if new_scroll != scroll {
            if let Widget::ListBox(lb) = &mut self.widgets[idx] {
                lb.scroll = new_scroll;
            }
        }
    }

    fn cycle_focus(&mut self) {
        let win_id = match self.windows.iter().rev().find(|w| w.vis) {
            Some(w) => w.id, None => return,
        };
        let candidates: Vec<usize> = self.widgets.iter().enumerate()
            .filter(|(_, w)| w.win_id() == win_id && w.is_focusable()
                          && w.is_visible() && w.is_enabled())
            .map(|(i, _)| i).collect();
        if candidates.is_empty() { return; }
        let next = match self.focused_widget {
            Some(cur) => {
                let pos = candidates.iter().position(|&i| i == cur).unwrap_or(0);
                candidates[(pos + 1) % candidates.len()]
            }
            None => candidates[0],
        };
        self.focused_widget = Some(next);
    }

    // -----------------------------------------------------------------------
    // Internal: mouse
    // -----------------------------------------------------------------------

    fn update_drag(&mut self) -> Option<usize> {
        if !self.lbtn { return None; }
        let cx = self.cx;
        let cy = self.cy;
        if let Some(w) = self.windows.last_mut() {
            if w.dragging { w.x = cx - w.drag_ox; w.y = cy - w.drag_oy; }
        }
        if let Some(slider_idx) = self.slider_drag {
            if let Some(new_val) = self.slider_value_at_x(slider_idx, cx) {
                if let Widget::Slider(s) = &mut self.widgets[slider_idx] {
                    if s.value != new_val { s.value = new_val; return Some(slider_idx); }
                }
            }
        }
        if let Some((widget_idx, drag_start_cy, scroll_start)) = self.scroll_drag {
            self.update_scroll_drag(widget_idx, cy, drag_start_cy, scroll_start);
        }
        None
    }

    fn update_scroll_drag(&mut self, idx: usize, cy: i32, drag_start_cy: i32, scroll_start: usize) {
        let lh = self.line_h();
        let (new_scroll, is_list) = match self.widgets.get(idx) {
            Some(Widget::ListBox(lb)) => {
                let n = lb.items.len();
                let n_vis = (lb.height / LIST_ITEM) as usize;
                if n <= n_vis { return; }
                let th = ((n_vis * lb.height as usize) / n).max(8);
                let track_h = (lb.height as usize).saturating_sub(th);
                if track_h == 0 { return; }
                let max_scroll = n - n_vis;
                let ns = ((scroll_start as i32
                    + (cy - drag_start_cy) * max_scroll as i32 / track_h as i32)
                    .max(0) as usize).min(max_scroll);
                (ns, true)
            }
            Some(Widget::TextArea(ta)) => {
                let n_lines = ta.text.split('\n').count();
                let height = ta.height;
                let n_vis = ((height as i32 - 4) / lh).max(1) as usize;
                if n_lines <= n_vis { return; }
                let th = ((n_vis * height as usize) / n_lines).max(8);
                let track_h = (height as usize).saturating_sub(th);
                if track_h == 0 { return; }
                let max_scroll = n_lines - n_vis;
                let ns = ((scroll_start as i32
                    + (cy - drag_start_cy) * max_scroll as i32 / track_h as i32)
                    .max(0) as usize).min(max_scroll);
                (ns, false)
            }
            _ => return,
        };
        if is_list {
            if let Widget::ListBox(lb) = &mut self.widgets[idx] { lb.scroll = new_scroll; }
        } else {
            if let Widget::TextArea(ta) = &mut self.widgets[idx] { ta.scroll_y = new_scroll; }
        }
    }

    fn slider_value_at_x(&self, idx: usize, cx: i32) -> Option<i32> {
        let (win_id, rel_x, label, width, min, max) = match self.widgets.get(idx) {
            Some(Widget::Slider(s)) => (s.win_id, s.rel_x, s.label, s.width, s.min, s.max),
            _ => return None,
        };
        let win = self.windows.iter().find(|w| w.id == win_id && w.vis && !w.minimized)?;
        let (ox, _) = win.client_origin(self.theme.metrics());
        let lw = self.label_px(label);
        let track_x = ox + rel_x + lw;
        let track_w = width as i32 - SLIDER_THW;
        if track_w <= 0 { return None; }
        let t = (cx - track_x).clamp(0, track_w);
        Some((min + t * (max - min) / track_w).clamp(min, max))
    }

    fn on_press(&mut self) -> Option<usize> {
        let (cx, cy) = (self.cx, self.cy);

        // Open combo dropdown hit
        for i in 0..self.widgets.len() {
            let is_open_combo = matches!(&self.widgets[i], Widget::ComboBox(c) if c.open);
            if !is_open_combo { continue; }
            let (win_id, lbl, width, n, rx, ry) = match &self.widgets[i] {
                Widget::ComboBox(c) =>
                    (c.win_id, c.label, c.width, c.options.len(), c.rel_x, c.rel_y),
                _ => unreachable!(),
            };
            let lw = self.label_px(lbl);
            let hit_item = self.windows.iter()
                .find(|w| w.id == win_id)
                .and_then(|win| {
                    if win.minimized || !win.vis { return None; }
                    let (ox, oy) = win.client_origin(self.theme.metrics());
                    let bx = ox + rx + lw;
                    let by = oy + ry + self.theme.metrics().combo_h as i32;
                    (0..n as i32).find(|&item| {
                        let iy = by + item * self.theme.metrics().combo_h as i32;
                        cx >= bx && cx < bx + width as i32
                            && cy >= iy && cy < iy + self.theme.metrics().combo_h as i32
                    }).map(|item| item as usize)
                });
            if let Widget::ComboBox(c) = &mut self.widgets[i] {
                let old = c.selected;
                if let Some(item) = hit_item { c.selected = item; }
                c.open = false;
                return if hit_item.map_or(false, |item| item != old) { Some(i) } else { None };
            }
            return None;
        }

        // Window chrome
        for i in (0..self.windows.len()).rev() {
            let w = &self.windows[i];
            if !w.vis { continue; }
            if w.hit_close(cx, cy, self.theme.metrics()) { self.windows[i].vis = false; return None; }
            if w.hit_minimize(cx, cy, self.theme.metrics()) {
                self.windows[i].minimized = !self.windows[i].minimized; return None;
            }
            if w.hit_body(cx, cy, self.theme.metrics()) { break; }
        }

        let hit_idx = self.windows.iter().rposition(|w| w.vis && w.hit_body(cx, cy, self.theme.metrics()))?;
        let in_title = {
            let w = &self.windows[hit_idx];
            w.hit_title(cx, cy, self.theme.metrics())
                && !w.hit_close(cx, cy, self.theme.metrics())
                && !w.hit_minimize(cx, cy, self.theme.metrics())
        };
        let win = self.windows.remove(hit_idx);
        let win_id = win.id;
        self.windows.push(win);

        if in_title {
            let w = self.windows.last_mut().unwrap();
            w.dragging = true; w.drag_ox = cx - w.x; w.drag_oy = cy - w.y;
            return None;
        }

        let hit_widget = {
            let win = self.windows.last().unwrap();
            if win.minimized { None }
            else {
                let (ox, oy) = win.client_origin(self.theme.metrics());
                self.widget_hit(cx, cy, win_id, ox, oy)
            }
        };

        match hit_widget {
            Some((i, WidgetHit::Focus)) => { self.focused_widget = Some(i); None }
            Some((i, WidgetHit::TextCursor(click_x, click_y))) => {
                self.focused_widget = Some(i);
                let (ox, oy) = self.windows.last().unwrap().client_origin(self.theme.metrics());
                let is_tb = matches!(&self.widgets[i], Widget::TextBox(_));
                let is_ta = matches!(&self.widgets[i], Widget::TextArea(_));
                if is_tb {
                    let new_cur = self.textbox_cursor_at(i, ox, click_x);
                    if let Widget::TextBox(tb) = &mut self.widgets[i] { tb.cursor = new_cur; }
                } else if is_ta {
                    let new_cur = self.textarea_cursor_at(i, ox, oy, click_x, click_y);
                    if let Widget::TextArea(ta) = &mut self.widgets[i] { ta.cursor = new_cur; }
                }
                None
            }
            Some((i, WidgetHit::Toggle)) => {
                if let Widget::CheckBox(c) = &mut self.widgets[i] { c.checked = !c.checked; }
                Some(i)
            }
            Some((i, WidgetHit::ComboToggle)) => {
                if let Widget::ComboBox(c) = &mut self.widgets[i] { c.open = !c.open; }
                self.focused_widget = Some(i);
                None
            }
            Some((i, WidgetHit::Press)) => {
                if let Widget::Button(b) = &mut self.widgets[i] { b.pressed = true; }
                self.armed_btn = Some(i);
                None
            }
            Some((i, WidgetHit::RadioSelect)) => {
                let (grp, wid) = match &self.widgets[i] {
                    Widget::RadioButton(r) => (r.group_id, r.win_id), _ => return None,
                };
                for w in &mut self.widgets {
                    if let Widget::RadioButton(r) = w {
                        if r.win_id == wid && r.group_id == grp { r.selected = false; }
                    }
                }
                if let Widget::RadioButton(r) = &mut self.widgets[i] { r.selected = true; }
                self.focused_widget = Some(i);
                Some(i)
            }
            Some((i, WidgetHit::ListSelect(item_idx))) => {
                if let Widget::ListBox(lb) = &mut self.widgets[i] {
                    let old = lb.selected;
                    lb.selected = Some(item_idx);
                    self.focused_widget = Some(i);
                    if old != Some(item_idx) { return Some(i); }
                }
                None
            }
            Some((i, WidgetHit::SliderPress)) => {
                self.slider_drag = Some(i);
                self.focused_widget = Some(i);
                if let Some(new_val) = self.slider_value_at_x(i, cx) {
                    if let Widget::Slider(s) = &mut self.widgets[i] {
                        if s.value != new_val { s.value = new_val; return Some(i); }
                    }
                }
                None
            }
            Some((i, WidgetHit::NudUp)) => {
                let changed = if let Widget::NumericUpDown(n) = &mut self.widgets[i] {
                    if let Some(buf) = n.edit_buf.take() {
                        if let Ok(v) = buf.parse::<i32>() { n.value = v.clamp(n.min, n.max); }
                    }
                    if n.enabled && n.value + n.step <= n.max { n.value += n.step; true } else { false }
                } else { false };
                if changed { Some(i) } else { None }
            }
            Some((i, WidgetHit::NudDown)) => {
                let changed = if let Widget::NumericUpDown(n) = &mut self.widgets[i] {
                    if let Some(buf) = n.edit_buf.take() {
                        if let Ok(v) = buf.parse::<i32>() { n.value = v.clamp(n.min, n.max); }
                    }
                    if n.enabled && n.value - n.step >= n.min { n.value -= n.step; true } else { false }
                } else { false };
                if changed { Some(i) } else { None }
            }
            Some((i, WidgetHit::ScrollBarThumb)) => {
                let cur_scroll = match &self.widgets[i] {
                    Widget::ListBox(lb) => lb.scroll,
                    Widget::TextArea(ta) => ta.scroll_y,
                    _ => 0,
                };
                self.scroll_drag = Some((i, cy, cur_scroll));
                self.focused_widget = Some(i);
                None
            }
            Some((i, WidgetHit::ScrollBarTrack(above))) => {
                let lh = self.line_h();
                match &mut self.widgets[i] {
                    Widget::ListBox(lb) => {
                        let n = lb.items.len();
                        let n_vis = (lb.height / LIST_ITEM) as usize;
                        if above {
                            lb.scroll = lb.scroll.saturating_sub(n_vis);
                        } else {
                            lb.scroll = (lb.scroll + n_vis).min(n.saturating_sub(n_vis));
                        }
                    }
                    Widget::TextArea(ta) => {
                        let n_lines = ta.text.split('\n').count();
                        let n_vis = ((ta.height as i32 - 4) / lh).max(1) as usize;
                        if above {
                            ta.scroll_y = ta.scroll_y.saturating_sub(n_vis);
                        } else {
                            ta.scroll_y = (ta.scroll_y + n_vis).min(n_lines.saturating_sub(n_vis));
                        }
                    }
                    _ => {}
                }
                self.focused_widget = Some(i);
                None
            }
            None => {
                self.focused_widget = self.widgets.iter().position(
                    |w| w.win_id() == win_id && matches!(w, Widget::TextBox(_))
                );
                None
            }
        }
    }

    fn widget_hit(&self, cx: i32, cy: i32, win_id: u32, ox: i32, oy: i32)
        -> Option<(usize, WidgetHit)>
    {
        for (i, widget) in self.widgets.iter().enumerate() {
            if widget.win_id() != win_id { continue; }
            if !widget.is_visible() || !widget.is_enabled() { continue; }
            let (rx, ry) = widget.rel_pos();
            match widget {
                Widget::TextBox(tb) => {
                    let bx = ox + rx + self.label_px(tb.label);
                    let by = oy + ry;
                    if cx >= bx && cx < bx + tb.width as i32
                        && cy >= by && cy < by + self.theme.metrics().textbox_h as i32 {
                        return Some((i, WidgetHit::TextCursor(cx, cy)));
                    }
                }
                Widget::TextArea(ta) => {
                    let bx = ox + rx; let by = oy + ry;
                    if cx < bx || cx >= bx + ta.width as i32
                        || cy < by || cy >= by + ta.height as i32 { continue; }
                    let n_lines = ta.text.split('\n').count();
                    let lh = self.line_h();
                    let n_vis = ((ta.height as i32 - 4) / lh).max(1) as usize;
                    if n_lines > n_vis {
                        let sb_x = bx + ta.width as i32 - 8;
                        if cx >= sb_x {
                            let th = ((n_vis * ta.height as usize) / n_lines).max(8) as i32;
                            let ty = (ta.scroll_y * (ta.height as usize - th as usize)
                                / (n_lines - n_vis).max(1)) as i32;
                            return Some((i, if cy >= by + ty && cy < by + ty + th {
                                WidgetHit::ScrollBarThumb
                            } else if cy < by + ty {
                                WidgetHit::ScrollBarTrack(true)
                            } else {
                                WidgetHit::ScrollBarTrack(false)
                            }));
                        }
                    }
                    return Some((i, WidgetHit::TextCursor(cx, cy)));
                }
                Widget::CheckBox(_) => {
                    let bx = ox + rx; let by = oy + ry;
                    if cx >= bx && cy >= by && cy < by + self.theme.metrics().checkbox_sz as i32 {
                        return Some((i, WidgetHit::Toggle));
                    }
                }
                Widget::RadioButton(_) => {
                    let bx = ox + rx; let by = oy + ry;
                    let diam = RADIO_R * 2 + 2;
                    if cx >= bx && cy >= by && cx < bx + diam + 60 && cy < by + diam {
                        return Some((i, WidgetHit::RadioSelect));
                    }
                }
                Widget::ComboBox(cb) => {
                    let bx = ox + rx + self.label_px(cb.label);
                    let by = oy + ry;
                    if cx >= bx && cx < bx + cb.width as i32
                        && cy >= by && cy < by + self.theme.metrics().combo_h as i32 {
                        return Some((i, WidgetHit::ComboToggle));
                    }
                }
                Widget::ListBox(lb) => {
                    let bx = ox + rx; let by = oy + ry;
                    if cx < bx || cx >= bx + lb.width as i32
                        || cy < by || cy >= by + lb.height as i32 { continue; }
                    let content_w = lb.width.saturating_sub(12);
                    let n = lb.items.len();
                    let n_vis = (lb.height / LIST_ITEM) as usize;
                    if cx >= bx + content_w as i32 && n > n_vis {
                        // Scrollbar column
                        let th = ((n_vis * lb.height as usize) / n).max(8) as i32;
                        let ty = (lb.scroll * (lb.height as usize - th as usize)
                            / (n - n_vis).max(1)) as i32;
                        return Some((i, if cy >= by + ty && cy < by + ty + th {
                            WidgetHit::ScrollBarThumb
                        } else if cy < by + ty {
                            WidgetHit::ScrollBarTrack(true)
                        } else {
                            WidgetHit::ScrollBarTrack(false)
                        }));
                    }
                    if cx < bx + content_w as i32 {
                        let item_y = (cy - by) / LIST_ITEM as i32;
                        let abs = lb.scroll + item_y as usize;
                        if abs < lb.items.len() {
                            return Some((i, WidgetHit::ListSelect(abs)));
                        }
                    }
                }
                Widget::Button(btn) => {
                    let bx = ox + rx; let by = oy + ry;
                    if cx >= bx && cx < bx + btn.width as i32
                        && cy >= by && cy < by + self.theme.metrics().button_h as i32 {
                        return Some((i, WidgetHit::Press));
                    }
                }
                Widget::Slider(s) => {
                    let lw = self.label_px(s.label);
                    let bx = ox + rx + lw; let by = oy + ry;
                    if cx >= bx && cx < bx + s.width as i32
                        && cy >= by && cy < by + self.theme.metrics().slider_h as i32 {
                        return Some((i, WidgetHit::SliderPress));
                    }
                }
                Widget::NumericUpDown(n) => {
                    let lw = self.label_px(n.label);
                    let val_x = ox + rx + lw;
                    let btn_x = val_x + n.width as i32;
                    let by    = oy + ry;
                    if cx >= btn_x && cx < btn_x + NUD_BTN_W
                        && cy >= by && cy < by + self.theme.metrics().nud_h as i32 {
                        if cy < by + self.theme.metrics().nud_h as i32 / 2 {
                            return Some((i, WidgetHit::NudUp));
                        } else {
                            return Some((i, WidgetHit::NudDown));
                        }
                    }
                    // Click on the value box focuses it
                    if cx >= val_x && cx < btn_x && cy >= by && cy < by + self.theme.metrics().nud_h as i32 {
                        return Some((i, WidgetHit::Focus));
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn is_over_button(&self, idx: usize) -> bool {
        let Widget::Button(btn) = &self.widgets[idx] else { return false };
        let Some(win) = self.windows.iter()
            .find(|w| w.id == btn.win_id && w.vis && !w.minimized) else { return false };
        let (ox, oy) = win.client_origin(self.theme.metrics());
        self.cx >= ox + btn.rel_x
            && self.cx < ox + btn.rel_x + btn.width as i32
            && self.cy >= oy + btn.rel_y
            && self.cy < oy + btn.rel_y + self.theme.metrics().button_h as i32
    }

    fn border_raised(&self, fb: &mut Framebuffer, x: i32, y: i32, w: u32, h: u32) {
        let p = self.theme.palette();
        if self.theme.is_modern() {
            fb.rect_outline(x, y, w, h, fb.pack(p.frame));
        } else {
            fb.border_raised_with(x, y, w, h, p.frame, p.highlight, p.shadow, p.dark_shadow);
        }
    }

    fn border_sunken(&self, fb: &mut Framebuffer, x: i32, y: i32, w: u32, h: u32) {
        let p = self.theme.palette();
        if self.theme.is_modern() {
            fb.rect_outline(x, y, w, h, fb.pack(p.shadow));
        } else {
            fb.border_sunken_with(x, y, w, h, p.highlight, p.shadow, p.dark_shadow);
        }
    }

    fn cursor_in(&self, x: i32, y: i32, w: u32, h: u32) -> bool {
        Window::hit(self.cx, self.cy, (x, y, w, h))
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    /// Draws the desktop, visible windows and widgets, and cursor into `fb`.
    ///
    /// Call once per frame after [`WindowManager::handle`]. Flush to the screen with
    /// [`Framebuffer::present_to`].
    pub fn render(&self, fb: &mut Framebuffer) {
        self.render_base(fb);
        if self.theme.is_modern() {
            draw_cursor_modern(fb, self.cx, self.cy);
        } else {
            draw_cursor(fb, self.cx, self.cy);
        }
    }

    fn render_base(&self, fb: &mut Framebuffer) {
        fb.fill(0, 0, fb.width(), fb.height(), fb.pack(self.theme.palette().desktop));

        let wn = self.windows.len();
        for (i, win) in self.windows.iter().enumerate() {
            if !win.vis { continue; }
            let focused = i == wn - 1;
            self.draw_window(fb, win, focused);
            if !win.minimized {
                for (wi, w) in self.widgets.iter().enumerate() {
                    if w.win_id() != win.id { continue; }
                    let wf = self.focused_widget == Some(wi) && focused;
                    self.draw_widget(fb, w, win, wf);
                }
            }
        }

        // Combo dropdowns drawn on top of everything
        for (wi, widget) in self.widgets.iter().enumerate() {
            if let Widget::ComboBox(cb) = widget {
                if !cb.open { continue; }
                if let Some(win) = self.windows.last()
                    .filter(|w| w.id == cb.win_id && w.vis)
                {
                    self.draw_combo_dropdown(fb, cb, win, self.focused_widget == Some(wi));
                }
            }
        }

    }

    // -----------------------------------------------------------------------
    // Window chrome
    // -----------------------------------------------------------------------

    fn draw_window(&self, fb: &mut Framebuffer, w: &Window, focused: bool) {
        if self.theme.is_modern() {
            self.draw_window_modern(fb, w, focused);
            return;
        }
        let vis_h = w.visible_height(self.theme.metrics());
        let face = fb.pack(self.theme.palette().face);
        fb.fill(w.x, w.y, w.w, vis_h, face);

        let fr = fb.pack(self.theme.palette().frame);
        let hi = fb.pack(self.theme.palette().highlight);
        let sh = fb.pack(self.theme.palette().shadow);
        let dk = fb.pack(self.theme.palette().dark_shadow);
        fb.fill(w.x,                  w.y,                    w.w, 1,     fr);
        fb.fill(w.x,                  w.y,                    1,   vis_h, fr);
        fb.fill(w.x + w.w as i32 - 1, w.y,                    1,   vis_h, dk);
        fb.fill(w.x,                  w.y + vis_h as i32 - 1, w.w, 1,     dk);
        fb.fill(w.x + 1,              w.y + 1,                w.w - 2, 1,       hi);
        fb.fill(w.x + 1,              w.y + 1,                1, vis_h - 2,     hi);
        fb.fill(w.x + w.w as i32 - 2, w.y + 1,                1, vis_h - 2,     sh);
        fb.fill(w.x + 1,              w.y + vis_h as i32 - 2, w.w - 2, 1,       sh);
        fb.fill(w.x + 2,              w.y + 2,                w.w - 4, 1,       face);
        fb.fill(w.x + 2,              w.y + 2,                1, vis_h - 4,     face);
        fb.fill(w.x + w.w as i32 - 3, w.y + 2,                1, vis_h - 4,     sh);
        fb.fill(w.x + 2,              w.y + vis_h as i32 - 3, w.w - 4, 1,       sh);

        let (tx, ty, tw, th) = w.title_rect(self.theme.metrics());
        let (tl, tr, tc) = if focused {
            (self.theme.palette().active_l, self.theme.palette().active_r, self.theme.palette().active_text)
        } else {
            (self.theme.palette().inactive_l, self.theme.palette().inactive_r, self.theme.palette().inactive_text)
        };
        fb.gradient_h(tx, ty, tw, th, tl, tr);

        let (cx, cy, cw, ch) = w.close_rect(self.theme.metrics());
        self.draw_capbtn_x(fb, cx, cy, cw, ch);
        let (mx, my, mw, mh) = w.maximize_rect(self.theme.metrics());
        self.draw_capbtn_sym(fb, mx, my, mw, mh, false);
        let (nx, ny, nw, nh) = w.minimize_rect(self.theme.metrics());
        self.draw_capbtn_sym(fb, nx, ny, nw, nh, true);

        let text_area_w = (nx - tx - 4).max(0) as u32;
        let title_w = self.text_w(&w.title);
        let text_off = if title_w < text_area_w as i32 { (text_area_w as i32 - title_w) / 2 } else { 0 };
        let baseline = ty + (self.theme.metrics().title_h + self.ascent) / 2;
        fb.text_aa(tx + 4 + text_off, baseline, &w.title, tc, &self.font, self.font_px);

        if !w.minimized {
            fb.fill(tx, ty + self.theme.metrics().title_h, tw, 1, sh);
            let cy2 = ty + self.theme.metrics().title_h + 1;
            let ch2 = (w.y + w.h as i32 - self.theme.metrics().border - cy2).max(0) as u32;
            fb.fill(tx, cy2, tw, ch2, face);
        }
    }

    fn draw_window_modern(&self, fb: &mut Framebuffer, w: &Window, focused: bool) {
        let p = self.theme.palette();
        let m = self.theme.metrics();
        let vis_h = w.visible_height(m);

        fb.fill(w.x + 4, w.y + 4, w.w, vis_h, fb.pack(p.dark_shadow));
        fb.fill(w.x, w.y, w.w, vis_h, fb.pack(p.face));
        fb.rect_outline(w.x, w.y, w.w, vis_h, fb.pack(p.frame));

        let (tx, ty, tw, th) = w.title_rect(m);
        let title_bg = if focused { p.active_l } else { p.inactive_l };
        let title_text = if focused { p.active_text } else { p.inactive_text };
        fb.fill(tx, ty, tw, th, fb.pack(title_bg));

        let (cx, cy, cw, ch) = w.close_rect(m);
        let (mx, my, mw, mh) = w.maximize_rect(m);
        let (nx, ny, nw, nh) = w.minimize_rect(m);
        self.draw_modern_capbtn(fb, cx, cy, cw, ch, 0, title_bg);
        self.draw_modern_capbtn(fb, mx, my, mw, mh, 1, title_bg);
        self.draw_modern_capbtn(fb, nx, ny, nw, nh, 2, title_bg);

        let baseline = ty + (m.title_h + self.ascent) / 2;
        let max_title = (nx - tx - 20).max(0);
        fb.text_aa(tx + 12, baseline, self.fit(&w.title, max_title),
                   title_text, &self.font, self.font_px);

        if !w.minimized {
            let client_y = ty + m.title_h;
            let client_h = (w.y + w.h as i32 - m.border - client_y).max(0) as u32;
            fb.fill(tx, client_y, tw, client_h, fb.pack(p.face));
        }
    }

    fn draw_modern_capbtn(&self, fb: &mut Framebuffer, x: i32, y: i32,
                          w: u32, h: u32, symbol: u8, base: Color) {
        let p = self.theme.palette();
        let hovered = self.cursor_in(x, y, w, h);
        let bg = if hovered && symbol == 0 {
            Color::rgb(0xE8, 0x11, 0x23)
        } else if hovered {
            p.highlight
        } else {
            base
        };
        fb.fill(x, y, w, h, fb.pack(bg));
        let ink = fb.pack(if hovered && symbol != 0 { p.window_text } else { p.active_text });
        let cx = x + w as i32 / 2;
        let cy = y + h as i32 / 2;
        match symbol {
            0 => {
                for i in -4..=4 {
                    fb.set_i(cx + i, cy + i, ink);
                    fb.set_i(cx + i, cy - i, ink);
                }
            }
            1 => fb.rect_outline(cx - 5, cy - 4, 10, 8, ink),
            _ => fb.fill(cx - 5, cy + 3, 10, 1, ink),
        }
    }

    fn draw_capbtn_x(&self, fb: &mut Framebuffer, x: i32, y: i32, w: u32, h: u32) {
        fb.fill(x, y, w, h, fb.pack(self.theme.palette().face));
        self.border_raised(fb, x, y, w, h);
        let k = fb.pack(self.theme.palette().window_text);
        for i in 0i32..4 {
            fb.set_i(x + 4 + i,     y + 3 + i, k);
            fb.set_i(x + 4 + i + 1, y + 3 + i, k);
            fb.set_i(x + 7 - i,     y + 3 + i, k);
            fb.set_i(x + 7 - i - 1, y + 3 + i, k);
        }
    }

    fn draw_capbtn_sym(&self, fb: &mut Framebuffer, x: i32, y: i32, w: u32, h: u32,
                       minimize: bool) {
        fb.fill(x, y, w, h, fb.pack(self.theme.palette().face));
        self.border_raised(fb, x, y, w, h);
        let k = fb.pack(self.theme.palette().window_text);
        if minimize {
            fb.fill(x + 4, y + h as i32 - 5, 7, 2, k);
        } else {
            fb.fill(x + 4, y + 3, 7, 1, k);
            fb.fill(x + 4, y + 3, 1, 7, k);
            fb.fill(x + 10, y + 3, 1, 7, k);
            fb.fill(x + 4, y + 9, 7, 1, k);
        }
    }

    // -----------------------------------------------------------------------
    // Widget dispatch
    // -----------------------------------------------------------------------

    fn draw_widget(&self, fb: &mut Framebuffer, widget: &Widget,
                   win: &Window, focused: bool) {
        if !widget.is_visible() { return; }
        let (ox, oy) = win.client_origin(self.theme.metrics());
        let enabled = widget.is_enabled();
        match widget {
            Widget::Label(w)         => self.draw_label(fb, w, ox, oy),
            Widget::TextBox(w)       => self.draw_textbox(fb, w, ox, oy, focused, enabled),
            Widget::TextArea(w)      => self.draw_textarea(fb, w, ox, oy, focused, enabled),
            Widget::CheckBox(w)      => self.draw_checkbox(fb, w, ox, oy, enabled),
            Widget::RadioButton(w)   => self.draw_radiobutton(fb, w, ox, oy, enabled),
            Widget::ComboBox(w)      => self.draw_combobox(fb, w, ox, oy, enabled),
            Widget::ListBox(w)       => self.draw_listbox(fb, w, ox, oy, focused, enabled),
            Widget::Button(w)        => self.draw_button(fb, w, ox, oy, enabled),
            Widget::ProgressBar(w)   => self.draw_progressbar(fb, w, ox, oy),
            Widget::Slider(w)        => self.draw_slider(fb, w, ox, oy, focused, enabled),
            Widget::NumericUpDown(w) => self.draw_nud(fb, w, ox, oy, focused, enabled),
            Widget::GroupBox(w)      => self.draw_groupbox(fb, w, ox, oy),
            Widget::Separator(w)     => self.draw_separator(fb, w, ox, oy),
        }
    }

    // -----------------------------------------------------------------------
    // Individual draw methods
    // -----------------------------------------------------------------------

    fn draw_label(&self, fb: &mut Framebuffer, lbl: &Label, ox: i32, oy: i32) {
        let ax = ox + lbl.rel_x;
        let ay = oy + lbl.rel_y + self.ascent;
        let text = if lbl.width > 0 {
            self.fit(&lbl.text, lbl.width as i32)
        } else {
            &lbl.text
        };
        fb.text_aa(ax, ay, text, self.theme.palette().window_text, &self.font, self.font_px);
    }

    fn draw_textbox(&self, fb: &mut Framebuffer, tb: &TextBox,
                    ox: i32, oy: i32, focused: bool, enabled: bool) {
        let ax = ox + tb.rel_x;
        let ay = oy + tb.rel_y;
        let lw = self.label_px(tb.label);
        let baseline = ay + self.ascent + (self.theme.metrics().textbox_h as i32 - self.ascent) / 2;
        let lc = if enabled { self.theme.palette().label_text } else { self.theme.palette().disabled_text };
        fb.text_aa(ax, baseline, tb.label, lc, &self.font, self.font_px);

        let bx = ax + lw;
        let bg = if enabled { self.theme.palette().window } else { self.theme.palette().face };
        fb.fill(bx, ay, tb.width, self.theme.metrics().textbox_h, fb.pack(bg));
        self.border_sunken(fb, bx, ay, tb.width, self.theme.metrics().textbox_h);
        if self.theme.is_modern()
            && enabled
            && (focused || self.cursor_in(bx, ay, tb.width, self.theme.metrics().textbox_h))
        {
            fb.rect_outline(bx, ay, tb.width, self.theme.metrics().textbox_h,
                    fb.pack(self.theme.palette().accent));
        }

        let inner_x = bx + 4;
        let max_px  = tb.width as i32 - 8;
        let cursor = tb.cursor.min(tb.text.len());
        // Scroll view so cursor is always visible
        let left_part  = self.fit_right(&tb.text[..cursor], max_px);
        let view_start = cursor - left_part.len();
        let visible    = self.fit(&tb.text[view_start..], max_px);
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        fb.text_aa(inner_x, baseline, visible, tc, &self.font, self.font_px);

        if focused && enabled && !tb.read_only {
            let cur_x = inner_x + self.text_w(left_part);
            let h = self.ascent + 3;
            fb.fill(cur_x, ay + (self.theme.metrics().textbox_h as i32 - h) / 2, 1, h as u32,
                    fb.pack(self.theme.palette().window_text));
        }
    }

    fn draw_textarea(&self, fb: &mut Framebuffer, ta: &TextArea,
                     ox: i32, oy: i32, focused: bool, enabled: bool) {
        let ax = ox + ta.rel_x;
        let ay = oy + ta.rel_y;
        let bg = if enabled { self.theme.palette().window } else { self.theme.palette().face };
        fb.fill(ax, ay, ta.width, ta.height, fb.pack(bg));
        self.border_sunken(fb, ax, ay, ta.width, ta.height);
        if self.theme.is_modern()
            && enabled
            && (focused || self.cursor_in(ax, ay, ta.width, ta.height))
        {
            fb.rect_outline(ax, ay, ta.width, ta.height, fb.pack(self.theme.palette().accent));
        }

        let lh   = self.line_h();
        let n_vis = ((ta.height as i32 - 4) / lh).max(0) as usize;
        let max_px = ta.width as i32 - 8;
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };

        let mut line_idx = 0usize;
        let mut byte_pos = 0usize;
        let mut display_row = 0i32;

        // Collect lines for rendering
        let text = &ta.text;
        for chunk in text.split('\n') {
            if line_idx >= ta.scroll_y && display_row < n_vis as i32 {
                let row_y = ay + 2 + display_row * lh + self.ascent;
                let visible = self.fit(chunk, max_px);
                fb.text_aa(ax + 4, row_y, visible, tc, &self.font, self.font_px);

                // Draw cursor if on this line
                if focused && enabled && !ta.read_only {
                    let line_start = byte_pos;
                    let line_end = byte_pos + chunk.len();
                    if ta.cursor >= line_start && ta.cursor <= line_end {
                        let col_text = &chunk[..ta.cursor - line_start];
                        let col_px = self.text_w(col_text).min(max_px);
                        let cur_x = ax + 4 + col_px;
                        fb.fill(cur_x, ay + 2 + display_row * lh,
                                1, lh as u32, fb.pack(self.theme.palette().window_text));
                    }
                }
                display_row += 1;
            }
            line_idx += 1;
            byte_pos += chunk.len() + 1; // +1 for '\n'
        }

        // Vertical scrollbar indicator
        let n_lines = text.split('\n').count();
        if n_lines > n_vis {
            let sb_x = ax + ta.width as i32 - 8;
            fb.fill(sb_x, ay, 8, ta.height, fb.pack(self.theme.palette().face));
            fb.fill(sb_x, ay, 1, ta.height, fb.pack(self.theme.palette().shadow));
            let th = ((n_vis * ta.height as usize) / n_lines).max(8) as u32;
            let ty2 = if n_lines > n_vis {
                ta.scroll_y * (ta.height as usize - th as usize) / (n_lines - n_vis)
            } else { 0 };
            fb.fill(sb_x + 1, ay + ty2 as i32, 7, th, fb.pack(self.theme.palette().shadow));
        }
    }

    fn draw_checkbox(&self, fb: &mut Framebuffer, cb: &CheckBox,
                     ox: i32, oy: i32, enabled: bool) {
        let ax = ox + cb.rel_x;
        let ay = oy + cb.rel_y;
        let baseline = ay + self.ascent + (self.theme.metrics().checkbox_sz as i32 - self.ascent) / 2;
        if self.theme.is_modern() && enabled
            && self.cursor_in(ax - 3, ay - 3,
                              (self.theme.metrics().checkbox_sz as i32 + self.text_w(cb.label) + 11) as u32,
                              self.theme.metrics().checkbox_sz + 6)
        {
            fb.fill(ax - 3, ay - 3,
                    (self.theme.metrics().checkbox_sz as i32 + self.text_w(cb.label) + 11) as u32,
                    self.theme.metrics().checkbox_sz + 6, fb.pack(self.theme.palette().highlight));
        }
        fb.fill(ax, ay, self.theme.metrics().checkbox_sz, self.theme.metrics().checkbox_sz, fb.pack(self.theme.palette().window));
        self.border_sunken(fb, ax, ay, self.theme.metrics().checkbox_sz, self.theme.metrics().checkbox_sz);

        if cb.checked {
            if self.theme.is_modern() {
                let fill = if enabled { self.theme.palette().accent }
                           else { self.theme.palette().disabled_text };
                fb.fill(ax, ay, self.theme.metrics().checkbox_sz,
                        self.theme.metrics().checkbox_sz, fb.pack(fill));
                let k = fb.pack(self.theme.palette().active_text);
                for i in 0i32..4 {
                    fb.set_i(ax + 3 + i, ay + 7 + i, k);
                    fb.set_i(ax + 6 + i, ay + 10 - i, k);
                    fb.set_i(ax + 7 + i, ay + 10 - i, k);
                }
            } else {
                let k = fb.pack(if enabled { self.theme.palette().window_text }
                                else { self.theme.palette().disabled_text });
                for i in 0i32..4 {
                    fb.set_i(ax + 2 + i,     ay + 5 + i, k);
                    fb.set_i(ax + 5 + i,     ay + 8 - i, k);
                    fb.set_i(ax + 5 + i + 1, ay + 8 - i, k);
                }
            }
        }

        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        fb.text_aa(ax + self.theme.metrics().checkbox_sz as i32 + 5, baseline, cb.label, tc, &self.font, self.font_px);
    }

    fn draw_radiobutton(&self, fb: &mut Framebuffer, rb: &RadioButton,
                        ox: i32, oy: i32, enabled: bool) {
        let ax = ox + rb.rel_x;
        let ay = oy + rb.rel_y;
        let cx = ax + RADIO_R + 1;
        let cy = ay + RADIO_R + 1;
        if self.theme.is_modern() && enabled
            && self.cursor_in(ax - 3, ay - 3, (RADIO_R * 2 + self.text_w(rb.label) + 14) as u32,
                              (RADIO_R * 2 + 8) as u32)
        {
            fb.fill(ax - 3, ay - 3, (RADIO_R * 2 + self.text_w(rb.label) + 14) as u32,
                    (RADIO_R * 2 + 8) as u32, fb.pack(self.theme.palette().highlight));
        }
        let outline = fb.pack(if enabled { self.theme.palette().shadow }
            else if self.theme.is_modern() { self.theme.palette().disabled_text }
            else { self.theme.palette().face });
        let fill_c  = fb.pack(if enabled && self.theme.is_modern() { self.theme.palette().accent }
                              else if enabled { self.theme.palette().window_text }
                              else { self.theme.palette().disabled_text });
        let bg      = fb.pack(self.theme.palette().window);

        // Draw circle via per-pixel test
        for dy in -RADIO_R..=RADIO_R {
            for dx in -RADIO_R..=RADIO_R {
                let d2 = dx * dx + dy * dy;
                let r2 = RADIO_R * RADIO_R;
                if d2 <= r2 {
                    let p = if d2 >= (RADIO_R - 1) * (RADIO_R - 1) { outline }
                            else if rb.selected && d2 <= (RADIO_R - 3) * (RADIO_R - 3) { fill_c }
                            else { bg };
                    fb.set_i(cx + dx, cy + dy, p);
                }
            }
        }

        let baseline = ay + self.ascent + (RADIO_R * 2 + 2 - self.ascent) / 2;
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        fb.text_aa(ax + RADIO_R * 2 + 6, baseline, rb.label, tc, &self.font, self.font_px);
    }

    fn draw_combobox(&self, fb: &mut Framebuffer, cb: &ComboBox,
                     ox: i32, oy: i32, enabled: bool) {
        let ax = ox + cb.rel_x;
        let ay = oy + cb.rel_y;
        let lw = self.label_px(cb.label);
        let baseline = ay + self.ascent + (self.theme.metrics().combo_h as i32 - self.ascent) / 2;
        fb.text_aa(ax, baseline, cb.label, self.theme.palette().label_text, &self.font, self.font_px);

        let bx = ax + lw;
        let bg = if enabled { self.theme.palette().window } else { self.theme.palette().face };
        fb.fill(bx, ay, cb.width, self.theme.metrics().combo_h, fb.pack(bg));
        self.border_sunken(fb, bx, ay, cb.width, self.theme.metrics().combo_h);
        if self.theme.is_modern() && enabled
            && self.cursor_in(bx, ay, cb.width, self.theme.metrics().combo_h)
        {
            fb.rect_outline(bx, ay, cb.width, self.theme.metrics().combo_h,
                    fb.pack(self.theme.palette().accent));
        }

        let text_max = (cb.width as i32 - COMBO_ARW as i32 - 6).max(0);
        let sel = cb.options.get(cb.selected).copied().unwrap_or("");
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        fb.text_aa(bx + 4, baseline, self.fit(sel, text_max), tc, &self.font, self.font_px);

        let arx = bx + cb.width as i32 - COMBO_ARW as i32;
        let arrow_bg = if self.theme.is_modern() && self.cursor_in(arx, ay, COMBO_ARW, self.theme.metrics().combo_h) {
            self.theme.palette().highlight
        } else {
            self.theme.palette().face
        };
        fb.fill(arx, ay, COMBO_ARW, self.theme.metrics().combo_h, fb.pack(arrow_bg));
        self.border_raised(fb, arx, ay, COMBO_ARW, self.theme.metrics().combo_h);
        let k2 = fb.pack(self.theme.palette().window_text);
        let tri_cx = arx + COMBO_ARW as i32 / 2;
        let tri_cy = ay + self.theme.metrics().combo_h as i32 / 2 - 1;
        if self.theme.is_modern() {
            for i in 0i32..4 {
                let dy = if cb.open { 2 - i } else { i - 2 };
                fb.set_i(tri_cx - 4 + i, tri_cy + dy, k2);
                fb.set_i(tri_cx + 4 - i, tri_cy + dy, k2);
            }
        } else {
            for row in 0i32..4 {
                // ▼ when closed, ▲ when open
                let y = if cb.open { tri_cy + (3 - row) } else { tri_cy + row };
                for col in -row..=row { fb.set_i(tri_cx + col, y, k2); }
            }
        }
    }

    fn draw_combo_dropdown(&self, fb: &mut Framebuffer, cb: &ComboBox,
                           win: &Window, _focused: bool) {
        let (ox, oy) = win.client_origin(self.theme.metrics());
        let bx = ox + cb.rel_x + self.label_px(cb.label);
        let by = oy + cb.rel_y + self.theme.metrics().combo_h as i32;
        let n  = cb.options.len() as u32;
        let dh = n * self.theme.metrics().combo_h;
        let wbg = fb.pack(self.theme.palette().window);
        let sbg = fb.pack(self.theme.palette().accent);
        fb.fill(bx, by, cb.width, dh, wbg);
        self.border_raised(fb, bx, by, cb.width, dh);

        let text_max = (cb.width as i32 - 8).max(0);
        for (i, &opt) in cb.options.iter().enumerate() {
            let iy = by + i as i32 * self.theme.metrics().combo_h as i32;
            let sel = i == cb.selected;
            fb.fill(bx + 1, iy, cb.width - 2, self.theme.metrics().combo_h, if sel { sbg } else { wbg });
            let baseline = iy + self.ascent + (self.theme.metrics().combo_h as i32 - self.ascent) / 2;
            fb.text_aa(bx + 4, baseline, self.fit(opt, text_max),
                       if sel { self.theme.palette().active_text } else { self.theme.palette().window_text },
                       &self.font, self.font_px);
        }
    }

    fn draw_listbox(&self, fb: &mut Framebuffer, lb: &ListBox,
                    ox: i32, oy: i32, _focused: bool, enabled: bool) {
        let ax = ox + lb.rel_x;
        let ay = oy + lb.rel_y;
        let content_w = lb.width.saturating_sub(12);
        let bg = if enabled { self.theme.palette().window } else { self.theme.palette().face };
        fb.fill(ax, ay, lb.width, lb.height, fb.pack(bg));
        self.border_sunken(fb, ax, ay, lb.width, lb.height);

        let n_vis = (lb.height / LIST_ITEM) as usize;
        let lh    = LIST_ITEM as i32;
        let max_px = content_w as i32 - 8;

        for row in 0..n_vis {
            let item_idx = lb.scroll + row;
            if item_idx >= lb.items.len() { break; }
            let item = lb.items[item_idx];
            let iy = ay + row as i32 * lh;
            let selected = lb.selected == Some(item_idx);
            let hovered = self.theme.is_modern() && enabled
                && self.cursor_in(ax + 2, iy, content_w - 2, LIST_ITEM);
            let bg2 = if selected && enabled { fb.pack(self.theme.palette().accent) }
                      else if hovered { fb.pack(self.theme.palette().highlight) }
                      else { fb.pack(bg) };
            fb.fill(ax + 2, iy, content_w - 2, LIST_ITEM, bg2);
            let baseline = iy + self.ascent + (lh - self.ascent) / 2;
            let tc = if selected && enabled { self.theme.palette().active_text }
                     else if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
            fb.text_aa(ax + 6, baseline, self.fit(item, max_px), tc, &self.font, self.font_px);
        }

        // Scrollbar
        let n = lb.items.len();
        if n > n_vis {
            let sb_x = ax + content_w as i32;
            fb.fill(sb_x, ay, 12, lb.height, fb.pack(self.theme.palette().face));
            fb.fill(sb_x, ay, 1, lb.height, fb.pack(self.theme.palette().shadow));
            let th = ((n_vis * lb.height as usize) / n).max(8) as u32;
            let ty2 = (lb.scroll * (lb.height as usize - th as usize)) / (n - n_vis).max(1);
            fb.fill(sb_x + 2, ay + ty2 as i32, 8, th, fb.pack(self.theme.palette().shadow));
        }
    }

    fn draw_button(&self, fb: &mut Framebuffer, btn: &Button,
                   ox: i32, oy: i32, enabled: bool) {
        let ax = ox + btn.rel_x;
        let ay = oy + btn.rel_y;
        let hovered = enabled && self.cursor_in(ax, ay, btn.width, self.theme.metrics().button_h);
        let button_bg = if self.theme.is_modern() && btn.pressed && enabled {
            self.theme.palette().accent
        } else if self.theme.is_modern() && hovered {
            self.theme.palette().highlight
        } else {
            self.theme.palette().face
        };
        fb.fill(ax, ay, btn.width, self.theme.metrics().button_h, fb.pack(button_bg));
        if btn.pressed && enabled { self.border_sunken(fb, ax, ay, btn.width, self.theme.metrics().button_h); }
        else                      { self.border_raised(fb, ax, ay, btn.width, self.theme.metrics().button_h); }

        let shift    = if btn.pressed && enabled { 1i32 } else { 0 };
        let tw       = self.text_w(btn.label);
        let off      = if tw < btn.width as i32 { (btn.width as i32 - tw) / 2 } else { 0 };
        let baseline = ay + (self.theme.metrics().button_h as i32 + self.ascent) / 2;
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        fb.text_aa(ax + off + shift, baseline + shift,
                   btn.label, tc, &self.font, self.font_px);
    }

    fn draw_progressbar(&self, fb: &mut Framebuffer, pb: &ProgressBar, ox: i32, oy: i32) {
        let ax = ox + pb.rel_x;
        let ay = oy + pb.rel_y;
        let lw = self.label_px(pb.label);
        let baseline = ay + self.ascent + (PBAR_H as i32 - self.ascent) / 2;
        fb.text_aa(ax, baseline, pb.label, self.theme.palette().label_text, &self.font, self.font_px);

        let bx = ax + lw;
        fb.fill(bx, ay, pb.width, PBAR_H, fb.pack(self.theme.palette().window));
        self.border_sunken(fb, bx, ay, pb.width, PBAR_H);

        if pb.max > 0 {
            let fill_w = ((pb.value as u64 * (pb.width as u64 - 4)) / pb.max as u64) as u32;
            if fill_w > 0 {
                fb.fill(bx + 2, ay + 2, fill_w, PBAR_H - 4, fb.pack(self.theme.palette().accent));
            }
        }
    }

    fn draw_slider(&self, fb: &mut Framebuffer, s: &Slider,
                   ox: i32, oy: i32, focused: bool, enabled: bool) {
        let ax = ox + s.rel_x;
        let ay = oy + s.rel_y;
        let lw = self.label_px(s.label);
        let baseline = ay + self.ascent + (self.theme.metrics().slider_h as i32 - self.ascent) / 2;
        fb.text_aa(ax, baseline, s.label, self.theme.palette().label_text, &self.font, self.font_px);

        let bx    = ax + lw;
        let track_w = s.width as i32 - SLIDER_THW;
        if track_w <= 0 { return; }

        // Track
        let track_y = ay + self.theme.metrics().slider_h as i32 / 2 - 2;
        fb.fill(bx, track_y, s.width, 4, fb.pack(self.theme.palette().window));
        self.border_sunken(fb, bx, track_y, s.width, 4);

        // Thumb
        let range = (s.max - s.min).max(1);
        let thumb_x = bx + (s.value - s.min) * track_w / range;
        let thumb_y = ay + 1;
        let thumb_h = self.theme.metrics().slider_h - 2;
        let thumb_hover = enabled && self.cursor_in(thumb_x, thumb_y, SLIDER_THW as u32, thumb_h);
        let face_c = if self.theme.is_modern() && thumb_hover { self.theme.palette().accent }
                     else if enabled { self.theme.palette().face }
                     else { self.theme.palette().disabled_text };
        fb.fill(thumb_x, thumb_y, SLIDER_THW as u32, thumb_h, fb.pack(face_c));
        if focused && enabled {
            self.border_sunken(fb, thumb_x, thumb_y, SLIDER_THW as u32, thumb_h);
        } else {
            self.border_raised(fb, thumb_x, thumb_y, SLIDER_THW as u32, thumb_h);
        }
    }

    fn draw_nud(&self, fb: &mut Framebuffer, n: &NumericUpDown,
                ox: i32, oy: i32, focused: bool, enabled: bool) {
        let ax = ox + n.rel_x;
        let ay = oy + n.rel_y;
        let lw = self.label_px(n.label);
        let baseline = ay + self.ascent + (self.theme.metrics().nud_h as i32 - self.ascent) / 2;
        fb.text_aa(ax, baseline, n.label, self.theme.palette().label_text, &self.font, self.font_px);

        // Value box
        let bx = ax + lw;
        let bg = if enabled { self.theme.palette().window } else { self.theme.palette().face };
        fb.fill(bx, ay, n.width, self.theme.metrics().nud_h, fb.pack(bg));
        self.border_sunken(fb, bx, ay, n.width, self.theme.metrics().nud_h);
        if self.theme.is_modern() && enabled
            && self.cursor_in(bx, ay, n.width + NUD_BTN_W as u32, self.theme.metrics().nud_h)
        {
            fb.rect_outline(bx, ay, n.width + NUD_BTN_W as u32, self.theme.metrics().nud_h,
                    fb.pack(self.theme.palette().accent));
        }

        // Value text: edit buffer (left-aligned + cursor) while typing, value (right-aligned) otherwise
        let tc = if enabled { self.theme.palette().window_text } else { self.theme.palette().disabled_text };
        if let Some(ref buf) = n.edit_buf {
            let max_px = n.width as i32 - 10;
            let visible = self.fit_right(buf, max_px);
            let ex = fb.text_aa(bx + 3, baseline, visible, tc, &self.font, self.font_px);
            fb.text_aa(ex, baseline, "|", self.theme.palette().accent, &self.font, self.font_px);
        } else {
            let mut vbuf = [0u8; 16];
            let vstr = fmt_i32(n.value, &mut vbuf);
            let vw = self.text_w(vstr);
            let vx = bx + n.width as i32 - 4 - vw;
            fb.text_aa(vx.max(bx + 2), baseline, vstr, tc, &self.font, self.font_px);
        }

        if focused && enabled {
            fb.fill(bx, ay, n.width, 1, fb.pack(self.theme.palette().accent));
            fb.fill(bx, ay + self.theme.metrics().nud_h as i32 - 1, n.width, 1, fb.pack(self.theme.palette().accent));
        }

        // Up/Down buttons
        let btn_x = bx + n.width as i32;
        let half_h = self.theme.metrics().nud_h as i32 / 2;
        let bc = fb.pack(self.theme.palette().face);
        let k  = fb.pack(self.theme.palette().window_text);
        // Up button
        fb.fill(btn_x, ay, NUD_BTN_W as u32, half_h as u32, bc);
        self.border_raised(fb, btn_x, ay, NUD_BTN_W as u32, half_h as u32);
        let tcx = btn_x + NUD_BTN_W / 2;
        let tcy = ay + half_h / 2;
        if self.theme.is_modern() {
            for i in 0i32..4 {
                fb.set_i(tcx - 4 + i, tcy + 1 - i, k);
                fb.set_i(tcx + 4 - i, tcy + 1 - i, k);
            }
        } else {
            for row in 0i32..3 { for _col in -row..=row { fb.set_i(tcx + row - 2, tcy - row + 1, k); } }
        }
        // Down button
        fb.fill(btn_x, ay + half_h, NUD_BTN_W as u32, (self.theme.metrics().nud_h as i32 - half_h) as u32, bc);
        self.border_raised(fb, btn_x, ay + half_h, NUD_BTN_W as u32, (self.theme.metrics().nud_h as i32 - half_h) as u32);
        let tcy2 = ay + half_h + (self.theme.metrics().nud_h as i32 - half_h) / 2;
        if self.theme.is_modern() {
            for i in 0i32..4 {
                fb.set_i(tcx - 4 + i, tcy2 - 1 + i, k);
                fb.set_i(tcx + 4 - i, tcy2 - 1 + i, k);
            }
        } else {
            for row in 0i32..3 { for _col in -row..=row { fb.set_i(tcx + row - 2, tcy2 + row - 1, k); } }
        }
    }

    fn draw_groupbox(&self, fb: &mut Framebuffer, gb: &GroupBox, ox: i32, oy: i32) {
        let ax = ox + gb.rel_x;
        let ay = oy + gb.rel_y;
        let lw = self.text_w(gb.label) + 8;

        // Border with gap for the label
        let sh = fb.pack(self.theme.palette().shadow);
        let hi = fb.pack(self.theme.palette().highlight);
        let by = ay + GROUPBOX_T;

        // Top: gap at label position
        fb.fill(ax,          by, 8, 1, sh);
        fb.fill(ax + lw + 4, by, gb.width - lw as u32 - 12, 1, sh);
        fb.fill(ax + 1,      by + 1, 8, 1, hi);
        fb.fill(ax + lw + 4, by + 1, gb.width - lw as u32 - 12, 1, hi);

        // Left, right, bottom
        fb.fill(ax,                      by, 1, gb.height - GROUPBOX_T as u32, sh);
        fb.fill(ax + 1,                  by, 1, gb.height - GROUPBOX_T as u32, hi);
        fb.fill(ax + gb.width as i32 - 2, by, 1, gb.height - GROUPBOX_T as u32, sh);
        fb.fill(ax + gb.width as i32 - 1, by, 1, gb.height - GROUPBOX_T as u32, hi);
        fb.fill(ax,     ay + gb.height as i32 - 2, gb.width, 1, sh);
        fb.fill(ax + 1, ay + gb.height as i32 - 1, gb.width - 2, 1, hi);

        // Label
        let lx = ax + 10;
        let ly = ay + self.ascent;
        fb.text_aa(lx, ly, gb.label, self.theme.palette().window_text, &self.font, self.font_px);
    }

    fn draw_separator(&self, fb: &mut Framebuffer, sep: &Separator, ox: i32, oy: i32) {
        let ax = ox + sep.rel_x;
        let ay = oy + sep.rel_y;
        fb.fill(ax, ay,     sep.width, 1, fb.pack(self.theme.palette().shadow));
        fb.fill(ax, ay + 1, sep.width, 1, fb.pack(self.theme.palette().highlight));
    }

    // -----------------------------------------------------------------------
    // -----------------------------------------------------------------------
    // Text helpers
    // -----------------------------------------------------------------------

    fn fit_right<'s>(&self, s: &'s str, max_px: i32) -> &'s str {
        let total = Framebuffer::text_width(s, &self.font, self.font_px);
        if total <= max_px { return s; }
        let mut start = 0usize;
        let mut w = total;
        for ch in s.chars() {
            if w <= max_px { break; }
            w -= self.font.metrics(ch, self.font_px).advance_width as i32;
            start += ch.len_utf8();
        }
        &s[start..]
    }
}

// ---------------------------------------------------------------------------
// Message-box loop
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn msgbox_loop(
    background: &[u32], fb: &mut Framebuffer, drv: &mut InputDriver,
    gop_fb: &mut GopFrameBuffer<'_>, gop_stride: usize,
    font: &fontdue::Font, ascent: i32, font_px: f32,
    title: &'static str, text: &'static str, buttons: &MsgBoxButtons,
    sw: u32, sh: u32, cx: &mut i32, cy: &mut i32,
    palette: Palette, modern: bool,
) -> MsgBoxResult {
    let dw: u32 = 360;
    let dh: u32 = 140;
    let dlg_x = (sw as i32 - dw as i32) / 2;
    let dlg_y = (sh as i32 - dh as i32) / 2;

    loop {
        let mut events = drv.read_keys();
        events.extend(drv.read_ptr());

        for ev in events {
            match ev {
                InputEvent::MouseMove { dx, dy } => {
                    *cx = (*cx + (dx * PTR_SCALE) as i32).clamp(0, sw as i32 - 1);
                    *cy = (*cy + (dy * PTR_SCALE) as i32).clamp(0, sh as i32 - 1);
                }
                InputEvent::MouseAbs { x, y } => {
                    *cx = x.clamp(0, sw as i32 - 1);
                    *cy = y.clamp(0, sh as i32 - 1);
                }
                InputEvent::LeftButton(true) => {
                    let result = match buttons {
                        MsgBoxButtons::Ok => {
                            if btn_hit(*cx, *cy, msgbox_btn_rect(dlg_x, dlg_y, dw, dh, 0, 1, modern)) {
                                Some(MsgBoxResult::Ok) } else { None }
                        }
                        MsgBoxButtons::YesNo => {
                            let ry = msgbox_btn_rect(dlg_x, dlg_y, dw, dh, 0, 2, modern);
                            let rn = msgbox_btn_rect(dlg_x, dlg_y, dw, dh, 1, 2, modern);
                            if      btn_hit(*cx, *cy, ry) { Some(MsgBoxResult::Yes) }
                            else if btn_hit(*cx, *cy, rn) { Some(MsgBoxResult::No)  }
                            else                          { None }
                        }
                    };
                    if let Some(r) = result { return r; }
                }
                InputEvent::Key(Key::Special(ScanCode::ESCAPE)) => return MsgBoxResult::No,
                InputEvent::Key(Key::Printable(c))
                    if u16::from(c) == b'\r' as u16 =>
                {
                    return match buttons {
                        MsgBoxButtons::Ok    => MsgBoxResult::Ok,
                        MsgBoxButtons::YesNo => MsgBoxResult::Yes,
                    };
                }
                _ => {}
            }
        }

        fb.pixels_mut().copy_from_slice(background);
        draw_msgbox_frame(fb, dlg_x, dlg_y, dw, dh, title, text, buttons,
                          font, ascent, font_px, palette, modern, *cx, *cy);
        if modern { draw_cursor_modern(fb, *cx, *cy); }
        else { draw_cursor(fb, *cx, *cy); }
        fb.present_to(gop_fb, gop_stride);
    }
}

fn draw_msgbox_frame(
    fb: &mut Framebuffer, dx: i32, dy: i32, dw: u32, dh: u32,
    title: &'static str, text: &'static str, buttons: &MsgBoxButtons,
    font: &fontdue::Font, ascent: i32, font_px: f32,
    palette: Palette, modern: bool, cursor_x: i32, cursor_y: i32,
) {
    let face = fb.pack(palette.face);
    let sh   = fb.pack(palette.shadow);
    fb.fill(dx, dy, dw, dh, face);
    if modern {
        fb.rect_outline(dx, dy, dw, dh, fb.pack(palette.frame));
    } else {
        fb.border_raised_with(dx, dy, dw, dh, palette.frame, palette.highlight,
                              palette.shadow, palette.dark_shadow);
    }

    let border = if modern { 1 } else { BORDER };
    let title_h = if modern { 32 } else { TITLE_H };
    let tx = dx + border; let ty = dy + border;
    let tw = dw - (border * 2) as u32;
    if modern { fb.fill(tx, ty, tw, title_h as u32, fb.pack(palette.active_l)); }
    else { fb.gradient_h(tx, ty, tw, title_h as u32, palette.active_l, palette.active_r); }
    let title_w   = Framebuffer::text_width(title, font, font_px);
    let title_off = if title_w < tw as i32 { (tw as i32 - title_w) / 2 } else { 0 };
    let t_base    = ty + (title_h + ascent) / 2;
    fb.text_aa(tx + title_off, t_base, title, palette.active_text, font, font_px);
    fb.fill(tx, ty + title_h, tw, 1, sh);

    let text_base = ty + title_h + 1 + 10 + ascent;
    fb.text_centered_aa(dx, dw, text_base, text, palette.window_text, font, font_px);

    match buttons {
        MsgBoxButtons::Ok => {
            let r = msgbox_btn_rect(dx, dy, dw, dh, 0, 1, modern);
            draw_dlg_btn(fb, r, "OK", font, ascent, font_px, palette, modern,
                         btn_hit(cursor_x, cursor_y, r));
        }
        MsgBoxButtons::YesNo => {
            let yes = msgbox_btn_rect(dx, dy, dw, dh, 0, 2, modern);
            let no = msgbox_btn_rect(dx, dy, dw, dh, 1, 2, modern);
            draw_dlg_btn(fb, yes, "Yes", font, ascent, font_px, palette, modern,
                         btn_hit(cursor_x, cursor_y, yes));
            draw_dlg_btn(fb, no, "No", font, ascent, font_px, palette, modern,
                         btn_hit(cursor_x, cursor_y, no));
        }
    }
}

fn msgbox_btn_rect(dx: i32, dy: i32, dw: u32, dh: u32,
                   idx: u32, total: u32, modern: bool) -> (i32, i32, u32, u32) {
    let bw: u32  = 80;
    let gap: i32 = 16;
    let total_w  = bw as i32 * total as i32 + gap * (total as i32 - 1);
    let bx = dx + (dw as i32 - total_w) / 2 + idx as i32 * (bw as i32 + gap);
    let bh = if modern { 30 } else { WBTN_H };
    let by = dy + dh as i32 - 14 - bh as i32;
    (bx, by, bw, bh)
}

fn draw_dlg_btn(fb: &mut Framebuffer, r: (i32, i32, u32, u32),
                label: &str, font: &fontdue::Font, ascent: i32, font_px: f32,
                palette: Palette, modern: bool, hovered: bool) {
    let (bx, by, bw, bh) = r;
    let bg = if modern && hovered { palette.highlight } else { palette.face };
    fb.fill(bx, by, bw, bh, fb.pack(bg));
    if modern {
        fb.rect_outline(bx, by, bw, bh, fb.pack(palette.frame));
    } else {
        fb.border_raised_with(bx, by, bw, bh, palette.frame, palette.highlight,
                              palette.shadow, palette.dark_shadow);
    }
    let tw   = Framebuffer::text_width(label, font, font_px);
    let off  = if tw < bw as i32 { (bw as i32 - tw) / 2 } else { 0 };
    let base = by + (bh as i32 + ascent) / 2;
    fb.text_aa(bx + off, base, label, palette.window_text, font, font_px);
}

fn btn_hit(cx: i32, cy: i32, r: (i32, i32, u32, u32)) -> bool {
    let (bx, by, bw, bh) = r;
    cx >= bx && cx < bx + bw as i32 && cy >= by && cy < by + bh as i32
}

fn draw_cursor(fb: &mut Framebuffer, x: i32, y: i32) {
    // Standard Windows 2000 "Normal Select" Cursor
    // '.' = Transparent, 'B' = Black, 'W' = White
    const CURSOR_BITMAP: [&str; 19] = [
        "B...............",
        "BB..............",
        "BWB.............",
        "BWWB............",
        "BWWWB...........",
        "BWWWWB..........",
        "BWWWWWB.........",
        "BWWWWWWB........",
        "BWWWWWWWB.......",
        "BWWWWWBBBB......",
        "BWBWWWB.........",
        "BB.BWWB.........",
        "B...BWWB........",
        "....BWWB........",
        ".....BWWB.......",
        ".....BWWB.......",
        "......BB........",
        "................",
        "................",
    ];

    let black = fb.pack(Color::BLACK);
    let white = fb.pack(Color::WHITE);

    for (row_idx, row_str) in CURSOR_BITMAP.iter().enumerate() {
        for (col_idx, char) in row_str.chars().enumerate() {
            let px = x + col_idx as i32;
            let py = y + row_idx as i32;

            // Optional: Add a boundary check to prevent crashing at screen edges
            // if px < 0 || py < 0 || px >= fb.width() || py >= fb.height() { continue; }

            match char {
                'B' => fb.set_i(px, py, black),
                'W' => fb.set_i(px, py, white),
                _ => {} // Transparent, do nothing
            }
        }
    }
}

fn draw_cursor_modern(fb: &mut Framebuffer, x: i32, y: i32) {
    // Compact, high-contrast pointer used by the modern themes.
    const CURSOR: [&str; 16] = [
        "B...............",
        "BWB.............",
        "BWWB............",
        "BWWWB...........",
        "BWWWWB..........",
        "BWWWWWB.........",
        "BWWWWWWB........",
        "BWWWWWWWB.......",
        "BWWWWBBBBB......",
        "BWWBWB...........",
        "BWB.BWB..........",
        "BB..BWB..........",
        "....BWWB.........",
        "....BWWB.........",
        ".....BB..........",
        "................",
    ];
    let outline = fb.pack(Color::rgb(0x0F, 0x17, 0x2A));
    let fill = fb.pack(Color::WHITE);
    for (row, bits) in CURSOR.iter().enumerate() {
        for (col, ch) in bits.chars().enumerate() {
            match ch {
                'B' => fb.set_i(x + col as i32, y + row as i32, outline),
                'W' => fb.set_i(x + col as i32, y + row as i32, fill),
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Text cursor / line helpers (free functions)
// ---------------------------------------------------------------------------

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    s[..pos].char_indices().next_back().map(|(i, _)| i).unwrap_or(0)
}

fn next_char_boundary(s: &str, pos: usize) -> usize {
    pos + s[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(0)
}

fn offset_to_lc(s: &str, offset: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col  = 0usize;
    for (i, c) in s.char_indices() {
        if i >= offset { return (line, col); }
        if c == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    (line, col)
}

fn lc_to_offset(s: &str, target_line: usize, target_col: usize) -> usize {
    let mut line = 0usize;
    let mut col  = 0usize;
    for (i, c) in s.char_indices() {
        if line == target_line && col == target_col { return i; }
        if c == '\n' {
            if line == target_line { return i; }
            line += 1; col = 0;
        } else {
            col += 1;
        }
    }
    s.len()
}

fn fmt_i32(n: i32, buf: &mut [u8; 16]) -> &str {
    let mut pos = buf.len();
    let neg = n < 0;
    let mut v = if neg { -(n as i64) } else { n as i64 };
    loop {
        pos -= 1; buf[pos] = b'0' + (v % 10) as u8; v /= 10;
        if v == 0 { break; }
    }
    if neg { pos -= 1; buf[pos] = b'-'; }
    core::str::from_utf8(&buf[pos..]).unwrap_or("?")
}

// ---------------------------------------------------------------------------
// Private
// ---------------------------------------------------------------------------

enum WidgetHit {
    Focus,
    TextCursor(i32, i32),  // click x, click y — focus + place cursor at click position
    Toggle,
    ComboToggle,
    Press,
    RadioSelect,
    ListSelect(usize),
    SliderPress,
    NudUp,
    NudDown,
    ScrollBarThumb,
    ScrollBarTrack(bool),  // true = above thumb (scroll up), false = below (scroll down)
}
