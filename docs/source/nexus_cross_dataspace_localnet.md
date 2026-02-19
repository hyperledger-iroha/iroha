---
title: Nexus Cross-Dataspace Localnet Proof
sidebar_label: Cross-Dataspace Localnet Proof
description: Reproducible workflow for proving ds1/ds2 atomic all-or-nothing settlement on localnet.
---

# Nexus Cross-Dataspace Localnet Proof

This runbook executes the Nexus integration proof that:

- boots a 4-peer localnet with two restricted private dataspaces (`ds1`, `ds2`),
- routes account traffic into each dataspace,
- creates an asset in each dataspace,
- executes atomic swap settlement across dataspaces in both directions,
- proves rollback semantics by submitting an underfunded leg and checking balances stay unchanged.

The canonical test is:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## Quick Run

Use the wrapper script from repository root:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

Default behavior:

- runs only the cross-dataspace proof test,
- sets `NORITO_SKIP_BINDINGS_SYNC=1`,
- sets `IROHA_TEST_SKIP_BUILD=1`,
- uses `--test-threads=1`,
- passes `--nocapture`.

## Useful Options

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` keeps temporary peer directories (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) for forensics.
- `--all-nexus` runs `mod nexus::` (full Nexus integration subset), not just the proof test.

## CI Gate

CI helper:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

Make target:

```bash
make check-nexus-cross-dataspace
```

This gate executes the deterministic proof wrapper and fails the job if the cross-dataspace atomic
swap scenario regresses.

## Manual Equivalent Commands

Targeted proof test:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Full Nexus subset:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## Expected Proof Signals

- The test passes.
- One expected warning appears for the intentionally failing underfunded settlement leg:
  `settlement leg requires 10000 but only ... is available`.
- Final balance assertions succeed after:
  - successful forward swap,
  - successful reverse swap,
  - failed underfunded swap (rollback unchanged balances).

## Current Validation Snapshot

As of **February 19, 2026**, this workflow passed with:

- targeted test: `1 passed; 0 failed`,
- full Nexus subset: `24 passed; 0 failed`.
