---
lang: uz
direction: ltr
source: docs/source/crypto/sm_perf_baseline_comparison.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fb1b094c1b081620abe8f47079a8d94e774c1eea445784856a6d116c6a1a993d
source_last_modified: "2025-12-29T18:16:35.944257+00:00"
translation_last_reviewed: 2026-02-07
---

### SM Performance Snapshot (Apple Silicon)

Benchmarks compare scalar, runtime auto-detection, and forced NEON runs (medians, µs).

| Benchmark | Scalar | Auto | Neon-Force | Auto vs Scalar | Neon vs Scalar | Neon vs Auto |
|-----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    32.83 |  32.91 |      32.82 |      +0.23% |       -0.03% |      -0.25% |
| sm2_vs_ed25519_sign/sm2_sign |   302.19 | 301.51 |     301.98 |      -0.23% |       -0.07% |      +0.16% |
| sm2_vs_ed25519_verify/verify/ed25519 |    35.43 |  35.80 |      35.47 |      +1.04% |       +0.12% |      -0.91% |
| sm2_vs_ed25519_verify/verify/sm2 |   270.66 | 269.30 |     269.28 |      -0.50% |       -0.51% |      -0.01% |
| sm3_vs_sha256_hash/sha256_hash |    11.59 |  11.61 |      11.62 |      +0.18% |       +0.24% |      +0.06% |
| sm3_vs_sha256_hash/sm3_hash |    11.34 |  57.95 |      57.94 |    +410.90% |     +410.86% |      -0.01% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1.89 |   1.89 |       1.90 |      -0.04% |       +0.47% |      +0.51% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.02 |  16.05 |      16.06 |      +0.20% |       +0.30% |      +0.10% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.83 |   1.82 |       1.82 |      -0.17% |       -0.18% |      -0.01% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    15.97 |  15.96 |      15.94 |      -0.01% |       -0.16% |      -0.14% |

Metadata: scalar={'cpu': 'm3-pro-local', 'target_arch': 'aarch64', 'target_os': 'macos'}, auto={'cpu': 'm3-pro-local', 'target_arch': 'aarch64', 'target_os': 'macos'}, neon-force={'cpu': 'm3-pro-local', 'target_arch': 'aarch64', 'target_os': 'macos'}.

### SM Performance Snapshot (x86_64 Rosetta, Apple M3 Pro)

Benchmarks compare scalar, runtime auto-detection, and forced NEON runs (medians, µs) captured under Rosetta.

| Benchmark | Scalar | Auto | Neon-Force | Auto vs Scalar | Neon vs Scalar | Neon vs Auto |
|-----------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57.43 |  57.12 |      55.77 |          -0.53% |         -2.88% |        -2.36% |
| sm2_vs_ed25519_sign/sm2_sign |   572.76 | 568.71 |     557.83 |          -0.71% |         -2.61% |        -1.91% |
| sm2_vs_ed25519_verify/verify/ed25519 |    69.03 |  68.42 |      66.28 |          -0.88% |         -3.97% |        -3.12% |
| sm2_vs_ed25519_verify/verify/sm2 |   521.73 | 514.50 |     502.17 |          -1.38% |         -3.75% |        -2.40% |
| sm3_vs_sha256_hash/sha256_hash |    16.78 |  16.58 |      16.16 |          -1.19% |         -3.69% |        -2.52% |
| sm3_vs_sha256_hash/sm3_hash |    15.78 |  15.51 |      15.04 |          -1.71% |         -4.69% |        -3.03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1.96 |   1.97 |       1.97 |           0.39% |          0.16% |        -0.23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16.26 |  16.38 |      16.26 |           0.72% |         -0.01% |        -0.72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1.96 |   2.00 |       1.93 |           2.23% |         -1.14% |        -3.30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16.60 |  16.58 |      16.15 |          -0.10% |         -2.66% |        -2.57% |

Metadata: scalar={'cpu': 'm3-pro-rosetta', 'target_arch': 'x86_64', 'target_os': 'macos'}, auto={'cpu': 'm3-pro-rosetta', 'target_arch': 'x86_64', 'target_os': 'macos'}, neon-force={'cpu': 'm3-pro-rosetta', 'target_arch': 'x86_64', 'target_os': 'macos'}.
