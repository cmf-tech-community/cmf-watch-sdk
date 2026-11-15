//! ui-gallery's screens: the watch's own LVGL widgets in the stock look (`cmf_wp2_app::stock`). The menu is a stock
//! focus list of sections; a section is a stock page with the accent title (tap: back), its widgets and "Back" at
//! the end. Leaving a screen deletes its page, and every object and the scroll bar with it.

use cmf_wp2_app::lv::*;
use cmf_wp2_app::stock::{self, ACCENT, CIRCLE, GREY, ICON, LETTER_BG, ROW_BG, ROW_PRESSED, ROW_W, WHITE};

pub const SECTIONS: [(&str, &str); 7] = [
    ("L", "Lists"),
    ("B", "Buttons"),
    ("S", "Switches"),
    ("G", "Gauges"),
    ("P", "Picker"),
    ("F", "Fonts"),
    ("V", "Pages"),
];
/// The section with the vertical pager.
pub const PAGES: usize = 6;
pub const N: usize = SECTIONS.len();
/// Font slots besides the system font (slot 0).
pub const F24: u8 = 1;
pub const F18: u8 = 2;
pub const F48: u8 = 3;
pub const START: i32 = 40;
// bytes one widget group can take: flush first when fewer are left
const GROUP: usize = 560;

/// What the app reacts to on the current screen; unused handles are `ROOT` (never an event target).
#[derive(Clone, Copy)]
pub struct Screen {
    pub page: Obj,
    /// `None`: the menu.
    pub section: Option<usize>,
    pub rows: [Obj; N],
    pub back: [Obj; 2],
    /// Buttons, rows and checkboxes: a tick on CLICKED.
    pub ticks: [Obj; 8],
    pub switches: [Obj; 2],
    pub slider: Obj,
    pub bar: Obj,
    pub arc: Obj,
    pub ring: Obj,
    pub value: Obj,
    pub hours: Obj,
    pub minutes: Obj,
    pub picked: Obj,
    pub pager: Option<stock::Pager<3>>,
}

const EMPTY: Screen = Screen {
    page: ROOT,
    section: None,
    rows: [ROOT; N],
    back: [ROOT; 2],
    ticks: [ROOT; 8],
    switches: [ROOT; 2],
    slider: ROOT,
    bar: ROOT,
    arc: ROOT,
    ring: ROOT,
    value: ROOT,
    hours: ROOT,
    minutes: ROOT,
    picked: ROOT,
    pager: None,
};

fn room(b: &mut Batch, flush: &mut impl FnMut(&mut Batch) -> Result<(), LvError>) -> Result<(), LvError> {
    if b.bytes().len() + GROUP > BUF { flush(b) } else { Ok(()) }
}

/// The fonts, in a batch of their own.
pub fn fonts(b: &mut Batch) -> Result<(), LvError> {
    stock::font(b)?;
    b.font(F24, StockFont::F24M)?;
    b.font(F18, StockFont::F18M)?;
    b.font(F48, StockFont::F48M)
}

/// Once, first: the system font and white text for everything.
pub fn theme(b: &mut Batch) -> Result<(), LvError> {
    stock::theme(b)
}

/// Deletes the current screen (its page, with every object and its scroll bar).
pub fn leave(b: &mut Batch, s: &Screen) -> Result<(), LvError> {
    if s.page != ROOT { b.del(s.page) } else { Ok(()) }
}

/// The menu: a stock focus list, a letter circle and the name per section.
pub fn menu(b: &mut Batch, flush: &mut impl FnMut(&mut Batch) -> Result<(), LvError>) -> Result<Screen, LvError> {
    room(b, flush)?;
    let (page, _bar) = stock::focus_page(b, ICON as u16)?;
    let mut s = Screen { page, ..EMPTY };
    for (row, (letter, name)) in s.rows.iter_mut().zip(SECTIONS) {
        room(b, flush)?;
        *row = stock::focus_row(b, page)?;
        let circle = b.create(Class::Obj, *row)?;
        b.size(circle, ICON, ICON)?;
        b.align(circle, Align::LeftMid, 4, 0)?;
        b.style(circle, Prop::BgColor, 0, LETTER_BG)?;
        b.style(circle, Prop::BgOpa, 0, 255)?;
        b.style(circle, Prop::Radius, 0, CIRCLE)?;
        b.style(circle, Prop::BorderWidth, 0, 0)?;
        b.flags(circle, &[], &[Flag::Clickable, Flag::Scrollable])?;
        let l = b.create(Class::Label, circle)?;
        b.text(l, letter)?;
        b.align(l, Align::Center, 0, 0)?;
        let n = b.create(Class::Label, *row)?;
        b.size(n, ROW_W - ICON - 64, 44)?;
        b.long_mode(n, LongMode::Dot)?;
        b.text(n, name)?;
        b.align(n, Align::LeftMid, ICON as i16 + 28, 0)?;
    }
    Ok(s)
}

#[derive(Clone, Copy)]
enum Kind {
    Accent,
    Secondary,
    Outline,
    Disabled,
}

fn button(b: &mut Batch, parent: Obj, text: &str, kind: Kind) -> Result<Obj, LvError> {
    let btn = b.create(Class::Btn, parent)?;
    b.size(btn, 300, 88)?;
    b.style(btn, Prop::Radius, 0, 44)?;
    let (bg, opa, fg) = match kind {
        Kind::Accent => (ACCENT, 255, WHITE),
        Kind::Secondary => (ROW_BG, 255, WHITE),
        Kind::Outline => (ACCENT, 0, ACCENT),
        Kind::Disabled => (ROW_BG, 255, GREY),
    };
    b.style(btn, Prop::BgColor, 0, bg)?;
    b.style(btn, Prop::BgOpa, 0, opa)?;
    b.style(btn, Prop::BgColor, stock::PRESSED, ROW_PRESSED)?;
    b.style(btn, Prop::BgOpa, stock::PRESSED, 255)?;
    if let Kind::Outline = kind {
        // an accent pill with a black one 3 px inside: borders stop the emulator (its display engine lacks them)
        b.style(btn, Prop::BgOpa, 0, 255)?;
        let inner = b.create(Class::Obj, btn)?;
        b.size(inner, 294, 82)?;
        b.align(inner, Align::Center, 0, 0)?;
        b.style(inner, Prop::Radius, 0, 41)?;
        b.style(inner, Prop::BgColor, 0, stock::BLACK)?;
        b.style(inner, Prop::BgOpa, 0, 255)?;
        b.style(inner, Prop::BorderWidth, 0, 0)?;
        b.flags(inner, &[], &[Flag::Clickable, Flag::Scrollable])?;
    }
    let l = b.create(Class::Label, btn)?;
    b.style(l, Prop::TextColor, 0, fg)?;
    b.text(l, text)?;
    b.align(l, Align::Center, 0, 0)?;
    if let Kind::Disabled = kind {
        b.flags(btn, &[], &[Flag::Clickable])?;
    } else {
        b.events(btn, &[Ev::Clicked])?;
    }
    Ok(btn)
}

/// A card with a label on the left; returns (card, label).
fn labelled(b: &mut Batch, parent: Obj, text: &str) -> Result<(Obj, Obj), LvError> {
    let card = stock::card(b, parent, stock::ROW_H)?;
    let l = b.create(Class::Label, card)?;
    b.size(l, 250, 44)?;
    b.long_mode(l, LongMode::Dot)?;
    b.text(l, text)?;
    b.align(l, Align::LeftMid, 36, 0)?;
    Ok((card, l))
}

/// Section `i`: a stock page with the title, the widgets, "Back".
pub fn section(b: &mut Batch, flush: &mut impl FnMut(&mut Batch) -> Result<(), LvError>, i: usize)
    -> Result<Screen, LvError> {
    room(b, flush)?;
    if i == PAGES {
        return pages(b, flush);
    }
    let (page, _bar) = stock::page(b)?;
    let mut s = Screen { page, section: Some(i), ..EMPTY };
    let title = stock::title(b, page, SECTIONS[i].1)?;
    b.flags(title, &[Flag::Clickable], &[])?;
    b.events(title, &[Ev::Clicked])?;
    s.back[0] = title;
    match i {
        0 => {
            let rows = [("W", "Wi-Fi", "On"), ("B", "Bluetooth", "Off"), ("D", "Brightness", "70%"), ("T", "Time format", "24h")];
            for (k, (letter, name, detail)) in rows.into_iter().enumerate() {
                room(b, flush)?;
                s.ticks[k] = stock::row(b, page, letter, name, detail)?;
            }
        }
        1 => {
            for (k, (text, kind)) in
                [("Done", Kind::Accent), ("Cancel", Kind::Secondary), ("More", Kind::Outline), ("Unavailable", Kind::Disabled)]
                    .into_iter()
                    .enumerate()
            {
                room(b, flush)?;
                s.ticks[k] = button(b, page, text, kind)?;
            }
        }
        2 => {
            for (k, (text, on)) in [("Do not disturb", true), ("Raise to wake", false)].into_iter().enumerate() {
                room(b, flush)?;
                let (card, _) = labelled(b, page, text)?;
                // a 0..1 slider in the stock colours is the switch: a tap on either half sets it
                let sw = b.create(Class::Slider, card)?;
                b.size(sw, 84, 44)?;
                b.align(sw, Align::RightMid, -36, 0)?;
                b.range(sw, 0, 1)?;
                b.value(sw, on as i32, false)?;
                stock::slider(b, sw)?;
                b.events(sw, &[Ev::ValueChanged])?;
                s.switches[k] = sw;
            }
            for (k, text) in ["Steps", "Heart rate", "Sleep"].into_iter().enumerate() {
                room(b, flush)?;
                let (card, _) = labelled(b, page, text)?;
                let check = b.create(Class::Btn, card)?;
                b.size(check, 48, 48)?;
                b.align(check, Align::RightMid, -36, 0)?;
                // filled, no border (see `button`): grey, the accent while CHECKED
                b.style(check, Prop::Radius, 0, 12)?;
                b.style(check, Prop::BgColor, 0, LETTER_BG)?;
                b.style(check, Prop::BgOpa, 0, 255)?;
                b.style(check, Prop::BgColor, CHECKED, ACCENT)?;
                // CHECKABLE: LVGL toggles CHECKED on every click
                b.flags(check, &[Flag::Checkable], &[])?;
                b.events(check, &[Ev::Clicked])?;
                s.ticks[k] = check;
            }
        }
        3 => {
            room(b, flush)?;
            let card = stock::card(b, page, 230)?;
            let value = b.create(Class::Label, card)?;
            b.style(value, Prop::TextFont, 0, F48 as i32)?;
            b.text(value, "40")?;
            b.align(value, Align::TopMid, 0, 24)?;
            let slider = b.create(Class::Slider, card)?;
            b.size(slider, 300, 16)?;
            b.align(slider, Align::TopMid, 0, 120)?;
            b.value(slider, START, false)?;
            stock::slider(b, slider)?;
            b.events(slider, &[Ev::ValueChanged])?;
            let bar = b.create(Class::Bar, card)?;
            b.size(bar, 300, 10)?;
            b.align(bar, Align::TopMid, 0, 176)?;
            b.value(bar, START, false)?;
            for (part, colour) in [(0, LETTER_BG), (stock::INDICATOR, GREY)] {
                b.style(bar, Prop::BgColor, part, colour)?;
                b.style(bar, Prop::BgOpa, part, 255)?;
                b.style(bar, Prop::Radius, part, CIRCLE)?;
            }
            room(b, flush)?;
            let card = stock::card(b, page, 220)?;
            let arc = b.create(Class::Arc, card)?;
            b.size(arc, 160, 160)?;
            b.align(arc, Align::LeftMid, 40, 0)?;
            b.value(arc, START, false)?;
            stock::arc(b, arc, 14)?;
            b.flags(arc, &[], &[Flag::Clickable])?;
            // a full ring from 12 o'clock
            let ring = b.create(Class::Arc, card)?;
            b.size(ring, 140, 140)?;
            b.align(ring, Align::RightMid, -40, 0)?;
            b.angles(ring, 0, 360, 270)?;
            b.value(ring, START, false)?;
            stock::arc(b, ring, 10)?;
            b.flags(ring, &[], &[Flag::Clickable])?;
            (s.value, s.slider, s.bar, s.arc, s.ring) = (value, slider, bar, arc, ring);
        }
        4 => {
            room(b, flush)?;
            let card = stock::card(b, page, 280)?;
            let mut hours = [0u8; 24 * 3];
            let mut minutes = [0u8; 60 * 3];
            let opts = |out: &mut [u8], n: usize| {
                for k in 0..n {
                    out[3 * k..3 * k + 3].copy_from_slice(&[b'0' + (k / 10) as u8, b'0' + (k % 10) as u8, b'\n']);
                }
            };
            opts(&mut hours, 24);
            opts(&mut minutes, 60);
            let mut rollers = [ROOT; 2];
            for (k, (o, v)) in [(&hours[..71], 7), (&minutes[..179], 30)].into_iter().enumerate() {
                room(b, flush)?;
                let r = b.create(Class::Roller, card)?;
                b.options(r, core::str::from_utf8(o).unwrap_or("00"), true)?;
                b.size(r, 120, 250)?;
                b.align(r, Align::Center, if k == 0 { -70 } else { 70 }, 0)?;
                b.value(r, v, false)?;
                b.style(r, Prop::BgOpa, 0, 0)?;
                b.style(r, Prop::BorderWidth, 0, 0)?;
                b.style(r, Prop::TextColor, 0, GREY)?;
                // the selected row uses the 48 px font: rows need the line space to hold it
                b.style(r, Prop::TextLineSpace, 0, 28)?;
                b.style(r, Prop::TextAlign, 0, 2)?;
                b.style(r, Prop::BgOpa, stock::SELECTED, 0)?;
                b.style(r, Prop::TextColor, stock::SELECTED, WHITE)?;
                b.style(r, Prop::TextFont, stock::SELECTED, F48 as i32)?;
                b.events(r, &[Ev::ValueChanged])?;
                rollers[k] = r;
            }
            let colon = b.create(Class::Label, card)?;
            b.style(colon, Prop::TextFont, 0, F48 as i32)?;
            b.text(colon, ":")?;
            b.align(colon, Align::Center, 0, -4)?;
            // a roller takes the crown when it is created: back to the page
            b.knob(page)?;
            room(b, flush)?;
            let picked = b.create(Class::Label, page)?;
            b.style(picked, Prop::TextColor, 0, GREY)?;
            b.text(picked, "Alarm 07:30")?;
            (s.hours, s.minutes, s.picked) = (rollers[0], rollers[1], picked);
        }
        _ => {
            for (font, text) in [(F18, "Aa 18 Font"), (F24, "Aa 24 Font"), (stock::FONT, "Aa 32 Font"), (F48, "Aa 48")] {
                room(b, flush)?;
                let l = b.create(Class::Label, page)?;
                b.style(l, Prop::TextFont, 0, font as i32)?;
                b.text(l, text)?;
            }
            room(b, flush)?;
            let card = stock::card(b, page, 200)?;
            let l = b.create(Class::Label, card)?;
            b.size(l, ROW_W - 72, 160)?;
            b.style(l, Prop::TextFont, 0, F24 as i32)?;
            b.style(l, Prop::TextColor, 0, GREY)?;
            b.text(l, "The quick brown fox jumps over the lazy dog.")?;
            b.align(l, Align::Center, 0, 0)?;
        }
    }
    room(b, flush)?;
    s.back[1] = button(b, page, "Back", Kind::Secondary)?;
    Ok(s)
}

/// A page of the pager: a centred column from the title line down, `pad` px under the last child.
fn pager_page(b: &mut Batch, page: Obj, pad: i32) -> Result<(), LvError> {
    b.style(page, Prop::PadTop, 0, stock::TITLE_Y)?;
    b.style(page, Prop::PadBottom, 0, pad)?;
    b.style(page, Prop::PadRow, 0, stock::ROW_GAP)?;
    b.flex(page, Flow::Column, FlexAlign::Start, FlexAlign::Center, FlexAlign::Start)
}

/// The pager section: three pages with dots. Page 1 a big number, page 2 rows taller than the screen (they scroll
/// first, then the pager pages on), page 3 "Back".
fn pages(b: &mut Batch, flush: &mut impl FnMut(&mut Batch) -> Result<(), LvError>) -> Result<Screen, LvError> {
    let p = stock::pager::<3>(b, 3)?;
    let mut s = Screen { page: p.root, section: Some(PAGES), pager: Some(p), ..EMPTY };
    let [one, two, three] = p.pages;
    pager_page(b, one, 0)?;
    let title = stock::title(b, one, "Pages")?;
    b.flags(title, &[Flag::Clickable], &[])?;
    b.events(title, &[Ev::Clicked])?;
    s.back[0] = title;
    let big = b.create(Class::Label, one)?;
    b.style(big, Prop::TextFont, 0, F48 as i32)?;
    b.text(big, "1 / 3")?;
    let hint = b.create(Class::Label, one)?;
    b.style(hint, Prop::TextColor, 0, GREY)?;
    b.style(hint, Prop::TextFont, 0, F24 as i32)?;
    b.text(hint, "Swipe up or turn the crown")?;
    room(b, flush)?;
    pager_page(b, two, stock::TITLE_Y)?;
    stock::title(b, two, "Long page")?;
    for (k, (letter, name)) in [("1", "One"), ("2", "Two"), ("3", "Three"), ("4", "Four"), ("5", "Five")].into_iter().enumerate() {
        room(b, flush)?;
        s.ticks[k] = stock::row(b, two, letter, name, "")?;
    }
    room(b, flush)?;
    pager_page(b, three, 0)?;
    stock::title(b, three, "Last page")?;
    let big = b.create(Class::Label, three)?;
    b.style(big, Prop::TextFont, 0, F48 as i32)?;
    b.text(big, "3 / 3")?;
    s.back[1] = button(b, three, "Back", Kind::Secondary)?;
    Ok(s)
}

/// "Alarm HH:MM".
pub fn alarm_text(h: u16, m: u16, buf: &mut [u8; 32]) -> &str {
    let pre = b"Alarm ";
    buf[..pre.len()].copy_from_slice(pre);
    let n = pre.len();
    buf[n..n + 5].copy_from_slice(&[b'0' + (h / 10 % 10) as u8, b'0' + (h % 10) as u8, b':', b'0' + (m / 10 % 10) as u8,
        b'0' + (m % 10) as u8]);
    core::str::from_utf8(&buf[..n + 5]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::boxed::Box;
    use wp2_core::lvcmd::{self, Table};

    struct Loader {
        t: Table,
        live_max: usize,
    }

    impl Loader {
        fn flush(&mut self, b: &mut Batch) -> Result<(), LvError> {
            if b.bytes().is_empty() {
                return Ok(());
            }
            assert_eq!(lvcmd::validate(b.bytes(), &self.t, 0b1111), Ok(()));
            for r in lvcmd::records(b.bytes()) {
                lvcmd::apply(&mut self.t, &r.unwrap());
            }
            let live = (1..=192u16).filter(|&h| self.t.class(h).is_some()).count();
            self.live_max = self.live_max.max(live);
            b.clear();
            Ok(())
        }
    }

    #[test]
    fn every_screen_validates_and_leaving_frees_it() {
        let mut f = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        fonts(&mut f).unwrap();
        assert_eq!(lvcmd::validate(f.bytes(), &Table::new(), 0), Ok(()));
        let mut b = Batch::with_buf(Box::leak(Box::new([0; BUF])));
        let mut l = Loader { t: Table::new(), live_max: 0 };
        theme(&mut b).unwrap();
        let menu_screen = menu(&mut b, &mut |b| l.flush(b)).unwrap();
        l.flush(&mut b).unwrap();
        let first = menu_screen.page;
        let mut s = menu_screen;
        for i in 0..N {
            leave(&mut b, &s).unwrap();
            s = section(&mut b, &mut |b| l.flush(b), i).unwrap();
            l.flush(&mut b).unwrap();
            assert_eq!(s.page, first, "section {i} reuses the freed handles");
            leave(&mut b, &s).unwrap();
            s = menu(&mut b, &mut |b| l.flush(b)).unwrap();
            l.flush(&mut b).unwrap();
        }
        std::println!("most live objects: {}", l.live_max);
        assert!(l.live_max < 80);
    }

    #[test]
    fn alarm() {
        let mut buf = [0u8; 32];
        assert_eq!(alarm_text(7, 5, &mut buf), "Alarm 07:05");
        assert_eq!(alarm_text(23, 59, &mut buf), "Alarm 23:59");
    }
}
