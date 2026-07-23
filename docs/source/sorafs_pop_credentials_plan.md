---
title: Proof-of-Personhood Credential Pipeline
summary: SFM-4b1 implementation status for PoP credential foundations and the remaining issuer, verifier, and juror proof services.
---

# Proof-of-Personhood Credential Pipeline

## Current Status

SFM-4b1 now has canonical PoP credential payloads, a production cryptographic
membership-proof backend, a consensus-owned issuer/registry foundation, a
durable issuer/wallet service, and the canonical authenticated Torii V1 API,
but it is not yet deployable from the standard `irohad` binary. The
first-release backend is a
fixed Halo2 circuit over Pasta with transparent IPA polynomial commitments. It
proves membership in the signed active credential root and empty-leaf membership
in the signed sparse revocation root while keeping the credential id, holder
commitment, revocation nonce, holder secret, and both authentication paths out
of the public payload. The verifier also binds eligibility class, verifier
challenge and context, credential expiry, root/list versions, and a
replay-resistant nullifier.

`crates/sorafs_node/src/pop_credentials.rs` now owns encrypted enrollment,
dual-control approval, HSM-backed issuance, durable registry outbox/dead-letter
and finalized reconciliation, encrypted wallet custody, witness
synchronization, local proof generation, and exactly-once nullifier
consumption. `crates/iroha_torii/src/sorafs/pop_api.rs` exposes the exact
14-route V1 family for enrollment, approval, issuance, revocation, registry,
wallet, proof generation, and verification. Every route has a strict
unknown-field-denying request, a route-specific body ceiling, bounded canonical
Norito decoding, action/request-bound authentication, payload-free errors, and
no-store responses. `iroha_config` is the sole source of non-secret policy and
resource bounds. HSM, KMS, authentication, private issuance/witness, ledger,
and finalized-time providers are runtime-only dependencies.

The remaining local release blocker is `V1-BLOCK-POP-RUNTIME-01`: the standard
`irohad` entrypoint does not yet construct and inject the governed production
provider bundle, and the repository has no deployable shared external-runtime
or sidecar adapter for those providers. Enabling
`torii.sorafs.storage.pop_credentials` without that injected runtime fails
startup; no file-key, environment-key, software-signing, or process-clock
fallback is permitted. The native registry remains available through signed
transactions and typed queries. The authoritative moderation intake now pins
its active root, revocation list, issuer-policy digest, and audit head; juror
enrollment verifies exact-canonical Halo2 membership proofs against that
snapshot and persists only a proof digest, deterministic appeal nullifier,
public eligibility class, expiry, and account binding. It rejects observers,
expired credentials, wrong or rotated roots, revocations, duplicate accounts,
and duplicate-person nullifiers before panel sortition. This integration is not
a claim that the missing external runtime adapter or reference deployment is
complete.
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
not replace the missing standard-`irohad` external-runtime adapter, HSM/KMS
integration, or deployed verifier evidence.
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
PoP rollout summaries must expose exactly one active root digest, revocation-list
digest, verifier policy digest, and moderation PoP snapshot digest; mixed valid
anchors fail closed before downstream evidence can satisfy the rollout gate.
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
registry, juror client, or deployed verifier service.

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
- `iroha_data_model::sorafs::pop_registry` defines a governance-controlled
  `PopIssuerPolicyV1`, payload-free credential commitments, signed root and
  revocation publication records, per-revocation commitment records,
  constant-time registry status, and deterministic audit-chain links. Policy
  revisions are predecessor-digest chained and hard-bound to a canonical
  `pop-issuer-*` id, exact universal issuer account, Ed25519 publication key,
  credential lifetime, clock skew, and bounded issuance/revocation limits.
- `SetSorafsPopIssuerPolicy`, `CommitSorafsPopCredentialBatch`, and
  `PublishSorafsPopRevocationList` provide consensus-owned state transitions.
  Issuance stores no credential body: it atomically commits only the exact
  signed root/revocation snapshot and domain-separated credential/nonce
  commitments. Every issuer operation binds the exact active policy digest;
  root and list versions must advance exactly, new roots must
  name the active predecessor, issuance snapshots must preserve every prior
  revocation, and later revocation lists must be strict signed supersets whose
  new nonces bind to registered credential commitments. Duplicate credentials,
  double revocations, unknown nonce bindings, stale roots, rollback,
  noncanonical Norito, and oversized batches fail before state mutation.
- `CanManageSorafsPopRegistry` and `CanOperateSorafsPopIssuer` protect policy
  and issuer transitions in both the default executor and native execution.
  Typed `FindSorafsPop*` queries expose the policy, payload-free commitment and
  revocation records, signed publications, audit links, and registry status as
  public transparency state through the existing generic query API. There are
  no dedicated PoP Torii routes or operator CLI commands yet.
- `prove_pop_membership_v1` creates a zero-knowledge Halo2/IPA proof from the
  signed credential plus fixed-depth private credential and sparse-revocation
  paths. `verify_pop_membership_proof_v1` verifies the signed active root and
  revocation publication, pinned transparent parameter and verifying-key
  fingerprints, exact expected challenge/context, expiry, replay cache, and
  cryptographic proof. The retired transcript-digest proof variant and policy
  verifier have been removed rather than retained as a compatibility surface.
- The native SoraFS moderation appeal lifecycle snapshots the exact active
  registry publications at intake, revalidates those immutable historical
  root/list/audit records after later registry advancement, verifies private
  membership proofs locally, and uses
  the deterministic per-credential appeal nullifier as the only candidate
  material in its domain-separated sortition score. Randomized proof bytes and
  caller-selected account strings cannot grind rank. Accepted proof payloads,
  credential bodies, witness paths, holder secrets, and revocation nonces are
  not retained. Nullifier replay, missing or mutated snapshot records, and
  detached audit anchors fail closed; valid later root/list rotations cannot
  rewrite or brick an already-admitted appeal. An active emergency registry
  pause still fails closed for pending appeals until governance resumes it.
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

Ed25519 is the issuer and publisher signature scheme for the V1 payload
foundation. Halo2/Pasta/IPA is the fixed first-release membership-proof backend;
proof generation uses operating-system randomness for zero knowledge, while
verification, public-input derivation, Poseidon parameters, tree folding, and
all accept/reject results are deterministic across supported hardware.

## First-Release Privacy Proof Contract

- The credential tree has a fixed depth of 32. The signed root publication
  carries that depth and rejects any other value.
- Revocation uses a fixed-depth 128-level sparse tree keyed by canonical,
  uniformly random, non-zero 128-bit little-endian nonces. An empty leaf is
  zero; a revoked leaf is domain-separated from internal nodes. The signed
  revocation publication carries the computed sparse root and rejects a root
  that disagrees with its bounded, sorted entry snapshot.
- Credential leaves bind the holder commitment, private credential id,
  eligibility class, issuance/renewal/expiry epochs, revocation nonce,
  credential-tree version, issuance-time revocation-list version, issuer/key
  binding, and committed attributes. The circuit derives the holder commitment
  from the private holder secret instead of accepting it as a public input.
- Public inputs have one canonical order: credential root, credential-tree
  version, eligibility class, challenge digest projection, verifier-context
  projection, expiry, sparse revocation root, current revocation-list version,
  and nullifier. Reordering any input invalidates the Halo2 transcript.
- The nullifier is derived in-circuit from the holder secret and verifier
  challenge/context. Verifiers must pass the expected challenge and context and
  atomically record an accepted nullifier in their replay store.
- Proof payloads contain no credential id, holder commitment, revocation nonce,
  holder secret, credential path, or revocation path. Reference validation emits
  only public roots/versions, class, challenge/context, nullifier, proof-byte
  length, and pinned parameter/verifying-key fingerprints.
- V1 fixes `k = 14`, credential depth 32, revocation depth 128, verifier context
  at 256 UTF-8 bytes, proof transcripts at 128 KiB, revocation snapshots at
  4,096 entries, credentials and enrollment requests at 64 attribute keys,
  attribute keys at 128 UTF-8 bytes, issuer/applicant identifiers at 256 UTF-8
  bytes, and the slice replay API at 65,536 nullifiers. Inputs outside those
  bounds fail before proof construction or expensive verification.

## Target Runtime Services

| Component | Responsibility | Local state |
|-----------|----------------|-------------|
| Enrollment portal | Captures encrypted candidate enrollment and governed approvals. | The authenticated submit/status/approval API and durable encrypted workflow are shipped; operator UI, WebAuthn enrollment ceremony, and the deployable external authenticator adapter remain open. |
| Credential issuer | Signs credentials, updates commitment roots, and publishes rollups. | The bounded durable service, HSM interface, strict policy binding, issuance/revocation APIs, and retry-safe outbox are shipped; standard-`irohad` HSM/KMS/provider wiring and deployment evidence remain open. |
| Credential registry | Stores commitment roots, revocation updates, and event digests. | Consensus-owned state, typed queries, authenticated submit/reconcile/projection APIs, cursor rollback rejection, and durable reconciliation are shipped; standard-daemon transaction/reader adapters and multi-peer evidence remain open. |
| Juror client | Stores credentials, syncs revocations, and generates proofs. | Encrypted KMS-wrapped wallet custody, delivery/import/acknowledgement, witness synchronization, and local proof APIs are shipped; a deployable KMS/witness adapter and operator client remain open. |
| Verification service | Validates juror proofs for sortition, voting, and appeal panels. | The Halo2/IPA verifier, atomic nullifier replay defense, native moderation integration, authenticated verification API, `sorafs-validate pop`, and SDK/bridge reference gate are shipped; the external runtime adapter and reviewed deployment evidence remain open. |

Do not document `sorafs pop sync`, `sorafs pop status`,
`sorafs pop prove`, or `sorafs pop revoke` as shipped commands until the CLI
handlers and backing services exist. The standalone `sorafs-validate pop`
reference validator is shipped only for local/CI payload validation.

## Runtime Adapter Blocker Runbook

`V1-BLOCK-POP-RUNTIME-01` blocks local completion and promotion. The standard
daemon builds `ToriiRuntimeDeps` in `crates/irohad/src/main.rs`, but no
production caller currently supplies
`PopCredentialRuntimeSecretsV1`. The missing deployable shared
external-runtime/sidecar adapter must provide:

- the runtime-only enrollment and wallet hybrid recipient secrets;
- the governed issuer HSM signer and KMS/PKCS#11 wallet-key wrapper;
- the action/request-bound API authenticator;
- the ledger transaction submitter and finalized registry reader;
- private issuance-draft and wallet-witness providers; and
- the finalized-chain time provider with an independent clock observation.

The adapter must bind every non-secret handle and public key to the exact
`iroha_config` policy, use the local queue/state or an authenticated equivalent
for transaction submission and finalized reads, participate in supervised
startup/shutdown, and expose only stable payload-free failures. Secret bytes,
PINs, bearer material, credentials, witnesses, attestations, and PII must not
come from `iroha_config`, environment overrides, repository files, or logs.

Closure requires a standard-`irohad` or explicitly packaged reference-daemon
startup test with PoP enabled, provider-unavailable and config/runtime-mismatch
negatives, HSM/KMS/authenticator and finalized-time rotation/rollback tests,
restart reconciliation, and a four-validator reference deployment run. Until
those checks pass, operators must leave
`torii.sorafs.storage.pop_credentials.enabled = false`; the intentional
enabled-without-runtime startup failure must not be bypassed.

## Remaining Production Gates

- Resolve `V1-BLOCK-POP-RUNTIME-01` with the shared deployable external-runtime
  adapter described above; do not add a software-key, file-key, environment, or
  process-clock fallback.
- Deploy the native registry on a reviewed multi-validator environment and
  collect restart, reconciliation, rollback-rejection, key/time rotation, and
  audit-head evidence through the shipped Torii facade.
- Package the operator/juror client around the shipped encrypted wallet,
  revocation synchronization, and local proof APIs without exposing private
  custody material.
- Extend deployment-level negatives around external-provider outage, issuer
  authorization, registry rollback, juror-wallet proof generation, deployed
  moderation submission, multi-peer restart reconciliation, and operator retry
  behavior. Native
  moderation coverage already rejects malformed/noncanonical proofs, wrong and
  rotated roots, replayed nullifiers, observer/expired credentials,
  biased/duplicate rosters, deadline violations, and failed transactions
  without partial state.
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
  and includes the pinned Halo2/Pasta/IPA privacy-proof backend rather than a
  transcript consistency digest. The aggregate production-readiness
  gate also rechecks `valid_juror_sync_bindings` against `valid_root_digests`
  and `valid_revocation_list_digests` before final promotion can report ready.
- Publish operator and juror command documentation only after the still-open
  CLI and shared external-runtime adapter exist.

## Validation

Local validation now covers both policy-jury foundations and PoP payload
foundations:

```sh
cargo test -p iroha_data_model policy_jury
cargo test -p iroha_data_model pop_registry
cargo test -p sorafs_manifest pop -- --nocapture
cargo test -p iroha_core sorafs_pop_registry -- --nocapture
cargo test --locked -p sorafs_node pop_credentials --lib -- --nocapture
cargo test --locked -p iroha_torii --features app_api sorafs::pop_api::tests --lib
cargo test -p connect_norito_bridge sorafs_reference_pop -- --nocapture
python3 -m pytest -q scripts/tests/check_sorafs_pop_credentials_rollout_evidence_test.py \
  scripts/tests/run_sorafs_pop_credentials_rollout_evidence_test.py \
  scripts/tests/build_sorafs_pop_credentials_canary_test.py
```

Add the standard-daemon external-runtime adapter, operator client,
multi-peer deployment, and deployed-verifier tests when those remaining
integrations land.

The runner validates the schema-closed collection-plan envelope before printing dry-run JSON or executing the verifier. The shared runner plan guard also rejects non-canonical nested required-kind, threshold, external-evidence, evidence-contract, and command-step shapes before dry-run output or verifier execution.
