#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Tests that a restarted peer restores its state.
use std::{
    borrow::Cow,
    collections::BTreeMap,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, eyre};
use integration_tests::sandbox;
use iroha::crypto::{Hash, Signature};
use iroha::data_model::prelude::*;
use iroha::{
    client::Client,
    crypto::KeyPair,
    data_model::{
        isi::{
            soracloud::{
                DeploySoracloudService, FinalizeSoracloudUploadedModelBundle,
                RecordSoracloudPrivateUploadedModelExecutionReceipt,
                RegisterSoracloudUploadedModelBundle,
            },
            sorafs::RegisterPinManifest,
        },
        smart_contract::manifest::ManifestProvenance,
        soracloud::{
            SORA_CONTAINER_MANIFEST_VERSION_V1, SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1, SORA_SERVICE_MANIFEST_VERSION_V1,
            SORA_STATE_BINDING_VERSION_V1, SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
            SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
            SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1, SoraArtifactKindV1, SoraArtifactRefV1,
            SoraCapabilityPolicyV1, SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1,
            SoraContainerManifestV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
            SoraNetworkAllowlistEntryV1, SoraNetworkPolicyV1, SoraPrivateModelArtifactRefV1,
            SoraPrivateUploadedModelExecutionReceiptV1, SoraResourceLimitsV1, SoraRolloutPolicyV1,
            SoraRouteTargetV1, SoraRouteVisibilityV1, SoraServiceExecutionPlaneV1,
            SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceManifestV1,
            SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1, SoraStateScopeV1,
            SoraTlsModeV1, SoraUploadedModelBundleV1, SoraUploadedModelEncryptionRecipientV1,
            SoraUploadedModelKeyEncapsulationV1, SoraUploadedModelKeyWrapAeadV1,
            SoraUploadedModelPricingPolicyV1, SoraUploadedModelRuntimeFormatV1,
            SoraUploadedModelWrappedKeyV1, encode_bundle_with_materials_provenance_payload,
            encode_uploaded_model_bundle_register_provenance_payload,
            encode_uploaded_model_finalize_provenance_payload,
        },
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestDigest, PinPolicy, StorageClass},
    },
};
use iroha_config_base::toml::WriteExt as _;
use iroha_test_network::*;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use iroha_torii::{
    HEADER_ACCOUNT, HEADER_NONCE, HEADER_SIGNATURE, HEADER_TIMESTAMP_MS, Method, Uri,
    canonical_request_signature_message, signature_header_value,
};
use norito::json::{self, Value};
use tokio::{
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::Table;

const TEST_SNS_LEASE_PAYMENT_NANOS: u64 = 500_000_000;
const TEST_ACCOUNT_ALIAS_LEASE_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(120);
const TEST_ACCOUNT_ALIAS_LEASE_VISIBILITY_POLL: Duration = Duration::from_millis(250);

fn test_account_alias_name_controller(
    account: &AccountId,
) -> Result<iroha::data_model::sns::NameControllerV1> {
    let address = iroha::data_model::account::AccountAddress::from_account_id(account)?;
    Ok(iroha::data_model::sns::NameControllerV1::account(&address))
}

fn test_account_alias_register_request(
    alias_literal: &str,
    owner: &AccountId,
) -> Result<iroha::data_model::sns::RegisterNameRequestV1> {
    Ok(iroha::data_model::sns::RegisterNameRequestV1 {
        selector: iroha::data_model::sns::NameSelectorV1 {
            version: iroha::data_model::sns::NameSelectorV1::VERSION,
            suffix_id: iroha::data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID,
            label: alias_literal.to_owned(),
        },
        owner: owner.clone(),
        controllers: vec![test_account_alias_name_controller(owner)?],
        term_years: 1,
        pricing_class_hint: None,
        payment: iroha::data_model::sns::PaymentProofV1 {
            asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
            gross_amount: TEST_SNS_LEASE_PAYMENT_NANOS,
            net_amount: TEST_SNS_LEASE_PAYMENT_NANOS,
            settlement_tx: iroha_primitives::json::Json::from("mock-settlement"),
            payer: owner.clone(),
            signature: iroha_primitives::json::Json::from("mock-signature"),
        },
        governance: None,
        metadata: Metadata::default(),
    })
}

fn ensure_account_alias_registration_lease(client: &Client, alias_literal: &str) -> Result<()> {
    if account_alias_registration_lease_visible_to_client(client, alias_literal)? {
        return Ok(());
    }

    let request = test_account_alias_register_request(alias_literal, &client.account)?;
    match client.sns().register(&request) {
        Ok(_) => Ok(()),
        Err(err) if is_duplicate_sns_selector_error(&err) => {
            if wait_for_account_alias_registration_lease(client, alias_literal)? {
                Ok(())
            } else {
                Err(err)
            }
        }
        Err(err) => Err(err),
    }
}

fn account_alias_registration_lease_visible_to_client(
    client: &Client,
    alias_literal: &str,
) -> Result<bool> {
    match client
        .sns()
        .get_name(iroha::sns::SnsNamespacePath::AccountAlias, alias_literal)
    {
        Ok(record)
            if record.owner == client.account
                && record.status == iroha::data_model::sns::NameStatus::Active =>
        {
            Ok(true)
        }
        Ok(record) => Err(eyre!(
            "account alias `{alias_literal}` requires an active SNS lease owned by `{}`; found owner `{}` with status {:?}",
            client.account,
            record.owner,
            record.status
        )),
        Err(_) => Ok(false),
    }
}

fn wait_for_account_alias_registration_lease(client: &Client, alias_literal: &str) -> Result<bool> {
    let deadline = Instant::now() + TEST_ACCOUNT_ALIAS_LEASE_VISIBILITY_TIMEOUT;
    while Instant::now() < deadline {
        if account_alias_registration_lease_visible_to_client(client, alias_literal)? {
            return Ok(true);
        }
        std::thread::sleep(TEST_ACCOUNT_ALIAS_LEASE_VISIBILITY_POLL);
    }
    account_alias_registration_lease_visible_to_client(client, alias_literal)
}

fn is_duplicate_sns_selector_error(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("selector `") && message.contains(" is already registered")
    })
}

fn ensure_account_alias_registration_lease_for_network(
    network: &Network,
    alias_literal: &str,
) -> Result<()> {
    let clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    let Some((primary, replicas)) = clients.split_first() else {
        return Ok(());
    };

    ensure_account_alias_registration_lease(primary, alias_literal)?;
    for client in replicas {
        if !wait_for_account_alias_registration_lease(client, alias_literal)? {
            return Err(eyre!(
                "account alias `{alias_literal}` SNS lease was not visible to peer `{}` within {:?}",
                client.torii_url,
                TEST_ACCOUNT_ALIAS_LEASE_VISIBILITY_TIMEOUT
            ));
        }
    }
    Ok(())
}

const PRIVATE_MODEL_SERVICE_NAME: &str = "portal";
const PRIVATE_MODEL_SERVICE_VERSION: &str = "1.0.0";
const PRIVATE_MODEL_ID: &str = "vision_model";
const PRIVATE_MODEL_NAME: &str = "vision";
const PRIVATE_MODEL_WEIGHT_VERSION: &str = "v1";
const PRIVATE_MODEL_ARTIFACT_ID: &str = "vision_model_artifact";
const PRIVATE_MODEL_DATASET_REF: &str = "sorafs://private-upload-fixture";
const PRIVATE_MODEL_POLICY_ID: &str = "policy-private-release";

#[derive(Clone, Debug, iroha::data_model::JsonSerialize)]
struct PrivateUploadedModelQuantizedCpuModelDtoForTest {
    input_len: u64,
    output_len: u64,
    weights_i8: Vec<i8>,
    bias_i32: Vec<i32>,
    output_shift: u8,
    output_min: i32,
    output_max: i32,
}

#[derive(Clone, Debug, iroha::data_model::JsonSerialize)]
struct PrivateUploadedModelExecuteRequestForTest {
    service_name: String,
    weight_version: String,
    model_id: Option<String>,
    model_name: Option<String>,
    bundle_root: Option<Hash>,
    policy_id: String,
    decryption_request_id: Option<String>,
    model: PrivateUploadedModelQuantizedCpuModelDtoForTest,
    plaintext_input_i32: Vec<i32>,
    input_artifact: SoraPrivateModelArtifactRefV1,
    output_artifact: SoraPrivateModelArtifactRefV1,
    emitted_sequence: u64,
}

fn with_soracloud_private_runtime_bootstrap(mut builder: NetworkBuilder) -> NetworkBuilder {
    for instruction in sorafs_pin_fee_bootstrap_instructions() {
        builder = builder.with_genesis_instruction(instruction);
    }
    builder.with_genesis_instruction(Grant::account_permission(
        Permission::from(iroha_executor_data_model::permission::sorafs::CanRegisterSorafsPin),
        ALICE_ID.clone(),
    ))
}

fn sorafs_pin_fee_bootstrap_instructions() -> Vec<InstructionBox> {
    let fee_asset_id: AssetDefinitionId =
        iroha_config::parameters::defaults::governance::sorafs_pin_fee::asset_id()
            .parse()
            .expect("default SoraFS pin fee asset id");
    let treasury =
        iroha_config::parameters::defaults::governance::sorafs_pin_fee::treasury_account_id();
    let fee_name = fee_asset_id
        .try_name()
        .map(ToString::to_string)
        .unwrap_or_else(|| "xor".to_owned());
    let fee_definition = AssetDefinition::numeric(fee_asset_id.clone()).with_name(fee_name);
    let seed_amount = Numeric::new(10_000_000_000_000_u128, 0);

    vec![
        Register::account(Account::new(treasury)).into(),
        Register::asset_definition(fee_definition).into(),
        Mint::asset_numeric(seed_amount, AssetId::new(fee_asset_id, ALICE_ID.clone())).into(),
    ]
}

fn test_sorafs_chunker_handle() -> ChunkerProfileHandle {
    ChunkerProfileHandle {
        profile_id: 1,
        namespace: "sorafs".to_owned(),
        name: "sf1".to_owned(),
        semver: "1.0.0".to_owned(),
        multihash_code: 0x1f,
    }
}

fn test_sorafs_pin_policy() -> PinPolicy {
    PinPolicy {
        min_replicas: 1,
        storage_class: StorageClass::Warm,
        retention_epoch: u64::MAX,
    }
}

fn register_private_model_pin(
    digest: ManifestDigest,
    content_length: u64,
    chunk_seed: u8,
) -> RegisterPinManifest {
    RegisterPinManifest::new(
        digest,
        test_sorafs_chunker_handle(),
        [chunk_seed; 32],
        content_length,
        test_sorafs_pin_policy(),
        1,
        None,
        None,
    )
}

fn soracloud_private_model_service_bundle() -> SoraDeploymentBundleV1 {
    let mut container = SoraContainerManifestV1 {
        schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash: Hash::new(b"private-upload-service-bundle"),
        bundle_path: "/bundles/private-upload-service.ivm".to_owned(),
        entrypoint: "main".to_owned(),
        args: Vec::new(),
        env: BTreeMap::new(),
        inrou: None,
        required_config_names: Vec::new(),
        required_secret_names: Vec::new(),
        config_exports: Vec::new(),
        capabilities: SoraCapabilityPolicyV1 {
            network: SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                "api.example.test",
                [443],
            )]),
            allow_wallet_signing: false,
            allow_state_writes: false,
            allow_model_inference: true,
            allow_model_training: false,
        },
        resources: SoraResourceLimitsV1 {
            cpu_millis: NonZeroU32::new(500).expect("non-zero cpu limit"),
            memory_bytes: NonZeroU64::new(64 * 1024 * 1024).expect("non-zero memory limit"),
            ephemeral_storage_bytes: NonZeroU64::new(64 * 1024 * 1024)
                .expect("non-zero storage limit"),
            max_open_files: NonZeroU32::new(1024).expect("non-zero fd limit"),
            max_tasks: NonZeroU16::new(32).expect("non-zero task limit"),
        },
        lifecycle: iroha::data_model::soracloud::SoraLifecycleHooksV1 {
            start_grace_secs: NonZeroU32::new(10).expect("non-zero start grace"),
            stop_grace_secs: NonZeroU32::new(10).expect("non-zero stop grace"),
            healthcheck_path: Some("/health".to_owned()),
        },
    };
    let container_manifest_hash = Hash::new(iroha::data_model::Encode::encode(&container));
    container.capabilities.allow_model_inference = true;

    SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service: SoraServiceManifestV1 {
            schema_version: SORA_SERVICE_MANIFEST_VERSION_V1,
            service_name: PRIVATE_MODEL_SERVICE_NAME
                .parse()
                .expect("valid service name"),
            service_version: PRIVATE_MODEL_SERVICE_VERSION.to_owned(),
            execution_plane: SoraServiceExecutionPlaneV1::DeterministicService,
            container: SoraContainerManifestRefV1 {
                manifest_hash: container_manifest_hash,
                expected_schema_version: SORA_CONTAINER_MANIFEST_VERSION_V1,
            },
            replicas: NonZeroU16::new(2).expect("non-zero replicas"),
            route: Some(SoraRouteTargetV1 {
                host: "portal.example.test".to_owned(),
                path_prefix: "/".to_owned(),
                service_port: NonZeroU16::new(8080).expect("non-zero port"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            }),
            rollout: SoraRolloutPolicyV1 {
                canary_percent: 0,
                max_unavailable_replicas: 0,
                health_window_secs: NonZeroU32::new(30).expect("non-zero health window"),
                automatic_rollback_failures: NonZeroU32::new(2)
                    .expect("non-zero rollback threshold"),
            },
            economics: Default::default(),
            state_bindings: vec![SoraStateBindingV1 {
                schema_version: SORA_STATE_BINDING_VERSION_V1,
                binding_name: "session".parse().expect("valid binding name"),
                key_prefix: "/state/session".to_owned(),
                scope: SoraStateScopeV1::ServiceState,
                encryption: SoraStateEncryptionV1::Plaintext,
                mutability: SoraStateMutabilityV1::ReadOnly,
                max_item_bytes: NonZeroU64::new(1024).expect("non-zero item size"),
                max_total_bytes: NonZeroU64::new(2048).expect("non-zero total size"),
            }],
            lease_volumes: Vec::new(),
            handlers: vec![SoraServiceHandlerV1 {
                handler_name: "query".parse().expect("valid handler name"),
                class: SoraServiceHandlerClassV1::Query,
                entrypoint: "serve_query".to_owned(),
                route_path: Some("/query".to_owned()),
                certified_response: SoraCertifiedResponsePolicyV1::AuditReceipt,
                mailbox: None,
            }],
            artifacts: vec![SoraArtifactRefV1 {
                kind: SoraArtifactKindV1::StaticAsset,
                artifact_hash: Hash::new(b"private-upload-service-artifact"),
                artifact_path: "/public/index.html".to_owned(),
                handler_name: Some("query".parse().expect("valid handler name")),
            }],
        },
    }
}

fn soracloud_service_bundle_provenance(
    bundle: &SoraDeploymentBundleV1,
) -> Result<ManifestProvenance> {
    let payload = encode_bundle_with_materials_provenance_payload(
        bundle,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )?;
    Ok(ManifestProvenance {
        signer: ALICE_KEYPAIR.public_key().clone(),
        signature: Signature::new(ALICE_KEYPAIR.private_key(), &payload),
    })
}

fn private_uploaded_model_bundle(model_digest: ManifestDigest) -> SoraUploadedModelBundleV1 {
    SoraUploadedModelBundleV1 {
        schema_version: SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        service_name: PRIVATE_MODEL_SERVICE_NAME
            .parse()
            .expect("valid service name"),
        model_id: PRIVATE_MODEL_ID.to_owned(),
        weight_version: PRIVATE_MODEL_WEIGHT_VERSION.to_owned(),
        family: "decoder-only".to_owned(),
        modalities: vec!["text".to_owned()],
        plaintext_root: Hash::new(b"private-upload-plaintext-root"),
        runtime_format: SoraUploadedModelRuntimeFormatV1::DeterministicQuantizedCpuV1,
        bundle_root: Hash::new(b"private-upload-bundle-root"),
        sorafs_manifest_digest: model_digest,
        chunk_count: 1,
        plaintext_bytes: 4_096,
        ciphertext_bytes: 4_352,
        chunk_manifest_root: Hash::new(b"private-upload-chunk-manifest-root"),
        upload_recipient: SoraUploadedModelEncryptionRecipientV1 {
            schema_version: SORA_UPLOADED_MODEL_ENCRYPTION_RECIPIENT_VERSION_V1,
            key_id: "soracloud-upload".to_owned(),
            key_version: NonZeroU32::new(1).expect("non-zero recipient key version"),
            kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
            aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
            public_key_bytes: vec![3u8; 32],
            public_key_fingerprint: Hash::new([3u8; 32]),
        },
        wrapped_bundle_key: SoraUploadedModelWrappedKeyV1 {
            schema_version: SORA_UPLOADED_MODEL_WRAPPED_KEY_VERSION_V1,
            recipient_key_id: "soracloud-upload".to_owned(),
            recipient_key_version: NonZeroU32::new(1).expect("non-zero wrapped key version"),
            kem: SoraUploadedModelKeyEncapsulationV1::X25519HkdfSha256,
            aead: SoraUploadedModelKeyWrapAeadV1::Aes256Gcm,
            ephemeral_public_key: vec![4u8; 32],
            nonce: vec![5u8; 12],
            wrapped_key_ciphertext: vec![6u8; 48],
            ciphertext_hash: Hash::new([6u8; 48]),
            aad_digest: Hash::new(b"private-upload-wrapped-aad"),
        },
        pricing_policy: SoraUploadedModelPricingPolicyV1 {
            storage_xor_nanos: 0,
        },
        decryption_policy_ref: PRIVATE_MODEL_POLICY_ID.to_owned(),
    }
}

fn uploaded_model_bundle_provenance(
    bundle: &SoraUploadedModelBundleV1,
) -> Result<ManifestProvenance> {
    let payload = encode_uploaded_model_bundle_register_provenance_payload(bundle.clone())?;
    Ok(ManifestProvenance {
        signer: ALICE_KEYPAIR.public_key().clone(),
        signature: Signature::new(ALICE_KEYPAIR.private_key(), &payload),
    })
}

#[allow(clippy::too_many_arguments)]
fn uploaded_model_finalize_provenance(
    service_name: &iroha::data_model::name::Name,
    model_name: &str,
    model_id: &str,
    artifact_id: &str,
    weight_version: &str,
    bundle_root: Hash,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
) -> Result<ManifestProvenance> {
    let payload = encode_uploaded_model_finalize_provenance_payload(
        service_name.as_ref(),
        model_name,
        model_id,
        artifact_id,
        weight_version,
        bundle_root,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    )?;
    Ok(ManifestProvenance {
        signer: ALICE_KEYPAIR.public_key().clone(),
        signature: Signature::new(ALICE_KEYPAIR.private_key(), &payload),
    })
}

fn private_model_artifact_ref(
    role: &str,
    digest_seed: u8,
    hash_seed: &[u8],
    ciphertext_bytes: u64,
) -> SoraPrivateModelArtifactRefV1 {
    SoraPrivateModelArtifactRefV1 {
        schema_version: SORA_PRIVATE_MODEL_ARTIFACT_REF_VERSION_V1,
        sorafs_manifest_digest: ManifestDigest::new([digest_seed; 32]),
        artifact_hash: Hash::new(hash_seed),
        ciphertext_bytes,
        artifact_role: role.to_owned(),
    }
}

fn private_uploaded_model_execute_request(
    bundle: &SoraUploadedModelBundleV1,
    input_artifact: SoraPrivateModelArtifactRefV1,
    output_artifact: SoraPrivateModelArtifactRefV1,
) -> PrivateUploadedModelExecuteRequestForTest {
    PrivateUploadedModelExecuteRequestForTest {
        service_name: bundle.service_name.to_string(),
        weight_version: bundle.weight_version.clone(),
        model_id: Some(bundle.model_id.clone()),
        model_name: None,
        bundle_root: Some(bundle.bundle_root),
        policy_id: bundle.decryption_policy_ref.clone(),
        decryption_request_id: None,
        model: PrivateUploadedModelQuantizedCpuModelDtoForTest {
            input_len: 2,
            output_len: 1,
            weights_i8: vec![3, -2],
            bias_i32: vec![1],
            output_shift: 1,
            output_min: -16,
            output_max: 16,
        },
        plaintext_input_i32: vec![5, -3],
        input_artifact,
        output_artifact,
        emitted_sequence: 17,
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_peer_should_restore_its_state() -> Result<()> {
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    );
    let quantity = numeric!(200);

    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_config_layer(|layer| {
                layer
                    .write(["snapshot", "mode"], "read_write")
                    .write(["snapshot", "create_every_ms"], 200_i64);
            }),
        stringify!(restarted_peer_should_restore_its_state),
    )
    .await?
    else {
        return Ok(());
    };
    let peers = network.peers();

    // create state on the first peer
    let peer_a = &peers[0];
    let peer_b = &peers[1];
    let client = peer_a.client();
    let client_for_submit = client.clone();
    let asset_definition_clone = asset_definition_id.clone();
    let mint_quantity = quantity.clone();
    let submit_res: eyre::Result<()> = spawn_blocking(move || {
        client_for_submit
            .submit_all_blocking::<InstructionBox>([
                Register::asset_definition({
                    let __asset_definition_id = asset_definition_clone.clone();
                    AssetDefinition::numeric(__asset_definition_id.clone())
                        .with_name(__asset_definition_id.name().to_string())
                })
                .into(),
                Mint::asset_numeric(
                    mint_quantity,
                    AssetId::new(asset_definition_clone, ALICE_ID.clone()),
                )
                .into(),
            ])
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(
        submit_res,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }
    if sandbox::handle_result(
        network.ensure_blocks(2).await,
        stringify!(restarted_peer_should_restore_its_state),
    )?
    .is_none()
    {
        return Ok(());
    }

    // Ensure the mint made it into the chain before shutting down peers.
    let mint_deadline = Instant::now() + network.sync_timeout();
    let minted = loop {
        let assets = sandbox::handle_result(
            spawn_blocking({
                let client = client.clone();
                move || client.query(FindAssets::new()).execute_all()
            })
            .await?
            .map_err(eyre::Report::new),
            stringify!(restarted_peer_should_restore_its_state),
        )?;
        let Some(assets) = assets else { return Ok(()) };

        if assets.iter().any(|asset| {
            *asset.id().account() == ALICE_ID.clone()
                && *asset.id().definition() == asset_definition_id
                && *asset.value() == quantity
        }) {
            break true;
        }

        if Instant::now() >= mint_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    assert!(minted, "minted asset not observed before restart");

    // Ensure a post-mint snapshot persists before shutdown so restart can rebuild from disk.
    let snapshot_dir = peer_b.kura_store_dir().join("snapshot");
    let snapshot_deadline = Instant::now() + network.sync_timeout();
    let expected_snapshot_height = 2_u64;
    let snapshot_ready = loop {
        let data = snapshot_dir.join("snapshot.data");
        let digest = snapshot_dir.join("snapshot.sha256");
        let sig = snapshot_dir.join("snapshot.sig");
        let merkle = snapshot_dir.join("snapshot.merkle.json");
        let ready = data.exists() && digest.exists() && sig.exists() && merkle.exists();
        if ready {
            if let Ok(snapshot_bytes) = std::fs::read(&data) {
                if let Ok(value) = norito::json::from_slice::<norito::json::Value>(&snapshot_bytes)
                {
                    let height = value
                        .get("block_hashes")
                        .and_then(norito::json::Value::as_array)
                        .map(|entries| entries.len() as u64)
                        .unwrap_or(0);
                    if height >= expected_snapshot_height {
                        break true;
                    }
                }
            }
        }
        if Instant::now() >= snapshot_deadline {
            break false;
        }
        sleep(Duration::from_millis(200)).await;
    };
    if !snapshot_ready {
        return Err(eyre!("snapshot not created before shutdown"));
    }

    // shutdown all
    network.shutdown().await;

    // restart another one, **without a genesis** even
    let config: Vec<_> = network.config_layers().collect();
    assert_ne!(peer_a, peer_b);
    let start_result = timeout(network.peer_startup_timeout(), async move {
        peer_b.start_checked(config.iter().cloned(), None).await?;
        peer_b.once_block(2).await;
        Ok::<(), eyre::Report>(())
    })
    .await;

    match start_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer_b: {reason}"
                )));
            }
            return Err(err);
        }
    }

    // ensure it has the state
    let client = peer_b.client();
    let deadline = Instant::now() + network.sync_timeout();
    let restored = loop {
        let assets = match sandbox::handle_result(
            spawn_blocking({
                let client = client.clone();
                move || client.query(FindAssets::new()).execute_all()
            })
            .await?
            .map_err(eyre::Report::new),
            stringify!(restarted_peer_should_restore_its_state),
        )? {
            Some(assets) => assets,
            None => return Ok(()),
        };
        if let Some(asset) = assets.into_iter().find(|asset| {
            *asset.id().account() == ALICE_ID.clone()
                && *asset.id().definition() == asset_definition_id
        }) {
            break Some(asset.value().clone());
        }
        if Instant::now() >= deadline {
            break None;
        }
        sleep(Duration::from_millis(200)).await;
    };
    let Some(restored_value) = restored else {
        return Err(eyre!("restarted peer did not restore asset before timeout"));
    };
    assert_eq!(quantity, restored_value);

    Ok(())
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn restarted_four_peers_rebuild_route_sensitive_state_from_kura_blocks() -> Result<()> {
    let test_name = stringify!(restarted_four_peers_rebuild_route_sensitive_state_from_kura_blocks);
    let manage_alias_permission =
        iroha_executor_data_model::permission::account::CanManageAccountAlias {
            scope:
                iroha_executor_data_model::permission::account::AccountAliasPermissionScope::Dataspace(
                    iroha::data_model::nexus::DataSpaceId::UNIVERSAL,
                ),
        };
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new()
            .with_peers(4)
            .with_genesis_instruction(Grant::account_permission(
                Permission::from(manage_alias_permission),
                ALICE_ID.clone(),
            )),
        test_name,
    )
    .await?
    else {
        return Ok(());
    };

    let domain_id = DomainId::try_new("paynet", "universal")?;
    let asset_definition_id = AssetDefinitionId::new(domain_id.clone(), "routecoin".parse()?);
    let account_keypair = KeyPair::random();
    let account_id = AccountId::new(account_keypair.public_key().clone());
    let alias = iroha::data_model::account::rekey::AccountAlias::domainless(
        "merchant".parse()?,
        iroha::data_model::nexus::DataSpaceId::UNIVERSAL,
    );
    let asset_id = AssetId::new(asset_definition_id.clone(), account_id.clone());
    let quantity = numeric!(321);

    ensure_domain_registration_lease_for_network(&network, &domain_id)?;
    ensure_account_alias_registration_lease_for_network(&network, "merchant@universal")?;

    let client = network.client();
    let submit_client = client.clone();
    let submit_domain = domain_id.clone();
    let submit_account = account_id.clone();
    let submit_alias = alias.clone();
    let submit_definition = asset_definition_id.clone();
    let submit_asset = asset_id.clone();
    let submit_quantity = quantity.clone();
    let submit_res: eyre::Result<()> = spawn_blocking(move || {
        submit_client
            .submit_all_blocking::<InstructionBox>([
                Register::domain(Domain::new(submit_domain)).into(),
                Register::account(Account::new(submit_account).with_label(Some(submit_alias)))
                    .into(),
                Register::asset_definition({
                    let definition_id = submit_definition.clone();
                    AssetDefinition::numeric(definition_id.clone())
                        .with_name(definition_id.name().to_string())
                })
                .into(),
                Mint::asset_numeric(submit_quantity, submit_asset).into(),
            ])
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(submit_res, test_name)?.is_none() {
        return Ok(());
    }

    if sandbox::handle_result(network.ensure_blocks(2).await, test_name)?.is_none() {
        return Ok(());
    }

    let expected_digest = wait_for_route_sensitive_state_digest(
        client.clone(),
        account_id.clone(),
        alias.clone(),
        domain_id.clone(),
        asset_definition_id.clone(),
        asset_id.clone(),
        quantity.clone(),
        network.sync_timeout(),
    )
    .await?;

    network.shutdown().await;

    for peer in network.peers() {
        remove_optional_recovery_sidecars(&peer.kura_store_dir())?;
    }

    let config_layers: Vec<_> = network.config_layers().collect();
    for peer in network.peers() {
        timeout(network.peer_startup_timeout(), async {
            peer.start_checked(config_layers.iter().cloned(), None)
                .await?;
            peer.once_block(2).await;
            Ok::<(), eyre::Report>(())
        })
        .await
        .map_err(eyre::Report::new)??;
    }

    for peer in network.peers() {
        let digest = wait_for_route_sensitive_state_digest(
            peer.client(),
            account_id.clone(),
            alias.clone(),
            domain_id.clone(),
            asset_definition_id.clone(),
            asset_id.clone(),
            quantity.clone(),
            network.sync_timeout(),
        )
        .await?;
        assert_eq!(
            digest,
            expected_digest,
            "restarted peer {} rebuilt a different route-sensitive WSV surface",
            peer.id()
        );
    }

    Ok(())
}

async fn wait_for_route_sensitive_state_digest(
    client: Client,
    account_id: AccountId,
    alias: iroha::data_model::account::rekey::AccountAlias,
    domain_id: DomainId,
    asset_definition_id: AssetDefinitionId,
    asset_id: AssetId,
    quantity: Numeric,
    timeout_after: Duration,
) -> Result<blake3::Hash> {
    let deadline = Instant::now() + timeout_after;
    let mut last_error = eyre!("route-sensitive state was not observed before timeout");
    loop {
        match route_sensitive_state_digest(
            client.clone(),
            account_id.clone(),
            alias.clone(),
            domain_id.clone(),
            asset_definition_id.clone(),
            asset_id.clone(),
            quantity.clone(),
        )
        .await
        {
            Ok(digest) => return Ok(digest),
            Err(err) => last_error = err,
        }
        if Instant::now() >= deadline {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(last_error)
}

async fn route_sensitive_state_digest(
    client: Client,
    account_id: AccountId,
    alias: iroha::data_model::account::rekey::AccountAlias,
    domain_id: DomainId,
    asset_definition_id: AssetDefinitionId,
    asset_id: AssetId,
    quantity: Numeric,
) -> Result<blake3::Hash> {
    spawn_blocking(move || {
        let account = client.query_single(FindAccountById::new(account_id.clone()))?;
        let alias_account = client.query_single(FindAccountByAlias::new(alias.clone()))?;
        let domain = client.query_single(FindDomainById::new(domain_id.clone()))?;
        let definition =
            client.query_single(FindAssetDefinitionById::new(asset_definition_id.clone()))?;
        let asset = client.query_single(FindAssetById::new(asset_id.clone()))?;
        if *asset.value() != quantity {
            return Err(eyre!(
                "asset `{}` has value `{}`, expected `{}`",
                asset.id(),
                asset.value(),
                quantity
            ));
        }

        let mut aliases =
            client.query_single(FindAliasesByAccountId::new(account_id.clone(), None, None))?;
        aliases.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));

        let mut surface = Vec::new();
        surface.push(format!("account={}", account.id()));
        surface.push(format!("alias_account={}", alias_account.id()));
        surface.push(format!("domain={}", domain.id()));
        surface.push(format!("asset_definition={}", definition.id()));
        surface.push(format!("asset={}:{}", asset.id(), asset.value()));
        for alias_record in aliases {
            surface.push(format!("alias_record={alias_record:?}"));
        }
        surface.sort();
        Ok(blake3::hash(surface.join("\n").as_bytes()))
    })
    .await
    .map_err(eyre::Report::from)?
}

fn remove_optional_recovery_sidecars(root: &Path) -> Result<()> {
    const SIDE_CAR_DIRS: [&str; 3] = ["snapshot", "wsv_checkpoints", "commit_manifests"];
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if SIDE_CAR_DIRS
            .iter()
            .any(|sidecar| name == std::ffi::OsStr::new(sidecar))
        {
            std::fs::remove_dir_all(&path)?;
        } else {
            remove_optional_recovery_sidecars(&path)?;
        }
    }
    Ok(())
}

#[test]
fn remove_optional_recovery_sidecars_preserves_non_sidecar_payloads() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_negative_{nanos}"));
    let result = (|| -> Result<()> {
        std::fs::create_dir_all(root.join("blocks/1"))?;
        std::fs::create_dir_all(root.join("nested/retained"))?;
        std::fs::create_dir_all(root.join("snapshot"))?;
        std::fs::create_dir_all(root.join("wsv_checkpoints"))?;
        std::fs::create_dir_all(root.join("nested/commit_manifests"))?;
        std::fs::write(root.join("blocks/1/block.wire"), b"canonical block")?;
        std::fs::write(root.join("nested/retained/block.wire"), b"retained block")?;
        std::fs::write(root.join("snapshot/stale"), b"optional")?;
        std::fs::write(root.join("wsv_checkpoints/stale"), b"optional")?;
        std::fs::write(root.join("nested/commit_manifests/stale"), b"optional")?;

        remove_optional_recovery_sidecars(&root)?;

        assert!(root.join("blocks/1/block.wire").exists());
        assert!(root.join("nested/retained/block.wire").exists());
        assert!(!root.join("snapshot").exists());
        assert!(!root.join("wsv_checkpoints").exists());
        assert!(!root.join("nested/commit_manifests").exists());
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&root);
    result
}

#[test]
fn remove_optional_recovery_sidecars_preserves_similarly_named_payload_dirs() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_exact_name_{nanos}"));
    let result = (|| -> Result<()> {
        std::fs::create_dir_all(root.join("snapshot_backup"))?;
        std::fs::create_dir_all(root.join("wsv_checkpoints.tmp"))?;
        std::fs::create_dir_all(root.join("commit_manifests_old"))?;
        std::fs::create_dir_all(root.join("nested/snapshot"))?;
        std::fs::write(root.join("snapshot_backup/block.wire"), b"payload")?;
        std::fs::write(root.join("wsv_checkpoints.tmp/block.wire"), b"payload")?;
        std::fs::write(root.join("commit_manifests_old/block.wire"), b"payload")?;
        std::fs::write(root.join("nested/snapshot/stale"), b"optional")?;

        remove_optional_recovery_sidecars(&root)?;

        assert!(root.join("snapshot_backup/block.wire").exists());
        assert!(root.join("wsv_checkpoints.tmp/block.wire").exists());
        assert!(root.join("commit_manifests_old/block.wire").exists());
        assert!(!root.join("nested/snapshot").exists());
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&root);
    result
}

#[test]
fn remove_optional_recovery_sidecars_ignores_missing_root() -> Result<()> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("iroha_sidecar_prune_missing_{nanos}"));
    remove_optional_recovery_sidecars(&root)
}

fn signing_uri(url: &reqwest::Url) -> Result<Uri> {
    match url.query() {
        Some(query) => Ok(format!("{}?{query}", url.path()).parse()?),
        None => Ok(url.path().parse()?),
    }
}

#[test]
fn signing_uri_binds_path_and_query_but_not_origin_or_fragment() -> Result<()> {
    let url = reqwest::Url::parse(
        "https://torii.example.test/v1/soracloud/private/uploaded-models/execute?model=b&policy=a#ignored",
    )?;
    let uri = signing_uri(&url)?;
    assert_eq!(
        uri.to_string(),
        "/v1/soracloud/private/uploaded-models/execute?model=b&policy=a",
    );
    assert!(!uri.to_string().contains("torii.example.test"));
    assert!(!uri.to_string().contains("ignored"));
    Ok(())
}

#[test]
fn signing_uri_keeps_percent_encoded_path_and_query_boundaries() -> Result<()> {
    let url = reqwest::Url::parse(
        "https://torii.example.test/v1/soracloud/private%2Fexecute?cursor=a%2Fb%3Fc&count_mode=exact",
    )?;
    let uri = signing_uri(&url)?;
    assert_eq!(
        uri.to_string(),
        "/v1/soracloud/private%2Fexecute?cursor=a%2Fb%3Fc&count_mode=exact",
    );
    Ok(())
}

fn add_canonical_app_headers(
    request: reqwest::RequestBuilder,
    client: &Client,
    method: Method,
    url: &reqwest::Url,
    body: &[u8],
) -> Result<reqwest::RequestBuilder> {
    let timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX);
    let nonce = format!(
        "soracloud-private-upload-{timestamp_ms}-{}",
        Hash::new(url.as_str())
    );
    let uri = signing_uri(url)?;
    let message = canonical_request_signature_message(&method, &uri, body, timestamp_ms, &nonce);
    let signature = Signature::new(client.key_pair.private_key(), &message);
    Ok(request
        .header(HEADER_ACCOUNT, client.account.to_string())
        .header(HEADER_SIGNATURE, signature_header_value(&signature))
        .header(HEADER_TIMESTAMP_MS, timestamp_ms.to_string())
        .header(HEADER_NONCE, nonce))
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn soracloud_private_uploaded_model_receipt_survives_four_peer_restart() -> Result<()> {
    let test_name = stringify!(soracloud_private_uploaded_model_receipt_survives_four_peer_restart);
    let Some(network) = sandbox::start_network_async_or_skip(
        with_soracloud_private_runtime_bootstrap(NetworkBuilder::new().with_peers(4)),
        test_name,
    )
    .await?
    else {
        return Ok(());
    };

    let model_digest = ManifestDigest::new([0xA5; 32]);
    let input_artifact = private_model_artifact_ref("input", 0xB1, b"private-input-artifact", 64);
    let output_artifact =
        private_model_artifact_ref("output", 0xB2, b"private-output-artifact", 96);
    let service_bundle = soracloud_private_model_service_bundle();
    let uploaded_bundle = private_uploaded_model_bundle(model_digest);
    let weight_artifact_hash = Hash::new(b"private-weight-artifact");
    let training_config_hash = Hash::new(b"private-training-config");
    let reproducibility_hash = Hash::new(b"private-reproducibility");
    let provenance_attestation_hash = Hash::new(b"private-provenance-attestation");

    let setup_instructions = vec![
        register_private_model_pin(
            uploaded_bundle.sorafs_manifest_digest,
            uploaded_bundle.ciphertext_bytes,
            0xC1,
        )
        .into(),
        register_private_model_pin(
            input_artifact.sorafs_manifest_digest,
            input_artifact.ciphertext_bytes,
            0xC2,
        )
        .into(),
        register_private_model_pin(
            output_artifact.sorafs_manifest_digest,
            output_artifact.ciphertext_bytes,
            0xC3,
        )
        .into(),
        DeploySoracloudService {
            bundle: service_bundle.clone(),
            initial_service_configs: BTreeMap::new(),
            initial_service_secrets: BTreeMap::new(),
            provenance: soracloud_service_bundle_provenance(&service_bundle)?,
        }
        .into(),
    ];
    let setup_client = network.client();
    let setup_res: eyre::Result<()> = spawn_blocking(move || {
        setup_client
            .submit_all_blocking::<InstructionBox>(setup_instructions)
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(setup_res, test_name)?.is_none() {
        return Ok(());
    }

    if sandbox::handle_result(network.ensure_blocks(2).await, test_name)?.is_none() {
        return Ok(());
    }

    let upload_instructions = vec![
        RegisterSoracloudUploadedModelBundle {
            bundle: uploaded_bundle.clone(),
            provenance: uploaded_model_bundle_provenance(&uploaded_bundle)?,
        }
        .into(),
        FinalizeSoracloudUploadedModelBundle {
            service_name: uploaded_bundle.service_name.clone(),
            model_name: PRIVATE_MODEL_NAME.to_owned(),
            model_id: uploaded_bundle.model_id.clone(),
            artifact_id: PRIVATE_MODEL_ARTIFACT_ID.to_owned(),
            weight_version: uploaded_bundle.weight_version.clone(),
            bundle_root: uploaded_bundle.bundle_root,
            weight_artifact_hash,
            dataset_ref: PRIVATE_MODEL_DATASET_REF.to_owned(),
            training_config_hash,
            reproducibility_hash,
            provenance_attestation_hash,
            provenance: uploaded_model_finalize_provenance(
                &uploaded_bundle.service_name,
                PRIVATE_MODEL_NAME,
                &uploaded_bundle.model_id,
                PRIVATE_MODEL_ARTIFACT_ID,
                &uploaded_bundle.weight_version,
                uploaded_bundle.bundle_root,
                weight_artifact_hash,
                PRIVATE_MODEL_DATASET_REF,
                training_config_hash,
                reproducibility_hash,
                provenance_attestation_hash,
            )?,
        }
        .into(),
    ];
    let upload_client = network.client();
    let upload_res: eyre::Result<()> = spawn_blocking(move || {
        upload_client
            .submit_all_blocking::<InstructionBox>(upload_instructions)
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(upload_res, test_name)?.is_none() {
        return Ok(());
    }

    if sandbox::handle_result(network.ensure_blocks(3).await, test_name)?.is_none() {
        return Ok(());
    }

    let execute_request = private_uploaded_model_execute_request(
        &uploaded_bundle,
        input_artifact.clone(),
        output_artifact.clone(),
    );
    let receipt = post_private_uploaded_model_execute(&network.client(), &execute_request).await?;
    receipt
        .validate()
        .map_err(|err| eyre!("private uploaded-model receipt did not validate: {err}"))?;
    assert_eq!(receipt.service_name, uploaded_bundle.service_name);
    assert_eq!(receipt.model_id, uploaded_bundle.model_id);
    assert_eq!(receipt.weight_version, uploaded_bundle.weight_version);
    assert_eq!(
        receipt.model_manifest_digest,
        uploaded_bundle.sorafs_manifest_digest
    );
    assert_eq!(receipt.input_artifact, input_artifact);
    assert_eq!(receipt.output_artifact, output_artifact);

    let receipt_instruction = RecordSoracloudPrivateUploadedModelExecutionReceipt {
        receipt: receipt.clone(),
    };
    let receipt_client = network.client();
    let receipt_res: eyre::Result<()> = spawn_blocking(move || {
        receipt_client
            .submit_all_blocking::<InstructionBox>([receipt_instruction.into()])
            .map(|_| ())
    })
    .await
    .map_err(eyre::Report::from)?;
    if sandbox::handle_result(receipt_res, test_name)?.is_none() {
        return Ok(());
    }

    if sandbox::handle_result(network.ensure_blocks(4).await, test_name)?.is_none() {
        return Ok(());
    }

    let committed = wait_for_private_uploaded_model_receipt(
        network.client(),
        receipt.clone(),
        network.sync_timeout(),
    )
    .await?;
    assert_eq!(committed, receipt);

    network.shutdown().await;

    for peer in network.peers() {
        remove_optional_recovery_sidecars(&peer.kura_store_dir())?;
    }

    let config_layers: Vec<_> = network.config_layers().collect();
    for peer in network.peers() {
        timeout(network.peer_startup_timeout(), async {
            peer.start_checked(config_layers.iter().cloned(), None)
                .await?;
            peer.once_block(4).await;
            Ok::<(), eyre::Report>(())
        })
        .await
        .map_err(eyre::Report::new)??;
    }

    for peer in network.peers() {
        let restored = wait_for_private_uploaded_model_receipt(
            peer.client(),
            receipt.clone(),
            network.sync_timeout(),
        )
        .await?;
        assert_eq!(
            restored,
            receipt,
            "restarted peer {} rebuilt a different Soracloud private receipt",
            peer.id()
        );
    }

    Ok(())
}

async fn post_private_uploaded_model_execute(
    client: &Client,
    request: &PrivateUploadedModelExecuteRequestForTest,
) -> Result<SoraPrivateUploadedModelExecutionReceiptV1> {
    let url = client
        .torii_url
        .join("/v1/soracloud/model/upload/private/execute")?;
    let body = json::to_vec(request)?;
    let request = integration_tests::http::client()
        .post(url.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .body(body.clone());
    let response = add_canonical_app_headers(request, client, Method::POST, &url, &body)?
        .send()
        .await?;
    let status = response.status();
    let response_body = response.bytes().await?.to_vec();
    if !status.is_success() {
        return Err(eyre!(
            "private uploaded-model execute returned status {}: {}",
            status,
            String::from_utf8_lossy(&response_body)
        ));
    }
    let value: Value = json::from_slice(&response_body)?;
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("private execute response was not a JSON object"))?;
    assert_eq!(root.get("schema_version").and_then(Value::as_u64), Some(1));
    let instructions = root
        .get("tx_instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("private execute response omitted tx_instructions"))?;
    let instruction = instructions
        .first()
        .and_then(Value::as_object)
        .ok_or_else(|| eyre!("private execute response omitted receipt instruction"))?;
    assert_eq!(
        instruction.get("wire_id").and_then(Value::as_str),
        Some(
            "iroha_data_model::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt"
        )
    );
    let payload_hex = instruction
        .get("payload_hex")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("private execute response omitted receipt instruction payload"))?;
    if payload_hex.is_empty() {
        return Err(eyre!(
            "private execute response returned an empty receipt instruction payload"
        ));
    }
    let receipt_value = root
        .get("receipt")
        .ok_or_else(|| eyre!("private execute response omitted receipt"))?;
    Ok(json::from_value(receipt_value.clone())?)
}

async fn wait_for_private_uploaded_model_receipt(
    client: Client,
    expected: SoraPrivateUploadedModelExecutionReceiptV1,
    timeout_after: Duration,
) -> Result<SoraPrivateUploadedModelExecutionReceiptV1> {
    let deadline = Instant::now() + timeout_after;
    let mut last_error = eyre!("private uploaded-model receipt was not observed before timeout");
    loop {
        match fetch_private_uploaded_model_receipt(&client, &expected).await {
            Ok(receipt) => return Ok(receipt),
            Err(err) => last_error = err,
        }
        if Instant::now() >= deadline {
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(last_error)
}

async fn fetch_private_uploaded_model_receipt(
    client: &Client,
    expected: &SoraPrivateUploadedModelExecutionReceiptV1,
) -> Result<SoraPrivateUploadedModelExecutionReceiptV1> {
    let mut url = client
        .torii_url
        .join("/v1/soracloud/model/upload/private/receipts")?;
    url.query_pairs_mut()
        .append_pair("service_name", expected.service_name.as_ref())
        .append_pair("model_id", &expected.model_id)
        .append_pair("weight_version", &expected.weight_version)
        .append_pair("limit", "1")
        .append_pair("count_mode", "exact");
    let response = integration_tests::http::client()
        .get(url.clone())
        .header(reqwest::header::ACCEPT, "application/json");
    let response = add_canonical_app_headers(response, client, Method::GET, &url, &[])?
        .send()
        .await?;
    let status = response.status();
    let response_body = response.bytes().await?.to_vec();
    if !status.is_success() {
        return Err(eyre!(
            "private uploaded-model receipt query returned status {}: {}",
            status,
            String::from_utf8_lossy(&response_body)
        ));
    }
    let value: Value = json::from_slice(&response_body)?;
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("private receipt query response was not a JSON object"))?;
    if root.get("count_mode").and_then(Value::as_str) != Some("exact") {
        return Err(eyre!(
            "private receipt query did not preserve exact count mode: {value:?}"
        ));
    }
    if root.get("returned_items").and_then(Value::as_u64) != Some(1) {
        return Err(eyre!(
            "private receipt query did not return exactly one item: {value:?}"
        ));
    }
    if root.get("remaining_items").and_then(Value::as_u64) != Some(0) {
        return Err(eyre!(
            "private receipt query reported unexpected remaining_items: {value:?}"
        ));
    }
    if root.get("has_more").and_then(Value::as_bool) != Some(false) {
        return Err(eyre!(
            "private receipt query reported unexpected has_more: {value:?}"
        ));
    }
    if root.get("total").and_then(Value::as_u64) != Some(1) {
        return Err(eyre!(
            "private receipt query reported unexpected exact total: {value:?}"
        ));
    }
    let receipts = root
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("private receipt query response omitted receipts"))?;
    let receipt_value = receipts
        .first()
        .ok_or_else(|| eyre!("private receipt query returned no receipts"))?;
    let receipt: SoraPrivateUploadedModelExecutionReceiptV1 =
        json::from_value(receipt_value.clone())?;
    if receipt.receipt_id != expected.receipt_id {
        return Err(eyre!(
            "private receipt query returned receipt `{}` instead of `{}`",
            receipt.receipt_id,
            expected.receipt_id
        ));
    }
    Ok(receipt)
}

#[tokio::test]
async fn restarted_peer_with_mismatched_genesis_pubkey_uses_stored_genesis() -> Result<()> {
    let test_name = stringify!(restarted_peer_with_mismatched_genesis_pubkey_uses_stored_genesis);
    let Some(network) =
        sandbox::start_network_async_or_skip(NetworkBuilder::new().with_peers(4), test_name)
            .await?
    else {
        return Ok(());
    };
    let peer = &network.peers()[0];

    if sandbox::handle_result(network.ensure_blocks(1).await, test_name)?.is_none() {
        return Ok(());
    }

    let config_layers: Vec<_> = network.config_layers().collect();
    let wrong_genesis_pubkey = KeyPair::random().public_key().to_string();
    let override_layer = Table::new().write(["genesis", "public_key"], wrong_genesis_pubkey);
    let genesis = network.genesis();

    network.shutdown().await;

    let start_result = timeout(network.peer_startup_timeout(), async {
        peer.start_checked(
            config_layers
                .iter()
                .cloned()
                .chain(std::iter::once(Cow::Owned(override_layer))),
            Some(&genesis),
        )
        .await?;
        peer.once_block(1).await;
        Ok::<(), eyre::Report>(())
    })
    .await;

    match start_result {
        Ok(Ok(())) => {}
        Ok(Err(err)) => {
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
                )));
            }
            return Err(err);
        }
        Err(err) => {
            let err = eyre::Report::new(err);
            if let Some(reason) = sandbox::sandbox_reason(&err) {
                return Err(err.wrap_err(format!(
                    "sandboxed network restriction detected while restarting peer with mismatched genesis pubkey: {reason}"
                )));
            }
            return Err(err);
        }
    }

    network.shutdown().await;
    Ok(())
}
