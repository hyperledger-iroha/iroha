---
lang: az
direction: ltr
source: docs/source/norito_crc64_parity_bench.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 41dc0836024322b773db070431b6659889dafcaa3619d2441cd6d0c89d9049ea
source_last_modified: "2026-01-14T17:51:31.484344+00:00"
translation_last_reviewed: 2026-02-07
---

# Norito CRC64: Parity and Bench Guide

This guide shows how to verify CRC64 parity and benchmark Norito’s CRC64 and encode/decode performance on your hardware.

Norito computes a CRC64-XZ checksum (ECMA polynomial `0x42F0E1EBA9EA3693`, reflected, init/xor all ones)
for each payload using the `crc64fast` crate. The `hardware_crc64`
and `crc64_fallback` functions are identical, providing a consistent implementation across platforms.

## 1) Quick Parity Checks

- Run unit/integration parity tests for Norito:

```bash
cargo test -p norito --test crc64
cargo test -p norito --test crc64_prop
```

- Force acceleration on a capable CPU (build‑time toggles for maximum speed):

```
# x86_64 (developer build flag for local benches; not node runtime configuration)
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64_prop

# aarch64 (developer build flag for local benches; not node runtime configuration)
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64_prop
```

Notes:
- To iterate faster, run a single test: `cargo test -p norito crc64_prop::random_lengths_parity_small -- --nocapture`

## 2) Ad‑Hoc Parity (Script Snippet)

```rust
use norito::{crc64_fallback, hardware_crc64};

fn main() {
    let mut ok = true;
    let sizes = [0usize, 1, 7, 8, 15, 16, 31, 32, 63, 64, 511, 512, 4095, 4096];
    for &n in &sizes {
        let mut buf = vec![0u8; n];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for b in &mut buf { x ^= x << 7; x ^= x >> 9; *b = (x as u8) ^ 0xA5; }
        let hw = hardware_crc64(&buf);
        let fb = crc64_fallback(&buf);
        if hw != fb { eprintln!("mismatch at n={n}: hw={hw:016X} fb={fb:016X}"); ok = false; break; }
    }
    if ok { println!("parity OK across sizes: {:?}", sizes); }
}
```

## 3) CRC64 Throughput Benchmarks

Run the Criterion benchmark:


```bash
cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30

# aarch64 (Apple M‑series / ARM servers):
RUSTFLAGS='-C target-feature=+neon,+aes' \
  cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30

```

What to expect:
- Benchmark numbers vary by CPU, frequency, and memory bandwidth.

## 4) End‑to‑End Encode/Decode Benches

The `crates/norito/benches/codec.rs` benchmark compares Norito vs SCALE and bincode for encode and decode. It also includes compressed variants.

Recommended invocation:

```bash
cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30


# With acceleration turned on (CRC becomes less dominant):
# x86_64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30
# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30

```

Interpreting results:
- Norito includes a header, schema hash, and CRC64; SCALE and bincode do not. Expect Norito to be competitive for larger payloads and slightly slower/smaller for very small ones.
- Compression benches (zstd) measure end‑to‑end time including compression; pick levels based on throughput needs (Norito default uses a “fast” level).

## 5) Troubleshooting & Tips

- Long build times: iterate on a single crate/test: `cargo test -p norito --lib` or `--test crc64`.
- Network/offline: use `--offline` to avoid pulling new dependencies.
- Apple Silicon: ensure your target is `aarch64-apple-darwin`.
- Determinism: the checksum implementation is stable across platforms; parity tests enforce this in CI.

---

If you capture benchmark outputs on your machine (e.g., Criterion’s reports), consider pasting representative excerpts here for
future reference, noting CPU model, OS, and compiler/toolchain versions.
