#![cfg_attr(any(target_os = "none", wp2_sim), no_main, cfg_attr(target_os = "none", no_std))]

#[cfg_attr(not(target_os = "none"), allow(dead_code))]
mod model;
#[cfg_attr(not(target_os = "none"), allow(dead_code))]
mod screen;

#[cfg(not(any(target_os = "none", wp2_sim)))]
fn main() {}

#[cfg(any(target_os = "none", wp2_sim))]
mod device {
    use crate::model::*;
    use crate::screen::{self, Screen};
    use cmf_wp2_app::lv::{Batch, LvError, FOCUSED};
    use cmf_wp2_app::{Api, BUTTON, LV, MSG_MAX, PAUSE, RESUME, STORE_MAX};
    use cmf_wp2_ui::haptic::Motor;
    use core::fmt::Write;

    cmf_wp2_app::entry!(main);
    cmf_wp2_app::app_icon!();

    const ASK_MS: u32 = 3000;
    const AWAKE_MS: u32 = 20_000;
    // working dots: a colour step per tick
    const PULSE_MS: u32 = 100;
    const CLICKED: i16 = 7;
    static mut STORE: [u8; STORE_MAX] = [0; STORE_MAX];
    // ~3 KB each: off the 8 KiB app stack
    static mut MODEL: Model = Model::new();
    static mut SCREEN: Option<Screen> = None;

    // read by tests/test_herdr_e2e.py: the first sync's result, FOCUSED events, the centred row, CLICKED rows
    const SYNC_RC: usize = 0;
    const FOCUS_EVENTS: usize = 1;
    const SEL: usize = 2;
    const CLICKS: usize = 3;
    // non-zero: in .data, fresh at every launch; SYNC_RC 1 until the first sync returned
    #[unsafe(no_mangle)]
    static mut STATS: [i32; 4] = [1, 0, -1, 0];

    fn set(i: usize, v: i32) {
        unsafe { (&raw mut STATS[i]).write_volatile(v) }
    }
    fn get(i: usize) -> i32 {
        unsafe { (&raw const STATS[i]).read_volatile() }
    }
    fn code(r: Result<(), LvError>) -> i32 {
        match r {
            Ok(()) => 0,
            Err(LvError::Code(c)) => c,
            Err(_) => -1000,
        }
    }

    fn after(now: u32, t: u32) -> bool {
        (now.wrapping_sub(t) as i32) >= 0
    }

    fn save(api: &Api, model: &Model) {
        let buf = unsafe { &mut *&raw mut STORE };
        let n = model.save(buf);
        api.save(&buf[..n]);
    }

    // the card widget (docs/app-guide.md, "Widget"): agent counts at exit
    fn widget(api: &Api, model: &Model) {
        let (working, _, _) = model.counts();
        let mut buf = [0u8; 40];
        let mut w = Buf(&mut buf, 0);
        let _ = write!(w, "{} agents\n{} working", model.len(), working);
        let n = w.1;
        api.widget("herdr", core::str::from_utf8(&buf[..n]).unwrap_or(""));
    }

    struct Buf<'a>(&'a mut [u8], usize);

    impl core::fmt::Write for Buf<'_> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let n = s.len().min(self.0.len() - self.1);
            self.0[self.1..self.1 + n].copy_from_slice(&s.as_bytes()[..n]);
            self.1 += n;
            Ok(())
        }
    }

    // true on RESUME, false on BUTTON
    fn paused(api: &Api) -> bool {
        loop {
            match api.event(u32::MAX).map(|e| e.kind) {
                Some(RESUME) => return true,
                Some(BUTTON) => return false,
                _ => {}
            }
        }
    }

    fn main(api: &'static Api) -> i32 {
        let Some(lv) = api.lv() else { return -95 }; // loader older than ABI 1.2
        let Some(mut b) = Batch::new() else { return -1 };
        let model = unsafe { &mut *&raw mut MODEL };
        let buf = unsafe { &mut *&raw mut STORE };
        let n = api.load(buf);
        if n > 0 {
            model.load(&buf[..n as usize]);
        }
        if Screen::fonts(&mut b).and_then(|_| b.submit(&lv)).is_err() {
            return 1;
        }
        let scr = unsafe { &mut *&raw mut SCREEN };
        let Ok(s) = Screen::build(&mut b) else { return 2 };
        let s = scr.insert(s);
        let mut flush = |b: &mut Batch| b.submit(&lv);
        let mut heard: Option<u32> = None;
        set(SYNC_RC, code(s.sync(&mut b, &mut flush, model, screen::status_line(heard, api.now()), api.now())));

        let mut motor = Motor::new(api);
        let mut want = true;
        let mut asked: Option<u32> = None;
        let mut awake: Option<u32> = None;
        let mut unsaved = false;
        let mut pulsed = 0u32;
        loop {
            let now = api.now();
            motor.update();
            if awake.is_some_and(|t| after(now, t)) {
                api.keep_screen_on(false);
                awake = None;
            }
            if now.wrapping_sub(pulsed) >= PULSE_MS {
                let _ = s.sync(&mut b, &mut flush, model, screen::status_line(heard, now), now);
                pulsed = now;
            }
            let stale = heard.is_none_or(|t| now.wrapping_sub(t) > screen::STALE_MS);
            if (want || stale) && asked.is_none_or(|t| now.wrapping_sub(t) >= ASK_MS) {
                api.send(REQUEST);
                asked = Some(now);
            }

            let wait = motor.wait().unwrap_or(PULSE_MS).min(PULSE_MS);
            match api.event(wait) {
                Some(e) if e.kind == BUTTON => break,
                Some(e) if e.kind == PAUSE => {
                    motor.off();
                    if !paused(api) {
                        break;
                    }
                    want = true;
                    asked = None;
                }
                Some(e) if e.kind == RESUME => want = true,
                Some(e) if e.kind == LV && e.y == FOCUSED => {
                    set(FOCUS_EVENTS, get(FOCUS_EVENTS) + 1);
                    set(SEL, e.delta.into());
                }
                Some(e) if e.kind == LV && e.y == CLICKED => {
                    if let Some(a) = s.row_of(e.x as u16).and_then(|i| model.get(i)) {
                        set(CLICKS, get(CLICKS) + 1);
                        api.send(&[b'h', b'1', b'F', a.id]);
                    }
                }
                _ => {}
            }

            let mut msg = [0u8; MSG_MAX];
            let mut buzz = None;
            let mut changed = false;
            while let Some(m) = api.recv(&mut msg) {
                let fx = model.apply(m);
                if decode(m).is_some() {
                    heard = Some(api.now());
                }
                changed |= fx.changed;
                unsaved |= fx.changed;
                buzz = buzz.max(fx.buzz);
                if fx.resync {
                    want = true;
                    asked = None;
                }
                if fx.synced {
                    want = false;
                    save(api, model);
                    unsaved = false;
                }
            }
            if changed {
                pulsed = pulsed.wrapping_sub(PULSE_MS); // sync on the next turn
            }
            if let Some(b) = buzz {
                motor.play(b.pattern());
                api.keep_screen_on(true);
                awake = Some(api.now().wrapping_add(AWAKE_MS));
            }
        }
        motor.off();
        if unsaved {
            save(api, model);
        }
        widget(api, model);
        0
    }
}
