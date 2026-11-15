#![cfg_attr(any(target_os = "none", wp2_sim), no_main, cfg_attr(target_os = "none", no_std))]

mod lab;

#[cfg(not(any(target_os = "none", wp2_sim)))]
fn main() {}

#[cfg(any(target_os = "none", wp2_sim))]
mod device {
    use crate::lab::{Field, Pattern, Player, Settings};
    use cmf_wp2_app::{Api, BUTTON, CROWN, PAUSE, RESUME, TOUCH_DOWN};
    use core::fmt::Write;

    cmf_wp2_app::entry!(main);
    cmf_wp2_app::app_icon!();
    cmf_wp2_app::app_features!(cmf_wp2_app::WIDGET);

    const FIELDS: [Field; 4] = [Field::Strength, Field::Frequency, Field::Pattern, Field::Pulse];
    const ROW_TOP: i16 = 110;
    const ROW_HEIGHT: i16 = 50;
    const PLAY_TOP: i16 = 330;
    const PLAY_HEIGHT: i16 = 60;
    const BLACK: u16 = 0x0000;
    const WHITE: u16 = 0xFFFF;
    const GREY: u16 = 0x39E7;
    const ACCENT: u16 = 0xFD20;

    struct Line {
        buf: [u8; 32],
        len: usize,
    }

    impl Line {
        fn new() -> Self {
            Line { buf: [0; 32], len: 0 }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
        }
    }

    impl Write for Line {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let n = s.len().min(self.buf.len() - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
            self.len += n;
            Ok(())
        }
    }

    fn label(settings: &Settings, field: Field) -> Line {
        let mut line = Line::new();
        let _ = match field {
            Field::Strength => write!(line, "Strength {}%", settings.duty_permille / 10),
            Field::Frequency => write!(line, "Freq {}Hz", settings.freq_hz),
            Field::Pattern => write!(line, "Pattern {}", match settings.pattern {
                Pattern::Continuous => "Long",
                Pattern::Tick => "Tick",
                Pattern::DoubleTick => "Double",
            }),
            Field::Pulse => write!(line, "Pulse {}ms", settings.pulse_ms),
        };
        line
    }

    fn draw(api: &Api, settings: &Settings, selected: usize) {
        let x = api.width as i16 / 2 - 100;
        api.rect(0, 0, api.width, api.height, BLACK);
        for (row, &field) in FIELDS.iter().enumerate() {
            let y = ROW_TOP + row as i16 * ROW_HEIGHT;
            if row == selected {
                api.rect(x - 16, y - 8, 232, (ROW_HEIGHT - 8) as u16, GREY);
            }
            api.text(x, y, label(settings, field).as_str(), if row == selected { ACCENT } else { WHITE });
        }
        api.rect(x, PLAY_TOP, 200, PLAY_HEIGHT as u16, ACCENT);
        api.text(x + 76, PLAY_TOP + 20, "PLAY", BLACK);
        api.show();
    }

    fn play(api: &Api, settings: &Settings) -> Option<i32> {
        let mut player = Player::start(settings);
        while let Some(step) = player.next() {
            api.vibrate(step.duty_permille, step.freq_hz);
            let deadline = api.now().wrapping_add(step.ms as u32);
            loop {
                let left = deadline.wrapping_sub(api.now()) as i32;
                if left <= 0 {
                    break;
                }
                match api.event(left as u32).map(|e| e.kind) {
                    Some(PAUSE) => {
                        let off = player.pause();
                        api.vibrate(off.duty_permille, off.freq_hz);
                        return None;
                    }
                    Some(BUTTON) => {
                        api.vibrate(0, 0);
                        return Some(0);
                    }
                    _ => {}
                }
            }
        }
        None
    }

    // the card widget (docs/app-guide.md, "Widget"): the last preset, on exit
    fn widget(api: &Api, settings: &Settings) {
        let mut value = Line::new();
        let _ = write!(value, "{}\n{}", label(settings, Field::Strength).as_str(), label(settings, Field::Pattern).as_str());
        api.widget("Vibration Lab", value.as_str());
    }

    fn main(api: &'static Api) -> i32 {
        let mut settings = Settings::default();
        let code = run(api, &mut settings);
        widget(api, &settings);
        code
    }

    fn run(api: &Api, settings: &mut Settings) -> i32 {
        let mut selected = 0;
        draw(api, settings, selected);
        loop {
            let Some(event) = api.event(u32::MAX) else { continue };
            match event.kind {
                BUTTON => {
                    api.vibrate(0, 0);
                    return 0;
                }
                CROWN if event.delta != 0 => settings.adjust(FIELDS[selected], event.delta.signum()),
                TOUCH_DOWN if event.y >= PLAY_TOP && event.y < PLAY_TOP + PLAY_HEIGHT => {
                    if let Some(code) = play(api, settings) {
                        return code;
                    }
                }
                TOUCH_DOWN if event.y >= ROW_TOP - 8 && event.y < ROW_TOP - 8 + ROW_HEIGHT * FIELDS.len() as i16 => {
                    selected = ((event.y - (ROW_TOP - 8)) / ROW_HEIGHT) as usize;
                }
                RESUME => {}
                _ => continue,
            }
            draw(api, settings, selected);
        }
    }
}
