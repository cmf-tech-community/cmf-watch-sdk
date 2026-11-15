import asyncio
import contextlib
import io
import unittest
from unittest import mock

from wp2apps import cards, cli
from wp2apps.cards import Cards, View, Widgets


def quiet():
    return contextlib.redirect_stderr(io.StringIO())


class FakeSession:
    def __init__(self, reply):
        self.reply, self.calls = reply, []

    async def request(self, c1, c2, payload, expect, timeout=6.0):
        self.calls.append((c2, payload, expect))
        return self.reply


class CardsTest(unittest.TestCase):
    def test_encode_set(self):
        got = cards.encode_set(Cards(True, (View(0xB2), Widgets(((0x19, 0), (0x40, 2))))))
        self.assertEqual(got, bytes.fromhex("01 01 02" "02b200" "0102 1900 4002"))

    def test_decode_get_reply(self):
        reply = bytes.fromhex("01 00 03" "02b200" "0101 0700" "02 f000")
        self.assertEqual(cards.decode(reply), Cards(False, (View(0xB2), Widgets(((7, 0),)), View(0xF0))))

    def test_roundtrip(self):
        c = Cards(True, (Widgets(((1, 0), (25, 3), (37, 0), (49, 1))), View(0x73)))
        self.assertEqual(cards.decode(b"\x01" + cards.encode_set(c)[1:]), c)

    def test_decode_rejects_truncated(self):
        for bad in ("01", "010001", "01000102b2", "0100010103 0700", "01000107"):
            with self.subTest(bad), self.assertRaises(ValueError):
                cards.decode(bytes.fromhex(bad))

    def test_limits(self):
        with self.assertRaises(ValueError):
            cards.encode_set(Cards(False, (View(1),) * 9))
        with self.assertRaises(ValueError):
            cards.encode_card(Widgets(((1, 0),) * 5))
        with self.assertRaises(ValueError):
            cards.encode_card(Widgets(()))
        with self.assertRaises(ValueError):
            cards.encode_card(View(0))

    def test_with_apps(self):
        base = Cards(False, (View(0xB2),))
        added = cards.with_apps(base)
        self.assertEqual(added.cards[-1], Widgets(((cards.APPS_WIDGET, 0),)))
        self.assertIs(cards.with_apps(added), added)
        with self.assertRaises(ValueError):
            cards.with_apps(Cards(False, (View(1),) * 8))

    def test_parse_card(self):
        self.assertEqual(cards.parse_card("view:0xB2"), View(0xB2))
        self.assertEqual(cards.parse_card("widgets:0x19,64/2"), Widgets(((0x19, 0), (64, 2))))
        for bad in ("tile:1", "view:", "widgets:", "widgets:1,,2", "view:0xB2,1", "widgets:0x40/2/3"):
            with self.subTest(bad), self.assertRaises(ValueError):
                cards.parse_card(bad)

    def test_cli_set_args(self):
        args = cli.parser().parse_args(["cards", "set", "view:0xB2", "widgets:0x40/2", "--wrap", "--hci-port", "9"])
        self.assertEqual((args.action, args.card, args.wrap, args.hci_port), ("set", ["view:0xB2", "widgets:0x40/2"],
                                                                               True, 9))
        with quiet(), self.assertRaises(SystemExit):
            cli.parser().parse_args(["cards", "set", "view:0xB2"])  # no --hci-port / --address

    def test_cli_set_rejects_bad_cards_before_connecting(self):
        for card in ("view:", "widgets:0x100", "tile:1"):
            with self.subTest(card), mock.patch.object(cli, "with_watch") as watch, quiet():
                self.assertEqual(cli.main(["cards", "set", card, "--hci-port", "9"]), 1)
                watch.assert_not_called()
        with mock.patch.object(cli, "with_watch") as watch, quiet():
            self.assertEqual(cli.main(["cards", "set", *["view:1"] * 9, "--hci-port", "9"]), 1)
            watch.assert_not_called()

    def test_get_and_put_requests(self):
        s = FakeSession(bytes.fromhex("010101 02b200"))
        self.assertEqual(asyncio.run(cards.get(s)), Cards(True, (View(0xB2),)))
        self.assertEqual(s.calls, [(0x906D, b"\0", 0xA06D)])
        ok = FakeSession(b"\0")
        asyncio.run(cards.put(ok, Cards(False, (View(0xF0),))))
        self.assertEqual(ok.calls, [(0x906D, bytes.fromhex("010001 02f000"), 0xA06D)])
        with self.assertRaises(ValueError):
            asyncio.run(cards.put(FakeSession(b"\x01"), Cards(False, ())))


if __name__ == "__main__":
    unittest.main()
