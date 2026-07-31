---
title: Emergency Canon & TTL Policy
summary: V1 governance policy for bounded emergency compliance rules in predecessor-bound threshold-signed gateway catalogs.
---

# Emergency Canon & TTL Policy (MINFO-6a)

Roadmap reference: **MINFO-6a — Emergency canon & TTL policy**.

This document defines the policy metadata, approval, review, and expiry rules
for emergency gateway-compliance decisions. V1 distributes those decisions only
through governed compliance feeds and predecessor-bound, threshold-signed
catalogs. A local file, pack, CLI mutation, or unsigned runtime override is not
a compliance authority.

## Catalog Policy

| Tier | Maximum validity | Review window | Requirements |
|------|------------------|---------------|--------------|
| Standard | 180 days | n/a | The normalized rule records an issuance time and a bounded expiry. Renewal requires a new catalog sequence. |
| Emergency | 30 days | 7 days | The rule cites a pre-approved emergency canon and review reference. The post-facto review deadline is derived deterministically from the issuance time. |
| Permanent | No automatic expiry | n/a | Reserved for a supermajority governance decision and requires a non-empty governance reference. |

The ratified feed policy defines these bounds. Catalog producers and gateways
use integer timestamps and identical validation rules; an operator cannot widen
a TTL through a local configuration or command-line option. Legal/safety holds
take precedence over accepted appeals, which take precedence over baseline
policy.

## Governed Publication Workflow

1. **Prepare source evidence.** The responsible authority creates a bounded,
   normalized feed record containing the subject digest, policy tier, issuance
   and expiry times, canon or governance reference, and evidence digest. Raw
   evidence, credentials, signing keys, and personal data remain outside the
   catalog and logs.
2. **Build the candidate externally.** A governed producer combines approved
   feeds into canonical Norito, assigns a monotonic sequence, binds the current
   promoted catalog as predecessor, and applies the configured freshness and
   size limits.
3. **Collect threshold signatures.** Distinct active, non-revoked Ed25519
   signers approve the exact catalog bytes. Torii and the gateway controller do
   not hold the threshold signing keys.
4. **Stage through Torii.** Operators inspect
   `GET /v1/sorafs/gateway/compliance/feeds/{feed_id}` and
   `GET /v1/sorafs/gateway/compliance/status`, then submit the bounded
   canonical catalog to
   `POST /v1/sorafs/gateway/compliance/stage` with canonical account-request
   authentication.
5. **Acknowledge independently.** Each regional gateway verifies the same
   catalog digest, sequence, predecessor, signer threshold, revocations, and
   validity window before an authorized operator submits
   `POST /v1/sorafs/gateway/compliance/acknowledge`.
6. **Promote atomically.** After the required distinct gateway
   acknowledgements, an authorized operator submits
   `POST /v1/sorafs/gateway/compliance/promote`. The controller atomically
   replaces serving state and retains the previous promoted catalog as
   last-known-good.
7. **Rollback safely.** Use
   `POST /v1/sorafs/gateway/compliance/rollback` only to restore the durable
   last-known-good catalog. Rollback does not rewrite the signed predecessor
   chain or permit a stale sequence to become a new head.

All control routes require the governed gateway-compliance operator role.
Idempotency, durable acknowledgements, bounded history, signer revocation, and
fail-closed freshness checks apply to ordinary and emergency changes alike.

## Emergency Review and Expiry

Emergency rules expire no later than their signed validity bound. The ministry
records the review outcome within seven days, links any accepted appeal or
continuing legal/safety hold, and either lets the rule expire or authorizes a
new successor catalog. Extending an emergency rule requires fresh governance
evidence and a new threshold-signed sequence; promotion never mutates the
previous catalog in place.

## Audit Evidence

Promotion evidence remains payload-free. Record catalog and policy digests,
sequence and predecessor, signer and gateway-acknowledgement counts, review
deadline, status results, promotion or rollback outcome, and governance
approval references. Do not publish feed payloads, matched subjects, private
evidence, tokens, credentials, or signing material.

The compliance status response and public transparency publication should bind
to the same promoted catalog digest. A mismatch, missing acknowledgement,
revoked signer, expired validity window, or stale predecessor blocks promotion.

## Retired Local Surface

V1 has no local gateway-compliance pack format, local Merkle registry, file
reload procedure, catalog mutation CLI, or unsigned compatibility routes. Old
development state is discarded rather than migrated; the governed controller
and its durable signed catalog are the only serving authority.
