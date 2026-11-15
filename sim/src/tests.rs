use super::*;

fn cfg(script: &str) -> Config {
    let mut c = Config::new("test");
    c.timeline = script::parse(script).unwrap();
    c.store = std::env::temp_dir().join(format!("wp2-sim-test-{}-{:?}", std::process::id(), thread::current().id()));
    let _ = fs::remove_dir_all(&c.store);
    c
}

fn sim(script: &str) -> Sim {
    Sim::new(cfg(script))
}

fn kinds(s: &Sim, n: usize) -> Vec<(u8, u32)> {
    (0..n).filter_map(|_| s.next_event(100).map(|e| (e.kind, s.lock().now()))).collect()
}

#[test]
fn script_parses_to_a_timeline() {
    let t = script::parse("# x\nt 10 20\nwait 100\nc -2; b\nm hi there\nmx 00ff\nshot a\nt 1 2 500\np").unwrap_err();
    assert!(t.contains("line 4"), "{t}");
    let t = script::parse("t 10 20\nwait 100\nc -2\nb\nm hi there\nmx 00ff\nshot a\nt 1 2 500\np").unwrap();
    assert_eq!(t, vec![
        (0, Action::Down(10, 20)),
        (80, Action::Up(10, 20)),
        (100, Action::Crown(-2)),
        (100, Action::Button),
        (100, Action::Msg(b"hi there".to_vec())),
        (100, Action::Msg(vec![0, 255])),
        (100, Action::Shot("a".into())),
        (100, Action::Down(1, 2)),
        (100, Action::Pause),
        (600, Action::Up(1, 2)),
    ]);
}

#[test]
fn timeouts_follow_the_virtual_clock() {
    let s = sim("wait 700\nc 1");
    assert_eq!(s.next_event(500), None);
    let now = s.lock().now();
    assert_eq!(now, 500);
    let e = s.next_event(u32::MAX).unwrap();
    let now = s.lock().now();
    assert_eq!((e.kind, e.delta, now), (CROWN, 1, 700));
    assert_eq!(s.next_event(0), None);
    let now = s.lock().now();
    assert_eq!(now, 700);
    // nothing left to wait for: the run is idle
    assert_eq!(s.next_event(u32::MAX), None);
    let end = s.lock().end;
    assert_eq!(end, Some(End::Idle));
}

#[test]
fn run_ends_at_until() {
    let mut c = cfg("");
    c.until = Some(300);
    let s = Sim::new(c);
    assert_eq!(s.next_event(u32::MAX), None);
    let st = s.lock();
    assert_eq!((st.end, st.now()), (Some(End::Timeout), 300));
}

#[test]
fn queue_keeps_15_and_pause_replaces_it() {
    let s = sim("c 20");
    s.lock().pump();
    assert_eq!(kinds(&s, 20).len(), 15);
    let s = sim("c 3\np\nc 2\nwait 10\np\nc -1");
    let got = kinds(&s, 10);
    // PAUSE drops the 3 queued detents, input while paused is dropped, RESUME comes before later input
    assert_eq!(got, vec![(PAUSE, 0), (RESUME, 10), (CROWN, 10)]);
}

#[test]
fn resume_republishes_frames_drawn_while_paused() {
    let s = sim("");
    let mut st = s.lock();
    st.fill_rect(0, 0, 10, 10, 0xFFFF);
    st.present();
    st.set_paused(true);
    st.fill_rect(0, 0, 10, 10, 0x001F);
    st.present();
    assert_eq!(st.shown[0], 0xFFFF);
    st.set_paused(false);
    assert_eq!(st.shown[0], 0x001F);
}

#[test]
fn motor_cap_locks_until_off() {
    let s = sim("");
    assert_eq!(s.lock().motor(1001, 31), -1);
    assert_eq!(s.lock().motor(500, 0), -1);
    s.next_event(100);
    assert_eq!(s.lock().motor(500, 31), 0);
    assert_eq!(s.next_event(1900), None);
    let on = s.lock().motor;
    assert!(on.is_some());
    // a call while on keeps the first start: the cut comes 1980 ms after t = 100
    assert_eq!(s.lock().motor(800, 31), 0);
    s.next_event(100);
    let on = s.lock().motor;
    assert_eq!(on, None);
    assert_eq!(s.lock().motor(500, 31), -4);
    assert_eq!(s.lock().motor(0, 31), 0);
    assert_eq!(s.lock().motor(500, 31), 0);
    s.lock().set_paused(true);
    let on = s.lock().motor;
    assert_eq!(on, None);
    assert_eq!(s.lock().motor(500, 31), -3);
}

#[test]
fn button_force_stops_after_a_second() {
    let s = sim("wait 50\nb");
    assert_eq!(s.next_event(u32::MAX).map(|e| e.kind), Some(BUTTON));
    assert_eq!(s.next_event(5000), None);
    let st = s.lock();
    assert_eq!((st.end, st.now()), (Some(End::Killed), 1050));
}

#[test]
fn owned_back_is_not_killed_once_read() {
    let s = sim("wait 50\nb\nwait 2000\nb");
    s.lock().own_back(true);
    assert_eq!(s.next_event(u32::MAX).map(|e| e.kind), Some(BUTTON));
    assert_eq!(s.next_event(u32::MAX).map(|e| e.kind), Some(BUTTON));
    assert_eq!(s.next_event(3000), None);
    assert_eq!(s.lock().end, None);
}

#[test]
fn owned_back_unread_is_still_killed() {
    let s = sim("wait 50\nb");
    s.lock().own_back(true);
    let mut st = s.lock();
    st.us = 100_000; // the press at 100 ms, never popped, no next_event either: hung
    st.pump();
    st.us = 5_099_000;
    st.pump();
    assert_eq!(st.end, None, "an owner gets HANG_MS");
    st.us = 5_100_000;
    st.pump();
    drop(st);
    assert_eq!(s.lock().end, Some(End::Killed));
}

#[test]
fn owned_back_busy_owner_is_not_killed() {
    // busy 2 s after the press (no next_event), then reads it: alive, as a view rebuild or a save
    let s = sim("wait 50\nb");
    s.lock().own_back(true);
    let mut st = s.lock();
    st.us = 100_000;
    st.pump();
    st.us = 2_100_000;
    st.pump();
    drop(st);
    assert_eq!(s.next_event(0).map(|e| e.kind), Some(BUTTON));
    assert_eq!(s.lock().end, None);
}

#[test]
fn messages_inbox_of_4_and_outbox_poll() {
    let s = sim("m a\nm bb\nm ccc\nm dddd\nm eeeee");
    let mut buf = [0u8; MSG_MAX];
    let got: Vec<(u8, i16)> = (0..4).map(|_| s.next_event(0).map(|e| (e.kind, e.x)).unwrap()).collect();
    assert_eq!(got, [(MESSAGE, 1), (MESSAGE, 2), (MESSAGE, 3), (MESSAGE, 4)]);
    // the fifth waits for room, like a FULL reply the host retries
    assert_eq!(s.next_event(0), None);
    assert_eq!(s.lock().recv(&mut [0u8; 1]), 1);
    assert_eq!(s.lock().recv(&mut [0u8; 1]), E_RANGE);
    assert_eq!(s.lock().recv(&mut buf), 2);
    assert_eq!(s.lock().recv(&mut buf), 3);
    assert_eq!(&buf[..3], b"ccc");
    assert_eq!(s.next_event(0).map(|e| (e.kind, e.x)), Some((MESSAGE, 5)));
    let mut st = s.lock();
    assert_eq!(st.send(b""), E_RANGE);
    assert_eq!(st.send(&[0; MSG_MAX + 1]), E_RANGE);
    for _ in 0..4 {
        assert_eq!(st.send(b"x"), 0);
    }
    assert_eq!(st.send(b"x"), E_AGAIN);
    drop(st);
    s.next_event(250);
    assert_eq!(s.lock().send(b"y"), 0);
    let n = s.lock().sent.len();
    assert_eq!(n, 3);
    let mut c = cfg("");
    c.tcp = Some(1);
    assert_eq!(Sim::new(c).lock().send(b"x"), E_NOTCONN);
}

#[test]
fn storage_round_trip_and_limits() {
    let s = sim("");
    let mut st = s.lock();
    let mut buf = [0u8; STORE_MAX];
    assert_eq!(st.load(&mut buf), 0);
    assert_eq!(st.save(&[7; STORE_MAX + 1]), E_RANGE);
    assert_eq!(st.save(&[7; STORE_MAX]), 0);
    assert_eq!(st.load(&mut buf[..10]), E_RANGE);
    assert_eq!(st.load(&mut buf), STORE_MAX as i32);
    assert_eq!(st.save(b"abc"), 0);
    assert_eq!(st.load(&mut buf), 3);
    assert_eq!(&buf[..3], b"abc");
    assert!(!st.cfg.store.join("data.new").exists());
    assert_eq!(st.save(b""), 0);
    assert_eq!(st.load(&mut buf), 0);
    let _ = fs::remove_dir_all(&st.cfg.store);
}

#[test]
fn storage_survives_a_cut_save() {
    let s = sim("");
    let mut st = s.lock();
    let mut buf = [0u8; 16];
    assert_eq!(st.save(b"old"), 0);
    let (dat, new) = st.store();
    // cut while writing data.new: the old blob stays
    let mut half = store_header(b"newer").to_vec();
    half.extend(b"ne");
    fs::write(&new, &half).unwrap();
    assert_eq!(st.load(&mut buf), 3);
    assert_eq!(&buf[..3], b"old");
    // cut after a complete data.new on a store without rename-over: data.new is used
    fs::remove_file(&dat).unwrap();
    let mut full = store_header(b"newer").to_vec();
    full.extend(b"newer");
    fs::write(&new, &full).unwrap();
    assert_eq!(st.load(&mut buf), 5);
    // both corrupt
    fs::write(&new, b"WP2Dxxxx").unwrap();
    assert_eq!(st.load(&mut buf), E_IO);
    let _ = fs::remove_dir_all(&st.cfg.store);
}

#[test]
fn battery_and_time() {
    assert_eq!(battery("80"), Ok(80));
    assert_eq!(battery("15c"), Ok(15 | 0x500));
    assert_eq!(battery("100f"), Ok(100 | 0x600));
    assert_eq!(battery("none"), Ok(E_IO));
    let mut c = cfg("");
    c.epoch_ms = Some(1_794_000_000_000);
    let t = Sim::new(c).lock().time().unwrap();
    let back = (i64::from(t.hour) * 60 + i64::from(t.minute) - i64::from(t.utc_offset_min)).rem_euclid(1440);
    assert_eq!(back, (1_794_000_000 / 60) % 1440);
    assert_eq!(t.year, 2026);
}

#[test]
fn panel_mask_is_round() {
    assert!(out::on_panel(233, 233) && out::on_panel(0, 233) && out::on_panel(233, 465));
    assert!(!out::on_panel(0, 0) && !out::on_panel(465, 465) && !out::on_panel(30, 30));
    assert_eq!(out::rgb(0xFFFF), [255; 3]);
    assert_eq!(out::rgb(0xF800), [255, 0, 0]);
}

#[test]
fn json_escapes() {
    let line = out::json(5, "send", &[("text", V::S("a\"\n")), ("n", V::I(-1))]);
    assert_eq!(line, r#"{"t":5,"ev":"send","text":"a\"\u000a","n":-1}"#);
}

#[test]
fn lv_submit_is_not_supported() {
    assert!(API.abi_minor >= 2);
    assert_eq!((API.lv_submit)(std::ptr::null(), 0), -95);
    assert_eq!(API.lv().unwrap().submit(&[1, 3, 1, 0, 1, 0, 0]), -95);
}
