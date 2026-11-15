"""ui-gallery (LVGL, stock look) in the emulator: the menu builds, every section opens (host message `!open N`) and
draws without stopping the guest, `!menu` comes back, and the GUI heap returns to the menu's level after each.
Needs target/{loader,ui-gallery}.wp2app. WP2_NETSIM_INSTANCE=88."""
import asyncio
import struct
import unittest
from unittest import skipUnless

from scripts.recon.gui_heap_probe import GUI_HEAP, GUI_SIZE, walk_heap
from tests.e2e import harness
from tests.test_api_e2e import session, unpaused
from tests.test_demo_e2e import save_png
from wp2apps import channel, paths

S = harness.STUB_RAM
RUNNING = S + 6
ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/ui-gallery"
OUT = paths.ROOT / "runs" / "gallery"
RC, OPEN, OPENED, TICKS = range(4)  # STATS, main.rs
SECTIONS = ["lists", "buttons", "switches", "gauges", "picker", "fonts", "pages"]


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class GalleryTest(unittest.TestCase):
    def test_every_section(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "ui-gallery"], name="gallery") as (emu, port):
            async def bind():  # before the app opens: harness.ready closes the "Paired" popup of this bind
                async with session(port):
                    pass

            asyncio.run(bind())
            harness.open_app(emu, row=0)
            self.assertEqual(emu.peek(RUNNING, 1), b"\x00", "ui-gallery did not start")

            def stats():
                at = harness.u32(emu, S + harness.APP_BLOCK_OFF) + harness.symbol(ELF, "STATS")
                return struct.unpack("<4i", emu.peek(at, 16))

            def free():
                ptr = harness.u32(emu, GUI_HEAP)
                return walk_heap(emu.peek(ptr, GUI_SIZE), ptr)["free"]

            def message(text):
                async def run():
                    async with session(port) as s:
                        await unpaused(s)
                        await channel.send(s, text)

                asyncio.run(run())

            def frame(name):
                w, h, rgb = emu.frame()
                save_png(OUT / name, w, h, rgb)
                return sum(1 for i in range(0, len(rgb), 3 * 97) if any(rgb[i:i + 3]))

            emu.sleep(1)
            self.assertEqual(stats()[RC], 0, stats())
            menu = free()
            self.assertGreater(frame("menu.png"), 50, "menu not drawn")
            for i, name in enumerate(SECTIONS):
                with self.subTest(section=name):
                    n = stats()[OPENED]
                    message(f"!open {i}".encode())
                    for _ in range(40):
                        if stats()[OPENED] > n:
                            break
                        emu.sleep(0.25)
                    emu.sleep(1)
                    st = stats()
                    print(f"\n{name}: STATS {st}, GUI heap free {free()} (menu {menu})")
                    self.assertEqual((st[RC], st[OPEN]), (0, i + 1), st)
                    self.assertGreater(frame(f"{i}-{name}.png"), 50, "section not drawn")
                    self.assertNotIn("stopped on", (emu.run / "qemu.log").read_text())
                    message(b"!menu")
                    emu.sleep(1.5)
                    self.assertEqual(stats()[OPEN], 0)
                    self.assertLessEqual(abs(free() - menu), 256, "section leaked GUI heap")
            emu.send("c 1", "c 0", pause=2)
            self.assertEqual(emu.peek(RUNNING, 1), b"\xff")
            uart = emu.uart()
            self.assertNotIn("MllcFailedHook", uart)
            self.assertNotIn("ZEPHYR FATAL ERROR", uart)
            self.assertEqual(uart.count("*** Booting Zephyr"), 1, "guest rebooted")


if __name__ == "__main__":
    unittest.main()
