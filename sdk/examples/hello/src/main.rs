#![no_std]
#![no_main]

use cmf_wp2_app::{Api, BUTTON, CROWN};

cmf_wp2_app::entry!(main);
cmf_wp2_app::app_features!(cmf_wp2_app::WIDGET | cmf_wp2_app::PAGE);

const BACKGROUNDS: [u16; 4] = [0x0000, 0x001F, 0x07E0, 0xF800];
const NAMES: [&str; 4] = ["Colour: black", "Colour: blue", "Colour: green", "Colour: red"];

fn main(api: &'static Api) -> i32 {
    let mut background = 0;
    // as the page next to the watch face the same screen says so
    let text = if api.on_page() { "hello page" } else { "hello" };
    api.widget("Hello", NAMES[background]);
    loop {
        api.rect(0, 0, api.width, api.height, BACKGROUNDS[background]);
        api.text(api.width as i16 / 2 - 4 * text.len() as i16, api.height as i16 / 2 - 8, text, 0xFFFF);
        api.show();
        let Some(event) = api.event(u32::MAX) else { continue };
        match event.kind {
            BUTTON => return 0,
            CROWN => {
                background = (background + 1) % BACKGROUNDS.len();
                api.widget("Hello", NAMES[background]);
            }
            _ => {}
        }
    }
}
