---
lang: ur
direction: rtl
source: docs/source/sorafs_pop_credentials_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 36bf8486013b9ea169ceda85dca31c82b2e10ea345a508fd32247d276fa73e94
source_last_modified: "2026-01-03T18:08:01.841449+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Proof-of-Personhood Credential Pipeline
summary: SFM-4b1 implementation status for PoP credential foundations and the remaining issuer, verifier, and juror proof services.
---

# Proof-of-Personhood Credential Pipeline

## Current Status

SFM-4b1 is not shipped as a SoraFS proof-of-personhood credential service. The
repository has reusable governance and policy-jury data structures that can bind
future juror sortition to a PoP snapshot, but it does not contain the enrollment
portal, credential issuer, credential registry, juror wallet, ZK membership
proof generator, or SoraFS verifier service described by the original plan.

## Existing Foundations

- `iroha_data_model::ministry::jury::PolicyJurySortitionV1` records a
  `pop_snapshot_digest_blake2b_256` and juror `pop_identity` values for policy
  jury draws.
- `PolicyJurySortitionV1::validate` enforces committee size, duplicate juror
  rejection, ordered waitlists, and valid failover references.
- `docs/examples/ministry/policy_jury_roster_example.json` and
  `docs/examples/ministry/policy_jury_sortition_example.json` provide example
  roster and sortition payloads.
- Governance ballot instructions and events can carry policy-jury votes, but
  they are not a SoraFS PoP credential issuer or verifier.

## Target Credential Model

The production SoraFS credential stack still needs a credential format that
binds:

- holder DID or account identity;
- juror eligibility class and optional regional or expertise attributes;
- issuance and expiry epochs;
- revocation nonce;
- issuer signature;
- commitment-tree root and revocation list version;
- proof material that lets sortition and voting services verify membership
  without exposing the holder identity.

The specific signature scheme and ZK backend remain open implementation choices.
When implemented, they must use deterministic Norito payloads, stable domain
separation, and hardware-independent verification.

## Target Runtime Services

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Enrollment portal | Captures candidate attestations and issuer approvals. | Not shipped. |
| Credential issuer | Signs credentials, updates commitment roots, and publishes rollups. | Not shipped. |
| Credential registry | Stores commitment roots, revocation updates, and event digests. | Not shipped. |
| Juror client | Stores credentials, syncs revocations, and generates proofs. | Not shipped. |
| Verification service | Validates juror proofs for sortition, voting, and appeal panels. | Not shipped. |

Do not document `sorafs pop sync`, `sorafs pop status`,
`sorafs pop prove`, or `sorafs pop revoke` as shipped commands until the CLI
handlers and backing services exist.

## Remaining Production Gates

- Define `PopCredentialV1`, commitment-root, revocation, enrollment, renewal,
  and proof payloads in the data model with Norito roundtrip tests.
- Choose and implement the credential signature and membership-proof scheme with
  deterministic verification.
- Build the issuer and registry services, including key management, revocation
  updates, commitment-root publication, and audit digests.
- Build juror client storage, revocation sync, proof generation, and local
  credential rotation.
- Integrate proof verification with SoraFS moderation sortition and
  commit-reveal voting.
- Add negative tests for expired credentials, revoked nonces, wrong roots,
  stale revocation lists, replayed proofs, and forged issuer signatures.
- Publish operator and juror docs only after the service, CLI, and verifier
  paths exist.

## Validation

Current validation is limited to the reusable policy-jury foundations:

```sh
cargo test -p iroha_data_model policy_jury
```

Add dedicated SFM-4b1 tests when the credential payloads and services land.
