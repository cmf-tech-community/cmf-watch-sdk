// "Apps" system widget for the cards left of the watch face (docs/recon.md, "Widget cards"). The patcher points
// the free widget registry slot 0x40 (wgt_reg 0x101EFC88) at WIDGET; a host puts it on a card with 0x906D
// (`wp2apps cards`). The stock card view lays the widget out, sizes and colours its container and calls `tap` on a
// click; the callbacks below only fill the container. They run on ui_service. param_id 0 is "Apps", N the app in
// slot N - 1 with its widget record (docs/app-guide.md, "Widget").
use core::mem::transmute;
use core::ptr::null_mut;
use wp2_core::fs::{self, File};
use wp2_core::heap::{self, RES};
use wp2_core::layout::{slot_path, stub_ram, widget_path, WIDGET_REC, WIDGET_TITLE};

use crate::applist::{icon_block, ImgDsc, ICON};

/// wgt_reg id; the stock table has NULL there
pub const ID: u8 = 0x40;
/// size class: index into the stock size table 0x101DF780 (0 circle 166, 1 square 166, 2 tall 166x344,
/// 3 wide 344x166, 4 full 344x344); 3 is the first style of every stock family
const WIDE: u8 = 3;
const W: i32 = 344;
const H: i32 = 166;
/// the loader view, opened by `tap`
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
/// stock widget cards: the emulator frame reads (26, 28, 31) on them, as on the stock Settings row, whose RGB565 is
/// 5/11/6 (white reads 160 there, loader menu.rs ROW_BG). #1A1C1F taken literally (0x18E3) came out darker than a stock
/// widget next to it
const CARD_BG: u32 = 0x2966;
const WHITE: u32 = 0xFFFF;
/// an app widget's title, as the stock cards' grey captions
const GREY: u32 = 0x9CF3;
/// container user_data (lv_obj_t +0x10): the card entry's param_id, set by the stock layout before create
/// (emulator, docs/recon.md); 0 = "Apps", N = the app in slot N - 1
const USER_DATA: usize = 0x10;
const OPEN_VIEW: u32 = 0x101898BD;
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
    /// a view id ≤ 0xF4, or a function the stock click handler (0x101B1B14) tail-calls with r0 = the container
    tap: extern "C" fn(Obj),
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
    value: Obj,
    block: *mut u8,
}

pub static WIDGET: WgtReg = WgtReg {
    id: ID,
    class: WIDE,
    state: size_of::<State>() as u16,
    scene: 0,
    tap,
    delete,
    create,
    update,
    focus,
    reload,
};

fn style(obj: Obj, prop: u32, value: u32) {
    stock!(OBJ_SET_LOCAL_STYLE_PROP, fn(Obj, u32, u32, u32) -> ())(obj, prop, value, 0)
}

fn param(obj: Obj) -> u8 {
    unsafe { *obj.add(USER_DATA) }
}

fn label(s: &State, parent: Obj, text: *const u8, color: u32, x: i32, y: i32, h: i32) -> Obj {
    let label = stock!(LABEL_CREATE, fn(Obj) -> Obj)(parent);
    if label.is_null() {
        return label;
    }
    stock!(OBJ_ADD_FLAG, fn(Obj, u32) -> ())(label, EVENT_BUBBLE);
    if s.font_open != 0 {
        style(label, STYLE_TEXT_FONT, s.font.as_ptr() as u32);
    }
    style(label, STYLE_TEXT_COLOR, color);
    stock!(LABEL_SET_LONG_MODE, fn(Obj, u32) -> ())(label, LONG_DOT);
    stock!(LABEL_SET_TEXT, fn(Obj, *const u8) -> ())(label, text);
    stock!(OBJ_SET_POS, fn(Obj, i32, i32) -> ())(label, x, y);
    stock!(OBJ_SET_SIZE, fn(Obj, i32, i32) -> ())(label, W - x - 24, h);
    label
}

/// `len` bytes of `path` into `buf`; false if the file is missing or short
fn read(path: &[u8], buf: &mut [u8]) -> bool {
    let mut file: File = [0; 0x2C];
    if fs::open(&mut file, path, fs::READ) < 0 {
        return false;
    }
    let ok = fs::read_exact(&mut file, buf);
    fs::close(&mut file);
    ok
}

extern "C" fn create(ctx: *mut Ctx) -> i32 {
    let ctx = unsafe { &mut *ctx };
    if ctx.obj.is_null() || ctx.state.is_null() {
        return 0;
    }
    let s = unsafe { &mut *ctx.state };
    style(ctx.obj, STYLE_BG_COLOR, CARD_BG);
    s.font_open = (stock!(FONT_OPEN, fn(*mut u32, *const u8) -> i32)(s.font.as_mut_ptr(), FONT_PATH.as_ptr()) == 0) as u32;
    let line = if s.font_open != 0 { unsafe { *s.font.as_ptr().add(2) as i32 } } else { 32 }.clamp(1, H);
    let slot = param(ctx.obj).wrapping_sub(1);
    if slot < wp2_core::layout::SLOTS {
        // ponytail: read once per layout; the card is laid out again whenever the face shows, and an app only
        // runs (and writes its record) under the loader view, which deletes the cards (emulator, docs/recon.md)
        let mut rec = [0u8; WIDGET_REC + 1];
        if !read(&widget_path(slot), &mut rec[..WIDGET_REC]) {
            // no record yet: the package name (header +8, 24 bytes NUL-padded) and no value
            let mut head = [0u8; 8 + WIDGET_TITLE];
            if !read(&slot_path(slot, false), &mut head) {
                head[8..13].copy_from_slice(TEXT);
            }
            rec = [0; WIDGET_REC + 1];
            rec[..WIDGET_TITLE].copy_from_slice(&head[8..]);
        }
        rec[WIDGET_TITLE - 1] = 0;
        s.label = label(s, ctx.obj, rec.as_ptr(), GREY, 24, 24, line);
        s.value = label(s, ctx.obj, rec[WIDGET_TITLE..].as_ptr(), WHITE, 24, 24 + line + 8, H - 48 - line - 8);
        return 0;
    }
    if let Some((block, dsc)) = icon_block() {
        let img = stock!(VIMG_CREATE, fn(Obj) -> Obj)(ctx.obj);
        if img.is_null() {
            unsafe { heap::free(RES, block) };
        } else {
            s.block = block;
            s.icon = img;
            stock!(OBJ_SET_POS, fn(Obj, i32, i32) -> ())(img, PAD, PAD);
            stock!(OBJ_SET_SIZE, fn(Obj, i32, i32) -> ())(img, ICON as i32, ICON as i32);
            stock!(VIMG_SET_SRC, fn(Obj, *const ImgDsc) -> ())(img, dsc);
            stock!(OBJ_ADD_FLAG, fn(Obj, u32) -> ())(img, EVENT_BUBBLE);
        }
    }
    s.label = label(s, ctx.obj, TEXT.as_ptr(), WHITE, PAD + ICON as i32 + 20, (H - line) / 2, line);
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
    for obj in [&mut s.value, &mut s.label, &mut s.icon] {
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

// r0 = the container (stock 0x101B1B14 tail-calls the action with the current target, emulator): the loader takes
// `launch` at LAYOUT and starts that slot instead of showing the menu
extern "C" fn tap(obj: Obj) {
    stub_ram().launch = param(obj);
    stock!(OPEN_VIEW, fn(u32, u32) -> i32)(TAP_VIEW, 0);
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
