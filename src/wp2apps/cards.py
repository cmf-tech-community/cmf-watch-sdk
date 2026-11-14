"""The cards left of the watch face (docs/recon.md, "Widget cards"): stock command 0x906D reads and writes the
list, the reply is 0xA06D. Both are sealed with the session key like any stock command.

Request: op 0 = get; op 1 = set, then u8 wrap (pages wrap around), u8 card count (<= 8), the cards.
Get reply: u8 1, u8 wrap, u8 count, the cards. Set reply: u8 0. A card is `02 <u16 LE view id>` (a whole stock
view) or `01 <u8 n (<= 4)> n x <u8 widget id, u8 param>` (a grid of system widgets)."""
from dataclasses import dataclass

from .link import SYSTEM

CMD = 0x906D
REPLY = 0xA06D
GET, SET = 0, 1
WIDGETS, VIEW = 1, 2
MAX_CARDS = 8
MAX_WIDGETS = 4
# the stub's "Apps" widget (fw/wp2-stub/src/card.rs): opens the loader menu, needs the stub with a widget entry
APPS_WIDGET = 0x40


@dataclass(frozen=True)
class Widgets:
    widgets: tuple[tuple[int, int], ...]  # (widget id, param)


@dataclass(frozen=True)
class View:
    view: int


Card = Widgets | View


@dataclass(frozen=True)
class Cards:
    wrap: bool
    cards: tuple[Card, ...]


def encode_card(card: Card) -> bytes:
    if isinstance(card, View):
        if not 0 < card.view <= 0xFFFF:
            raise ValueError(f"bad view id {card.view}")
        return bytes([VIEW]) + card.view.to_bytes(2, "little")
    if not 0 < len(card.widgets) <= MAX_WIDGETS:
        raise ValueError(f"a widget card holds 1..{MAX_WIDGETS} widgets")
    if any(not (0 <= w <= 0xFF and 0 <= p <= 0xFF) for w, p in card.widgets):
        raise ValueError("widget ids and params are bytes")
    return bytes([WIDGETS, len(card.widgets)]) + b"".join(bytes(pair) for pair in card.widgets)


def encode_set(cards: Cards) -> bytes:
    if len(cards.cards) > MAX_CARDS:
        raise ValueError(f"at most {MAX_CARDS} cards")
    return bytes([SET, int(cards.wrap), len(cards.cards)]) + b"".join(encode_card(c) for c in cards.cards)


def decode(reply: bytes) -> Cards:
    """A get reply: the cards after the leading byte, in the request's encoding."""
    if len(reply) < 3:
        raise ValueError(f"short cards reply {reply.hex()}")
    wrap, count, at, cards = bool(reply[1]), reply[2], 3, []
    for _ in range(count):
        kind = reply[at] if at < len(reply) else None
        if kind == VIEW and at + 3 <= len(reply):
            cards.append(View(int.from_bytes(reply[at + 1:at + 3], "little")))
            at += 3
        elif kind == WIDGETS and at + 2 <= len(reply) and at + 2 + 2 * reply[at + 1] <= len(reply):
            n = reply[at + 1]
            cards.append(Widgets(tuple((reply[at + 2 + 2 * i], reply[at + 3 + 2 * i]) for i in range(n))))
            at += 2 + 2 * n
        else:
            raise ValueError(f"bad card at byte {at} of {reply.hex()}")
    return Cards(wrap, tuple(cards))


def with_apps(cards: Cards) -> Cards:
    """The list with a card holding the Apps widget at the end, unless one has it already."""
    if any(isinstance(c, Widgets) and any(w == APPS_WIDGET for w, _ in c.widgets) for c in cards.cards):
        return cards
    if len(cards.cards) >= MAX_CARDS:
        raise ValueError(f"all {MAX_CARDS} cards are taken")
    return Cards(cards.wrap, cards.cards + (Widgets(((APPS_WIDGET, 0),)),))


def parse_card(spec: str) -> Card:
    """`view:0xB2` or `widgets:0x19,0x40/2` (id[/param], comma separated)."""
    kind, _, rest = spec.partition(":")
    if kind == "view":
        return View(int(rest, 0))
    if kind == "widgets":
        pairs = []
        for item in rest.split(","):
            wid, _, param = item.partition("/")
            pairs.append((int(wid, 0), int(param, 0) if param else 0))
        return Widgets(tuple(pairs))
    raise ValueError(f"bad card {spec!r}: view:ID or widgets:ID[/PARAM],...")


def to_json(cards: Cards) -> dict:
    return {"wrap": cards.wrap, "cards": [
        {"view": c.view} if isinstance(c, View) else {"widgets": [{"id": w, "param": p} for w, p in c.widgets]}
        for c in cards.cards]}


async def get(s) -> Cards:
    return decode(await s.request(SYSTEM, CMD, bytes([GET]), REPLY))


async def put(s, cards: Cards) -> None:
    reply = await s.request(SYSTEM, CMD, encode_set(cards), REPLY)
    if reply[:1] != b"\0":
        raise ValueError(f"cards set refused: {reply.hex()}")
