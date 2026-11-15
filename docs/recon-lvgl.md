# Stock LVGL in app.bin: reconnaissance for ABI 2

Firmware: JX402 `1.0.0.73`, `app.bin` sha256 prefix `30e7978c39d8fb79`, base `0x10000000`. Addresses are
function entries (even); call them with bit 0 set. Same confidence rules as `docs/recon.md`: **high** = read
from code and consistent with a second source (emulator, callers, upstream source); **medium** = read from code
only; **low** = inferred.

Upstream reference: LVGL v8.3.0–v8.3.11 (MIT), cloned into `runs/lvgl-src/` (not tracked).

## Tools and how to reproduce

| Tool | What it gives |
|---|---|
| `uv run --locked python scripts/recon/lvgl_inventory.py` | Markdown tables: ~230 LVGL functions named by their `__func__` string, every class derived from `lv_obj_class` with instance size and `create`, all `lv_obj_set_style_*` setters. Needs `runs/recon/app.bin` (`scripts/recon/extract.py`). No Ghidra |
| `scripts/recon/gui_heap_probe.py [fixtures]` | Emulator walk over stock screens: GUI heap free bytes and the largest block on each screen, frames, the thread that runs `lv_timer_handler`, the display theme. Reuses `tests/e2e/harness.Emu`; needs `nix-shell -p gdb` |
| `uv run --locked python scripts/recon/thumb_dis.py ADDR [N]` | Thumb disassembly of `app.bin` (capstone) |
| `scripts/recon/decomp` | Ghidra decompile. Ghidra headless refuses any path with an element starting with `.`, so from a herdr worktree (`~/.herdr/…`) run it through a symlink without dots |

Method notes. `LV_USE_ASSERT_OBJ` is on, so every LVGL function with an assert loads its own `__func__` string;
the function is the last `push {…, lr}` before that literal load. 21 of these rows are not call targets
(variadic prologues `push {r1-r3}`, GCC `.part` clones, destructors reached only by pointer); the script marks
them, and the addresses below were checked against callers or the disassembly. LVGL code sits in two places:
asserting functions at `0x10125000–0x10146000`, small functions without asserts at `0x101C4000–0x101CE100`
(in upstream file order in both). Small functions were named by shape and order against upstream source.

## LVGL configuration (question 2)

| Item | Value | Confidence | How |
|---|---|---|---|
| Version | LVGL **8.3.x**, Actions fork (`thirdparty/lib/gui/lvgl`, `porting/gpu/lv_gpu_acts*.c`, VG-Lite, `lv_disp_add_knob_obj`, `lv_label_set_dot_tmp`). Style property ids match v8.3.10/8.3.11 (`TRANSLATE_X = 107`), but Actions also added properties (ids 45, 92, 93), so the patch level is not proven | high (8.3), low (patch) | `__FILE__` paths, string diff against all 8.3 tags, style ids from setters |
| `LV_COLOR_DEPTH` | **16** (RGB565): `lv_color_t` passed as 16-bit, `lv_color_to32` = `(c>>11)*0x107…` at `0x101C84A8`; `LV_COLOR_16_SWAP` not seen (plain 565 conversion) | high / swap medium | decompile + setters |
| `lv_coord_t` | **int32** (`LV_USE_LARGE_COORD 1`): `LV_SIZE_CONTENT = 0x200007D1`, `LV_PCT(100) = 0x20000064` in class defaults | high | class descriptors |
| `LV_USE_USER_DATA` | 1: `lv_obj_class_t` is 0x20 B with `user_data`; `lv_anim_t` is 0x44 B with `user_data` at `+0x18` | high | descriptors, `lv_anim_init` |
| `sizeof(lv_obj_t)` | **44** (`instance_size` of `lv_obj_class` and `lv_btn_class`) | high | class descriptor `0x101FB27C` |
| `lv_obj_class_t` | `{base, constructor, destructor, user_data, event_cb, i32 width_def, i32 height_def, u32 editable:2 group_def:2 instance_size:16}` = 0x20 B | high | 28 descriptors |
| `LV_DPI_DEF` | 130 (`lv_obj_class.width_def = 0x82`) | high | descriptor |
| Flex / grid | both on: `lv_flex_init 0x101334D4` and `lv_grid_init 0x10133D78` call `lv_layout_register 0x10129680` and register 5 and 10 style props | high | callers |
| Asserts / logs | `LV_USE_ASSERT_OBJ`, `ASSERT_NULL`, `ASSERT_MALLOC` on; an assert logs and calls `0x100EEB30(file, line)` (system assert) | high | decompile |
| `lv_mem` | `LV_MEM_CUSTOM`: `lv_mem_alloc(size, tag) 0x1013A14C` → `ui_mem_alloc(1, …)`, `lv_mem_free 0x1013A160` → `ui_mem_free(1, …)`, `lv_mem_realloc 0x1013A178` → `ui_mem_realloc(1, …)`. So LVGL lives in the `GUI` heap of `ui_mem`, 0x18000 B. Note the second `tag` argument (C string, used in failure logs) | high | disassembly |
| GUI heap | **Zephyr `sys_heap`**, not JHeap: init `0x10188630` = `sys_heap_init(0x3807F300, 0x38067300, 0x18000)`; ops `0x1018864C` alloc, `0x1018866C` aligned, `0x1018867C` realloc, `0x101886A4` free. Small heap: 8-byte chunks, u16 headers, 0x2FFF chunks. Unlike JHeap, `sys_heap_alloc` returns NULL on failure; what `lv_mem_alloc` callers do with NULL is LVGL's usual `LV_ASSERT_MALLOC` → assert → reset | high / NULL path medium | disassembly + emulator walk of the chunk list |
| GUI thread check | every GUI alloc/free first calls `0x101885F8`: remembers the first thread that allocated (`*0x38043894`) and prints `"%s called in thread 0x%x"` when another thread calls. A warning, not a lock | high | disassembly + emulator (`*0x38043894 = 0x2FFB6740` = `ui_service`) |
| Display | `lv_disp_t` is 0x278 B (Actions-extended); `_lv_disp_def` at `0x3802AF24` (`lv_disp_get_default 0x10138DCC`); `+0x10` act_scr, `+0x1C` top layer, `+0x20` sys layer, `+0x268` current knob object, `+0x26C` knob object list | high (offsets used in code) | decompile |

## Widget and API inventory (question 1)

### Widget classes present

`lv_switch`, `lv_checkbox`, `lv_dropdown`, `lv_list`, `lv_msgbox`, `lv_textarea`, `lv_keyboard`, `lv_spinbox`,
`lv_table`, `lv_tabview`, `lv_menu`, `lv_meter`, `lv_spinner`, `lv_line`, `lv_led`, `lv_win` are **not in the
image** (no class descriptor derives from `lv_obj_class`; the class scan covers the whole image). Confidence high.

| Widget | Class | Instance size | `create(parent)` | Key functions (entry) | Confidence |
|---|---|---|---|---|---|
| `lv_obj` | `0x101FB27C` | 44 | `0x10127CFC` | see "core" below | high |
| `lv_label` | `0x101FB7B0` | 96 | `0x1013F5F0` | `set_text 0x101400A4`, `set_text_fmt 0x10140238` (variadic), `set_text_static 0x10140368`, `set_long_mode 0x1014051C`, `get_text 0x1013F6C0`, `set_max_height 0x1013F614` (Actions) | high |
| `lv_btn` | `0x101FB730` | 44 | `0x1013D388` | none of its own (plain `lv_obj` with `group_def` on) | high |
| `lv_img` | `0x101FB790` | 80 | `0x1013ED38` | `set_src 0x1013ED50`; zoom/angle setters not found by name | high / zoom low |
| `lv_arc` | `0x101FB6F0` | 72 | `0x1013BA54` | `set_value 0x1013C108`, `get_value 0x1013C2EC`, `set_start_angle 0x1013BA6C`, `set_end_angle 0x1013BB84`, `set_angles 0x101CCC6A`, `set_bg_start_angle 0x1013BE34`, `set_bg_end_angle 0x1013BF44`, `set_rotation 0x1013C054`, `set_mode 0x1013C1E0` | high (`set_angles` medium) |
| `lv_bar` | `0x101FB710` | 112 | `0x1013CEA0` | `set_value(obj, v, anim) 0x1013CEB8`, `set_start_value 0x1013CF94`, `set_range 0x1013D294`, `get_value 0x1013D078` | high |
| `lv_slider` | `0x101FB810` (base bar) | 152 | `0x10141EB8` | value via `lv_bar_*` (upstream slider setters are inline wrappers) | high |
| `lv_roller` | `0x101FB7D0` (+ `lv_roller_label` `0x101FB7F0`) | 72 | `0x10140E00` | `set_options 0x10140E18`, `set_selected 0x10141044`, `set_visible_row_count 0x101415B8`, `get_selected 0x10141690`, `get_selected_str 0x10141750` | high |
| `lv_canvas` | `0x101FB770` (base img) | 92 | `0x1013E514` | `set_buffer 0x1013E52C`, `set_px_color 0x1013E64C`, `fill_bg 0x1013E9AC`, `draw_line 0x1013EB54`, `set_palette 0x1013E710`, `get_img 0x1013E7CC` | high |
| `lv_btnmatrix` | `0x101FB750` | 64 | `0x1013D738` | `set_map 0x1013D750`, `get_selected_btn 0x1013DE80`, ctrl setters `0x1013DAE8…0x1013DF34` | high |
| `lv_chart` | `0x101FB308` | 180 | `0x10135518` | `set_type`, `set_point_count`, `set_range`, `add_series`, `set_next_value` … (script table) | high |
| `lv_imgbtn` | `0x101FB328` | 120 | `0x10137A3C` | `set_src 0x10137BE4`, `set_state 0x10137CA8` | high |
| `lv_tileview` / tile | `0x101FB348` / `0x101FB368` | 48 / 48 | `0x101380A0` / `add_tile 0x101380B8` | `lv_obj_set_tile_id 0x101380E8`; `lv_obj_set_tile 0x101CB64A` (stores `tile_act` at tv+0x2C, `set_scroll_dir 0x101C6474` with the tile's dir at tile+0x2C, then `lv_obj_scroll_to 0x101C6650` = `scroll_to_x 0x1012B354` + `scroll_to_y 0x1012B380`); `get_tile_act 0x101CB680`. `add_tile(tv, col, row, dir)` stores the three in statics for the tile constructor. Scroll: `lv_obj_get_scroll_y 0x101C6514` (`-spec_attr->scroll.y`, spec_attr +0x14), `get_scroll_top 0x101C651E` (tail call to it), `get_scroll_dir 0x101C64DC`; `lv_obj_scroll_by 0x101C6598`. Used by the pager (`docs/abi-1.4.md`), emulator-proven | high (names medium) |
| `lv_qrcode` | `0x101FB2A8` (base canvas) | — | `0x10134228` | `update 0x10134258` | high |
| `lv_barcode` | (canvas) | — | `0x10133ECC` | `update 0x10133F80`, `delete 0x10134130` | high |

### Core object API

| Function | Entry | Confidence | How |
|---|---|---|---|
| `lv_obj_class_create_obj(class, parent)` / `lv_obj_class_init_obj(obj)` | `0x10128E70` / `0x101C5534` | high | `__func__`; pattern of every `create` |
| `lv_obj_del` / `lv_obj_clean` | `0x1012C774` / `0x1012C880` | high | `__func__` |
| `lv_obj_set_pos`, `_x`, `_y`, `_size`, `_width`, `_height` | `0x1012926C`, `0x101290CC`, `0x1012919C`, `0x101294C8`, `0x10129328`, `0x101293F8` | high | `__func__` |
| `lv_obj_set_align(obj, align)` / `lv_obj_align(obj, align, x, y)` | `0x101C5DDE` / `0x101C5DE4` | medium | shape: `set_style_align` + `set_pos` |
| `lv_obj_align_to`, `lv_obj_set_layout` | `0x1012A87C`, `0x101295B8` | high | `__func__` |
| `lv_obj_get_x/_y/_width/_height/_coords/_content_width` | `0x101297DC`, `0x101298B8`, `0x10129994`, `0x10129A4C`, `0x10129718`, `0x10129BC8` | high | `__func__` |
| `lv_obj_add_flag` / `lv_obj_clear_flag` / `lv_obj_has_flag` | `0x10128D38` / `0x10127E7C` / `0x101283DC` | high | `__func__` |
| `lv_obj_add_state` / `lv_obj_clear_state` / `lv_obj_has_state` | `0x10128244` / `0x10128310` / `0x10128610` | high | `__func__` |
| `lv_obj_invalidate`, `lv_obj_get_parent`, `lv_obj_get_child`, `lv_obj_get_child_cnt`, `lv_obj_get_screen` | `0x10129FD8`, `0x1012C19C`, `0x1012C3DC`, `0x1012C4AC`, `0x1012C210` | high | `__func__` |
| `lv_obj_invalidate_area(obj, area)` | `0x10129EF0` | high | `__func__`; `lv_obj_invalidate` ends in `bl 0x10129EF0` with `obj->coords` (+0x14, i32 x1 y1 x2 y2 inclusive, absolute) grown by the ext draw size; body as upstream: `lv_disp_is_invalidation_enabled 0x1012576C`, `lv_obj_area_is_visible 0x101C5F90` (clips to the object), `_lv_inv_area(disp, area) 0x1012D388`. Emulator (loader tick, agent-glow): break on the entry with the loader image (coords `0, 0, 465, 465`) gets the app's dirty box, `_lv_inv_area` is reached from `0x10129FA6`, and the frames keep updating (`runs/dirty/check.log`, not tracked) |
| `lv_obj_set_scrollbar_mode`; scroll getters | `0x1012AD38`; `0x1012AE08…0x1012B0B0` | high | `__func__` |
| `lv_obj_scroll_to*`, `lv_obj_set_scroll_dir`, `lv_obj_set_scroll_snap_*` | not identified (no asserts); `lv_obj_get_scroll_dir` is `0x101C64DC` (shape) | low | — |
| `lv_disp_get_scr_act` / `lv_disp_get_layer_top` / `_sys` | `0x101255E0` / `0x10125648` / `0x1012567C` | high | `__func__` |
| `lv_obj_set_flex_flow` / `lv_obj_set_flex_align` | `0x10133544` / `0x10133594` | high | callers of the flex style setters; `lv_obj_set_flex_grow` absent |

### Styles

| Item | Value | Confidence | How |
|---|---|---|---|
| `lv_obj_set_local_style_prop(obj, prop, value, selector)` | `0x101C6F9A` | high | calls `get_local_style 0x1012B7E8`, `lv_style_set_prop_internal 0x1013B0A4`, `lv_obj_refresh_style 0x1012B908`; 48 setters (44 + 4 flex) tail-call it, see "Verified for ABI 1.2" |
| `lv_obj_get_style_prop(obj, part, prop)` | `0x101C6E06` | high (emulator, "Verified for ABI 1.2") | dozens of `lv_obj_get_style_*` wrappers call it with constant ids |
| `lv_obj_set_style_*` present (44 + 4 flex) | `width 0x101C72CC`, `height`, `max_height`, `x`, `y`, `align 0x101C72FE`, `translate_x/y`, `pad_top/bottom/left/right/row/column`, `bg_color 0x101C7358`, `bg_opa 0x101C7378`, `bg_grad_color`, `bg_img_src`, `border_color`, `border_width`, `outline_width/color/opa`, `img_opa`, `img_recolor`, `img_recolor_opa`, `arc_width`, `arc_rounded`, `arc_color 0x101C7470`, `arc_opa`, `arc_img_src`, `text_color 0x101C74A4`, `text_opa`, `text_font 0x101C74CE`, `text_line_space`, `text_align`, `radius 0x101C74EC`, `opa`, `anim_speed`, `layout`, `base_dir`, Actions ids 45, 92, 93; flex `flow 0x10133534`, `main_place 0x10133564`, `cross_place 0x10133574`, `track_place 0x10133584`. Full list with entries: `lvgl_inventory.py` | high | tail calls with `movs r1, #prop` |
| Missing style setters | anything not in the list (e.g. `min_width`, `shadow_*`, `border_side`, `text_letter_space`, `transform_zoom/angle`, `clip_corner`) — call `lv_obj_set_local_style_prop` with the 8.3 id instead | high | — |
| `lv_style_init` / `lv_obj_add_style(obj, style, selector)` / `lv_obj_remove_style` | `0x1013A8E4` / `0x1012BB14` / `0x1012BD5C` | high | `__func__` |
| `lv_style_set_*` (shared styles) | `width 0x101CC6EA`, `height 0x101CC6F2`, `pad_top 0x101CC6FA`, `pad_bottom 0x101CC702`, `bg_color 0x101CC712`, `bg_opa 0x101CC724`, `img_recolor 0x101CC72C`, `img_recolor_opa 0x101CC73E`, `arc_width 0x101CC746`, `arc_rounded 0x101CC74E`, `arc_color 0x101CC756`, `arc_opa 0x101CC768`, `text_color 0x101CC770`, `text_opa 0x101CC782`, `text_font 0x101CC78A`, `text_align 0x101CC792`, `radius 0x101CC79A` | high | `lv_style_set_prop_internal(style, id, v)` with constant ids |

### Events

| Item | Value | Confidence | How |
|---|---|---|---|
| `lv_obj_add_event_cb(obj, cb, filter, user_data)` / `lv_obj_remove_event_cb` / `lv_event_send` | `0x10125A94` / `0x10125BD4` / `0x101259C4` | high | `__func__` |
| `lv_event_get_target`, `_current_target`, `_code`, `_param`, `_user_data` | `0x101C4C92`, `0x101C4C96`, `0x101C4C9A` (`code & 0x7F`), `0x101C4CA2`, `0x101C4CA6` | `_get_target` high (emulator, "Verified for ABI 1.2"), the others medium | shape (`ldr r0, [r0, #off]`) and upstream order |
| `lv_event_t` | `+0` target, `+4` current_target, `+8` code (u8, bit 7 = PREPROCESS), `+0xC` user_data, `+0x10` param | medium | getters |
| `lv_event_get_indev` / `lv_event_get_draw_part_dsc` | `0x10125D40` / `0x10125D74` | high | `__func__` |
| Event codes | upstream 8.3 numbering (`PRESSED 1`, `SHORT_CLICKED 4`, `CLICKED 7`, `RELEASED 8`, `SCROLL_BEGIN 9`, `SCROLL_END 10`, `SCROLL 11`, `VALUE_CHANGED 28`, `DELETE 33`); **`0x2D` (45, upstream `_LV_EVENT_LAST`) = crown event** added by Actions | high for 1, 4, 8, 9–11, 45; medium for the rest | callers (`jx_scroll_bar` uses 9/10/11), indev code, `docs/recon.md` |

### Timers and animations

| Item | Value | Confidence | How |
|---|---|---|---|
| `lv_timer_create(cb, period_ms, user_data)` / `lv_timer_del` | `0x1013B10C` / `0x1013B1A4` | high | `__func__`, callers |
| `lv_timer_set_cb`, `_pause`, `_resume`, `_set_period`, `_ready`, `_set_repeat_count` | `0x101CC7A2`, `0x101CC7A6`, `0x101CC7B0`, `0x101CC7BA`, `0x101CC7BE`, `0x101CC7D0` | medium | shape + upstream order |
| `lv_timer_t` | `+0` period, `+4` last_run, `+8` cb, `+0xC` user_data, `+0x10` repeat_count, `+0x14` bit 0 paused | high | `lv_timer_handler` decompile |
| `lv_timer_handler` | `0x1013B1D0` | high | `__func__` + emulator break |
| `lv_anim_init` / `lv_anim_start` / `lv_anim_del(var, exec_cb)` | `0x101391E4` / `0x1013927C` / `0x10139210` | high / init medium | `__func__`; init writes `time 500`, `end 100`, `repeat_cnt 1`, `path_cb` |
| `lv_anim_t` (0x44 B) | `+0` var, `+4` exec_cb, `+8` start_cb, `+0xC` ready_cb, `+0x10` deleted_cb, `+0x14` get_value_cb, `+0x18` user_data, `+0x1C` path_cb, `+0x20` start, `+0x24` current, `+0x28` end, `+0x2C` time, `+0x30` act_time, `+0x34…0x3C` playback/repeat delays, `+0x40` u16 repeat_cnt, `+0x42` flags. `lv_anim_set_*` are inline: the app writes the fields | medium | `lv_anim_init` + upstream layout |

### Input: crown and groups

| Item | Value | Confidence | How |
|---|---|---|---|
| Groups | `lv_group_add_obj 0x10126028` is called only from `lv_obj_class_init_obj` (default group); no encoder group drives the UI | medium | callers |
| Crown routing | the crown delta arrives in the pointer indev's data (`enc_diff`) and is handled by Actions code in the indev read (`0x101266xx`), not by `lv_group` | medium | decompile |
| Crown target | per display, the **knob object** `disp+0x268`, the last one registered. Registered by `lv_obj_add_event_cb(obj, cb, 0x2D, ud)` or `lv_obj_add_flag(obj, 1 << 30)` (`LV_OBJ_FLAG_USER_4`); both call `lv_disp_add_knob_obj 0x10128C50`; removed by `lv_disp_remove_knob_obj 0x10127D14` (also on `lv_obj_clear_flag` and delete) | high | decompile of all three |
| Crown event | without the flag: `lv_event_send(knob_obj, 0x2D, &param)`, `param` an `int32_t *` = `enc_diff * 30`, and `param = 0` once the crown has been still for 300 ms. With flag `1 << 30`: the crown scrolls the object (direction from `lv_obj_get_scroll_dir 0x101C64DC`); two classes (checked at `0x101C54B6`, not identified, likely roller-like) take a third path that was not read | medium | indev decompile |
| Keys | `0x2D` covers rotation only; crown press and buttons arrive as view key messages (`docs/recon.md`, view proc msg `0xD`) | medium | — |

## Fonts and theme (question 3)

| Item | Value | Confidence | How |
|---|---|---|---|
| Built-in `lv_font_t` | only 4, all `fmt_txt`: `0x101FB6BC` = `LV_FONT_DEFAULT` (1 bpp ASCII 0x20–0x7F, line height 9; used by `lv_style` defaults and `lv_draw_label`); `0x101EC220` (lh 44), `0x101EC2E0` (lh 29, digits and `%`), `0x101ECD48` (lh 41) used only by one stock view (`0x1006AC50…`). No Montserrat | high / names low | `lv_font_get_bitmap_fmt_txt` pointer scan |
| Stock text fonts | Actions bitmap fonts from NAND: `/NAND:A/font{18M,24L,24M,24R,28M,28R,30R,32M,32R,34R,36L,36M,36N,36R,44L,48M}.font`, `num*.font`, `nm*.font`, `/NAND:A/sans34.font`, `/NAND:C/sans32.font`, `/NAND:C/emoji28.font` | high | strings + users |
| Open | `lvgl_bitmap_font_open(lv_font_t *font, const char *path) 0x10185C30` → 0 / -1: fills `get_glyph_dsc`, `get_glyph_bitmap`, `line_height`, `base_line`, puts its context in `font->user_data` (`+0x1C`). Stock wrapper `0x100DF9A0(font, path)` additionally tunes a few fonts (emoji fallback, line height) | high | decompile |
| Close | `lvgl_bitmap_font_close(lv_font_t *font) 0x10185D3C` | high | decompile |
| Sharing | the backing font is a refcounted global table (`0x10182DCC`, entries 0xB8 B, keyed by path): opening a path that a stock view already has open costs only the 0xC-byte context (`ui_mem` small alloc `0x10146A3C`) | medium | decompile |
| Stock font pointers | none global: every view embeds its `lv_font_t` (0x20 B) in its own data, opens them on preload and closes them on unload (e.g. `0x100873E4` opens 4 fonts into `data+0x7C…+0xFC`). An app cannot borrow a stock font pointer; it opens the same file itself | high | decompile |
| Glyph cache memory | not traced (which heap holds glyph bitmaps) | — | open |
| Theme | **none**: `lv_disp_drv_register 0x10138B38` does not call `lv_theme_default_init`; at runtime `disp->theme (+8) = NULL` on every run | high | decompile + emulator |
| Look of stock screens | black background, dark grey cards (`#1A1C1F` in the emulator frame), light grey/white text, red accent; icons are images from resources. In emulator frames full white reads `#A0A0A0`, so absolute values are scaled | low | frame sampling |

With no theme, a freshly created widget has only the `lv_obj` defaults: transparent background, the 9-px
default font, no button/slider look. Every app style must be set locally or through `lv_style_t`.

## UI thread (question 4)

| Item | Value | Confidence | How |
|---|---|---|---|
| Thread | **`ui_service`**: static thread table entry `0x10000464` (`{name, stack, u16 size, u8 prio, u8 attr, …, entry}`): stack `0x2FFB6740`, 0x4000 B, **priority 3**, entry `0x1018B234` (message loop); `k_thread` at the stack base `0x2FFB6740` | high | table + UART `Thread 0x2ffb6740 ui_service run time` |
| `lv_timer_handler` caller | GUI ops table `0x101FE4A0` `[7]` = `0x1018F8B0`: reads every indev without a read timer, then tail-calls `lv_timer_handler`; driven by the `ui_service` loop | high | disassembly + emulator: break on `0x1013B1D0` lands with `sp = 0x2FFBA6F0` inside the `ui_service` stack |
| Lock for other threads | **none**. The GUI heap only warns when a thread other than the first allocator (`ui_service`) uses it. Stock code from other threads posts messages to `ui_service` (view messages, `docs/recon.md`); view `proc` callbacks, including the stub's view `0xF0`, run on `ui_service` | high | disassembly + emulator |
| Other names | `lvglworkq`, `displayworkq` exist as work queues (display flush / GPU), not as the LVGL caller | medium | strings |

## GUI heap free space (question 5)

Emulator, stock 1.0.0.73, default face, then the e2e heavy face `w23jrco8sqew3nv` (Concentric Hal). The heap
is 0x18000 = 98,304 B. "Free" = sum of free chunks minus 4-byte headers; "max block" = largest free chunk.
Rows below are the steps whose frame confirms the screen.

| Screen | Default face: free / max block / used | Heavy face: free / max block / used |
|---|---|---|
| Watch face | 67,456 / 67,420 / 30,824 | 59,052 / 58,508 / 39,224 (worst) |
| Widget cards (face swipe left), control centre, notifications (lowest of the three) | 67,452 / 67,380 / 30,824 | 60,448 / 60,388 / 37,824 |
| App list | 83,296 / 79,148 / 14,856 | same |
| Exercise (outdoor/indoor run) | 78,148 / 76,252 / 20,120 | same |
| Exercise Records | 96,064 / 96,036 / 2,216 | same |
| Other walk steps (scrolled list, Activity, face picker; frames do not pin the screen) | 81,600–88,332 free, 9.9–16.6 KB used | same |

| Item | Value | Confidence |
|---|---|---|
| Minimum free seen | **59,052 B** (heavy face), 67,452 B (default face) | high (emulator), only the walked screens |
| Smallest largest block | **58,508 B** | high (emulator) |
| Water mark | not available: the heap has no runtime stats (`z_heap` header ends after `avail_buckets`) | high |
| Where it goes | the watch face is the biggest LVGL user (30–39 KB); stock list screens use 2–20 KB because images and text come from the resource heap (`RES`/`FB`) | medium |

## Vendor components (question 6)

There is no generic list-row, toggle-row or confirm-dialog wrapper: no such class exists and the settings
screens build their rows themselves. The reusable pieces are widgets in `application/bt_watch/src/{,jx_src/}ui/widgets/`:

| Component | Entry points | Confidence |
|---|---|---|
| `text_canvas` (label rendered to an image, emoji, `…` ellipsis), class `0x101EADE4`, 120 B | `create(parent) 0x1005B578`, `set_text(obj, txt) 0x1005B63C` (copies into `lv_mem`), `set_text_static 0x1005B5C8`, `set_text_fmt 0x1005B6AC` | medium |
| `text_roller`, class `0x101EAE24`, 220 B | `create 0x1005BEF4`, `set_str(obj, strings, stride, count ≥ 3) 0x1005BF78`, `set_visible_row_count` (string `text_roller_set_visible_row_count`) | medium |
| `simple_img` (rotate/zoom image), class `0x101EADC4`, 84 B; `jx_pic_anim` (frame animation) `0x101F369C` | `simple_img_create 0x1005A014`, `simple_img_set_src/pivot/angle/zoom` (names via `__func__` at `0x1005A048…0x1005A2EC`); `jx_pic_anim_create 0x100D8D30`, `start/stop/set_pic_idx` | medium |
| `jx_scroll_bar` (round scroll indicator, base `lv_arc`), class `0x101F36BC` | `create 0x100D93CC`, `set_scroll_obj(bar, obj) 0x100D93E4` (listens to `SCROLL_BEGIN/END/SCROLL` on obj), `set_knob(bar, on) 0x100D94EC` (adds/clears flag `1 << 30` on obj, i.e. crown scrolling). This is how stock lists get crown scrolling. Stock call (app list `0x1005EFA6`, same in ~40 lists): `bar = create(screen)` after the list, `lv_obj_set_size(bar, 462, 462)`, `set_scroll_obj(bar, list)`, `set_knob(bar, 1)`, `lv_obj_align(bar, CENTER 9, 0, 0)` (`0x101C5DE4`). The constructor hides the bar and starts a 1000 ms timer that hides it again (`0x101B6291`); scrolling shows it. While the knob is on, scrolling sets ux2sys property `0x56` (byte `0x38053248`, read in `0x1014B244`, a key handler, purpose not traced); the destructor clears it and deletes the timer, it does not touch the list's callbacks. Emulator: shown on a crown step and hidden ~1 s later in the stock app list, Settings and the loader menu (`runs/scrollbar/`) | high |
| `snapshot_cont`, `simple_video`, `jx_widget_calc` | classes `0x101F36FC`, `0x101F36DC`, `0x101EAD84`; creates `0x100DA2CC`, `0x100D9A00`, `0x10059668` | medium |
| Popups / confirm | views opened by id through `msgbox_cache_popup(id, udata) 0x100D5E34` (`jx_msgbox_creater.c`), see `docs/recon.md`; not a widget API | medium |

## What this means for ABI 2

Shipped as the compatible extension ABI 1.2 (`docs/abi-1.2.md`, `docs/specs/2026-11-11-lvgl-apps-design.md`): the app
keeps its thread and sends command buffers; the loader runs them on `ui_service`. The bullets below are the recon-time
analysis; where they differ from 1.2 (50 setters, `lv_arc_set_range`, "needs event-driven apps"), 1.2 and "Verified
for ABI 1.2" win.

- **Widgets available as stock code:** `lv_obj` (with flex and grid layout, scrolling, styles), `lv_label`,
  `lv_btn`, `lv_img`, `lv_arc`, `lv_bar`, `lv_slider`, `lv_roller`, `lv_canvas`, `lv_btnmatrix`, `lv_chart`,
  `lv_imgbtn`, `lv_tileview`, `lv_qrcode`, plus vendor `text_canvas`, `text_roller`, `simple_img`,
  `jx_scroll_bar`. No switch, checkbox, list, msgbox, dropdown, textarea, keyboard, spinner: ABI 2 either
  composes them from `lv_obj`/`lv_arc`/`lv_bar` + styles or ships them inside the app.
- **Layout facts the SDK must hard-code:** `lv_coord_t` = i32, `lv_color_t` = RGB565 u16, `sizeof(lv_obj_t)` =
  44, `lv_obj_class_t` = 0x20 B, `lv_anim_t` = 0x44 B, `lv_event_t` field offsets above, style ids of 8.3.10
  plus the Actions extras. `lv_mem_alloc` takes a second `tag` argument.
- **No theme:** every app sets its own styles; a shared `lv_style_t` set in the SDK replaces the theme.
  Fonts are opened by path with `lvgl_bitmap_font_open`/`_close` and closed when the app exits.
- **Memory budget:** the GUI heap has ≥ 58 KB contiguous free on the worst screen seen, but the app runs
  while the previous stock view may still hold its objects. A safe budget for an app's LVGL objects is
  about **32–40 KB**; images and canvases belong in the app's own block (or `RES`), not in `lv_mem`.
  `sys_heap` returns NULL instead of rebooting, but most LVGL paths assert on NULL and then reset.
- **Thread:** all LVGL calls must run on `ui_service` (priority 3): from the stub's view `proc`, from
  LVGL event callbacks or from `lv_timer` callbacks. The ABI 1 model (app in its own Zephyr thread) cannot
  call LVGL directly; it needs an event-driven app (callbacks on `ui_service`) or a message queue into the view.
- **Crown:** register the app's scroll container with `lv_obj_add_flag(obj, 1 << 30)` for crown scrolling, or
  listen to event `0x2D` for raw rotation; one knob object per display, the last registered wins, so the app
  must unregister (delete or clear the flag) on exit.
- **Main risks:** an LVGL assert or a GUI-heap exhaustion resets the watch; a crash inside an app callback
  takes `ui_service` down with it; leaked objects or fonts after app exit shrink the heap for stock screens;
  the knob object and default group are global; small functions were identified by shape (medium), so ABI 2
  should verify each imported entry in the emulator before use.

## Verified for ABI 1.2

Every entry the loader's LVGL executor calls, run on stock 1.0.0.73 in the emulator by
`scripts/recon/lv_entries.py` (addresses read from `fw/wp2-core/src/stock.rs`, Thumb bit set there). Calls run on
`ui_service` (`emu.call(..., thread=b"ui_s")`). Each widget is created under a root on the active screen, then
configured and read back: the class pointer at `+0`, the size from `coords` (`+0x14..+0x20`) after a layout pass,
values through the getters and the struct fields. At the end the font is opened and closed, everything is deleted,
and the guest is checked for a reboot. Output of the run (`runs/lv/entries.log`, not tracked): every line `ok`,
one boot banner.

| Entry | Address | Verified by | Confidence |
|---|---|---|---|
| `lv_btn_create` | `0x1013D388` | class `0x101FB730`, 120×60 coords | high |
| `lv_arc_create` | `0x1013BA54` | class `0x101FB6F0`, 200×200 coords | high |
| `lv_arc_set_rotation` | `0x1013C054` | `rotation` (u16 `+0x2C`) = 135 | high |
| `lv_arc_set_bg_angles(obj, start, end)` | `0x101CCC82` | new: small-function area right after `lv_arc_set_angles` `0x101CCC6A`; body `bl set_bg_end_angle(obj, end)`, `b.w set_bg_start_angle(obj, start)` (upstream 8.3); 14 stock callers, each after `set_rotation`. Emulator: `bg_start/bg_end` (u16 `+0x32/+0x34`) = 10, 270 | high |
| `lv_arc_set_value` / `lv_arc_get_value` | `0x1013C108` / `0x1013C2EC` | `value` (i16 `+0x36`) and getter = 40; 150 clamps to 100 | high |
| `lv_bar_create` | `0x1013CEA0` | class `0x101FB710`, 200×20 coords | high |
| `lv_bar_set_range` | `0x1013D294` | `min/max` (i32 `+0x30/+0x34`) = -50, 150 | high |
| `lv_bar_set_value` / `lv_bar_get_value` | `0x1013CEB8` / `0x1013D078` | `cur_value` (i32 `+0x2C`) and getter = 120; 500 clamps to 150 | high |
| `lv_slider_create` | `0x10141EB8` | class `0x101FB810`, coords; `lv_bar_set/get_value` work on it (30) | high |
| `lv_roller_create` | `0x10140E00` | class `0x101FB7D0`, 150×120 coords | high |
| `lv_roller_set_options(obj, opts, mode)` | `0x10140E18` | 3 options (`"one\ntwo\nthree"`, mode 0): selecting 5 clamps to 2 (the 5 default options would give 4) | high |
| `lv_roller_set_selected` / `lv_roller_get_selected` | `0x10141044` / `0x10141690` | select 1, read 1 | high |
| `lv_obj_set_flex_flow` | `0x10133544` | `lv_obj_get_style_prop(obj, 0, 113)` = 1 (COLUMN), `LAYOUT` (10) = 1 (`LV_LAYOUT_FLEX`); two 50×30 children stack (second `y1` > first `y2`) | high |
| `lv_obj_set_flex_align(obj, main, cross, track)` | `0x10133594` | style props 114, 115, 116 = 2, 1, 3 | high |
| `lv_event_get_target` | `0x101C4C92` | `ldr r0, [r0]`; returns `+0` of a hand-made `lv_event_t` | high |
| `jx_scroll_bar_create` | `0x100D93CC` | class `0x101F36BC`, 462×462 coords | high |
| `jx_scroll_bar_set_scroll_obj` / `_set_knob` | `0x100D93E4` / `0x100D94EC` | after `set_scroll_obj(bar, obj)`, `set_knob(bar, 1)` sets flag `1 << 30` on obj and `set_knob(bar, 0)` clears it (`lv_obj_has_flag 0x101283DC`) | high |
| `lvgl_bitmap_font_open` / `_close` | `0x10185C30` / `0x10185D3C` | `/NAND:A/font32M.font`: returns 0, `line_height` 33, context in `user_data` (`+0x1C`); close frees it and zeroes `+0x1C` | high |
| `lv_obj_get_style_prop(obj, part, prop)` | `0x101C6E06` | returns the local values set above (used as the checker) | high |

Facts for the executor:

- **`lv_arc_set_range` is not in the image.** The image has no `lv_arc_set_range` `__func__` string, though every
  asserting arc function in the image has one (only `set_start/end_angle`, `set_bg_start/end_angle`, `set_rotation`,
  `set_mode`, `set_value`, `get_value`; `set_change_rate`, `get_min/max_value` and `get_angle_*` are gone too, dropped by
  `--gc-sections`). The static `value_update` (`0x1013BC9C`) has only three callers: `set_bg_start_angle`,
  `set_bg_end_angle` and `set_value`. No arc code stores the `min/max` pair (`+0x38/+0x3A`) outside the constructor,
  which writes `0x00640000` at `+0x38`. **Arcs keep the default range 0..100.**
- **Arc value, min and max are i16** (`ldrsh` at `+0x36/+0x38/+0x3A`); `set_value(obj, i16)` returns early when the
  value is unchanged, then clamps. Bar (and slider) values are i32.
- **Flex style ids are registered at boot**, not fixed: `lv_flex_init` registers `LV_LAYOUT_FLEX` = 1 (u16 at
  `0x38047FEA`) and the props flow 113 (`0x38047FEE`), main_place 114 (`0x38047FF2`), cross_place 115 (`0x38047FEC`),
  track_place 116 (`0x38047FF4`), in upstream order. The setters load them
  (`ldr r1, =var; ldrh r1, [r1]`); the values above are read in the emulator after boot.
- **48 style setters, not 50:** 44 tail-call `lv_obj_set_local_style_prop` with `movs r1, #id`
  (`0x101C72CC…0x101C751A`), plus the 4 flex setters, and nothing else branches to it.
  `wp2_core::stock::STYLE_PROPS` holds these 48 ids.
- **`lv_disp_get_scr_act` takes the display** (`NULL` = default).
- Emulator limit: a visible object over the default face makes LVGL redraw the face's rotated images through `sw_rotate`
  (`sw_transform_config`, LR `0x1014381F`), which ends in a compatibility-ROM API QEMU lacks ("unsupported API
  0x524f4d"). The probe hides its root; e2e runs that draw LVGL objects need a screen without rotated images,
  or a face without them.

## Still unknown

- Exact 8.3 patch level; whether `LV_COLOR_16_SWAP` is set (the flush path was not read).
- Entries for `lv_obj_scroll_to*`, `lv_obj_set_scroll_dir/snap`, `lv_img_set_zoom/angle`, `lv_group_*` beyond
  add/remove (none have asserts).
- Where glyph bitmaps of NAND fonts are cached and how much memory a font costs.
- Low-water mark of the GUI heap during transitions (no runtime stats; only screen snapshots were taken), and
  screens the walk did not reach (calls, notifications with content, workouts in progress).
- The exact semantics of the Actions style ids 45, 92 and 93.
