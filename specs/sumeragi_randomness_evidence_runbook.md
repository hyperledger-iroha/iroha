<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sumeragi Threshold-Beacon & Evidence Runbook

This runbook records the first-release operator boundary for consensus
randomness and equivocation evidence. Use it with {doc}`sumeragi` and
{doc}`sumeragi_chaos_performance_runbook` when qualifying a validator build or
assembling release evidence.

## Current protocol boundary

Production randomness comes only from a finalized global threshold-BLS pulse.
The live producer binds each pulse to the exact network, active key session,
frozen roster, pulse height, fixed round, and finalized parent anchor; verified
partial shares are combined into one canonical public pulse.

- For NPoS, the last committed pre-boundary block must contain the finalized
  pulse used to derive the successor epoch's election seed. Missing or invalid
  pulse effects reject that mandatory candidate.
- Committed Parliament sortition and timed-ballot requests use the same
  producer. Those demand slots remain optional for chain liveness so the
  governance reducer can classify objective pulse absence and start a fresh
  attempt.
- The first-release wire and signed-effects schemas contain no consensus
  `VrfCommit`/`VrfReveal`, VRF participant record, or VRF absence penalty.

The `sumeragi_vrf_*` metric series are not registered or exported; operators
must not restore them, the former
`/v1/sumeragi/vrf/*` routes, `vrf-epoch` or `vrf-penalties` CLI commands, VRF
alerts, or VRF dashboard panels.

## Prerequisites

- `iroha_cli` configured for the target cluster (see `specs/cli.md`).
- An allow-listed operator key in an absolute runtime-only mode-`0600` file.
  Each command below assumes the global
  `--operator-private-key-file /absolute/runtime/operator.key` option.
- Access to every validator's authenticated status, canonical committed block
  archive, logs, and `/metrics` output.
- The expected signed revision-4 height context, roster, network identifier,
  and active threshold-beacon key session for the cut under review.

## 1. Capture consensus and pulse state

Capture authenticated status from every validator:

```bash
iroha --output-format text ops sumeragi status
```

For each required pulse slot, archive the canonical block and its consensus
effects. Check that all validators agree on the height context and finalized
parent, and that the stored pulse names the exact pulse height, network, active
session, roster/transcript commitments, and fixed protocol round. For an NPoS
boundary, also retain the successor context showing the seed derived from that
pre-boundary pulse. A local log or partial-share capture is supporting
transport evidence; it is not a substitute for the committed finalized pulse.

For a Parliament demand slot, retain the committed request/attempt state and
either its exact finalized pulse or the reducer transition that classified the
slot unavailable. A later pulse cannot repair a slot already terminally
classified unavailable, and a retry must use its newly committed attempt and
future pulse height.

## 2. Capture canonical equivocation evidence

Evidence is read-only through Torii and the CLI. Mutation is admitted only by
the authenticated consensus peer path and canonical signed-block evidence
batches.

```bash
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 100
```

Record the corresponding `sumeragi_evidence_records_total` observation and, if
event consumers are in scope, capture the filtered `/v1/events/sse` stream
described in {doc}`torii/sumeragi_evidence_app_api`. The CLI count, list, and
SSE projection must identify the same canonical Sumeragi-v2 equivocation
records. Torii, CLI, MCP, and SDK surfaces provide no evidence-injection or
rebroadcast operation.

Governed `SumeragiNposParameters.evidence_horizon_blocks` bounds
admission age. A penalty may consume only self-contained evidence admitted by
a prior committed block; `slashing_delay_blocks` leaves the governed
cancellation window. These two windows are immutable after signed genesis and
their sum cannot exceed three epochs, matching the four-roster durable table.
A node-local pending observation cannot authorize a slash.

## 3. Release evidence checklist

For every rehearsal or release candidate, retain:

1. each validator's authenticated Sumeragi status and signed height context;
2. each required or requested pulse slot's canonical block/effect and the
   active public key-session record used to verify it;
3. threshold-share transport logs for missing-share, invalid-share,
   retransmission, view-change, and restart scenarios, without exporting secret
   shares;
4. the evidence count/list and any SSE capture used to check read-surface
   parity; and
5. the exact build revision, network identifier, roster, and artifact paths in
   the run-local evidence README.

Do not describe a flat legacy VRF counter as healthy beacon operation. The
current release has no dedicated public beacon metric family, so release proof
must remain anchored in authenticated context and committed pulse state.

## 4. Troubleshooting

- **Mandatory NPoS pulse missing** — Check that the key session is active at
  the pulse height, its roster matches the frozen height context, local runtime
  custody owns the correct seat, and authenticated
  `GlobalBeaconPartialSignature` traffic reaches threshold. Do not bypass the
  candidate failure or substitute a local seed.
- **Pulse rejected** — Compare network, session, roster/transcript, height,
  fixed round, and finalized parent anchor before investigating cryptography.
  A stale key or foreign-chain pulse must fail closed.
- **Parliament demand pulse absent** — Preserve the exact request and deadline
  state, allow the reducer to classify that attempt objectively, and verify the
  retry commits a new attempt with a strictly future pulse height. Do not reuse
  a late pulse or caller-supplied entropy.
- **Legacy VRF frames or alerts appear** — Treat them as a retired sender,
  fixture, or deployment artifact. Exact ingress decoding rejects the frames and
  no VRF telemetry threshold is a recovery procedure.
- **Evidence views diverge** — Compare authenticated peer-ingress logs,
  canonical evidence admissions, CLI count/list, and SSE output on multiple
  validators. Repair the diverging node from canonical state; do not inject
  evidence through Torii.
