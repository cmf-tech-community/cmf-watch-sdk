//! ui-gallery: the watch's own widgets (ABI 1.2, LVGL) in the stock look: a stock focus list of sections (lists,
//! buttons, switches, gauges, a time picker, the stock fonts, the label long modes); a section opens on a tap and goes
//! back with its title or "Back". The crown scrolls. "Pages" is the vertical pager (`stock::pager`), "Cards" the
//! stock Exercise list (`FOCUS_CARDS`), "Swipe" the horizontal pager (`stock::hpager`). Host messages for tests/test_gallery_e2e.py: `!open N`, `!menu`;
//! for tests/test_pager_e2e.py `!hpager` (the horizontal pager with music labels), `!key N` (media_key); for
//! tests/test_music_e2e.py `!music` (the stock music tile).
#![cfg_attr(target_os = "none", no_std, no_main)]

#[cfg_attr(not(target_os = "none"), allow(dead_code))]
mod screens;

#[cfg(not(target_os = "none"))]
fn main() {}

#[cfg(target_os = "none")]
mod device {
    use crate::screens::{self, Screen, N};
    use cmf_wp2_app::lv::{Batch, LvError};
    use cmf_wp2_app::{Api, BUTTON, LV, MEDIA, MESSAGE, MSG_MAX, PAUSE};
    use cmf_wp2_ui::haptic::{Motor, TICK};

    cmf_wp2_app::entry!(main);
    cmf_wp2_app::app_icon!();

    const CLICKED: i16 = 7;
    const VALUE_CHANGED: i16 = 28;

    // read by tests/test_gallery_e2e.py: last build result, open screen (0 menu, 1 + section), screens opened
    const RC: usize = 0;
    const OPEN: usize = 1;
    const OPENED: usize = 2;
    const TICKS: usize = 3;
    const PAGE: usize = 4;
    // !hpager: media_bind result, MEDIA events, last MEDIA (x | y << 1), last media_key result
    const BIND: usize = 5;
    const MEDIAS: usize = 6;
    const MEDIA_STATE: usize = 7;
    const KEY: usize = 8;
    // the menu's centred row (last FOCUSED), -1 before the first
    const FOCUS: usize = 9;
    // !music: the last music tile button tapped
    const MUSIC: usize = 10;
    // non-zero: in .data, fresh at every launch; RC 1 until the menu is built
    #[unsafe(no_mangle)]
    static mut STATS: [i32; 11] = [1, 0, 0, 0, 0, -1, 0, -1, 1, -1, -1];

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

    fn main(api: &'static Api) -> i32 {
        let Some(lv) = api.lv() else { return -95 }; // loader older than ABI 1.2
        let Some(mut b) = Batch::new() else { return -1 };
        if screens::fonts(&mut b).and_then(|_| b.submit(&lv)).is_err() {
            return 1;
        }
        let mut flush = |b: &mut Batch| b.submit(&lv);
        let mut s = match screens::theme(&mut b).and_then(|_| screens::menu(&mut b, &mut flush)) {
            Ok(s) => s,
            Err(e) => {
                set(RC, code(Err(e)));
                return 2;
            }
        };
        set(RC, code(b.submit(&lv)));
        let mut motor = Motor::new(api);
        let (mut hour, mut minute) = (7u16, 30u16);
        // a screen to switch to after the events of this turn: None stays, Some(None) the menu, Some(Some(i))
        let mut go: Option<Option<usize>>;
        let mut msg = [0u8; MSG_MAX];
        let (mut hp, mut mu) = (false, false);
        loop {
            go = None;
            motor.update();
            let Some(e) = api.event(motor.wait().unwrap_or(u32::MAX).max(1)) else { continue };
            match e.kind {
                BUTTON => break,
                PAUSE => motor.off(),
                MEDIA => {
                    set(MEDIAS, get(MEDIAS) + 1);
                    set(MEDIA_STATE, i32::from(e.x) | i32::from(e.y) << 1);
                }
                LV if s.music.is_some_and(|m| m.event(&e).is_some()) => {
                    set(MUSIC, s.music.and_then(|m| m.event(&e)).unwrap_or(-1).into());
                }
                LV if s.hpager.is_some_and(|p| p.obj.0 == e.x as u16) => {
                    if let Some(i) = s.hpager.as_mut().and_then(|p| p.event(&e)) {
                        set(PAGE, i as i32);
                    }
                }
                LV if s.pager.is_some_and(|p| p.page(&e).is_some()) => {
                    if let Some(p) = s.pager.as_mut() {
                        if let Ok(Some(i)) = p.event(&mut b, &e) {
                            set(PAGE, i as i32);
                        }
                    }
                    set(RC, code(b.submit(&lv)));
                }
                LV => {
                    let h = e.x as u16;
                    if e.y == cmf_wp2_app::lv::FOCUSED {
                        set(FOCUS, e.delta as i32);
                    } else if e.y == CLICKED {
                        if let Some(i) = s.rows.iter().position(|r| r.0 == h) {
                            go = Some(Some(i));
                        } else if s.back.iter().any(|r| r.0 == h && h != 0) {
                            go = Some(None);
                        } else if s.ticks.iter().any(|r| r.0 == h && h != 0) {
                            motor.play(TICK);
                            set(TICKS, get(TICKS) + 1);
                        }
                    } else if e.y == VALUE_CHANGED {
                        let v = e.delta as i32;
                        let r = if s.switches.iter().any(|r| r.0 == h) {
                            motor.play(TICK);
                            Ok(())
                        } else if h == s.slider.0 {
                            let mut buf = [0u8; 11];
                            b.value(s.bar, v, false)
                                .and_then(|_| b.value(s.arc, v, false))
                                .and_then(|_| b.value(s.ring, v, false))
                                .and_then(|_| b.text(s.value, cmf_wp2_ui::text::int(v, &mut buf)))
                        } else if h == s.hours.0 || h == s.minutes.0 {
                            if h == s.hours.0 { hour = v as u16 } else { minute = v as u16 }
                            let mut buf = [0u8; 32];
                            b.text(s.picked, screens::alarm_text(hour, minute, &mut buf))
                        } else {
                            Ok(())
                        };
                        let _ = r.and_then(|_| b.submit(&lv));
                    }
                }
                MESSAGE => {
                    while let Some(m) = api.recv(&mut msg) {
                        match m {
                            b"!menu" => go = Some(None),
                            b"!hpager" => hp = true,
                            b"!music" => mu = true,
                            [b'!', b'k', b'e', b'y', b' ', d] => set(KEY, api.media_key(d.wrapping_sub(b'0'))),
                            [b'!', b'o', b'p', b'e', b'n', b' ', d] if (*d as usize) >= b'0' as usize
                                && ((*d - b'0') as usize) < N => go = Some(Some((*d - b'0') as usize)),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            if core::mem::take(&mut mu) {
                match screens::leave(&mut b, &s).and_then(|_| screens::music(&mut b)) {
                    Ok(next) => {
                        s = next;
                        set(RC, code(b.submit(&lv)));
                        set(PAGE, 2);
                    }
                    Err(e) => set(RC, code(Err(e))),
                }
            }
            if core::mem::take(&mut hp) {
                match screens::leave(&mut b, &s).and_then(|_| screens::hpages(&mut b, false)) {
                    Ok(next) => {
                        s = next;
                        set(RC, code(b.submit(&lv)));
                        set(PAGE, 0);
                        set(BIND, api.media_bind(Some(s.media[0]), Some(s.media[1])));
                    }
                    Err(e) => set(RC, code(Err(e))),
                }
            }
            if let Some(to) = go {
                if s.hpager.is_some() {
                    api.media_unbind(); // its labels go away
                }
                let r = screens::leave(&mut b, &s).and_then(|_| match to {
                    None => screens::menu(&mut b, &mut flush),
                    Some(i) => screens::section(&mut b, &mut flush, i),
                });
                match r {
                    Ok(next) => {
                        s = next;
                        set(RC, code(b.submit(&lv)));
                        set(OPEN, to.map_or(0, |i| i as i32 + 1));
                        set(OPENED, get(OPENED) + 1);
                    }
                    Err(e) => set(RC, code(Err(e))),
                }
                (hour, minute) = (7, 30);
                set(PAGE, 0);
            }
        }
        motor.off();
        0
    }
}
