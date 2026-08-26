# Soracloud V1 Manifest Schemas

This page defines the first deterministic Norito schemas for Soracloud
deployment on Iroha 3:

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

The Rust definitions live in `crates/iroha_data_model/src/soracloud.rs`.

Uploaded-model storage references are intentionally separate from these SCR
deployment manifests. They extend the Soracloud model plane and point at
approved SoraFS manifests rather than being encoded as new service/container
manifests. See `uploaded_private_models.md`.

## Hosted-service reporting epochs

The hosted-service economic lease and its usage-reporting epoch are separate
clocks. `lease_started_height` and `lease_expires_height` use canonical block
height; reporting and unrelated Soracloud audit events cannot advance or stall
economic billing. Every encoded lease, leased volume, reporting-epoch rollover,
and Inrou placement carries
`economic_clock = {"clock":"CanonicalBlockHeight"}`. First-release decoders
reject the retired implicit audit-sequence layout instead of interpreting its
numeric values as heights. The admitted runtime/storage/egress unit prices
remain fixed for one economic lease. An upgrade or rollback that retains that
lease must reject unit-price drift; a reporting rollover never renews or
reprices the lease and never changes a leased volume's start or expiry.

Every newly assigned Inrou reporter must first submit an accepted zero/open
checkpoint for the current `reporting_epoch` before its replica may be marked
as serving. Reporter counters are monotonic and terminal delivery is explicit.
Reports that neither increase bytes nor change open/terminal state are rejected
instead of consuming an audit or state transition.
At exactly 4,096 reporter identities, the exact newly assigned active
public-lane validator may compare-and-swap to `reporting_epoch + 1` only with a
zero/open counter, after every old checkpoint is finalized and none of those
old keys remains placed. The transition folds the exact checked `u128` old
total into `settled_egress_bytes`, opens the trigger checkpoint, and records a
typed audit event atomically. No manager or reconciliation path may force-clear
unknown usage.

Liveness therefore assumes an honest retiring worker can stop, join its final
writes, and deliver its terminal report. A crashed or malicious reporter that
cannot provide that terminal value intentionally stalls rollover until the
missing usage is recovered; the protocol chooses accounting safety over an
administrative data-loss escape hatch.

## Authoritative Inrou placement

An Inrou V1 replica placement is sticky for one economic lease. Reconciliation
retains the exact validator account, peer ID, guest ISA, and
`placement_incarnation`; it never reassigns a stateful replica to another host
or ISA during that lease. If the original advert becomes ineligible, the
required `host_availability` value becomes `Unavailable` and the slot fails
closed. Only the exact original host becoming eligible again may change it back
to `Available`. A new economic lease may select a fresh assignment.

Placement records use strictly increasing, unique slot numbers bounded by
`desired_replica_count`; sparse records are canonical when an earlier slot has
no assignment. Runtime reconciliation removes state for unavailable or
identity/incarnation-mismatched placements. Serving health and Torii routing
require `Available`, the exact placement incarnation and host identity, and
authoritative `Healthy` state.

## Scope

These manifests are designed for the `IVM` + custom Sora Container Runtime
(SCR) direction (no WASM, no Docker dependency in runtime admission).

- `SoraContainerManifestV1` captures executable bundle identity, runtime type,
  capability policy, resources, lifecycle probe settings, and explicit
  required-config exports into the runtime environment or mounted revision
  tree.
- `SoraServiceManifestV1` captures deployment intent: service identity,
  referenced container manifest hash/version, routing, rollout policy, and
  state bindings.
- `SoraStateBindingV1` captures deterministic state-write scope and limits
  (namespace prefix, mutability mode, encryption mode, item/total quotas).
- `SoraDeploymentBundleV1` couples container + service manifests and enforces
  deterministic admission checks (manifest-hash linkage, schema alignment, and
  capability/binding consistency).
- `AgentApartmentManifestV1` captures persistent agent runtime policy:
  tool caps, policy caps, spend limits, state quota, network egress, and
  upgrade behavior.
- `FheParamSetV1` captures governance-managed FHE parameter sets:
  deterministic backend/scheme identifiers, modulus profile, security/depth
  bounds, and lifecycle heights (`activation`/`withdraw`).
- `FheExecutionPolicyV1` captures deterministic ciphertext execution limits:
  admitted payload sizes, input/output fan-in, depth/rotation/bootstrap caps,
  and canonical rounding mode.
- `FheGovernanceBundleV1` couples a parameter set and policy for deterministic
  admission validation.
- `FheJobSpecV1` captures deterministic ciphertext job admission/execution
  requests: operation class, ordered input commitments, output key, and bounded
  depth/rotation/bootstrap demand linked to a policy + parameter set.
  Runtime execution loads the referenced ciphertext envelopes from
  authoritative service state, verifies their commitments and parameter/key
  identifiers, performs the requested FHE operation, then persists the encoded
  output ciphertext envelope and its commitment. Output byte counts are derived
  from the encoded ciphertext bytes, not from deterministic estimates.
- `DecryptionAuthorityPolicyV1` captures governance-managed disclosure policy:
  authority mode (client-held vs threshold service), approver quorum/members,
  break-glass allowance, jurisdiction tagging, consent-evidence requirement,
  TTL bounds, and canonical audit tagging.
- `DecryptionRequestV1` captures policy-linked disclosure attempts:
  ciphertext key reference (`binding_name` + `state_key` + commitment),
  justification, jurisdiction tag, optional consent-evidence hash, TTL,
  break-glass intent/reason, and governance hash linkage.
- `CiphertextQuerySpecV1` captures deterministic ciphertext-only query intent:
  service/binding scope, key-prefix filter, bounded result limit, metadata
  projection level, and proof inclusion toggle.
- `CiphertextQueryResponseV1` captures disclosure-minimized query outputs:
  digest-oriented key references, ciphertext metadata, optional inclusion proofs,
  and response-level truncation/sequence context.
- `SecretEnvelopeV1` captures encrypted payload material itself:
  encryption mode, key identifier/version, nonce, ciphertext bytes, and
  integrity commitments.
- `CiphertextStateRecordV1` captures ciphertext-native state entries that
  combine public metadata (content type, policy tags, commitment, payload size)
  with a `SecretEnvelopeV1`.
- User-uploaded model bundles reference approved active SoraFS manifests.
  Model bytes do not live in chain state; Soracloud records keep only registry,
  weight lineage, roots, byte counts, and SoraFS digest metadata in V1.

## Versioning

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

Validation rejects unsupported versions with
`SoracloudManifestError::UnsupportedVersion`.

## Deterministic Validation Rules (V1)

- Container manifest:
  - `bundle_path` and `entrypoint` must be non-empty.
  - Inrou `entrypoint` values are canonical absolute portable-ASCII paths of
    at most 256 bytes and 64 components. Guest-image member paths use the same
    64-component limit below `/inrou/`; CLI publication also rejects missing,
    non-regular, or empty kernel, rootfs, and initrd members before any upload.
  - Every admitted Inrou guest-image profile must carry one concrete,
    validation-complete `published_artifact` reference. Missing and JSON `null`
    artifact representations are not V1 wire values. Its canonical
    `manifest_digest_hex` is the sole storage-manifest identity; V1 has no
    nullable or duplicate storage-identifier field.
  - CLI source workspaces use a separate unpublished shape whose
    `published_artifact` key is exactly JSON `null`. Publication consumes that
    shape, installs immutable SoraFS references for every guest ISA, refreshes
    the service's container hash, and only then constructs an admitted bundle.
    Runtime hydration never falls back to guest files extracted from the
    service bundle.
  - Inrou V1 `lifecycle.start_grace_secs` and `stop_grace_secs` are each at
    most 600 seconds. Admission rejects larger values; the runtime never clamps
    a signed workload grace period. This ceiling does not apply to non-Inrou
    container manifests.
  - `healthcheck_path` (if set) must start with `/`.
  - `config_exports` may reference only configs declared in
    `required_config_names`.
  - config-export env targets must use canonical environment-variable names
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - config-export file targets must stay relative, use `/` separators, use
    only `[A-Za-z0-9._-]` within each segment, and must not contain empty, `.`
    or `..` segments. The runtime preserves each admitted segment exactly.
  - config exports must not target the same env var or relative file path more
    than once.
- Service manifest:
  - every Inrou lease-volume declaration is materialized separately for each
    replica, including non-root service and confidential volumes; V1 never
    treats one disk image as shared or safe to multi-attach.
  - `service_version` must be non-empty.
  - `container.expected_schema_version` must match container schema v1.
  - `rollout.canary_percent` must be `0..=100`. Deterministic IVM services may
    use partial canaries. First-release Inrou HTTP services accept only `0` or
    `100`: host-local lease disks have no authenticated cross-revision state
    migration, and Torii fails closed if hosted deployment state nevertheless
    contains an active canary.
  - `route.path_prefix` (if set) must start with `/`.
  - state binding names must be unique.
- State binding:
  - `key_prefix` must be non-empty and start with `/`.
  - `max_item_bytes <= max_total_bytes`.
  - `ConfidentialState` bindings cannot use plaintext encryption.
- Deployment bundle:
  - `service.container.manifest_hash` must match the canonical encoded
    container manifest hash.
  - `service.container.expected_schema_version` must match the container schema.
  - Mutable state bindings require `container.capabilities.allow_state_writes=true`.
  - Public routes require `container.lifecycle.healthcheck_path`.
- Agent apartment manifest:
  - `container.expected_schema_version` must match container schema v1.
  - tool capability names must be non-empty and unique.
  - policy capability names must be unique.
  - spend-limit assets must be non-empty and unique.
  - `max_per_tx_nanos <= max_per_day_nanos` for each spend limit.
  - allowlist network policy must include unique non-empty hosts.
- FHE parameter set:
  - `backend` and `ciphertext_modulus_bits` must be non-empty.
  - each ciphertext modulus bit-size must be within `2..=120`.
  - ciphertext modulus chain order must be non-increasing.
  - `plaintext_modulus_bits` must be smaller than the largest ciphertext modulus.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - lifecycle height ordering must be strict:
    `activation < withdraw` when present.
  - lifecycle status requirements:
    - `Proposed` disallows a withdraw height.
    - `Active` requires `activation_height`.
    - `Withdrawn` requires `activation_height` + `withdraw_height`.
  - V1 has no deprecated lifecycle or `deprecation_height` field; governance
    transitions parameter sets directly from `Active` to `Withdrawn`.
- FHE execution policy:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - Full-bootstrap verifier-key artifacts carry exactly the governed
    BFV-native V1 payload on every build. A normalized Core STARK payload is
    not a second wire format; `zk-stark` builds validate the native payload and
    convert it deterministically for internal verification.
  - Bootstrap-capable policies (`max_bootstrap_count > 0`) must use exactly one
    governed mode: `bootstrap_key_zero_refresh_proof_statement_digest` for
    `RefreshOnlyV1`, or a digest-pinned, trusted-reviewer-signed full-bootstrap
    release-audit package for `FullBootstrapV1`. Policies with
    `max_bootstrap_count = 0` must omit both modes.
  - parameter-set binding must match by `(param_set, version)`.
  - `max_multiplication_depth` must not exceed parameter-set depth.
  - policy admission rejects `Proposed` or `Withdrawn` parameter-set lifecycle.
- FHE governance bundle:
  - validates policy + parameter-set compatibility as one deterministic admission payload.
  - immutable policy material is registered under an exact `(service, policy,
    version, material_digest)` reference; rotations are consecutive and
    supersede the prior version, while revocation permanently removes the
    active version.
  - jobs carry only the exact active reference and proof attachments. Parameter
    sets, policies, evaluation keys, refresh transcripts, full-bootstrap
    artifacts, and their admitting transaction hash are resolved from
    authoritative deployment state before statement derivation or execution.
- FHE job spec:
  - `job_id` and `output_state_key` must be non-empty (`output_state_key` starts with `/`).
  - input set must be non-empty and input keys must be unique canonical paths.
  - operation-specific constraints are strict (`Add`/`Multiply` multi-input,
    `RotateLeft`/`Bootstrap` single-input, with mutually exclusive depth/rotation/bootstrap knobs).
  - policy-linked admission enforces:
    - policy/param identifiers and versions match.
    - input count/bytes, depth, rotation, and bootstrap limits are within policy caps.
    - deterministic projected output bytes fit policy ciphertext limits.
  - runtime execution enforces input commitment equality against stored
    ciphertext payload bytes before Add, Multiply, RotateLeft, or Bootstrap.
    `RotateLeft` is slot rotation by `rotation_steps`; `Bootstrap` requires the
    registered bootstrap key for the job's evaluation-key bundle plus a signed
    `bootstrap_key_zero_refresh_proof` attachment whose statement hash matches
    the policy-bound transcript digest over the BFV parameter set, public key,
    evaluation-key bundle digest, refresh-transcript digest, bootstrap
    transcript seed/key id/round capacity, and refresh ciphertexts. Its
    verifier record must be active for the canonical Soracloud STARK circuit.
    `FullBootstrapV1` material and artifacts are admitted by the governance
    lifecycle together with the release-audit package; they are not accepted
    as caller-supplied proof material. Each full-bootstrap output still requires
    its canonical execution proof and an active verifier record for the
    reserved execution circuit.
- Decryption authority policy:
  - `approver_ids` must be non-empty, unique, and strictly lexicographically sorted.
  - `ClientHeld` mode requires exactly one approver, `approver_quorum=1`,
    and `allow_break_glass=false`.
  - `ThresholdService` mode requires at least two approvers and
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` must be non-empty and must not contain control characters.
  - `audit_tag` must be non-empty and must not contain control characters.
- Decryption request:
  - `request_id`, `state_key`, and `justification` must be non-empty
    (`state_key` starts with `/`).
  - `jurisdiction_tag` must be non-empty and must not contain control characters.
  - `break_glass_reason` is required when `break_glass=true` and must be omitted when
    `break_glass=false`.
  - policy-linked admission enforces policy-name equality, request TTL not
    exceeding `policy.max_ttl_blocks`, jurisdiction-tag equality, break-glass
    gating, and consent-evidence requirements when
    `policy.require_consent_evidence=true` for non-break-glass requests.
- Ciphertext query spec:
  - `state_key_prefix` must be non-empty and start with `/`.
  - `max_results` is deterministically bounded (`<=256`).
  - metadata projection is explicit (`Minimal` digest-only vs `Standard` key-visible).
- Ciphertext query response:
  - `result_count` must equal serialized row count.
  - `Minimal` projection must not expose `state_key`; `Standard` must expose it.
  - rows must never surface plaintext encryption mode.
  - inclusion proofs (when present) must include non-empty scheme ids and
    `anchor_sequence >= event_sequence`.
- Secret envelope:
  - `key_id`, `nonce`, and `ciphertext` must be non-empty.
  - nonce length is bounded (`<=256` bytes).
  - ciphertext length is bounded (`<=33554432` bytes).
- Ciphertext state record:
  - `state_key` must be non-empty and start with `/`.
  - metadata content type must be non-empty; tags must be unique non-empty strings.
  - `metadata.payload_bytes` must equal `secret.ciphertext.len()`.
  - `metadata.commitment` must equal `secret.commitment`.

## Canonical Fixtures

Canonical JSON fixtures are stored at:

- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
- `fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
- `fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

Fixture/roundtrip tests:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
