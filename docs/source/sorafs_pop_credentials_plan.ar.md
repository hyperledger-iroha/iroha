---
lang: ar
direction: rtl
source: docs/source/sorafs_pop_credentials_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22dbc13f3322ef703f4d1c091a66b8206335411af2e06d8ff92c5d641b980ada
source_last_modified: "2026-06-25T17:19:30+00:00"
translation_last_reviewed: 2026-06-25
---

# Proof-of-Personhood Credential Pipeline

## Current Status

SFM-4b1 now has local PoP credential payload foundations, but it is not shipped
as a complete SoraFS proof-of-personhood credential service. The repository has
reusable governance and policy-jury data structures that can bind future juror
sortition to a PoP snapshot, plus canonical SoraFS credential, commitment-root,
revocation, issued-credential-bundle, enrollment, renewal, and membership-proof
payloads with local validators and reference SDK/CLI/bridge validation. It does
now expose a production-facing membership verifier that fails closed for the
transcript-digest proof system so local policy fixtures cannot be mistaken for
deployed privacy-preserving proofs. It does not yet contain the enrollment
portal, credential issuer daemon, credential registry service, juror wallet,
privacy-preserving ZK membership proof generator, or deployed SoraFS verifier
service described by the original plan.
`scripts/check_sorafs_pop_credentials_rollout_evidence.py` now provides the
SFM-4b1 promotion gate for future deployed evidence. It requires payload-free
issuer-bundle, commitment-root, revocation-registry, enrollment-portal,
juror-client, verifier-service, moderation-integration, metrics/alert, and
governance-approval artifacts before reporting `ready`, and it rejects raw
credentials, proofs, holder identities, secrets, response bodies, stale
root/revocation publications, unaudited transcript-digest-only proof backends,
and missing `iroha_config` governance bindings. The gate also requires the
issuer bundle to match the published commitment root and revocation registry,
and requires juror-client, verifier-service, moderation-integration,
metrics/alert, and governance artifacts to carry the same root and
revocation-list digests, so rollout packets cannot mix evidence from different
credential publication runs. Root or revocation-list disagreements mark the
offending anchor and downstream artifacts invalid in the emitted summary, not
only the top-level promotion status. This checker is a rollout gate; it does
not replace the missing runtime services or privacy proof backend.
`scripts/run_sorafs_pop_credentials_rollout_evidence.py` provides the matching
collection planner/runner for reviewed staged evidence. It accepts explicit
payload-free canary artifacts, supports shell-style `@ARGFILE` inputs, forwards
the gate thresholds, prints a dry-run command plan, and invokes the checker with
a reproducible summary path.

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
- `sorafs_manifest::pop_credentials` defines `PopCredentialV1`,
  `PopCommitmentRootV1`, `PopRevocationListV1`,
  `PopIssuedCredentialBundleV1`, `PopEnrollmentRequestV1`,
  `PopRenewalRequestV1`, and `PopMembershipProofV1` with deterministic Norito
  schemas and local validators.
- The local payload layer signs credentials, commitment roots, and revocation
  lists with Ed25519 over domain-separated BLAKE3 digests of canonical Norito
  bytes, with the signature bytes cleared before hashing.
- `issue_pop_credential_bundle_ed25519_v1` signs a credential, commitment-root
  publication, and revocation-list snapshot together, then verifies issuer id,
  issuer public key, commitment root, tree version, revocation-list version,
  and revoked-nonce consistency before returning the local issuance bundle.
- The local transcript-policy verifier performs deterministic transcript,
  expiry, root, revocation-list, revoked-nonce, and replay/nullifier checks for
  fixtures and reference tooling. The production `verify_pop_membership_proof_v1`
  API rejects `TranscriptDigestV1` with a policy error until a
  privacy-preserving proof backend is selected and implemented.
- `sorafs_manifest::validate_pop_payload_bytes` and `sorafs-validate pop`
  validate Norito-encoded PoP credentials, commitment roots, revocation lists,
  issued-credential bundles, enrollment requests, renewal requests, and
  membership proofs with stable `ValidationOutcomeV1` diagnostics for CI and
  operator tooling.
- `sorafs_reference_validate_pop_json` and
  `connect_norito_sorafs_reference_validate_pop_json` expose the same PoP
  reference validator through the SoraFS C/JNI bridge. Kotlin/JVM, Java Android,
  and Swift now carry `SorafsPopPayloadKind` selectors plus pre-native label and
  timestamp validation for mobile CI/client integration.
- `scripts/check_sorafs_pop_credentials_rollout_evidence.py` validates
  payload-free staged rollout evidence for the issuer, registry, juror client,
  verifier service, moderation integration, metrics, alerts, and governance
  binding. The verifier fails closed if the issuer bundle, commitment-root
  publication, revocation registry, synced juror state, verifier service,
  moderation integration, dashboard metrics, or governance approval disagree on
  the active root or revocation-list digests, and records those cross-artifact
  binding failures on the offending artifacts in the summary. It supports
  shell-style `@ARGFILE` inputs so reviewed operator evidence paths can be
  replayed without
  storing runtime secrets in the repo.
- `scripts/run_sorafs_pop_credentials_rollout_evidence.py` composes reviewed
  evidence paths into a single verifier invocation, checks required files before
  running, supports subset gates for staged operator drills, and emits
  `sorafs.pop_credentials.rollout_evidence_collection_plan.v1` dry-run plans
  for rollout review.

## Target Credential Model

The local V1 credential payload now binds:

- holder commitment, without embedding the canonical account identity;
- juror eligibility class and optional regional or expertise attributes;
- issuance and expiry epochs;
- revocation nonce;
- issuer signature;
- commitment-tree root and revocation list version;
- proof material that lets sortition and voting services verify membership
  without exposing the holder identity.

Ed25519 is the local issuer and publisher signature scheme for the V1 payload
foundation. The privacy-preserving membership-proof backend remains open; when
implemented, it must keep the existing deterministic Norito payloads, stable
domain separation, and hardware-independent verification.

## Target Runtime Services

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Enrollment portal | Captures candidate attestations and issuer approvals. | Not shipped. |
| Credential issuer | Signs credentials, updates commitment roots, and publishes rollups. | Payload signatures and a local issued-credential bundle helper are shipped; service is not shipped. |
| Credential registry | Stores commitment roots, revocation updates, and event digests. | Payload schemas and local bundle validation are shipped; service is not shipped. |
| Juror client | Stores credentials, syncs revocations, and generates proofs. | Not shipped. |
| Verification service | Validates juror proofs for sortition, voting, and appeal panels. | Local transcript-policy payload verifier, production fail-closed proof verifier, `sorafs-validate pop`, and SDK/bridge reference gate shipped; deployed service and ZK verifier are not shipped. |

Do not document `sorafs pop sync`, `sorafs pop status`,
`sorafs pop prove`, or `sorafs pop revoke` as shipped commands until the CLI
handlers and backing services exist. The standalone `sorafs-validate pop`
reference validator is shipped only for local/CI payload validation.

## Remaining Production Gates

- Replace the transcript-digest membership proof foundation with the selected
  privacy-preserving proof system and deterministic verifier, then move
  `verify_pop_membership_proof_v1` from fail-closed policy rejection to the
  selected production proof verification path.
- Build the issuer and registry services, including key management, revocation
  updates, commitment-root publication, and audit digests.
- Build juror client storage, revocation sync, proof generation, and local
  credential rotation.
- Integrate proof verification with SoraFS moderation sortition and
  commit-reveal voting.
- Extend service-level negative tests around issuer authorization, registry
  rollback, juror-wallet proof generation, and moderation integration.
- Collect payload-free rollout evidence with
  `scripts/run_sorafs_pop_credentials_rollout_evidence.py
  @scripts/examples/sorafs_pop_credentials_rollout_collection.args.example`,
  then validate already-collected evidence directly with
  `scripts/check_sorafs_pop_credentials_rollout_evidence.py
  @scripts/examples/sorafs_pop_credentials_rollout_evidence.args.example`.
  Production promotion remains blocked unless the summary status is `ready`
  and includes a production privacy-preserving proof backend rather than the
  local transcript-digest policy foundation.
- Publish operator and juror docs only after the service CLI/API and verifier
  paths exist.

## Validation

Local validation now covers both policy-jury foundations and PoP payload
foundations:

```sh
cargo test -p iroha_data_model policy_jury
cargo test -p sorafs_manifest pop -- --nocapture
cargo test -p connect_norito_bridge sorafs_reference_pop -- --nocapture
python3 -m pytest -q scripts/tests/check_sorafs_pop_credentials_rollout_evidence_test.py \
  scripts/tests/run_sorafs_pop_credentials_rollout_evidence_test.py
```

Add dedicated service, CLI, and deployed verifier tests when the issuer,
registry, juror client, and privacy-proof backend land.
