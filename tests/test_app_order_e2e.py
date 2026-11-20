"""Apps menu order from the phone (docs/app-order.md): a loader package with a WP2ORDER table puts the named apps
first, skips names that are not installed, and leaves the rest in slot order (loader menu.rs)."""
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import package, paths


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd and gdb)")
class AppOrderTest(unittest.TestCase):
    def test_order_from_loader_package(self):
        target = paths.ROOT / "target"
        ordered = package.set_order((target / "loader.wp2app").read_bytes(), ["not-installed", "vibration-lab"])
        (target / "loader-order.wp2app").write_bytes(ordered)
        # apps in slots 0, 1, 2; the order arrives last, with the menu closed, as from the phone
        with harness.installed(["hello", "api-demo", "vibration-lab", "loader-order"], name="app-order") as (emu, _):
            harness.open_menu(emu)
            save_png(emu.run / "menu.png", *emu.frame())
            harness.open_app(emu, 0)
            self.assertEqual(emu.peek(harness.STUB_RAM + 6, 1), b"\x02", "row 0 is not vibration-lab (slot 2)")
