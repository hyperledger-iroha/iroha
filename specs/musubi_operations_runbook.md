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
  quarantines only a validated descendant. Prune accepts only exact identities
  carrying an explicit finalized prune disposition.
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
| `musubi_replication_shortfall_releases` | none | Release references on non-selectable archives, including yanked and taken-down releases. |
| `musubi_ingest_deadletters_total` | `reason` | Terminal authenticated-ingress failures. |
| `musubi_integrity_failures_total` | `surface` | Commitment, extraction, provider, or readback verification failures. |
| `musubi_cache_corruption_total` | `operation` | Corruption found by fetch, verify, repair, or prune validation. |
| `musubi_cursor_failures_total` | `reason` | Invalid, stale-anchor, stale-revision, wrong-query, wrong-caller, boundary, or other cursor failures. |
| `musubi_governance_rejections_total` | `action`, `reason` | Rejected bounded namespace/archive/package/alias/Parliament mutations. |
| `musubi_storage_bytes_used` | none | Bytes occupied by the measured registry archive/cache root. |
| `musubi_storage_bytes_capacity` | none | Configured capacity of the same measured root. |

Producer boundaries are deliberately strict. The private publication service
increments an ingest deadletter only after request authentication and only for
a terminal seed-storage rejection, invalid broker receipt, or conflicting
receipt/idempotency binding. An authenticated storage-coordination request with
an invalid or future-skewed staging receipt records `receipt_invalid`; an
expired staging receipt records `receipt_expired`. Retryable backend outages
and rejected unauthenticated requests are not deadletters. Authenticated CAR
commitment, composite storage-coordination evidence, and
full-provider-readback mismatches increment the archive-commitment, bounded
fallback, and provider-readback integrity surfaces respectively; a later
journal-abort failure must not hide the original observation.

The service constructs the complete seed-receipt payload and expiry before
calling the deployment signing provider. HSM/KMS or threshold adapters return
only broker-controller approvals. Wrong-payload signatures, non-controller or
duplicate approvals, and broker/backend identity substitutions are permanent
provider faults; an explicitly retryable signing-provider outage returns a
redacted retryable response and leaves the exact journal tombstone available
for a freshly authorized retry. Request objects carry no time supplied by a
caller or transport adapter. The service clock is sampled at authorization
admission, after staging for receipt issuance, and after signing before commit;
zero, unavailable, or regressing time fails with the typed trusted-clock error.
If signer latency consumes the receipt lifetime, the reservation is aborted and
the expired receipt is never cached. On Unix,
`DurableMusubiPublicationServiceClockV1` is the restart-persistent floor
adapter. Initialize it exactly once with `initialize` or `initialize_system` in
an existing, empty, service-exclusive `0700` directory, then use `open` or
`open_system` on every restart. It holds the owner lock for its lifetime and
commits a bounded, digest-bound canonical Norito floor through singly linked
`0600` files, atomic rename, file `fsync`, and directory `fsync`. Missing,
deleted, malformed, linked, substituted, or regressing state fails closed;
ordinary open never regenerates it. The backing storage must itself resist
external snapshot rollback, and its ACL must exclude same-owner writers outside
the service. Until child mutation is descriptor-relative, its canonical root
must also sit below a trusted, non-replaceable ancestor. The adapter cannot
detect an older valid envelope if both its storage and trusted source are rolled
back together; deployments must provide rollback-resistant storage or an
external sealed monotonic head. A raw system clock on unqualified storage is
insufficient, and V1 remains fail-closed on non-Unix platforms. Reject at
startup any broker policy whose best subset of at most 64 controller approvals
cannot meet its threshold.

`DurableMusubiPublicationServiceJournalV1` is the concrete Unix restart journal.
Give it a separate existing, empty, service-owned `0700` directory; call
`initialize` exactly once and `open` thereafter with the exact
chain/genesis/broker/provider binding and deployment-fixed operation,
authorization, response-byte, and snapshot limits. It holds an empty `0600`
lifetime lock and commits a digest-bound canonical Norito snapshot through a
fixed `0600` next file, exact rereads, file and directory `fsync`, and atomic
rename. Startup durably changes interrupted fresh attempts to aborted
tombstones and interrupted receipt refreshes back to their prior completed
response before accepting traffic. The snapshot contains operation bindings,
request and authorization digests, expiry, and typed response bytes, but no CAR,
authorization body, credential, URL, token, or provider secret. Missing,
malformed, linked, substituted, configuration-mismatched, limit-mismatched, or
noncanonical state fails closed; ordinary open never initializes or resizes it.
The same rollback-resistant-storage and trusted-ancestor requirements apply.
Select deployment limits below the protocol hard caps and qualify peak memory
and latency: V1 commits complete snapshots and deliberately performs full-state
validation rather than claiming a database-scale journal implementation.

The phase-age gauge requires a long-lived worker to project the oldest active
operation from the secret-free publication journal. The one-shot CLI is not a
Prometheus producer. Likewise, cache-corruption and cache-capacity gauges need
a long-lived cache service, and the storage pair must describe one explicitly
selected root; do not mirror aggregate SoraFS capacity into a Musubi-only gauge.
The authenticated production fetch runtime carries the closed
`archive_commitment` integrity surface on provider manifest, plan, CAR, and
chunk errors because each is checked against the exact registry commitment.
Token and other authenticated control failures use the bounded fallback. The
archive adapter invokes its injected observer once when admitting a failed
provider attempt into deterministic failover; it never derives the label from
the public error code. The one-shot CLI installs no observer, so a long-lived
host must map this boundary to its metrics registry. The distinct
`provider_readback` surface remains reserved for the full publication readback
workflow.

For the six paged Musubi registry queries, Core carries its exact typed cursor
failure to Torii alongside the unchanged public `Expired` query error. Torii
maps finalized-anchor, index-revision, query, caller, and last-key failures to
`stale_anchor`, `stale_revision`, `wrong_query`, `wrong_caller`, and `boundary`
respectively. Torii's structural cursor validation records `invalid` before
query execution. Do not infer one of these exact causes from an ordinary
`Expired` error on an unrelated query path; that bounded fallback is `other`.

Core updates replication shortfall from exact archive reverse references when
availability crosses the selectable boundary. The exact count is persisted in
a universal consensus cell, validated against reverse references during
snapshot load, bound into the Native AMX write set, read once to seed the gauge
at startup, and mirrored only after world-state commit succeeds. It deliberately
counts yanked and Parliament-taken-down releases while their archive remains
non-selectable because this signal measures replication exposure, not fresh
resolver eligibility. A zero sample therefore means the committed aggregate is
zero; operators should still correlate it with archive-location and provider
health alerts.

Governance rejection producers must receive typed action and reason values at
the ISI rejection site; classifying a returned error string is forbidden. Core
records each namespace-binding, archive-registration, package,
archive-location, alias, or Parliament mutation exactly once at its final
`Execute` error boundary. The attempt-local reason is `other` unless the exact
semantic branch marks unauthorized authority, stale revision, last-owner
protection, closed admission, invalid/replayed Parliament decision, or alias
payment. Until the other long-lived producers exist, absence of their series is
unknown, not proof that the corresponding queue is empty or healthy.

The dashboard is `dashboards/grafana/musubi_registry.json`; Prometheus rules
and their rule-unit fixture are under `dashboards/alerts/`.

The publisher-owned `publication-v1` directory stores exact canonical Norito
journals and persistent zero-length operation lock files. On Unix, the
directory must remain exact mode `0700` and each lock exact mode `0600`;
hard links, symlinks, special bits, trailing or bare frames, and concurrent
lock ownership fail closed. The operation lock spans the complete
load/compare/write/reload transition. Never delete it to force a competing
resume process through compare-and-set.

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

## Private publication service outage

1. Confirm the stock daemon has not exposed a Torii upload route. The private
   service exists only when a deployment runner was explicitly injected and
   supervised through `run_with_musubi_publication` or the combined
   runtime-provider launcher; `Unavailable` with no runner is the intended
   fail-closed state.
2. Check the deployment-owned HTTPS listener, TLS identity, qualified durable
   clock source and private root, durable replay journal and its separate root, broker
   HSM/signer, seed-ingress backend, permanent pin/replication coordinator, and
   provider readback adapters independently. Use the durable journal for
   restart-persistent service state; the bundled in-memory journal remains only
   for development/tests. Do not move credentials into node configuration,
   argv, a project, or an incident log.
3. Record only the clock's stable, path-free startup code. Keep the service
   offline for `MUSUBI_PUBLICATION_CLOCK_UNSUPPORTED_PLATFORM`,
   `MUSUBI_PUBLICATION_CLOCK_UNSAFE_ROOT`,
   `MUSUBI_PUBLICATION_CLOCK_UNINITIALIZED`,
   `MUSUBI_PUBLICATION_CLOCK_INVALID_STATE`,
   `MUSUBI_PUBLICATION_CLOCK_ROLLBACK`, or
   `MUSUBI_PUBLICATION_CLOCK_STORAGE_UNAVAILABLE`. For
   `MUSUBI_PUBLICATION_CLOCK_LOCKED`, locate the existing supervised owner; do
   not delete the lock. `MUSUBI_PUBLICATION_CLOCK_ALREADY_INITIALIZED` means a
   restart incorrectly used the one-time initializer, while
   `MUSUBI_PUBLICATION_CLOCK_SOURCE_UNAVAILABLE` requires restoring the trusted
   time source. Do not manually delete, replace, or regenerate
   `clock-floor-v1.lock`, `clock-floor-v1.norito`, or `clock-floor-v1.next`.
   Preserve the evidence; V1 has no automatic recovery path for an invalid or
   lost floor.
4. Record only the journal's stable, path-free startup code. Treat
   `MUSUBI_PUBLICATION_JOURNAL_UNSUPPORTED_PLATFORM`,
   `MUSUBI_PUBLICATION_JOURNAL_UNSAFE_ROOT`,
   `MUSUBI_PUBLICATION_JOURNAL_INVALID_STATE`, and
   `MUSUBI_PUBLICATION_JOURNAL_STORAGE_UNAVAILABLE` as integrity or platform
   incidents. Resolve `MUSUBI_PUBLICATION_JOURNAL_LOCKED` by locating the live
   owner. `MUSUBI_PUBLICATION_JOURNAL_UNINITIALIZED` and
   `MUSUBI_PUBLICATION_JOURNAL_ALREADY_INITIALIZED` identify lifecycle misuse;
   `MUSUBI_PUBLICATION_JOURNAL_CONFIGURATION_MISMATCH` and
   `MUSUBI_PUBLICATION_JOURNAL_LIMITS_MISMATCH` require the original deployment
   identity and capacities. Never delete, replace, or regenerate
   `publication-journal-v1.lock`, `publication-journal-v1.norito`, or
   `publication-journal-v1.next` during ordinary recovery.
5. Treat an unexpected runner return as a supervised fatal failure. Restore
   the same qualified identities and durable journal before restarting; never
   substitute a fresh process-local replay map in production.
6. Preserve immutable operation bindings and completed typed responses. An
   exact retry may reuse its prior result, but the same operation ID with a
   different chain or genesis incarnation, publisher, archive, CAR
   digest/length, route request, or provider target is an equivocation and must
   remain rejected.
   If the exact completed seed receipt alone expired, use a fresh authorization
   to trigger the journaled refresh path; the backend must idempotently confirm
   the same CAR before the broker replaces that receipt.
7. For authorization incidents, compare only bounded error classes. Verify
   canonical encoding, exact request digest, controller signature, expiry,
   future-clock skew, and replay status without logging the authorization
   header. Invalid authorization must be rejected before a full CAR hash or
   any seed backend call.

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
client misuse or possible replay and should be investigated. A `boundary`
failure means the cursor's last key is absent or noncanonical in the bound
snapshot; `invalid` means the supplied cursor failed structural validation.

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

1. At 85%, pause nonessential prefetch. At 95%, close new
   archive/release/alias admission if operational policy requires it; reads,
   repair, recovery, yank, and retention queries remain available.
2. Inventory only canonical cache `ArchiveId` directories. Query retention in
   sorted, distinct batches of at most 100 exact identities. The first batch
   establishes the chain ID, genesis hash, and finalized registry snapshot;
   send that snapshot as `expected_snapshot` on every later batch.
3. Abort the whole prune without deleting anything if any batch is stale, has
   a different chain/genesis/snapshot, omits or reorders an identity, or exposes
   inconsistent archive, reverse-reference, release, or availability records.
   Unknown archives retain fail-closed because the user cache is not
   chain-scoped.
4. Retain archives referenced by any governance-available active or yanked
   release, regardless of replication quorum or availability. Feed only
   explicit `PruneUnreferenced` and `PruneGovernedTakedown` decisions into the
   point-targeted `prune_exact` operation. Never derive deletion authority from
   a workspace lock or from absence in a global retained set.
5. Run `musubi cache prune --dry-run` first during an incident. It performs and
   reports the same finalized classification but must not rename or delete any
   cache path. A live prune may remove only the exact queried candidates; a
   concurrently installed, never-queried archive remains untouched.
6. Renew or rebalance archive locations before retiring any pin. Confirm every
   active and yanked release retains at least one valid location and fresh
   releases retain quorum.
7. Record reclaimed bytes, affected immutable IDs, and final availability
   revisions without logging content, credentials, or bearer URLs.

## Recovery and closure

Close an incident only after exact home and universal reads agree at one
finalized anchor, alerts have cleared for a full evaluation window, any
quarantined evidence is retained under incident policy, and the remediation has
a deterministic regression test. Parliament drills must also prove enactment
delay and replay rejection.
