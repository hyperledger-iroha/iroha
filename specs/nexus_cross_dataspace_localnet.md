---
title: Nexus Cross-Dataspace Localnet Proof
sidebar_label: Cross-Dataspace Localnet Proof
description: Reproducible workflow for proving ds1/ds2 atomic all-or-nothing settlement on localnet.
---

# Nexus Cross-Dataspace Localnet Proof

This runbook executes the Nexus integration proof that:

- boots an exact 13-peer revision-4 global committee whose first 12 peers form
  disjoint four-validator Nexus, `ds1`, and `ds2` lane committees; the final
  peer remains a global voter and lane observer,
- routes account traffic into each dataspace,
- creates an asset in each dataspace,
- executes atomic swap settlement across dataspaces in both directions,
- runs ten paired forward/reverse swaps under the strict DvP liveness gate,
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

## Native AMX fault soak

The same wrapper can run the ignored four-peer Native AMX fault corridor:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh \
  --native-amx-fault-soak \
  --native-amx-iterations 10
```

Each iteration rotates one validator fully offline before submission, requires the remaining three
validators to produce independently verifiable, lane-bound prepare and commit attestation QCs for
both participant legs, and proves that the offline validator's signer bit is absent. The validator
is then restarted and must recover the byte-identical coordinator block, Native AMX receipt,
participant QCs, settlement commitment, and relay before the next fault is injected. The bounded
override accepts `1..100`; invalid values fail in the wrapper, while a direct invalid environment
override falls back to the ten-iteration default.

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
  cargo test --locked --offline -p integration_tests --test nexus_and_streaming \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

Full Nexus subset:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test --locked --offline -p integration_tests --test nexus_and_streaming nexus:: -- --nocapture --test-threads=1
```

Native AMX rotating-validator fault soak:

```bash
IROHA_RUN_IGNORED=1 IROHA_NATIVE_AMX_SOAK_ITERATIONS=10 \
  IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test --locked --offline -p integration_tests --test native_amx_routing \
  native_amx_rotating_validator_fault_soak_preserves_independent_participant_qcs \
  -- --exact --nocapture --test-threads=1
```

The production multilane corridor runs this test and the four-peer autoscale
A/B/A lifecycle gate as ordinary, non-ignored inventory tests through
`scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
--multilane-four-peer-release`. Release mode requires a real network and exact
start/completion markers; a developer opt-out cannot satisfy the gate.

## Expected Proof Signals

- The test passes.
- One expected warning appears for the intentionally failing underfunded settlement leg:
  `settlement leg requires 10000 but only ... is available`.
- Final balance assertions succeed after:
  - successful forward swap,
  - successful reverse swap,
  - failed underfunded swap (rollback unchanged balances).

## Release validation requirement

The former February 19, 2026 four-peer snapshot predates the disjoint
12-lane-validator corridor on an exact 13-peer global committee and is not
release evidence for the current implementation. A production sign-off
requires the actual network startup logs, all three four-validator committees,
at least 9 of 10 successful paired swap iterations with no more than two
retries, and the final adversarial underfunded rollback. A sandbox bind skip or
a partial 3-of-10 run is not a pass.

For a multi-seed run, use `scripts/nexus/run_cross_runtime_matrix.sh` after prebuilding a compatible
`irohad`; do not rely on `IROHA_TEST_SKIP_BUILD=1` unless that binary is present and matches the
workspace.
