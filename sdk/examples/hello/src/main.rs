#![no_std]
#![no_main]

use cmf_wp2_app::{Api, BUTTON, CROWN};

cmf_wp2_app::entry!(main);

const BACKGROUNDS: [u16; 4] = [0x0000, 0x001F, 0x07E0, 0xF800];
const NAMES: [&str; 4] = ["Colour: black", "Colour: blue", "Colour: green", "Colour: red"];

fn main(api: &'static Api) -> i32 {
    let mut background = 0;
    api.widget("Hello", NAMES[background]);
    loop {
        api.rect(0, 0, api.width, api.height, BACKGROUNDS[background]);
        api.text(api.width as i16 / 2 - 20, api.height as i16 / 2 - 8, "hello", 0xFFFF);
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
