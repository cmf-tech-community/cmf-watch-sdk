# App ABI 1.3: the app owns "back"

Status: implemented, 2026-11-13. JX402 `1.0.0.73` only. Emulator proof: `tests/test_back_e2e.py` (`OwnBackTest`, app
`sdk/examples/back-owner`). Not yet run on a real watch: installing the 1.3 firmware or loader needs the owner's yes
(`AGENTS.md`, hard rule 1).

## Why

Up to 1.2 the loader force-stops an app 1000 ms after `BUTTON` (crown press or right swipe). An app cannot use the
button as "back" inside itself, nor ask "leave?" before it exits.

## The call

`own_back(on: bool)`, `Api` field after `lv_submit`; SDK `Api::own_back(&self, on)`.

- `on = true`: `BUTTON` is an ordinary event. The app leaves by returning from `main` (or `exit`).
- The escape stays for a hung app: the loader still force-stops the app if a `BUTTON` is not read (`event()` did not
  return it) within `ESCAPE_MS` = 1000 ms. Each press the app has read opens a fresh window for the next one.
- Reset to `false` at every launch. A paused app (popup) gets no `BUTTON`, as before.
- Rule in one place: `wp2_core::events::escape_due(owned, button_at, consumed, now)`; the loader
  (`fw/wp2-loader/src/main.rs`, `tick`) and the simulator (`sim/src/lib.rs`, `pump`) both call it.

## Compatibility

- `Api.abi` stays 1, `Api.abi_minor` becomes 3; the 1.0–1.2 fields keep their offsets.
- `LOADER_ABI_MINOR = 3` (`fw/wp2-core/src/layout.rs`): rebuild the firmware (`scripts/build-fw`). Apps built with the
  1.3 SDK carry `abi_minor` 3 and need a 1.3 loader, as with 1.2.
- `Api::own_back` is a no-op when `abi_minor < 3`, so the SDK never reads the field on an older table.
