<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraFS Orchestrator Fixtures

The files under this directory capture the deterministic multi-provider fetch
scenario used by every SDK parity harness (`SF-6d` roadmap item). Each fixture
bundle exposes the canonical chunk plan, provider metadata, telemetry snapshot,
and orchestrator options so Rust/JS/Swift/Python tests exercise the exact same
inputs without regenerating them ad hoc.

## Layout

```
fixtures/sorafs_orchestrator/
  multi_peer_parity_v1/
    metadata.json   # summary + payload location
    plan.json       # payload-bound `sorafs.chunk_fetch_plan.v1` envelope
    providers.json  # provider metadata + range/stream budgets
    telemetry.json  # qos/latency/failure-rate snapshot per provider
    options.json    # orchestrator options + scoreboard parameters
```

The payload bytes referenced in `metadata.json` live in
`fuzz/sorafs_chunker/sf1_profile_v1_input.bin` (shared with the chunker
fixtures) and remain 1 MiB so parity runs complete quickly.

## Regeneration

Run the helper script whenever the chunker profile or provider template
changes:

```bash
python3 scripts/build_sorafs_orchestrator_fixture.py
```

The script derives the chunk specs from
`fixtures/sorafs_chunker/sf1_profile_v1.json`, binds the BLAKE3 digest of the
complete referenced payload, applies the deterministic provider template
(`fixture-provider-{i}`), and writes refreshed JSON files under
`multi_peer_parity_v1/`. Bare-array plans are retired and rejected. Rerun all
parity suites after regeneration (`ci/sdk_sorafs_orchestrator.sh`) to capture
the updated metrics in `specs/sorafs/reports/orchestrator_ga.md`.
