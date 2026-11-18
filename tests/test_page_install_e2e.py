"""An app page behind the face keeps the loader loaded, yet a K host installs a new loader and the page's app without
the user removing the page (stub 6, ble.rs `behind_face`): the stub takes the packages and loads them again on the
page's next FOCUS (view.rs). A page in view stays BUSY. Needs target/app-wp2.bin (scripts/build-fw, stub 6),
target/loader.wp2app, target/loader-v2.wp2app (WP2_LOADER_VERSION=2 scripts/build-fw) and hello (scripts/build-app).
Frames go to runs/page-install/."""
import asyncio
import json
import struct
import unittest
import zlib
from unittest import skipUnless

from tests.e2e import harness
from tests.test_chan_key_e2e import k_session
from tests.test_demo_e2e import save_png
from wp2apps import h4, install, paths

OUT = paths.ROOT / "runs" / "page-install"
RAM = harness.STUB_RAM
ACTIVE, RUNNING, VIEW, FOCUS, RELOAD = RAM + 5, RAM + 6, RAM + 0xB62, RAM + 0xB64, RAM + 0xB65  # layout.rs
LOADER_V2 = paths.ROOT / "target" / "loader-v2.wp2app"
HELLO = paths.ROOT / "target" / "hello.wp2app"


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class PageInstallTest(unittest.TestCase):
    def k(self, port, work):
        async def run():
            async with h4.gatt_over_h4(port) as gatt, k_session(gatt) as s:
                return await work(s)
        return asyncio.run(run())

    def test_install_under_a_page_behind_the_face(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "hello"], name="page-install") as (emu, port):
            harness.ready(emu)
            hello = int(next(k for k, v in json.loads(harness.cli_out(["list", "--hci-port", port]))["slots"].items()
                             if v["name"] == "hello"))
            page = 0xF8 + hello
            harness.ready(emu)
            harness.cli_out(["cards", "set", f"view:{page:#x}", "--hci-port", port])
            harness.open_menu(emu)  # the bind left the face as it was: a menu round trip lays the face out again
            emu.send("c 1", "c 0", pause=3)
            harness.ready(emu)
            self.assertEqual(emu.peek(ACTIVE, 1), b"\x01", "the page holds the loader")
            self.assertEqual(struct.unpack("<H", emu.peek(VIEW, 2))[0], page)
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "hello runs behind the face")
            self.assertEqual(emu.peek(FOCUS, 1), b"\x00")

            async def update(s):
                await install.upload(s, install.LOADER_SLOT, LOADER_V2.read_bytes())
                await install.upload(s, hello, HELLO.read_bytes())
            self.k(port, update)  # stub 5: BUSY on the first begin
            self.assertEqual(emu.peek(RELOAD, 1), b"\x01")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "the old instance runs on until the page shows")

            mark = len(emu.uart())
            harness.swipe(emu, 420, 233, 40, 233, steps=4)
            save_png(OUT / "1-page-after-update.png", *emu.frame())
            log = emu.uart()[mark:]
            self.assertIn(f"view {page} focused", log)
            self.assertIn(f"view {page} layout", log, "the new loader was laid out")
            self.assertEqual(emu.peek(RELOAD, 1), b"\x00")
            self.assertEqual(emu.peek(FOCUS, 1), b"\x01")
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello, "the new loader started hello on the page")

            hello_pkg = HELLO.read_bytes()

            async def in_view(s):
                version = (await install.info(s, install.LOADER_SLOT)).version
                with self.assertRaisesRegex(ValueError, "BUSY"):  # the page is in view: a foreground app
                    await install._call(s, install.BEGIN,
                                        struct.pack("<BII", hello, len(hello_pkg), zlib.crc32(hello_pkg)))
                return version
            self.assertEqual(self.k(port, in_view), 2)
            self.assertEqual(emu.peek(RUNNING, 1)[0], hello)
            self.assertNotIn("FATAL", emu.uart().upper())


if __name__ == "__main__":
    unittest.main()
