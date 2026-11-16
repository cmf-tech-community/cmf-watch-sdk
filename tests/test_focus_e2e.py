"""The stock focus list (`stock::focus_page`, loader FOCUS) behaves like the stock app list, in ui-gallery's menu:
every letter circle sits where the stock fisheye puts the stock icon (`docs/recon.md`, `0x1005ED98`: x by the
distance from the centre line, zoom around the icon's centre), the centred row gets no pill, the crown moves the
list by the stock step and FOCUSED follows the row nearest the centre line, and a drag rests where it was released
(no snap, as in the stock list). Frames: runs/focus/. Needs target/{loader,ui-gallery}.wp2app.
WP2_NETSIM_INSTANCE=89."""
import asyncio
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_api_e2e import session
from tests.test_demo_e2e import save_png
from wp2apps import paths

S = harness.STUB_RAM
ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/ui-gallery"
OUT = paths.ROOT / "runs" / "focus"
FOCUS = 9  # STATS index, ui-gallery main.rs
CY = 233
LETTER = (51, 53, 56)  # stock::LETTER_BG as the emulator's frame shows it


def near(px, colour, tol=12):
    return all(abs(a - b) <= tol for a, b in zip(px, colour))


def stock_icon_cx(d):
    """Centre x of a stock app list icon `d` px from the centre line (the float fisheye, docs/recon.md)."""
    d = abs(d)
    x = d / 3.25 + 16 if d < 104 else (min(d, 280) - 104) / 1.92 + 48
    return x + 48


def circles(w, h, px):
    """Bounding boxes of the letter circles (left half of the screen), top to bottom: (cx, cy, side)."""
    bg = LETTER
    rows = {}
    for y in range(h):
        # the first run of the circle colour from the left: the text further right is anti-aliased grey
        x0 = next((x for x in range(0, 260) if near(px(x, y), bg, 3)), None)
        if x0 is not None:
            x1 = x0
            while x1 + 1 < 260 and near(px(x1 + 1, y), bg, 3):
                x1 += 1
            rows[y] = (x0, x1)
    out, run = [], []
    for y in sorted(rows) + [None]:
        if run and (y is None or y != run[-1] + 1):
            x0, x1 = min(rows[r][0] for r in run), max(rows[r][1] for r in run)
            out.append(((x0 + x1) / 2, (run[0] + run[-1]) / 2, run[-1] - run[0] + 1))
            run = []
        if y is not None:
            run.append(y)
    return out


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class FocusListTest(unittest.TestCase):
    def test_like_stock_app_list(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "ui-gallery"], name="focus") as (emu, port):
            async def bind():
                async with session(port):
                    pass

            asyncio.run(bind())
            harness.open_app(emu, row=0)
            emu.sleep(1)

            def focus():
                at = harness.u32(emu, S + harness.APP_BLOCK_OFF) + harness.symbol(ELF, "STATS")
                return struct.unpack("<11i", emu.peek(at, 44))[FOCUS]

            def frame(name):
                w, h, data = emu.frame()
                save_png(OUT / f"{name}.png", w, h, data)
                return w, h, lambda x, y: data[3 * (y * w + x):3 * (y * w + x) + 3]

            def check(name):
                """Circles where the stock fisheye puts icons; returns the centre y of the one nearest CY."""
                w, h, px = frame(name)
                found = [c for c in circles(w, h, px) if 60 < c[1] < h - 60 and c[2] > 40]
                self.assertTrue(found, f"{name}: no letter circle")
                for cx, cy, side in found:
                    # 4 px: the loader's integer shift and zoom (up to 1.5 px) and anti-aliased edges
                    self.assertAlmostEqual(cx, stock_icon_cx(cy - CY), delta=4,
                                           msg=f"{name}: circle at y {cy} (side {side}) off the stock fisheye")
                # the centred row has no pill: the stock rows are transparent, black right of the text
                self.assertFalse(any(sum(px(x, CY)) > 30 for x in range(330, 440, 4)),
                                 f"{name}: a pill behind the centred row")
                return min((c[1] for c in found), key=lambda y: abs(y - CY))

            self.assertEqual(focus(), 0, "row 0 is not in focus at the top")
            self.assertAlmostEqual(check("0-top"), CY, delta=2, msg="row 0 is not on the centre line at the top")

            # the crown: the stock knob scrolls by its own step (no row snap); FOCUSED names the row nearest the
            # centre line. One notch is `r -40` (emulator web UI).
            seen = []
            for i in range(1, 9):
                emu.send("r -40", pause=0.8)
                cy = check(f"1-crown{i}")
                seen.append((focus(), cy))
            print(f"\ncrown notches -> (FOCUSED, centre y of the nearest circle): {seen}")
            self.assertEqual([f for f, _ in seen], sorted(f for f, _ in seen), "focus went backwards")
            self.assertGreater(seen[-1][0], 0, "the crown did not move the focus")
            self.assertTrue(any(abs(cy - CY) > 10 for _, cy in seen), "the crown snapped rows to the centre line")

            # a slow drag rests where it was released: no snap
            harness.swipe(emu, 233, 300, 233, 250, steps=5, pause=0.5)
            rest = check("2-drag")
            emu.sleep(1)
            self.assertEqual(check("2-drag-later"), rest, "the list kept moving after the drag")
            print(f"drag rest: nearest circle at y {rest}, FOCUSED {focus()}")
            uart = emu.uart()
            self.assertNotIn("ZEPHYR FATAL ERROR", uart)
            self.assertEqual(uart.count("*** Booting Zephyr"), 1, "guest rebooted")


if __name__ == "__main__":
    unittest.main()
