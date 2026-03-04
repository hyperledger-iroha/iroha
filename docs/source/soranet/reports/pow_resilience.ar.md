---
lang: ar
direction: rtl
source: docs/source/soranet/reports/pow_resilience.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c25bb7596d2225d8d69ec704242f9db8e98a4ef7f3dd631693e7598dc740d35
source_last_modified: "2026-01-03T18:08:01.714333+00:00"
translation_last_reviewed: 2026-01-30
---

# PoW Resilience Soak Report

The `volumetric_dos_soak_preserves_puzzle_and_latency_slo` test in
`tools/soranet-relay/tests/adaptive_and_puzzle.rs` exercises the SNNet-6a
Argon2 gate under sustained load. The harness drives the `DoSControls`
implementation with a 6-request burst window and a 300 ms handshake SLO, using
the production puzzle policy (4 MiB memory, single lane, time cost 1) at
difficulty 6.

| Phase | Attempts | Latency Samples (ms) | Cooldown | Notes |
|-------|----------|----------------------|----------|-------|
| Burst soak | 6 | 190, 190, 190, 190, 190, 190 | 4 s remote cooldown | Tickets are minted and verified (`puzzle::mint_ticket`/`verify`) while staying within the 300 ms SLO. |
| Slowloris penalty | 3 | 340, 340, 340 | 5 s slowloris penalty | Exceeding the SLO three times triggers the configured slowloris penalty and registers an active cooldown in relay metrics. |

Across both phases the puzzle difficulty mirrors the PoW difficulty, ensuring
that Argon2 tickets stay aligned with adaptive policy decisions even under
volumetric DoS attempts.
