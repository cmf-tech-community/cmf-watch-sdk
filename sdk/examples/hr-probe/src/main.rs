//! Heart-rate test app for tests/test_hr_e2e.py (docs/abi-1.4.md, "Heart rate"); a canvas app, no LVGL. Asks for
//! the heart rate at start and shows the latest HEART event. A tap toggles the request, a crown detent panics
//! (the crash path), BUTTON returns.
#![no_std]
#![no_main]

use cmf_wp2_app::{Api, BUTTON, CROWN, HEART, TOUCH_DOWN};

cmf_wp2_app::entry!(main);

// [heart_rate() result, HEART events, last delta, last x]; non-zero initialiser: in .data, fresh at every launch
#[unsafe(no_mangle)]
static mut PROBE: [i32; 4] = [-1; 4];

fn put(i: usize, v: i32) {
    unsafe { (&raw mut PROBE[i]).write_volatile(v) }
}

fn get(i: usize) -> i32 {
    unsafe { (&raw const PROBE[i]).read_volatile() }
}

fn draw(api: &Api, bpm: i16) {
    let mut text = *b"HR --  ";
    if bpm > 0 {
        let b = bpm as u16;
        text[3..6].copy_from_slice(&[b'0' + (b / 100 % 10) as u8, b'0' + (b / 10 % 10) as u8, b'0' + (b % 10) as u8]);
    }
    api.rect(150, 210, 180, 40, 0);
    api.text(170, 220, core::str::from_utf8(&text).unwrap_or("HR"), 0xFFFF);
    api.show();
}

// the card widget (docs/app-guide.md, "Widget"): the last reading, written once on exit
fn widget(api: &Api, bpm: i32) {
    let mut text = *b"--- bpm";
    if bpm > 0 {
        let b = bpm as u16;
        text[..3].copy_from_slice(&[b'0' + (b / 100 % 10) as u8, b'0' + (b / 10 % 10) as u8, b'0' + (b % 10) as u8]);
    }
    let value = if bpm > 0 { core::str::from_utf8(&text).unwrap_or("") } else { "No reading" };
    api.widget("Last heart rate", value);
}

fn main(api: &'static Api) -> i32 {
    let mut on = true;
    put(0, api.heart_rate(on));
    put(1, 0);
    draw(api, 0);
    loop {
        match api.event(u32::MAX) {
            Some(e) if e.kind == HEART => {
                put(1, get(1) + 1);
                put(2, e.delta.into());
                put(3, e.x.into());
                draw(api, e.delta);
            }
            Some(e) if e.kind == TOUCH_DOWN => {
                on = !on;
                put(0, api.heart_rate(on));
            }
            Some(e) if e.kind == CROWN => panic!("crash test"),
            Some(e) if e.kind == BUTTON => {
                widget(api, get(2));
                return 0;
            }
            _ => {}
        }
    }
}
