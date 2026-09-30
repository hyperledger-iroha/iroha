# Sumeragi size gate before removal

These exact working-tree originals retain the source-size policy removed on
2026-09-30 at the user's explicit request. `capture.json` records their original
paths, lengths, SHA-256 hashes and the checkout HEAD at capture.

The final simulator run before removal passed 406 tests with two existing
stress/report exclusions. Its separate source-size test rejected 9,342 lines
against the former 8,000-line limit. That limit and its planned module shares are
historical policy, not current qualification criteria.

The active crate retains specification traceability, quorum checks, deterministic
simulator scenarios and mutation testing. Whole-node/network qualification remains
separate from these component results.
