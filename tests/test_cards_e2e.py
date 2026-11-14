"""The stub's Apps widget on the cards left of the watch face (docs/recon.md, "Widget cards"): `wp2apps cards set`
puts it on the only card over 0x906D, a left swipe from the face shows it, a tap opens the loader menu (view 0xF0),
and leaving the card runs its delete callback without a fault. Needs target/app-wp2.bin from scripts/build-fw (stub
version 3) and target/loader.wp2app. Frames go to runs/cards-e2e/."""
import json
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import cards, paths

OUT = paths.ROOT / "runs" / "cards-e2e"
# the single 344x166 widget sits at (61, 61) on its card
CARD_TAP = (61 + 344 // 2, 61 + 166 // 2)


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class AppsWidgetTest(unittest.TestCase):
    def shot(self, emu, name):
        w, h, rgb = emu.frame()
        save_png(OUT / f"{name}.png", w, h, rgb)

    def test_apps_card(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader"], name="cards") as (emu, port):
            harness.ready(emu)
            got = json.loads(harness.cli_out(["cards", "set", f"widgets:{cards.APPS_WIDGET}", "--hci-port", port]))
            self.assertEqual(got["cards"], [{"widgets": [{"id": cards.APPS_WIDGET, "param": 0}]}])
            harness.ready(emu)  # the host's session popup and screen-off, as after install

            mark = len(emu.uart())
            harness.swipe(emu, 420, 233, 40, 233, steps=4)
            self.shot(emu, "1-card")
            uart = emu.uart()[mark:]
            self.assertNotIn("wgt_reg not found", uart)
            self.assertNotIn("card widget_info == NULL", uart)

            mark = len(emu.uart())
            emu.send(f"t 1 {CARD_TAP[0]} {CARD_TAP[1]}", f"t 0 {CARD_TAP[0]} {CARD_TAP[1]}", pause=3)
            self.shot(emu, "2-menu")
            self.assertRegex(emu.uart()[mark:], r"view_stack: jump \d+ -> 240")

            mark = len(emu.uart())
            emu.send("c 1", "c 0", pause=3)  # crown: back to the face, the card view goes with it
            self.shot(emu, "3-face")
            uart = emu.uart()[mark:]
            self.assertNotIn("FATAL", uart.upper())


if __name__ == "__main__":
    unittest.main()
