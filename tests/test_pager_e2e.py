"""The vertical pager (stock lv_tileview, `stock::pager`) in ui-gallery's "Pages" section: the crown steps one page,
a page taller than the screen scrolls first and pages at its end, swipes up/down page, the app gets the page event,
and the right swipe still leaves the app (BUTTON). Frames in runs/pager/. Needs target/{loader,ui-gallery}.wp2app."""
import asyncio
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_api_e2e import session, unpaused
from tests.test_demo_e2e import save_png
from wp2apps import channel, paths

S = harness.STUB_RAM
RUNNING = S + 6
ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/ui-gallery"
OUT = paths.ROOT / "runs" / "pager"
RC, OPEN, OPENED, TICKS, PAGE = range(5)  # STATS, ui-gallery main.rs
PAGES = 6  # screens::PAGES
NEXT, PREV = "r -80", "r 80"  # crown: down the content / up


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class PagerTest(unittest.TestCase):
    def test_pager(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "ui-gallery"], name="pager") as (emu, port):
            async def bind():
                async with session(port):
                    pass

            asyncio.run(bind())
            harness.open_app(emu, row=0)
            self.assertEqual(emu.peek(RUNNING, 1), b"\x00", "ui-gallery did not start")

            def stats():
                at = harness.u32(emu, S + harness.APP_BLOCK_OFF) + harness.symbol(ELF, "STATS")
                return struct.unpack("<5i", emu.peek(at, 20))

            def frame(name):
                w, h, rgb = emu.frame()
                save_png(OUT / name, w, h, rgb)
                return rgb

            def message(text):
                async def run():
                    async with session(port) as s:
                        await unpaused(s)
                        await channel.send(s, text)

                asyncio.run(run())

            def crown(cmd):
                emu.send(cmd, pause=1.5)

            def swipe(y0, y1):
                harness.swipe(emu, 233, y0, 233, y1, steps=8, pause=2)

            emu.sleep(1)
            message(f"!open {PAGES}".encode())
            for _ in range(40):
                if stats()[OPEN] == PAGES + 1:
                    break
                emu.sleep(0.25)
            emu.sleep(1)
            self.assertEqual(stats()[RC], 0, stats())
            self.assertEqual(stats()[PAGE], 0)
            first = frame("0-page1.png")

            crown(NEXT)  # page 1 fits the screen: one step, next page
            self.assertEqual(stats()[PAGE], 1, "crown did not page")
            frame("1-page2-top.png")

            steps, last = 0, frame("tmp.png")
            while stats()[PAGE] == 1 and steps < 20:  # page 2 is taller: it scrolls first
                crown(NEXT)
                steps += 1
                rgb = frame("tmp.png")
                if stats()[PAGE] == 1:
                    self.assertNotEqual(rgb, last, f"crown step {steps} neither scrolled nor paged")
                last = rgb
            print(f"\nlong page: {steps} crown steps to the next page")
            self.assertEqual(stats()[PAGE], 2)
            self.assertGreater(steps, 1, "the long page did not scroll before paging")
            frame("2-page3.png")

            crown(PREV)  # page 3 fits: back to page 2, which kept its scroll at the bottom
            self.assertEqual(stats()[PAGE], 1)
            frame("3-page2-bottom.png")

            swipe(400, 100)  # page 2 is at its end: the drag pages
            self.assertEqual(stats()[PAGE], 2, "swipe up did not page")
            swipe(100, 400)
            self.assertEqual(stats()[PAGE], 1, "swipe down did not page")
            frame("4-swiped-back.png")

            message(b"!open 6")  # rebuilt: page 1 again, as at the start
            emu.sleep(1.5)
            self.assertEqual(stats()[PAGE], 0)
            self.assertEqual(frame("5-reopened.png"), first)

            mark = len(emu.uart())
            harness.swipe(emu, 60, 233, 420, 233, steps=8, pause=3)  # the stock back swipe: BUTTON, the app returns
            self.assertEqual(emu.peek(RUNNING, 1), b"\xff", "back swipe did not leave the app")
            self.assertNotIn("view_stack: jump", emu.uart()[mark:])  # to the loader menu, not the stock list
            frame("6-back-menu.png")
            (OUT / "tmp.png").unlink(missing_ok=True)
            uart = emu.uart()
            self.assertNotIn("MllcFailedHook", uart)
            self.assertNotIn("ZEPHYR FATAL ERROR", uart)
            self.assertEqual(uart.count("*** Booting Zephyr"), 1, "guest rebooted")


if __name__ == "__main__":
    unittest.main()
