---
title: Proof-of-Personhood Credential Pipeline
summary: SFM-4b1 implementation status for PoP credential foundations and the remaining issuer, verifier, and juror proof services.
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
issuer bundle to use a canonical lowercase `pop-issuer-*` identifier without
non-production markers, to match the published commitment root and revocation registry,
and requires juror-client, verifier-service, moderation-integration,
metrics/alert, and governance artifacts to carry the same root and
revocation-list digests, so rollout packets cannot mix evidence from different
credential publication runs. Root or revocation-list disagreements mark the
offending anchor and downstream artifacts invalid in the emitted summary, not
only the top-level promotion status. This checker is a rollout gate; it does
not replace the missing runtime services or privacy proof backend.
Verifier-service artifacts also publish `policy_digest_hex`; governance
approval artifacts must bind `policy_digest_hex` to that valid verifier policy
digest, and the checker emits those valid verifier policy digests as
`valid_policy_digests`. Valid juror-client artifacts now publish their reviewed
root/revocation sync tuple as `valid_juror_sync_bindings`, and valid
moderation-integration artifacts publish `pop_snapshot_digest_hex` values as
`valid_pop_snapshot_digests`; the aggregate production-readiness gate accepts
both only as payload-free metadata tethered to recognized artifact
fingerprints. The aggregate production-readiness gate also rechecks the
juror-client sync tuple before final promotion: synced roots in
`valid_juror_sync_bindings` must appear in `valid_root_digests`, and synced
revocation lists must appear in `valid_revocation_list_digests`.
Aggregate promotion also rechecks the lane-proven PoP digest relationships:
root-bound artifact fingerprints must match `valid_root_digests`,
revocation-bound artifact fingerprints must match
`valid_revocation_list_digests`, and policy-bound artifact fingerprints must
match `valid_policy_digests` before final promotion can report ready.
Issuer-bundle artifacts also bind `credential_count` to the unique
canonical `credentials[].name` inventory, require reviewed lowercase
`pop-credential-*` labels without non-production markers, and reject duplicate
credential entries before promotion can report ready. Revocation-registry artifacts also bind `revoked_nonce_count` to the unique canonical `revoked_nonce_refs[].name` inventory, require reviewed `pop-revoked-nonce-*` labels without non-production markers, keep raw revoked nonce payloads excluded, and reject duplicate revoked-nonce refs before promotion can report ready. Enrollment-portal artifacts also bind
`route_count` to the unique canonical `routes[].name` inventory and reject
duplicate or unknown route entries before promotion can report ready. Verifier-service
artifacts also bind `route_count` to the unique canonical `routes[].name`
inventory and `proof_probe_count` to the unique canonical `probes[].name`
inventory, require accepted and rejected proof counts to match the
`probes[].accepted` partitions, require accepted probe labels to use reviewed
`pop-valid-proof-*` names and rejected probe labels to use reviewed
`pop-invalid-proof-*` names without non-production markers, and reject duplicate or
unknown route entries and duplicate proof-probe entries before promotion can
report ready. Verifier-service artifacts must explicitly set
`raw_proofs_included` and `holder_identity_disclosed` to `false`, and
metrics/alert artifacts must explicitly set `critical_alerts_firing` and
`response_bodies_included` to `false`, before promotion can report ready.
PoP credential payload-safety artifacts must explicitly set
`credential_payloads_included`, `holder_identities_included`,
`credential_leaves_included`, `rollback_detected`,
`revoked_nonces_included`, `pii_fields_included`, `attestations_included`,
`holder_identity_included`, `proof_payloads_included`,
`raw_proofs_included`, `holder_identity_disclosed`,
`identity_payloads_included`, `critical_alerts_firing`, and
`response_bodies_included` to `false` before promotion can report ready.
Moderation-integration artifacts also bind
`sortition_probe_count` and `commit_reveal_probe_count` to the unique canonical
`sortition_probes[].name` and `commit_reveal_probes[].name` inventories,
require reviewed lowercase `pop-sortition-probe-*` and `pop-commit-reveal-probe-*`
labels without non-production markers, and reject duplicate moderation-probe
entries before promotion can report ready.
Metrics/alert artifacts also bind `metric_count` to the unique canonical
`metrics` inventory, require the reviewed POP metrics set, and reject duplicate
or unknown metric entries before promotion can report ready.
The summary exports the sorted reviewed `metrics` inventory plus
`metric_count_values`, and the aggregate production-readiness gate requires
those fields to match the metrics/alert artifact fingerprint before final
promotion can report ready.
`scripts/run_sorafs_pop_credentials_rollout_evidence.py` provides the matching
collection planner/runner for reviewed staged evidence. It accepts explicit
payload-free canary artifacts, supports shell-style `@ARGFILE` inputs, forwards
the gate thresholds, prints a dry-run command plan with the checker-backed
`evidence_contract` map for the selected required kinds, and invokes the
checker with a reproducible summary path. The checker exports those required
top-level payload fields as `EVIDENCE_REQUIRED_FIELDS` for downstream
automation.
`scripts/build_sorafs_pop_credentials_canary.py` is a payload-free SFM-4b1 PoP credential canary builder
for issuer bundle, commitment root, revocation registry, enrollment portal,
juror client, verifier service, moderation integration, metrics/alerts, and
governance approval evidence. It takes reviewed deployment facts, requires every
positive proof claim and required credential/proof-probe/moderation-probe/route/metric coverage explicitly, forces raw
credential, holder-identity, proof, attestation, response-body, and revocation
nonce payload flags to `false`, validates each generated artifact through the
PoP rollout gate, requires explicit `body_blake3_hex` response digest evidence
for enrollment and verifier routes, requires reviewed policy-digest input for verifier-service
and governance-approval evidence, requires reviewed `--credential` labels whose
unique inventory matches `--credential-count` for issuer bundles and uses
`pop-credential-*` production labels without non-production markers, requires
reviewed `--accepted-proof-probe` and `--rejected-proof-probe` labels whose
unique inventories match the verifier proof counts and use partitioned
`pop-valid-proof-*`/`pop-invalid-proof-*` production labels without non-production
markers, requires reviewed
`--sortition-probe` and `--commit-reveal-probe` labels whose unique inventories
match the moderation-integration probe counts and use
`pop-sortition-probe-*`/`pop-commit-reveal-probe-*` production labels without
non-production markers, rejects duplicate or unknown `--verified-claim`, route,
and metric closed-set inputs plus malformed, generic-family, or
non-production `--issuer-id` values before writing, and
writes atomically without following output symlinks. The
builder is an evidence packaging aid; it does not replace the missing issuer,
registry, juror client, verifier service, or production privacy-preserving proof
backend.

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
  verifier-service policy digest anchors, emits them as `valid_policy_digests`,
  and requires governance approval `policy_digest_hex` to match one of those
  valid verifier policies. Enrollment-portal artifacts also bind `route_count`
  to the unique canonical `routes[].name` inventory and reject duplicate or
  unknown route entries before promotion can report ready. Enrollment-portal and
  verifier-service route responses must also carry a `body_blake3_hex` digest
  before readiness can report ready. Moderation-integration artifacts
  also bind `sortition_probe_count` and `commit_reveal_probe_count` to reviewed
  `pop-sortition-probe-*` and `pop-commit-reveal-probe-*` probe inventories without
  non-production markers before promotion can report ready. It supports
  shell-style `@ARGFILE` inputs so reviewed operator evidence paths can be
  replayed without
  storing runtime secrets in the repo.
- `scripts/run_sorafs_pop_credentials_rollout_evidence.py` composes reviewed
  evidence paths into a single verifier invocation, checks required files before
  running, supports subset gates for staged operator drills, and emits
  `sorafs.pop_credentials.rollout_evidence_collection_plan.v1` dry-run plans
  with checker-backed `evidence_contract` maps for rollout review.

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
  For reviewed local canary packaging, generate payload-free artifacts with
  `scripts/build_sorafs_pop_credentials_canary.py
  @scripts/examples/sorafs_pop_credentials_issuer_canary.args.example` and
  `scripts/build_sorafs_pop_credentials_canary.py
  @scripts/examples/sorafs_pop_credentials_verifier_canary.args.example` before
  passing the generated evidence files to the rollout gate. Issuer canaries bind
  `credential_count` to reviewed `pop-credential-*` `credentials[].name` inventory
  without non-production markers before local
  evidence can be generated. Verifier canaries bind `route_count` to reviewed `routes[].name`
  inventory without unknown routes, bind `proof_probe_count` to reviewed
  partitioned `pop-valid-proof-*`/`pop-invalid-proof-*` `probes[].name` inventory, and
  bind accepted/rejected proof counts to the reviewed `probes[].accepted`
  partitions before local evidence can be generated. Moderation-integration canaries bind `sortition_probe_count` and
  `commit_reveal_probe_count` to reviewed `pop-sortition-probe-*` and
  `pop-commit-reveal-probe-*` probe inventories without non-production markers
  before local evidence can be generated.
  Production promotion remains blocked unless the summary status is `ready`
  and includes a production privacy-preserving proof backend rather than the
  local transcript-digest policy foundation. The aggregate production-readiness
  gate also rechecks `valid_juror_sync_bindings` against `valid_root_digests`
  and `valid_revocation_list_digests` before final promotion can report ready.
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
  scripts/tests/run_sorafs_pop_credentials_rollout_evidence_test.py \
  scripts/tests/build_sorafs_pop_credentials_canary_test.py
```

Add dedicated service, CLI, and deployed verifier tests when the issuer,
registry, juror client, and privacy-proof backend land.

The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier. The shared runner plan guard also rejects non-canonical nested required-kind, threshold, external-evidence, evidence-contract, and command-step shapes before dry-run output or verifier execution.
