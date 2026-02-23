# SoraCloud V1 Manifest Schemas

This page defines the first deterministic Norito schemas for SoraCloud
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

## Scope

These manifests are designed for the `IVM` + custom Sora Container Runtime
(SCR) direction (no WASM, no Docker dependency in runtime admission).

- `SoraContainerManifestV1` captures executable bundle identity, runtime type,
  capability policy, resources, and lifecycle probe settings.
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
  bounds, and lifecycle heights (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` captures deterministic ciphertext execution limits:
  admitted payload sizes, input/output fan-in, depth/rotation/bootstrap caps,
  and canonical rounding mode.
- `FheGovernanceBundleV1` couples a parameter set and policy for deterministic
  admission validation.
- `FheJobSpecV1` captures deterministic ciphertext job admission/execution
  requests: operation class, ordered input commitments, output key, and bounded
  depth/rotation/bootstrap demand linked to a policy + parameter set.
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
`SoraCloudManifestError::UnsupportedVersion`.

## Deterministic Validation Rules (V1)

- Container manifest:
  - `bundle_path` and `entrypoint` must be non-empty.
  - `healthcheck_path` (if set) must start with `/`.
- Service manifest:
  - `service_version` must be non-empty.
  - `container.expected_schema_version` must match container schema v1.
  - `rollout.canary_percent` must be `0..=100`.
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
    `activation < deprecation < withdraw` when present.
  - lifecycle status requirements:
    - `Proposed` disallows deprecation/withdraw heights.
    - `Active` requires `activation_height`.
    - `Deprecated` requires `activation_height` + `deprecation_height`.
    - `Withdrawn` requires `activation_height` + `withdraw_height`.
- FHE execution policy:
  - `max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - parameter-set binding must match by `(param_set, version)`.
  - `max_multiplication_depth` must not exceed parameter-set depth.
  - policy admission rejects `Proposed` or `Withdrawn` parameter-set lifecycle.
- FHE governance bundle:
  - validates policy + parameter-set compatibility as one deterministic admission payload.
- FHE job spec:
  - `job_id` and `output_state_key` must be non-empty (`output_state_key` starts with `/`).
  - input set must be non-empty and input keys must be unique canonical paths.
  - operation-specific constraints are strict (`Add`/`Multiply` multi-input,
    `RotateLeft`/`Bootstrap` single-input, with mutually exclusive depth/rotation/bootstrap knobs).
  - policy-linked admission enforces:
    - policy/param identifiers and versions match.
    - input count/bytes, depth, rotation, and bootstrap limits are within policy caps.
    - deterministic projected output bytes fit policy ciphertext limits.
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
