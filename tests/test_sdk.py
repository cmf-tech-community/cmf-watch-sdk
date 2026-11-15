from pathlib import Path
import shutil
import struct
import subprocess
import unittest

from wp2apps import icon, package

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which("cargo") and shutil.which("llvm-objcopy"), "needs cargo and llvm-objcopy")
class SdkTest(unittest.TestCase):
    def test_examples_build_and_pack(self):
        for name in ("hello", "vibration-lab"):
            with self.subTest(name):
                subprocess.run([str(ROOT / "scripts" / "build-app"), name, "1"], check=True, capture_output=True)
                pkg = package.parse((ROOT / "target" / f"{name}.wp2app").read_bytes())
                lab = name == "vibration-lab"  # has icon.svg: the descriptor sits in front of the code
                self.assertEqual((pkg.name, icon.parse(pkg.image) is not None), (name, lab))
                self.assertTrue(package.features(pkg.image))  # both declare app_features!: 8 bytes after the icon
                self.assertEqual(pkg.entry, (icon.DESC.size + icon.SIDE ** 2 * 3 if lab else 0) + 8)
                self.assertTrue(pkg.relocs)
                for off in pkg.relocs:
                    self.assertLess(struct.unpack_from("<I", pkg.image, off)[0] & ~1, len(pkg.image))
                self.assertTrue(0 < len(pkg.image) - pkg.entry < 16 * 1024)
                elf = ROOT / f"sdk/target/thumbv7em-none-eabi/release/{name}"
                self.assertEqual(package.check_elf(pkg.image, elf.read_bytes()), pkg.entry)


if __name__ == "__main__":
    unittest.main()
