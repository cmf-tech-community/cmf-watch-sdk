"""A host deletes an app whose page is behind the face (stub 7, ble.rs `forget`): the stub takes its page and widget off
the cards (as "Remove page" / "Remove widget" on the watch), unlinks the package and answers OK. The same DELETE while
the page is in view is BUSY. The stock lays the face out again at once (as after the phone's 0x906D set): the page
view is deleted, which stops its app. Deleting an app with a widget while another app is in the foreground leaves
that app running. Needs target/app-wp2.bin (scripts/build-fw, stub 7), target/loader.wp2app, hello, vibration-lab and
hr-probe (scripts/build-app). Frames go to runs/page-delete/."""
import asyncio
import json
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_chan_key_e2e import k_session
from tests.test_demo_e2e import save_png
from wp2apps import h4, install, paths

OUT = paths.ROOT / "runs" / "page-delete"
RAM = harness.STUB_RAM
ACTIVE, RUNNING, VIEW, FOCUS = RAM + 5, RAM + 6, RAM + 0xB62, RAM + 0xB64  # layout.rs
CARDS_STORE = 0x38007880  # wp2-core stock.rs


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class PageDeleteTest(unittest.TestCase):
    def k(self, port, work):
        async def run():
            async with h4.gatt_over_h4(port) as gatt, k_session(gatt) as s:
                return await work(s)
        return asyncio.run(run())

    def cards(self, emu):
        raw = emu.peek(CARDS_STORE, 0x60)
        return [raw[i:i + 12] for i in range(0, 0x60, 12) if raw[i] in (1, 2)]

    def test_delete_a_page_app(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "hello", "vibration-lab", "hr-probe"], name="page-delete") as (emu, port):
            harness.ready(emu)
            slots = {v["name"]: int(k) for k, v in json.loads(harness.cli_out(["list", "--hci-port", port]))["slots"].items()
                     if k != "L"}
            hello, lab, hr = slots["hello"], slots["vibration-lab"], slots["hr-probe"]
            page = 0xF8 + hello
            harness.ready(emu)
            harness.cli_out(["cards", "set", f"view:{page:#x}", f"widgets:0x40/{hello + 1},0x40/{lab + 1}", f"widgets:0x40/{hr + 1}",
                             "--hci-port", port])
            harness.open_menu(emu)  # the bind left the face as it was: a menu round trip lays the face out again
            emu.send("c 1", "c 0", pause=3)
            harness.ready(emu)

            harness.swipe(emu, 420, 233, 40, 233, steps=4)  # the page in view: a foreground app
            self.assertEqual(emu.peek(FOCUS, 1), b"\x01")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello)

            def delete(slot):
                async def work(s):
                    try:
                        await install.remove(s, slot)
                        return "OK"
                    except ValueError as e:
                        return str(e)
                return self.k(port, work)
            self.assertIn("BUSY", delete(hello))
            self.assertEqual(len(self.cards(emu)), 3, "a refused DELETE leaves the cards")

            harness.swipe(emu, 40, 233, 420, 233, steps=4)  # back to the face: the page is behind it
            harness.ready(emu)
            self.assertEqual(emu.peek(ACTIVE, 1), b"\x01", "the page holds the loader")
            self.assertEqual(struct.unpack("<H", emu.peek(VIEW, 2))[0], page)
            self.assertEqual(emu.peek(FOCUS, 1), b"\x00")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "hello runs behind the face")

            mark = len(emu.uart())
            self.assertEqual(delete(hello), "OK")  # stub 6: BUSY
            cards = self.cards(emu)
            self.assertEqual(len(cards), 2, "the page is gone")
            self.assertEqual(cards[0][:5], bytes([1, 0, 1, 0x40, lab + 1]), "hello's widget is gone, the lab's stays")
            self.assertEqual(harness.read_file(emu, f"/NAND:/wp2apps/{hello}.pkg".encode(), 64), None)
            log = emu.uart()[mark:]
            self.assertIn(f"view {page} deleted", log, "the stock dropped the page view")
            self.assertEqual(emu.peek(ACTIVE, 1), b"\x00", "the page's loader instance was deleted")
            self.assertEqual(emu.peek(RUNNING, 1), b"\xff", "hello was stopped")

            harness.ready(emu)
            mark = len(emu.uart())
            harness.swipe(emu, 420, 233, 40, 233, steps=4)
            save_png(OUT / "1-left-of-face.png", *emu.frame())
            self.assertIn("view 208 focused", emu.uart()[mark:], "left of the face is the widget card now")
            self.assertNotIn(f"view {page}", emu.uart()[mark:])
            emu.send("c 1", "c 0", pause=3)

            harness.open_app(emu, sorted(v for v in slots.values() if v != hello).index(lab))  # rows in slot order
            self.assertEqual(emu.peek(RUNNING, 1)[0], lab)
            self.assertEqual(delete(hr), "OK")  # only a widget, the lab in the foreground
            self.assertEqual(emu.peek(RUNNING, 1)[0], lab, "the foreground app keeps running")
            self.assertEqual(emu.peek(FOCUS, 1), b"\x01")
            self.assertEqual([c[:5] for c in self.cards(emu)], [bytes([1, 0, 1, 0x40, lab + 1])])
            save_png(OUT / "2-lab-after-delete.png", *emu.frame())
            self.assertNotIn("FATAL", emu.uart().upper())


if __name__ == "__main__":
    unittest.main()
