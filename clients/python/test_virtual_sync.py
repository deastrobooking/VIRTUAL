"""Checks the Python client against the Rust encoder's golden datagrams:
python3 clients/python/test_virtual_sync.py"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import virtual_sync as vs  # noqa: E402

data = json.loads((pathlib.Path(__file__).parent.parent / "test-vectors.json").read_text())
key = vs.derive_key(data["passphrase"])
vectors = {v["name"]: bytes.fromhex(v["hex"]) for v in data["vectors"]}

assert vs.siphash24((0x0706050403020100, 0x0F0E0D0C0B0A0908), bytes(range(15))) == 0xA129CA6149BE45E5
announce = vs.parse(vectors["announce"], key)
assert (announce.name, announce.streams, announce.zones, announce.port) == ("Main room", 15, 4, 50000)
clock = vs.parse(vectors["clock"], key)
assert (clock.leader_us, clock.beat, clock.bpm, clock.beats_per_bar, clock.playing) == (5_000_000, 33.5, 128.0, 4, True)
controls = vs.parse(vectors["controls"], key)
assert controls.values == [(vs.control_id("deck.0.level"), 0.75), (vs.control_id("master.blackout"), 1.0)]
audio = vs.parse(vectors["audio"], key)
assert audio.bass == 0.875 and audio.bands[7] == 0.875
assert vs.parse(vectors["zones"], key).colors == [(255, 0, 0), (0, 255, 0), (0, 0, 255), (10, 20, 30)]
assert vs.parse(vectors["pong"], key).leader_us == 99_999
assert vs.build_hello("Strip", vs.STREAM_CLOCK | vs.STREAM_ZONES, 0x12345678, 101, key) == vectors["hello"]
assert vs.parse(vectors["clock_unsigned"], None).bpm == 128.0
for bad_key, packet in ((None, vectors["clock"]), (vs.derive_key("other"), vectors["clock"]), (key, vectors["clock_unsigned"])):
    try:
        vs.parse(packet, bad_key)
        raise AssertionError("accepted a bad signature")
    except vs.SyncError:
        pass
tampered = bytearray(vectors["clock"]); tampered[25] ^= 1
try:
    vs.parse(bytes(tampered), key)
    raise AssertionError("accepted a tampered datagram")
except vs.SyncError:
    pass
window = vs.ReplayWindow()
assert window.accept(7, 10) and not window.accept(7, 10) and window.accept(7, 12) and window.accept(7, 11)
print("virtual_sync.py: all checks passed")
