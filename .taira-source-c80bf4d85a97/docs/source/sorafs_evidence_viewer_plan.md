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
head. A missing installation must durably create this signed genesis anchor
before the service becomes available. Non-receipt state therefore cannot be
altered independently of the receipt chain. Audit reads return the retained
anchor without calling the signer. Secret wrapper types redact debug output and
scrub owned buffers on drop.

## Runtime Dependencies and Configuration

Production behavior is configured under
`sorafs.storage.evidence_viewer`. Enabling it requires SoraFS storage and
all of these non-secret policy values:

- an absolute private checkpoint file and bounded checkpoint size;
- session, challenge, and grant lifetimes within the 15-minute ceiling;
- an authenticated range limit and bounded collection limits;
- the WebAuthn relying-party id and exact canonical HTTPS origins;
- opaque production handles for WebAuthn, grant, erasure, and Ed25519 receipt
  signing services; and
- the exact governed Ed25519 receipt-verification key.

Torii exposes runtime injection seams whose opaque handles and public key must
match configuration exactly. Startup fails closed for missing, partial, or
mismatched dependencies. There is no file key, environment secret, `KeyPair`,
or in-process production fallback. The standard launcher intentionally does not
construct real WebAuthn, grant, HSM signer, or KMS implementations; deployment
owners must construct and inject those runtime dependencies.

The injected boundaries are:

- finalized moderation authorization reader;
- WebAuthn challenge and assertion verifier;
- rotating-grant issuer, verifier, and revoker;
- PKCS#11/HSM Ed25519 receipt signer; and
- KMS or cryptographic-erasure service.

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
valid receipt and checkpoint-envelope signatures under the configured signer
identity. Atomic checkpoint writes use the hardened local checkpoint
implementation, including symlink, hardlink, and replacement defenses. A
post-rename durability ambiguity makes the service unavailable rather than
allowing unrecorded access.

The signed checkpoint is the sole durable audit record. Its signed public
anchor binds the complete canonical checkpoint digest, receipt count, and exact
receipt-chain head. A consumer first obtains and verifies that anchor from
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
signature change, or local checkpoint rollback fails closed. The authenticated
audit and status routes are read projections of the checkpoint and do not call
the signer. Neither legacy tombstone can create or modify audit state.

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
ambiguous checkpoint result leaves the service fail-closed.

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

- Construct and inject deployment-owned WebAuthn, rotating-grant, PKCS#11/HSM
  receipt-signer, and KMS/erasure implementations, with startup identity and
  readiness checks.
- Build a deployment-owned transparency producer adapter that consumes only
  `EvidenceViewerTransparencyProjectionV1`, durably acknowledges the exact
  signed checkpoint anchor and `(sequence, digest)` cursor, anchors a monotonic
  public head for first-contact freshness, and reconciles publication. A
  Torii-local scheduler is not part of this design.
- Add durable, retry-safe notification delivery for the surrounding
  evidence-viewer workflow, including reconciliation and dead-letter handling.
- Replace single-process checkpoint serialization with multi-instance
  compare-and-swap or an equivalent single-writer lease so two Torii instances
  cannot allocate the same receipt sequence or overwrite each other's state.
- Add bounded compaction and authenticated archive/recovery while preserving
  receipt-chain verification and exact-cursor continuity.
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
range substitution, receipt/signature/chain tampering, checkpoint corruption
and filesystem attacks, crash points around atomic persistence, concurrent
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
