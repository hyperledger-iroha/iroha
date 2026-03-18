# Sumeragi Formal Model (TLA+ / Apalache)

This directory contains a bounded formal model for Sumeragi commit-path safety and liveness.

## Scope

The model captures:
- phase progression (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- vote and quorum thresholds (`CommitQuorum`, `ViewQuorum`),
- weighted stake quorum (`StakeQuorum`) for NPoS-style commit guards,
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) with header/digest evidence,
- GST and weak fairness assumptions over honest progress actions.

It intentionally abstracts away wire formats, signatures, and full networking details.

## Files

- `Sumeragi.tla`: protocol model and properties.
- `Sumeragi_fast.cfg`: smaller CI-friendly parameter set.
- `Sumeragi_deep.cfg`: larger stress parameter set.

## Properties

Invariants:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Temporal property:
- `EventuallyCommit` (`[] (gst => <> committed)`), with post-GST fairness encoded
  operationally in `Next` (timeout/fault preemption guards on enabled
  progress actions). This keeps the model checkable with Apalache 0.52.x, which
  does not support `WF_` fairness operators inside checked temporal properties.

## Running

From repository root:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Reproducible local setup (no Docker required)

Install the pinned local Apalache toolchain used by this repository:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner auto-detects this install at:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
After installation, `ci/check_sumeragi_formal.sh` should work without extra env vars:

```bash
bash ci/check_sumeragi_formal.sh
```

If Apalache is not in `PATH`, you can:

- set `APALACHE_BIN` to the executable path, or
- use the Docker fallback (enabled by default when `docker` is available):
  - image: `APALACHE_DOCKER_IMAGE` (default `ghcr.io/apalache-mc/apalache:latest`)
  - requires a running Docker daemon
  - disable fallback with `APALACHE_ALLOW_DOCKER=0`.

Examples:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Notes

- This model complements (does not replace) executable Rust model tests in
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  and
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- The checks are bounded by constant values in the `.cfg` files.
- PR CI runs these checks in `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.
