# Host channel 1.4: own channel key K and unsolicited app messages

Stub and loader changes for hosts that cannot bind or poll: a phone app next to Nothing X on the system's shared
BLE link. Proof in the emulator:
`tests/test_chan_key_e2e.py`. Not yet on hardware. Apps see no change; `Api` is unchanged.

## Channel key K

- `K` = the pairing secret's 16 bytes: the 32 hex chars `AT GETSECRET` returns, which the stub reads from NVRAM
  `APP_SECRET` (`docs/recon.md`, "Project 2"). Host: `link.channel_key`, `Session.open_channel`,
  `Channel(channel_key=True)`.
- Frames `0x90A0..0x90A7` (install, `MSG`, `PING`) may be sealed with `K` instead of the stock session key, in the
  same format (`plain ‖ crc32le`, PKCS7, AES-128-CBC, IV `5051…665A`). The stub tries the stock key first, then `K`.
  Replies and `EVENT`s go out under the key of the last frame.
- No bind: no "Paired" popup, the stock session (Nothing X) is untouched, `K` survives reboots.
- App messages to a `K` host need a stock session word that is not zero (a bind since the last factory reset): the
  loader checks `session_word() == host_key`. Any watch paired with Nothing X has one.
- Security: as before. Any BLE client reads the secret; the owner accepted that (`docs/recon.md`, threat model).

## Shared link

Every host on the link sees every notification. A host skips frames and `EVENT`s its key cannot open
(`link.Session.request`, `channel.recv`). Switching between a stock host and a `K` host drops the app messages
queued for the other one (as a new host key always did).

## Unsolicited app messages

- At boot the stub stores `StubRam.kick` (its `wp2_kick`) and `kick_magic = "KICK"`. The loader calls `kick` on the
  app thread right after the app's `send` queued a message, if the magic is there (an older stub leaves it unset;
  `api` stays 1 because loader 1.3 needs `api == 1`).
- With a `K` host listening, `wp2_kick` sends all queued messages at once (software AES, the stock crypto buffers are
  not used). The stock itself writes the stream from threads other than `wewear_read` (the bind popup's `0x0049`
  reply goes out from the UI thread), and the stream write has its own lock.
- `StubRam.busy` serializes the stub's frame path: `wewear_read` waits for it (`os_sleep(1)`), the kick skips when it
  is taken (`wewear_read` drains after its reply anyway).
- A stock host still polls with `PING`. A `K` host may poll too, but does not have to: 40 messages in a burst
  arrived without a host frame, none lost, `seq` contiguous (emulator).

## Install order

New stub (OTA, `docs/ota.md`) and new loader (`wp2apps install target/loader.wp2app`). Each works with the other's
old version: an old loader never calls `kick` (messages wait for the host's next frame), an old stub has no magic.

## Cards (loader of 2026-11-13, no ABI bump)

`FOCUS` with `icon = 0xFFFF` (`lv::FOCUS_CARDS`) gives the stock Exercise list instead of the app list: every
child is a card (`stock::focus_card`) that narrows with its distance from the centre line, 434 px in the centre and
`434 − 2·shift` further out (`lvcmd::card_width`, 370 px one row away, as in the stock list); text fades as before.
The container lays the cards out at a fixed x (`stock::focus_page` sets `PadLeft` 16 and a start-aligned flex) and
the loader centres each one with `TRANSLATE_X = shift`: a width set from `LAYOUT_CHANGED` is not laid out again, so
a flex centre lags one scroll behind. The loader owns the cards' `Width`, `TranslateX` and `TextOpa`. `ABI_MINOR`
stays 3: the stub on the watch refuses packages above its `LOADER_ABI_MINOR` (3), and raising it takes an OTA. An
older loader answers `-22` to this FOCUS, so install the loader before an app that uses it.

## Heart rate (loader of 2026-11-14, no ABI bump)

Live heart rate for an app, as the stock Strength training shows it under the clock. Stock side:
`docs/recon.md`, "Heart rate". Emulator proof: `tests/test_hr_e2e.py` (app `sdk/examples/hr-probe`, a canvas app).
Not yet on hardware, and the emulator has no PPG model: the test writes a synthetic reading into the stock cell.

### Why not a new `Api` field

The stub on the watch refuses packages above its `LOADER_ABI_MINOR` (3), and raising it takes an OTA. Options that
keep `abi_minor` 3:

1. A record on `lv_submit` (taken). The table has the field since 1.2, an older loader answers `-22` to an unknown
   op, as with `FOCUS_CARDS`.
2. Extra values on `screen_on(bool)` or `own_back(bool)`: a `bool` above 1 is undefined in Rust and mixes meanings.
3. A package flag ("uses heart rate") that the loader reads at launch: no off switch while the app runs, and an
   older loader ignores it silently.
4. The host channel (`send`): goes to the phone, not to the loader.

### The call

SDK: `Api::heart_rate(&self, on: bool) -> i32`, event kind `HEART` = 9.

- `on = true`: the loader starts the stock measurement (ux2sys property 2 = 1, as the stock heart view does; the
  stock reading is cleared first) and queues `Event { kind: HEART, x: worn, delta: bpm }` once per 1000 ms
  (`wp2_core::heart::EMIT_MS`) while the app runs in the foreground. `delta` 0 = no reading yet, or off the wrist
  (`x` 0). Queued HEART events coalesce: the app sees the latest.
- `on = false` stops it. Returns 0, or `-22` on an older loader (or `abi_minor < 2`): no heart rate there.
- The loader stops the measurement on exit, crash (`exit(-1)` from the panic handler), force stop, view delete,
  and `PAUSE`; it starts it again on `RESUME`, with the reading cleared. It keeps measuring while the panel is dark
  (AOD included): the readings keep coming, so a workout has heart rate through its rests; the watch stays out of deep
  sleep meanwhile. A stop while
  the app runs queues one HEART with `delta` 0: no reading while it is off. A new launch starts with it off.
- The panel keeps its own auto-sleep: the loader does not take the heart view's 25 s hold (`docs/recon.md`, "Heart
  view: screen"). A reading needs the panel lit for a few seconds; an app that wants one holds the screen itself
  (`screen_on`). The app's request is a wish
  (`fw/wp2-loader/src/hr.rs`); the UI tick does the stock calls, the rule is `wp2_core::heart::Heart::tick`.
- Exactly one start per hold, one stop at its end: the stock counts requests (`docs/recon.md`), so a repeated
  start would keep the sensor on after the app left. The background measurement uses the same count.
- A stock owner already measuring (owner byte `0x3804B30F` non-zero, e.g. a stock workout left running): the loader
  only reads and never starts or stops it.

### Wire format

A batch holding one SENSOR record, `[0x13, 2, 0, 0, sensor, on]` (sensor 1 = heart rate, on 0 or 1;
`wp2_core::lvcmd::sensor`). The loader takes it in `lv::submit` on the app thread, before LVGL: no root is made,
so a canvas app may send it. In a longer batch, and on an older loader, op `0x13` is `Malformed` (`-22`). The SDK
keeps the two records as statics (`cmf_wp2_app::HEART_ON`, `HEART_OFF`): `lv_submit` takes buffers in the app
block only.

### Privacy and battery

Heart-rate values are personal data: apps and tests never log them; the e2e test uses synthetic values. The PPG
LED runs while an app holds the measurement: ask for it only while the value is shown or recorded.

### Screen-off

The measurement keeps the watch out of deep sleep, so with the panel dark the stock "return to watch face" (30 s,
2 min or 30 min, ux2sys `0x9C`) is reached and late-resume would close the app view. The loader of 2026-11-14 holds
that return off while an app runs, as the stock does for its own workout views (`docs/recon.md`, "Return to watch
face"). No ABI change. Since the stop on a dark panel the measurement no longer holds the watch out of deep sleep;
the return stays held anyway.

## Pager (loader of 2026-11-14, no ABI bump)

A vertical pager of full-screen pages, the stock `lv_tileview` (entries: `docs/recon-lvgl.md`, `lv_tileview`).
SDK: `stock::pager` (`docs/app-guide.md`, "Pages"). Emulator proof: `tests/test_pager_e2e.py` (ui-gallery's
"Pages"), frames `runs/pager/`.

- `CREATE` class **8 Pager**: `lv_tileview_create(parent)`. Children: class **9 Page** only, and a page only in a
  pager. A page is `lv_tileview_add_tile(pager, 0, row, LV_DIR_TOP | LV_DIR_BOTTOM)` with `row` = the pager's child
  count, so pages stack in creation order. `DEL` of a page is `BadClass` (the rows after it would leave a gap); it
  goes with its pager.
- `VALUE` on a pager: `lv_obj_set_tile_id(pager, 0, v, anim)` (`v` 0..=192, past the last page: the last).
- At `CREATE` the loader registers the crown filter (`0x2D`) on the pager, which makes it the knob object, and its
  own `VALUE_CHANGED` callback. `KNOB(pager)` takes the crown back. A crown event scrolls the current page by its
  step (`lv_obj_scroll_to_y`, no animation) while it has room in that direction (`lv_obj_get_scroll_bottom`,
  `lv_obj_get_scroll_y`); at the end it glides to the next or previous page. A drag does the same through LVGL's
  scroll chain (the tile is scrollable, the tileview snaps with `SCROLL_ONE`).
- Event: when the tileview settles (`VALUE_CHANGED`, sent by its own `SCROLL_END` handler) the app gets
  `Event { kind: LV, x: pager, y: 28, delta: index of lv_tileview_get_tile_act }`, coalesced, with no `EVENTS`
  needed. `EVENTS` `VALUE_CHANGED` on a pager adds nothing.
- The tileview scrolls vertically only, so the back swipe (`0x71`, a right swipe) reaches the loader as before.
- `ABI_MINOR` stays 3 (an OTA would be needed otherwise). An older loader answers `-22` to class 8 and 9: install the
  loader first. The golden fixture is unchanged; `lvcmd::tests::pager_holds_pages` pins the rules.

## Stub version (stub 2, no ABI bump)

The watch reports `1.0.0.73` (`AT GETVERSION`, `0x8006`) whatever `app.bin` it runs, and the OTA `version_code`
can't be read back. So the stub answers its own release:

- Request: `INFO` (`0x90A0`) with slot `STUB_SLOT` = `0xFE`, under the stock session or `K` like every stub frame.
- Reply (`0xB0A0`): `status` (0) ‖ `STUB_VERSION` (u32 LE) ‖ `LOADER_ABI_MINOR` (u8), 6 bytes. `STUB_VERSION` lives
  in `fw/wp2-core/src/layout.rs`; bump it with every stub change that goes out by OTA. It is not the stub header's
  `version` (the header format, which `wp2apps firmware` requires to be 1).
- An older stub parses `0xFE` as a bad slot and answers `BadRequest` (1): version 1. Stock firmware has no stub and
  doesn't answer at all. One request tells the three apart: no reply, `01`, `00 …`.
- Host: `install.stub` (`wp2apps list` prints it as `stub`).
