"""App widgets on the cards left of the watch face (docs/app-guide.md, "Widget"; docs/recon.md, "Widget cards"):
four example apps, each as widget 0x40/(slot + 1), two wide widgets per card. Before its first run a widget shows
the package name; a tap starts exactly that app through the loader; the app writes its record on exit and the card
shows it from the next layout. Needs target/app-wp2.bin (scripts/build-fw, stub version 4), target/loader.wp2app
and the four apps (scripts/build-app). Frames go to runs/app-widgets/."""
import json
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import paths

OUT = paths.ROOT / "runs" / "app-widgets"
RUNNING = harness.STUB_RAM + 6
APPS = ["hello", "vibration-lab", "hr-probe", "herdr-agents"]
TITLES = {"hello": b"Hello", "vibration-lab": b"Vibration Lab", "hr-probe": b"Last heart rate", "herdr-agents": b"herdr"}
# wide widgets at (61, 61) and (61, 239) on a card, 344x166
TOP, BOTTOM = 61, 239
X, W, H = 61, 344, 166


def crop(frame, y0):
    w, _, rgb = frame
    return b"".join(rgb[(y * w + X) * 3:(y * w + X + W) * 3] for y in range(y0, y0 + H))


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class AppWidgetsTest(unittest.TestCase):
    def shot(self, emu, name):
        frame = emu.frame()
        save_png(OUT / f"{name}.png", *frame)
        return frame

    def to_card(self, emu, card):
        for _ in range(card + 1):
            harness.swipe(emu, 420, 233, 40, 233, steps=4)

    def test_app_widgets(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", *APPS], name="app-widgets") as (emu, port):
            harness.ready(emu)
            slots = {v["name"]: int(k) for k, v in json.loads(harness.cli_out(["list", "--hci-port", port]))["slots"].items()
                     if k != "L"}
            spec = [f"widgets:0x40/{slots[a] + 1},0x40/{slots[b] + 1}" for a, b in (APPS[:2], APPS[2:])]
            harness.cli_out(["cards", "set", *spec, "--hci-port", port])
            harness.ready(emu)

            before = {}
            for card in (0, 1):
                self.to_card(emu, card)
                frame = self.shot(emu, f"0-card{card + 1}-names")
                for i, y in enumerate((TOP, BOTTOM)):
                    before[APPS[card * 2 + i]] = crop(frame, y)
                emu.send("c 1", "c 0", pause=3)  # crown: back to the face
            self.assertNotIn("wgt_reg not found", emu.uart())

            for n, app in enumerate(APPS, 1):
                with self.subTest(app):
                    card, y = divmod(APPS.index(app), 2)
                    y = (TOP, BOTTOM)[y] + H // 2
                    self.to_card(emu, card)
                    mark = len(emu.uart())
                    emu.send(f"t 1 233 {y}", f"t 0 233 {y}", pause=4)
                    self.assertRegex(emu.uart()[mark:], r"view_stack: jump \d+ -> 240")
                    self.assertEqual(emu.peek(RUNNING, 1)[0], slots[app], f"the tap did not start {app}")
                    self.shot(emu, f"{n}-{app}-app")
                    emu.send("c 1", "c 0", pause=3)  # BUTTON: the app writes its record and returns to the menu
                    self.assertEqual(emu.peek(RUNNING, 1)[0], 0xFF, f"{app} did not exit")
                    rec = harness.read_file(emu, f"/NAND:/wp2apps/{slots[app]}.wgt".encode(), 64)
                    self.assertEqual(len(rec or b""), 64, f"{app} wrote no widget record")
                    self.assertEqual(rec[:24].rstrip(b"\0"), TITLES[app])
                    print(app, rec[:24].rstrip(b"\0"), rec[24:].rstrip(b"\0"))
                    emu.send("c 1", "c 0", pause=3)  # menu -> face, which lays the cards out again
                    self.to_card(emu, card)
                    frame = self.shot(emu, f"{n}-{app}-widget")
                    self.assertNotEqual(crop(frame, y - H // 2), before[app], f"{app}: the card still shows the name")
                    emu.send("c 1", "c 0", pause=3)

            for card in (0, 1):
                self.to_card(emu, card)
                self.shot(emu, f"5-card{card + 1}-data")
                emu.send("c 1", "c 0", pause=3)
            self.assertNotIn("FATAL", emu.uart().upper())


if __name__ == "__main__":
    unittest.main()
