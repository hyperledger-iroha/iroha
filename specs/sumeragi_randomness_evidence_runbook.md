<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Sumeragi Randomness and Evidence Runbook

First-release NPoS uses the adaptive global threshold beacon as its only
consensus-randomness source. The pre-release VRF commit/reveal protocol, its
penalty counters, and its Torii/CLI surfaces do not exist in the live protocol.

Use this checklist with `specs/sumeragi_v2.md` and the Sumeragi chaos runbook
when qualifying a validator release.

## Capture the authoritative context

1. Run `iroha --output-format text ops sumeragi status` on every validator.
2. Record the network, protocol and shared-configuration fingerprints, height,
   view, epoch, committee, and latest durable commit.
3. Confirm every validator reports the same signed height-context identity.
   Repair mismatches through genesis or governed chain state; there is no local
   consensus-mode or randomness fallback.

At an NPoS epoch boundary, preserve the validator logs covering the final
pre-boundary height and successor-context construction. The transition must
authenticate the unique persisted threshold-beacon pulse, its canonical chain
anchor, and the active network-bound DKG session. A missing, duplicated, stale,
or invalid pulse fails the transition closed.

## Monitor progress

Correlate authoritative height/view progress with:

- committed-block and view-change counters;
- consensus ingress drops and bounded queue pressure;
- authenticated peer and threshold-beacon validation logs;
- agreement on the successor roster and `leader_seed` after the boundary.

Do not alert on `sumeragi_vrf_*`: those retired metrics are absent. A sustained
epoch-boundary stall should be handled as a threshold-beacon or authenticated
peer-availability incident. Preserve the signed contexts and logs before
restarting a validator.

## Inspect consensus evidence

Equivocation evidence is admitted only through the authenticated consensus peer
path and exposed read-only by Torii:

```bash
iroha --output-format text ops sumeragi evidence count
iroha --output-format text ops sumeragi evidence list --limit 5
iroha ops sumeragi evidence list --limit 100 > artifacts/evidence_snapshot.json
```

Confirm the CLI and `/v1/sumeragi/evidence/count` agree. Records older than the
governed evidence horizon are rejected. Torii and the CLI provide no evidence
injection path.

## Package release evidence

For each rehearsal or release candidate, retain:

- every validator's Sumeragi status capture;
- the boundary-height threshold-beacon validation logs;
- the evidence snapshot and any event-stream capture;
- queue, ingress-drop, view-change, and committed-block telemetry;
- the inspected epoch and artifact paths in `status.md`.

If evidence counts diverge, compare authenticated ingress and Kura state before
repairing the affected validator. If successor contexts diverge or fail to
open, treat the signed context, canonical chain anchor, DKG session, and pulse
verification as the source of truth; do not restore VRF records or a seed
fallback.
