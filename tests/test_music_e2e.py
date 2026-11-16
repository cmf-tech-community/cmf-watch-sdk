"""The stock music tile (`stock::music`, Class::Music) against the stock workout screen's own (sport1_view, its third
tile): the same pixels above the stock page dots, which belong to the stock screen, not to the tile. Once without
phone media ("No music") and once with a synthetic track playing (the ux2sys getters of the phone link, media,
playing and track info pointed at RAM stubs). A tap on play reaches the app. With media, the volume button opens the
stock volume popup (the same pixels, the whole screen); +, − and the crown send volume up/down (ux2sys 0x4E setter
pointed at a recorder), a right swipe closes it and leaves the app on its page. The emulator cannot draw the
slider's alpha images (opaque_slider). Frames in runs/music/:
{stock,sdk,side}-{none,playing,volume}.png (side = stock left, SDK right). Needs target/{loader,ui-gallery}.wp2app."""
import asyncio
import struct
import unittest
from unittest import skipUnless

from tests.e2e import harness
from tests.test_api_e2e import session, unpaused
from tests.test_demo_e2e import save_png
from wp2apps import channel, paths

S = harness.STUB_RAM
ELF = paths.ROOT / "sdk/target/thumbv7em-none-eabi/release/ui-gallery"
OUT = paths.ROOT / "runs" / "music"
RC, PAGE, MUSIC = 0, 4, 10  # STATS, ui-gallery main.rs
DOTS_Y = 425  # the stock screen's page dots sit below
UX2SYS_GETTERS = 0x101F4274  # {u32, getter, size} per property (getter 0x100E082C)
PHONE_LINK, MEDIA_AVAILABLE, MEDIA_PLAYING, MEDIA_CMD, MEDIA_INFO, VOLUME = 0x31, 0x4C, 0x4D, 0x4E, 0x4F, 0x50
VOL, REC = S + 0xFE0, S + 0xFE8  # {max, current}; the recorder's {last command, count}
VOLUME_BTN, PLUS, MINUS = (233, 398), (233, 93), (233, 373)
VOLUME_UP, VOLUME_DOWN = 5, 6
SLIDER_HDR = 0x12803005  # cf 5, 12 x 148
SLIDER_DSC = struct.pack("<2I", SLIDER_HDR, 12 * 148 * 3)
TITLE, ARTIST = b"Test Song", b"Test Artist"  # synthetic


def fake_media(emu: harness.Emu):
    """Phone link and media up, playing, TITLE / ARTIST: the four getters point at Thumb stubs in free stub RAM."""
    copy, one, info = S + 0xF00, S + 0xF20, S + 0xF30
    # copy: r1 = info; 0x82 times *r0++ = *r1++; return 0
    emu.poke(copy, struct.pack("<10HI", 0x4904, 0x2282, 0xF811, 0x3B01, 0xF800, 0x3B01, 0x3A01, 0xD1F9, 0x2000, 0x4770,
                               info))
    emu.poke(one, struct.pack("<4H", 0x2101, 0x6001, 0x2000, 0x4770))  # *r0 = 1; return 0
    emu.poke(info, TITLE.ljust(0x41, b"\0") + ARTIST.ljust(0x41, b"\0"))
    for prop, fn in ((PHONE_LINK, one), (MEDIA_AVAILABLE, one), (MEDIA_PLAYING, one), (MEDIA_INFO, copy)):
        emu.poke(UX2SYS_GETTERS + prop * 12 + 4, struct.pack("<I", fn | 1))


def fake_volume(emu: harness.Emu):
    """Volume 8 of 16 (getter 0x50), and the media command setter (0x4E) records instead of sending."""
    get, rec = S + 0xFB8, S + 0xFC8  # past the track info (0xF30 + 0x82)
    # *(u16 *)r0 = *(u16 *)VOL; return 0
    emu.poke(get, struct.pack("<6HI", 0x4902, 0x880A, 0x8002, 0x2000, 0x4770, 0, VOL))
    # REC[0] = *r0; REC[1]++; return 0
    emu.poke(rec, struct.pack("<8HI", 0x4B03, 0x6802, 0x601A, 0x685A, 0x3201, 0x605A, 0x2000, 0x4770, REC))
    emu.poke(VOL, bytes([16, 8]))
    emu.poke(REC, bytes(8))
    emu.poke(UX2SYS_GETTERS + VOLUME * 12 + 4, struct.pack("<I", get | 1))
    emu.poke(UX2SYS_GETTERS + MEDIA_CMD * 12, struct.pack("<I", rec | 1))


def opaque_slider(emu: harness.Emu):
    """The emulator stops ("fatal or unsupported path") drawing the slider's TRUE_COLOR_ALPHA images (cf 5, 12 x 148,
    the stock popup's too): their descriptors, in the scene's image arrays, become TRUE_COLOR (cf 4), stock and SDK
    alike. The alpha channel then reads as colour, the same garbage on both sides."""
    lo, size = 0x38000000, 0x200000
    mem, n = emu.peek(lo, size), 0
    for at in range(0, size - 12, 4):
        if mem[at:at + 8] == SLIDER_DSC:
            emu.poke(lo + at, struct.pack("<I", SLIDER_HDR & ~0x1F | 4))
            n += 1
    assert n, "no slider image descriptor"


def recorded(emu: harness.Emu) -> tuple[int, int]:
    return struct.unpack("<2I", emu.peek(REC, 8))


def tap(emu: harness.Emu, xy, pause=1.5):
    emu.send(f"t 1 {xy[0]} {xy[1]}", f"t 0 {xy[0]} {xy[1]}", pause=pause)


def stock_frame(media: bool, volume: bool = False) -> tuple[int, int, bytes]:
    with harness.installed(["loader"], name="music-stock") as (emu, _port):
        harness.ready(emu)
        if media:
            fake_media(emu)
            fake_volume(emu)
        emu.call(harness.OPEN_VIEW, 0x64, 0)  # sport1_view; its music tile is the third
        emu.sleep(3)
        for _ in range(2):
            harness.swipe(emu, 400, 233, 60, 233, steps=8, pause=2)
        emu.sleep(2)
        if volume:
            opaque_slider(emu)
            tap(emu, VOLUME_BTN, pause=2.5)
        return emu.frame()


def sdk_frame(test: unittest.TestCase, media: bool, volume: bool = False) -> tuple[int, int, bytes]:
    with harness.installed(["loader", "ui-gallery"], name="music-sdk") as (emu, port):
        async def send(text):
            async with session(port) as s:
                await unpaused(s)
                await channel.send(s, text)

        async def bind():
            async with session(port):
                pass

        asyncio.run(bind())
        harness.open_app(emu, row=0)

        def stats():
            at = harness.u32(emu, S + harness.APP_BLOCK_OFF) + harness.symbol(ELF, "STATS")
            return struct.unpack("<11i", emu.peek(at, 44))

        emu.sleep(1)
        asyncio.run(send(b"!music"))
        for _ in range(40):
            if stats()[PAGE] == 2:
                break
            emu.sleep(0.25)
        if media:
            fake_media(emu)  # after the session: the BLE link stays real
            fake_volume(emu)
        emu.sleep(2.5)  # a refresh a second
        test.assertEqual(stats()[RC], 0, stats())
        if volume:
            tile = emu.frame()[2]
            opaque_slider(emu)
            tap(emu, VOLUME_BTN, pause=2.5)
            test.assertEqual(stats()[MUSIC], 3, "the volume tap did not reach the app")
            frame = emu.frame()
            test.assertNotEqual(frame[2], tile, "no popup")
            tap(emu, PLUS)
            test.assertEqual(recorded(emu), (VOLUME_UP, 1))
            tap(emu, MINUS)
            test.assertEqual(recorded(emu), (VOLUME_DOWN, 2))
            emu.send(*["r 80"] * 6, pause=0.5)  # six knob steps: one command
            test.assertEqual(recorded(emu)[1], 3, "the crown")
            harness.swipe(emu, 60, 233, 400, 233, steps=8, pause=2)  # right: closes the popup only
            emu.sleep(1.5)
            test.assertEqual(emu.peek(S + 6, 1), b"\x00", "the app stopped")
            test.assertEqual(stats()[PAGE], 2, "the pager moved")
            w, h, after = emu.frame()
            save_png(OUT / "sdk-volume-closed.png", w, h, after)
            test.assertEqual(after[:DOTS_Y * w * 3], tile[:DOTS_Y * w * 3], "the popup did not close")
            return frame
        frame = emu.frame()
        if not media:
            emu.send("t 1 233 233", "t 0 233 233", pause=1.5)  # play
            test.assertEqual(stats()[MUSIC], 0, "the play tap did not reach the app")
            test.assertEqual(emu.peek(S + 6, 1), b"\x00", "the app stopped")
        return frame


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class MusicTest(unittest.TestCase):
    def compare(self, media: bool, volume: bool = False):
        OUT.mkdir(parents=True, exist_ok=True)
        tag = "volume" if volume else "playing" if media else "none"
        w, h, sdk = sdk_frame(self, media, volume)
        save_png(OUT / f"sdk-{tag}.png", w, h, sdk)
        w2, h2, stock = stock_frame(media, volume)
        save_png(OUT / f"stock-{tag}.png", w2, h2, stock)
        side = b"".join(stock[y * w * 3:(y + 1) * w * 3] + sdk[y * w * 3:(y + 1) * w * 3] for y in range(h))
        save_png(OUT / f"side-{tag}.png", 2 * w, h, side)
        self.assertEqual((w, h), (w2, h2))
        rows = h if volume else DOTS_Y  # the popup covers the dots
        diff = sum(1 for i in range(0, rows * w * 3, 3) if stock[i:i + 3] != sdk[i:i + 3])
        self.assertEqual(diff, 0, f"{diff} pixels differ above y {rows} (runs/music/side-{tag}.png)")

    def test_no_music(self):
        self.compare(False)

    def test_playing(self):
        self.compare(True)

    def test_volume(self):
        self.compare(True, volume=True)


if __name__ == "__main__":
    unittest.main()
