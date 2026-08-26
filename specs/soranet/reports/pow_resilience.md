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

Across both phases the puzzle difficulty remains at the single configured PoW
difficulty. The first-release schema has no adaptive enablement, so an external
issuer and verifier cannot drift during volumetric DoS attempts. The retired
`adaptive` key is rejected as unknown rather than retained as an inert schema.
