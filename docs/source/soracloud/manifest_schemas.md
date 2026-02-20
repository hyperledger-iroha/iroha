# SoraCloud V1 Manifest Schemas

This page defines the first deterministic Norito schemas for SoraCloud
deployment on Iroha 3:

- `SoraContainerManifestV1`
- `SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`

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

## Versioning

- `SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`

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

## Canonical Fixtures

Canonical JSON fixtures are stored at:

- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`

Fixture/roundtrip tests:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`
