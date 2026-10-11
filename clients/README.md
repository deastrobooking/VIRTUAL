# VIRTUAL Sync clients

Receive a VIRTUAL show's tempo, beat phase, parameter values, audio bands and
light zones on other devices. The protocol is documented in
[docs/NETWORK_SYNC.md](../docs/NETWORK_SYNC.md).

| Client | For | Test |
|---|---|---|
| [`c/virtual_sync.h`](c/virtual_sync.h) | ESP32, Arduino with Wi-Fi, Raspberry Pi, any C/C++ | `cc -std=c99 -I c c/test/test_virtual_sync.c -lm && ./a.out` |
| [`python/virtual_sync.py`](python/virtual_sync.py) | Raspberry Pi, laptops, lighting software | `python3 python/test_virtual_sync.py` |
| [`arduino/VirtualSyncLeds`](arduino/VirtualSyncLeds/VirtualSyncLeds.ino) | ESP32 + WS2812 strip (FastLED) | example sketch |

Both libraries are tested against the same golden datagrams produced by the
Rust encoder (`test-vectors.json`, `c/test/vectors.h`). Regenerate them with
`cargo run -p virtual-sync --example vectors`; a Rust test fails if they drift.
