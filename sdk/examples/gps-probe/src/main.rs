//! GPS probe for tests/test_gps_e2e.py (docs/abi-1.4.md, "GPS"); a canvas app, no LVGL. Asks for the GPS at start
//! and shows the fix status, satellites, the time to first fix, the position rounded to 0.001° (about 100 m) and the speed between the
//! last two fixes. A tap toggles the request, a crown detent panics (the crash path), BUTTON returns. Coordinates
//! are personal data: shown rounded, never logged or sent.
#![no_std]
#![no_main]

use cmf_wp2_app::{Api, GpsFix, BUTTON, CROWN, GPS, TOUCH_DOWN};

cmf_wp2_app::entry!(main);

// [gps() result, GPS events, last fix quality, last sats, lat_e7, lon_e7, ttff_s, speed cm/s]; non-zero
// initialiser: in .data, fresh at every launch
#[unsafe(no_mangle)]
static mut PROBE: [i32; 8] = [-1; 8];

fn put(i: usize, v: i32) {
    unsafe { (&raw mut PROBE[i]).write_volatile(v) }
}

fn get(i: usize) -> i32 {
    unsafe { (&raw const PROBE[i]).read_volatile() }
}

/// Distance in cm between two fixes: equirectangular, cos by its Taylor series (core has no trig); within about
/// 1% below 70° latitude, plenty for walking and running speeds.
fn distance_cm(a: &GpsFix, b: &GpsFix) -> u64 {
    const CM_PER_E7: i64 = 1113; // 1e-7° of latitude ≈ 1.113 cm (×1000 below)
    let lat = (a.lat_e7 as f32) * (core::f32::consts::PI / 180.0e7);
    let x2 = lat * lat;
    let cos = 1.0 - x2 / 2.0 + x2 * x2 / 24.0;
    let dy = (b.lat_e7 - a.lat_e7) as i64 * CM_PER_E7 / 1000;
    let dx = ((b.lon_e7 - a.lon_e7) as f32 * cos) as i64 * CM_PER_E7 / 1000;
    ((dx * dx + dy * dy) as u64).isqrt()
}

struct Line<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Line<N> {
    fn new() -> Self {
        Line { buf: [b' '; N], len: 0 }
    }
    fn s(&mut self, s: &str) -> &mut Self {
        for &b in s.as_bytes() {
            if self.len < N {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
        self
    }
    fn n(&mut self, v: u32, min_digits: u32) -> &mut Self {
        let mut d = 1;
        while d * 10 <= v || d * 10 < 10u32.pow(min_digits) {
            d *= 10;
        }
        while d > 0 {
            self.s(core::str::from_utf8(&[b'0' + (v / d % 10) as u8]).unwrap_or("?"));
            d /= 10;
        }
        self
    }
    /// 1e-7 degrees as ±D.ddd
    fn deg(&mut self, e7: i32) -> &mut Self {
        let milli = (e7.unsigned_abs() + 5_000) / 10_000;
        self.s(if e7 < 0 { "-" } else { "" }).n(milli / 1000, 1).s(".").n(milli % 1000, 3)
    }
    fn str(&self) -> &str {
        core::str::from_utf8(&self.buf).unwrap_or("")
    }
}

fn draw(api: &Api, fix: Option<&GpsFix>, speed_cm_s: u32) {
    api.rect(60, 150, 346, 170, 0);
    let mut a = Line::<20>::new();
    let mut b = Line::<20>::new();
    let mut c = Line::<20>::new();
    match fix {
        Some(f) if f.fix > 0 => {
            a.s("FIX ").n(f.sats.into(), 1).s(" SAT TTFF ").n(f.ttff_s.into(), 1);
            b.deg(f.lat_e7).s(" ").deg(f.lon_e7);
            c.n(speed_cm_s * 36 / 1000, 1).s(".").n(speed_cm_s * 36 / 100 % 10, 1).s(" KM/H");
        }
        Some(f) => {
            a.s("SEARCHING ").n(f.sats.into(), 1);
        }
        None => {
            a.s("GPS OFF");
        }
    }
    api.text(80, 170, a.str(), 0xFFFF);
    api.text(80, 220, b.str(), 0xFFFF);
    api.text(80, 270, c.str(), 0xFFFF);
    api.show();
}

fn main(api: &'static Api) -> i32 {
    let mut on = true;
    put(0, api.gps(on));
    put(1, 0);
    draw(api, None, 0);
    let mut last: Option<GpsFix> = None;
    loop {
        match api.event(u32::MAX) {
            Some(e) if e.kind == GPS => {
                put(1, get(1) + 1);
                put(2, e.x.into());
                put(3, e.y.into());
                let fix = api.gps_fix();
                let mut speed = 0;
                if let Some(f) = fix.filter(|f| f.fix > 0) {
                    put(4, f.lat_e7);
                    put(5, f.lon_e7);
                    put(6, f.ttff_s.into());
                    if let Some(p) = last.filter(|p| f.ms.wrapping_sub(p.ms) > 0) {
                        speed = (distance_cm(&p, &f) * 1000 / u64::from(f.ms.wrapping_sub(p.ms))) as u32;
                    }
                    put(7, speed as i32);
                    last = Some(f);
                }
                draw(api, fix.as_ref(), speed);
            }
            Some(e) if e.kind == TOUCH_DOWN => {
                on = !on;
                put(0, api.gps(on));
                last = None;
                if !on {
                    draw(api, None, 0);
                }
            }
            Some(e) if e.kind == CROWN => panic!("crash test"),
            Some(e) if e.kind == BUTTON => return 0,
            _ => {}
        }
    }
}
