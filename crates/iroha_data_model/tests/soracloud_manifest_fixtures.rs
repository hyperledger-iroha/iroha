//! Canonical fixture and roundtrip checks for SoraCloud V1 manifests.

use std::{
    collections::BTreeMap,
    fmt::Debug,
    fs,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::Path,
};

use iroha_crypto::Hash;
use iroha_data_model::{
    Decode, Encode,
    soracloud::{
        AGENT_APARTMENT_MANIFEST_VERSION_V1, AgentApartmentManifestV1, AgentSpendLimitV1,
        AgentToolCapabilityV1, AgentUpgradePolicyV1, SORA_CONTAINER_MANIFEST_VERSION_V1,
        SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
        SORA_STATE_BINDING_VERSION_V1, SoraCapabilityPolicyV1, SoraContainerManifestRefV1,
        SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraLifecycleHooksV1, SoraNetworkPolicyV1, SoraResourceLimitsV1, SoraRolloutPolicyV1,
        SoraRouteTargetV1, SoraRouteVisibilityV1, SoraServiceManifestV1, SoraStateBindingV1,
        SoraStateEncryptionV1, SoraStateMutabilityV1, SoraStateScopeV1, SoraTlsModeV1,
    },
};
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize};

const CONTAINER_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_container_manifest_v1.json"
));
const SERVICE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_service_manifest_v1.json"
));
const STATE_BINDING_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_state_binding_v1.json"
));
const DEPLOYMENT_BUNDLE_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/sora_deployment_bundle_v1.json"
));
const AGENT_APARTMENT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/soracloud/agent_apartment_manifest_v1.json"
));

fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = seed.wrapping_add(u8::try_from(index).expect("index fits in u8"));
    }
    Hash::prehashed(bytes)
}

fn expected_state_binding() -> SoraStateBindingV1 {
    SoraStateBindingV1 {
        schema_version: SORA_STATE_BINDING_VERSION_V1,
        binding_name: "session_store".parse().expect("valid name"),
        scope: SoraStateScopeV1::ServiceState,
        mutability: SoraStateMutabilityV1::ReadWrite,
        encryption: SoraStateEncryptionV1::ClientCiphertext,
        key_prefix: "/state/session".to_string(),
        max_item_bytes: NonZeroU64::new(4_096).expect("nonzero"),
        max_total_bytes: NonZeroU64::new(262_144).expect("nonzero"),
    }
}

fn expected_container_manifest() -> SoraContainerManifestV1 {
    SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash: sample_hash(7),
        bundle_path: "/bundles/webapp.to".to_string(),
        entrypoint: "main".to_string(),
        args: vec!["--http".to_string(), "--port=8080".to_string()],
        env: BTreeMap::from([
            ("APP_ENV".to_string(), "production".to_string()),
            ("LOG_LEVEL".to_string(), "info".to_string()),
        ]),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Allowlist(vec![
                "api.sora.internal".to_string(),
                "wallet.sora.internal".to_string(),
            ]),
            allow_wallet_signing: true,
            allow_state_writes: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(750).expect("nonzero"),
            memory_bytes: NonZeroU64::new(536_870_912).expect("nonzero"),
            ephemeral_storage_bytes: NonZeroU64::new(2_147_483_648).expect("nonzero"),
            max_open_files: NonZeroU32::new(512).expect("nonzero"),
            max_tasks: NonZeroU16::new(64).expect("nonzero"),
        },
        lifecycle: SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(30).expect("nonzero"),
            stop_grace_secs: NonZeroU32::new(20).expect("nonzero"),
            healthcheck_path: Some("/healthz".to_string()),
        },
    }
}

fn expected_service_manifest() -> SoraServiceManifestV1 {
    SoraServiceManifestV1 {
        schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
        service_name: "web_portal".parse().expect("valid name"),
        service_version: "2026.02.0".to_string(),
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(17),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        replicas: NonZeroU16::new(3).expect("nonzero"),
        route: Some(SoraRouteTargetV1 {
            host: "portal.sora".to_string(),
            path_prefix: "/app".to_string(),
            service_port: NonZeroU16::new(8080).expect("nonzero"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        }),
        rollout: SoraRolloutPolicyV1 {
            canary_percent: 20,
            max_unavailable_replicas: 1,
            health_window_secs: NonZeroU32::new(45).expect("nonzero"),
            automatic_rollback_failures: NonZeroU32::new(3).expect("nonzero"),
        },
        state_bindings: vec![
            expected_state_binding(),
            SoraStateBindingV1 {
                schema_version: SORA_STATE_BINDING_VERSION_V1,
                binding_name: "patient_records".parse().expect("valid name"),
                scope: SoraStateScopeV1::ConfidentialState,
                mutability: SoraStateMutabilityV1::AppendOnly,
                encryption: SoraStateEncryptionV1::FheCiphertext,
                key_prefix: "/state/health".to_string(),
                max_item_bytes: NonZeroU64::new(16_384).expect("nonzero"),
                max_total_bytes: NonZeroU64::new(16_777_216).expect("nonzero"),
            },
        ],
    }
}

fn expected_deployment_bundle() -> SoraDeploymentBundleV1 {
    let container = expected_container_manifest();
    let mut service = expected_service_manifest();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    }
}

fn expected_agent_apartment_manifest() -> AgentApartmentManifestV1 {
    AgentApartmentManifestV1 {
        schema_version: AGENT_APARTMENT_MANIFEST_VERSION_V1,
        apartment_name: "ops_agent".parse().expect("valid name"),
        container: SoraContainerManifestRefV1 {
            manifest_hash: sample_hash(33),
            expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        },
        tool_capabilities: vec![
            AgentToolCapabilityV1 {
                tool: "soracloud.deploy".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(120).expect("nonzero"),
                allow_network: true,
                allow_filesystem_write: false,
            },
            AgentToolCapabilityV1 {
                tool: "wallet.transfer".to_string(),
                max_invocations_per_epoch: NonZeroU32::new(24).expect("nonzero"),
                allow_network: false,
                allow_filesystem_write: false,
            },
        ],
        policy_capabilities: vec![
            "wallet.sign".parse().expect("valid name"),
            "governance.audit".parse().expect("valid name"),
        ],
        spend_limits: vec![
            AgentSpendLimitV1 {
                asset_definition: "xor#sora".to_string(),
                max_per_tx_nanos: NonZeroU64::new(5_000_000).expect("nonzero"),
                max_per_day_nanos: NonZeroU64::new(20_000_000).expect("nonzero"),
            },
            AgentSpendLimitV1 {
                asset_definition: "usd#bank".to_string(),
                max_per_tx_nanos: NonZeroU64::new(2_000_000).expect("nonzero"),
                max_per_day_nanos: NonZeroU64::new(10_000_000).expect("nonzero"),
            },
        ],
        state_quota_bytes: NonZeroU64::new(134_217_728).expect("nonzero"),
        network_egress: SoraNetworkPolicyV1::Allowlist(vec![
            "rpc.sora.internal".to_string(),
            "torii.sora.internal".to_string(),
        ]),
        upgrade_policy: AgentUpgradePolicyV1::Governed,
    }
}

fn assert_norito_roundtrip<T>(value: &T)
where
    T: Encode + Decode + PartialEq + Debug,
{
    let encoded = Encode::encode(value);
    let mut cursor = encoded.as_slice();
    let decoded = <T as Decode>::decode(&mut cursor).expect("decode succeeds");
    assert!(cursor.is_empty(), "decode must consume all bytes");
    assert_eq!(decoded, *value, "roundtrip must preserve payload");
}

#[cfg(feature = "json")]
fn assert_fixture_eq<T>(path: &str, fixture: &str, expected: &T)
where
    T: Clone + PartialEq + Debug + FastJsonWrite + JsonDeserialize + JsonSerialize,
{
    let parsed: T = json::from_str(fixture).expect("fixture must decode");
    assert_eq!(
        parsed, *expected,
        "fixture `{path}` content does not match expected data model"
    );
    let canonical = json::to_json_pretty(expected).expect("serialize fixture");
    assert_eq!(
        fixture.trim(),
        canonical.trim(),
        "fixture `{path}` is not canonical JSON for the current schema"
    );
}

#[cfg(feature = "json")]
fn write_fixture<T: JsonSerialize>(path: &Path, value: &T) {
    let json = json::to_json_pretty(value).expect("serialize fixture");
    fs::write(path, json).unwrap_or_else(|error| {
        panic!("failed writing {}: {error}", path.display());
    });
}

#[cfg(feature = "json")]
#[test]
fn container_manifest_fixture_is_canonical() {
    let manifest = expected_container_manifest();
    assert_fixture_eq(
        "sora_container_manifest_v1.json",
        CONTAINER_FIXTURE,
        &manifest,
    );
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn state_binding_fixture_is_canonical() {
    let binding = expected_state_binding();
    assert_fixture_eq(
        "sora_state_binding_v1.json",
        STATE_BINDING_FIXTURE,
        &binding,
    );
    assert_norito_roundtrip(&binding);
    binding.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn service_manifest_fixture_is_canonical() {
    let manifest = expected_service_manifest();
    assert_fixture_eq("sora_service_manifest_v1.json", SERVICE_FIXTURE, &manifest);
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn deployment_bundle_fixture_is_canonical() {
    let bundle = expected_deployment_bundle();
    assert_fixture_eq(
        "sora_deployment_bundle_v1.json",
        DEPLOYMENT_BUNDLE_FIXTURE,
        &bundle,
    );
    assert_norito_roundtrip(&bundle);
    bundle
        .validate_for_admission()
        .expect("deployment bundle fixture should validate");
}

#[cfg(feature = "json")]
#[test]
fn agent_apartment_manifest_fixture_is_canonical() {
    let manifest = expected_agent_apartment_manifest();
    assert_fixture_eq(
        "agent_apartment_manifest_v1.json",
        AGENT_APARTMENT_FIXTURE,
        &manifest,
    );
    assert_norito_roundtrip(&manifest);
    manifest.validate().expect("fixture should validate");
}

#[cfg(feature = "json")]
#[test]
#[ignore = "regenerates Soracloud fixture files"]
fn regenerate_soracloud_fixtures() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("soracloud");
    write_fixture(
        &base.join("sora_container_manifest_v1.json"),
        &expected_container_manifest(),
    );
    write_fixture(
        &base.join("sora_service_manifest_v1.json"),
        &expected_service_manifest(),
    );
    write_fixture(
        &base.join("sora_state_binding_v1.json"),
        &expected_state_binding(),
    );
    write_fixture(
        &base.join("sora_deployment_bundle_v1.json"),
        &expected_deployment_bundle(),
    );
    write_fixture(
        &base.join("agent_apartment_manifest_v1.json"),
        &expected_agent_apartment_manifest(),
    );
}
