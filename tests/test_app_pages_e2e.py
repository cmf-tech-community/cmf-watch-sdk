"""Apps that declare a widget or a page (app_features!, docs/app-guide.md "Page") put them next to the watch face from
the Apps menu: a long press on a row offers "Add page" / "Add widget" (wp2-loader main.rs, wp2-core cards.rs), which
edits the stock card list as the phone's 0x906D set does. The page is view 0xF8 + slot: the loader starts that app
on it, a swipe away pauses it, and a widget tap on the next card hands the loader to the tapped app. Needs
target/app-wp2.bin (scripts/build-fw, stub version 5), target/loader.wp2app, hello and vibration-lab
(scripts/build-app). Frames go to runs/app-pages/."""
import json
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import paths

OUT = paths.ROOT / "runs" / "app-pages"
RUNNING = harness.STUB_RAM + 6
APP_FLAGS, APP_PAUSED = harness.STUB_RAM + 0xA, 2
CARDS_STORE = 0x38007880  # wp2-core stock.rs
ROW_Y = lambda row: 180 + 128 * row  # menu row centres, as harness.open_app
SHEET_Y = lambda i: 130 + 84 * i + 36  # sheet button centres: wp2-loader menu.rs sheet_y


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class AppPagesTest(unittest.TestCase):
    def shot(self, emu, name):
        save_png(OUT / f"{name}.png", *emu.frame())

    def long_press(self, emu, row):
        emu.send(f"t 1 200 {ROW_Y(row)}", pause=1.5)
        emu.send(f"t 0 200 {ROW_Y(row)}", pause=2)

    def tap(self, emu, x, y, pause=3):
        emu.send(f"t 1 {x} {y}", f"t 0 {x} {y}", pause=pause)

    def cards(self, emu):
        raw = emu.peek(CARDS_STORE, 0x60)
        return [raw[i:i + 12] for i in range(0, 0x60, 12) if raw[i] in (1, 2)]

    def left(self, emu):
        harness.swipe(emu, 420, 233, 40, 233, steps=4)

    def test_page_and_widget_from_the_menu(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "hello", "vibration-lab"], name="app-pages") as (emu, port):
            harness.ready(emu)
            slots = {v["name"]: int(k) for k, v in json.loads(harness.cli_out(["list", "--hci-port", port]))["slots"].items()
                     if k != "L"}
            rows = sorted(slots, key=slots.get)  # the menu lists slots in order
            hello, lab = slots["hello"], slots["vibration-lab"]
            harness.ready(emu)  # the host's disconnect first: a second connection at once can fail
            harness.cli_out(["cards", "set", "widgets:0x40", "--hci-port", port])  # just the Apps tile

            harness.open_menu(emu)
            self.long_press(emu, rows.index("hello"))
            self.shot(emu, "1-sheet-hello")  # Add widget, Add page, Delete, Cancel
            self.assertEqual(emu.peek(RUNNING, 1), b"\xff", "the long press launched the app")
            self.tap(emu, 233, SHEET_Y(1))  # Add page
            self.long_press(emu, rows.index("vibration-lab"))
            self.shot(emu, "2-sheet-lab")  # Add widget, Delete, Cancel
            self.tap(emu, 233, SHEET_Y(0))  # Add widget
            cards = self.cards(emu)
            self.assertEqual(cards[0][:4], bytes([2, 0, 0xF8 + hello, 0]), "hello's page first")
            self.assertEqual(cards[1][:7], bytes([1, 0, 2, 0x40, 0, 0x40, lab + 1]), "lab's widget next to Apps")
            self.assertEqual(len(cards), 2)

            emu.send("c 1", "c 0", pause=3)  # crown: the menu goes, the face shows the new pages
            harness.ready(emu)
            mark = len(emu.uart())
            self.left(emu)
            self.shot(emu, "3-hello-page")
            self.assertIn(f"view {0xF8 + hello} focused", emu.uart()[mark:])
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "hello runs on its page")
            self.assertFalse(emu.peek(APP_FLAGS, 1)[0] & APP_PAUSED)

            self.left(emu)
            self.shot(emu, "4-widget-card")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "still running behind the card")
            self.assertTrue(emu.peek(APP_FLAGS, 1)[0] & APP_PAUSED, "paused while its page is away")

            mark = len(emu.uart())
            self.tap(emu, 233, 239 + 83)  # the lower widget: vibration-lab
            self.shot(emu, "5-lab-from-widget")
            self.assertRegex(emu.uart()[mark:], r"view_stack: jump \d+ -> 240")
            self.assertEqual(emu.peek(RUNNING, 1)[0], lab, "the widget's app took the loader over")

            emu.send("c 1", "c 0", pause=3)  # BUTTON: the lab exits to the menu
            emu.send("c 1", "c 0", pause=3)  # the face
            harness.ready(emu)
            self.left(emu)
            self.shot(emu, "6-hello-page-again")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "the page starts hello again")
            self.assertNotIn("FATAL", emu.uart().upper())


if __name__ == "__main__":
    unittest.main()
