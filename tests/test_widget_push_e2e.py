"""Stub 8 PUSH (docs/widget-push.md): the host sets hello's widget text while no app runs and the card shows it; the
same bytes again are no write, a change within a minute is LATER; a blob pushed for api-demo is its first message
when it starts. Needs target/app-wp2.bin, target/loader.wp2app, hello and api-demo, llvm-nm on PATH."""
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import install, paths

RUNNING = harness.STUB_RAM + 6
DEMO_ELF = paths.ROOT / "sdk" / "target" / "thumbv7em-none-eabi" / "release" / "api-demo"
CARD = (61, 150, 405, 316)  # the upper widget of the first card (x0, y0, x1, y1)


def region(frame, box):
    w, _, rgb = frame
    x0, y0, x1, y1 = box
    return b"".join(rgb[(y * w + x0) * 3:(y * w + x1) * 3] for y in range(y0, y1))


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class WidgetPushTest(unittest.TestCase):
    def card(self, emu, name):
        harness.ready(emu)
        harness.swipe(emu, 420, 233, 40, 233, steps=4)
        frame = emu.frame()
        save_png(emu.run / f"{name}.png", *frame)
        emu.send("c 1", "c 0", pause=3)  # back to the face
        return frame

    def test_push_widget_and_blob(self):
        with harness.installed(["loader", "hello", "api-demo"], name="widget-push") as (emu, port):
            push = lambda *a: harness.cli(["push", *a, "--hci-port", port])
            harness.cli_out(["cards", "set", "widgets:0x40/1", "--hci-port", port])  # hello (slot 0)
            before = self.card(emu, "1-package-name")

            self.assertEqual(push("--slot", 0, "--title", "Pushed", "--value", "from the phone"), 0)
            self.assertEqual(push("--slot", 0, "--title", "Pushed", "--value", "from the phone"), 0, "same: Ok")
            self.assertNotEqual(push("--slot", 0, "--title", "Pushed", "--value", "again"), 0, "within 60 s: LATER")
            self.assertNotEqual(push("--slot", 5, "--title", "x"), 0, "empty slot: NO_APP")
            harness.ready(emu)
            self.assertEqual(harness.read_file(emu, b"/NAND:/wp2apps/0.wgt", 64),
                             install.widget_record("Pushed", "from the phone"))
            after = self.card(emu, "2-pushed")
            self.assertNotEqual(region(before, CARD), region(after, CARD), "the card did not change")
            self.assertEqual(emu.peek(RUNNING, 1), b"\xff", "no app ran")

            self.assertEqual(push("--slot", 1, "--blob", b"blob!".hex()), 0)
            harness.open_app(emu, 1)  # api-demo: STATS[0] counts received messages
            emu.sleep(2)
            base = harness.u32(emu, harness.STUB_RAM + harness.APP_BLOCK_OFF) + harness.symbol(DEMO_ELF, "STATS")
            self.assertEqual(struct.unpack("<i", emu.peek(base, 4))[0], 1, "the blob was not its first message")
            save_png(emu.run / "3-demo.png", *emu.frame())
            self.assertNotIn("FATAL", emu.uart().upper())


if __name__ == "__main__":
    unittest.main()
