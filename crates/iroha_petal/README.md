# iroha_petal

Reference implementation of **Petal Stream**, Iroha's animated optical transport: a storm of 3,376
sakura petals on concentric bands (white inside the SORA `天`, pink outside, over a faint `天`),
inside three dotted rings, framed by four sakura-blossom finders. A phone camera reads the animation
and reassembles the payload: a 12 KB KAGEMUSHA Payment in 4.5 seconds at the default 8 frames a second
(animated QR at the same rate takes 6.4 s). The wire format, geometry and receiver rules are specified in
[`specs/petal_stream.md`](../../specs/petal_stream.md).

The crate is `std`-only and has no runtime dependencies.

| Module | Purpose |
| --- | --- |
| `rs`, `crc`, `prng` | GF(256) Reed–Solomon with erasures, CRC-32C, xorshift32 |
| `layout` | normative geometry: the 34 petal bands and their four sectors, the `天` mask, rings, finders |
| `lanes`, `fountain`, `stream` | the five lane codewords (`D` and `F0`–`F3`), the GF(2) fountain code, `StreamEncoder` / `StreamAssembler` |
| `render` | software renderer for frames (SDKs also expose a vector draw list) |
| `geometry`, `locate`, `decode`, `session` | camera luma → finders (three are enough) → pose (with lens distortion) → lanes; `ScanSession` tracks the pose between frames and merges the captures of one shown frame |
| `sim`, `qualify` | deterministic camera simulator and end-to-end stream trials |

```text
cargo test -p iroha_petal                                   # unit tests + golden fixtures + golden captures
cargo run --release -p iroha_petal --example qualify -- 40  # per-frame decode matrix over camera conditions
cargo run --release -p iroha_petal --example stress -- 24   # light, motion and hidden-corner stress
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 8    # end to end, the largest KAGEMUSHA Payment at 8 fps
cargo run --release -p iroha_petal --example stream_sim -- 8 12182 12 modern --camera-fps 10 --readout-ms 20   # slow decoder, rolling shutter
cargo run --release -p iroha_petal --example decode_cost                 # single-threaded decode time
cargo run -p iroha_petal --example gen_fixtures -- fixtures/petal/petal_stream_v2.json
cargo run --release -p iroha_petal --example gen_captures -- fixtures/petal/petal_captures_v2.json
```

`fixtures/petal/*_v2.json` are the cross-SDK conformance suite; `tests/fixtures.rs` and `tests/captures.rs`
fail if they drift from this implementation. The `iroha offline petal` commands in `iroha_cli` are thin wrappers.
