// "Apps" system widget for the cards left of the watch face (docs/recon.md, "Widget cards"). The patcher points
// the free widget registry slot 0x40 (wgt_reg 0x101EFC88) at WIDGET; a host puts it on a card with 0x906D
// (`wp2apps cards`). The stock card view lays the widget out, sizes and colours its container and opens
// TAP_VIEW on a click; the callbacks below only fill the container. They run on ui_service.
use core::mem::transmute;
use core::ptr::null_mut;
use wp2_core::heap::{self, RES};

use crate::applist::{icon_block, ImgDsc, ICON};

/// wgt_reg id; the stock table has NULL there
pub const ID: u8 = 0x40;
/// size class: index into the stock size table 0x101DF780 (0 circle 166, 1 square 166, 2 tall 166x344,
/// 3 wide 344x166, 4 full 344x344); 3 is the first style of every stock family
const WIDE: u8 = 3;
const W: i32 = 344;
const H: i32 = 166;
/// a view id at or below 0xF4: the stock click handler (0x101B1B14) opens it with OPEN_VIEW
const TAP_VIEW: u32 = 0xF0;

const OBJ_DEL: u32 = 0x1012C775;
const OBJ_ADD_FLAG: u32 = 0x10128D39;
const OBJ_SET_POS: u32 = 0x1012926D;
const OBJ_SET_SIZE: u32 = 0x101294C9;
const OBJ_SET_LOCAL_STYLE_PROP: u32 = 0x101C6F9B;
const LABEL_CREATE: u32 = 0x1013F5F1;
const LABEL_SET_TEXT: u32 = 0x101400A5;
const LABEL_SET_LONG_MODE: u32 = 0x1014051D;
const VIMG_CREATE: u32 = 0x1005A015;
const VIMG_SET_SRC: u32 = 0x1005A02D;
const FONT_OPEN: u32 = 0x10185C31;
const FONT_CLOSE: u32 = 0x10185D3D;

const STYLE_BG_COLOR: u32 = 0x20;
const STYLE_TEXT_COLOR: u32 = 0x55;
const STYLE_TEXT_FONT: u32 = 0x57;
const LONG_DOT: u32 = 1;
/// clicks on the icon and the text reach the container and its stock tap handler, as on the Custom Apps row
const EVENT_BUBBLE: u32 = 0x4000;
/// stock widget cards: #1A1C1F (docs/recon-lvgl.md); the icon's circle (#282C30) stays visible on it
const CARD_BG: u32 = 0x18E3;
const WHITE: u32 = 0xFFFF;
const FONT_PATH: &[u8] = b"/NAND:A/font32M.font\0";
const TEXT: &[u8] = b"Apps\0";
const PAD: i32 = (H - ICON as i32) / 2;

type Obj = *mut u8;

macro_rules! stock {
    ($addr:expr, fn($($t:ty),*) -> $r:ty) => {
        unsafe { transmute::<usize, extern "C" fn($($t),*) -> $r>($addr as usize) }
    };
}

/// A wgt_reg entry (0x101F0784 and on): {u8 id, u8 size class, u16 state size, u32 resource scene, u32 tap action,
/// 5 callbacks}. The card view allocates `state` bytes (zeroed) per instance; without state it skips every callback
/// but create and the first update.
#[repr(C)]
pub struct WgtReg {
    id: u8,
    class: u8,
    state: u16,
    scene: u32,
    tap: u32,
    delete: extern "C" fn(*mut Ctx) -> i32,
    create: extern "C" fn(*mut Ctx) -> i32,
    /// right after create, then from the card's 1 s timer while the card view is on top
    update: extern "C" fn(*mut Ctx) -> i32,
    focus: extern "C" fn(*mut Ctx, u32) -> i32,
    reload: extern "C" fn(*mut Ctx, u32) -> i32,
}

unsafe impl Sync for WgtReg {}

/// The card view's per-widget record (0x1C bytes from view data + 0x10): the first 16 bytes are a copy of the
/// view's presenter header, then the container, the state and the widget id.
#[repr(C)]
pub struct Ctx {
    presenter: [u32; 4],
    obj: Obj,
    state: *mut State,
    id: u16,
}

#[repr(C)]
pub struct State {
    font: [u32; 8],
    font_open: u32,
    icon: Obj,
    label: Obj,
    block: *mut u8,
}

pub static WIDGET: WgtReg = WgtReg {
    id: ID,
    class: WIDE,
    state: size_of::<State>() as u16,
    scene: 0,
    tap: TAP_VIEW,
    delete,
    create,
    update,
    focus,
    reload,
};

fn style(obj: Obj, prop: u32, value: u32) {
    stock!(OBJ_SET_LOCAL_STYLE_PROP, fn(Obj, u32, u32, u32) -> ())(obj, prop, value, 0)
}

extern "C" fn create(ctx: *mut Ctx) -> i32 {
    let ctx = unsafe { &mut *ctx };
    if ctx.obj.is_null() || ctx.state.is_null() {
        return 0;
    }
    let s = unsafe { &mut *ctx.state };
    style(ctx.obj, STYLE_BG_COLOR, CARD_BG);
    let set_pos = stock!(OBJ_SET_POS, fn(Obj, i32, i32) -> ());
    let set_size = stock!(OBJ_SET_SIZE, fn(Obj, i32, i32) -> ());
    if let Some((block, dsc)) = icon_block() {
        let img = stock!(VIMG_CREATE, fn(Obj) -> Obj)(ctx.obj);
        if img.is_null() {
            unsafe { heap::free(RES, block) };
        } else {
            s.block = block;
            s.icon = img;
            set_pos(img, PAD, PAD);
            set_size(img, ICON as i32, ICON as i32);
            stock!(VIMG_SET_SRC, fn(Obj, *const ImgDsc) -> ())(img, dsc);
            stock!(OBJ_ADD_FLAG, fn(Obj, u32) -> ())(img, EVENT_BUBBLE);
        }
    }
    s.font_open = (stock!(FONT_OPEN, fn(*mut u32, *const u8) -> i32)(s.font.as_mut_ptr(), FONT_PATH.as_ptr()) == 0) as u32;
    let label = stock!(LABEL_CREATE, fn(Obj) -> Obj)(ctx.obj);
    if label.is_null() {
        return 0;
    }
    s.label = label;
    stock!(OBJ_ADD_FLAG, fn(Obj, u32) -> ())(label, EVENT_BUBBLE);
    if s.font_open != 0 {
        style(label, STYLE_TEXT_FONT, s.font.as_ptr() as u32);
    }
    style(label, STYLE_TEXT_COLOR, WHITE);
    stock!(LABEL_SET_LONG_MODE, fn(Obj, u32) -> ())(label, LONG_DOT);
    stock!(LABEL_SET_TEXT, fn(Obj, *const u8) -> ())(label, TEXT.as_ptr());
    let line = if s.font_open != 0 { unsafe { *s.font.as_ptr().add(2) as i32 } } else { 32 }.clamp(1, H);
    let x = PAD + ICON as i32 + 20;
    set_pos(label, x, (H - line) / 2);
    set_size(label, W - x - PAD, line);
    0
}

// Children go before the font and the pixels they use; the stock then frees the state and deletes the screen.
extern "C" fn delete(ctx: *mut Ctx) -> i32 {
    let ctx = unsafe { &mut *ctx };
    if ctx.state.is_null() {
        return 0;
    }
    let s = unsafe { &mut *ctx.state };
    let del = stock!(OBJ_DEL, fn(Obj) -> ());
    for obj in [&mut s.label, &mut s.icon] {
        if !obj.is_null() {
            del(*obj);
            *obj = null_mut();
        }
    }
    if s.font_open != 0 {
        stock!(FONT_CLOSE, fn(*mut u32) -> ())(s.font.as_mut_ptr());
        s.font_open = 0;
    }
    if !s.block.is_null() {
        unsafe { heap::free(RES, s.block) };
        s.block = null_mut();
    }
    0
}

extern "C" fn update(_ctx: *mut Ctx) -> i32 {
    0
}

extern "C" fn focus(_ctx: *mut Ctx, _on: u32) -> i32 {
    0
}

extern "C" fn reload(_ctx: *mut Ctx, _arg: u32) -> i32 {
    0
}
