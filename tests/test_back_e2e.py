"""Back navigation (docs/recon.md "Back navigation"): app -> Custom Apps menu -> stock app list, like stock apps."""
import shutil
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_e2e import CMU_PWMCLK0, START, bright_pixels, dismiss_popup, same_screen
from wp2apps import paths

OUT = paths.ROOT / "runs" / "back"
CUSTOM_APPS = (170, 108, 200, 40)  # "Custom Apps" label, list scrolled to the top
CLOCK_VIEW = 16
RETURN_CB, RETURN_CLOCK = 0x38019B10, 0x1005D969  # wp2_core::stock RETURN_CB, RETURN_CLOCK


def back(emu):
    """Right swipe with 8 moves, closer to a finger than the 2-move swipe of the harness."""
    harness.swipe(emu, 60, 233, 420, 233, steps=8, pause=3)


def save(emu, name):
    OUT.mkdir(parents=True, exist_ok=True)
    shutil.copy(emu.run / "frame.ppm", OUT / f"{name}.ppm")


def tap_row(emu, row, pause=2):
    y = 180 + 128 * row  # wp2-loader menu.rs
    emu.send(f"t 1 200 {y}", f"t 0 200 {y}", pause=pause)
    if emu.peek(START + 6, 1) == b"\xff":
        raise RuntimeError(f"app in row {row} did not start")


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd and gdb)")
class BackTest(unittest.TestCase):
    def assert_menu(self, emu, mark):
        self.assertEqual(emu.peek(START + 4, 3), b"\x01\x01\xff")  # loader active, no app
        self.assertEqual(harness.u32(emu, START + harness.APP_BLOCK_OFF), 0)
        self.assertEqual(emu.motor()[-1][1], 0)
        self.assertNotIn("view_stack: jump", emu.uart()[mark:])
        self.assertEqual(harness.focused(emu), 240)

    def assert_clean(self, emu):
        uart = emu.uart()
        self.assertNotIn("ZEPHYR FATAL ERROR", uart)
        self.assertNotIn("MllcFailedHook", uart)
        self.assertEqual(uart.count("*** Booting Zephyr"), 1, "guest rebooted")

    def test_back_app_menu_list(self):
        with harness.installed(["loader", "hello", "motor-hog"]) as (emu, port):
            dismiss_popup(emu)
            harness.ready(emu)
            harness.open_apps(emu)
            shutil.copy(emu.run / "list.ppm", OUT / "list-before.ppm")
            self.assertEqual(harness.focused(emu), 240)

            tap_row(emu, 0)  # hello: returns on BUTTON
            mark = len(emu.uart())
            back(emu)
            self.assert_menu(emu, mark)
            save(emu, "hello-back-menu")

            tap_row(emu, 1, pause=0.5)  # motor-hog: never reads events, motor on
            self.assertGreater(emu.motor()[-1][1], 0)
            mark, start = len(emu.uart()), emu.clock()
            harness.swipe(emu, 60, 233, 420, 233, steps=8, pause=0)
            while emu.peek(START + 6, 1) != b"\xff" and emu.clock() - start < 4:
                emu.sleep(0.125)
            print(f"\nback escape after {emu.clock() - start:.3f} s")
            self.assertLessEqual(emu.clock() - start, 3)
            emu.sleep(1)
            self.assert_menu(emu, mark)
            self.assertEqual(harness.u32(emu, CMU_PWMCLK0), 0x303)  # stock PWM clock back
            save(emu, "hog-back-menu")

            mark = len(emu.uart())
            back(emu)
            uart = emu.uart()[mark:]
            self.assertIn("view_stack: jump 240 -> 18", uart)
            self.assertEqual(harness.focused(emu), harness.APPLIST_VIEW)
            self.assertEqual(emu.peek(START + 4, 2), b"\x00\x00")  # loader unloaded
            save(emu, "menu-back-list")
            w, h, rgb = emu.frame()
            self.assertGreater(bright_pixels(rgb, w, CUSTOM_APPS), 50)
            self.assertTrue(same_screen(emu.frame(), harness_frame(OUT / "list-before.ppm")), "list moved")
            self.assert_clean(emu)

    def test_button_unchanged(self):
        with harness.installed(["loader", "hello"]) as (emu, port):
            dismiss_popup(emu)
            harness.ready(emu)
            harness.open_apps(emu)
            tap_row(emu, 0)
            mark = len(emu.uart())
            emu.send("c 1", "c 0", pause=3)  # app: BUTTON, hello returns, menu
            self.assert_menu(emu, mark)
            save(emu, "button-app-menu")
            mark = len(emu.uart())
            emu.send("c 1", "c 0", pause=3)  # menu: stock closes the view, clock
            self.assertIn("view_stack: jump 240 -> 16", emu.uart()[mark:])
            self.assertEqual(harness.focused(emu), CLOCK_VIEW)
            self.assertEqual(emu.peek(START + 4, 2), b"\x00\x00")
            save(emu, "button-menu-clock")
            self.assert_clean(emu)


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd and gdb)")
class OwnBackTest(unittest.TestCase):
    """ABI 1.3 own_back (docs/abi-1.3.md): back-owner reads every BUTTON and is not stopped; hung, it still is."""
    ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/back-owner"

    def presses(self, emu):
        addr = harness.u32(emu, START + harness.APP_BLOCK_OFF) + harness.symbol(self.ELF, "PRESSES")
        return struct.unpack("<i", emu.peek(addr, 4))[0]

    def running(self, emu):
        return emu.peek(START + 6, 1) != b"\xff"

    def test_owned_back(self):
        with harness.installed(["loader", "back-owner"], name="own-back") as (emu, port):
            dismiss_popup(emu)
            harness.ready(emu)
            harness.open_apps(emu)
            tap_row(emu, 0)
            for n in (1, 2):
                emu.send("c 1", "c 0", pause=2)  # well past the 1 s escape
                self.assertTrue(self.running(emu), f"stopped after press {n}")
                self.assertEqual(self.presses(emu), n)
            emu.send("c 1", "c 0", pause=2)  # third: the app returns 0
            self.assertFalse(self.running(emu))
            self.assertEqual(emu.peek(START + 4, 3), b"\x01\x01\xff")  # back in the loader menu

            tap_row(emu, 0)
            emu.send("r -80", pause=0.5)  # crown: the app stops reading events
            start = emu.clock()
            emu.send("c 1", "c 0", pause=0)
            while self.running(emu) and emu.clock() - start < 4:
                emu.sleep(0.125)
            print(f"\nhung owner stopped after {emu.clock() - start:.3f} s")
            self.assertFalse(self.running(emu))
            self.assertLessEqual(emu.clock() - start, 3)


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd and gdb)")
class ScreenOffTest(unittest.TestCase):
    """Auto-sleep blanks the panel while an app runs (docs/recon.md "Screen-off and the app view"). A BLE session
    holds the guest in S1: deep sleep (S2/S3BT) is not modelled and stops the emulator."""
    ELF = OwnBackTest.ELF

    @staticmethod
    def blank_and_wake(emu, port, during=None, wake=("c 1", "c 0")):
        """Blanks the panel by auto-sleep (1.5 s) while a BLE session holds S1, then wakes it with `wake`."""
        import asyncio, threading
        from tests.test_api_e2e import session
        stop = threading.Event()

        async def hold():
            async with session(port):
                while not stop.is_set():
                    await asyncio.sleep(0.2)

        t = threading.Thread(target=lambda: asyncio.run(hold()), daemon=True)
        t.start()
        try:
            emu.sleep(2)
            mark = len(emu.uart())
            emu.gdb(f"set *(unsigned *)*(unsigned *){harness.AWAKE_PTR:#x} = 1500")
            for _ in range(80):  # 40 s: a heart-rate measurement holds the stock 25 s auto-sleep (docs/abi-1.4.md)
                emu.sleep(0.5)
                if "display blanking on" in emu.uart()[mark:]:
                    break
            else:
                raise AssertionError("panel never blanked")
            if during:
                during()
            emu.gdb(f"set *(unsigned *)*(unsigned *){harness.AWAKE_PTR:#x} = 0xFFFFFFFF")
            emu.send(*wake, pause=3)
            if "display blanking off" not in emu.uart()[mark:]:
                raise AssertionError("panel did not wake")
        finally:
            stop.set()
            t.join(10)

    def open(self, emu, port):
        import asyncio
        from tests.test_api_e2e import session

        async def bind():  # before the app opens: harness.ready closes the "Paired" popup of this bind
            async with session(port):
                pass
        asyncio.run(bind())
        harness.ready(emu)
        harness.open_apps(emu)
        tap_row(emu, 0)

    def test_app_survives_blank(self):
        with harness.installed(["loader", "back-owner"], name="screen-off") as (emu, port):
            self.open(emu, port)
            self.blank_and_wake(emu, port)
            self.assertEqual(emu.peek(START + 6, 1), b"\x00", "app stopped by the blank or the wake")
            self.assertEqual(harness.focused(emu), 240)
            self.assertEqual(OwnBackTest.presses(self, emu), 0, "the waking press reached the app")
            emu.send("c 1", "c 0", pause=1.5)  # 3 s after the wake, past the loader's WAKE_KEY_MS: a real press
            self.assertEqual(OwnBackTest.presses(self, emu), 1, "the press after the wake was eaten")

    def test_app_survives_return_to_face(self):
        """Late-resume goes back to the clock when the panel was dark for the "return to watch face" time (ux2sys
        0x9C, 30 s at least); on hardware only light sleep counts, so it struck once heart rate kept deep sleep off."""
        with harness.installed(["loader", "back-owner"], name="screen-off-return") as (emu, port):
            self.open(emu, port)
            self.assertEqual(harness.u32(emu, RETURN_CB), 0, "the loader holds the return off while an app runs")
            mark = len(emu.uart())
            # early-suspend wrote the setting in ms at +0x18: 1 s here, 3 s dark
            self.blank_and_wake(emu, port, during=lambda: (emu.poke(RETURN_CB + 0x18, struct.pack("<I", 1000)),
                                                           emu.sleep(3)))
            self.assertNotIn("return clock", emu.uart()[mark:])
            self.assertTrue(OwnBackTest.running(self, emu), "app stopped by the return to the clock")
            self.assertEqual(harness.focused(emu), 240)
            for _ in range(3):  # back-owner returns on its third press
                emu.send("c 1", "c 0", pause=2)
            self.assertFalse(OwnBackTest.running(self, emu))
            self.assertEqual(harness.u32(emu, RETURN_CB), RETURN_CLOCK, "the stock return is back for the menu")

    def test_buzz_while_dark(self):
        with harness.installed(["loader", "back-owner"], name="screen-off-buzz") as (emu, port):
            self.open(emu, port)
            emu.send("t 1 233 233", "t 0 233 233", pause=0)  # buzz in 4 s; the panel blanks before that
            seen = []
            self.blank_and_wake(emu, port, during=lambda: (emu.sleep(5), seen.extend(emu.motor())))
            uart = emu.uart()
            blank_ms = int(uart[:uart.index("display blanking on")].rsplit("[", 4)[-4].split("]")[0])
            buzz = [ms for ms, duty, _ in seen if duty > 0 and ms > blank_ms]
            print(f"\nblank at {blank_ms} ms, buzz at {buzz[:1]}")
            self.assertTrue(buzz, f"no buzz while dark: blank {blank_ms}, motor {seen[-5:]}")


def harness_frame(path):
    magic, size, maxval, pixels = path.read_bytes().split(b"\n", 3)
    w, h = map(int, size.split())
    return w, h, pixels[:w * h * 3]


if __name__ == "__main__":
    unittest.main()
