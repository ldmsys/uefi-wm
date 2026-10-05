#![no_std]
#![no_main]

extern crate alloc;

use uefi::prelude::*;
use uefi::boot;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::runtime::ResetType;
use alloc::boxed::Box;

use uefi_wm::gfx::Framebuffer;
use uefi_wm::input::InputDriver;
use uefi_wm::wm::{EventCtx, MsgBoxButtons, MsgBoxResult, Theme, WindowManager};

static FONT: &[u8] = include_bytes!("../fonts/NanumGothicHang2.ttf");
const  FONT_PX: f32 = 16.0;

#[entry]
fn efi_main() -> Status {
    uefi::helpers::init().expect("helpers::init failed");

    let handle = boot::get_handle_for_protocol::<GraphicsOutput>()
        .expect("No GOP handle");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(handle)
        .expect("Cannot open GOP");
    let info = gop.current_mode_info();
    let (w, h) = info.resolution();
    let (width, height) = (w as u32, h as u32);
    let stride = info.stride();

    let mut fb  = Framebuffer::from_mode_info(&info)
        .expect("Cannot create software framebuffer for GOP mode");
    let mut wm  = WindowManager::new(width, height, FONT, FONT_PX);
    let mut drv = InputDriver::new(width, height);

    // --- Window 0: UEFI Setup Utility ---
    let id0 = wm.open("UEFI Setup Utility", 40, 40, 680, 550);

    wm.add_groupbox(id0, 0, 0, 654, 76, "System Identity");
    wm.add_textbox( id0, 10, 20, 280, "Product Name: ");
    let serial_textbox = wm.add_textbox( id0, 10, 48, 280, "Serial No.  : ");
    wm.widget_set_enabled(serial_textbox, false);

    wm.add_groupbox(id0, 0, 88, 654, 100, "Boot Options");
    wm.add_checkbox(id0, 10, 108, "Enable Fast Boot");
    wm.add_checkbox(id0, 10, 132, "Enable USB Boot");
    wm.add_combobox(id0, 10, 156, 240, "Boot Device : ",
                    &["SATA HDD 0", "USB Drive", "PXE Network"]);

    wm.add_groupbox(id0, 0, 200, 654, 96, "OS Settings");
    wm.add_combobox(id0, 10, 220, 200, "OS Type     : ",
                    &["Windows 10", "Linux", "Other"]);
    let theme_combo = wm.add_combobox(id0, 330, 220, 150, "Theme: ",
                                      &["Classic", "Light", "Dark"]);
    wm.set_combobox_selected(theme_combo, 1);
    wm.set_on_change(theme_combo, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
        let theme = match wm.combobox_selected(theme_combo) {
            1 => Theme::Light,
            2 => Theme::Dark,
            _ => Theme::Classic,
        };
        wm.set_theme(theme);
    }));
    let timeout_nud = wm.add_numeric_updown(id0, 10, 252, 80, "Timeout (s) : ", -5, 30, 5);
    wm.set_numeric_step(timeout_nud, 1);

    wm.add_groupbox(id0, 0, 308, 654, 72, "Fan Control");
    let fan_slider = wm.add_slider(id0, 10, 328, 380, "Fan Speed : ", 0, 100, 50);
    let fan_label  = wm.add_label(id0, 500, 332, 60, "50%");
    wm.set_on_change(fan_slider, Box::new(move |wm: &mut WindowManager, _: &mut EventCtx| {
        let v = wm.slider_value(fan_slider);
        let mut buf = [0u8; 8];
        wm.label_set_text(fan_label, fmt_pct(v, &mut buf));
    }));

    wm.add_groupbox(id0, 0, 392, 654, 56, "POST Progress");
    let pbar = wm.add_progressbar(id0, 10, 412, 460, "Initializing : ");
    wm.set_progressbar(pbar, 72, 100);

    wm.add_separator(id0, 0, 460, 654);

    let save_btn    = wm.add_button(id0,   0, 472, 120, "Save & Exit");
    let discard_btn = wm.add_button(id0, 136, 472, 100, "Discard");
                      wm.add_button(id0, 252, 472, 110, "Load Defaults");

    wm.set_on_click(save_btn, Box::new(|wm: &mut WindowManager, ctx: &mut EventCtx| {
        if wm.message_box(ctx, "Save & Exit", "Save settings and restart the system?",
                          MsgBoxButtons::YesNo) == MsgBoxResult::Yes
        {
            uefi::runtime::reset(ResetType::COLD, Status::SUCCESS, None);
        }
    }));
    wm.set_on_click(discard_btn, Box::new(|_: &mut WindowManager, ctx: &mut EventCtx| {
        ctx.quit = true;
    }));

    // --- Window 1: System information ---
    let id1 = wm.open("시스템 정보", 200, 100, 460, 450);

    wm.add_label(id1, 0, 0, 434, "Boot log:");
    let log_area = wm.add_textarea(id1, 0, 20, 434, 150);
    wm.set_textarea_text(log_area,
        "UEFI v2.10  Spec 2.10\nCPU: x86_64  Cores: 8\nRAM: 16384 MB\nGOP: 1920x1080 BGR\nFS0: SATA HDD 0 (512 GB)");

    wm.add_separator(id1, 0, 180, 434);

    wm.add_label(id1, 0, 192, 434, "Graphics mode:");
    wm.add_listbox(id1, 0, 212, 434, 100,
                   &["1920x1080 BGR (current)", "1280x720 BGR",
                     "1024x768 BGR", "800x600 BGR", "640x480 BGR", "80x42 Text"]);

    wm.add_separator(id1, 0, 322, 434);

    wm.add_label(      id1,   0, 334, 110, "Secure Boot:");
    wm.add_radiobutton(id1, 116, 334, "Enabled",  0);
    wm.add_radiobutton(id1, 216, 334, "Disabled", 0);

    wm.add_separator(id1, 0, 358, 434);

    wm.add_label(   id1,  0, 370, 76, "Note:");
    wm.add_checkbox(id1, 80, 370, "Read Only");

    // --- Window 2: QR Code widget demo ---
    let qr_x = i32::try_from(width).unwrap_or(i32::MAX)
        .saturating_sub(260).max(20);
    let qr_y = i32::try_from(height).unwrap_or(i32::MAX)
        .saturating_sub(280).max(20);
    let id2 = wm.open("QR Code Demo", qr_x, qr_y, 220, 240);
    wm.add_qrcode(id2, 23, 0, 148, "https://github.com/ldmsys/uefi-wm")
        .expect("QR Code demo payload must fit");
    wm.add_label(id2, 0, 156, 194, "Scan to visit the project");

    wm.run(&mut fb, &mut drv, gop.frame_buffer(), stride);

    Status::SUCCESS
}

fn fmt_pct(v: i32, buf: &mut [u8; 8]) -> &str {
    let mut pos = 0usize;
    let v = v.clamp(0, 100) as u32;
    if v >= 100 {
        buf[pos] = b'1'; pos += 1;
        buf[pos] = b'0'; pos += 1;
        buf[pos] = b'0'; pos += 1;
    } else if v >= 10 {
        buf[pos] = b'0' + (v / 10) as u8; pos += 1;
        buf[pos] = b'0' + (v % 10) as u8; pos += 1;
    } else {
        buf[pos] = b'0' + v as u8; pos += 1;
    }
    buf[pos] = b'%'; pos += 1;
    core::str::from_utf8(&buf[..pos]).unwrap_or("?")
}
