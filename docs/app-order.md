# App order in the Apps menu (contract for the phone)

The watch's Apps menu ("Custom Apps") lists the apps the phone names first, in the phone's order, then every other
installed app in slot order. The order travels inside the **loader package**: the phone writes its list into a table
in the loader image and installs that loader with the usual `BEGIN`/`DATA`/`END` to slot `0xFF`. No new command, no
stub change: works on every stub that installs loaders (stub 1 and later), from the loader of 2026-11-19 on.

## Why the loader package

- `MSG` (`0x90A5`) only reaches a running ABI 1.1 app: with no app running the stub answers `NoApp` (7) and keeps
  nothing (`fw/wp2-stub/src/ble.rs`, `msg`). The order has to arrive while the menu is closed, so `MSG` is out.
- The phone cannot write any other file: `END` keeps a transfer only if it is a valid package for its slot (header,
  kind, size, CRCs), and the loader's own data file is written only on the watch.
- A loader package is a valid package the phone may change and re-sign: the CRCs are plain CRC-32. On NAND it
  survives reboots and is read every time the menu is built.

Cost: one loader install (about 54 KiB, the same transfer as a loader update) per order change.

## The table

In the loader image (the bytes after the 64-byte package header) there is exactly one occurrence of the 8 ASCII
bytes `WP2ORDER` (`57 50 32 4F 52 44 45 52`). The 384 bytes after it are the table: 16 fields of 24 bytes.

- Field `i` is the package name of the app for row `i`, exactly as in that app's package header at offset `0x08`
  (UTF-8, 1..23 bytes, padded with `00` to 24 bytes). Byte-exact compare, case-sensitive. The phone gets the names
  from `INFO` (header bytes `0x08..0x20`) or from the packages it installed.
- The first field that starts with `00` ends the list; fill all fields after it with `00`. At most 16 names: one per
  slot on stub 8 (16 slots), 8 slots before it.
- Names that are not installed are skipped. Installed apps not in the list follow, in slot order. A name listed
  twice counts at its first place. All fields zero = slot order (the shipped loader).
- The table holds no relocations: `wp2apps order` refuses a loader where it would.

## What the phone does

1. Take the loader package it ships (`loader.wp2app`, the same file it installs on a loader update).
2. Find `WP2ORDER` in the image (search `package[64:]`; it must occur once; none = a loader older than this feature,
   install a newer one). Let `t = 64 + offset + 8`.
3. Write the 384-byte table at `package[t : t + 384]`.
4. Re-sign the header (little-endian, CRC-32 = zlib/IEEE, `crc32` in any standard library):
   - `body_crc` at header `0x30` = `crc32(package[64:])` (image and relocation table);
   - `header_crc` at `0x34` = `crc32(package[0:0x34])`, computed after writing `body_crc`.
   Name, version, sizes, entry and relocations stay as they are.
5. Install it to slot `0xFF` as any loader: `BEGIN` (`0x90A1`, `FF ‖ size:u32le ‖ crc32(whole package):u32le`),
   `DATA` (`0x90A2`, `offset:u32le ‖ ≤128 bytes`), `END` (`0x90A3`, `00`). Statuses as in `docs/abi-1.1.md`.

When:

- After the user changes the order, and every time the phone installs a loader (an update would otherwise reset the
  order to slot order): the phone patches every loader it sends with its current list.
- `BEGIN` answers `Busy` (5) while the menu, or an app started from it, is on screen. The menu holds the loader
  while its view lives, so retry later (a minute, or when the phone comes to the foreground); nothing is lost. An app page behind the face does not block it
  (stub 6, `docs/app-guide.md`, section 2 "Cards").
- The menu shows the new order the next time it is built: when it opens, or after a delete on the watch.

Reference implementation: `wp2apps order target/loader.wp2app NAME... --output loader-ordered.wp2app`
(`package.set_order`, `src/wp2apps/package.py`), then `wp2apps install loader-ordered.wp2app`. Watch side:
`fw/wp2-loader/src/menu.rs` (`WP2_ORDER`, `rank`, `scan`). Test: `tests/test_app_order_e2e.py`.

## Example

Order `vibration-lab`, `hello`: the table is

```text
76 69 62 72 61 74 69 6F 6E 2D 6C 61 62 00 00 00 00 00 00 00 00 00 00 00   "vibration-lab"
68 65 6C 6C 6F 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00   "hello"
00 × 336                                                                   end of list
```
