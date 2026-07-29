---
title: Secure Evidence Viewer & Access Logging
summary: SFM-4b3 reference implementation for case-bound, WebAuthn-attested evidence viewing with rotating grants, authenticated range decryption, signed payload-free access receipts, and legal-hold-aware retention and erasure.
---

# Secure Evidence Viewer & Access Logging

## Implementation Status

The SFM-4b3 reference implementation is present in `sorafs_node` and Torii.
It provides a case-bound evidence service, finalized-ledger authorization,
WebAuthn challenge consumption, rotating short-lived grants, authenticated
range decryption, an embedded no-cache viewer shell, signed hash-chained access
receipts, and legal-hold-aware retention and erasure.

This repository state does not by itself prove a production deployment. Release
promotion still requires the reviewed payload-free evidence-viewer canary,
multi-instance deployment evidence, security testing, and inclusion in the
SFM-4b moderation-panel and aggregate readiness envelopes.

The pre-release Torii branch for the former operator-only local endpoints
`/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-sessions` and
`/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-access` has been
deleted, including its request DTOs, handlers, conversions, and response
formats. Those routes are neither mountable nor catalogued, advertised by the
operator panel, or documented in OpenAPI.

The two legacy aggregate-audit POST routes remain only as authenticated and
operator-authorized retirement tombstones. They return HTTP `410 Gone` without
parsing the retired request body or mutating the former local registry. There is
no Torii evidence-viewer audit scheduler. The signed receipt checkpoint and the
exact `(sequence, receipt_digest)` transparency projection derived from it are
the sole audit authority.

The surrounding finalized moderation workflow now has a supervised,
payload-free panel-notification path. It durably claims a finalized-event
identity before calling a separately qualified idempotent provider and records
the exact receipt or bounded retry/dead-letter result. That source path carries
no evidence locator, message body, assertion, grant, or bearer secret. A real
deployment-owned messaging/portal provider and its multi-instance evidence are
still required.

## Authoritative Security Model

Every evidence API operation that reads or changes protected state requires
X-Iroha canonical request authentication. Torii derives the actor from the
verified canonical account; requests cannot supply a substitute viewer account.

The service then reads one immutable finalized state view and authorizes exactly
one of:

- a juror currently assigned to the exact open moderation case and round;
- an account holding the explicit `sorafs_evidence_auditor` role; or
- an account holding the explicit `sorafs_legal_reviewer` role.

The generic moderation operator role is insufficient. The finalized case must
commit the same evidence digest as the encrypted quarantine object. Every
manifest, range, and interaction access rechecks authorization and rejects
policy rollback or object substitution.

The runtime WebAuthn boundary issues an unpredictable challenge bound to the
case, round, object, evidence digest, actor, role, purpose digest, policy digest,
and finalized block anchor. Session creation consumes that challenge exactly
once and rejects assertion replay. Sessions have a hard maximum lifetime of 15
minutes. Each successful manifest, range, or event request consumes the active
grant and returns a replacement in the sensitive
`X-SoraFS-Evidence-Grant` response header.

Challenge values, grants, WebAuthn assertions, credential identifiers, signing
keys, KMS credentials, and evidence bytes never enter the checkpoint or logs.
The checkpoint stores only one-way digests, finalized anchors, bounded
payload-free session metadata, signed payload-free receipts, legal holds,
retention decisions, erasure commitments, and idempotency tombstones. The
service persists a public Ed25519-signed checkpoint anchor containing the
canonical checkpoint digest, retained receipt count, and exact receipt-chain
head. The canonical checkpoint is retained in a signed, predecessor-bound
record under a qualified external compare-and-swap authority; the configured
local file is only an exact or one-generation-behind verified cache and can
never seed or replace that authority. A missing installation must durably create
this signed genesis anchor before the service becomes available. Startup
accepts that genesis only after exact authoritative readback. Non-receipt state
therefore cannot be altered independently of the receipt chain. Audit reads
return the retained anchor without calling the signer. Secret wrapper types
redact debug output and scrub owned buffers on drop.

## Runtime Dependencies and Configuration

Production behavior is configured under
`sorafs.storage.evidence_viewer`. Enabling it requires SoraFS storage and
all of these non-secret policy values:

- an absolute private local checkpoint-cache file and bounded checkpoint size;
- session, challenge, and grant lifetimes within the 15-minute ceiling;
- an authenticated range limit and bounded collection limits;
- the WebAuthn relying-party id and exact canonical HTTPS origins;
- opaque production handles for the authoritative checkpoint store, WebAuthn,
  grant, erasure, Ed25519 receipt-signing, and immutable compaction-archive
  services;
- the independently governed non-zero revision and 32-byte public policy digest
  for each of those six runtime providers;
- the archive's stable non-zero namespace id, exact Ed25519 verification key,
  `1000..=86400000` millisecond worker cadence, and `1..=1024` record tick
  bound; and
- the exact governed Ed25519 receipt-verification key.

The qualification keys are the exact
`checkpoint_store_revision`/`checkpoint_store_policy_digest_hex`,
`webauthn_revision`/`webauthn_policy_digest_hex`,
`grant_revision`/`grant_policy_digest_hex`,
`erasure_revision`/`erasure_policy_digest_hex`,
`compaction_archive_revision`/`compaction_archive_policy_digest_hex`, and
`receipt_signer_revision`/`receipt_signer_policy_digest_hex` pairs. They are
required when the service is enabled, forbidden as stale bindings when it is
disabled, and accept only non-zero revisions plus canonical lowercase non-zero
32-byte digests.

The sanitized daemon registry and Torii expose runtime injection seams whose
opaque handles, archive namespace, and public keys must match configuration
exactly. Startup fails closed for missing, partial, unrequested, or mismatched
dependencies. There is no file key, environment secret, `KeyPair`, or
in-process production fallback.
The local checkpoint cache is not an authority fallback. The standard launcher
intentionally does not construct a real external CAS, WebAuthn, grant, HSM
signer, KMS, or immutable object-lock implementation; deployment owners must
construct and inject those runtime dependencies. The complete non-secret shape
is shown in
[`sorafs/snippets/evidence_viewer_runtime_binding.toml`](sorafs/snippets/evidence_viewer_runtime_binding.toml).

Each external security provider must expose its active non-zero
deployment/policy revision and non-zero public-policy digest through a typed,
payload-free readiness result. Before reading the local cache or reading or
creating the authoritative evidence-viewer checkpoint, the service validates
the configured and injected handles, rejects test/development markers and
substitutions, and requires each provider's exact qualification to match the
independently configured expected revision and digest. It also pins the receipt
signer's governed Ed25519 public key plus the archive namespace and Ed25519
verification key. Every external checkpoint load/CAS, WebAuthn, grant, signing,
erasure, and archive install/readback call revalidates that configured identity
and policy around the operation. A stale or changed provider therefore fails
closed, including when it changes while an operation is in flight. Vendor
diagnostic text remains inside protected provider telemetry and cannot enter
service errors or debug output.
When an authoritative checkpoint already carries an archive head, startup and
an explicit live refresh traverse the signed predecessor operation ids all the
way to archive genesis. Every exact historical artifact is re-read and its
canonical bytes, payload/head commitment, configured archive identity, archive
receipt signature, signer signature, generation, and predecessor link are
verified before the local cache or in-memory state is adopted. A missing,
forged, substituted, forked, rolled-back, or over-bound historical readback
fails closed.

The injected boundaries are:

- finalized moderation authorization reader;
- WebAuthn challenge and assertion verifier;
- rotating-grant issuer, verifier, and revoker;
- PKCS#11/HSM Ed25519 receipt signer;
- KMS or cryptographic-erasure service;
- linearizable signed checkpoint-store CAS authority; and
- immutable object-lock archive returning an Ed25519-authenticated exact
  readback receipt.

The shutdown-aware Torii worker takes the current signed checkpoint and archive
head as its fence, archives at most `compaction_max_records`, and uses a stable
operation id for exact retry. The service verifies canonical readback, its own
signed payload/head commitment, the configured archive identity, and the
archive receipt signature before it removes any expired challenge/session
record or attempts the authoritative checkpoint CAS. A failed or ambiguous CAS
therefore leaves the installed artifact replayable under the same operation id;
forged, trailing, forked, rolled-back, skipped-generation, or provider-drift
readback never authorizes pruning. A completed erasure remains terminal even
when an older expired session is compacted later: compaction must not recreate
a default retention floor for that quarantine object.

Transparency publication is a separate deployment boundary. The node provides
a bounded, signed `EvidenceViewerTransparencyProjectionV1`, but the repository
does not yet construct or run a deployment-owned producer adapter that consumes
it and durably advances its exact cursor.

## API Surface

The catalogued and OpenAPI-described active evidence family is:

| Method | Route | Purpose |
| --- | --- | --- |
| `POST` | `/v1/evidence/session/challenge` | Authorize an exact case tuple and issue a single-use WebAuthn challenge. |
| `POST` | `/v1/evidence/session` | Consume the challenge from the sensitive `X-SoraFS-Evidence-Challenge` header and the bounded assertion body, then issue a case-bound session plus initial rotating grant. |
| `GET` | `/v1/evidence/manifest/{session_id_hex}` | Return the canonical payload-free manifest and append a signed access receipt. |
| `GET` | `/v1/evidence/segment/{session_id_hex}` | Authenticate, decrypt, durably receipt, and return one bounded byte range. |
| `POST` | `/v1/evidence/log/{session_id_hex}` | Append a signed payload-free browser interaction. |
| `GET` | `/v1/evidence/audit` | Page one exact signed checkpoint using its required digest, an explicit page limit, and an optional exact receipt predecessor; explicit auditor or legal role required. |
| `GET` | `/v1/evidence/status` | Read the exact signed checkpoint anchor and bounded counts before beginning or restarting audit pagination. |
| `POST` | `/v1/evidence/legal-hold` | Place a legal hold and signed receipt. |
| `POST` | `/v1/evidence/legal-hold/{hold_id_hex}/release` | Release a legal hold and signed receipt. |
| `GET`, `POST` | `/v1/evidence/retention` | Read due candidates or record a signed retention decision. |
| `POST` | `/v1/evidence/erasure` | Perform legal-hold-aware irreversible erasure and issue a signed receipt. |

The only retained legacy aggregate-audit routes are authenticated retirement
tombstones, not compatibility implementations:

| Method | Route | Purpose |
| --- | --- | --- |
| `POST` | `/v1/sorafs/moderation/viewer-audit-reports` | Require canonical request authentication and the moderation-operator role, then return HTTP `410 Gone`; the retired body is not parsed and no local audit state is changed. |
| `POST` | `/v1/sorafs/moderation/viewer-audit-reports/publish-due` | Require the same authentication and authorization, then return HTTP `410 Gone`; it does not run a scheduler or publish a report. |

The deleted `/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-sessions`
and
`/v1/sorafs/moderation/quarantine/{quarantine_id_hex}/viewer-access`
routes are not compatibility aliases and are absent from routing, the route
catalogue, and OpenAPI.

The embedded shell is served at `/v1/evidence/viewer`, with same-origin script
and stylesheet assets. It contains no evidence, account data, or bearer
material and is intentionally unprojected from SDK/OpenAPI generation.

JSON requests use Norito JSON decoding with unknown fields denied and bounded
base64/hex inputs. Durable and audit responses include byte-identical canonical
Norito envelopes. Range offsets are start-inclusive/end-exclusive and are
bounded by configuration. Mutating or receipt-producing requests require a
non-zero idempotency key; conflicting or replayed keys fail closed.

## Receipt and Durability Contract

Every session issuance, manifest access, range access, browser interaction,
legal-hold transition, retention decision, erasure completion, and
legal-hold-based erasure denial appends an Ed25519-signed receipt. Receipts have:

- one global monotonic sequence;
- the previous receipt digest;
- the exact event kind and timestamp;
- object and evidence digests;
- actor-account and idempotency-key digests;
- a request metadata digest; and
- optional range bounds.

Checkpoint loading requires canonical Norito re-encoding, bounded sequences,
unique sorted identities, an unbroken receipt chain, signed record linkage, and
valid receipt and checkpoint-record signatures under the configured signer
identity. The external store enforces monotonic predecessor CAS over the
deterministic signed record revision. Reported-success and ambiguous CAS
results require exact authoritative readback; an unchanged predecessor is a
safe rejection, while a different successor or failed readback poisons local
state until restart. The local cache retains the hardened no-follow,
same-directory atomic replacement, symlink, hardlink, and replacement
defenses, but it is never accepted as authority.

The signed record in the external checkpoint authority is the sole durable
audit record. Its signed public anchor binds the complete canonical checkpoint
digest, receipt count, and exact receipt-chain head. A consumer first obtains
and verifies that anchor from
`/v1/evidence/status`, then supplies its non-zero
`expected_checkpoint_digest_hex` on every `/v1/evidence/audit` request. The
bounded transparency projection also requires an explicit page limit and an
optional exact predecessor cursor containing both receipt sequence and receipt
digest. It returns a contiguous page of signed receipts and binds the signed
anchor, requested page limit, page, next cursor, and continuation marker in the
projection digest and canonical Norito response.

The accepted raw query forms are intentionally singular:

```text
expected_checkpoint_digest_hex=<64-lowercase-hex>&limit=<1..256>
expected_checkpoint_digest_hex=<64-lowercase-hex>&after_sequence=<nonzero-canonical-decimal>&after_receipt_digest_hex=<64-lowercase-hex>&limit=<1..256>
```

Field reordering, percent or plus aliases, duplicate/empty/unknown fields,
implicit limits, leading-zero numbers, uppercase hex, and partial predecessor
pairs fail closed. If the durable checkpoint changes during pagination, Torii
returns HTTP `409 Conflict`; the consumer must fetch and verify the new status
anchor and explicitly restart instead of silently mixing pages. Consumers must
durably retain the exact signed checkpoint anchor and `(sequence, digest)`
cursor. An unknown digest, same-sequence substitution, malformed anchor,
signature change, local cache fork, or cache rollback beyond the single
verified predecessor fails closed. The authenticated audit and status routes
are read projections of the checkpoint and do not call the signer. Neither
legacy tombstone can create or modify audit state.

A signature proves anchor integrity and governed signer identity, not global
freshness to a first-time client. Such a client cannot distinguish an older
validly signed anchor from the latest anchor without an independently
authenticated transparency or ledger head. Binding and publishing that
monotonic external head remains part of the deployment-owned transparency
producer blocker below.

For a range request, decryption completes first but bytes are not returned until
the signed access receipt and rotated grant state are durably committed. Erasure
holds the state boundary across legal-hold evaluation and the irreversible KMS
operation, preventing a hold/erasure race. A definite erasure followed by an
ambiguous checkpoint result leaves the service fail-closed. Restart and live
refresh reconcile retained erasure intents by their stable operation ids; an
unavailable or ambiguous provider, a zero commit digest, a finalize failure, or
a partial multi-intent result immediately marks process durability uncertain
and blocks subsequent reads until authoritative restart reconciliation.
Likewise, a poisoned quarantine-object index is surfaced as unavailable state,
never converted into a false object-not-found result.

## Embedded Viewer Controls

The embedded viewer shell is delivered with private `no-store`/`no-cache`
headers, same-origin isolation, no-referrer, a strict CSP, `worker-src 'none'`,
`connect-src 'none'`, no object/media/image sources, and restrictive
Permissions-Policy. It does not register a service worker, use Cache Storage,
IndexedDB, local storage, download URLs, or offline persistence.

The shell accepts already-authorized `ImageBitmap` frames only from its
same-origin parent, renders them to a canvas, closes transferred frames, and
clears the canvas on page hide. It includes a trauma warning, visible repeating
per-session watermark, print suppression, and instrumentation for view, pause,
download-attempt, and screenshot-attempt events. The authenticated parent is
responsible for signing range/log requests and forwarding those interaction
events to `/v1/evidence/log/{session_id_hex}`.

## Remaining Production Blockers

- Package and operate the stock local runtime-provider broker service. Its
  bounded canonical client/server protocol now covers all six viewer slots
  (22–26 and 47), including exact public qualification metadata, replay-safe
  operation identities, ambiguity typing, signed readback, and authoritative
  CAS/archive verification; it does not itself supply deployment credentials
  or vendor backends.
- Construct and inject deployment-owned WebAuthn, rotating-grant, PKCS#11/HSM
  receipt-signer, KMS/erasure, linearizable sealed-CAS checkpoint-store, and
  immutable object-lock/archive implementations that satisfy the shipped
  qualification and exact-readback contracts; collect external readiness,
  rotation, revocation, CAS, archive durability, rollback, and recovery
  evidence for those real services.
- Build a deployment-owned transparency producer adapter that consumes only
  `EvidenceViewerTransparencyProjectionV1`, durably acknowledges the exact
  signed checkpoint anchor and `(sequence, digest)` cursor, anchors a monotonic
  public head for first-contact freshness, and reconciles publication. A
  Torii-local scheduler is not part of this design.
- Deploy and qualify a real messaging/portal provider for the shipped
  retry-safe surrounding panel-notification path, then exercise reconciliation,
  dead-letter handling, outage, restart, and failover with the protected viewer.
- Deploy at least two Torii replicas against the same qualified checkpoint
  authority and prove stale-reader fencing, concurrent CAS rejection,
  ambiguous-result readback, restart adoption, rollback resistance, and
  identical post-recovery state.
- Deploy the supervised compaction worker against at least two replicas sharing
  the real archive/checkpoint authorities and prove exact replay after archive
  success plus checkpoint failure, archive-signature rejection, bounded
  pruning, recovery, and receipt-chain/exact-cursor continuity.
- Collect reviewed multi-instance deployment, security, recovery, and
  payload-free promotion evidence.

## Validation and Promotion

The focused implementation checks are:

```sh
cargo test -p sorafs_node evidence_viewer
cargo test -p iroha_torii evidence_viewer --features app_api
cargo test -p iroha_torii openapi --features app_api
cargo test -p iroha_torii_shared route_catalog
```

Release validation must additionally exercise unauthorized and operator-only
accounts, revoked assignments, challenge/assertion/grant replay, expiry and
rotation, wrong case/object/policy binding, malformed and oversized inputs,
range substitution, missing/substituted/test-marked/stale provider startup,
provider revision and policy drift before and during operations,
receipt/signature/chain tampering, authoritative checkpoint substitution,
stale/forked CAS heads, ambiguous CAS readback, local-cache corruption and
filesystem attacks, crash points around persistence, concurrent
legal-hold/erasure requests, KMS ambiguity, viewer CSP/offline controls, and
payload/secret log scanning.

Deployment promotion remains gated by:

```sh
python3 scripts/build_sorafs_evidence_viewer_canary.py \
  @scripts/examples/sorafs_evidence_viewer_canary.args.example
python3 scripts/check_sorafs_moderation_panel_rollout_evidence.py \
  @scripts/examples/sorafs_moderation_panel_rollout_evidence.args.example \
  --require-kind evidence_viewer
```

`scripts/build_sorafs_evidence_viewer_canary.py` is the payload-free `evidence_viewer` canary builder. It requires reviewed `moderation-viewer-session-*` `--viewer-session` labels, emits `role_count`,
`security_control_count`, `access_event_kind_count`, and
`export_target_count` from the reviewed role/control/event/export inventories
before checker prevalidation, and rejects unreviewed `--deployment-id` and
`--environment` values before checker prevalidation. Promotion evidence must
include explicit audit-log tamper rejection and watermark metadata mismatch
rejection.

The canary and aggregate evidence must remain payload-free. Raw evidence,
assertions, credential identifiers, grants, signed URLs, response bodies,
watermark secrets, legal-hold authority payloads, and transparency report
payloads must never enter readiness artifacts.
