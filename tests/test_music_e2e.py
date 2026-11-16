"""The stock music tile (`stock::music`, Class::Music) against the stock workout screen's own (sport1_view, its third
tile): the same pixels above the stock page dots, which belong to the stock screen, not to the tile. Once without
phone media ("No music") and once with a synthetic track playing (the ux2sys getters of the phone link, media,
playing and track info pointed at RAM stubs). A tap on play reaches the app. Frames in runs/music/: {stock,sdk,side}
-{none,playing}.png (side = stock left, SDK right). Needs target/{loader,ui-gallery}.wp2app."""
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
PHONE_LINK, MEDIA_AVAILABLE, MEDIA_PLAYING, MEDIA_INFO = 0x31, 0x4C, 0x4D, 0x4F
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


def stock_frame(media: bool) -> tuple[int, int, bytes]:
    with harness.installed(["loader"], name="music-stock") as (emu, _port):
        harness.ready(emu)
        if media:
            fake_media(emu)
        emu.call(harness.OPEN_VIEW, 0x64, 0)  # sport1_view; its music tile is the third
        emu.sleep(3)
        for _ in range(2):
            harness.swipe(emu, 400, 233, 60, 233, steps=8, pause=2)
        emu.sleep(2)
        return emu.frame()


def sdk_frame(test: unittest.TestCase, media: bool) -> tuple[int, int, bytes]:
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
        emu.sleep(2.5)  # a refresh a second
        test.assertEqual(stats()[RC], 0, stats())
        frame = emu.frame()
        if not media:
            emu.send("t 1 233 233", "t 0 233 233", pause=1.5)  # play
            test.assertEqual(stats()[MUSIC], 0, "the play tap did not reach the app")
            test.assertEqual(emu.peek(S + 6, 1), b"\x00", "the app stopped")
        return frame


@skipUnless(harness.E2E, "set WP2_E2E=1 (needs the emulator, netsimd, gdb and llvm-nm)")
class MusicTest(unittest.TestCase):
    def compare(self, media: bool):
        OUT.mkdir(parents=True, exist_ok=True)
        tag = "playing" if media else "none"
        w, h, sdk = sdk_frame(self, media)
        save_png(OUT / f"sdk-{tag}.png", w, h, sdk)
        w2, h2, stock = stock_frame(media)
        save_png(OUT / f"stock-{tag}.png", w2, h2, stock)
        side = b"".join(stock[y * w * 3:(y + 1) * w * 3] + sdk[y * w * 3:(y + 1) * w * 3] for y in range(h))
        save_png(OUT / f"side-{tag}.png", 2 * w, h, side)
        self.assertEqual((w, h), (w2, h2))
        diff = sum(1 for i in range(0, DOTS_Y * w * 3, 3) if stock[i:i + 3] != sdk[i:i + 3])
        self.assertEqual(diff, 0, f"{diff} pixels differ above y {DOTS_Y} (runs/music/side-{tag}.png)")

    def test_no_music(self):
        self.compare(False)

    def test_playing(self):
        self.compare(True)


if __name__ == "__main__":
    unittest.main()
