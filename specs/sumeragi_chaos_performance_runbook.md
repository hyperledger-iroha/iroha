<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sumeragi revision-4 performance and fault validation

This runbook maps release evidence to the live revision-4 protocol. Pair it
with `specs/sumeragi.md`, `specs/sumeragi_soak_matrix.md`, and
`specs/sumeragi_pacemaker.md`.

Finite Sumeragi reads require a fresh exact-`NetworkId` operator signature.
Examples assume the global runtime-only
`--operator-private-key-file /absolute/runtime/operator.key` option; account
keys, bearer tokens, environment variables, and client TOML are not fallbacks.

## Evidence classes

Use separate evidence for distinct properties:

- baseline throughput and latency: `npos_baseline_1s_captures_metrics`;
- finite transaction-queue saturation and producer deferral:
  `npos_queue_backpressure_triggers_metrics`;
- authenticated safety/message-ordering faults: the controlled message-release
  scenarios in `integration_tests/tests/sumeragi_v2_runner.rs`;
- process recovery and sustained liveness:
  `npos_localnet_realistic_30tps_2h_rotating_validator_outage`;
- admissible committee scaling: the 4/7/10-peer soak matrix.

The V1 global RBC store/chunk, collector fan-out, jitter/EMA pacemaker, and
debug-message scenarios are not revision-4 release evidence.

## Baseline and queue pressure

Build the exact binaries under review, then run:

```bash
python3 scripts/run_sumeragi_stress.py \
  --artifacts artifacts/sumeragi-stress-$(date +%Y%m%d-%H%M)
```

The helper invokes only the two current NPoS performance tests and records
stdout/stderr plus a JSON summary. Use `SUMERAGI_NPOS_STRESS_PEERS` only for an
admissible exact `3f + 1` committee. Mode, cadence, Set A/B, quorum, and DA
layout come from signed genesis/current-height context and are not environment
or local-config dimensions.

Capture:

- authenticated `/v1/sumeragi/status` before and after the run;
- shared configuration and height-context fingerprints;
- transaction/adapter queue capacities, depths, saturation, and deferrals;
- committed height/transaction deltas and wall-clock run bounds;
- host, binary digest, repository commit, and generated genesis digest.

## Authenticated protocol faults

Use `integration_tests/tests/sumeragi_v2_runner.rs` for conflicting or reordered
consensus evidence. Its control plane holds and releases real authenticated
revision-4 Proposal/Vote/QC/TimeoutCertificate traffic. Verify exact certified
body convergence, sign-once/lock behavior, and common durable successor state.

Do not create protocol faults through node-local TOML. A config layer cannot
forge a valid message, lower `q`, disable mandatory DA, or install a view.

## Validator outage and recovery

The ignored rotating-outage localnet test stops one validator process at a
time, keeps the other `2f + 1` validators live, restarts the stopped validator
with the unchanged base configuration, and verifies status recovery, body/state
catch-up, and convergence before the next outage when strict bounds are enabled.

Run it on a dedicated host and preserve its throughput artifact directory.
Treat missed recovery/convergence bounds as liveness failures; do not respond by
adding retired timeout or recovery knobs.

## Triage

1. Confirm every peer advertises protocol version 4 and the expected signed
   height-context/shared-config fingerprints.
2. Confirm committee geometry is exact `3f + 1` and at least `2f + 1` members
   are responsive.
3. Inspect authoritative height/view/phase, QC/TC references, body/persistence
   state, and latest durable commit in `/v1/sumeragi/status`.
4. Correlate transaction/body/chunk/adapter queue pressure with P2P drop and
   ingress-rejection metrics.
5. Preserve consensus logs and exact controlled-message/outage timestamps.

Node-local RBC/DA, queue, transport, and ingress metrics are non-authoritative
observations and must not override revision-4 status. Retired adaptive-
pacemaker fields and endpoints are not part of the first-release telemetry
surface.

## Sign-off

A release evidence pack must identify every executed scenario and show that no
retired test name or local Sumeragi protocol/timing/DA/debug field was used.
Baseline and queue-pressure rows must pass for each requested committee size;
authenticated safety cases must converge on one certified body; outage runs
must demonstrate bounded recovery. Record any skipped ignored soak as remaining
runtime uncertainty rather than inferring success from static checks.
