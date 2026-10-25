# iroha_petal

Reference implementation of **Petal Stream** format 3, Iroha's animated optical transport: a storm of
3,376 sakura petals on concentric bands (white inside the SORA `天`, pink outside, over a faint `天`),
inside three dotted rings, framed by four sakura-blossom finders. Which petals are lit carries four
field lanes; how the lit ones are turned (0°, 45°, 90° or 135°) carries four more turn lanes. A phone
camera reads the animation and reassembles the payload: in simulation a 12 KB KAGEMUSHA Payment arrives
in 1.65 s at the default 12 frames a second on a sharp 720p camera and 1.8 s on an older one (3.0 s on
cameras too soft for the turns; animated QR takes 4.3 s at 12 fps and 6.4 s at its 8 fps). The wire
format, geometry and receiver rules are specified in [`specs/petal_stream.md`](../../specs/petal_stream.md).

The crate is `std`-only and has no runtime dependencies.

| Module | Purpose |
| --- | --- |
| `rs`, `crc`, `prng` | GF(256) Reed–Solomon with erasures, CRC-32C, xorshift32 |
| `layout` | normative geometry: the 34 petal bands and their four sectors, the `天` mask, rings, finders |
| `lanes`, `fountain`, `stream` | the lane codewords (`D` and `F0`–`F3`), the GF(2) fountain code, `StreamEncoder` / `StreamAssembler` |
| `turns` | the turn lanes (`T0`–`T3`): carrier order, turn codewords and their repair atoms, and the turn reader |
| `render` | software renderer for frames (SDKs also expose a vector draw list) |
| `geometry`, `locate`, `decode`, `session` | camera luma → finders (three are enough) → pose (with lens distortion) → lanes; `ScanSession` tracks the pose between frames, merges the captures of one shown frame and reads each turn lane once |
| `sim`, `qualify` | deterministic camera simulator and end-to-end stream trials |

```text
cargo test -p iroha_petal                                   # unit tests + golden fixtures + golden captures
cargo run --release -p iroha_petal --example qualify -- 40  # per-frame decode matrix over camera conditions
cargo run --release -p iroha_petal --example stress -- 24   # light, motion and hidden-corner stress
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12   # end to end, the largest KAGEMUSHA Payment at 12 fps
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 modern --camera-fps 10 --readout-ms 20   # slow decoder, rolling shutter
cargo run --release -p iroha_petal --example decode_cost                 # single-threaded decode time
cargo run -p iroha_petal --example gen_fixtures -- fixtures/petal/petal_stream_v3.json
cargo run --release -p iroha_petal --example gen_captures -- fixtures/petal/petal_captures_v3.json
```

`fixtures/petal/*_v3.json` are the cross-SDK conformance suite; `tests/fixtures.rs` and `tests/captures.rs`
fail if they drift from this implementation. The `iroha offline petal` commands in `iroha_cli` are thin wrappers.
