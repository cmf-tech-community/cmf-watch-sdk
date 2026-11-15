"""Custom Apps: a long press on a row opens the sheet (Add widget / Add page for what the app declared, then Delete,
Cancel); Delete removes the app, Cancel keeps it (loader menu.rs)."""
import json
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png

ROW_Y = lambda row: 180 + 128 * row  # menu row centres, as harness.open_app
SHEET_Y = lambda i: 130 + 84 * i + 36  # sheet button centres: menu.rs sheet_y
# hello declares a widget and a page (two buttons first), api-demo nothing
HELLO_CANCEL, DEMO_DELETE = (233, SHEET_Y(3)), (233, SHEET_Y(0))


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd and gdb)")
class MenuDeleteTest(unittest.TestCase):
    def test_long_press_deletes(self):
        with harness.installed(["loader", "hello", "api-demo"], name="menu-delete") as (emu, port):
            names = lambda: {v["name"] for k, v in json.loads(harness.cli_out(["list", "--hci-port", port]))["slots"].items()
                             if k != "L"}
            self.assertEqual(names(), {"hello", "api-demo"})
            harness.open_menu(emu)
            shot = lambda n: save_png(emu.run / f"{n}.png", *emu.frame())
            press = lambda x, y: emu.send(f"t 1 {x} {y}", f"t 0 {x} {y}", pause=2)
            # Cancel keeps the app, and the CLICKED after the long press does not launch it
            emu.send(f"t 1 200 {ROW_Y(0)}", pause=1.5)
            emu.send(f"t 0 200 {ROW_Y(0)}", pause=2)
            shot("confirm")
            self.assertEqual(emu.peek(harness.STUB_RAM + 6, 1), b"\xff", "the long press launched the app")
            press(*HELLO_CANCEL)
            shot("cancelled")  # both rows still there (a `list` here would pop "Paired" over the menu)
            # Delete removes row 1 (api-demo) and the list is built again
            emu.send(f"t 1 200 {ROW_Y(1)}", pause=1.5)
            emu.send(f"t 0 200 {ROW_Y(1)}", pause=2)
            press(*DEMO_DELETE)
            shot("deleted")
            self.assertEqual(names(), {"hello"})
            harness.open_app(emu, 0)  # the rebuilt list still launches
