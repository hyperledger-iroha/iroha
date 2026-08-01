---
title: Musubi V1 Operations and Incident Runbook
---

# Musubi V1 Operations and Incident Runbook

This source-adjacent runbook defines the deterministic response boundaries for
Musubi publication, registry, archive, query, and cache incidents. The public
operator guide belongs in `iroha-docs`; this file stays with the metric and
wire contracts that an implementation change can affect.

## Safety invariants

- Never place a signer, private key, bearer token, seed-ingress credential,
  provider URL, or stream token in a project file, command line, operation
  journal, dashboard label, alert, or incident attachment.
- Do not restore the retired public Torii upload route. Stage only through an
  admitted authenticated SoraFS seed-ingress service.
- Do not mutate an immutable release, archive commitment, or completed cache
  destination. Resume only an operation whose derived operation ID and every
  retained commitment are identical.
- Do not delete from the shared cache using paths or archive IDs supplied by a
  workspace lock. Repair first authenticates the registry commitment and
  quarantines only a validated descendant.
- Do not retire the last valid location of an active or yanked release. A
  governed takedown is a separate enacted action, not a storage repair tool.
- Do not clear, migrate, or reinterpret legacy Musubi state. A node that finds
  it must remain stopped until the disposable pre-release state is explicitly
  reset under the launch procedure.

## Metric contract

All labels are fixed enumerations. Package names, namespaces, aliases,
accounts, archive IDs, CIDs, provider IDs, operation IDs, transaction hashes,
and raw errors are forbidden as metric labels.

| Metric | Bounded labels | Meaning |
| --- | --- | --- |
| `musubi_publication_phase_age_seconds` | `phase` (the seven V1 phases) | Oldest active operation age in each phase. |
| `musubi_replication_shortfall_releases` | none | Freshly indexed releases currently below three replicas. |
| `musubi_ingest_deadletters_total` | `reason` | Terminal authenticated-ingress failures. |
| `musubi_integrity_failures_total` | `surface` | Commitment, extraction, provider, or readback verification failures. |
| `musubi_cache_corruption_total` | `operation` | Corruption found by fetch, verify, repair, or prune validation. |
| `musubi_cursor_failures_total` | `reason` | Invalid, stale-anchor, stale-revision, wrong-query, or wrong-caller cursors. |
| `musubi_governance_rejections_total` | `action`, `reason` | Rejected bounded package/alias/recovery mutations. |
| `musubi_storage_bytes_used` | none | Bytes occupied by the measured registry archive/cache root. |
| `musubi_storage_bytes_capacity` | none | Configured capacity of the same measured root. |

The dashboard is `dashboards/grafana/musubi_registry.json`; Prometheus rules
and their rule-unit fixture are under `dashboards/alerts/`.

## Stalled publication

1. Identify the bounded phase from telemetry. Obtain the operation ID from the
   publisher's local journal, not from a metric label.
2. Run a read-only journal inspection. Confirm schema/version, operation ID,
   chain/genesis, publisher, semantic manifest digest, `ArchiveId`, CAR digest
   and length, and the latest completed phase.
3. Recreate runtime credentials from explicit or platform Iroha
   configuration. Never copy them into the journal.
4. Resume the exact operation. The workflow revalidates every retained receipt,
   archive/pin result, completion, readback, AMX transaction, and finalized row
   before advancing.
5. If a retained object differs, stop. Preserve the journal and evidence as an
   integrity incident; do not start a same-version publication with different
   commitments.

## Replication shortfall or provider loss

1. Confirm the universal resolver row is no longer fresh-selectable and record
   the finalized availability revision.
2. Query the exact archive and its bounded location directory. Verify pin
   status, order status, retained provider completions, attestations, expiry,
   and takedown state; do not scan or use fuzzy search for resolver decisions.
3. If at least one valid location remains, preserve locked fetches while
   arranging a replacement registry-grade pin and distinct providers.
4. Add the replacement location with compare-and-set revision and finalized
   evidence. Confirm the home/universal projection becomes selectable only at
   three healthy replicas.
5. Retire a failed location only after the replacement is finalized. If no
   location remains, page storage and registry owners immediately; ordinary
   owners cannot use Parliament takedown or recovery as a shortcut.

## Seed-ingress deadletter or integrity failure

1. Stop the affected publication before archive registration or release claim.
2. Retain the signed receipt/attestation, canonical CAR digest/length, finalized
   anchor, and bounded failure class. Redact runtime authentication material.
3. Check chain/genesis, publisher, admitted broker/provider, semantic digest,
   archive and body commitment, nonce, expiry, signer policy, and replay state.
4. Independently parse the bundle and verify descriptor, source tree,
   verification lock, chunk plan, PoR root, and CAR commitments.
5. A substituted, replayed, expired, redirect-derived, or DNS-rebound staging
   result is terminal evidence. Do not retry it as a valid receipt.

## Cache corruption

1. Use `musubi cache verify` with authenticated finalized archive commitments
   and the canonical SoraFS plan. A consumer lock is not sufficient evidence.
2. Run `musubi cache repair` for the exact archive. Repair opens descendants
   with no-follow semantics and quarantines only after validating ancestry.
3. Refetch into a private sibling, verify every commitment, fsync, and rename
   only into an absent `registry-v1/<archive-id>/src` destination.
4. If corruption repeats, retain the quarantined tree and classify storage,
   provider, concurrent-writer, or crash/disk-full cause before restoring use.

## Cursor failures

An invalid cursor is never silently restarted by the server. Confirm the
failure class, discard the cursor client-side, acquire a new finalized snapshot,
and repeat the identical typed query. Repeated stale-revision failures indicate
registry churn or a lagging endpoint; wrong-query or wrong-caller failures are
client misuse or possible replay and should be investigated.

## Unauthorized governance attempts

1. Retain the rejected signed transaction and bounded action class.
2. Verify exact package identity, accepted role/capability, namespace delegation
   generation where applicable, and expected governance or metadata revision.
3. For Parliament recovery, verify enacted decision digest, delay, action
   digest, and replay-consumption state. Never accept an advisory or unrelated
   decision.
4. Escalate repeated unauthorized attempts. Do not grant a role or change a
   revision merely to make a rejected transaction succeed.

## Storage pressure

1. At 85%, pause nonessential prefetch and confirm the trusted global retained
   `ArchiveId` projection. At 95%, close new archive/release/alias admission if
   operational policy requires it; reads, repair, recovery, and yank remain.
2. Prune only complete validated cache roots absent from the retained set.
3. Renew or rebalance archive locations before retiring any pin. Confirm every
   active and yanked release retains at least one valid location and fresh
   releases retain quorum.
4. Record reclaimed bytes, affected immutable IDs, and final availability
   revisions without logging content, credentials, or bearer URLs.

## Recovery and closure

Close an incident only after exact home and universal reads agree at one
finalized anchor, alerts have cleared for a full evaluation window, any
quarantined evidence is retained under incident policy, and the remediation has
a deterministic regression test. Parliament drills must also prove enactment
delay and replay rejection.
