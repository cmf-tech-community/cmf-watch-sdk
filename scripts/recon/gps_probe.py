"""GPS recon in the emulator (docs/recon.md, "GPS"); synthetic coordinates only, frames in runs/gps/.

  nix-shell -p gdb llvm --run 'uv run --locked python -m scripts.recon.gps_probe modes|workout'

modes: property 0x63 set to 1, 4, 5 and 0 through the ux2sys setter while the emulator feeds NMEA (`g 1 lat lon`);
prints the stock GPS state and the parsed fix after each step. workout: the sport list, Outdoor run's GO screen and
its run screen, then its end; prints the GPS state on each. Needs target/app-wp2.bin (scripts/build-fw)."""
import re
import struct
import sys

from tests.e2e import harness
from tests.test_demo_e2e import save_png
from wp2apps import paths

OUT = paths.ROOT / "runs" / "gps"
UX2SYS_SET = 0x100E07B9
PROP_GPS_MODE = 0x63
INFO = 0x38016C40  # GGA/GSV/speed cells the NMEA parser 0x10197D40 fills (docs/recon.md, "GPS")
STATE = 0x380443F4  # gps_service state: [0] epo, [1] mode, [2] applied mode, [3] special, [4] recovery
DATA = harness.STUB_RAM + 0xF00
SPORT_LIST = 0x6A
LAT, LON = 12_345_678, 98_765_432  # synthetic, e6 (the emulator's input unit)
LOG = re.compile(r"gps|GGA|PAIR|ag3352|epo|location", re.I)


def state(emu, tag, since):
    s = emu.peek(STATE, 5)
    i = emu.peek(INFO, 0x68)
    lat, lon = struct.unpack_from("<dd", i, 0x38)
    hdop, quality, sats, ttff, speed = struct.unpack_from("<fiiIf", i, 0x48)
    print(f"[{tag}] state={list(s)} lat={lat:.6f} lon={lon:.6f} f48={hdop:.2f} quality={quality} sats={sats} "
          f"ttff={ttff} f5c={speed:.2f} snr={list(i[:8])} view={harness.focused(emu)}", flush=True)
    for line in emu.uart()[since:].splitlines():
        if LOG.search(line):
            print("   ", line.strip()[:160], flush=True)
    return len(emu.uart())


def set_mode(emu, mode):
    emu.poke(DATA, struct.pack("<I", mode))
    emu.call(UX2SYS_SET, PROP_GPS_MODE, DATA, thread=b"ui_s")


def shot(emu, name):
    emu.sleep(0.5)
    w, h, rgb = emu.frame()
    save_png(OUT / f"{name}.png", w, h, rgb)


def tap(emu, xy):
    emu.send(f"t 1 {xy[0]} {xy[1]}", f"t 0 {xy[0]} {xy[1]}", pause=2)


def modes(emu, mark):
    emu.send(f"g 1 {LAT} {LON}")
    for mode in (1, 0, 4, 0, 5, 0):
        set_mode(emu, mode)
        emu.sleep(6)
        mark = state(emu, f"mode {mode}", mark)
    emu.send(f"g 0 {LAT} {LON}")


def workout(emu, mark):
    emu.send(f"g 1 {LAT} {LON}")
    emu.call(harness.OPEN_VIEW, SPORT_LIST, 0)
    emu.sleep(3)
    mark = state(emu, "sport list", mark)
    shot(emu, "w1-list")
    tap(emu, (233, 200))  # first sport
    emu.sleep(4)
    mark = state(emu, "first sport", mark)
    shot(emu, "w2-go")
    tap(emu, (233, 243))  # GO
    emu.sleep(10)
    mark = state(emu, "running", mark)
    shot(emu, "w3-run")
    emu.send("c 1", "c 0", pause=2)  # the button: pause / end menu
    mark = state(emu, "button", mark)
    shot(emu, "w4-button")
    emu.send("c 1", "c 0", pause=4)
    mark = state(emu, "button 2", mark)
    shot(emu, "w5-button2")


def main(what):
    OUT.mkdir(parents=True, exist_ok=True)
    with harness.Emu("gps-probe", nor=harness.patched_nor()) as emu:
        emu.wait_uart("launcher enter", 90)
        emu.keep_awake()
        emu.sleep(5)
        mark = state(emu, "boot", 0)
        {"modes": modes, "workout": workout}[what](emu, mark)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "modes")
