"""Heart rate for apps (docs/abi-1.4.md, "Heart rate"): hr-probe asks for it, the loader starts the stock measurement
and hands the app the stock reading as HEART events; pause, a request off, a crash and exit stop it again. The
emulator has no PPG model: the reading is a synthetic value written into the stock cell (docs/recon.md).
Needs target/{loader,hr-probe}.wp2app."""
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_back_e2e import tap_row
from tests.test_demo_e2e import save_png
from tests.test_e2e import START, dismiss_popup
from wp2apps import paths

OUT = paths.ROOT / "runs" / "hr-e2e"
ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/hr-probe"
WEAR, OWNER, BPM = 0x3804B30C, 0x3804B30F, 0x3804B310  # wp2_core::stock HR_*
REQUESTS = 0x38048D16 + 5  # stock start/stop request count of the HEARTRATE algorithm (docs/recon.md)
RESULT, EVENTS, DELTA, X = range(4)  # PROBE, hr-probe main.rs


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class HeartRateTest(unittest.TestCase):
    def probe(self, emu):
        addr = harness.u32(emu, START + harness.APP_BLOCK_OFF) + harness.symbol(ELF, "PROBE")
        return struct.unpack("<4i", emu.peek(addr, 16))

    def owner(self, emu):
        return emu.peek(OWNER, 1)[0]

    def assert_on(self, emu, on, msg=None):
        """Owner flag and the stock's request count: one start per hold, every start stopped."""
        self.assertEqual((self.owner(emu), emu.peek(REQUESTS, 1)[0]), (1, 1) if on else (0, 0), msg)

    def running(self, emu):
        return emu.peek(START + 6, 1) != b"\xff"

    def shot(self, emu, name):
        w, h, rgb = emu.frame()
        save_png(OUT / f"{name}.png", w, h, rgb)

    def test_heart_rate(self):
        OUT.mkdir(parents=True, exist_ok=True)
        with harness.installed(["loader", "hr-probe"], name="hr") as (emu, port):
            dismiss_popup(emu)
            harness.ready(emu)
            harness.open_apps(emu)
            mark = len(emu.uart())
            tap_row(emu, 0)
            emu.sleep(2)
            self.assertEqual(self.probe(emu)[RESULT], 0)
            self.assert_on(emu, True, "measurement on (property 2)")
            self.assertIn("ux2sys change_hr_mode :1", emu.uart()[mark:])
            self.assertIn("VC9202 Start Later---m0 h1", emu.uart()[mark:])
            self.assertNotIn("set_autosleep_time:25000", emu.uart()[mark:], "no screen hold: the panel keeps its timeout")
            self.assertEqual(emu.peek(BPM, 1), b"\x00", "no reading before the sensor gives one")

            emu.poke(BPM, bytes([67]))
            emu.sleep(2.5)
            p = self.probe(emu)
            self.assertGreaterEqual(p[EVENTS], 2)
            self.assertEqual((p[DELTA], p[X]), (67, 1))
            self.shot(emu, "1-app-67")

            events = p[EVENTS]
            emu.sleep(12)  # past 10 s: still one start request
            self.assertIn(self.probe(emu)[EVENTS] - events, range(9, 15), "about one HEART per second")
            self.assert_on(emu, True)

            emu.poke(WEAR, b"\x00")
            emu.sleep(2)
            self.assertEqual(self.probe(emu)[DELTA:], (0, 0), "off the wrist")
            emu.poke(WEAR, b"\x01")

            emu.send("t 1 233 233", "t 0 233 233", pause=1)  # request off
            self.assert_on(emu, False)
            emu.send("t 1 233 233", "t 0 233 233", pause=1)  # on again
            self.assert_on(emu, True)

            harness.popup(emu)  # PAUSE
            emu.sleep(1)
            self.assert_on(emu, False, "off while paused")
            harness.close_popup(emu)  # RESUME
            emu.sleep(1)
            self.assert_on(emu, True, "on again after resume")

            emu.send("r -80", pause=2)  # the app panics
            self.assertFalse(self.running(emu))
            self.assert_on(emu, False, "off after the crash")

            tap_row(emu, 0)
            emu.sleep(2)
            self.assert_on(emu, True)
            mark = len(emu.uart())
            emu.send("c 1", "c 0", pause=2)  # BUTTON: the app returns
            self.assertFalse(self.running(emu))
            self.assert_on(emu, False, "off after exit")
            self.assertIn("jxdis_algo_ret 5) 0 -> 1", emu.uart()[mark:], "the sensor's last user left")
            emu.sleep(2)
            self.assertIn("VC9202 Stop Later---m0 h0", emu.uart()[mark:], "the driver stopped")
            uart = emu.uart()
            self.assertNotIn("ZEPHYR FATAL ERROR", uart)
            self.assertEqual(uart.count("*** Booting Zephyr"), 1, "guest rebooted")

    def test_dark_panel(self):
        """It keeps measuring while the panel is dark (AOD on hardware): readings still reach the app, no restart."""
        from tests.test_back_e2e import ScreenOffTest
        with harness.installed(["loader", "hr-probe"], name="hr-dark") as (emu, port):
            ScreenOffTest().open(emu, port)
            emu.sleep(2)
            self.assert_on(emu, True)
            emu.poke(BPM, bytes([67]))
            emu.sleep(2)
            self.assertEqual(self.probe(emu)[DELTA], 67)
            dark = []
            ScreenOffTest.blank_and_wake(emu, port, during=lambda: (emu.poke(BPM, bytes([71])), emu.sleep(2.5), dark.append(
                (self.owner(emu), emu.peek(REQUESTS, 1)[0], self.probe(emu)[DELTA]))))
            self.assertEqual(dark, [(1, 1, 71)], "on in the dark, the app gets the reading")
            emu.sleep(1)
            self.assert_on(emu, True, "still the one start after the wake")
            self.assertEqual(self.probe(emu)[DELTA], 71, "no restart on wake")
            self.assertTrue(self.running(emu))


if __name__ == "__main__":
    unittest.main()
