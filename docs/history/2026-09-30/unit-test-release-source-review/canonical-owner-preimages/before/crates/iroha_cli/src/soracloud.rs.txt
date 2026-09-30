//! Soracloud deployment helpers (`init/deploy/status/upgrade/rollback/rollout`).
//!
//! Offline helpers scaffold manifests, plan workspaces, and produce canonical
//! deployment artifacts. Mutation and status commands validate request inputs
//! locally and then call the authoritative Soracloud control plane through
//! Torii. Model-training, inert Hugging Face storage-lease metadata, and
//! weight-lifecycle helpers also use live Torii endpoints. Outbound mutation DTOs
//! carry signed provenance only; single-signature account headers use canonical
//! lowercase hex while signed bodies and paths retain I105. Protected GETs
//! require the exact NetworkId and local key.
use crate::{Run, RunContext};
use eyre::{Report, Result, WrapErr, eyre};
#[cfg(test)]
use iroha::data_model::{
    nexus::FeeDebitSource,
    soracloud::{
        CanonicalRequestSignatureWitnessV1, SORA_UPLOADED_MODEL_BUNDLE_VERSION_V1,
        SoraUploadedModelPackageFormatV1, SoracloudTxInstruction,
    },
};
use iroha::{
    blocking::Client,
    client::{
        CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1, FeeQuoteRequest,
        canonical_network_request_hash, canonical_network_request_signature_message,
        canonical_request_account_header_value, canonical_request_signature_header_value,
        canonical_request_timestamp_header_value, canonical_request_witness_header_value,
    },
    config::Config as ClientConfig,
    data_model::{
        Encode,
        account::AccountId,
        asset::AssetDefinitionId,
        isi::{InstructionBox, decode_instruction_from_pair},
        prelude::TransactionEntrypoint,
        smart_contract::manifest::ManifestProvenance,
        soracloud::{
            AgentApartmentManifestV1, AgentUpgradePolicyV1, CANONICAL_REQUEST_WITNESS_VERSION_V1,
            CanonicalRequestWitnessV1, SORA_APP_INFRA_MANIFEST_VERSION_V1,
            SORA_APP_INFRA_SERVICE_REF_VERSION_V1, SORA_APP_ROUTE_PROJECTION_VERSION_V1,
            SORA_APP_STATIC_SITE_BINDING_VERSION_V1, SORA_CONTAINER_MANIFEST_VERSION_V1,
            SORA_DEPLOYMENT_BUNDLE_VERSION_V1, SORA_INROU_MANIFEST_VERSION_V1,
            SORA_SERVICE_AUDIT_EVENT_VERSION_V1, SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
            SORA_SERVICE_ROLLOUT_STATE_VERSION_V1, SORA_STATE_BINDING_VERSION_V1,
            SecretEnvelopeEncryptionV1, SecretEnvelopeV1, SoraAgentRuntimeStatusV1,
            SoraAppInfraAuditEventV1, SoraAppInfraExactCurrentRevisionPreconditionV1,
            SoraAppInfraManifestV1, SoraAppInfraMutationPreconditionV1, SoraAppInfraServiceRefV1,
            SoraAppInfraStateV1, SoraAppRouteProjectionV1, SoraAppStaticSiteBindingV1,
            SoraArtifactKindV1, SoraArtifactRefV1, SoraCapabilityPolicyV1,
            SoraCertifiedResponsePolicyV1, SoraConfigExportV1, SoraContainerManifestV1,
            SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraHfSharedLeaseAuditEventV1,
            SoraHfSharedLeaseMemberV1, SoraHfSharedLeasePoolV1, SoraHfSourceRecordV1,
            SoraInrouGuestImageV1, SoraInrouGuestIsaV1, SoraInrouHostCapabilityRecordV1,
            SoraInrouManifestV1, SoraInrouPlacementTargetV1, SoraLeaseVolumeBindingV1,
            SoraLeaseVolumeKindV1, SoraLifecycleHooksV1, SoraMailboxContractV1,
            SoraNetworkAllowlistEntryV1, SoraNetworkPolicyV1,
            SoraPublishedInrouGuestImageArtifactV1, SoraResourceLimitsV1, SoraRolloutStageV1,
            SoraRouteTargetV1, SoraRouteVisibilityV1, SoraRuntimeDeterministicValidatorHostV1,
            SoraServiceAuditEventV1, SoraServiceConfigEntryV1, SoraServiceConfigMutationV1,
            SoraServiceExactCurrentRevisionPreconditionV1, SoraServiceExecutionPlaneV1,
            SoraServiceHandlerClassV1, SoraServiceHandlerV1,
            SoraServiceLeaseReportingEpochRolloverV1, SoraServiceLeaseStateV1,
            SoraServiceLeaseStatusV1, SoraServiceLeaseUsageAuditV1, SoraServiceLifecycleActionV1,
            SoraServiceManifestV1, SoraServiceMutationPreconditionV1, SoraServiceRolloutStateV1,
            SoraServiceSecretMutationV1, SoraStateBindingV1, SoraStateEncryptionV1,
            SoraStateMutabilityV1, SoraStateScopeV1, SoraTlsModeV1, SoraTrainingJobStatusV1,
            SoraUploadedModelBundleV1, SoracloudMutationDraftResponse,
            encode_agent_artifact_allow_provenance_payload, encode_agent_deploy_provenance_payload,
            encode_agent_lease_renew_provenance_payload,
            encode_agent_message_ack_provenance_payload,
            encode_agent_message_send_provenance_payload,
            encode_agent_policy_revoke_provenance_payload, encode_agent_restart_provenance_payload,
            encode_agent_wallet_approve_provenance_payload,
            encode_agent_wallet_spend_provenance_payload, encode_app_infra_provenance_payload,
            encode_bundle_with_materials_provenance_payload,
            encode_delete_service_config_provenance_payload,
            encode_delete_service_secret_provenance_payload,
            encode_hf_shared_lease_join_provenance_payload,
            encode_hf_shared_lease_leave_provenance_payload,
            encode_hf_shared_lease_renew_provenance_payload,
            encode_model_artifact_register_provenance_payload,
            encode_model_weight_promote_provenance_payload,
            encode_model_weight_register_provenance_payload,
            encode_model_weight_rollback_provenance_payload, encode_rollback_provenance_payload,
            encode_rollout_provenance_payload, encode_set_service_config_provenance_payload,
            encode_set_service_secret_provenance_payload,
            encode_training_job_checkpoint_provenance_payload,
            encode_training_job_retry_provenance_payload,
            encode_training_job_start_provenance_payload,
            encode_uploaded_model_bundle_register_provenance_payload,
            encode_uploaded_model_finalize_provenance_payload,
            is_canonical_agent_wallet_request_id_v1, is_canonical_hf_commit_oid_v1,
            is_canonical_hf_repo_id_v1,
        },
        sorafs::pin_registry::{
            ManifestDigest, ManifestRootCid, PinManifestFinalizedRecordV1, PinStatus, StorageClass,
        },
        transaction::{Executable, FeePaymentIntent, SignedTransaction},
    },
};
#[cfg(unix)]
use iroha_config::{
    base::toml::{MAX_TOML_SOURCE_BYTES, TomlSource},
    parameters::{actual, defaults},
};
use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
#[cfg(test)]
use iroha_model_base::peer::PeerId;
#[cfg(test)]
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::{json::Json, numeric::Quantity};
#[cfg(test)]
use iroha_torii_shared::{
    FeeQuoteDecision, FeeQuoteObservation, FeeQuoteRequest as FeeQuoteWireRequest,
};
use iroha_torii_shared::{FeeQuoteResponse, PipelineTransactionStatusResponse};
use iroha_version::codec::DecodeVersioned as _;
use norito::json::{self, JsonDeserialize, JsonSerialize};
use rand::{
    rand_core::{TryCryptoRng, TryRngCore as _},
    rngs::OsRng,
};
use reqwest::{
    blocking::Client as BlockingHttpClient,
    header::{self, HeaderValue},
};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, CarWriter, ChunkStore, DirectoryPayload, FilePlan,
    PayloadSource,
    bundle_archive::{
        BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES, BundleArchiveFile, write_gzip_ustar,
    },
    compute_por_root,
    verifier::CarVerifier,
};
use sorafs_manifest::{
    ChunkingProfileV1, CouncilSignature, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
    MetadataEntry, PinPolicy, PinPolicyConstraints, StorageClass as ManifestStorageClass,
    chunker_registry,
    operator_preseed::{
        OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1, OPERATOR_PRESEED_SESSION_MAX_STORES_V1,
        OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1, OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1,
        OperatorPreseedArtifactReceiptV1, OperatorPreseedSessionReceiptV1,
        OperatorPreseedTargetReceiptV1,
    },
    validate_manifest,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read as _, Seek as _, SeekFrom, Write as _},
    marker::PhantomData,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command as ProcessCommand, Stdio},
    rc::Rc,
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tiny_keccak::{Hasher as _, Sha3};
#[cfg(unix)]
use zeroize::{Zeroize as _, Zeroizing};

/// CLI-only source image metadata. The explicit unit field forces the workspace
/// JSON key to be exactly `null`; published objects belong only to admitted
/// [`SoraInrouGuestImageV1`] values.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct UnpublishedInrouGuestImageWorkspaceV1 {
    kernel_image_path: String,
    rootfs_image_path: String,
    #[norito(required)]
    initrd_image_path: Option<String>,
    published_artifact: (),
}

/// CLI-only Inrou source manifest used before guest-image publication.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct UnpublishedInrouManifestWorkspaceV1 {
    schema_version: u16,
    guest_images: BTreeMap<String, UnpublishedInrouGuestImageWorkspaceV1>,
}

/// CLI-only container source manifest. It cannot be encoded as an admitted
/// deployment because its Inrou image references are strict-null markers.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct UnpublishedContainerManifestV1 {
    schema_version: u16,
    runtime: SoraContainerRuntimeV1,
    bundle_hash: Hash,
    bundle_path: String,
    entrypoint: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    #[norito(required)]
    inrou: Option<UnpublishedInrouManifestWorkspaceV1>,
    required_config_names: Vec<String>,
    required_secret_names: Vec<String>,
    config_exports: Vec<SoraConfigExportV1>,
    capabilities: SoraCapabilityPolicyV1,
    resources: SoraResourceLimitsV1,
    lifecycle: SoraLifecycleHooksV1,
}

/// Private pre-publication bundle. No signing or submission API accepts it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct UnpublishedDeploymentBundleV1 {
    container: UnpublishedContainerManifestV1,
    service: SoraServiceManifestV1,
}

fn parse_unpublished_inrou_guest_isa(value: &str) -> Result<SoraInrouGuestIsaV1> {
    SoraInrouGuestIsaV1::parse_key(value).ok_or_else(|| {
        eyre!("unpublished Inrou workspace contains unsupported guest ISA `{value}`")
    })
}

impl UnpublishedContainerManifestV1 {
    fn from_non_inrou_manifest(container: SoraContainerManifestV1) -> Result<Self> {
        if container.inrou.is_some() {
            return Err(eyre!(
                "an admitted Inrou manifest cannot be converted back into an unpublished workspace"
            ));
        }
        Ok(Self {
            schema_version: container.schema_version,
            runtime: container.runtime,
            bundle_hash: container.bundle_hash,
            bundle_path: container.bundle_path,
            entrypoint: container.entrypoint,
            args: container.args,
            env: container.env,
            inrou: None,
            required_config_names: container.required_config_names,
            required_secret_names: container.required_secret_names,
            config_exports: container.config_exports,
            capabilities: container.capabilities,
            resources: container.resources,
            lifecycle: container.lifecycle,
        })
    }

    fn workspace_hash(&self) -> Result<Hash> {
        if self.runtime != SoraContainerRuntimeV1::Inrou {
            let admitted = self.clone().into_admitted(BTreeMap::new())?;
            return Ok(Hash::new(Encode::encode(&admitted)));
        }
        let bytes = json::to_vec(self).wrap_err("encode unpublished container workspace")?;
        Ok(Hash::new(&bytes))
    }

    fn into_admitted(
        self,
        mut published_artifacts: BTreeMap<
            SoraInrouGuestIsaV1,
            SoraPublishedInrouGuestImageArtifactV1,
        >,
    ) -> Result<SoraContainerManifestV1> {
        let inrou = self
            .inrou
            .map(|inrou| {
                let mut guest_images = BTreeMap::new();
                for (guest_isa, image) in inrou.guest_images {
                    let guest_isa = parse_unpublished_inrou_guest_isa(&guest_isa)?;
                    let published_artifact = published_artifacts.remove(&guest_isa).ok_or_else(|| {
                        eyre!(
                            "unpublished Inrou workspace guest ISA `{}` was not published",
                            guest_isa.as_str()
                        )
                    })?;
                    if guest_images
                        .insert(
                            guest_isa,
                            SoraInrouGuestImageV1 {
                                kernel_image_path: image.kernel_image_path,
                                rootfs_image_path: image.rootfs_image_path,
                                initrd_image_path: image.initrd_image_path,
                                published_artifact,
                            },
                        )
                        .is_some()
                    {
                        return Err(eyre!(
                            "unpublished Inrou workspace repeats guest ISA `{}`",
                            guest_isa.as_str()
                        ));
                    }
                }
                if !published_artifacts.is_empty() {
                    return Err(eyre!(
                        "publication returned guest-image artifacts absent from the source workspace"
                    ));
                }
                Ok(SoraInrouManifestV1 {
                    schema_version: inrou.schema_version,
                    guest_images,
                })
            })
            .transpose()?;
        if inrou.is_none() && !published_artifacts.is_empty() {
            return Err(eyre!(
                "publication returned Inrou artifacts for a non-Inrou workspace"
            ));
        }
        Ok(SoraContainerManifestV1 {
            schema_version: self.schema_version,
            runtime: self.runtime,
            bundle_hash: self.bundle_hash,
            bundle_path: self.bundle_path,
            entrypoint: self.entrypoint,
            args: self.args,
            env: self.env,
            inrou,
            required_config_names: self.required_config_names,
            required_secret_names: self.required_secret_names,
            config_exports: self.config_exports,
            capabilities: self.capabilities,
            resources: self.resources,
            lifecycle: self.lifecycle,
        })
    }
}

impl UnpublishedDeploymentBundleV1 {
    fn into_admitted(
        self,
        published_artifacts: BTreeMap<SoraInrouGuestIsaV1, SoraPublishedInrouGuestImageArtifactV1>,
    ) -> Result<SoraDeploymentBundleV1> {
        let container = self.container.into_admitted(published_artifacts)?;
        let mut service = self.service;
        service.container.manifest_hash = Hash::new(Encode::encode(&container));
        service.container.expected_schema_version = container.schema_version;
        let bundle = SoraDeploymentBundleV1 {
            schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
            container,
            service,
        };
        bundle
            .validate_for_admission()
            .wrap_err("published deployment bundle failed first-release admission")?;
        Ok(bundle)
    }
}

fn validate_unpublished_inrou_source(
    container: &UnpublishedContainerManifestV1,
    service: &SoraServiceManifestV1,
    inrou: &UnpublishedInrouManifestWorkspaceV1,
) -> Result<()> {
    if service.execution_plane != SoraServiceExecutionPlaneV1::HttpService {
        return Err(eyre!(
            "unpublished Inrou workspaces require the HttpService execution plane"
        ));
    }
    if inrou.schema_version != SORA_INROU_MANIFEST_VERSION_V1 {
        return Err(eyre!(
            "unpublished Inrou schema_version must equal {SORA_INROU_MANIFEST_VERSION_V1}"
        ));
    }
    if inrou.guest_images.is_empty() {
        return Err(eyre!(
            "unpublished Inrou workspace must include at least one guest image"
        ));
    }
    SoraContainerManifestV1::validate_inrou_entrypoint(&container.entrypoint)
        .wrap_err("validate unpublished Inrou entrypoint")?;
    container
        .capabilities
        .network
        .validate_for_inrou()
        .wrap_err("validate unpublished Inrou egress policy")?;
    for (guest_isa, image) in &inrou.guest_images {
        parse_unpublished_inrou_guest_isa(guest_isa)?;
        SoraInrouGuestImageV1::validate_source_fields(
            &image.kernel_image_path,
            &image.rootfs_image_path,
            image.initrd_image_path.as_deref(),
        )
        .wrap_err("validate unpublished Inrou guest-image source fields")?;
    }
    Ok(())
}

fn validate_unpublished_deployment_source(bundle: &UnpublishedDeploymentBundleV1) -> Result<()> {
    bundle
        .service
        .validate()
        .wrap_err("validate unpublished service manifest")?;
    if bundle.container.schema_version != SORA_CONTAINER_MANIFEST_VERSION_V1 {
        return Err(eyre!(
            "unpublished container schema_version must equal {SORA_CONTAINER_MANIFEST_VERSION_V1}"
        ));
    }
    if bundle.service.container.expected_schema_version != bundle.container.schema_version {
        return Err(eyre!(
            "unpublished service container schema reference {} does not match source schema {}",
            bundle.service.container.expected_schema_version,
            bundle.container.schema_version
        ));
    }
    let workspace_hash = bundle.container.workspace_hash()?;
    if bundle.service.container.manifest_hash != workspace_hash {
        return Err(eyre!(
            "unpublished service container hash {} does not match source workspace hash {workspace_hash}",
            bundle.service.container.manifest_hash
        ));
    }
    let shared_projection = SoraContainerManifestV1 {
        schema_version: bundle.container.schema_version,
        runtime: SoraContainerRuntimeV1::Ivm,
        bundle_hash: bundle.container.bundle_hash,
        bundle_path: bundle.container.bundle_path.clone(),
        entrypoint: bundle.container.entrypoint.clone(),
        args: bundle.container.args.clone(),
        env: bundle.container.env.clone(),
        inrou: None,
        required_config_names: bundle.container.required_config_names.clone(),
        required_secret_names: bundle.container.required_secret_names.clone(),
        config_exports: bundle.container.config_exports.clone(),
        capabilities: bundle.container.capabilities.clone(),
        resources: bundle.container.resources,
        lifecycle: bundle.container.lifecycle.clone(),
    };
    shared_projection
        .validate()
        .wrap_err("validate unpublished container fields shared by every runtime")?;
    SoraDeploymentBundleV1::validate_source_compatibility(
        bundle.container.runtime,
        &bundle.container.capabilities,
        bundle.container.resources,
        &bundle.container.lifecycle,
        &bundle.service,
    )
    .wrap_err("validate unpublished deployment source compatibility")?;
    match (&bundle.container.runtime, &bundle.container.inrou) {
        (SoraContainerRuntimeV1::Inrou, Some(inrou)) => {
            validate_unpublished_inrou_source(&bundle.container, &bundle.service, inrou)?;
        }
        (SoraContainerRuntimeV1::Inrou, None) => {
            return Err(eyre!(
                "unpublished Inrou workspace is missing its Inrou manifest"
            ));
        }
        (_, Some(_)) => {
            return Err(eyre!(
                "only unpublished Inrou workspaces may carry Inrou metadata"
            ));
        }
        (_, None) => {
            bundle
                .clone()
                .into_admitted(BTreeMap::new())
                .wrap_err("validate non-Inrou workspace as an admitted deployment")?;
        }
    }
    Ok(())
}
macro_rules! define_torii_args {
    (
        $url_doc:literal, $token_doc:literal, $timeout_doc:literal;
        $(#[$meta:meta])*
        pub struct $name:ident { $($fields:tt)* }
    ) => {
        $(#[$meta])*
        #[derive(clap::Args, Debug)]
        pub struct $name {
            $($fields)*
            #[doc = $url_doc]
            #[arg(long, value_name = "URL")]
            torii_url: Option<String>,
            #[doc = $token_doc]
            #[arg(long, value_name = "TOKEN")]
            api_token: Option<String>,
            #[doc = $timeout_doc]
            #[arg(
                long,
                value_name = "SECS",
                default_value_t = 10,
                value_parser = clap::value_parser!(u64).range(1..)
            )]
            timeout_secs: u64,
        }
    };
}
const DEFAULT_CONTAINER_MANIFEST: &str = "fixtures/soracloud/sora_container_manifest_v1.json";
const DEFAULT_SERVICE_MANIFEST: &str = "fixtures/soracloud/sora_service_manifest_v1.json";
const DEFAULT_AGENT_APARTMENT_MANIFEST: &str =
    "fixtures/soracloud/agent_apartment_manifest_v1.json";
const SORACLOUD_APP_MANIFEST_VERSION_V1: u16 = 1;
const SORACLOUD_STATUS_SCHEMA_VERSION_V1: u16 = 1;
const AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS: u64 = 10_000;
const AGENT_AUTONOMY_MAX_HASH_BYTES: usize = 256;
const AGENT_AUTONOMY_MAX_REQUEST_BYTES: usize = 16 * 1024;
const APP_STATIC_SITE_CONFIG_NAME: &str = "soracloud/app_static_site";
const PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME: &str = "soracloud/public_service_discovery";
const APP_STATIC_SITE_BINDING_SCHEMA_VERSION_V1: u16 = 1;
const PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1: u16 = 1;
const APP_STATIC_SITE_INDEX_DOCUMENT: &str = "index.html";
const PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT: &str = "index.json";
const SORAFS_RELEASE_RETENTION_EPOCH_METADATA_KEY_V1: &str = "soracloud.retention_epoch";
const TAIRA_INROU_STAGE_SCHEMA_VERSION_V1: u16 = 1;
const TAIRA_INROU_WORKSPACE_SCHEMA_VERSION_V1: u16 = 1;
const TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1: &str = "container_manifest.json";
const TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1: &str = "service_manifest.json";
const TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1: &str = "bundle.tgz";
const TAIRA_INROU_WORKSPACE_KERNEL_FILE_V1: &str = "inrou/aarch64/vmlinux";
const TAIRA_INROU_WORKSPACE_ROOTFS_FILE_V1: &str = "inrou/aarch64/rootfs.ext4";
const TAIRA_INROU_WORKSPACE_INITRD_FILE_V1: &str = "inrou/aarch64/initrd.img";
const TAIRA_INROU_STAGE_RECEIPT_FILE_V1: &str = "receipt.json";
const TAIRA_INROU_STAGE_CONTAINER_FILE_V1: &str = "container.json";
const TAIRA_INROU_STAGE_SERVICE_FILE_V1: &str = "service.json";
const TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1: &str = "payloads/bundle.bin";
const TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1: &str = "payloads/guest";
const TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1: &str = "payloads/discovery";
const TAIRA_INROU_STAGE_DISCOVERY_DOCUMENT_FILE_V1: &str = "payloads/discovery/index.json";
const TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1: &str = "manifests/bundle.to";
const TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1: &str = "manifests/aarch64.to";
const TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1: &str = "manifests/discovery.to";
const TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1: u64 = 1024 * 1024;
const TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1: u64 = defaults::taira::INROU_GUEST_IMAGE_MAX_BYTES;
const TAIRA_INROU_STAGE_STREAM_BUFFER_BYTES: usize = 1024 * 1024;
const TAIRA_INROU_CANARY_SERVICE_NAME_V1: &str = "taira_inrou_canary";
const TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1: &str = "artifact-";
const TAIRA_INROU_CANARY_ROUTE_HOST_V1: &str = "taira.sora.org";
const TAIRA_INROU_CANARY_ROUTE_PREFIX_V1: &str = "/api/v1/inrou-canary";
const TAIRA_INROU_CANARY_SERVICE_PORT_V1: u16 = 8787;
const TAIRA_INROU_CANARY_ENTRYPOINT_V1: &str = "/app/server.py";
const TAIRA_INROU_CANARY_HEALTHCHECK_V1: &str = "/health";
const TAIRA_INROU_CANARY_HTTP_SERVICE_ENV_V1: &str = "HTTP_SERVICE_NAME";
const TAIRA_INROU_CANARY_BUNDLE_MEMBER_V1: &str = "app/server.py";
const TAIRA_INROU_CANARY_SERVER_SOURCE_V1: &[u8] =
    include_bytes!("soracloud/taira_inrou_canary_server_v1.py");
const TAIRA_INROU_CANARY_CONTAINER_TEMPLATE_V1: &str =
    include_str!("../../../fixtures/soracloud/sora_container_manifest_v1.json");
const TAIRA_INROU_CANARY_SERVICE_TEMPLATE_V1: &str =
    include_str!("../../../fixtures/soracloud/sora_service_manifest_v1.json");
const TAIRA_INROU_CANARY_KERNEL_PATH_V1: &str = "/inrou/aarch64/vmlinux";
const TAIRA_INROU_CANARY_ROOTFS_PATH_V1: &str = "/inrou/aarch64/rootfs.ext4";
const TAIRA_INROU_CANARY_INITRD_PATH_V1: &str = "/inrou/aarch64/initrd.img";
const TAIRA_INROU_CANARY_CPU_MILLIS_V1: u32 = defaults::taira::INROU_CANARY_CPU_MILLIS;
const TAIRA_INROU_CANARY_MEMORY_BYTES_V1: u64 = defaults::taira::INROU_CANARY_MEMORY_BYTES;
const TAIRA_INROU_CANARY_EPHEMERAL_STORAGE_BYTES_V1: u64 =
    defaults::taira::INROU_CANARY_EPHEMERAL_STORAGE_BYTES;
const TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1: u64 =
    defaults::taira::INROU_CANARY_ROOT_VOLUME_BYTES;
const TAIRA_INROU_CANARY_SHARED_VOLUME_BYTES_V1: u64 =
    defaults::taira::INROU_CANARY_SHARED_VOLUME_BYTES;
const TAIRA_INROU_CANARY_HOST_STORAGE_BYTES_V1: u64 =
    defaults::taira::INROU_CANARY_HOST_STORAGE_BYTES;
const TAIRA_INROU_CANARY_MAX_OPEN_FILES_PER_PROCESS_V1: u32 = 512;
const TAIRA_INROU_CANARY_MAX_TASKS_V1: u16 = 64;
const INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES: u64 = BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES;
const INROU_BUNDLE_PACK_MAX_SOURCE_BYTES: u64 = INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES - 1024 * 1024;
const INROU_BUNDLE_PACK_TEMP_ATTEMPTS: usize = 16;
const SORACLOUD_ARTIFACT_MIN_REPLICAS_V1: usize = 3;
const HEADER_IROHA_ACCOUNT: &str = "X-Iroha-Account";
const HEADER_IROHA_TIMESTAMP_MS: &str = "X-Iroha-Timestamp-Ms";
const HEADER_IROHA_NONCE: &str = "X-Iroha-Nonce";
const HEADER_IROHA_SIGNATURE: &str = "X-Iroha-Signature";
const HEADER_IROHA_WITNESS: &str = "X-Iroha-Witness";
const SORACLOUD_HTTP_WITNESS_FILE_MAX_BYTES_V1: u64 =
    (CANONICAL_REQUEST_WITNESS_MAX_DECODED_BYTES_V1 * 2) as u64;
#[derive(Clone)]
struct SoracloudInvocationContext {
    submission_config: ClientConfig,
    http_witness_file: Option<PathBuf>,
    fee_payment: Result<FeePaymentIntent, String>,
}
thread_local! {
    static SORACLOUD_INVOCATION_CONTEXT: RefCell<Option<SoracloudInvocationContext>> = const { RefCell::new(None) };
}
struct SoracloudInvocationGuard {
    previous: Option<SoracloudInvocationContext>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}
impl SoracloudInvocationGuard {
    fn install(context: SoracloudInvocationContext) -> Self {
        Self {
            previous: SORACLOUD_INVOCATION_CONTEXT.with(|slot| slot.replace(Some(context))),
            _not_send_or_sync: PhantomData,
        }
    }
}
impl Drop for SoracloudInvocationGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        SORACLOUD_INVOCATION_CONTEXT.with(|slot| {
            slot.replace(previous);
        });
    }
}
#[cfg(test)]
thread_local! {
    static SORACLOUD_TEST_SUBMITTED_TX_HASH: RefCell<Option<Hash>> = const { RefCell::new(None) };
}
/// Soracloud control-plane commands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Scaffold and deploy multi-service Soracloud apps.
    #[command(subcommand)]
    App(AppCommand),
    /// Single-service Soracloud manifest and control-plane helpers.
    #[command(subcommand)]
    Service(ServiceCommand),
    /// Soracloud model-training and model-registry helpers.
    #[command(subcommand)]
    Model(ModelCommand),
    /// Inert Hugging Face source metadata and shared storage-lease helpers.
    #[command(subcommand)]
    Hf(HfCommand),
    /// Persistent Soracloud agent/apartment helpers.
    #[command(subcommand)]
    Agent(AgentCommand),
}
impl Command {
    pub(crate) fn allows_fallback_config(&self) -> bool {
        match self {
            Self::App(command) => command.allows_fallback_config(),
            Self::Service(command) => command.allows_fallback_config(),
            Self::Model(command) => command.allows_fallback_config(),
            Self::Hf(command) => command.allows_fallback_config(),
            Self::Agent(command) => command.allows_fallback_config(),
        }
    }
}
#[derive(clap::Subcommand, Debug)]
pub enum ServiceCommand {
    /// Scaffold baseline container/service manifests.
    Init(InitArgs),
    /// Pack one regular file into a deterministic canonical Inrou bundle.
    #[command(name = "bundle-pack")]
    BundlePack(BundlePackArgs),
    /// Validate one service pair locally and print the local runtime/workspace plan.
    #[command(name = "plan")]
    LocalPlan(LocalPlanArgs),
    /// Run the manifest-adjacent local dev entrypoint for a single service workspace.
    #[command(name = "dev")]
    LocalDev(LocalDevArgs),
    /// Run the manifest-adjacent build and sync entrypoint for a single service workspace.
    #[command(name = "build")]
    BuildAndSync(BuildAndSyncArgs),
    /// Run the manifest-adjacent deploy entrypoint for a single service workspace.
    DeployWorkspace(WorkspaceMutationArgs),
    /// Run the manifest-adjacent upgrade entrypoint for a single service workspace.
    UpgradeWorkspace(WorkspaceMutationArgs),
    /// Recompute Soracloud manifest hashes after local edits or bundle rebuilds.
    SyncManifests(SyncManifestsArgs),
    /// Qualify exact Inrou artifacts in offline validator stores and emit a durable receipt.
    #[command(name = "preseed")]
    Preseed(InrouServicePreseedArgs),
    /// Validate manifests and register a new service deployment.
    Deploy(DeployArgs),
    /// Show authoritative Soracloud service state (all services or one service).
    Status(StatusArgs),
    /// Record or replace an authoritative service config entry.
    ConfigSet(ConfigSetArgs),
    /// Delete an authoritative service config entry.
    ConfigDelete(ConfigDeleteArgs),
    /// Query authoritative service config state.
    ConfigStatus(ConfigStatusArgs),
    /// Record or replace an authoritative service secret entry.
    SecretSet(SecretSetArgs),
    /// Delete an authoritative service secret entry.
    SecretDelete(SecretDeleteArgs),
    /// Query authoritative service secret state.
    SecretStatus(SecretStatusArgs),
    /// Validate manifests and upgrade an existing deployed service.
    Upgrade(UpgradeArgs),
    /// Roll back a deployed service to an explicitly selected admitted version.
    Rollback(RollbackArgs),
    /// Advance or fail a rollout step using health-gated canary controls.
    Rollout(RolloutArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum AgentCommand {
    /// Register a persistent AI apartment manifest in the live control plane.
    #[command(name = "deploy")]
    Deploy(AgentDeployArgs),
    /// Renew an apartment lease in the live control plane.
    #[command(name = "lease-renew")]
    LeaseRenew(AgentLeaseRenewArgs),
    /// Request deterministic apartment restart in the live control plane.
    #[command(name = "restart")]
    Restart(AgentRestartArgs),
    /// Show authoritative apartment runtime status.
    #[command(name = "status")]
    Status(AgentStatusArgs),
    /// Submit an apartment wallet spend request under policy guardrails.
    #[command(name = "wallet-spend")]
    WalletSpend(AgentWalletSpendArgs),
    /// Approve a pending apartment wallet spend request.
    #[command(name = "wallet-approve")]
    WalletApprove(AgentWalletApproveArgs),
    /// Revoke an apartment policy capability.
    #[command(name = "policy-revoke")]
    PolicyRevoke(AgentPolicyRevokeArgs),
    /// Send a deterministic mailbox message between apartments.
    #[command(name = "message-send")]
    MessageSend(AgentMessageSendArgs),
    /// Acknowledge (consume) a mailbox message from an apartment queue.
    #[command(name = "message-ack")]
    MessageAck(AgentMessageAckArgs),
    /// Inspect mailbox queue state for an apartment.
    #[command(name = "mailbox-status")]
    MailboxStatus(AgentMailboxStatusArgs),
    /// Add an artifact hash (and optional provenance hash) to autonomy allowlist.
    #[command(name = "artifact-allow")]
    ArtifactAllow(AgentArtifactAllowArgs),
    /// Show autonomous-run policy state for an apartment.
    #[command(name = "autonomy-status")]
    AutonomyStatus(AgentAutonomyStatusArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum ModelCommand {
    /// Start a distributed training job in live Torii control-plane mode.
    TrainingJobStart(TrainingJobStartArgs),
    /// Record a training checkpoint in live Torii control-plane mode.
    TrainingJobCheckpoint(TrainingJobCheckpointArgs),
    /// Submit a training retry request in live Torii control-plane mode.
    TrainingJobRetry(TrainingJobRetryArgs),
    /// Query training job status in live Torii control-plane mode.
    TrainingJobStatus(TrainingJobStatusArgs),
    /// Register model-artifact metadata in live Torii control-plane mode.
    #[command(name = "artifact-register")]
    ArtifactRegister(ModelArtifactRegisterArgs),
    /// Query model-artifact status in live Torii control-plane mode.
    #[command(name = "artifact-status")]
    ArtifactStatus(ModelArtifactStatusArgs),
    /// Register a model weight version in live Torii control-plane mode.
    #[command(name = "weight-register")]
    WeightRegister(ModelWeightRegisterArgs),
    /// Promote a model weight version in live Torii control-plane mode.
    #[command(name = "weight-promote")]
    WeightPromote(ModelWeightPromoteArgs),
    /// Roll back a model weight version in live Torii control-plane mode.
    #[command(name = "weight-rollback")]
    WeightRollback(ModelWeightRollbackArgs),
    /// Query model weight status in live Torii control-plane mode.
    #[command(name = "weight-status")]
    WeightStatus(ModelWeightStatusArgs),
    /// Register a SoraFS-backed uploaded-model bundle into the model registry.
    #[command(name = "upload-register")]
    UploadRegister(ModelUploadRegisterArgs),
    /// Query SoraFS-backed uploaded-model storage and registry status.
    #[command(name = "upload-status")]
    UploadStatus(ModelUploadStatusArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum HfCommand {
    /// Register immutable source metadata and join or create its shared storage lease.
    #[command(name = "join")]
    Join(HfSharedLeaseJoinArgs),
    /// Query immutable source metadata and shared storage-lease status.
    #[command(name = "status")]
    Status(HfStatusArgs),
    /// Leave an immutable source's shared storage lease.
    #[command(name = "lease-leave")]
    LeaseLeave(HfLeaseLeaveArgs),
    /// Renew an immutable source's expired or drained shared storage-lease window.
    #[command(name = "lease-renew")]
    LeaseRenew(HfLeaseRenewArgs),
}
#[derive(clap::Subcommand, Debug)]
pub enum AppCommand {
    /// Scaffold a buildable single-service or split-plane Soracloud app workspace.
    Init(AppInitArgs),
    /// Validate a mixed app manifest and print the local split-plane route/runtime plan.
    #[command(name = "plan")]
    LocalPlan(AppLocalPlanArgs),
    /// Fail-closed validation for a scaffolded app workspace before release.
    Doctor(AppDoctorArgs),
    /// Run the manifest-adjacent local dev entrypoint for a scaffolded app workspace.
    #[command(name = "dev")]
    LocalDev(AppLocalDevArgs),
    /// Run the manifest-adjacent app build and manifest-sync entrypoint.
    #[command(name = "build")]
    BuildAndSync(AppBuildAndSyncArgs),
    /// Simulate a prod-like release locally without live Torii mutation.
    Simulate(AppSimulateArgs),
    /// Qualify every hosted Inrou artifact offline and emit one durable app receipt.
    #[command(name = "preseed")]
    Preseed(InrouAppPreseedArgs),
    /// Build, validate, deploy, and live-verify every service referenced by an app manifest.
    Release(AppReleaseArgs),
    /// Show app-scoped Soracloud service status from the control plane.
    Status(AppStatusArgs),
}
impl AppCommand {
    fn allows_fallback_config(&self) -> bool {
        matches!(
            self,
            Self::Init(_)
                | Self::LocalPlan(_)
                | Self::Doctor(_)
                | Self::LocalDev(_)
                | Self::BuildAndSync(_)
                | Self::Simulate(_)
        )
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Init(args) => context.print_data(&args.run()?),
            Self::LocalPlan(args) => context.print_data(&args.run()?),
            Self::Doctor(args) => context.print_data(&args.run()?),
            Self::LocalDev(args) => context.print_data(&args.run()?),
            Self::BuildAndSync(args) => context.print_data(&args.run()?),
            Self::Simulate(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Preseed(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Release(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Status(args) => context.print_data(&args.run()?),
        }
    }
}
impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let _invocation = SoracloudInvocationGuard::install(SoracloudInvocationContext {
            submission_config: context.config().clone(),
            http_witness_file: context.soracloud_http_witness_file().map(Path::to_path_buf),
            fee_payment: context
                .transaction_fee_payment()
                .map_err(|error| format!("{error:#}")),
        });
        match self {
            Command::App(command) => command.run(context),
            Command::Service(command) => command.run(context),
            Command::Model(command) => command.run(context),
            Command::Hf(command) => command.run(context),
            Command::Agent(command) => command.run(context),
        }
    }
}
impl ServiceCommand {
    fn allows_fallback_config(&self) -> bool {
        matches!(
            self,
            Self::Init(_)
                | Self::BundlePack(_)
                | Self::LocalPlan(_)
                | Self::LocalDev(_)
                | Self::BuildAndSync(_)
                | Self::DeployWorkspace(_)
                | Self::UpgradeWorkspace(_)
                | Self::SyncManifests(_)
        )
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Init(args) => context.print_data(&args.run()?),
            Self::BundlePack(args) => context.print_data(&args.run()?),
            Self::LocalPlan(args) => context.print_data(&args.run()?),
            Self::LocalDev(args) => context.print_data(&args.run()?),
            Self::BuildAndSync(args) => context.print_data(&args.run()?),
            Self::DeployWorkspace(args) => context.print_data(&args.run(MutationMode::Deploy)?),
            Self::UpgradeWorkspace(args) => context.print_data(&args.run(MutationMode::Upgrade)?),
            Self::SyncManifests(args) => context.print_data(&args.run()?),
            Self::Preseed(args) => {
                let output = args.run(&context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Deploy(args) => {
                let output = args.run(
                    MutationMode::Deploy,
                    &context.config().account,
                    &context.config().key_pair,
                )?;
                context.print_data(&output)
            }
            Self::Status(args) => context.print_data(&args.run()?),
            Self::ConfigSet(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::ConfigDelete(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::ConfigStatus(args) => context.print_data(&args.run()?),
            Self::SecretSet(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::SecretDelete(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::SecretStatus(args) => context.print_data(&args.run()?),
            Self::Upgrade(args) => {
                let output = args.run(
                    MutationMode::Upgrade,
                    &context.config().account,
                    &context.config().key_pair,
                )?;
                context.print_data(&output)
            }
            Self::Rollback(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Rollout(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
        }
    }
}
impl AgentCommand {
    fn allows_fallback_config(&self) -> bool {
        false
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Deploy(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::LeaseRenew(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Restart(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Status(args) => context.print_data(&args.run()?),
            Self::WalletSpend(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::WalletApprove(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::PolicyRevoke(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::MessageSend(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::MessageAck(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::MailboxStatus(args) => context.print_data(&args.run()?),
            Self::ArtifactAllow(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::AutonomyStatus(args) => context.print_data(&args.run()?),
        }
    }
}
impl ModelCommand {
    fn allows_fallback_config(&self) -> bool {
        false
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::TrainingJobStart(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::TrainingJobCheckpoint(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::TrainingJobRetry(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::TrainingJobStatus(args) => context.print_data(&args.run()?),
            Self::ArtifactRegister(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::ArtifactStatus(args) => context.print_data(&args.run()?),
            Self::WeightRegister(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::WeightPromote(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::WeightRollback(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::WeightStatus(args) => context.print_data(&args.run()?),
            Self::UploadRegister(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::UploadStatus(args) => context.print_data(&args.run()?),
        }
    }
}
impl HfCommand {
    fn allows_fallback_config(&self) -> bool {
        false
    }
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Self::Join(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::Status(args) => context.print_data(&args.run()?),
            Self::LeaseLeave(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
            Self::LeaseRenew(args) => {
                let output = args.run(&context.config().account, &context.config().key_pair)?;
                context.print_data(&output)
            }
        }
    }
}
/// Arguments for `iroha soracloud service init`.
#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Directory where manifests and template artifacts will be created.
    #[arg(long, value_name = "DIR", default_value = ".soracloud")]
    output_dir: PathBuf,
    /// Logical service name used in the scaffolded service manifest.
    #[arg(long, value_name = "NAME", default_value = "web_portal")]
    service_name: String,
    /// Version string used in the scaffolded service manifest.
    #[arg(long, value_name = "VERSION", default_value = "0.1.0")]
    service_version: String,
    /// Scaffolding template to generate in addition to control-plane manifests.
    #[arg(long, value_enum, default_value_t = InitTemplate::Baseline)]
    template: InitTemplate,
    /// Overwrite existing files in the output directory.
    #[arg(long)]
    overwrite: bool,
}
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum InitTemplate {
    /// Generate only Soracloud control-plane manifests.
    #[default]
    Baseline,
    /// Generate a hosted HTTP Soracloud starter that targets Inrou.
    HttpService,
    /// Generate a Vue3/Vite static SPA starter with SoraFS publish workflow.
    Site,
    /// Generate a Vue3 SPA + API starter with deterministic challenge-signature auth.
    Webapp,
    /// Generate a private PII app starter with consent + retention workflows.
    PiiApp,
    /// Generate a Hayahi app starter with wallet sessions and Soracloud-backed state.
    HayahiApp,
}
impl InitTemplate {
    fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::HttpService => "http-service",
            Self::Site => "site",
            Self::Webapp => "webapp",
            Self::PiiApp => "pii-app",
            Self::HayahiApp => "hayahi-app",
        }
    }
}
impl InitArgs {
    fn run(self) -> Result<InitOutput> {
        fs::create_dir_all(&self.output_dir).wrap_err_with(|| {
            format!(
                "failed to create output directory {}",
                self.output_dir.display()
            )
        })?;
        let service_name = parse_exact_name_arg("--service-name", &self.service_name)?
            .parse::<Name>()
            .wrap_err("invalid --service-name for soracloud scaffold")?;
        let service_version =
            require_exact_nonempty_arg("--service-version", &self.service_version)?;
        let mut container = UnpublishedContainerManifestV1::from_non_inrou_manifest(load_json::<
            SoraContainerManifestV1,
        >(
            &workspace_fixture(DEFAULT_CONTAINER_MANIFEST),
        )?)?;
        let mut service =
            load_json::<SoraServiceManifestV1>(&workspace_fixture(DEFAULT_SERVICE_MANIFEST))?;
        apply_init_template_defaults(self.template, &service_name, &mut service, &mut container)?;
        service.service_name = service_name;
        service.service_version = service_version;
        let container_hash = container.workspace_hash()?;
        service.container.manifest_hash = container_hash;
        service.container.expected_schema_version = container.schema_version;
        let bundle = UnpublishedDeploymentBundleV1 {
            container: container.clone(),
            service: service.clone(),
        };
        validate_unpublished_deployment_source(&bundle)?;
        let container_path = self.output_dir.join("container_manifest.json");
        let service_path = self.output_dir.join("service_manifest.json");
        ensure_can_write(&container_path, self.overwrite)?;
        ensure_can_write(&service_path, self.overwrite)?;
        write_json(&container_path, &container)?;
        write_json(&service_path, &service)?;
        let template_artifacts = scaffold_init_template(
            self.template,
            &self.output_dir,
            service.service_name.as_ref(),
            self.overwrite,
        )?;
        Ok(InitOutput {
            template: self.template.as_str().to_owned(),
            container_manifest_path: container_path.to_string_lossy().into_owned(),
            service_manifest_path: service_path.to_string_lossy().into_owned(),
            container_manifest_hash: container_hash,
            service_manifest_hash: Hash::new(Encode::encode(&service)),
            template_artifacts,
        })
    }
}
/// Arguments for `soracloud service bundle-pack`.
#[derive(clap::Args, Debug)]
pub struct BundlePackArgs {
    /// Regular file, up to 511 MiB, whose bytes become the sole archive member.
    #[arg(long, value_name = "PATH")]
    source: PathBuf,
    /// Canonical relative path assigned to the file inside the bundle.
    #[arg(long, value_name = "ARCHIVE_PATH")]
    archive_path: String,
    /// Destination for the deterministic canonical gzip/USTAR archive (at most 512 MiB).
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
    /// Store the archive member with canonical executable mode 0755.
    #[arg(long, default_value_t = false)]
    executable: bool,
}
impl BundlePackArgs {
    fn run(self) -> Result<BundlePackOutput> {
        let source = read_stable_bundle_pack_source(&self.source)?;
        reject_bundle_pack_source_output_alias(&self.source, &self.output, &source)?;
        let archive_member_mode = if self.executable { 0o755 } else { 0o644 };
        let archive_file = BundleArchiveFile::new(
            self.archive_path.as_str(),
            archive_member_mode,
            &source.payload,
        );
        let mut staged = BundlePackAtomicOutput::create(&self.output)?;
        let written_bytes = {
            let archive_writer = BundlePackArchiveWriter::new(
                staged.file_mut()?,
                INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES,
            );
            let archive_writer =
                write_gzip_ustar(archive_writer, &[archive_file]).wrap_err_with(|| {
                    format!(
                        "failed to encode canonical Inrou bundle member `{}`",
                        self.archive_path
                    )
                })?;
            archive_writer.written_bytes()
        };
        ensure_bundle_pack_archive_size_within_limit(written_bytes).wrap_err_with(|| {
            format!(
                "canonical Inrou bundle member `{}` exceeded the archive limit",
                self.archive_path,
            )
        })?;
        source.revalidate(&self.source)?;
        let (bundle_hash, bundle_size_bytes) =
            staged.finish_and_install(&self.output, &self.source, &source, written_bytes)?;
        Ok(BundlePackOutput {
            source_file: self.source.to_string_lossy().into_owned(),
            source_size_bytes: source.snapshot.size(),
            archive_member_path: self.archive_path,
            archive_member_mode,
            bundle_file: self.output.to_string_lossy().into_owned(),
            bundle_size_bytes,
            bundle_hash,
        })
    }
}
/// Arguments for `soracloud service sync-manifests`.
#[derive(clap::Args, Debug)]
pub struct SyncManifestsArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document. When set, every
    /// referenced service manifest pair is synchronized.
    #[arg(long, value_name = "PATH")]
    app_manifest: Option<PathBuf>,
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Optional compiled IVM/native bundle file used to refresh `container.bundle_hash`.
    #[arg(long, value_name = "PATH")]
    bundle_file: Option<PathBuf>,
}
impl SyncManifestsArgs {
    fn run(self) -> Result<SyncManifestsOutput> {
        if let Some(app_manifest_path) = self.app_manifest.as_ref() {
            let manifest: SoracloudAppManifestV1 = load_json(app_manifest_path)?;
            manifest.validate()?;
            let manifest_dir = app_manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let services =
                sync_app_manifest_service_refs(&manifest, &manifest_dir).wrap_err_with(|| {
                    format!(
                        "failed to synchronize Soracloud app manifest {}",
                        app_manifest_path.display()
                    )
                })?;
            return Ok(SyncManifestsOutput {
                app_manifest_path: Some(app_manifest_path.to_string_lossy().into_owned()),
                container_manifest_path: None,
                service_manifest_path: None,
                container_manifest_hash: None,
                service_manifest_hash: None,
                bundle_file: None,
                bundle_hash: None,
                services,
            });
        }
        let synced = sync_manifest_pair(
            &self.container,
            &self.service,
            self.bundle_file.as_deref(),
            None,
        )?;
        Ok(SyncManifestsOutput {
            app_manifest_path: None,
            container_manifest_path: Some(synced.container_manifest_path.clone()),
            service_manifest_path: Some(synced.service_manifest_path.clone()),
            container_manifest_hash: Some(synced.container_manifest_hash),
            service_manifest_hash: Some(synced.service_manifest_hash),
            bundle_file: synced.bundle_file.clone(),
            bundle_hash: Some(synced.bundle_hash),
            services: Vec::new(),
        })
    }
}
/// Arguments for `soracloud service plan`.
#[derive(clap::Args, Debug)]
pub struct LocalPlanArgs {
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
}
impl LocalPlanArgs {
    fn run(self) -> Result<ServiceLocalPlanOutput> {
        build_service_local_plan_output(&self.container, &self.service)
    }
}
/// Arguments for `soracloud service dev`.
#[derive(clap::Args, Debug)]
pub struct LocalDevArgs {
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Print the resolved dev command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
impl LocalDevArgs {
    fn run(self) -> Result<ServiceWorkspaceScriptOutput> {
        let plan = build_service_workspace_plan(&self.container, &self.service)?;
        let ServiceWorkspacePlan {
            service_name,
            execution_plane,
            runtime,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            workspace_dir: _,
            workspace_scripts,
            notes,
        } = plan;
        let script_path =
            resolve_service_workspace_script(&self.container, &self.service, "dev.sh")?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let command = vec!["./dev.sh".to_owned()];
        if self.dry_run {
            return Ok(ServiceWorkspaceScriptOutput {
                service_name,
                container_manifest_path: self.container.to_string_lossy().into_owned(),
                service_manifest_path: self.service.to_string_lossy().into_owned(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                script_name: "dev.sh".to_owned(),
                mode: "dry_run".to_owned(),
                execution_plane,
                runtime,
                workspace_scripts,
                route_host,
                route_path_prefix,
                route_visibility,
                replica_count,
                state_binding_count,
                lease_volume_count,
                handler_count,
                routes,
                command,
                exit_status: None,
                notes,
            });
        }
        let status = ProcessCommand::new(&script_path)
            .current_dir(&working_dir)
            .status()
            .wrap_err_with(|| {
                format!(
                    "failed to run local dev script `{}` resolved from `{}` and `{}`",
                    script_path.display(),
                    self.container.display(),
                    self.service.display()
                )
            })?;
        let exit_status = status.code();
        if exit_status == Some(130) {
            let mut notes = notes;
            notes.push(
                "local dev entrypoint exited with status 130; treating the session as an interactive interrupt"
                    .to_owned(),
            );
            return Ok(ServiceWorkspaceScriptOutput {
                service_name,
                container_manifest_path: self.container.to_string_lossy().into_owned(),
                service_manifest_path: self.service.to_string_lossy().into_owned(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                script_name: "dev.sh".to_owned(),
                mode: "interrupted".to_owned(),
                execution_plane,
                runtime,
                workspace_scripts,
                route_host,
                route_path_prefix,
                route_visibility,
                replica_count,
                state_binding_count,
                lease_volume_count,
                handler_count,
                routes,
                command,
                exit_status,
                notes,
            });
        }
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "local dev script `{}` exited with status {rendered_status}",
                script_path.display()
            ));
        }
        Ok(ServiceWorkspaceScriptOutput {
            service_name,
            container_manifest_path: self.container.to_string_lossy().into_owned(),
            service_manifest_path: self.service.to_string_lossy().into_owned(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            script_name: "dev.sh".to_owned(),
            mode: "completed".to_owned(),
            execution_plane,
            runtime,
            workspace_scripts,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            command,
            exit_status,
            notes,
        })
    }
}
/// Arguments for `soracloud service build`.
#[derive(clap::Args, Debug)]
pub struct BuildAndSyncArgs {
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Print the resolved build-and-sync command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
impl BuildAndSyncArgs {
    fn run(self) -> Result<ServiceWorkspaceScriptOutput> {
        let plan = build_service_workspace_plan(&self.container, &self.service)?;
        let ServiceWorkspacePlan {
            service_name,
            execution_plane,
            runtime,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            workspace_dir: _,
            workspace_scripts,
            notes,
        } = plan;
        let script_path =
            resolve_service_workspace_script(&self.container, &self.service, "build-and-sync.sh")?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let command = vec!["./build-and-sync.sh".to_owned()];
        if self.dry_run {
            return Ok(ServiceWorkspaceScriptOutput {
                service_name,
                container_manifest_path: self.container.to_string_lossy().into_owned(),
                service_manifest_path: self.service.to_string_lossy().into_owned(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                script_name: "build-and-sync.sh".to_owned(),
                mode: "dry_run".to_owned(),
                execution_plane,
                runtime,
                workspace_scripts,
                route_host,
                route_path_prefix,
                route_visibility,
                replica_count,
                state_binding_count,
                lease_volume_count,
                handler_count,
                routes,
                command,
                exit_status: None,
                notes,
            });
        }
        let status = ProcessCommand::new(&script_path)
            .current_dir(&working_dir)
            .status()
            .wrap_err_with(|| {
                format!(
                    "failed to run build-and-sync script `{}` resolved from `{}` and `{}`",
                    script_path.display(),
                    self.container.display(),
                    self.service.display()
                )
            })?;
        let exit_status = status.code();
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "build-and-sync script `{}` exited with status {rendered_status}",
                script_path.display()
            ));
        }
        let mut notes = notes;
        notes.push("build-and-sync completed through the manifest-adjacent root script".to_owned());
        Ok(ServiceWorkspaceScriptOutput {
            service_name,
            container_manifest_path: self.container.to_string_lossy().into_owned(),
            service_manifest_path: self.service.to_string_lossy().into_owned(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            script_name: "build-and-sync.sh".to_owned(),
            mode: "completed".to_owned(),
            execution_plane,
            runtime,
            workspace_scripts,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            command,
            exit_status,
            notes,
        })
    }
}
/// Arguments for `soracloud service deploy` and `soracloud service upgrade`.
#[derive(clap::Args, Debug)]
pub struct WorkspaceMutationArgs {
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Exact Unix-second retention boundary forwarded unchanged to the workspace release script.
    /// Reuse the same value for every retry of one release.
    #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
    sorafs_retention_epoch: NonZeroU64,
    /// Optional JSON file containing a map of inline config values committed atomically with deploy or upgrade.
    #[arg(long, value_name = "PATH")]
    initial_configs: Option<PathBuf>,
    /// Optional JSON file containing a map of inline secret envelopes committed atomically with deploy or upgrade.
    #[arg(long, value_name = "PATH")]
    initial_secrets: Option<PathBuf>,
    /// Torii base URL forwarded to the workspace entrypoint through `TORII_URL`.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token forwarded to the workspace entrypoint through `API_TOKEN`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout forwarded to the underlying deploy or upgrade command.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
    /// Print the resolved deploy or upgrade command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
impl WorkspaceMutationArgs {
    fn run(self, mode: MutationMode) -> Result<ServiceWorkspaceMutationScriptOutput> {
        let plan = build_service_workspace_plan(&self.container, &self.service)?;
        let ServiceWorkspacePlan {
            service_name,
            execution_plane,
            runtime,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            workspace_dir: _,
            workspace_scripts,
            notes: plan_notes,
        } = plan;
        let script_name = mode.workspace_script_name();
        let script_path =
            resolve_service_workspace_script(&self.container, &self.service, script_name)?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let torii_url = require_torii_url(self.torii_url.as_deref())?.to_owned();
        let initial_configs =
            canonicalize_cli_arg_path(self.initial_configs.as_deref(), "--initial-configs")?;
        let initial_secrets =
            canonicalize_cli_arg_path(self.initial_secrets.as_deref(), "--initial-secrets")?;
        let sorafs_retention_epoch = self.sorafs_retention_epoch.get().to_string();
        let command = build_service_workspace_mutation_command(
            script_name,
            self.timeout_secs,
            initial_configs.as_deref(),
            initial_secrets.as_deref(),
        );
        let mut notes = plan_notes;
        notes.push(format!(
            "{} will run through the manifest-adjacent root script after exporting TORII_URL and the exact SoraFS retention epoch {}",
            mode.label_lowercase(),
            self.sorafs_retention_epoch
        ));
        if self.api_token.is_some() {
            notes.push(
                "API token will be forwarded through API_TOKEN instead of the command line"
                    .to_owned(),
            );
        }
        if self.dry_run {
            return Ok(ServiceWorkspaceMutationScriptOutput {
                service_name,
                container_manifest_path: self.container.to_string_lossy().into_owned(),
                service_manifest_path: self.service.to_string_lossy().into_owned(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                script_name: script_name.to_owned(),
                mode: "dry_run".to_owned(),
                execution_plane,
                runtime,
                workspace_scripts,
                route_host,
                route_path_prefix,
                route_visibility,
                replica_count,
                state_binding_count,
                lease_volume_count,
                handler_count,
                routes,
                torii_url,
                uses_api_token: self.api_token.is_some(),
                command,
                exit_status: None,
                notes,
            });
        }
        let mut process = ProcessCommand::new(&script_path);
        process
            .current_dir(&working_dir)
            .env("TORII_URL", &torii_url)
            .env("SORAFS_RETENTION_EPOCH", &sorafs_retention_epoch);
        if let Some(api_token) = self.api_token.as_deref() {
            process.env("API_TOKEN", api_token);
        }
        if let Some(path) = initial_configs.as_deref() {
            process.arg("--initial-configs").arg(path);
        }
        if let Some(path) = initial_secrets.as_deref() {
            process.arg("--initial-secrets").arg(path);
        }
        process
            .arg("--timeout-secs")
            .arg(self.timeout_secs.to_string());
        let status = process.status().wrap_err_with(|| {
            format!(
                "failed to run {} script `{}` resolved from `{}` and `{}`",
                mode.label_lowercase(),
                script_path.display(),
                self.container.display(),
                self.service.display()
            )
        })?;
        let exit_status = status.code();
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "{} script `{}` exited with status {rendered_status}",
                mode.label_lowercase(),
                script_path.display()
            ));
        }
        notes.push(format!(
            "{} completed through the manifest-adjacent root script",
            mode.label_lowercase()
        ));
        Ok(ServiceWorkspaceMutationScriptOutput {
            service_name,
            container_manifest_path: self.container.to_string_lossy().into_owned(),
            service_manifest_path: self.service.to_string_lossy().into_owned(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            script_name: script_name.to_owned(),
            mode: "completed".to_owned(),
            execution_plane,
            runtime,
            workspace_scripts,
            route_host,
            route_path_prefix,
            route_visibility,
            replica_count,
            state_binding_count,
            lease_volume_count,
            handler_count,
            routes,
            torii_url,
            uses_api_token: self.api_token.is_some(),
            command,
            exit_status,
            notes,
        })
    }
}
/// Arguments for `soracloud app init`.
#[derive(clap::Args, Debug)]
pub struct AppInitArgs {
    /// Directory where the app manifest and starter service manifests will be created.
    #[arg(long, value_name = "DIR", default_value = ".soracloud-app")]
    output_dir: PathBuf,
    /// Logical app name used in the scaffolded manifest.
    #[arg(long, value_name = "NAME", default_value = "sora_app")]
    app_name: String,
    /// Version string used in the scaffolded service manifest set.
    #[arg(long, value_name = "VERSION", default_value = "0.1.0")]
    app_version: String,
    /// App template to scaffold.
    #[arg(long, value_enum, default_value_t = AppInitTemplate::SingleApi)]
    template: AppInitTemplate,
    /// Optional public hostname used for the app URL and service routes.
    #[arg(long, value_name = "HOST")]
    public_host: Option<String>,
    /// Optional static frontend dist directory recorded in the app manifest.
    #[arg(long, value_name = "PATH")]
    static_site_dist_dir: Option<String>,
    /// Emit only control-plane manifests plus minimal root scripts for wiring the app into an existing repo.
    #[arg(long)]
    existing_repo: bool,
    /// Overwrite existing files in the output directory.
    #[arg(long)]
    overwrite: bool,
}
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum AppInitTemplate {
    /// Generate a root-bound app with a static frontend and one deterministic API service.
    #[default]
    SingleApi,
    /// Generate a split app with a static frontend, an Inrou live API, and an IVM vault API.
    SplitApp,
}
impl AppInitTemplate {
    fn as_str(self) -> &'static str {
        match self {
            Self::SingleApi => "single-api",
            Self::SplitApp => "split-app",
        }
    }
}
impl AppInitArgs {
    fn run(self) -> Result<AppInitOutput> {
        if self.existing_repo && self.template != AppInitTemplate::SplitApp {
            return Err(eyre!(
                "--existing-repo is only supported with --template split-app"
            ));
        }
        match self.template {
            AppInitTemplate::SingleApi => self.run_single_api(),
            AppInitTemplate::SplitApp => self.run_split_app(),
        }
    }
    fn run_single_api(self) -> Result<AppInitOutput> {
        fs::create_dir_all(&self.output_dir).wrap_err_with(|| {
            format!(
                "failed to create output directory {}",
                self.output_dir.display()
            )
        })?;
        let app_name = normalized_service_label(&self.app_name);
        let host = self.resolve_public_host(&app_name)?;
        let public_url = format!("https://{host}");
        let static_site_dist_dir = self.resolve_static_site_dist_dir("web/dist")?;
        let manifest_path = self.output_dir.join("app_manifest.json");
        ensure_can_write(&manifest_path, self.overwrite)?;
        let mut container =
            load_json::<SoraContainerManifestV1>(&workspace_fixture(DEFAULT_CONTAINER_MANIFEST))?;
        let mut service =
            load_json::<SoraServiceManifestV1>(&workspace_fixture(DEFAULT_SERVICE_MANIFEST))?;
        let api_service_name: Name = format!("{app_name}_api")
            .parse()
            .wrap_err("invalid derived api service name for soracloud app scaffold")?;
        container.runtime = SoraContainerRuntimeV1::Ivm;
        container.bundle_path = "/bundles/api-service.to".to_owned();
        container.entrypoint = "main".to_owned();
        container.args = vec!["--http".to_owned(), "--port=8787".to_owned()];
        container
            .env
            .insert("SORACLOUD_TEMPLATE".to_owned(), "single-api".to_owned());
        container.capabilities.network =
            SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                "torii.sora.internal",
                [443],
            )]);
        container.capabilities.allow_state_writes = true;
        container.capabilities.allow_model_training = false;
        container.lifecycle.healthcheck_path = Some("/healthz".to_owned());
        service.service_name = api_service_name.clone();
        service.service_version = self.app_version.clone();
        service.route = Some(SoraRouteTargetV1 {
            host: host.clone(),
            path_prefix: "/api".to_owned(),
            service_port: NonZeroU16::new(8787).expect("nonzero literal"),
            visibility: SoraRouteVisibilityV1::Public,
            tls_mode: SoraTlsModeV1::Required,
        });
        service.replicas = NonZeroU16::new(2).expect("nonzero literal");
        service.state_bindings.clear();
        service.handlers = vec![service_handler(
            "healthz",
            SoraServiceHandlerClassV1::Query,
            "serve_healthz",
            Some("/healthz"),
            SoraCertifiedResponsePolicyV1::AuditReceipt,
            None,
        )];
        service.artifacts.clear();
        let container_hash = Hash::new(Encode::encode(&container));
        service.container.manifest_hash = container_hash;
        service.container.expected_schema_version = container.schema_version;
        let api_dir = self.output_dir.join("services").join("api");
        let container_path = api_dir.join("container_manifest.json");
        let service_path = api_dir.join("service_manifest.json");
        let bundle_path = api_dir.join("build").join("api-service.to");
        ensure_can_write(&container_path, self.overwrite)?;
        ensure_can_write(&service_path, self.overwrite)?;
        write_json(&container_path, &container)?;
        write_json(&service_path, &service)?;
        let template_artifacts =
            scaffold_single_api_app_template(&self.output_dir, &self.app_name, self.overwrite)?;
        let manifest = SoracloudAppManifestV1 {
            schema_version: SORACLOUD_APP_MANIFEST_VERSION_V1,
            app_name: self.app_name,
            app_version: Some(self.app_version.clone()),
            public_url: public_url.clone(),
            static_site: Some(SoracloudAppStaticSiteV1 {
                dist_dir: static_site_dist_dir,
                mount_path: "/".to_owned(),
                publish_mode: APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING.to_owned(),
                api_base_path: Some("/api".to_owned()),
                publish_label: Some(format!("{app_name}-site")),
            }),
            services: vec![SoracloudAppServiceRefV1 {
                service_name: api_service_name.to_string(),
                container_manifest: relative_path_string(&manifest_path, &container_path),
                service_manifest: relative_path_string(&manifest_path, &service_path),
                bundle_file: Some(relative_path_string(&manifest_path, &bundle_path)),
                initial_configs: None,
                initial_secrets: None,
            }],
        };
        manifest.validate()?;
        write_json(&manifest_path, &manifest)?;
        Ok(AppInitOutput {
            template: self.template.as_str().to_owned(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            public_url,
            service_manifest_paths: vec![
                container_path.to_string_lossy().into_owned(),
                service_path.to_string_lossy().into_owned(),
            ],
            template_artifacts,
        })
    }
    fn run_split_app(self) -> Result<AppInitOutput> {
        fs::create_dir_all(&self.output_dir).wrap_err_with(|| {
            format!(
                "failed to create output directory {}",
                self.output_dir.display()
            )
        })?;
        let app_name = normalized_service_label(&self.app_name);
        let host = self.resolve_public_host(&app_name)?;
        let public_url = format!("https://{host}");
        let static_site_dist_dir = self.resolve_static_site_dist_dir("frontend/dist")?;
        let existing_repo = self.existing_repo;
        let manifest_path = self.output_dir.join("app_manifest.json");
        ensure_can_write(&manifest_path, self.overwrite)?;
        let live_bundle = build_split_app_live_service_bundle(&app_name, &host, &self.app_version)?;
        let vault_bundle =
            build_split_app_vault_service_bundle(&app_name, &host, &self.app_version)?;
        let live_dir = self.output_dir.join("services").join("live");
        let vault_dir = self.output_dir.join("services").join("vault");
        let live_container_path = live_dir.join("container_manifest.json");
        let live_service_path = live_dir.join("service_manifest.json");
        let vault_container_path = vault_dir.join("container_manifest.json");
        let vault_service_path = vault_dir.join("service_manifest.json");
        for path in [
            &live_container_path,
            &live_service_path,
            &vault_container_path,
            &vault_service_path,
        ] {
            ensure_can_write(path, self.overwrite)?;
        }
        write_json(&live_container_path, &live_bundle.container)?;
        write_json(&live_service_path, &live_bundle.service)?;
        write_json(&vault_container_path, &vault_bundle.container)?;
        write_json(&vault_service_path, &vault_bundle.service)?;
        let manifest = SoracloudAppManifestV1 {
            schema_version: SORACLOUD_APP_MANIFEST_VERSION_V1,
            app_name: self.app_name.clone(),
            app_version: Some(self.app_version),
            public_url: public_url.clone(),
            static_site: Some(SoracloudAppStaticSiteV1 {
                dist_dir: static_site_dist_dir,
                mount_path: "/".to_owned(),
                publish_mode: APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY.to_owned(),
                api_base_path: Some("/api".to_owned()),
                publish_label: Some(format!("{app_name}-frontend")),
            }),
            services: vec![
                SoracloudAppServiceRefV1 {
                    service_name: live_bundle.service.service_name.to_string(),
                    container_manifest: relative_path_string(&manifest_path, &live_container_path),
                    service_manifest: relative_path_string(&manifest_path, &live_service_path),
                    bundle_file: Some(relative_path_string(
                        &manifest_path,
                        &live_dir.join("build/live-api.tgz"),
                    )),
                    initial_configs: None,
                    initial_secrets: None,
                },
                SoracloudAppServiceRefV1 {
                    service_name: vault_bundle.service.service_name.to_string(),
                    container_manifest: relative_path_string(&manifest_path, &vault_container_path),
                    service_manifest: relative_path_string(&manifest_path, &vault_service_path),
                    bundle_file: Some(relative_path_string(
                        &manifest_path,
                        &vault_dir.join("build/vault-api.to"),
                    )),
                    initial_configs: None,
                    initial_secrets: None,
                },
            ],
        };
        manifest.validate()?;
        write_json(&manifest_path, &manifest)?;
        let template_artifacts = scaffold_split_app_template(
            &self.output_dir,
            &self.app_name,
            self.overwrite,
            existing_repo,
        )?;
        Ok(AppInitOutput {
            template: self.template.as_str().to_owned(),
            manifest_path: manifest_path.to_string_lossy().into_owned(),
            public_url,
            service_manifest_paths: vec![
                live_container_path.to_string_lossy().into_owned(),
                live_service_path.to_string_lossy().into_owned(),
                vault_container_path.to_string_lossy().into_owned(),
                vault_service_path.to_string_lossy().into_owned(),
            ],
            template_artifacts,
        })
    }
    fn resolve_public_host(&self, default_host: &str) -> Result<String> {
        let Some(host) = self.public_host.as_deref() else {
            return Ok(format!("{default_host}.sora"));
        };
        let host = require_exact_nonempty_arg("--public-host", host)?;
        if host.contains("://") || host.contains('/') {
            return Err(eyre!("--public-host must be a hostname only, got `{host}`"));
        }
        Ok(host)
    }
    fn resolve_static_site_dist_dir(&self, default_dist_dir: &str) -> Result<String> {
        let Some(dist_dir) = self.static_site_dist_dir.as_deref() else {
            return Ok(default_dist_dir.to_owned());
        };
        require_exact_nonempty_arg("--static-site-dist-dir", dist_dir)
    }
}
define_torii_args! {
    " Torii base URL for the canonical app-infra release mutation.",
    " Optional API token sent as `x-api-token` when mutating live control-plane APIs.",
    " Positive timeout for each Torii request; the durable Inrou qualification is reread before online side effects.";
    /// Internal canonical app-infra mutation arguments used by `app release`.
    pub struct AppReleaseMutationArgs {
        /// Path to a `SoracloudAppManifestV1` JSON document.
        #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
        manifest: PathBuf,
        /// Exact Unix-second retention boundary embedded in every SoraFS manifest in this release.
        /// Reuse the same value for every retry; it must remain ahead of consensus time.
        #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
        sorafs_retention_epoch: NonZeroU64,
        /// Absolute owner-only ingest qualification produced by `soracloud app preseed`.
        #[arg(long = "inrou-preseed-receipt", value_name = "PATH")]
        inrou_preseed_receipt: Option<PathBuf>,
    }
}
impl AppReleaseMutationArgs {
    fn run(
        self,
        mode: MutationMode,
        authority: &AccountId,
        key_pair: &KeyPair,
    ) -> Result<AppMutationOutput> {
        let release_identity = SorafsReleaseIdentityV1::new(self.sorafs_retention_epoch);
        let manifest_path = self.manifest.clone();
        let manifest_dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let manifest: SoracloudAppManifestV1 = load_json(&manifest_path)?;
        manifest.validate()?;
        let synced_manifests = match sync_app_manifest_service_refs(&manifest, &manifest_dir) {
            Ok(synced) => synced,
            Err(error) if is_app_service_name_mismatch_error(&error) => return Err(error),
            Err(error) => {
                return Err(
                    error.wrap_err("failed to sync app service manifests before deployment")
                );
            }
        };
        let root = build_app_root_projection(&manifest_path, &manifest.public_url)?;
        let frontend = build_app_frontend_projection(
            &manifest.public_url,
            manifest.static_site.as_ref(),
            &manifest_path,
        )?;
        let routes = build_app_local_plan_output(&manifest_path)?.routes;
        let torii_url = require_torii_url(self.torii_url.as_deref())?.to_owned();
        let app_version = manifest
            .app_version
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                eyre!(
                    "live app deploy/upgrade requires an explicit `app_version` so the authoritative app preflight can reject replays before artifact publication"
                )
            })?;
        let mut planned_service_mutations = manifest
            .services
            .iter()
            .map(|service| {
                let container_manifest =
                    resolve_manifest_path(&manifest_dir, &service.container_manifest);
                let service_manifest =
                    resolve_manifest_path(&manifest_dir, &service.service_manifest);
                let container: UnpublishedContainerManifestV1 = load_json(&container_manifest)?;
                let service_manifest_payload: SoraServiceManifestV1 = load_json(&service_manifest)?;
                ensure_app_service_ref_matches_manifest_name(
                    &service.service_name,
                    &service_manifest,
                    &service_manifest_payload,
                )?;
                let bundle = UnpublishedDeploymentBundleV1 {
                    container,
                    service: service_manifest_payload,
                };
                Ok((service, container_manifest, service_manifest, bundle))
            })
            .collect::<Result<Vec<_>>>()?;
        validate_inrou_preseed_artifact_count(
            planned_service_mutations
                .iter()
                .map(|(_, _, _, bundle)| bundle),
        )?;
        let required_preseed_store_count = required_inrou_preseed_store_count(
            planned_service_mutations
                .iter()
                .map(|(_, _, _, bundle)| bundle),
        );
        let preseed_qualification = load_inrou_preseed_qualification(
            required_preseed_store_count,
            self.inrou_preseed_receipt.as_deref(),
        )?;
        if let Some(qualification) = preseed_qualification.as_ref() {
            let placement_targets = qualification.placement_targets();
            for (_, _, _, bundle) in &mut planned_service_mutations {
                if bundle.service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
                    && bundle.container.runtime == SoraContainerRuntimeV1::Inrou
                {
                    bind_exact_inrou_placement_targets(&mut bundle.service, &placement_targets)?;
                }
            }
        }
        for (_, container_manifest, service_manifest, bundle) in &planned_service_mutations {
            validate_unpublished_deployment_source(bundle)?;
            let service_workspace_dir =
                app_service_workspace_dir(container_manifest, service_manifest);
            validate_local_inrou_guest_image_sources(service_workspace_dir.as_deref(), bundle)?;
        }
        let (_, app_preflight_status) = fetch_torii_soracloud_app_infra_status(
            &torii_url,
            None,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let app_precondition = derive_app_infra_mutation_precondition(
            &app_preflight_status,
            &manifest.app_name,
            app_version,
            mode,
            "Soracloud app infra",
        )?;
        let (_, service_preflight_status) = fetch_torii_soracloud_status(
            &torii_url,
            None,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mode_label = match mode {
            MutationMode::Deploy => "deploy",
            MutationMode::Upgrade => "upgrade",
        }
        .to_owned();
        let prepared_static_site_publication = manifest
            .static_site
            .as_ref()
            .map(|static_site| {
                prepare_app_static_site(
                    &manifest,
                    &manifest_dir,
                    static_site,
                    key_pair,
                    release_identity,
                )
            })
            .transpose()?;
        let static_site_root_binding = plan_app_static_site_root_binding(
            &manifest.app_name,
            &manifest.public_url,
            manifest.static_site.as_ref(),
            prepared_static_site_publication
                .as_ref()
                .map(|(publication, _)| publication),
        )?;
        let mut static_site_binding_attached = false;
        let mut prepared_service_publications = Vec::with_capacity(manifest.services.len());
        let mut hosted_http_service_count = 0_u32;
        let mut deterministic_service_count = 0_u32;
        for (service, container_manifest, service_manifest, bundle) in planned_service_mutations {
            let precondition = derive_service_mutation_precondition(
                &service_preflight_status,
                &service.service_name,
                &bundle.service.service_version,
                mode,
                "Soracloud app service",
            )?;
            preflight_service_upgrade_identity(
                &service_preflight_status,
                &bundle.service,
                bundle.container.runtime,
                mode,
                "Soracloud app service",
            )?;
            let is_hosted_http = bundle.service.execution_plane
                == SoraServiceExecutionPlaneV1::HttpService
                && bundle.container.runtime == SoraContainerRuntimeV1::Inrou;
            let is_deterministic = bundle.service.execution_plane
                == SoraServiceExecutionPlaneV1::DeterministicService
                && bundle.container.runtime == SoraContainerRuntimeV1::Ivm;
            if is_hosted_http {
                hosted_http_service_count += 1;
            }
            if is_deterministic {
                deterministic_service_count += 1;
            }
            let service_workspace_dir =
                app_service_workspace_dir(&container_manifest, &service_manifest);
            let mut initial_service_configs = load_initial_service_configs(
                service
                    .initial_configs
                    .as_deref()
                    .map(|path| resolve_manifest_path(&manifest_dir, path))
                    .as_deref(),
            )?;
            static_site_binding_attached |= apply_app_static_site_root_binding(
                &service.service_name,
                &bundle.service,
                &mut initial_service_configs,
                static_site_root_binding.as_ref(),
                static_site_binding_attached,
            )?;
            let bundle_file = service
                .bundle_file
                .as_deref()
                .map(|path| resolve_manifest_path(&manifest_dir, path))
                .ok_or_else(|| {
                    eyre!(
                        "app service `{}` must declare `bundle_file`; V1 does not accept prepublished or hash-only service bundles",
                        service.service_name
                    )
                })?;
            let service_artifacts = prepare_service_artifacts(
                &bundle_file,
                service_workspace_dir.as_deref(),
                bundle,
                key_pair,
                release_identity,
            )?;
            let public_discovery = prepare_public_service_discovery_config(
                &service_artifacts.admitted_bundle,
                &initial_service_configs,
                &torii_url,
                self.api_token.as_deref(),
                self.timeout_secs,
                key_pair,
                release_identity,
            )?;
            let initial_service_secrets = load_initial_service_secrets(
                service
                    .initial_secrets
                    .as_deref()
                    .map(|path| resolve_manifest_path(&manifest_dir, path))
                    .as_deref(),
            )?;
            prepared_service_publications.push(PreparedAppServiceMutation {
                service,
                container_manifest,
                service_manifest,
                precondition,
                initial_service_configs,
                initial_service_secrets,
                is_hosted_http,
                is_deterministic,
                service_artifacts,
                public_discovery,
            });
        }
        ensure_app_static_site_root_binding_attached(
            static_site_root_binding.as_ref(),
            static_site_binding_attached,
        )?;
        let mut inrou_artifacts = Vec::new();
        for prepared in &prepared_service_publications {
            if prepared.is_hosted_http {
                inrou_artifacts.extend(prepared.service_artifacts.artifacts.iter());
                if let Some(discovery) = prepared.public_discovery.as_ref() {
                    inrou_artifacts.push(&discovery.artifact);
                }
            }
        }
        let inrou_artifacts = distinct_prepared_sorafs_artifacts(inrou_artifacts);
        if let Some(qualification) = preseed_qualification.as_ref() {
            qualification.require_exact_artifacts(&inrou_artifacts)?;
            qualification.revalidate()?;
            let (_, current_host_status) = fetch_torii_soracloud_status(
                &torii_url,
                None,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            let (_, current_host_status) =
                decode_network_control_plane_snapshot(&current_host_status)?;
            require_active_inrou_qualification_targets(
                qualification,
                &current_host_status.active_inrou_hosts,
            )?;
        }
        if let Some((_, artifact)) = prepared_static_site_publication.as_ref() {
            register_prepared_sorafs_artifact(
                artifact,
                &torii_url,
                authority,
                key_pair,
                self.timeout_secs,
            )?;
        }
        for prepared in &prepared_service_publications {
            register_prepared_sorafs_artifacts(
                prepared.service_artifacts.artifacts.iter(),
                &torii_url,
                authority,
                key_pair,
                self.timeout_secs,
            )?;
            if let Some(discovery) = prepared.public_discovery.as_ref() {
                register_prepared_sorafs_artifact(
                    &discovery.artifact,
                    &torii_url,
                    authority,
                    key_pair,
                    self.timeout_secs,
                )?;
            }
        }
        let mut services = Vec::with_capacity(manifest.services.len());
        let mut signed_service_requests = Vec::with_capacity(manifest.services.len());
        let mut app_infra_bundles = Vec::with_capacity(manifest.services.len());
        for prepared in prepared_service_publications {
            let PreparedAppServiceMutation {
                service,
                container_manifest,
                service_manifest,
                precondition,
                mut initial_service_configs,
                initial_service_secrets,
                is_hosted_http,
                is_deterministic,
                service_artifacts,
                public_discovery,
            } = prepared;
            let service_workspace_dir =
                app_service_workspace_dir(&container_manifest, &service_manifest);
            let workspace_dir = service_workspace_dir
                .clone()
                .unwrap_or_else(|| manifest_dir.clone())
                .to_string_lossy()
                .into_owned();
            let workspace_scripts = app_service_workspace_scripts(service_workspace_dir.as_deref());
            let PreparedServiceArtifacts {
                admitted_bundle: bundle,
                published_bundle,
                inrou_guest_images: published_inrou_guest_images,
                artifacts: _,
            } = service_artifacts;
            let execution_plane = format!("{:?}", bundle.service.execution_plane);
            let runtime = format!("{:?}", bundle.container.runtime);
            let route_host = bundle
                .service
                .route
                .as_ref()
                .map(|route| route.host.clone());
            let route_path_prefix = bundle
                .service
                .route
                .as_ref()
                .map(|route| route.path_prefix.clone());
            let route_visibility = bundle
                .service
                .route
                .as_ref()
                .map(|route| format!("{:?}", route.visibility));
            let published_public_discovery = attach_prepared_public_service_discovery_config(
                &mut initial_service_configs,
                public_discovery,
            )?;
            let request = signed_bundle_request(
                bundle,
                initial_service_configs,
                initial_service_secrets,
                precondition,
                Some(authority),
                key_pair,
            )?;
            app_infra_bundles.push(request.bundle.clone());
            signed_service_requests.push(request);
            let mut notes = Vec::new();
            if is_hosted_http {
                notes.push(
                    "app service deploys onto the hosted HttpService + Inrou production plane"
                        .to_owned(),
                );
            }
            if is_deterministic {
                notes.push(
                    "app service deploys onto the deterministic IVM production plane".to_owned(),
                );
            }
            if !published_inrou_guest_images.is_empty() {
                notes.push(
                    "Inrou bundle, guest-image, and discovery bytes were operator-preseeded into every selected offline SoraFS replica before pin registration"
                        .to_owned(),
                );
            }
            services.push(AppServiceMutationOutput {
                service_name: service.service_name.clone(),
                container_manifest: container_manifest.to_string_lossy().into_owned(),
                service_manifest: service_manifest.to_string_lossy().into_owned(),
                workspace_dir,
                workspace_scripts,
                execution_plane,
                runtime,
                route_host,
                route_path_prefix,
                route_visibility,
                published_public_discovery,
                published_bundle,
                published_inrou_guest_images,
                response: norito::json!({
                    "action": "PendingAppInfraMutation"
                }),
                notes,
            });
        }
        let has_mixed_planes = hosted_http_service_count > 0 && deterministic_service_count > 0;
        let mut notes = Vec::new();
        let app_infra_manifest = build_app_infra_manifest(
            &manifest,
            prepared_static_site_publication
                .as_ref()
                .map(|(publication, _)| publication),
            &app_infra_bundles,
        )?;
        let app_infra_manifest_hash = app_infra_manifest.manifest_hash();
        let app_infra_request = signed_app_infra_request(
            mode,
            app_infra_manifest,
            signed_service_requests.clone(),
            app_precondition,
            key_pair,
        )?;
        if let Some(qualification) = preseed_qualification.as_ref() {
            qualification.revalidate()?;
            let (_, current_status) = fetch_torii_soracloud_status(
                &torii_url,
                None,
                self.api_token.as_deref(),
                self.timeout_secs,
            )?;
            let (_, current_status) = decode_network_control_plane_snapshot(&current_status)?;
            require_active_inrou_qualification_targets(
                qualification,
                &current_status.active_inrou_hosts,
            )?;
        }
        let app_infra_response = run_app_infra_mutation(
            mode,
            &app_infra_request,
            &torii_url,
            self.api_token.as_deref(),
            self.timeout_secs,
        )
        .wrap_err("failed to submit canonical app-level Soracloud infra mutation")?;
        for service in &mut services {
            service.response = app_infra_response.clone();
        }
        notes.push(
            "app mutation submitted the canonical app-level Soracloud infra request".to_owned(),
        );
        let app_infra_response = Some(app_infra_response);
        if prepared_static_site_publication.is_some() {
            notes.push(
                "app mutation published the configured static site before mutating services"
                    .to_owned(),
            );
        }
        if has_mixed_planes {
            notes.push(
                "app mutation spans both hosted HttpService + Inrou and deterministic IVM services"
                    .to_owned(),
            );
        }
        Ok(AppMutationOutput {
            app_name: manifest.app_name,
            manifest_path: root.manifest_path,
            public_url: root.public_url,
            hostname: root.hostname,
            workspace_dir: root.workspace_dir,
            workspace_scripts: root.workspace_scripts,
            mode: mode_label,
            has_mixed_planes,
            hosted_http_service_count,
            deterministic_service_count,
            static_site: manifest.static_site,
            published_static_site: prepared_static_site_publication
                .map(|(publication, _)| publication),
            frontend,
            synced_manifests,
            app_infra_manifest_hash: Some(app_infra_manifest_hash),
            app_infra_response,
            services,
            routes,
            notes,
        })
    }
}
define_torii_args! {
    " Torii base URL for authoritative Soracloud status.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane status query.";
    /// Arguments for `soracloud app status`.
    pub struct AppStatusArgs {
        /// Path to a `SoracloudAppManifestV1` JSON document.
        #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
        manifest: PathBuf,
    }
}
impl AppStatusArgs {
    fn run(self) -> Result<AppStatusOutput> {
        let manifest_path = self.manifest.clone();
        let manifest_dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let manifest: SoracloudAppManifestV1 = load_json(&manifest_path)?;
        manifest.validate()?;
        let root = build_app_root_projection(&manifest_path, &manifest.public_url)?;
        let frontend = build_app_frontend_projection(
            &manifest.public_url,
            manifest.static_site.as_ref(),
            &manifest_path,
        )?;
        let routes = build_app_local_plan_output(&manifest_path)?.routes;
        for service_ref in &manifest.services {
            let service_path = resolve_manifest_path(&manifest_dir, &service_ref.service_manifest);
            let service_manifest: SoraServiceManifestV1 = load_json(&service_path)?;
            ensure_app_service_ref_matches_manifest_name(
                &service_ref.service_name,
                &service_path,
                &service_manifest,
            )?;
        }
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let app_status_attempt = fetch_torii_soracloud_app_infra_status(
            torii_url,
            Some(&manifest.app_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        );
        let (endpoint, app_payload) = app_status_attempt?;
        let (_, payload) = fetch_torii_soracloud_status(
            torii_url,
            None,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let app_infra_status = Some(app_payload);
        let source = "torii_app_infra".to_owned();
        let service_status_note = None;
        let (_, control_plane) = decode_network_control_plane_snapshot(&payload)?;
        let mut control_plane_services = BTreeMap::new();
        for service in control_plane.services {
            let service_name = service.service_name.clone();
            let encoded = json::to_value(&service)
                .wrap_err("failed to encode exact Soracloud service status")?;
            if control_plane_services
                .insert(service_name.clone(), encoded)
                .is_some()
            {
                return Err(eyre!(
                    "authoritative Soracloud control-plane status contains duplicate service `{service_name}`"
                ));
            }
        }
        let mut services = Vec::with_capacity(manifest.services.len());
        let mut hosted_http_service_count = 0_u32;
        let mut deterministic_service_count = 0_u32;
        for service_ref in &manifest.services {
            let container_path =
                resolve_manifest_path(&manifest_dir, &service_ref.container_manifest);
            let service_path = resolve_manifest_path(&manifest_dir, &service_ref.service_manifest);
            let container: UnpublishedContainerManifestV1 = load_json(&container_path)?;
            let service: SoraServiceManifestV1 = load_json(&service_path)?;
            ensure_app_service_ref_matches_manifest_name(
                &service_ref.service_name,
                &service_path,
                &service,
            )?;
            let service_name = service.service_name.to_string();
            let is_hosted_http = service.execution_plane
                == SoraServiceExecutionPlaneV1::HttpService
                && container.runtime == SoraContainerRuntimeV1::Inrou;
            let is_deterministic = service.execution_plane
                == SoraServiceExecutionPlaneV1::DeterministicService
                && container.runtime == SoraContainerRuntimeV1::Ivm;
            if is_hosted_http {
                hosted_http_service_count += 1;
            }
            if is_deterministic {
                deterministic_service_count += 1;
            }
            let service_workspace_dir = app_service_workspace_dir(&container_path, &service_path);
            let mut notes = Vec::new();
            if is_hosted_http {
                notes.push(
                    "app service targets the hosted HttpService + Inrou production plane"
                        .to_owned(),
                );
            }
            if is_deterministic {
                notes.push("app service targets the deterministic IVM production plane".to_owned());
            }
            let live_status = control_plane_services.get(service_name.as_str()).cloned();
            if live_status.is_none() {
                notes.push(
                    "service is declared in the app manifest but no matching Torii control-plane entry was returned"
                        .to_owned(),
                );
            }
            let workspace_scripts = app_service_workspace_scripts(service_workspace_dir.as_deref());
            services.push(AppServiceStatusOutput {
                service_name,
                container_manifest_path: container_path.to_string_lossy().into_owned(),
                service_manifest_path: service_path.to_string_lossy().into_owned(),
                workspace_dir: service_workspace_dir
                    .unwrap_or_else(|| manifest_dir.clone())
                    .to_string_lossy()
                    .into_owned(),
                workspace_scripts,
                execution_plane: format!("{:?}", service.execution_plane),
                runtime: format!("{:?}", container.runtime),
                route_host: service.route.as_ref().map(|route| route.host.clone()),
                route_path_prefix: service
                    .route
                    .as_ref()
                    .map(|route| route.path_prefix.clone()),
                route_visibility: service
                    .route
                    .as_ref()
                    .map(|route| format!("{:?}", route.visibility)),
                present_in_control_plane: live_status.is_some(),
                status: live_status,
                notes,
            });
        }
        let has_mixed_planes = hosted_http_service_count > 0 && deterministic_service_count > 0;
        let mut notes = Vec::new();
        if has_mixed_planes {
            notes.push(
                "app status spans both hosted HttpService + Inrou and deterministic IVM services"
                    .to_owned(),
            );
        }
        if let Some(note) = service_status_note {
            notes.push(note);
        }
        let blockers = services
            .iter()
            .filter(|service| !service.present_in_control_plane)
            .map(|service| {
                format!(
                    "service `{}` is absent from the Torii control-plane status payload",
                    service.service_name
                )
            })
            .collect::<Vec<_>>();
        let ok = blockers.is_empty();
        let report = build_soracloud_app_report(
            manifest.app_name.clone(),
            root.manifest_path.clone(),
            ok,
            vec![
                skipped_app_phase("build", "app status is read-only"),
                skipped_app_phase("sync_manifests", "app status is read-only"),
                skipped_app_phase("doctor", "app status is read-only"),
                skipped_app_phase("publish", "app status is read-only"),
                skipped_app_phase("sign", "app status is read-only"),
                skipped_app_phase("submit", "app status is read-only"),
                app_phase_report("status", ok, notes.clone()),
                skipped_app_phase("verify", "status reports control-plane presence only"),
            ],
            None,
            routes.clone(),
            app_report_services_from_status(&services),
            manifest.static_site.clone(),
            blockers.clone(),
            if ok {
                "No app status blockers found.".to_owned()
            } else {
                "Resolve app status blockers before promoting this release.".to_owned()
            },
        );
        Ok(AppStatusOutput {
            report,
            app_name: manifest.app_name,
            manifest_path: root.manifest_path,
            public_url: root.public_url,
            hostname: root.hostname,
            workspace_dir: root.workspace_dir,
            workspace_scripts: root.workspace_scripts,
            source,
            torii_endpoint: Some(endpoint),
            static_site: manifest.static_site,
            frontend,
            has_mixed_planes,
            hosted_http_service_count,
            deterministic_service_count,
            app_infra_status,
            services,
            routes,
            notes,
        })
    }
}
/// Arguments for `soracloud app plan`.
#[derive(clap::Args, Debug)]
pub struct AppLocalPlanArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
}
impl AppLocalPlanArgs {
    fn run(self) -> Result<AppLocalPlanOutput> {
        build_app_local_plan_output(self.manifest.as_path())
    }
}
/// Arguments for `soracloud app doctor`.
#[derive(clap::Args, Debug)]
pub struct AppDoctorArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
}
impl AppDoctorArgs {
    fn run(self) -> Result<AppDoctorOutput> {
        let manifest_path = self.manifest.clone();
        let manifest_dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let manifest: SoracloudAppManifestV1 = load_json(&manifest_path)?;
        manifest.validate()?;
        let plan = build_app_local_plan_output(manifest_path.as_path())?;
        let mut checks = Vec::new();
        let mut failing_checks = Vec::new();
        let mut push_check = |name: &str, passed: bool, detail: String| {
            if !passed {
                failing_checks.push(name.to_owned());
            }
            checks.push(AppDoctorCheckOutput {
                name: name.to_owned(),
                status: if passed { "pass" } else { "fail" }.to_owned(),
                detail,
            });
        };
        push_check(
            "root_scripts",
            plan.workspace_scripts.local_dev.is_some()
                && plan.workspace_scripts.build_and_sync.is_some()
                && plan.workspace_scripts.doctor.is_some()
                && plan.workspace_scripts.release.is_some(),
            format!(
                "root scripts local_dev={} build_and_sync={} doctor={} release={}",
                plan.workspace_scripts.local_dev.is_some(),
                plan.workspace_scripts.build_and_sync.is_some(),
                plan.workspace_scripts.doctor.is_some(),
                plan.workspace_scripts.release.is_some()
            ),
        );
        let static_site = manifest.static_site.as_ref();
        push_check(
            "frontend_publish_mode",
            static_site.is_some_and(|site| {
                (site.publish_mode == APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING
                    || site.publish_mode == APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
                    && site.mount_path == "/"
                    && site.api_base_path.as_deref() == Some("/api")
            }),
            match static_site {
                Some(site) => format!(
                    "publish_mode={} mount_path={} api_base_path={}",
                    site.publish_mode,
                    site.mount_path,
                    site.api_base_path.as_deref().unwrap_or("<none>")
                ),
                None => "app manifest has no static_site section".to_owned(),
            },
        );
        let hosted_services = plan
            .services
            .iter()
            .filter(|service| {
                service.execution_plane == "HttpService" && service.runtime == "Inrou"
            })
            .collect::<Vec<_>>();
        let deterministic_services = plan
            .services
            .iter()
            .filter(|service| {
                service.execution_plane == "DeterministicService" && service.runtime == "Ivm"
            })
            .collect::<Vec<_>>();
        push_check(
            "service_planes",
            !plan.services.is_empty()
                && hosted_services.len() + deterministic_services.len() == plan.services.len(),
            format!(
                "hosted_http_services={} deterministic_services={}",
                hosted_services.len(),
                deterministic_services.len()
            ),
        );
        let live_prefix_ok = hosted_services
            .iter()
            .all(|service| service.route_path_prefix.as_deref() == Some("/api/v1"));
        push_check(
            "live_route_prefix",
            hosted_services.is_empty() || live_prefix_ok,
            if hosted_services.is_empty() {
                "no hosted HttpService + Inrou services declared; check is not applicable"
                    .to_owned()
            } else {
                hosted_services
                    .iter()
                    .map(|service| {
                        format!(
                            "{}:{}",
                            service.service_name,
                            service.route_path_prefix.as_deref().unwrap_or("<none>")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        );
        let mut service_manifest_summaries = Vec::new();
        for service_ref in &manifest.services {
            let container_path =
                resolve_manifest_path(&manifest_dir, &service_ref.container_manifest);
            let service_path = resolve_manifest_path(&manifest_dir, &service_ref.service_manifest);
            let container: UnpublishedContainerManifestV1 = load_json(&container_path)?;
            let service: SoraServiceManifestV1 = load_json(&service_path)?;
            let bundle_path = service_ref
                .bundle_file
                .as_deref()
                .map(|path| resolve_manifest_path(&manifest_dir, path));
            let bundle_exists = bundle_path.as_ref().is_some_and(|path| path.is_file());
            push_check(
                &format!("bundle_file:{}", service_ref.service_name),
                bundle_exists,
                service_ref
                    .bundle_file
                    .as_deref()
                    .map(|path| {
                        resolve_manifest_path(&manifest_dir, path)
                            .display()
                            .to_string()
                    })
                    .unwrap_or_else(|| "<none>".to_owned()),
            );
            let service_workspace_dir = app_service_workspace_dir(&container_path, &service_path);
            let bundle_is_in_service_workspace = service_workspace_dir
                .as_ref()
                .zip(bundle_path.as_ref())
                .is_some_and(|(workspace_dir, bundle_path)| bundle_path.starts_with(workspace_dir));
            let child_scripts_ok = if container.runtime == SoraContainerRuntimeV1::Inrou {
                let scripts = app_service_workspace_scripts(service_workspace_dir.as_deref());
                (scripts.dev.is_some() && scripts.build.is_some())
                    || (bundle_exists && !bundle_is_in_service_workspace)
            } else {
                let scripts = app_service_workspace_scripts(service_workspace_dir.as_deref());
                scripts.dev.is_some() && scripts.build.is_some() && scripts.verify_build.is_some()
            };
            push_check(
                &format!("workspace_scripts:{}", service_ref.service_name),
                child_scripts_ok,
                format!(
                    "service={} runtime={:?}",
                    service_ref.service_name, container.runtime
                ),
            );
            service_manifest_summaries.push((service_ref.service_name.clone(), container, service));
        }
        let live_storage_ok =
            service_manifest_summaries
                .iter()
                .all(|(_service_name, container, service)| {
                    if service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
                        && container.runtime == SoraContainerRuntimeV1::Inrou
                    {
                        !service.lease_volumes.is_empty() && service.state_bindings.is_empty()
                    } else {
                        true
                    }
                });
        push_check(
            "live_storage_plane",
            live_storage_ok,
            service_manifest_summaries
                .iter()
                .filter(|(_, container, service)| {
                    service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
                        && container.runtime == SoraContainerRuntimeV1::Inrou
                })
                .map(|(service_name, _, service)| {
                    format!(
                        "{}: lease_volumes={} state_bindings={}",
                        service_name,
                        service.lease_volumes.len(),
                        service.state_bindings.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        let deterministic_surface_ok =
            service_manifest_summaries
                .iter()
                .all(|(_service_name, container, service)| {
                    if service.execution_plane == SoraServiceExecutionPlaneV1::DeterministicService
                        && container.runtime == SoraContainerRuntimeV1::Ivm
                    {
                        let routes_are_declared = service
                            .handlers
                            .iter()
                            .all(|handler| handler.route_path.is_some());
                        let vault_surface_is_valid = service.state_bindings.is_empty()
                            || (service.handlers.iter().all(|handler| {
                                handler.route_path.as_deref().is_some_and(|path| {
                                    path.starts_with("/auth/")
                                        || path == "/auth/me"
                                        || path.starts_with("/v1/user/")
                                })
                            }) && service.state_bindings.iter().all(|binding| {
                                matches!(
                                    binding.binding_name.as_ref(),
                                    "auth_challenges"
                                        | "auth_sessions"
                                        | "user_preferences"
                                        | "user_saved_searches"
                                )
                            }));
                        service.lease_volumes.is_empty()
                            && routes_are_declared
                            && vault_surface_is_valid
                    } else {
                        true
                    }
                });
        push_check(
            "deterministic_service_surface",
            deterministic_surface_ok,
            service_manifest_summaries
                .iter()
                .filter(|(_, container, service)| {
                    service.execution_plane == SoraServiceExecutionPlaneV1::DeterministicService
                        && container.runtime == SoraContainerRuntimeV1::Ivm
                })
                .map(|(service_name, _, service)| {
                    format!(
                        "{}: handlers={} bindings={} lease_volumes={}",
                        service_name,
                        service.handlers.len(),
                        service.state_bindings.len(),
                        service.lease_volumes.len()
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
        let mut duplicate_routes = BTreeMap::<(String, String), BTreeSet<String>>::new();
        for route in &plan.routes {
            duplicate_routes
                .entry((route.host.clone(), route.path.clone()))
                .or_default()
                .insert(route.service_name.clone());
        }
        let conflicting_routes = duplicate_routes
            .into_iter()
            .filter(|(_, services)| services.len() > 1)
            .collect::<Vec<_>>();
        push_check(
            "route_collisions",
            conflicting_routes.is_empty(),
            if conflicting_routes.is_empty() {
                "no exact host/path collisions across app routes".to_owned()
            } else {
                conflicting_routes
                    .iter()
                    .map(|((host, path), services)| {
                        format!(
                            "{host}{path} => {}",
                            services
                                .iter()
                                .map(String::as_str)
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            },
        );
        let ok = failing_checks.is_empty();
        let mut notes = plan.notes.clone();
        if ok {
            notes.push(
                "doctor validated local source artifacts and the split-plane release contract; the canonical app-infra manifest is validated only after release publishes immutable artifacts"
                    .to_owned(),
            );
        } else {
            notes.push(format!(
                "doctor found failing checks: {}",
                failing_checks.join(", ")
            ));
        }
        let blockers = failing_checks
            .iter()
            .map(|check| format!("doctor check `{check}` failed"))
            .collect::<Vec<_>>();
        let report = build_soracloud_app_report(
            plan.app_name.clone(),
            plan.manifest_path.clone(),
            ok,
            vec![
                skipped_app_phase("build", "app doctor validates existing local artifacts"),
                skipped_app_phase("sync_manifests", "app doctor does not mutate manifests"),
                app_phase_report("doctor", ok, notes.clone()),
                skipped_app_phase("publish", "app doctor does not publish artifacts"),
                skipped_app_phase("sign", "app doctor does not sign requests"),
                skipped_app_phase("submit", "app doctor does not submit transactions"),
                skipped_app_phase("status", "app doctor does not query live status"),
                skipped_app_phase("verify", "app doctor verifies local release readiness only"),
            ],
            None,
            plan.routes.clone(),
            app_report_services_from_plan(&plan.services),
            manifest.static_site.clone(),
            blockers,
            if ok {
                "Run `iroha soracloud app release` to publish and submit the app.".to_owned()
            } else {
                "Fix failing doctor checks, then rerun `iroha soracloud app doctor`.".to_owned()
            },
        );
        Ok(AppDoctorOutput {
            report,
            app_name: plan.app_name,
            manifest_path: plan.manifest_path,
            public_url: plan.public_url,
            hostname: plan.hostname,
            workspace_dir: plan.workspace_dir,
            workspace_scripts: plan.workspace_scripts,
            ok,
            has_mixed_planes: plan.has_mixed_planes,
            hosted_http_service_count: plan.hosted_http_service_count,
            deterministic_service_count: plan.deterministic_service_count,
            frontend: plan.frontend,
            services: plan.services,
            routes: plan.routes,
            checks,
            notes,
        })
    }
}
/// Arguments for `soracloud app release`.
#[derive(Clone, Debug, JsonSerialize)]
struct InrouPreseedQualificationOutput {
    status: String,
    receipt_path: String,
    receipt: OperatorPreseedSessionReceiptV1,
}

/// Arguments for the separate offline `soracloud service preseed` phase.
#[derive(clap::Args, Debug)]
pub struct InrouServicePreseedArgs {
    /// Path to an unpublished Soracloud container workspace JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
    container: PathBuf,
    /// Path to a `SoraServiceManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
    service: PathBuf,
    /// Canonical service bundle bytes to qualify in every target store.
    #[arg(long, value_name = "PATH")]
    bundle_file: PathBuf,
    /// Exact Unix-second retention identity reused by the later online publication.
    #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
    sorafs_retention_epoch: NonZeroU64,
    /// Exact validator account, active peer identity, and offline SoraFS store root.
    #[arg(long = "inrou-preseed-target", value_name = "VALIDATOR,PEER,PATH")]
    inrou_preseed_targets: Vec<InrouOperatorPreseedTargetArg>,
    /// Exact common configured capacity of every selected store.
    #[arg(long = "inrou-preseed-max-capacity-bytes", value_name = "BYTES")]
    inrou_preseed_max_capacity_bytes: Option<NonZeroU64>,
    /// Absolute path to the exact offline `sorafs-node` helper.
    #[arg(long = "inrou-preseed-helper", value_name = "PATH")]
    inrou_preseed_helper: Option<PathBuf>,
    /// Lowercase SHA-256 of the exact offline helper.
    #[arg(long = "inrou-preseed-helper-sha256", value_name = "HEX")]
    inrou_preseed_helper_sha256: Option<String>,
    /// Absolute owner-only output path for the immutable online qualification.
    #[arg(long = "receipt-out", value_name = "PATH")]
    receipt_out: PathBuf,
    /// Positive helper readiness/release timeout.
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 10,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    timeout_secs: u64,
}

impl InrouServicePreseedArgs {
    fn run(self, key_pair: &KeyPair) -> Result<InrouPreseedQualificationOutput> {
        let release_identity = SorafsReleaseIdentityV1::new(self.sorafs_retention_epoch);
        let container: UnpublishedContainerManifestV1 = load_json(&self.container)?;
        let service: SoraServiceManifestV1 = load_json(&self.service)?;
        let mut bundle = UnpublishedDeploymentBundleV1 { container, service };
        validate_inrou_preseed_artifact_count([&bundle])?;
        let required_count = required_inrou_preseed_store_count([&bundle]).ok_or_else(|| {
            eyre!("soracloud service preseed requires an HttpService + Inrou manifest pair")
        })?;
        let config = validate_inrou_operator_preseed(
            Some(required_count),
            &self.inrou_preseed_targets,
            self.inrou_preseed_max_capacity_bytes,
            self.inrou_preseed_helper.as_deref(),
            self.inrou_preseed_helper_sha256.as_deref(),
        )?
        .expect("required Inrou preseed returns a configuration");
        bind_exact_inrou_placement_targets(&mut bundle.service, &config.placement_targets())?;
        let workspace_dir = direct_service_artifact_workspace_dir(
            &self.container,
            &self.service,
            &self.bundle_file,
        );
        validate_unpublished_deployment_source(&bundle)?;
        validate_local_inrou_guest_image_sources(workspace_dir.as_deref(), &bundle)?;
        let mut prepared = prepare_service_artifacts(
            &self.bundle_file,
            workspace_dir.as_deref(),
            bundle,
            key_pair,
            release_identity,
        )?;
        if service_uses_public_inrou_http_route(&prepared.admitted_bundle) {
            let (_, _, discovery) = prepare_public_service_discovery(
                &prepared.admitted_bundle,
                key_pair,
                release_identity,
            )?;
            prepared.artifacts.push(discovery);
        }
        let artifact_refs = distinct_prepared_sorafs_artifacts(prepared.artifacts.iter());
        let session =
            start_inrou_operator_preseed_session(Some(&config), &artifact_refs, self.timeout_secs)?
                .expect("required Inrou preseed starts a session");
        let receipt = session.receipt.clone();
        let (receipt_path, _) =
            write_inrou_preseed_qualification_file(&self.receipt_out, &receipt)?;
        if session.finish()? != receipt {
            return Err(eyre!(
                "released Inrou preseed helper returned a different durable qualification"
            ));
        }
        Ok(InrouPreseedQualificationOutput {
            status: "qualified_offline".to_owned(),
            receipt_path: receipt_path.to_string_lossy().into_owned(),
            receipt,
        })
    }
}

/// Arguments for the separate offline `soracloud app preseed` phase.
#[derive(clap::Args, Debug)]
pub struct InrouAppPreseedArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Exact Unix-second retention identity reused by the later online release.
    #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
    sorafs_retention_epoch: NonZeroU64,
    /// Exact validator account, active peer identity, and offline SoraFS store root.
    #[arg(long = "inrou-preseed-target", value_name = "VALIDATOR,PEER,PATH")]
    inrou_preseed_targets: Vec<InrouOperatorPreseedTargetArg>,
    /// Exact common configured capacity of every selected store.
    #[arg(long = "inrou-preseed-max-capacity-bytes", value_name = "BYTES")]
    inrou_preseed_max_capacity_bytes: Option<NonZeroU64>,
    /// Absolute path to the exact offline `sorafs-node` helper.
    #[arg(long = "inrou-preseed-helper", value_name = "PATH")]
    inrou_preseed_helper: Option<PathBuf>,
    /// Lowercase SHA-256 of the exact offline helper.
    #[arg(long = "inrou-preseed-helper-sha256", value_name = "HEX")]
    inrou_preseed_helper_sha256: Option<String>,
    /// Absolute owner-only output path for the immutable online qualification.
    #[arg(long = "receipt-out", value_name = "PATH")]
    receipt_out: PathBuf,
    /// Positive helper readiness/release timeout.
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 10,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    timeout_secs: u64,
}

impl InrouAppPreseedArgs {
    fn run(self, key_pair: &KeyPair) -> Result<InrouPreseedQualificationOutput> {
        AppBuildAndSyncArgs {
            manifest: self.manifest.clone(),
            dry_run: false,
        }
        .run()
        .wrap_err("build and synchronize exact app artifacts before offline Inrou preseed")?;
        let release_identity = SorafsReleaseIdentityV1::new(self.sorafs_retention_epoch);
        let manifest: SoracloudAppManifestV1 = load_json(&self.manifest)?;
        manifest.validate()?;
        let manifest_dir = self
            .manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let mut bundles = Vec::with_capacity(manifest.services.len());
        for service in &manifest.services {
            let container_path = resolve_manifest_path(&manifest_dir, &service.container_manifest);
            let service_path = resolve_manifest_path(&manifest_dir, &service.service_manifest);
            let container: UnpublishedContainerManifestV1 = load_json(&container_path)?;
            let service_manifest: SoraServiceManifestV1 = load_json(&service_path)?;
            ensure_app_service_ref_matches_manifest_name(
                &service.service_name,
                &service_path,
                &service_manifest,
            )?;
            bundles.push((
                service,
                container_path,
                service_path,
                UnpublishedDeploymentBundleV1 {
                    container,
                    service: service_manifest,
                },
            ));
        }
        validate_inrou_preseed_artifact_count(bundles.iter().map(|(_, _, _, bundle)| bundle))?;
        let required_count =
            required_inrou_preseed_store_count(bundles.iter().map(|(_, _, _, bundle)| bundle))
                .ok_or_else(|| {
                    eyre!("soracloud app preseed requires at least one HttpService + Inrou service")
                })?;
        let config = validate_inrou_operator_preseed(
            Some(required_count),
            &self.inrou_preseed_targets,
            self.inrou_preseed_max_capacity_bytes,
            self.inrou_preseed_helper.as_deref(),
            self.inrou_preseed_helper_sha256.as_deref(),
        )?
        .expect("required Inrou app preseed returns a configuration");
        let placement_targets = config.placement_targets();
        let mut artifacts = Vec::new();
        for (service, container_path, service_path, mut bundle) in bundles {
            if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
                || bundle.container.runtime != SoraContainerRuntimeV1::Inrou
            {
                continue;
            }
            bind_exact_inrou_placement_targets(&mut bundle.service, &placement_targets)?;
            let workspace_dir = app_service_workspace_dir(&container_path, &service_path);
            validate_unpublished_deployment_source(&bundle)?;
            validate_local_inrou_guest_image_sources(workspace_dir.as_deref(), &bundle)?;
            let bundle_file = service
                .bundle_file
                .as_deref()
                .map(|path| resolve_manifest_path(&manifest_dir, path))
                .ok_or_else(|| {
                    eyre!(
                        "app service `{}` must declare bundle_file for offline Inrou preseed",
                        service.service_name
                    )
                })?;
            let mut prepared = prepare_service_artifacts(
                &bundle_file,
                workspace_dir.as_deref(),
                bundle,
                key_pair,
                release_identity,
            )?;
            if service_uses_public_inrou_http_route(&prepared.admitted_bundle) {
                let (_, _, discovery) = prepare_public_service_discovery(
                    &prepared.admitted_bundle,
                    key_pair,
                    release_identity,
                )?;
                prepared.artifacts.push(discovery);
            }
            artifacts.extend(prepared.artifacts);
        }
        let artifact_refs = distinct_prepared_sorafs_artifacts(artifacts.iter());
        let session =
            start_inrou_operator_preseed_session(Some(&config), &artifact_refs, self.timeout_secs)?
                .expect("required Inrou app preseed starts a session");
        let receipt = session.receipt.clone();
        let (receipt_path, _) =
            write_inrou_preseed_qualification_file(&self.receipt_out, &receipt)?;
        if session.finish()? != receipt {
            return Err(eyre!(
                "released Inrou preseed helper returned a different durable qualification"
            ));
        }
        Ok(InrouPreseedQualificationOutput {
            status: "qualified_offline".to_owned(),
            receipt_path: receipt_path.to_string_lossy().into_owned(),
            receipt,
        })
    }
}

/// Arguments for `soracloud app release`.
#[derive(clap::Args, Debug)]
pub struct AppReleaseArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Exact Unix-second retention boundary embedded in every SoraFS manifest in this release.
    /// Reuse the same value for every retry; it must remain ahead of consensus time.
    #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
    sorafs_retention_epoch: NonZeroU64,
    /// Torii base URL for the canonical app-infra release mutation.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// Positive timeout for Torii requests.
    #[arg(
        long,
        value_name = "SECS",
        default_value_t = 10,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    timeout_secs: u64,
    /// Print the resolved release plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    /// Absolute owner-only ingest qualification from `soracloud app preseed`.
    #[arg(long = "inrou-preseed-receipt", value_name = "PATH")]
    inrou_preseed_receipt: Option<PathBuf>,
}
impl AppReleaseArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<AppReleaseOutput> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?.to_owned();
        let uses_api_token = self.api_token.is_some();
        let mut notes = Vec::new();
        notes.push(
            "release composes the manifest-adjacent build-and-sync path with one explicit deploy through one canonical app-infra submission"
                .to_owned(),
        );
        if uses_api_token {
            notes.push("API token will be forwarded to the Torii control plane".to_owned());
        }
        let build_and_sync = AppBuildAndSyncArgs {
            manifest: self.manifest.clone(),
            dry_run: self.dry_run,
        }
        .run()?;
        let plan = build_app_local_plan_output(self.manifest.as_path())?;
        let live_verification_targets = app_live_verification_targets(self.manifest.as_path())?;
        notes.push(format!(
            "release requires {} exact live route/health verification target(s)",
            live_verification_targets.len()
        ));
        notes.extend(plan.notes.iter().cloned());
        if self.dry_run {
            let report = build_soracloud_app_report(
                plan.app_name.clone(),
                plan.manifest_path.clone(),
                false,
                vec![
                    skipped_app_phase("build", "dry-run only resolved the build script plan"),
                    skipped_app_phase("sync_manifests", "dry-run does not mutate manifests"),
                    skipped_app_phase("doctor", "dry-run does not run app doctor"),
                    skipped_app_phase("publish", "dry-run does not publish artifacts"),
                    skipped_app_phase("sign", "dry-run does not sign requests"),
                    skipped_app_phase("submit", "dry-run does not submit transactions"),
                    skipped_app_phase("status", "dry-run does not query live status"),
                    skipped_app_phase("verify", "dry-run reports the planned release only"),
                ],
                None,
                plan.routes.clone(),
                app_report_services_from_plan(&plan.services),
                None,
                vec!["dry-run did not build, publish, submit, reconcile, or live-verify the release".to_owned()],
                "Run `iroha soracloud app simulate` for a local non-mutating readiness report or `iroha soracloud app release` without --dry-run to deploy.".to_owned(),
            );
            return Ok(AppReleaseOutput {
                report,
                mode: "dry_run".to_owned(),
                release_mode: "deploy".to_owned(),
                torii_url,
                uses_api_token,
                plan,
                build_and_sync,
                release_response: None,
                status_response: None,
                live_verifications: Vec::new(),
                notes,
            });
        }
        let doctor = AppDoctorArgs {
            manifest: self.manifest.clone(),
        }
        .run()?;
        if !doctor.ok {
            return Err(eyre!(
                "app doctor failed before release: {}",
                doctor.report.blockers.join(", ")
            ));
        }
        let mutation_args = AppReleaseMutationArgs {
            manifest: self.manifest.clone(),
            sorafs_retention_epoch: self.sorafs_retention_epoch,
            inrou_preseed_receipt: self.inrou_preseed_receipt.clone(),
            torii_url: Some(torii_url.clone()),
            api_token: self.api_token.clone(),
            timeout_secs: self.timeout_secs,
        };
        let release_response = mutation_args.run(MutationMode::Deploy, authority, key_pair)?;
        let release_mode = "Deploy".to_owned();
        notes.extend(release_response.notes.iter().cloned());
        let status_response = AppStatusArgs {
            manifest: self.manifest.clone(),
            torii_url: Some(torii_url.clone()),
            api_token: self.api_token.clone(),
            timeout_secs: self.timeout_secs,
        }
        .run()?;
        notes.extend(status_response.notes.iter().cloned());
        if !status_response.report.ok {
            return Err(eyre!(
                "release status contains app-level blockers: {}",
                status_response.report.blockers.join(", ")
            ));
        }
        let live_verifications =
            verify_app_live_targets(&live_verification_targets, self.timeout_secs)?;
        notes.push(format!(
            "verified {} live route/health target(s) with exact 2xx responses",
            live_verifications.len()
        ));
        let app_infra_manifest_hash = release_response.app_infra_manifest_hash;
        let report = build_soracloud_app_report(
            plan.app_name.clone(),
            plan.manifest_path.clone(),
            status_response.report.ok,
            vec![
                app_phase_report(
                    "build",
                    true,
                    vec!["build-and-sync script completed".to_owned()],
                ),
                app_phase_report(
                    "sync_manifests",
                    true,
                    vec!["manifest hashes were synchronized before release submission".to_owned()],
                ),
                app_phase_report("doctor", true, doctor.notes.clone()),
                app_phase_report("publish", true, release_response.notes.clone()),
                app_phase_report("sign", true, vec!["signed app-infra request".to_owned()]),
                app_phase_report(
                    "submit",
                    true,
                    vec![format!("release mode: {release_mode}")],
                ),
                app_phase_report(
                    "status",
                    status_response.report.ok,
                    status_response.notes.clone(),
                ),
                app_phase_report(
                    "verify",
                    true,
                    live_verifications
                        .iter()
                        .map(|verification| {
                            format!(
                                "{} returned HTTP {} at {}",
                                verification.label, verification.status_code, verification.url
                            )
                        })
                        .collect(),
                ),
            ],
            app_infra_manifest_hash,
            plan.routes.clone(),
            app_report_services_from_plan(&plan.services),
            None,
            status_response.report.blockers.clone(),
            if status_response.report.ok {
                "Release submitted, reconciled against authoritative status, and passed live route/health verification.".to_owned()
            } else {
                "Resolve status blockers before promoting the rollout.".to_owned()
            },
        );
        Ok(AppReleaseOutput {
            report,
            mode: "completed".to_owned(),
            release_mode,
            torii_url,
            uses_api_token,
            plan,
            build_and_sync,
            release_response: Some(release_response),
            status_response: Some(status_response),
            live_verifications,
            notes,
        })
    }
}
/// Arguments for `iroha soracloud app simulate`.
#[derive(clap::Args, Debug)]
pub struct AppSimulateArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Exact Unix-second retention boundary used to reproduce release manifest identities.
    #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
    sorafs_retention_epoch: NonZeroU64,
}
impl AppSimulateArgs {
    fn run(self, _authority: &AccountId, key_pair: &KeyPair) -> Result<AppSimulateOutput> {
        let release_identity = SorafsReleaseIdentityV1::new(self.sorafs_retention_epoch);
        let manifest_path = self.manifest.clone();
        let manifest_dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let manifest: SoracloudAppManifestV1 = load_json(&manifest_path)?;
        manifest.validate()?;
        let (synced_manifests, unpublished_bundles) =
            project_app_manifest_service_refs(&manifest, &manifest_dir)?;
        let plan = build_app_local_plan_output(&manifest_path)?;
        let planned_static_site = manifest
            .static_site
            .as_ref()
            .map(|static_site| {
                plan_app_static_site_publication(
                    &manifest,
                    &manifest_dir,
                    static_site,
                    key_pair,
                    release_identity,
                )
            })
            .transpose()?;
        let notes = vec![
            "simulate completed without live Torii reads or transaction submission".to_owned(),
            format!(
                "would synchronize {} service manifest pair(s)",
                synced_manifests.len()
            ),
            "would publish static site and service artifacts required by the app release"
                .to_owned(),
            format!(
                "would publish {} source service bundle(s) before constructing an admitted app-infra request",
                unpublished_bundles.len()
            ),
        ];
        let report = build_soracloud_app_report(
            plan.app_name.clone(),
            plan.manifest_path.clone(),
            true,
            vec![
                skipped_app_phase("build", "simulate does not run build scripts"),
                app_phase_report(
                    "sync_manifests",
                    true,
                    vec![format!(
                        "manifest sync projection covers {} service manifest pair(s)",
                        synced_manifests.len()
                    )],
                ),
                app_phase_report(
                    "doctor",
                    true,
                    vec!["local topology projection succeeded".to_owned()],
                ),
                skipped_app_phase(
                    "publish",
                    "simulate does not publish source guest images or service bundles",
                ),
                skipped_app_phase(
                    "sign",
                    "an admitted app-infra request exists only after concrete artifact publication",
                ),
                skipped_app_phase("submit", "simulate never submits transactions"),
                skipped_app_phase("status", "simulate never queries live Torii status"),
                skipped_app_phase("verify", "simulate reports release readiness only"),
            ],
            None,
            plan.routes.clone(),
            app_report_services_from_plan(&plan.services),
            manifest.static_site.clone(),
            Vec::new(),
            "Run `iroha soracloud app release` against a Torii URL to publish and submit."
                .to_owned(),
        );
        Ok(AppSimulateOutput {
            report,
            mode: "simulated".to_owned(),
            plan,
            synced_manifests,
            planned_static_site,
            app_infra_request: None,
            notes,
        })
    }
}
/// Arguments for `soracloud app dev`.
#[derive(clap::Args, Debug)]
pub struct AppLocalDevArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Print the resolved dev command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
impl AppLocalDevArgs {
    fn run(self) -> Result<AppLocalDevOutput> {
        let plan = build_app_local_plan_output(self.manifest.as_path())?;
        let script_path = resolve_app_root_script(self.manifest.as_path(), "dev.sh")?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let command = vec!["./dev.sh".to_owned()];
        if self.dry_run {
            return Ok(AppLocalDevOutput {
                app_name: plan.app_name,
                public_url: plan.public_url,
                hostname: plan.hostname.clone(),
                manifest_path: self.manifest.to_string_lossy().into_owned(),
                workspace_dir: plan.workspace_dir.clone(),
                workspace_scripts: plan.workspace_scripts.clone(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                mode: "dry_run".to_owned(),
                has_mixed_planes: plan.has_mixed_planes,
                hosted_http_service_count: plan.hosted_http_service_count,
                deterministic_service_count: plan.deterministic_service_count,
                frontend: plan.frontend.clone(),
                services: plan.services.clone(),
                routes: plan.routes.clone(),
                command,
                exit_status: None,
                notes: plan.notes.clone(),
            });
        }
        let status = ProcessCommand::new(&script_path)
            .current_dir(&working_dir)
            .status()
            .wrap_err_with(|| {
                format!(
                    "failed to run local dev script `{}` resolved from `{}`",
                    script_path.display(),
                    self.manifest.display()
                )
            })?;
        let exit_status = status.code();
        if exit_status == Some(130) {
            let mut notes = plan.notes;
            notes.push(
                "local dev entrypoint exited with status 130; treating the session as an interactive interrupt"
                    .to_owned(),
            );
            return Ok(AppLocalDevOutput {
                app_name: plan.app_name,
                public_url: plan.public_url,
                hostname: plan.hostname.clone(),
                manifest_path: self.manifest.to_string_lossy().into_owned(),
                workspace_dir: plan.workspace_dir.clone(),
                workspace_scripts: plan.workspace_scripts.clone(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                mode: "interrupted".to_owned(),
                has_mixed_planes: plan.has_mixed_planes,
                hosted_http_service_count: plan.hosted_http_service_count,
                deterministic_service_count: plan.deterministic_service_count,
                frontend: plan.frontend.clone(),
                services: plan.services.clone(),
                routes: plan.routes.clone(),
                command,
                exit_status,
                notes,
            });
        }
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "local dev script `{}` exited with status {rendered_status}",
                script_path.display()
            ));
        }
        Ok(AppLocalDevOutput {
            app_name: plan.app_name,
            public_url: plan.public_url,
            hostname: plan.hostname.clone(),
            manifest_path: self.manifest.to_string_lossy().into_owned(),
            workspace_dir: plan.workspace_dir.clone(),
            workspace_scripts: plan.workspace_scripts.clone(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            mode: "completed".to_owned(),
            has_mixed_planes: plan.has_mixed_planes,
            hosted_http_service_count: plan.hosted_http_service_count,
            deterministic_service_count: plan.deterministic_service_count,
            frontend: plan.frontend.clone(),
            services: plan.services.clone(),
            routes: plan.routes.clone(),
            command,
            exit_status,
            notes: plan.notes,
        })
    }
}
/// Arguments for `soracloud app build`.
#[derive(clap::Args, Debug)]
pub struct AppBuildAndSyncArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Print the resolved build-and-sync command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
impl AppBuildAndSyncArgs {
    fn run(self) -> Result<AppBuildAndSyncOutput> {
        let script_path = resolve_app_root_script(self.manifest.as_path(), "build-and-sync.sh")?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let command = vec!["./build-and-sync.sh".to_owned()];
        if self.dry_run {
            let plan = build_app_local_plan_output(self.manifest.as_path())?;
            return Ok(AppBuildAndSyncOutput {
                app_name: plan.app_name,
                public_url: plan.public_url,
                hostname: plan.hostname.clone(),
                manifest_path: self.manifest.to_string_lossy().into_owned(),
                workspace_dir: plan.workspace_dir.clone(),
                workspace_scripts: plan.workspace_scripts.clone(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                mode: "dry_run".to_owned(),
                has_mixed_planes: plan.has_mixed_planes,
                hosted_http_service_count: plan.hosted_http_service_count,
                deterministic_service_count: plan.deterministic_service_count,
                frontend: plan.frontend.clone(),
                services: plan.services.clone(),
                routes: plan.routes.clone(),
                command,
                exit_status: None,
                notes: plan.notes.clone(),
            });
        }
        let status = ProcessCommand::new(&script_path)
            .current_dir(&working_dir)
            .status()
            .wrap_err_with(|| {
                format!(
                    "failed to run build-and-sync script `{}` resolved from `{}`",
                    script_path.display(),
                    self.manifest.display()
                )
            })?;
        let exit_status = status.code();
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "build-and-sync script `{}` exited with status {rendered_status}",
                script_path.display()
            ));
        }
        let plan = build_app_local_plan_output(self.manifest.as_path())?;
        let mut notes = plan.notes;
        notes.push("build-and-sync completed through the manifest-adjacent root script".to_owned());
        Ok(AppBuildAndSyncOutput {
            app_name: plan.app_name,
            public_url: plan.public_url,
            hostname: plan.hostname.clone(),
            manifest_path: self.manifest.to_string_lossy().into_owned(),
            workspace_dir: plan.workspace_dir.clone(),
            workspace_scripts: plan.workspace_scripts.clone(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            mode: "completed".to_owned(),
            has_mixed_planes: plan.has_mixed_planes,
            hosted_http_service_count: plan.hosted_http_service_count,
            deterministic_service_count: plan.deterministic_service_count,
            frontend: plan.frontend.clone(),
            services: plan.services.clone(),
            routes: plan.routes.clone(),
            command,
            exit_status,
            notes,
        })
    }
}
/// Arguments for `soracloud app release`.
#[cfg(test)]
#[derive(clap::Args, Debug)]
pub struct AppReleaseWorkspaceArgs {
    /// Path to a `SoracloudAppManifestV1` JSON document.
    #[arg(long, value_name = "PATH", default_value = "app_manifest.json")]
    manifest: PathBuf,
    /// Torii base URL forwarded to the workspace entrypoint through `TORII_URL`.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token forwarded to the workspace entrypoint through `API_TOKEN`.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout forwarded to the underlying release command.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
    /// Print the resolved release command plan without executing it.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}
#[cfg(test)]
impl AppReleaseWorkspaceArgs {
    fn run(self) -> Result<AppReleaseWorkspaceScriptOutput> {
        let plan = build_app_local_plan_output(self.manifest.as_path())?;
        let script_path = resolve_app_root_script(self.manifest.as_path(), "release.sh")?;
        let working_dir = script_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let torii_url = require_torii_url(self.torii_url.as_deref())?.to_owned();
        let command = build_app_release_workspace_command(self.timeout_secs);
        let mut notes = plan.notes;
        notes.push(
            "release will run through the manifest-adjacent root script after exporting TORII_URL"
                .to_owned(),
        );
        if self.api_token.is_some() {
            notes.push(
                "API token will be forwarded through API_TOKEN instead of the command line"
                    .to_owned(),
            );
        }
        if self.dry_run {
            return Ok(AppReleaseWorkspaceScriptOutput {
                app_name: plan.app_name,
                public_url: plan.public_url,
                hostname: plan.hostname.clone(),
                manifest_path: self.manifest.to_string_lossy().into_owned(),
                workspace_dir: plan.workspace_dir.clone(),
                workspace_scripts: plan.workspace_scripts.clone(),
                working_dir: working_dir.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
                script_name: "release.sh".to_owned(),
                mode: "dry_run".to_owned(),
                torii_url,
                uses_api_token: self.api_token.is_some(),
                has_mixed_planes: plan.has_mixed_planes,
                hosted_http_service_count: plan.hosted_http_service_count,
                deterministic_service_count: plan.deterministic_service_count,
                frontend: plan.frontend.clone(),
                services: plan.services.clone(),
                routes: plan.routes.clone(),
                command,
                exit_status: None,
                notes,
            });
        }
        let mut process = ProcessCommand::new(&script_path);
        process
            .current_dir(&working_dir)
            .env("TORII_URL", &torii_url);
        if let Some(api_token) = self.api_token.as_deref() {
            process.env("API_TOKEN", api_token);
        }
        process
            .arg("--timeout-secs")
            .arg(self.timeout_secs.to_string());
        let status = process.status().wrap_err_with(|| {
            format!(
                "failed to run release script `{}` resolved from `{}`",
                script_path.display(),
                self.manifest.display()
            )
        })?;
        let exit_status = status.code();
        if !status.success() {
            let rendered_status = exit_status
                .map(|code| code.to_string())
                .unwrap_or_else(|| "terminated by signal".to_owned());
            return Err(eyre!(
                "release script `{}` exited with status {rendered_status}",
                script_path.display()
            ));
        }
        notes.push("release completed through the manifest-adjacent root script".to_owned());
        Ok(AppReleaseWorkspaceScriptOutput {
            app_name: plan.app_name,
            public_url: plan.public_url,
            hostname: plan.hostname.clone(),
            manifest_path: self.manifest.to_string_lossy().into_owned(),
            workspace_dir: plan.workspace_dir.clone(),
            workspace_scripts: plan.workspace_scripts.clone(),
            working_dir: working_dir.to_string_lossy().into_owned(),
            script_path: script_path.to_string_lossy().into_owned(),
            script_name: "release.sh".to_owned(),
            mode: "completed".to_owned(),
            torii_url,
            uses_api_token: self.api_token.is_some(),
            has_mixed_planes: plan.has_mixed_planes,
            hosted_http_service_count: plan.hosted_http_service_count,
            deterministic_service_count: plan.deterministic_service_count,
            frontend: plan.frontend.clone(),
            services: plan.services.clone(),
            routes: plan.routes.clone(),
            command,
            exit_status,
            notes,
        })
    }
}
fn resolve_app_root_script(manifest_path: &Path, script_name: &str) -> Result<PathBuf> {
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let script_path = manifest_dir.join(script_name);
    if !script_path.is_file() {
        return Err(eyre!(
            "expected app-local script `{}` adjacent to manifest `{}`",
            script_path.display(),
            manifest_path.display()
        ));
    }
    fs::canonicalize(&script_path).wrap_err_with(|| {
        format!(
            "failed to canonicalize app-local script {}",
            script_path.display()
        )
    })
}
fn canonicalize_cli_arg_path(path: Option<&Path>, flag_name: &str) -> Result<Option<PathBuf>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let canonical = fs::canonicalize(path)
        .wrap_err_with(|| format!("failed to resolve {flag_name} path `{}`", path.display()))?;
    Ok(Some(canonical))
}
fn build_service_workspace_mutation_command(
    script_name: &str,
    timeout_secs: u64,
    initial_configs: Option<&Path>,
    initial_secrets: Option<&Path>,
) -> Vec<String> {
    let mut command = vec![format!("./{script_name}")];
    if let Some(path) = initial_configs {
        command.push("--initial-configs".to_owned());
        command.push(path.to_string_lossy().into_owned());
    }
    if let Some(path) = initial_secrets {
        command.push("--initial-secrets".to_owned());
        command.push(path.to_string_lossy().into_owned());
    }
    command.push("--timeout-secs".to_owned());
    command.push(timeout_secs.to_string());
    command
}
#[cfg(test)]
fn build_app_release_workspace_command(timeout_secs: u64) -> Vec<String> {
    vec![
        "./release.sh".to_owned(),
        "--timeout-secs".to_owned(),
        timeout_secs.to_string(),
    ]
}
fn resolve_optional_workspace_service_name(
    service_name: Option<String>,
    container_manifest: Option<&Path>,
    service_manifest: Option<&Path>,
    command_name: &str,
) -> Result<Option<String>> {
    let exact_service_name = service_name
        .map(|value| {
            let parsed: Name = value.parse().wrap_err("invalid --service-name")?;
            if parsed.as_ref() != value {
                return Err(eyre!(
                    "--service-name must use its exact canonical V1 spelling"
                ));
            }
            Ok(value)
        })
        .transpose()?;
    match (container_manifest, service_manifest) {
        (None, None) => Ok(exact_service_name),
        (Some(_), None) | (None, Some(_)) => Err(eyre!(
            "`{command_name}` requires both --container and --service when resolving the service name from workspace manifests"
        )),
        (Some(container_manifest), Some(service_manifest)) => {
            let _: UnpublishedContainerManifestV1 = load_json(container_manifest)?;
            let service: SoraServiceManifestV1 = load_json(service_manifest)?;
            let resolved_service_name = service.service_name.to_string();
            if let Some(expected_service_name) = exact_service_name
                && expected_service_name != resolved_service_name
            {
                return Err(eyre!(
                    "--service-name `{expected_service_name}` does not match the service manifest `{}` resolved as `{resolved_service_name}`",
                    service_manifest.display()
                ));
            }
            Ok(Some(resolved_service_name))
        }
    }
}
fn resolve_required_workspace_service_name(
    service_name: Option<String>,
    container_manifest: Option<&Path>,
    service_manifest: Option<&Path>,
    command_name: &str,
) -> Result<String> {
    resolve_optional_workspace_service_name(
        service_name,
        container_manifest,
        service_manifest,
        command_name,
    )?
    .ok_or_else(|| {
        eyre!(
            "`{command_name}` requires --service-name or a manifest pair via --container and --service"
        )
    })
}
fn workspace_script_path_if_exists(workspace_dir: &Path, script_name: &str) -> Option<String> {
    let path = workspace_dir.join(script_name);
    path.is_file().then(|| path.to_string_lossy().into_owned())
}
fn app_service_workspace_dir(
    container_manifest: &Path,
    service_manifest: &Path,
) -> Option<PathBuf> {
    let container_dir = container_manifest.parent()?;
    let service_dir = service_manifest.parent()?;
    (container_dir == service_dir).then(|| container_dir.to_path_buf())
}
fn direct_service_artifact_workspace_dir(
    container_manifest: &Path,
    service_manifest: &Path,
    bundle_file: &Path,
) -> Option<PathBuf> {
    let manifest_dir = app_service_workspace_dir(container_manifest, service_manifest)?;
    if manifest_dir.join("inrou").is_dir() {
        return Some(manifest_dir);
    }
    bundle_file
        .parent()
        .and_then(Path::parent)
        .filter(|workspace| workspace.join("inrou").is_dir())
        .map(Path::to_path_buf)
        .or(Some(manifest_dir))
}
fn app_service_workspace_scripts(
    service_workspace_dir: Option<&Path>,
) -> AppLocalServiceWorkspaceScriptsOutput {
    AppLocalServiceWorkspaceScriptsOutput {
        dev: service_workspace_dir.and_then(|dir| workspace_script_path_if_exists(dir, "dev.sh")),
        build: service_workspace_dir
            .and_then(|dir| workspace_script_path_if_exists(dir, "build.sh")),
        verify_build: service_workspace_dir
            .and_then(|dir| workspace_script_path_if_exists(dir, "verify-build.sh")),
    }
}
struct ServiceWorkspacePlan {
    service_name: String,
    execution_plane: String,
    runtime: String,
    route_host: Option<String>,
    route_path_prefix: Option<String>,
    route_visibility: Option<String>,
    replica_count: u16,
    state_binding_count: u32,
    lease_volume_count: u32,
    handler_count: u32,
    routes: Vec<ServiceLocalRouteOutput>,
    workspace_dir: String,
    workspace_scripts: ServiceWorkspaceScriptsOutput,
    notes: Vec<String>,
}
fn resolve_service_workspace_script(
    container_manifest: &Path,
    service_manifest: &Path,
    script_name: &str,
) -> Result<PathBuf> {
    let container_dir = container_manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let service_dir = service_manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if container_dir != service_dir {
        return Err(eyre!(
            "container manifest `{}` and service manifest `{}` must share a parent directory for workspace script resolution",
            container_manifest.display(),
            service_manifest.display()
        ));
    }
    let script_path = container_dir.join(script_name);
    if !script_path.is_file() {
        return Err(eyre!(
            "expected workspace script `{}` adjacent to `{}` and `{}`",
            script_path.display(),
            container_manifest.display(),
            service_manifest.display()
        ));
    }
    fs::canonicalize(&script_path).wrap_err_with(|| {
        format!(
            "failed to canonicalize workspace script {}",
            script_path.display()
        )
    })
}
fn build_service_workspace_plan(
    container_manifest: &Path,
    service_manifest: &Path,
) -> Result<ServiceWorkspacePlan> {
    let container_dir = container_manifest
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let container: UnpublishedContainerManifestV1 = load_json(container_manifest)?;
    let service: SoraServiceManifestV1 = load_json(service_manifest)?;
    let bundle = UnpublishedDeploymentBundleV1 {
        container: container.clone(),
        service: service.clone(),
    };
    validate_unpublished_deployment_source(&bundle).wrap_err_with(|| {
        format!(
            "service `{}` is not locally admissible; run `iroha soracloud service sync-manifests --container {} --service {}` after manifest edits",
            service.service_name,
            container_manifest.display(),
            service_manifest.display()
        )
    })?;
    let mut notes = Vec::new();
    if service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
        && container.runtime == SoraContainerRuntimeV1::Inrou
    {
        notes.push("workspace targets the hosted HttpService + Inrou production plane".to_owned());
    }
    if service.execution_plane == SoraServiceExecutionPlaneV1::DeterministicService
        && container.runtime == SoraContainerRuntimeV1::Ivm
    {
        notes.push("workspace targets the deterministic IVM production plane".to_owned());
    }
    let route_host = service.route.as_ref().map(|route| route.host.clone());
    let route_path_prefix = service
        .route
        .as_ref()
        .map(|route| route.path_prefix.clone());
    let route_visibility = service
        .route
        .as_ref()
        .map(|route| format!("{:?}", route.visibility));
    let mut routes = Vec::new();
    if let Some(route) = service.route.as_ref() {
        routes.push(ServiceLocalRouteOutput {
            route_kind: if service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
                && container.runtime == SoraContainerRuntimeV1::Inrou
            {
                "hosted_http_prefix".to_owned()
            } else {
                "service_prefix".to_owned()
            },
            host: route.host.clone(),
            path: route.path_prefix.clone(),
            handler_name: None,
            handler_class: None,
            certified_response: None,
            mailbox_queue: None,
        });
        for handler in &service.handlers {
            if let Some(handler_path) = handler.route_path.as_deref() {
                routes.push(ServiceLocalRouteOutput {
                    route_kind: "handler".to_owned(),
                    host: route.host.clone(),
                    path: join_service_route_path(&route.path_prefix, handler_path),
                    handler_name: Some(handler.handler_name.to_string()),
                    handler_class: Some(format!("{:?}", handler.class)),
                    certified_response: Some(format!("{:?}", handler.certified_response)),
                    mailbox_queue: handler
                        .mailbox
                        .as_ref()
                        .map(|mailbox| mailbox.queue_name.to_string()),
                });
            }
        }
    }
    Ok(ServiceWorkspacePlan {
        service_name: service.service_name.to_string(),
        execution_plane: format!("{:?}", service.execution_plane),
        runtime: format!("{:?}", container.runtime),
        route_host,
        route_path_prefix,
        route_visibility,
        replica_count: service.replicas.get(),
        state_binding_count: u32::try_from(service.state_bindings.len())
            .expect("service state binding count fits in u32"),
        lease_volume_count: u32::try_from(service.lease_volumes.len())
            .expect("service lease volume count fits in u32"),
        handler_count: u32::try_from(service.handlers.len())
            .expect("service handler count fits in u32"),
        routes,
        workspace_dir: container_dir.to_string_lossy().into_owned(),
        workspace_scripts: ServiceWorkspaceScriptsOutput {
            local_dev: workspace_script_path_if_exists(&container_dir, "dev.sh"),
            build_and_sync: workspace_script_path_if_exists(&container_dir, "build-and-sync.sh"),
            doctor: workspace_script_path_if_exists(&container_dir, "doctor.sh"),
            release: workspace_script_path_if_exists(&container_dir, "release.sh"),
            deploy: workspace_script_path_if_exists(&container_dir, "deploy.sh"),
            upgrade: workspace_script_path_if_exists(&container_dir, "upgrade.sh"),
        },
        notes,
    })
}
fn build_service_local_plan_output(
    container_manifest: &Path,
    service_manifest: &Path,
) -> Result<ServiceLocalPlanOutput> {
    let plan = build_service_workspace_plan(container_manifest, service_manifest)?;
    Ok(ServiceLocalPlanOutput {
        service_name: plan.service_name,
        container_manifest_path: container_manifest.to_string_lossy().into_owned(),
        service_manifest_path: service_manifest.to_string_lossy().into_owned(),
        workspace_dir: plan.workspace_dir,
        workspace_scripts: plan.workspace_scripts,
        execution_plane: plan.execution_plane,
        runtime: plan.runtime,
        route_host: plan.route_host,
        route_path_prefix: plan.route_path_prefix,
        route_visibility: plan.route_visibility,
        replica_count: plan.replica_count,
        state_binding_count: plan.state_binding_count,
        lease_volume_count: plan.lease_volume_count,
        handler_count: plan.handler_count,
        routes: plan.routes,
        notes: plan.notes,
    })
}
fn build_direct_service_mutation_output(
    container_manifest: &Path,
    service_manifest: &Path,
    plan: ServiceWorkspacePlan,
    mode: MutationMode,
    torii_url: &str,
    uses_api_token: bool,
    published_public_discovery: Option<PublicServiceDiscoveryPublishOutput>,
    published_bundle: ServiceBundlePublishOutput,
    published_inrou_guest_images: Vec<InrouGuestImageArtifactPublishOutput>,
    response: norito::json::Value,
) -> ServiceMutationOutput {
    let mut notes = plan.notes;
    notes.push(format!(
        "{} reconciled the local manifest pair against live Torii status",
        mode.label_lowercase()
    ));
    if uses_api_token {
        notes.push("API token was forwarded to the Torii control plane".to_owned());
    }
    ServiceMutationOutput {
        service_name: plan.service_name,
        container_manifest_path: container_manifest.to_string_lossy().into_owned(),
        service_manifest_path: service_manifest.to_string_lossy().into_owned(),
        workspace_dir: plan.workspace_dir,
        workspace_scripts: plan.workspace_scripts,
        mode: match mode {
            MutationMode::Deploy => "Deploy",
            MutationMode::Upgrade => "Upgrade",
        }
        .to_owned(),
        execution_plane: plan.execution_plane,
        runtime: plan.runtime,
        route_host: plan.route_host,
        route_path_prefix: plan.route_path_prefix,
        route_visibility: plan.route_visibility,
        replica_count: plan.replica_count,
        state_binding_count: plan.state_binding_count,
        lease_volume_count: plan.lease_volume_count,
        handler_count: plan.handler_count,
        torii_url: torii_url.to_owned(),
        uses_api_token,
        routes: plan.routes,
        published_public_discovery,
        published_bundle,
        published_inrou_guest_images,
        response,
        notes,
    }
}
fn attach_service_plan_to_output(
    output: &mut json::Value,
    service_plan: Option<ServiceLocalPlanOutput>,
) -> Result<()> {
    let Some(service_plan) = service_plan else {
        return Ok(());
    };
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("expected JSON object when attaching service plan"));
    };
    root.insert("service_plan".to_owned(), json::to_value(&service_plan)?);
    Ok(())
}
fn app_phase_report(name: &str, ok: bool, diagnostics: Vec<String>) -> SoracloudAppPhaseReportV1 {
    SoracloudAppPhaseReportV1 {
        name: name.to_owned(),
        ok,
        skipped: false,
        diagnostics,
    }
}
fn skipped_app_phase(name: &str, reason: &str) -> SoracloudAppPhaseReportV1 {
    SoracloudAppPhaseReportV1 {
        name: name.to_owned(),
        ok: false,
        skipped: true,
        diagnostics: vec![reason.to_owned()],
    }
}
fn app_report_services_from_plan(
    services: &[AppLocalServicePlanOutput],
) -> Vec<SoracloudAppReportServiceV1> {
    services
        .iter()
        .map(|service| SoracloudAppReportServiceV1 {
            service_name: service.service_name.clone(),
            execution_plane: service.execution_plane.clone(),
            runtime: service.runtime.clone(),
        })
        .collect()
}
fn app_report_services_from_status(
    services: &[AppServiceStatusOutput],
) -> Vec<SoracloudAppReportServiceV1> {
    services
        .iter()
        .map(|service| SoracloudAppReportServiceV1 {
            service_name: service.service_name.clone(),
            execution_plane: service.execution_plane.clone(),
            runtime: service.runtime.clone(),
        })
        .collect()
}
fn build_soracloud_app_report(
    app_name: String,
    manifest_path: String,
    ok: bool,
    phases: Vec<SoracloudAppPhaseReportV1>,
    app_infra_manifest_hash: Option<Hash>,
    routes: Vec<AppLocalRoutePlanOutput>,
    services: Vec<SoracloudAppReportServiceV1>,
    static_site: Option<SoracloudAppStaticSiteV1>,
    blockers: Vec<String>,
    next_action: String,
) -> SoracloudAppReportV1 {
    SoracloudAppReportV1 {
        schema_version: SORACLOUD_APP_REPORT_SCHEMA_VERSION.to_owned(),
        app_name,
        manifest_path,
        ok,
        phases,
        app_infra_manifest_hash,
        routes,
        services,
        static_site,
        blockers,
        next_action,
    }
}
fn maybe_service_local_plan(
    container_manifest: Option<&Path>,
    service_manifest: Option<&Path>,
) -> Result<Option<ServiceLocalPlanOutput>> {
    match (container_manifest, service_manifest) {
        (Some(container_manifest), Some(service_manifest)) => Ok(Some(
            build_service_local_plan_output(container_manifest, service_manifest)?,
        )),
        _ => Ok(None),
    }
}
fn build_app_frontend_projection(
    public_url: &str,
    static_site: Option<&SoracloudAppStaticSiteV1>,
    manifest_path: &Path,
) -> Result<Option<AppLocalFrontendPlanOutput>> {
    let public_origin = normalize_app_public_origin_url(public_url, manifest_path)?;
    Ok(static_site.map(|static_site| {
        let cid_gateway_url_template = (static_site.publish_mode
            == APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY)
            .then(|| format!("{public_origin}sorafs/cid/<cid>"));
        let root_binding_url = (static_site.publish_mode
            == APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING)
            .then(|| public_origin.to_string());
        AppLocalFrontendPlanOutput {
            dist_dir: static_site.dist_dir.clone(),
            mount_path: static_site.mount_path.clone(),
            publish_mode: static_site.publish_mode.clone(),
            api_base_path: static_site.api_base_path.clone(),
            cid_gateway_url_template,
            root_binding_url,
        }
    }))
}
#[derive(Clone, Debug)]
struct AppRootProjection {
    manifest_path: String,
    public_url: String,
    hostname: String,
    workspace_dir: String,
    workspace_scripts: AppLocalWorkspaceScriptsOutput,
}
fn build_app_root_projection(manifest_path: &Path, public_url: &str) -> Result<AppRootProjection> {
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let public_origin = normalize_app_public_origin_url(public_url, manifest_path)?;
    let hostname = public_origin
        .host_str()
        .ok_or_else(|| eyre!("app manifest public_url must include a hostname"))?
        .to_owned();
    Ok(AppRootProjection {
        manifest_path: manifest_path.to_string_lossy().into_owned(),
        public_url: public_url.to_owned(),
        hostname,
        workspace_dir: manifest_dir.to_string_lossy().into_owned(),
        workspace_scripts: AppLocalWorkspaceScriptsOutput {
            local_dev: workspace_script_path_if_exists(&manifest_dir, "dev.sh"),
            build_and_sync: workspace_script_path_if_exists(&manifest_dir, "build-and-sync.sh"),
            doctor: workspace_script_path_if_exists(&manifest_dir, "doctor.sh"),
            release: workspace_script_path_if_exists(&manifest_dir, "release.sh"),
        },
    })
}
fn build_app_local_plan_output(manifest_path: &Path) -> Result<AppLocalPlanOutput> {
    let manifest: SoracloudAppManifestV1 = load_json(manifest_path)?;
    manifest.validate()?;
    let manifest_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let root = build_app_root_projection(manifest_path, &manifest.public_url)?;
    let frontend = build_app_frontend_projection(
        &manifest.public_url,
        manifest.static_site.as_ref(),
        manifest_path,
    )?;
    let mut services = Vec::with_capacity(manifest.services.len());
    let mut routes = Vec::new();
    let mut hosted_http_service_count = 0_u32;
    let mut deterministic_service_count = 0_u32;
    for service_ref in &manifest.services {
        let container_path = resolve_manifest_path(&manifest_dir, &service_ref.container_manifest);
        let service_path = resolve_manifest_path(&manifest_dir, &service_ref.service_manifest);
        let (_synced, bundle) = match project_sync_manifest_pair(
            &container_path,
            &service_path,
            None,
            Some(&service_ref.service_name),
        ) {
            Ok(projected) => projected,
            Err(error) if is_app_service_name_mismatch_error(&error) => return Err(error),
            Err(error) => {
                let context = format!(
                    "app service `{}` is not locally admissible; run `iroha soracloud service sync-manifests --app-manifest {}` after manifest edits",
                    service_ref.service_name,
                    manifest_path.display()
                );
                return Err(error.wrap_err(context));
            }
        };
        let container = bundle.container;
        let service = bundle.service;
        let is_hosted_http = service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
            && container.runtime == SoraContainerRuntimeV1::Inrou;
        let is_deterministic = service.execution_plane
            == SoraServiceExecutionPlaneV1::DeterministicService
            && container.runtime == SoraContainerRuntimeV1::Ivm;
        if is_hosted_http {
            hosted_http_service_count += 1;
        }
        if is_deterministic {
            deterministic_service_count += 1;
        }
        let route_host = service.route.as_ref().map(|route| route.host.clone());
        let route_path_prefix = service
            .route
            .as_ref()
            .map(|route| route.path_prefix.clone());
        let route_visibility = service
            .route
            .as_ref()
            .map(|route| format!("{:?}", route.visibility));
        let service_workspace_dir = app_service_workspace_dir(&container_path, &service_path);
        let service_workspace_scripts =
            app_service_workspace_scripts(service_workspace_dir.as_deref());
        services.push(AppLocalServicePlanOutput {
            service_name: service.service_name.to_string(),
            container_manifest_path: container_path.to_string_lossy().into_owned(),
            service_manifest_path: service_path.to_string_lossy().into_owned(),
            workspace_dir: service_workspace_dir
                .unwrap_or_else(|| manifest_dir.clone())
                .to_string_lossy()
                .into_owned(),
            workspace_scripts: service_workspace_scripts,
            execution_plane: format!("{:?}", service.execution_plane),
            runtime: format!("{:?}", container.runtime),
            route_host: route_host.clone(),
            route_path_prefix: route_path_prefix.clone(),
            route_visibility,
            replica_count: service.replicas.get(),
            state_binding_count: u32::try_from(service.state_bindings.len())
                .expect("service state binding count fits in u32"),
            lease_volume_count: u32::try_from(service.lease_volumes.len())
                .expect("service lease volume count fits in u32"),
            handler_count: u32::try_from(service.handlers.len())
                .expect("service handler count fits in u32"),
        });
        if let Some(route) = service.route.as_ref() {
            routes.push(AppLocalRoutePlanOutput {
                service_name: service.service_name.to_string(),
                route_kind: if is_hosted_http {
                    "hosted_http_prefix".to_owned()
                } else {
                    "service_prefix".to_owned()
                },
                host: route.host.clone(),
                path: route.path_prefix.clone(),
                handler_name: None,
                handler_class: None,
                certified_response: None,
                mailbox_queue: None,
            });
            for handler in &service.handlers {
                if let Some(handler_path) = handler.route_path.as_deref() {
                    routes.push(AppLocalRoutePlanOutput {
                        service_name: service.service_name.to_string(),
                        route_kind: "handler".to_owned(),
                        host: route.host.clone(),
                        path: join_service_route_path(&route.path_prefix, handler_path),
                        handler_name: Some(handler.handler_name.to_string()),
                        handler_class: Some(format!("{:?}", handler.class)),
                        certified_response: Some(format!("{:?}", handler.certified_response)),
                        mailbox_queue: handler
                            .mailbox
                            .as_ref()
                            .map(|mailbox| mailbox.queue_name.to_string()),
                    });
                }
            }
        }
    }
    let has_mixed_planes = hosted_http_service_count > 0 && deterministic_service_count > 0;
    let mut notes = Vec::new();
    if has_mixed_planes {
        notes.push(
            "mixed app plan includes both hosted HttpService + Inrou and deterministic IVM services"
                .to_owned(),
        );
    }
    if let Some(frontend) = frontend.as_ref()
        && frontend.publish_mode == APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY
    {
        notes.push(
            "frontend is CID-only; Torii root remains unbound and the published build is expected under /sorafs/cid/<cid>"
                .to_owned(),
        );
    }
    Ok(AppLocalPlanOutput {
        app_name: manifest.app_name,
        manifest_path: root.manifest_path,
        public_url: root.public_url,
        hostname: root.hostname,
        has_mixed_planes,
        hosted_http_service_count,
        deterministic_service_count,
        frontend,
        workspace_dir: root.workspace_dir,
        workspace_scripts: root.workspace_scripts,
        services,
        routes,
        notes,
    })
}
macro_rules! define_live_mutation_args {
    (
        $url_doc:literal;
        $(#[$meta:meta])*
        pub struct $name:ident { $($fields:tt)* }
    ) => {
        $(#[$meta])*
        #[derive(clap::Args, Debug)]
        pub struct $name {
            $($fields)*
            #[doc = $url_doc]
            #[arg(long, value_name = "URL")]
            torii_url: Option<String>,
            /// Optional API token sent as `x-api-token` when mutating live control-plane APIs.
            #[arg(long, value_name = "TOKEN")]
            api_token: Option<String>,
            /// HTTP timeout for live control-plane mutations.
            #[arg(long, value_name = "SECS", default_value_t = 10)]
            timeout_secs: u64,
        }
    };
}
macro_rules! post_live_mutation {
    ($args:expr, $torii_url:expr, $path:expr, $request:expr) => {
        post_torii_soracloud_mutation(
            $torii_url,
            $path,
            $request,
            $args.api_token.as_deref(),
            $args.timeout_secs,
        )
    };
}
define_torii_args! {
    " Torii base URL to execute deploy against authoritative control-plane APIs.",
    " Optional API token sent as `x-api-token` when mutating live control-plane APIs.",
    " Positive timeout for each Torii request; the durable Inrou qualification is reread before online side effects.";
    /// Arguments for `soracloud service deploy`.
    pub struct DeployArgs {
        /// Path to an unpublished Soracloud container workspace JSON document.
        #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
        container: PathBuf,
        /// Path to a `SoraServiceManifestV1` JSON document.
        #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
        service: PathBuf,
        /// Canonical service bundle bytes to publish before submitting the deployment.
        #[arg(long, value_name = "PATH")]
        bundle_file: PathBuf,
        /// Exact Unix-second retention boundary embedded in every SoraFS manifest in this release.
        /// Reuse the same value for every retry; it must remain ahead of consensus time.
        #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
        sorafs_retention_epoch: NonZeroU64,
        /// Optional JSON file containing a map of inline config values committed atomically with deploy.
        #[arg(long, value_name = "PATH")]
        initial_configs: Option<PathBuf>,
        /// Optional JSON file containing a map of inline secret envelopes committed atomically with deploy.
        #[arg(long, value_name = "PATH")]
        initial_secrets: Option<PathBuf>,
        /// Absolute owner-only ingest qualification produced by `soracloud service preseed`.
        #[arg(long = "inrou-preseed-receipt", value_name = "PATH")]
        inrou_preseed_receipt: Option<PathBuf>,
    }
}
macro_rules! impl_service_bundle_mutation {
    ($args:ty) => {
        impl $args {
            fn run(
                self,
                mode: MutationMode,
                authority: &AccountId,
                key_pair: &KeyPair,
            ) -> Result<ServiceMutationOutput> {
                let release_identity = SorafsReleaseIdentityV1::new(self.sorafs_retention_epoch);
                let plan = build_service_workspace_plan(&self.container, &self.service)?;
                let container: UnpublishedContainerManifestV1 = load_json(&self.container)?;
                let service: SoraServiceManifestV1 = load_json(&self.service)?;
                let mut bundle = UnpublishedDeploymentBundleV1 { container, service };
                let workspace_dir = direct_service_artifact_workspace_dir(
                    &self.container,
                    &self.service,
                    &self.bundle_file,
                );
                validate_inrou_preseed_artifact_count([&bundle])?;
                let preseed_qualification = load_inrou_preseed_qualification(
                    required_inrou_preseed_store_count([&bundle]),
                    self.inrou_preseed_receipt.as_deref(),
                )?;
                if let Some(qualification) = preseed_qualification.as_ref() {
                    bind_exact_inrou_placement_targets(
                        &mut bundle.service,
                        &qualification.placement_targets(),
                    )?;
                }
                validate_unpublished_deployment_source(&bundle)?;
                validate_local_inrou_guest_image_sources(workspace_dir.as_deref(), &bundle)?;
                let mut initial_service_configs =
                    load_initial_service_configs(self.initial_configs.as_deref())?;
                let initial_service_secrets =
                    load_initial_service_secrets(self.initial_secrets.as_deref())?;
                let torii_url = require_torii_url(self.torii_url.as_deref())?.to_owned();
                let service_name = bundle.service.service_name.to_string();
                let (_, preflight_status) = fetch_torii_soracloud_status(
                    &torii_url,
                    Some(&service_name),
                    self.api_token.as_deref(),
                    self.timeout_secs,
                )?;
                let precondition = derive_service_mutation_precondition(
                    &preflight_status,
                    &service_name,
                    &bundle.service.service_version,
                    mode,
                    "Soracloud service",
                )?;
                preflight_service_upgrade_identity(
                    &preflight_status,
                    &bundle.service,
                    bundle.container.runtime,
                    mode,
                    "Soracloud service",
                )?;
                let prepared_artifacts = prepare_service_artifacts(
                    &self.bundle_file,
                    workspace_dir.as_deref(),
                    bundle,
                    key_pair,
                    release_identity,
                )?;
                let prepared_public_discovery = prepare_public_service_discovery_config(
                    &prepared_artifacts.admitted_bundle,
                    &initial_service_configs,
                    &torii_url,
                    self.api_token.as_deref(),
                    self.timeout_secs,
                    key_pair,
                    release_identity,
                )?;
                let mut inrou_artifacts = Vec::new();
                if preseed_qualification.is_some() {
                    inrou_artifacts.extend(prepared_artifacts.artifacts.iter());
                    if let Some(discovery) = prepared_public_discovery.as_ref() {
                        inrou_artifacts.push(&discovery.artifact);
                    }
                }
                let inrou_artifacts = distinct_prepared_sorafs_artifacts(inrou_artifacts);
                if let Some(qualification) = preseed_qualification.as_ref() {
                    qualification.require_exact_artifacts(&inrou_artifacts)?;
                    qualification.revalidate()?;
                    let (_, current_host_status) = fetch_torii_soracloud_status(
                        &torii_url,
                        Some(&service_name),
                        self.api_token.as_deref(),
                        self.timeout_secs,
                    )?;
                    let (_, current_host_status) =
                        decode_network_control_plane_snapshot(&current_host_status)?;
                    require_active_inrou_qualification_targets(
                        qualification,
                        &current_host_status.active_inrou_hosts,
                    )?;
                }
                register_prepared_sorafs_artifacts(
                    prepared_artifacts.artifacts.iter(),
                    &torii_url,
                    authority,
                    key_pair,
                    self.timeout_secs,
                )?;
                if let Some(discovery) = prepared_public_discovery.as_ref() {
                    register_prepared_sorafs_artifact(
                        &discovery.artifact,
                        &torii_url,
                        authority,
                        key_pair,
                        self.timeout_secs,
                    )?;
                }
                let PreparedServiceArtifacts {
                    admitted_bundle: bundle,
                    published_bundle,
                    inrou_guest_images,
                    artifacts: _,
                } = prepared_artifacts;
                let published_public_discovery = attach_prepared_public_service_discovery_config(
                    &mut initial_service_configs,
                    prepared_public_discovery,
                )?;
                if let Some(qualification) = preseed_qualification.as_ref() {
                    qualification.revalidate()?;
                    let (_, current_status) = fetch_torii_soracloud_status(
                        &torii_url,
                        Some(&service_name),
                        self.api_token.as_deref(),
                        self.timeout_secs,
                    )?;
                    let (_, current_status) =
                        decode_network_control_plane_snapshot(&current_status)?;
                    require_active_inrou_qualification_targets(
                        qualification,
                        &current_status.active_inrou_hosts,
                    )?;
                }
                let response = run_service_bundle_mutation(
                    mode,
                    bundle,
                    initial_service_configs,
                    initial_service_secrets,
                    precondition,
                    &torii_url,
                    self.api_token.as_deref(),
                    self.timeout_secs,
                    authority,
                    key_pair,
                )?;
                Ok(build_direct_service_mutation_output(
                    &self.container,
                    &self.service,
                    plan,
                    mode,
                    &torii_url,
                    self.api_token.is_some(),
                    published_public_discovery,
                    published_bundle,
                    inrou_guest_images,
                    response,
                ))
            }
        }
    };
}
impl_service_bundle_mutation!(DeployArgs);
define_torii_args! {
    " Torii base URL to execute upgrade against authoritative control-plane APIs.",
    " Optional API token sent as `x-api-token` when mutating live control-plane APIs.",
    " Positive timeout for each Torii request; the durable Inrou qualification is reread before online side effects.";
    /// Arguments for `soracloud service upgrade`.
    pub struct UpgradeArgs {
        /// Path to an unpublished Soracloud container workspace JSON document.
        #[arg(long, value_name = "PATH", default_value = DEFAULT_CONTAINER_MANIFEST)]
        container: PathBuf,
        /// Path to a `SoraServiceManifestV1` JSON document.
        #[arg(long, value_name = "PATH", default_value = DEFAULT_SERVICE_MANIFEST)]
        service: PathBuf,
        /// Canonical service bundle bytes to publish before submitting the upgrade.
        #[arg(long, value_name = "PATH")]
        bundle_file: PathBuf,
        /// Exact Unix-second retention boundary embedded in every SoraFS manifest in this release.
        /// Reuse the same value for every retry; it must remain ahead of consensus time.
        #[arg(long = "sorafs-retention-epoch", value_name = "UNIX_SECONDS")]
        sorafs_retention_epoch: NonZeroU64,
        /// Optional JSON file containing a map of inline config values committed atomically with upgrade.
        #[arg(long, value_name = "PATH")]
        initial_configs: Option<PathBuf>,
        /// Optional JSON file containing a map of inline secret envelopes committed atomically with upgrade.
        #[arg(long, value_name = "PATH")]
        initial_secrets: Option<PathBuf>,
        /// Absolute owner-only ingest qualification produced by `soracloud service preseed`.
        #[arg(long = "inrou-preseed-receipt", value_name = "PATH")]
        inrou_preseed_receipt: Option<PathBuf>,
    }
}
impl_service_bundle_mutation!(UpgradeArgs);
/// Arguments for `soracloud service status`.
#[derive(clap::Args, Debug)]
pub struct StatusArgs {
    /// Optional service name filter.
    #[arg(long, value_name = "NAME")]
    service_name: Option<String>,
    /// Optional unpublished Soracloud container workspace used to resolve the service filter.
    #[arg(long, value_name = "PATH")]
    container: Option<PathBuf>,
    /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service filter.
    #[arg(long, value_name = "PATH")]
    service: Option<PathBuf>,
    /// Torii base URL (for example `http://127.0.0.1:8080/`) to query
    /// `/v1/soracloud/status` from the authoritative control plane.
    #[arg(long, value_name = "URL")]
    torii_url: Option<String>,
    /// Optional API token sent as `x-api-token` when querying Torii.
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    /// HTTP timeout for Torii status requests.
    #[arg(long, value_name = "SECS", default_value_t = 10)]
    timeout_secs: u64,
}
impl StatusArgs {
    fn run(self) -> Result<StatusOutput> {
        let service_plan = match (self.container.as_deref(), self.service.as_deref()) {
            (Some(container_manifest), Some(service_manifest)) => Some(
                build_service_local_plan_output(container_manifest, service_manifest)?,
            ),
            _ => None,
        };
        let service_filter = resolve_optional_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (endpoint, payload) = fetch_torii_soracloud_status(
            torii_url,
            service_filter.as_deref(),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        StatusOutput::from_network(endpoint, payload, service_plan)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `service/config/set`.";
    /// Arguments for `iroha soracloud service config-set`.
    pub struct ConfigSetArgs {
        /// Service name owning the config entry.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Stable service-scoped config name.
        #[arg(long, value_name = "NAME")]
        config_name: String,
        /// Inline JSON value for the config entry.
        #[arg(long, value_name = "JSON")]
        value_json: Option<String>,
        /// Path to a JSON document used as the config value.
        #[arg(long, value_name = "PATH")]
        value_file: Option<PathBuf>,
    }
}
impl ConfigSetArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service config-set",
        )?;
        let value_json =
            load_service_config_value(self.value_json.as_deref(), self.value_file.as_deref())?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_service_config_set_request(
            &service_name,
            &self.config_name,
            value_json,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/service/config/set", &request)?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `service/config/delete`.";
    /// Arguments for `iroha soracloud service config-delete`.
    pub struct ConfigDeleteArgs {
        /// Service name owning the config entry.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Stable service-scoped config name.
        #[arg(long, value_name = "NAME")]
        config_name: String,
    }
}
impl ConfigDeleteArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service config-delete",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_service_config_delete_request(
            &service_name,
            &self.config_name,
            authority,
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/service/config/delete",
            &request
        )?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_torii_args! {
    " Torii base URL for authoritative `service/config/status`.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane queries.";
    /// Arguments for `soracloud service config-status`.
    pub struct ConfigStatusArgs {
        /// Service name owning the config entries.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Optional config name filter.
        #[arg(long, value_name = "NAME")]
        config_name: Option<String>,
    }
}
impl ConfigStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service config-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_service_config_status(
            torii_url,
            &service_name,
            self.config_name.as_deref(),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `service/secret/set`.";
    /// Arguments for `iroha soracloud service secret-set`.
    pub struct SecretSetArgs {
        /// Service name owning the secret entry.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Stable service-scoped secret name.
        #[arg(long, value_name = "NAME")]
        secret_name: String,
        /// Path to a `SecretEnvelopeV1` JSON document.
        #[arg(long, value_name = "PATH")]
        secret_file: PathBuf,
    }
}
impl SecretSetArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service secret-set",
        )?;
        let secret: SecretEnvelopeV1 = load_json(&self.secret_file)?;
        secret.validate().wrap_err("invalid secret envelope")?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_service_secret_set_request(
            &service_name,
            &self.secret_name,
            secret,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/service/secret/set", &request)?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `service/secret/delete`.";
    /// Arguments for `iroha soracloud service secret-delete`.
    pub struct SecretDeleteArgs {
        /// Service name owning the secret entry.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Stable service-scoped secret name.
        #[arg(long, value_name = "NAME")]
        secret_name: String,
    }
}
impl SecretDeleteArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service secret-delete",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_service_secret_delete_request(
            &service_name,
            &self.secret_name,
            authority,
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/service/secret/delete",
            &request
        )?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_torii_args! {
    " Torii base URL for authoritative `service/secret/status`.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane queries.";
    /// Arguments for `soracloud service secret-status`.
    pub struct SecretStatusArgs {
        /// Service name owning the secret entries.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Optional secret name filter.
        #[arg(long, value_name = "NAME")]
        secret_name: Option<String>,
    }
}
impl SecretStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service secret-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_service_secret_status(
            torii_url,
            &service_name,
            self.secret_name.as_deref(),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut payload = payload;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_torii_args! {
    " Torii base URL to execute rollback against authoritative control-plane APIs.",
    " Optional API token sent as `x-api-token` when mutating live control-plane APIs.",
    " HTTP timeout for Torii mutation requests.";
    /// Arguments for `soracloud service rollback`.
    pub struct RollbackArgs {
        /// Service name to roll back.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Already-admitted target version to restore.
        #[arg(long, value_name = "VERSION")]
        target_version: String,
    }
}
impl RollbackArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan = match (self.container.as_deref(), self.service.as_deref()) {
            (Some(container_manifest), Some(service_manifest)) => Some(
                build_service_local_plan_output(container_manifest, service_manifest)?,
            ),
            _ => None,
        };
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service rollback",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_rollback_request(
            &service_name,
            &self.target_version,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(self, torii_url, "v1/soracloud/rollback", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_status(
            torii_url,
            Some(&service_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output =
            build_service_mutation_output(payload, &status_payload, &service_name, "Rollback")?;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL to execute a deterministic IVM rollout against authoritative control-plane APIs.",
    " Optional API token sent as `x-api-token` when mutating live control-plane APIs.",
    " HTTP timeout for Torii mutation requests.";
    /// Arguments for `soracloud service rollout` on deterministic IVM services.
    pub struct RolloutArgs {
        /// Deterministic IVM service name with an active rollout.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished deterministic IVM workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to its `SoraServiceManifestV1` JSON document.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Rollout handle emitted by `upgrade` output (`rollout_handle`).
        #[arg(long, value_name = "HANDLE")]
        rollout_handle: String,
        /// Health signal for this rollout step.
        #[arg(long, value_enum, default_value_t = RolloutHealth::Healthy)]
        health: RolloutHealth,
        /// Explicit target traffic percentage; required for healthy steps and forbidden otherwise.
        #[arg(long, value_name = "PERCENT")]
        promote_to_percent: Option<u8>,
        /// Governance transaction hash linked to this rollout action.
        #[arg(long, value_name = "HASH")]
        governance_tx_hash: Hash,
    }
}
impl RolloutArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan = match (self.container.as_deref(), self.service.as_deref()) {
            (Some(container_manifest), Some(service_manifest)) => Some(
                build_service_local_plan_output(container_manifest, service_manifest)?,
            ),
            _ => None,
        };
        if service_plan
            .as_ref()
            .is_some_and(|plan| plan.execution_plane == "HttpService" && plan.runtime == "Inrou")
        {
            return Err(eyre!(
                "first-release HttpService + Inrou services use atomic exact-revision upgrades; `iroha soracloud service rollout` does not accept Inrou manifest pairs"
            ));
        }
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud service rollout",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_rollout_request(
            &service_name,
            &self.rollout_handle,
            self.health.is_healthy(),
            self.promote_to_percent,
            self.governance_tx_hash,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(self, torii_url, "v1/soracloud/rollout", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_status(
            torii_url,
            Some(&service_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output =
            build_service_mutation_output(payload, &status_payload, &service_name, "Rollout")?;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/deploy`.";
    /// Arguments for `iroha soracloud agent deploy`.
    pub struct AgentDeployArgs {
        /// Path to an `AgentApartmentManifestV1` JSON document.
        #[arg(long, value_name = "PATH", default_value = DEFAULT_AGENT_APARTMENT_MANIFEST)]
        manifest: PathBuf,
        /// Lease length, measured in deterministic control-plane sequence ticks.
        #[arg(long, value_name = "TICKS", default_value_t = 120)]
        lease_ticks: u64,
        /// Initial autonomy execution budget units.
        #[arg(long, value_name = "UNITS", default_value_t = AGENT_AUTONOMY_DEFAULT_BUDGET_UNITS)]
        autonomy_budget_units: u64,
    }
}
impl AgentDeployArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.lease_ticks == 0 {
            return Err(eyre!("--lease-ticks must be greater than zero"));
        }
        if self.autonomy_budget_units == 0 {
            return Err(eyre!("--autonomy-budget-units must be greater than zero"));
        }
        let manifest: AgentApartmentManifestV1 = load_json(&self.manifest)?;
        manifest.validate()?;
        let apartment_name = manifest.apartment_name.to_string();
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_agent_deploy_request(
            manifest,
            self.lease_ticks,
            self.autonomy_budget_units,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/deploy", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(payload, &status_payload, &apartment_name, "Deploy")
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/lease/renew`.";
    /// Arguments for `iroha soracloud agent lease-renew`.
    pub struct AgentLeaseRenewArgs {
        /// Apartment name to renew.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Lease extension ticks.
        #[arg(long, value_name = "TICKS", default_value_t = 120)]
        lease_ticks: u64,
    }
}
impl AgentLeaseRenewArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        if self.lease_ticks == 0 {
            return Err(eyre!("--lease-ticks must be greater than zero"));
        }
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_agent_lease_renew_request(
            &self.apartment_name,
            self.lease_ticks,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/lease/renew", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(payload, &status_payload, &self.apartment_name, "LeaseRenew")
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/restart`.";
    /// Arguments for `iroha soracloud agent restart`.
    pub struct AgentRestartArgs {
        /// Apartment name to restart.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Human-readable reason captured in scheduler events.
        #[arg(long, value_name = "TEXT")]
        reason: String,
    }
}
impl AgentRestartArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request =
            signed_agent_restart_request(&self.apartment_name, &self.reason, authority, key_pair)?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/restart", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(payload, &status_payload, &self.apartment_name, "Restart")
    }
}
define_torii_args! {
    " Torii base URL for authoritative `agent/status`.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane status query.";
    /// Arguments for `iroha soracloud agent status`.
    pub struct AgentStatusArgs {
        /// Optional apartment name filter.
        #[arg(long, value_name = "NAME")]
        apartment_name: Option<String>,
    }
}
impl AgentStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            self.apartment_name.as_deref(),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/wallet/spend`.";
    /// Arguments for `iroha soracloud agent wallet-spend`.
    pub struct AgentWalletSpendArgs {
        /// Apartment name issuing the spend request.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Caller-selected unique wallet request identifier committed by the signed V1 request.
        #[arg(long, value_name = "REQUEST", value_parser = parse_agent_wallet_request_id)]
        request_id: String,
        /// Asset definition identifier (canonical unprefixed Base58 address).
        #[arg(long, value_name = "ASSET", value_parser = parse_agent_wallet_asset_definition)]
        asset_definition: String,
        /// Exact, positive spend amount.
        #[arg(long, value_name = "QUANTITY", value_parser = parse_positive_quantity)]
        amount: Quantity,
    }
}
impl AgentWalletSpendArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let asset_definition = parse_agent_wallet_asset_definition(&self.asset_definition)
            .map_err(|error| eyre!("invalid --asset-definition: {error}"))?;
        let request = signed_agent_wallet_spend_request(
            &self.apartment_name,
            &self.request_id,
            &asset_definition,
            &self.amount,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/wallet/spend", &request)?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_wallet_spend_output(
            payload,
            &status_payload,
            &self.apartment_name,
            &self.request_id,
        )
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/wallet/approve`.";
    /// Arguments for `iroha soracloud agent wallet-approve`.
    pub struct AgentWalletApproveArgs {
        /// Apartment name owning the request.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Caller-selected wallet request identifier supplied to the original `agent wallet-spend`.
        #[arg(long, value_name = "REQUEST", value_parser = parse_agent_wallet_request_id)]
        request_id: String,
    }
}
impl AgentWalletApproveArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_agent_wallet_approve_request(
            &self.apartment_name,
            &self.request_id,
            authority,
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/agent/wallet/approve",
            &request
        )?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(
            payload,
            &status_payload,
            &self.apartment_name,
            "WalletSpendApproved",
        )
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/policy/revoke`.";
    /// Arguments for `iroha soracloud agent policy-revoke`.
    pub struct AgentPolicyRevokeArgs {
        /// Apartment name whose policy should be updated.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Capability identifier to revoke (for example `wallet.sign`).
        #[arg(long, value_name = "CAPABILITY")]
        capability: String,
        /// Optional reason included in audit events.
        #[arg(long, value_name = "TEXT")]
        reason: Option<String>,
    }
}
impl AgentPolicyRevokeArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_agent_policy_revoke_request(
            &self.apartment_name,
            &self.capability,
            self.reason.as_deref(),
            authority,
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/agent/policy/revoke",
            &request
        )?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(
            payload,
            &status_payload,
            &self.apartment_name,
            "PolicyRevoked",
        )
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/message/send`.";
    /// Arguments for `iroha soracloud agent message-send`.
    pub struct AgentMessageSendArgs {
        /// Sender apartment name.
        #[arg(long, value_name = "NAME")]
        from_apartment: String,
        /// Recipient apartment name.
        #[arg(long, value_name = "NAME")]
        to_apartment: String,
        /// Logical mailbox channel.
        #[arg(long, value_name = "CHANNEL", default_value = "default")]
        channel: String,
        /// Message payload (UTF-8 text).
        #[arg(long, value_name = "TEXT")]
        payload: String,
    }
}
impl AgentMessageSendArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, mailbox_status_before) = fetch_torii_soracloud_agent_mailbox_status(
            torii_url,
            &self.to_apartment,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let known_message_ids = mailbox_message_ids(&mailbox_status_before)?;
        let request = signed_agent_message_send_request(
            &self.from_apartment,
            &self.to_apartment,
            &self.channel,
            &self.payload,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/message/send", &request)?;
        let (_, mailbox_status_payload) = fetch_torii_soracloud_agent_mailbox_status(
            torii_url,
            &self.to_apartment,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_message_send_output(
            payload,
            &mailbox_status_payload,
            &known_message_ids,
            &self.from_apartment,
            &self.channel,
            &self.payload,
        )
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/message/ack`.";
    /// Arguments for `iroha soracloud agent message-ack`.
    pub struct AgentMessageAckArgs {
        /// Apartment name consuming the message.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Message identifier emitted by `agent message-send`.
        #[arg(long, value_name = "MESSAGE")]
        message_id: String,
    }
}
impl AgentMessageAckArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, mailbox_status_before) = fetch_torii_soracloud_agent_mailbox_status(
            torii_url,
            &self.apartment_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        mailbox_message_by_id(&mailbox_status_before, &self.message_id)?;
        let request = signed_agent_message_ack_request(
            &self.apartment_name,
            &self.message_id,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/agent/message/ack", &request)?;
        let (_, mailbox_status_payload) = fetch_torii_soracloud_agent_mailbox_status(
            torii_url,
            &self.apartment_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_message_ack_output(
            payload,
            &mailbox_status_before,
            &mailbox_status_payload,
            &self.message_id,
        )
    }
}
define_torii_args! {
    " Torii base URL for authoritative `agent/mailbox/status`.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane status query.";
    /// Arguments for `iroha soracloud agent mailbox-status`.
    pub struct AgentMailboxStatusArgs {
        /// Apartment name to inspect.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
    }
}
impl AgentMailboxStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_agent_mailbox_status(
            torii_url,
            &self.apartment_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `agent/autonomy/allow`.";
    /// Arguments for `iroha soracloud agent artifact-allow`.
    pub struct AgentArtifactAllowArgs {
        /// Apartment name whose allowlist should be updated.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
        /// Artifact hash identifier.
        #[arg(long, value_name = "HASH")]
        artifact_hash: String,
        /// Optional provenance hash required for this artifact.
        #[arg(long, value_name = "HASH")]
        provenance_hash: Option<String>,
    }
}
impl AgentArtifactAllowArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_agent_artifact_allow_request(
            &self.apartment_name,
            &self.artifact_hash,
            self.provenance_hash.as_deref(),
            authority,
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/agent/autonomy/allow",
            &request
        )?;
        let (_, status_payload) = fetch_torii_soracloud_agent_status(
            torii_url,
            Some(&self.apartment_name),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        build_agent_mutation_output(
            payload,
            &status_payload,
            &self.apartment_name,
            "ArtifactAllowed",
        )
    }
}
define_torii_args! {
    " Torii base URL for authoritative `agent/autonomy/status`.",
    " Optional API token sent as `x-api-token` when querying Torii.",
    " HTTP timeout for live control-plane query.";
    /// Arguments for `iroha soracloud agent autonomy-status`.
    pub struct AgentAutonomyStatusArgs {
        /// Apartment name to inspect.
        #[arg(long, value_name = "NAME")]
        apartment_name: String,
    }
}
impl AgentAutonomyStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_agent_autonomy_status(
            torii_url,
            &self.apartment_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        Ok(payload)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model training-job-start`.
    pub struct TrainingJobStartArgs {
        /// Service name that owns the training job.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name for the training job.
        #[arg(long, value_name = "NAME")]
        model_name: String,
        /// Deterministic training job identifier.
        #[arg(long, value_name = "ID")]
        job_id: String,
        /// Worker-group size for the distributed training run.
        #[arg(long, value_name = "COUNT", default_value_t = 1)]
        worker_group_size: u16,
        /// Target number of steps to complete the training job.
        #[arg(long, value_name = "STEPS")]
        target_steps: u32,
        #[doc = concat!(" St", "ep cadence for checkpoint creation.")]
        #[arg(long, value_name = "STEPS")]
        checkpoint_interval_steps: u32,
        /// Maximum allowed retries for the training job.
        #[arg(long, value_name = "COUNT", default_value_t = 3)]
        max_retries: u8,
        /// Compute units charged per step.
        #[arg(long, value_name = "UNITS")]
        step_compute_units: u64,
        /// Total compute budget units for the training job.
        #[arg(long, value_name = "UNITS")]
        compute_budget_units: u64,
        /// Total storage budget bytes for checkpoints.
        #[arg(long, value_name = "BYTES")]
        storage_budget_bytes: u64,
    }
}
impl TrainingJobStartArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        if self.worker_group_size == 0 {
            return Err(eyre!("--worker-group-size must be greater than zero"));
        }
        if self.target_steps == 0 {
            return Err(eyre!("--target-steps must be greater than zero"));
        }
        if self.checkpoint_interval_steps == 0 {
            return Err(eyre!(
                "--checkpoint-interval-steps must be greater than zero"
            ));
        }
        if self.step_compute_units == 0 {
            return Err(eyre!("--step-compute-units must be greater than zero"));
        }
        if self.compute_budget_units == 0 {
            return Err(eyre!("--compute-budget-units must be greater than zero"));
        }
        if self.storage_budget_bytes == 0 {
            return Err(eyre!("--storage-budget-bytes must be greater than zero"));
        }
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model training-job-start",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_start_request(
            &service_name,
            &self.model_name,
            &self.job_id,
            self.worker_group_size,
            self.target_steps,
            self.checkpoint_interval_steps,
            self.max_retries,
            self.step_compute_units,
            self.compute_budget_units,
            self.storage_budget_bytes,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/training/job/start", &request)?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model training-job-checkpoint`.
    pub struct TrainingJobCheckpointArgs {
        /// Service name that owns the training job.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Training job identifier.
        #[arg(long, value_name = "ID")]
        job_id: String,
        /// Completed step represented by this checkpoint.
        #[arg(long, value_name = "STEP")]
        completed_step: u32,
        /// Checkpoint payload size in bytes.
        #[arg(long, value_name = "BYTES")]
        checkpoint_size_bytes: u64,
        /// Hash of metrics/telemetry emitted for this checkpoint.
        #[arg(long, value_name = "HASH")]
        metrics_hash: Hash,
    }
}
impl TrainingJobCheckpointArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        if self.completed_step == 0 {
            return Err(eyre!("--completed-step must be greater than zero"));
        }
        if self.checkpoint_size_bytes == 0 {
            return Err(eyre!("--checkpoint-size-bytes must be greater than zero"));
        }
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model training-job-checkpoint",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_checkpoint_request(
            &service_name,
            &self.job_id,
            self.completed_step,
            self.checkpoint_size_bytes,
            self.metrics_hash,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/training/job/checkpoint",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model training-job-retry`.
    pub struct TrainingJobRetryArgs {
        /// Service name that owns the training job.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Training job identifier.
        #[arg(long, value_name = "ID")]
        job_id: String,
        /// Human-readable retry reason recorded in audit logs.
        #[arg(long, value_name = "TEXT")]
        reason: String,
    }
}
impl TrainingJobRetryArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model training-job-retry",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_training_job_retry_request(
            &service_name,
            &self.job_id,
            &self.reason,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/training/job/retry", &request)?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane query.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane query.";
    /// Arguments for `iroha soracloud model training-job-status`.
    pub struct TrainingJobStatusArgs {
        /// Service name that owns the training job.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Training job identifier.
        #[arg(long, value_name = "ID")]
        job_id: String,
    }
}
impl TrainingJobStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model training-job-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_training_job_status(
            torii_url,
            &service_name,
            &self.job_id,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model artifact-register`.
    pub struct ModelArtifactRegisterArgs {
        /// Service name that owns the model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name.
        #[arg(long, value_name = "NAME")]
        model_name: String,
        /// Training job identifier backing this artifact registration.
        #[arg(long, value_name = "ID")]
        training_job_id: String,
        /// Weight artifact hash.
        #[arg(long, value_name = "HASH")]
        weight_artifact_hash: Hash,
        /// Dataset reference identifier.
        #[arg(long, value_name = "REF")]
        dataset_ref: String,
        /// Hash of training config used for the run.
        #[arg(long, value_name = "HASH")]
        training_config_hash: Hash,
        /// Reproducibility metadata hash.
        #[arg(long, value_name = "HASH")]
        reproducibility_hash: Hash,
        /// Provenance attestation hash.
        #[arg(long, value_name = "HASH")]
        provenance_attestation_hash: Hash,
    }
}
impl ModelArtifactRegisterArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model artifact-register",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_artifact_register_request(
            &service_name,
            &self.model_name,
            &self.training_job_id,
            self.weight_artifact_hash,
            &self.dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/model/artifact/register",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane query.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane query.";
    /// Arguments for `iroha soracloud model artifact-status`.
    pub struct ModelArtifactStatusArgs {
        /// Service name that owns the model artifact.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Training job identifier associated with the artifact.
        #[arg(long, value_name = "ID")]
        training_job_id: String,
    }
}
impl ModelArtifactStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model artifact-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_model_artifact_status(
            torii_url,
            &service_name,
            &self.training_job_id,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model weight-register`.
    pub struct ModelWeightRegisterArgs {
        /// Service name that owns the model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name.
        #[arg(long, value_name = "NAME")]
        model_name: String,
        /// New weight version identifier.
        #[arg(long, value_name = "VERSION")]
        weight_version: String,
        /// Training job identifier backing this weight version.
        #[arg(long, value_name = "ID")]
        training_job_id: String,
        /// Optional lineage parent version.
        #[arg(long, value_name = "VERSION")]
        parent_version: Option<String>,
        /// Weight artifact hash.
        #[arg(long, value_name = "HASH")]
        weight_artifact_hash: Hash,
        /// Dataset reference identifier.
        #[arg(long, value_name = "REF")]
        dataset_ref: String,
        /// Hash of training config used for the run.
        #[arg(long, value_name = "HASH")]
        training_config_hash: Hash,
        /// Reproducibility metadata hash.
        #[arg(long, value_name = "HASH")]
        reproducibility_hash: Hash,
        /// Provenance attestation hash.
        #[arg(long, value_name = "HASH")]
        provenance_attestation_hash: Hash,
    }
}
impl ModelWeightRegisterArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model weight-register",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_register_request(
            &service_name,
            &self.model_name,
            &self.weight_version,
            &self.training_job_id,
            self.parent_version.as_deref(),
            self.weight_artifact_hash,
            &self.dataset_ref,
            self.training_config_hash,
            self.reproducibility_hash,
            self.provenance_attestation_hash,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/model/weight/register",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model weight-promote`.
    pub struct ModelWeightPromoteArgs {
        /// Service name that owns the model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name.
        #[arg(long, value_name = "NAME")]
        model_name: String,
        /// Weight version to promote.
        #[arg(long, value_name = "VERSION")]
        weight_version: String,
        /// Gate approval flag.
        #[arg(long)]
        gate_approved: bool,
        /// Hash of gate report/evidence for this promotion decision.
        #[arg(long, value_name = "HASH")]
        gate_report_hash: Hash,
    }
}
impl ModelWeightPromoteArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model weight-promote",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_promote_request(
            &service_name,
            &self.model_name,
            &self.weight_version,
            self.gate_approved,
            self.gate_report_hash,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/model/weight/promote",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane mutation.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model weight-rollback`.
    pub struct ModelWeightRollbackArgs {
        /// Service name that owns the model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name.
        #[arg(long, value_name = "NAME")]
        model_name: String,
        /// Target version to roll back to.
        #[arg(long, value_name = "VERSION")]
        target_version: String,
        /// Human-readable rollback reason.
        #[arg(long, value_name = "TEXT")]
        reason: String,
    }
}
impl ModelWeightRollbackArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model weight-rollback",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_model_weight_rollback_request(
            &service_name,
            &self.model_name,
            &self.target_version,
            &self.reason,
            Some(authority),
            key_pair,
        )?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/model/weight/rollback",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for live control-plane query.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane query.";
    /// Arguments for `iroha soracloud model weight-status`.
    pub struct ModelWeightStatusArgs {
        /// Service name that owns the model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Model name.
        #[arg(long, value_name = "NAME")]
        model_name: String,
    }
}
impl ModelWeightStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model weight-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_model_weight_status(
            torii_url,
            &service_name,
            &self.model_name,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for authoritative `model/upload/register`.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane mutation.";
    /// Arguments for `iroha soracloud model upload-register`.
    pub struct ModelUploadRegisterArgs {
        /// Path to a `SoraUploadedModelBundleV1` JSON document with an approved SoraFS digest.
        #[arg(long, value_name = "PATH")]
        bundle_file: PathBuf,
        /// Path to an `UploadedModelFinalizePayload` JSON document describing registry metadata.
        #[arg(long, value_name = "PATH")]
        request_file: PathBuf,
        /// Service name that owns the uploaded model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
    }
}
impl ModelUploadRegisterArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let resolved_service_name = resolve_optional_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model upload-register",
        )?;
        let mut bundle: SoraUploadedModelBundleV1 = load_json(&self.bundle_file)?;
        let mut finalize: UploadedModelFinalizePayload = load_json(&self.request_file)?;
        apply_uploaded_model_register_service_name_override(
            &mut bundle,
            &mut finalize,
            resolved_service_name.as_deref(),
        )?;
        let request =
            signed_uploaded_model_register_request(bundle, finalize, authority, key_pair)?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = post_live_mutation!(
            self,
            torii_url,
            "v1/soracloud/model/upload/register",
            &request
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
fn apply_uploaded_model_register_service_name_override(
    bundle: &mut SoraUploadedModelBundleV1,
    finalize: &mut UploadedModelFinalizePayload,
    service_name: Option<&str>,
) -> Result<()> {
    let Some(service_name) = service_name else {
        return Ok(());
    };
    let service_name = parse_exact_name_arg("resolved service name", service_name)?;
    bundle.service_name = service_name.parse().wrap_err_with(|| {
        format!("resolved service name `{service_name}` is not a valid Soracloud service name")
    })?;
    finalize.service_name = service_name;
    Ok(())
}
define_torii_args! {
    " Torii base URL for authoritative `model/upload/status`.",
    " Optional API token sent as `x-api-token`.",
    " HTTP timeout for live control-plane query.";
    /// Arguments for `iroha soracloud model upload-status`.
    pub struct ModelUploadStatusArgs {
        /// Service name that owns the uploaded model.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Uploaded-model pinned weight version.
        #[arg(long, value_name = "VERSION")]
        weight_version: String,
        /// Optional uploaded-model identifier.
        #[arg(long, value_name = "ID", conflicts_with = "model_name")]
        model_id: Option<String>,
        /// Optional logical model name used to resolve the uploaded-model record.
        #[arg(long, value_name = "NAME", conflicts_with = "model_id")]
        model_name: Option<String>,
        /// Optional bundle-root filter.
        #[arg(long, value_name = "HASH")]
        bundle_root: Option<Hash>,
    }
}
impl ModelUploadStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud model upload-status",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, payload) = fetch_torii_soracloud_uploaded_model_status(
            torii_url,
            "v1/soracloud/model/upload/status",
            &service_name,
            &self.weight_version,
            self.model_id.as_deref(),
            self.model_name.as_deref(),
            self.bundle_root,
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = payload;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `hf/lease/join`.";
    /// Arguments for `iroha soracloud hf join`.
    pub struct HfSharedLeaseJoinArgs {
        /// Hugging Face repository identifier (for example `openai/gpt-oss`).
        #[arg(long, value_name = "REPO")]
        repo_id: String,
        /// Full 40-character lowercase Hugging Face commit OID.
        #[arg(long, value_name = "COMMIT_OID")]
        revision: String,
        /// Soracloud service name recorded as inert lease-membership metadata.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Optional agent apartment name recorded as inert lease-membership metadata.
        #[arg(long, value_name = "NAME")]
        apartment_name: Option<String>,
        /// Shared-lease storage tier.
        #[arg(long, value_enum)]
        storage_class: HfStorageClassArg,
        /// Shared-lease window length in milliseconds.
        #[arg(long, value_name = "MS")]
        lease_term_ms: u64,
        /// Settlement asset definition identifier.
        #[arg(long, value_name = "ASSET")]
        lease_asset_definition: String,
        /// Exact, positive base lease fee in the settlement asset.
        #[arg(long, value_name = "QUANTITY", value_parser = parse_positive_quantity)]
        base_fee: Quantity,
    }
}
impl HfSharedLeaseJoinArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud hf join",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_hf_shared_lease_join_request(
            &self.repo_id,
            &self.revision,
            &service_name,
            self.apartment_name.as_deref(),
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            &self.lease_asset_definition,
            &self.base_fee,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/hf/lease/join", &request)?;
        let account_id = authority.to_string();
        let (_, status_payload) = fetch_torii_soracloud_hf_status(
            torii_url,
            &self.repo_id,
            &self.revision,
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            Some(account_id.as_str()),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = build_hf_mutation_output(payload, &status_payload, "Join")?;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_torii_args! {
    " Torii base URL for authoritative `hf/lease/status`.",
    " Optional API token sent as `x-api-token` when querying live control-plane APIs.",
    " HTTP timeout for live control-plane queries.";
    /// Arguments for `iroha soracloud hf status`.
    pub struct HfStatusArgs {
        /// Hugging Face repository identifier (for example `openai/gpt-oss`).
        #[arg(long, value_name = "REPO")]
        repo_id: String,
        /// Full 40-character lowercase Hugging Face commit OID.
        #[arg(long, value_name = "COMMIT_OID")]
        revision: String,
        /// Shared-lease storage tier.
        #[arg(long, value_enum)]
        storage_class: HfStorageClassArg,
        /// Shared-lease window length in milliseconds.
        #[arg(long, value_name = "MS")]
        lease_term_ms: u64,
        /// Optional account filter for membership-specific status.
        #[arg(long, value_name = "ACCOUNT")]
        account_id: Option<String>,
        /// Optional unpublished Soracloud container workspace used to project the local service plan.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to project the local service plan.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
    }
}
impl HfStatusArgs {
    fn run(self) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let (_, mut payload) = fetch_torii_soracloud_hf_status(
            torii_url,
            &self.repo_id,
            &self.revision,
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            self.account_id.as_deref(),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        attach_service_plan_to_output(&mut payload, service_plan)?;
        Ok(payload)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `hf/lease/leave`.";
    /// Arguments for `iroha soracloud hf lease-leave`.
    pub struct HfLeaseLeaveArgs {
        /// Hugging Face repository identifier.
        #[arg(long, value_name = "REPO")]
        repo_id: String,
        /// Full 40-character lowercase Hugging Face commit OID.
        #[arg(long, value_name = "COMMIT_OID")]
        revision: String,
        /// Shared-lease storage tier.
        #[arg(long, value_enum)]
        storage_class: HfStorageClassArg,
        /// Shared-lease window length in milliseconds.
        #[arg(long, value_name = "MS")]
        lease_term_ms: u64,
        /// Optional inert service association to include in the signed leave request.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Optional inert apartment association to include in the signed leave request.
        #[arg(long, value_name = "NAME")]
        apartment_name: Option<String>,
    }
}
impl HfLeaseLeaveArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_optional_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud hf lease-leave",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_hf_lease_leave_request(
            &self.repo_id,
            &self.revision,
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            service_name.as_deref(),
            self.apartment_name.as_deref(),
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/hf/lease/leave", &request)?;
        let account_id = authority.to_string();
        let (_, status_payload) = fetch_torii_soracloud_hf_status(
            torii_url,
            &self.repo_id,
            &self.revision,
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            Some(account_id.as_str()),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = build_hf_mutation_output(payload, &status_payload, "LeaseLeave")?;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
define_live_mutation_args! {
    " Torii base URL for authoritative `hf/lease/renew`.";
    /// Arguments for `iroha soracloud hf lease-renew`.
    pub struct HfLeaseRenewArgs {
        /// Hugging Face repository identifier.
        #[arg(long, value_name = "REPO")]
        repo_id: String,
        /// Full 40-character lowercase Hugging Face commit OID.
        #[arg(long, value_name = "COMMIT_OID")]
        revision: String,
        /// Soracloud service name recorded as inert renewed-membership metadata.
        #[arg(long, value_name = "NAME")]
        service_name: Option<String>,
        /// Optional unpublished Soracloud container workspace used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        container: Option<PathBuf>,
        /// Optional path to a `SoraServiceManifestV1` JSON document used to resolve the service name.
        #[arg(long, value_name = "PATH")]
        service: Option<PathBuf>,
        /// Optional agent apartment name recorded as inert renewed-membership metadata.
        #[arg(long, value_name = "NAME")]
        apartment_name: Option<String>,
        /// Shared-lease storage tier.
        #[arg(long, value_enum)]
        storage_class: HfStorageClassArg,
        /// Shared-lease window length in milliseconds.
        #[arg(long, value_name = "MS")]
        lease_term_ms: u64,
        /// Settlement asset definition identifier.
        #[arg(long, value_name = "ASSET")]
        lease_asset_definition: String,
        /// Exact, positive base lease fee in the settlement asset.
        #[arg(long, value_name = "QUANTITY", value_parser = parse_positive_quantity)]
        base_fee: Quantity,
    }
}
impl HfLeaseRenewArgs {
    fn run(self, authority: &AccountId, key_pair: &KeyPair) -> Result<norito::json::Value> {
        let service_plan =
            maybe_service_local_plan(self.container.as_deref(), self.service.as_deref())?;
        let service_name = resolve_required_workspace_service_name(
            self.service_name,
            self.container.as_deref(),
            self.service.as_deref(),
            "iroha soracloud hf lease-renew",
        )?;
        let torii_url = require_torii_url(self.torii_url.as_deref())?;
        let request = signed_hf_lease_renew_request(
            &self.repo_id,
            &self.revision,
            &service_name,
            self.apartment_name.as_deref(),
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            &self.lease_asset_definition,
            &self.base_fee,
            authority,
            key_pair,
        )?;
        let (_, payload) =
            post_live_mutation!(self, torii_url, "v1/soracloud/hf/lease/renew", &request)?;
        let account_id = authority.to_string();
        let (_, status_payload) = fetch_torii_soracloud_hf_status(
            torii_url,
            &self.repo_id,
            &self.revision,
            self.storage_class.to_storage_class(),
            self.lease_term_ms,
            Some(account_id.as_str()),
            self.api_token.as_deref(),
            self.timeout_secs,
        )?;
        let mut output = build_hf_mutation_output(payload, &status_payload, "LeaseRenew")?;
        attach_service_plan_to_output(&mut output, service_plan)?;
        Ok(output)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MutationMode {
    Deploy,
    Upgrade,
}
impl MutationMode {
    const fn label_lowercase(self) -> &'static str {
        match self {
            Self::Deploy => "deploy",
            Self::Upgrade => "upgrade",
        }
    }
    const fn workspace_script_name(self) -> &'static str {
        match self {
            Self::Deploy => "deploy.sh",
            Self::Upgrade => "upgrade.sh",
        }
    }
}
impl From<crate::taira::InrouCanaryMode> for MutationMode {
    fn from(mode: crate::taira::InrouCanaryMode) -> Self {
        match mode {
            crate::taira::InrouCanaryMode::Deploy => Self::Deploy,
            crate::taira::InrouCanaryMode::Upgrade => Self::Upgrade,
        }
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TairaInrouStageReceiptV1 {
    pub schema_version: u16,
    pub sorafs_retention_epoch: u64,
    pub placement_targets: BTreeSet<SoraInrouPlacementTargetV1>,
    pub mutation_mode: String,
    pub service_name: String,
    pub service_version: String,
    pub container_file: String,
    pub service_file: String,
    pub bundle_payload_file: String,
    pub bundle_manifest_file: String,
    pub bundle_hash: String,
    pub bundle_content_cid: String,
    pub bundle_manifest_digest_hex: String,
    pub guest_isa: String,
    pub guest_payload_dir: String,
    pub guest_manifest_file: String,
    pub guest_content_cid: String,
    pub guest_manifest_digest_hex: String,
    pub discovery_payload_dir: String,
    pub discovery_manifest_file: String,
    pub discovery_document_hash: String,
    pub discovery_content_cid: String,
    pub discovery_manifest_digest_hex: String,
    pub public_discovery_url: String,
    pub public_discovery_cid_host_url: String,
    pub container_manifest_hash: String,
    pub service_manifest_hash: String,
}
/// Stable report describing one generated canonical Taira Inrou workspace.
#[derive(Clone, Debug, JsonSerialize)]
pub(crate) struct TairaInrouWorkspaceReceiptV1 {
    schema_version: u16,
    container_file: String,
    service_file: String,
    bundle_file: String,
    kernel_file: String,
    rootfs_file: String,
    initrd_file: String,
    bundle_hash: String,
    container_manifest_hash: String,
    service_manifest_hash: String,
    guest_total_bytes: u64,
}
struct VerifiedTairaInrouStage {
    receipt: TairaInrouStageReceiptV1,
    bundle: SoraDeploymentBundleV1,
    bundle_manifest: BuiltSorafsManifest,
    guest_manifest: BuiltSorafsManifest,
    discovery: SoracloudPublicServiceDiscoveryV1,
    discovery_manifest: BuiltSorafsManifest,
}
fn canonical_taira_inrou_canary_bundle_payload() -> Result<Vec<u8>> {
    write_gzip_ustar(
        Vec::new(),
        &[BundleArchiveFile::new(
            TAIRA_INROU_CANARY_BUNDLE_MEMBER_V1,
            0o755,
            TAIRA_INROU_CANARY_SERVER_SOURCE_V1,
        )],
    )
    .wrap_err("encode the canonical Taira Inrou V1 server archive")
}
fn canonical_taira_inrou_canary_deploy_bundle() -> Result<(UnpublishedDeploymentBundleV1, Vec<u8>)>
{
    let mut container = UnpublishedContainerManifestV1::from_non_inrou_manifest(
        json::from_str(TAIRA_INROU_CANARY_CONTAINER_TEMPLATE_V1)
            .wrap_err("decode the embedded Taira Inrou container template")?,
    )?;
    let mut service: SoraServiceManifestV1 = json::from_str(TAIRA_INROU_CANARY_SERVICE_TEMPLATE_V1)
        .wrap_err("decode the embedded Taira Inrou service template")?;
    let service_name: Name = TAIRA_INROU_CANARY_SERVICE_NAME_V1
        .parse()
        .wrap_err("parse the canonical Taira Inrou service name")?;
    apply_init_template_defaults(
        InitTemplate::HttpService,
        &service_name,
        &mut service,
        &mut container,
    )?;
    let bundle_payload = canonical_taira_inrou_canary_bundle_payload()?;
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.bundle_hash = Hash::new(&bundle_payload);
    container.bundle_path = TAIRA_INROU_CANARY_ENTRYPOINT_V1.to_owned();
    container.entrypoint = TAIRA_INROU_CANARY_ENTRYPOINT_V1.to_owned();
    container.args.clear();
    container.env.clear();
    container.env.insert(
        TAIRA_INROU_CANARY_HTTP_SERVICE_ENV_V1.to_owned(),
        TAIRA_INROU_CANARY_SERVICE_NAME_V1.to_owned(),
    );
    container.required_config_names.clear();
    container.required_secret_names.clear();
    container.config_exports.clear();
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    container.capabilities.allow_state_writes = false;
    container.capabilities.allow_model_inference = false;
    container.capabilities.allow_model_training = false;
    container.resources.cpu_millis =
        NonZeroU32::new(TAIRA_INROU_CANARY_CPU_MILLIS_V1).expect("nonzero CPU budget");
    container.resources.memory_bytes =
        NonZeroU64::new(TAIRA_INROU_CANARY_MEMORY_BYTES_V1).expect("nonzero memory budget");
    container.resources.ephemeral_storage_bytes =
        NonZeroU64::new(TAIRA_INROU_CANARY_EPHEMERAL_STORAGE_BYTES_V1)
            .expect("nonzero ephemeral budget");
    container.resources.max_open_files_per_process =
        NonZeroU32::new(TAIRA_INROU_CANARY_MAX_OPEN_FILES_PER_PROCESS_V1)
            .expect("nonzero per-process file budget");
    container.resources.max_tasks =
        NonZeroU16::new(TAIRA_INROU_CANARY_MAX_TASKS_V1).expect("nonzero task budget");
    container.lifecycle.healthcheck_path = Some(TAIRA_INROU_CANARY_HEALTHCHECK_V1.to_owned());
    let inrou = container
        .inrou
        .as_mut()
        .ok_or_else(|| eyre!("canonical HTTP service template did not select Inrou"))?;
    inrou
        .guest_images
        .retain(|guest_isa, _| guest_isa == "aarch64");
    let aarch64 = inrou
        .guest_images
        .get_mut("aarch64")
        .ok_or_else(|| eyre!("canonical Inrou template is missing its AArch64 guest image"))?;
    aarch64.kernel_image_path = TAIRA_INROU_CANARY_KERNEL_PATH_V1.to_owned();
    aarch64.rootfs_image_path = TAIRA_INROU_CANARY_ROOTFS_PATH_V1.to_owned();
    aarch64.initrd_image_path = Some(TAIRA_INROU_CANARY_INITRD_PATH_V1.to_owned());
    aarch64.published_artifact = ();
    service.service_name = service_name;
    // The unpublished workspace does not claim a deployable revision. Staging
    // replaces this sentinel with the digest of the admitted artifact graph.
    service.service_version = "staging-placeholder".to_owned();
    service.rollout.canary_percent = 100;
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.replicas = NonZeroU16::new(4).expect("four replicas");
    service.route = Some(SoraRouteTargetV1 {
        host: TAIRA_INROU_CANARY_ROUTE_HOST_V1.to_owned(),
        path_prefix: TAIRA_INROU_CANARY_ROUTE_PREFIX_V1.to_owned(),
        service_port: NonZeroU16::new(TAIRA_INROU_CANARY_SERVICE_PORT_V1)
            .expect("nonzero service port"),
        visibility: SoraRouteVisibilityV1::Public,
        tls_mode: SoraTlsModeV1::Required,
    });
    service.state_bindings.clear();
    service.handlers.clear();
    service.artifacts.clear();
    service.lease_volumes = vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("root volume name"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1)
                .expect("nonzero root volume"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "app_data".parse().expect("shared volume name"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/app_data".to_owned(),
            max_total_bytes: NonZeroU64::new(TAIRA_INROU_CANARY_SHARED_VOLUME_BYTES_V1)
                .expect("nonzero shared volume"),
        },
    ];
    service.container.manifest_hash = container.workspace_hash()?;
    service.container.expected_schema_version = container.schema_version;
    let bundle = UnpublishedDeploymentBundleV1 { container, service };
    validate_unpublished_deployment_source(&bundle)?;
    validate_taira_inrou_canary_bundle_payload(&bundle_payload)?;
    Ok((bundle, bundle_payload))
}
fn validate_taira_inrou_canary_bundle_payload(payload: &[u8]) -> Result<()> {
    let canonical = canonical_taira_inrou_canary_bundle_payload()?;
    if payload != canonical {
        return Err(eyre!(
            "Taira Inrou canary requires the exact deterministic gzip/USTAR archive containing the canonical `{TAIRA_INROU_CANARY_BUNDLE_MEMBER_V1}` source at mode 0755 (expected hash {}, found {})",
            Hash::new(&canonical),
            Hash::new(payload),
        ));
    }
    Ok(())
}
fn validate_taira_inrou_canary_container(container: &SoraContainerManifestV1) -> Result<()> {
    if container.bundle_path != TAIRA_INROU_CANARY_ENTRYPOINT_V1
        || container.entrypoint != TAIRA_INROU_CANARY_ENTRYPOINT_V1
        || !container.args.is_empty()
    {
        return Err(eyre!(
            "Taira Inrou canary requires the exact argument-free `{TAIRA_INROU_CANARY_ENTRYPOINT_V1}` bundle entrypoint"
        ));
    }
    let expected_env = BTreeMap::from([(
        TAIRA_INROU_CANARY_HTTP_SERVICE_ENV_V1.to_owned(),
        TAIRA_INROU_CANARY_SERVICE_NAME_V1.to_owned(),
    )]);
    if container.env != expected_env {
        return Err(eyre!(
            "Taira Inrou canary requires exactly `{TAIRA_INROU_CANARY_HTTP_SERVICE_ENV_V1}={TAIRA_INROU_CANARY_SERVICE_NAME_V1}`; the runtime owns `PORT`, `SORACLOUD_REPLICA_SLOT`, and `SORACLOUD_SERVICE_VERSION`"
        ));
    }
    if !container.required_config_names.is_empty()
        || !container.required_secret_names.is_empty()
        || !container.config_exports.is_empty()
    {
        return Err(eyre!(
            "Taira Inrou canary must not depend on external config or secret material"
        ));
    }
    let capabilities = &container.capabilities;
    if capabilities.network != SoraNetworkPolicyV1::Isolated
        || capabilities.allow_state_writes
        || capabilities.allow_model_inference
        || capabilities.allow_model_training
    {
        return Err(eyre!(
            "Taira Inrou canary requires the exact isolated, capability-free policy"
        ));
    }
    let resources = container.resources;
    if resources.cpu_millis.get() != TAIRA_INROU_CANARY_CPU_MILLIS_V1
        || resources.memory_bytes.get() != TAIRA_INROU_CANARY_MEMORY_BYTES_V1
        || resources.ephemeral_storage_bytes.get() != TAIRA_INROU_CANARY_EPHEMERAL_STORAGE_BYTES_V1
        || resources.max_open_files_per_process.get()
            != TAIRA_INROU_CANARY_MAX_OPEN_FILES_PER_PROCESS_V1
        || resources.max_tasks.get() != TAIRA_INROU_CANARY_MAX_TASKS_V1
    {
        return Err(eyre!(
            "Taira Inrou canary resource request must match the configured canonical CPU, memory, temporary storage, open-file, and task limits"
        ));
    }
    if container.lifecycle.healthcheck_path.as_deref() != Some(TAIRA_INROU_CANARY_HEALTHCHECK_V1) {
        return Err(eyre!(
            "Taira Inrou canary healthcheck path must be exactly `{TAIRA_INROU_CANARY_HEALTHCHECK_V1}`"
        ));
    }
    let inrou = container
        .inrou
        .as_ref()
        .ok_or_else(|| eyre!("Taira Inrou canary container is missing its Inrou manifest"))?;
    if inrou.guest_images.len() != 1
        || !inrou
            .guest_images
            .contains_key(&SoraInrouGuestIsaV1::Aarch64)
    {
        return Err(eyre!(
            "Taira Inrou canary requires exactly one AArch64 guest-image profile"
        ));
    }
    let aarch64 = &inrou.guest_images[&SoraInrouGuestIsaV1::Aarch64];
    if aarch64.kernel_image_path != TAIRA_INROU_CANARY_KERNEL_PATH_V1
        || aarch64.rootfs_image_path != TAIRA_INROU_CANARY_ROOTFS_PATH_V1
        || aarch64.initrd_image_path.as_deref() != Some(TAIRA_INROU_CANARY_INITRD_PATH_V1)
    {
        return Err(eyre!(
            "Taira Inrou canary requires the exact AArch64 kernel, rootfs, and initrd paths"
        ));
    }
    Ok(())
}
fn validate_taira_inrou_canary_service(service: &SoraServiceManifestV1) -> Result<()> {
    if service.service_name.as_ref() != TAIRA_INROU_CANARY_SERVICE_NAME_V1 {
        return Err(eyre!(
            "Taira Inrou canary requires canonical service identity `{TAIRA_INROU_CANARY_SERVICE_NAME_V1}`"
        ));
    }
    if service.replicas.get() != 4 {
        return Err(eyre!(
            "Taira Inrou canary requires exactly four replicas, found {}",
            service.replicas
        ));
    }
    let route = service
        .route
        .as_ref()
        .ok_or_else(|| eyre!("Taira Inrou canary requires one canonical public route"))?;
    if route.host != TAIRA_INROU_CANARY_ROUTE_HOST_V1
        || route.path_prefix != TAIRA_INROU_CANARY_ROUTE_PREFIX_V1
        || route.service_port.get() != TAIRA_INROU_CANARY_SERVICE_PORT_V1
        || route.visibility != SoraRouteVisibilityV1::Public
        || route.tls_mode != SoraTlsModeV1::Required
    {
        return Err(eyre!(
            "Taira Inrou canary requires canonical public TLS route `https://{TAIRA_INROU_CANARY_ROUTE_HOST_V1}{TAIRA_INROU_CANARY_ROUTE_PREFIX_V1}` on port {TAIRA_INROU_CANARY_SERVICE_PORT_V1}"
        ));
    }
    if !service.state_bindings.is_empty()
        || !service.handlers.is_empty()
        || !service.artifacts.is_empty()
    {
        return Err(eyre!(
            "Taira Inrou canary must not declare deterministic bindings, handlers, or artifacts"
        ));
    }
    if service.rollout.canary_percent != 100 {
        return Err(eyre!(
            "Taira Inrou canary requires rollout.canary_percent = 100 so upgrades promote atomically without doubling four-replica placement demand"
        ));
    }
    Ok(())
}

pub(crate) fn is_taira_inrou_canary_service_version(value: &str) -> bool {
    let Some(digest) = value.strip_prefix(TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1) else {
        return false;
    };
    digest
        .parse::<Hash>()
        .is_ok_and(|hash| hash.to_string() == digest)
}

fn derive_taira_inrou_canary_service_version(bundle: &SoraDeploymentBundleV1) -> Result<String> {
    let mut revision_seed = bundle.clone();
    revision_seed.service.service_version.clear();
    let revision_digest = Hash::new(
        json::to_vec(&revision_seed)
            .wrap_err("encode the canonical Taira Inrou revision identity")?,
    );
    Ok(format!(
        "{TAIRA_INROU_CANARY_SERVICE_VERSION_PREFIX_V1}{revision_digest}"
    ))
}

fn install_taira_inrou_canary_service_version(bundle: &mut SoraDeploymentBundleV1) -> Result<()> {
    bundle.service.service_version = derive_taira_inrou_canary_service_version(bundle)?;
    Ok(())
}
fn validate_taira_inrou_canary_storage(
    resources: &SoraResourceLimitsV1,
    service: &SoraServiceManifestV1,
) -> Result<()> {
    let [root, shared] = service.lease_volumes.as_slice() else {
        return Err(eyre!(
            "Taira Inrou canary requires exactly one root volume and one shared service volume"
        ));
    };
    if root.volume_name.as_ref() != "root_disk"
        || root.kind != SoraLeaseVolumeKindV1::PersistentRootLeaseVolume
        || root.storage_class != StorageClass::Warm
        || root.mount_path != "/"
        || root.max_total_bytes.get() != TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1
        || shared.volume_name.as_ref() != "app_data"
        || shared.kind != SoraLeaseVolumeKindV1::ServiceLeaseVolume
        || shared.storage_class != StorageClass::Warm
        || shared.mount_path != "/var/lib/soracloud/volumes/app_data"
        || shared.max_total_bytes.get() != TAIRA_INROU_CANARY_SHARED_VOLUME_BYTES_V1
    {
        return Err(eyre!(
            "Taira Inrou canary requires canonical root ({TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1} bytes) and shared service-volume ({TAIRA_INROU_CANARY_SHARED_VOLUME_BYTES_V1} bytes) geometry"
        ));
    }
    let per_host_storage_bytes = resources
        .ephemeral_storage_bytes
        .get()
        .checked_add(root.max_total_bytes.get())
        .ok_or_else(|| eyre!("Taira Inrou canary per-host storage geometry overflow"))?;
    // Host-local admission charges the root and temporary filesystem. The
    // complete writable budget additionally includes the shared app-data lease.
    let writable_storage_bytes = per_host_storage_bytes
        .checked_add(shared.max_total_bytes.get())
        .ok_or_else(|| eyre!("Taira Inrou canary writable storage geometry overflow"))?;
    if per_host_storage_bytes != TAIRA_INROU_CANARY_HOST_STORAGE_BYTES_V1
        || writable_storage_bytes != defaults::taira::INROU_MAX_STORAGE_BYTES
    {
        return Err(eyre!(
            "Taira Inrou canary storage geometry requires exactly {TAIRA_INROU_CANARY_HOST_STORAGE_BYTES_V1} host-local bytes and {} total writable bytes",
            defaults::taira::INROU_MAX_STORAGE_BYTES,
        ));
    }
    Ok(())
}
fn validate_taira_inrou_canary_bundle(bundle: &SoraDeploymentBundleV1) -> Result<()> {
    bundle.validate_for_admission()?;
    if bundle.container.runtime != SoraContainerRuntimeV1::Inrou
        || bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
    {
        return Err(eyre!(
            "Taira Inrou canary requires the canonical HttpService + Inrou execution plane"
        ));
    }
    validate_taira_inrou_canary_container(&bundle.container)?;
    validate_taira_inrou_canary_service(&bundle.service)?;
    validate_taira_inrou_canary_storage(&bundle.container.resources, &bundle.service)?;
    let expected_service_version = derive_taira_inrou_canary_service_version(bundle)?;
    if bundle.service.service_version != expected_service_version {
        return Err(eyre!(
            "Taira Inrou canary service version must be the exact artifact-derived revision `{expected_service_version}`"
        ));
    }
    Ok(())
}
fn validate_taira_inrou_canary_source_bundle(bundle: &UnpublishedDeploymentBundleV1) -> Result<()> {
    validate_unpublished_deployment_source(bundle)?;
    validate_taira_inrou_canary_storage(&bundle.container.resources, &bundle.service)?;
    let (canonical, _) = canonical_taira_inrou_canary_deploy_bundle()?;
    if bundle != &canonical {
        return Err(eyre!(
            "Taira Inrou staging requires the exact canonical deploy container and service manifests"
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn set_taira_stage_permissions(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .wrap_err_with(|| format!("set owner-only permissions on {}", path.display()))
}
#[cfg(not(unix))]
fn set_taira_stage_permissions(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
fn create_taira_owner_only_directory(path: &Path, description: &str) -> Result<()> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(eyre!("{description} must name one concrete directory"));
    }
    validate_taira_path_ancestors(path, description)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(path).wrap_err_with(|| {
        format!(
            "create fresh {description} {}; existing directories are never reused",
            path.display()
        )
    })?;
    set_taira_stage_permissions(path, 0o700)
}
fn create_taira_stage_directory(path: &Path) -> Result<()> {
    create_taira_owner_only_directory(path, "Taira Inrou stage directory")
}
fn validate_taira_path_ancestors(path: &Path, description: &str) -> Result<()> {
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(eyre!(
            "{description} path {} must not contain parent traversal",
            path.display()
        ));
    }
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(eyre!(
                        "{description} ancestor {} must not be a symbolic link",
                        ancestor.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(eyre!(
                        "{description} ancestor {} must be a directory",
                        ancestor.display()
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).wrap_err_with(|| {
                    format!("inspect {description} ancestor {}", ancestor.display())
                });
            }
        }
    }
    Ok(())
}
fn create_taira_owner_only_subdirectory(path: &Path, description: &str) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .wrap_err_with(|| format!("create {description} {}", path.display()))?;
    set_taira_stage_permissions(path, 0o700)
}
fn create_taira_stage_subdirectory(path: &Path) -> Result<()> {
    create_taira_owner_only_subdirectory(path, "Taira Inrou stage directory")
}
fn create_taira_stage_member_parent(root: &Path, logical_path: &[String]) -> Result<PathBuf> {
    let mut current = root.to_path_buf();
    for component in logical_path
        .iter()
        .take(logical_path.len().saturating_sub(1))
    {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(eyre!(
                        "Taira guest staging component {} must be one direct directory",
                        current.display()
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current).wrap_err_with(|| {
                    format!("create Taira guest staging directory {}", current.display())
                })?;
            }
            Err(error) => {
                return Err(error).wrap_err_with(|| {
                    format!(
                        "inspect Taira guest staging directory {}",
                        current.display()
                    )
                });
            }
        }
        set_taira_stage_permissions(&current, 0o700)?;
    }
    Ok(current)
}
fn write_taira_stage_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .wrap_err_with(|| format!("create Taira Inrou staged file {}", path.display()))?;
    file.write_all(bytes)
        .wrap_err_with(|| format!("write Taira Inrou staged file {}", path.display()))?;
    file.sync_all()
        .wrap_err_with(|| format!("synchronize Taira Inrou staged file {}", path.display()))?;
    set_taira_stage_permissions(path, 0o600)
}
#[cfg(unix)]
fn open_taira_stage_source(path: &Path) -> io::Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )?;
    Ok(fs::File::from(descriptor))
}
#[cfg(not(unix))]
fn open_taira_stage_source(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TairaFileIdentity {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
    mode: u32,
    uid: u32,
    nlink: u64,
}
#[cfg(unix)]
fn taira_metadata_identity(metadata: &fs::Metadata) -> TairaFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    TairaFileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        len: metadata.len(),
        mtime: metadata.mtime(),
        mtime_nsec: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_nsec: metadata.ctime_nsec(),
        mode: metadata.mode(),
        uid: metadata.uid(),
        nlink: metadata.nlink(),
    }
}
#[cfg(not(unix))]
fn taira_metadata_identity(metadata: &fs::Metadata) -> (u64, Option<SystemTime>) {
    (metadata.len(), metadata.modified().ok())
}
fn copy_taira_stage_source_file(
    source_path: &Path,
    destination_path: &Path,
    captured: &fs::Metadata,
) -> Result<u64> {
    validate_taira_path_ancestors(source_path, "Taira guest-image source member")?;
    let mut source = open_taira_stage_source(source_path)
        .wrap_err_with(|| format!("open Taira guest-image source {}", source_path.display()))?;
    let opened = source.metadata().wrap_err_with(|| {
        format!(
            "inspect opened Taira guest-image source {}",
            source_path.display()
        )
    })?;
    if !opened.is_file() || taira_metadata_identity(&opened) != taira_metadata_identity(captured) {
        return Err(eyre!(
            "Taira guest-image source {} changed before descriptor-bound copy",
            source_path.display()
        ));
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut destination = options.open(destination_path).wrap_err_with(|| {
        format!(
            "create Taira guest-image staged file {}",
            destination_path.display()
        )
    })?;
    let copy_limit = captured
        .len()
        .checked_add(1)
        .ok_or_else(|| eyre!("Taira guest-image source length overflow"))?;
    let mut bounded_source = io::Read::take(&mut source, copy_limit);
    let copied = io::copy(&mut bounded_source, &mut destination).wrap_err_with(|| {
        format!(
            "copy Taira guest-image source {} to {}",
            source_path.display(),
            destination_path.display()
        )
    })?;
    drop(bounded_source);
    destination.sync_all().wrap_err_with(|| {
        format!(
            "synchronize Taira guest-image staged file {}",
            destination_path.display()
        )
    })?;
    set_taira_stage_permissions(destination_path, 0o600)?;
    let opened_after = source.metadata().wrap_err_with(|| {
        format!(
            "reinspect opened Taira guest-image source {}",
            source_path.display()
        )
    })?;
    let named_after = fs::symlink_metadata(source_path).wrap_err_with(|| {
        format!(
            "reinspect named Taira guest-image source {}",
            source_path.display()
        )
    })?;
    validate_taira_path_ancestors(source_path, "Taira guest-image source member")?;
    if taira_metadata_identity(&opened_after) != taira_metadata_identity(captured)
        || taira_metadata_identity(&named_after) != taira_metadata_identity(captured)
        || copied != captured.len()
    {
        return Err(eyre!(
            "Taira guest-image source {} changed during descriptor-bound copy",
            source_path.display()
        ));
    }
    let destination_metadata = fs::symlink_metadata(destination_path).wrap_err_with(|| {
        format!(
            "inspect Taira guest-image staged file {}",
            destination_path.display()
        )
    })?;
    if !destination_metadata.is_file()
        || destination_metadata.file_type().is_symlink()
        || destination_metadata.len() != copied
    {
        return Err(eyre!(
            "Taira guest-image staged file {} does not match the copied source",
            destination_path.display()
        ));
    }
    Ok(copied)
}
fn taira_stage_regular_file_bytes(
    path: &Path,
    description: &str,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    validate_taira_path_ancestors(path, description)?;
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {description} {}", path.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(eyre!(
            "{description} {} must be one direct regular file",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if metadata.nlink() != 1 {
            return Err(eyre!(
                "{description} {} must have exactly one hard link",
                path.display()
            ));
        }
    }
    if metadata.len() == 0 {
        return Err(eyre!("{description} {} must not be empty", path.display()));
    }
    if metadata.len() > max_bytes {
        return Err(eyre!(
            "{description} {} contains {} bytes; maximum is {max_bytes}",
            path.display(),
            metadata.len()
        ));
    }
    let expected_len = usize::try_from(metadata.len()).wrap_err_with(|| {
        format!(
            "{description} {} is too large for this platform",
            path.display()
        )
    })?;
    let captured_identity = taira_metadata_identity(&metadata);
    let mut source = open_taira_stage_source(path).wrap_err_with(|| {
        format!(
            "open {description} {} without following links",
            path.display()
        )
    })?;
    let opened = source
        .metadata()
        .wrap_err_with(|| format!("inspect opened {description} {}", path.display()))?;
    if !opened.is_file() || taira_metadata_identity(&opened) != captured_identity {
        return Err(eyre!(
            "{description} {} changed before descriptor-bound read",
            path.display()
        ));
    }
    let mut bytes = vec![0_u8; expected_len];
    source
        .read_exact(&mut bytes)
        .wrap_err_with(|| format!("read exact {description} bytes from {}", path.display()))?;
    let mut trailing = [0_u8; 1];
    if source
        .read(&mut trailing)
        .wrap_err_with(|| format!("check exact {description} length for {}", path.display()))?
        != 0
    {
        return Err(eyre!(
            "{description} {} changed length during descriptor-bound read",
            path.display()
        ));
    }
    let opened_after = source
        .metadata()
        .wrap_err_with(|| format!("reinspect opened {description} {}", path.display()))?;
    let named_after = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("reinspect named {description} {}", path.display()))?;
    validate_taira_path_ancestors(path, description)?;
    if !opened_after.is_file()
        || !named_after.is_file()
        || named_after.file_type().is_symlink()
        || taira_metadata_identity(&opened_after) != captured_identity
        || taira_metadata_identity(&named_after) != captured_identity
    {
        return Err(eyre!(
            "{description} {} changed during descriptor-bound read",
            path.display()
        ));
    }
    Ok(bytes)
}
fn decode_taira_stage_json<T>(bytes: &[u8], path: &Path) -> Result<T>
where
    T: JsonDeserialize,
{
    let text = std::str::from_utf8(bytes)
        .wrap_err_with(|| format!("decode staged JSON {} as UTF-8", path.display()))?;
    json::from_str(text).wrap_err_with(|| format!("decode staged JSON {}", path.display()))
}
fn taira_direct_member_metadata(
    root: &Path,
    logical_path: &[String],
    description: &str,
) -> Result<fs::Metadata> {
    let root_metadata = fs::symlink_metadata(root)
        .wrap_err_with(|| format!("inspect {description} root {}", root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(eyre!(
            "{description} root {} must be one direct directory",
            root.display()
        ));
    }
    if logical_path.is_empty() {
        return Err(eyre!("{description} path must not be empty"));
    }
    let mut current = root.to_path_buf();
    for (index, component) in logical_path.iter().enumerate() {
        if component.is_empty() || matches!(component.as_str(), "." | "..") {
            return Err(eyre!(
                "{description} path contains non-canonical component `{component}`"
            ));
        }
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .wrap_err_with(|| format!("inspect {description} {}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(eyre!(
                "{description} {} must not contain symbolic links",
                current.display()
            ));
        }
        let is_last = index + 1 == logical_path.len();
        if is_last {
            if !metadata.is_file() {
                return Err(eyre!(
                    "{description} {} must be one direct regular file",
                    current.display()
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt as _;
                if metadata.nlink() != 1 {
                    return Err(eyre!(
                        "{description} {} must have exactly one hard link",
                        current.display()
                    ));
                }
            }
            return Ok(metadata);
        }
        if !metadata.is_dir() {
            return Err(eyre!(
                "{description} intermediate component {} must be one direct directory",
                current.display()
            ));
        }
    }
    Err(eyre!("{description} path did not resolve to a file"))
}
fn taira_stage_logical_member_paths(member_paths: &[String]) -> Result<Vec<Vec<String>>> {
    let mut logical_paths = member_paths
        .iter()
        .map(|path| {
            let components = path.split('/').map(ToOwned::to_owned).collect::<Vec<_>>();
            if components.is_empty()
                || components.iter().any(|component| {
                    component.is_empty() || matches!(component.as_str(), "." | "..")
                })
            {
                return Err(eyre!(
                    "Taira guest-image member `{path}` is not one canonical relative path"
                ));
            }
            Ok(components)
        })
        .collect::<Result<Vec<_>>>()?;
    logical_paths.sort();
    if logical_paths.len() != 3
        || logical_paths
            .windows(2)
            .any(|pair| pair[0] == pair[1] || pair[1].starts_with(&pair[0]))
    {
        return Err(eyre!(
            "Taira guest-image stage requires exactly three distinct non-overlapping member paths"
        ));
    }
    Ok(logical_paths)
}
fn taira_stage_guest_total_bytes(file_sizes: impl IntoIterator<Item = u64>) -> Result<u64> {
    let total = file_sizes.into_iter().try_fold(0_u64, |total, size| {
        if size == 0 {
            return Err(eyre!("Taira guest-image members must not be empty"));
        }
        total
            .checked_add(size)
            .ok_or_else(|| eyre!("Taira guest-image byte length overflow"))
    })?;
    if total > TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1 {
        return Err(eyre!(
            "Taira guest-image stage contains {total} bytes; maximum is {TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1}"
        ));
    }
    Ok(total)
}
fn validate_taira_inrou_rootfs_source_bytes(rootfs_bytes: u64) -> Result<()> {
    if rootfs_bytes > TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1 {
        return Err(eyre!(
            "Taira guest rootfs contains {rootfs_bytes} bytes; the canonical root volume allows at most {TAIRA_INROU_CANARY_ROOT_VOLUME_BYTES_V1}"
        ));
    }
    Ok(())
}
fn taira_inrou_workspace_source_metadata(path: &Path, description: &str) -> Result<fs::Metadata> {
    validate_taira_path_ancestors(path, description)?;
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {description} {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(eyre!(
            "{description} {} must be one direct regular file",
            path.display()
        ));
    }
    if metadata.len() == 0 {
        return Err(eyre!("{description} {} must not be empty", path.display()));
    }
    Ok(metadata)
}
fn validate_taira_inrou_workspace_entry(
    path: &Path,
    directory: bool,
    description: &str,
) -> Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {description} {}", path.display()))?;
    let expected_kind = if directory {
        "directory"
    } else {
        "regular file"
    };
    #[cfg(unix)]
    let expected_mode = if directory { 0o700 } else { 0o600 };
    if metadata.file_type().is_symlink()
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        return Err(eyre!(
            "{description} {} must be one direct {expected_kind}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let effective_uid = rustix::process::geteuid().as_raw();
        if metadata.uid() != effective_uid || metadata.mode() & 0o7777 != expected_mode {
            return Err(eyre!(
                "{description} {} must be owned by uid {effective_uid} with mode {expected_mode:04o}",
                path.display()
            ));
        }
        if !directory && metadata.nlink() != 1 {
            return Err(eyre!(
                "{description} {} must have exactly one hard link",
                path.display()
            ));
        }
    }
    Ok(metadata)
}
fn validate_taira_inrou_workspace_members(directory: &Path, expected: &[&str]) -> Result<()> {
    let actual = fs::read_dir(directory)
        .wrap_err_with(|| format!("read Taira Inrou workspace {}", directory.display()))?
        .map(|entry| {
            let name = entry
                .wrap_err_with(|| {
                    format!(
                        "read Taira Inrou workspace entry in {}",
                        directory.display()
                    )
                })?
                .file_name();
            name.into_string().map_err(|_| {
                eyre!(
                    "Taira Inrou workspace {} contains a non-UTF-8 member",
                    directory.display()
                )
            })
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let expected = expected
        .iter()
        .map(|member| (*member).to_owned())
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(eyre!(
            "Taira Inrou workspace {} must contain exactly {expected:?}, found {actual:?}",
            directory.display()
        ));
    }
    Ok(())
}
fn validate_generated_taira_inrou_workspace(output_dir: &Path) -> Result<()> {
    validate_taira_inrou_workspace_entry(output_dir, true, "Taira Inrou workspace")?;
    let inrou_dir = output_dir.join("inrou");
    let aarch64_dir = inrou_dir.join("aarch64");
    validate_taira_inrou_workspace_entry(&inrou_dir, true, "Taira Inrou workspace directory")?;
    validate_taira_inrou_workspace_entry(&aarch64_dir, true, "Taira Inrou workspace directory")?;
    validate_taira_inrou_workspace_members(
        output_dir,
        &[
            TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1,
            TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1,
            TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1,
            "inrou",
        ],
    )?;
    validate_taira_inrou_workspace_members(&inrou_dir, &["aarch64"])?;
    validate_taira_inrou_workspace_members(
        &aarch64_dir,
        &["vmlinux", "rootfs.ext4", "initrd.img"],
    )?;

    let container_path = output_dir.join(TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1);
    let service_path = output_dir.join(TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1);
    let bundle_path = output_dir.join(TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1);
    let workspace = taira_inrou_source_workspace(&container_path, &service_path, &bundle_path)?;
    let canonical_output = fs::canonicalize(output_dir).wrap_err_with(|| {
        format!(
            "canonicalize generated Taira Inrou workspace {}",
            output_dir.display()
        )
    })?;
    if workspace != canonical_output {
        return Err(eyre!(
            "generated Taira Inrou workspace escaped its requested output directory"
        ));
    }
    for path in [&container_path, &service_path, &bundle_path] {
        validate_taira_inrou_workspace_entry(path, false, "Taira Inrou workspace file")?;
    }
    let container_bytes = taira_stage_regular_file_bytes(
        &container_path,
        "Taira Inrou workspace container manifest",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let service_bytes = taira_stage_regular_file_bytes(
        &service_path,
        "Taira Inrou workspace service manifest",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let container: UnpublishedContainerManifestV1 =
        decode_taira_stage_json(&container_bytes, &container_path)?;
    let service: SoraServiceManifestV1 = decode_taira_stage_json(&service_bytes, &service_path)?;
    let bundle = UnpublishedDeploymentBundleV1 { container, service };
    validate_taira_inrou_canary_source_bundle(&bundle)?;
    let bundle_payload = taira_stage_regular_file_bytes(
        &bundle_path,
        "Taira Inrou workspace bundle",
        INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES,
    )?;
    validate_taira_inrou_canary_bundle_payload(&bundle_payload)?;
    if Hash::new(&bundle_payload) != bundle.container.bundle_hash {
        return Err(eyre!(
            "generated Taira Inrou workspace bundle does not match its container manifest"
        ));
    }

    let image = bundle
        .container
        .inrou
        .as_ref()
        .and_then(|inrou| inrou.guest_images.get("aarch64"))
        .ok_or_else(|| eyre!("generated Taira Inrou workspace lost its AArch64 guest image"))?;
    let rootfs_member = inrou_member_path(&image.rootfs_image_path)?;
    let members = vec![
        inrou_member_path(&image.kernel_image_path)?,
        rootfs_member.clone(),
        inrou_member_path(
            image
                .initrd_image_path
                .as_deref()
                .ok_or_else(|| eyre!("generated Taira Inrou workspace lost its initrd"))?,
        )?,
    ];
    let logical_members = taira_stage_logical_member_paths(&members)?;
    let metadata = logical_members
        .iter()
        .map(|logical_path| {
            let path = inrou_dir.join(logical_path.join("/"));
            validate_taira_inrou_workspace_entry(&path, false, "Taira Inrou workspace guest asset")
        })
        .collect::<Result<Vec<_>>>()?;
    taira_stage_guest_total_bytes(metadata.iter().map(fs::Metadata::len))?;
    let rootfs_logical = rootfs_member
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let rootfs_bytes = logical_members
        .iter()
        .zip(&metadata)
        .find_map(|(logical_path, metadata)| {
            (logical_path == &rootfs_logical).then_some(metadata.len())
        })
        .ok_or_else(|| eyre!("generated Taira Inrou workspace lost its rootfs"))?;
    validate_taira_inrou_rootfs_source_bytes(rootfs_bytes)
}
/// Create one fresh canonical deploy-mode Taira Inrou canary workspace.
pub(crate) fn create_taira_inrou_canary_workspace(
    kernel_path: &Path,
    rootfs_path: &Path,
    initrd_path: &Path,
    output_dir: &Path,
) -> Result<TairaInrouWorkspaceReceiptV1> {
    let kernel_metadata =
        taira_inrou_workspace_source_metadata(kernel_path, "Taira AArch64 kernel source")?;
    let rootfs_metadata =
        taira_inrou_workspace_source_metadata(rootfs_path, "Taira AArch64 rootfs source")?;
    let initrd_metadata =
        taira_inrou_workspace_source_metadata(initrd_path, "Taira AArch64 initrd source")?;
    #[cfg(unix)]
    {
        let identities = [
            taira_metadata_identity(&kernel_metadata),
            taira_metadata_identity(&rootfs_metadata),
            taira_metadata_identity(&initrd_metadata),
        ];
        if identities[0] == identities[1]
            || identities[0] == identities[2]
            || identities[1] == identities[2]
        {
            return Err(eyre!(
                "Taira AArch64 kernel, rootfs, and initrd sources must be three distinct files"
            ));
        }
    }
    let guest_total_bytes = taira_stage_guest_total_bytes([
        kernel_metadata.len(),
        rootfs_metadata.len(),
        initrd_metadata.len(),
    ])?;
    validate_taira_inrou_rootfs_source_bytes(rootfs_metadata.len())?;
    let (bundle, bundle_payload) = canonical_taira_inrou_canary_deploy_bundle()?;

    create_taira_owner_only_directory(output_dir, "Taira Inrou workspace directory")?;
    let result = (|| -> Result<TairaInrouWorkspaceReceiptV1> {
        let inrou_dir = output_dir.join("inrou");
        let aarch64_dir = inrou_dir.join("aarch64");
        create_taira_owner_only_subdirectory(&inrou_dir, "Taira Inrou workspace directory")?;
        create_taira_owner_only_subdirectory(&aarch64_dir, "Taira Inrou workspace directory")?;

        let container_path = output_dir.join(TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1);
        let service_path = output_dir.join(TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1);
        let bundle_path = output_dir.join(TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1);
        write_taira_stage_json(&container_path, &bundle.container)?;
        write_taira_stage_json(&service_path, &bundle.service)?;
        write_taira_stage_file(&bundle_path, &bundle_payload)?;

        for (source, destination, metadata) in [
            (
                kernel_path,
                output_dir.join(TAIRA_INROU_WORKSPACE_KERNEL_FILE_V1),
                &kernel_metadata,
            ),
            (
                rootfs_path,
                output_dir.join(TAIRA_INROU_WORKSPACE_ROOTFS_FILE_V1),
                &rootfs_metadata,
            ),
            (
                initrd_path,
                output_dir.join(TAIRA_INROU_WORKSPACE_INITRD_FILE_V1),
                &initrd_metadata,
            ),
        ] {
            let copied = copy_taira_stage_source_file(source, &destination, metadata)?;
            if copied != metadata.len() {
                return Err(eyre!(
                    "Taira guest-image copy {} wrote {copied} of {} bytes",
                    destination.display(),
                    metadata.len()
                ));
            }
        }
        validate_generated_taira_inrou_workspace(output_dir)?;
        Ok(TairaInrouWorkspaceReceiptV1 {
            schema_version: TAIRA_INROU_WORKSPACE_SCHEMA_VERSION_V1,
            container_file: TAIRA_INROU_WORKSPACE_CONTAINER_FILE_V1.to_owned(),
            service_file: TAIRA_INROU_WORKSPACE_SERVICE_FILE_V1.to_owned(),
            bundle_file: TAIRA_INROU_WORKSPACE_BUNDLE_FILE_V1.to_owned(),
            kernel_file: TAIRA_INROU_WORKSPACE_KERNEL_FILE_V1.to_owned(),
            rootfs_file: TAIRA_INROU_WORKSPACE_ROOTFS_FILE_V1.to_owned(),
            initrd_file: TAIRA_INROU_WORKSPACE_INITRD_FILE_V1.to_owned(),
            bundle_hash: bundle.container.bundle_hash.to_string(),
            container_manifest_hash: bundle.container.workspace_hash()?.to_string(),
            service_manifest_hash: Hash::new(Encode::encode(&bundle.service)).to_string(),
            guest_total_bytes,
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output_dir);
    }
    result
}
struct TairaSequentialPayloadReader<'a, P> {
    source: &'a mut P,
    offset: u64,
    length: u64,
}
impl<'a, P> TairaSequentialPayloadReader<'a, P> {
    const fn new(source: &'a mut P, length: u64) -> Self {
        Self {
            source,
            offset: 0,
            length,
        }
    }
}
impl<P: PayloadSource> io::Read for TairaSequentialPayloadReader<'_, P> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.offset == self.length || buffer.is_empty() {
            return Ok(0);
        }
        let remaining = usize::try_from((self.length - self.offset).min(buffer.len() as u64))
            .map_err(|_| io::Error::other("Taira staged payload length exceeds host width"))?;
        PayloadSource::read_exact(self.source, self.offset, &mut buffer[..remaining])
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.offset = self
            .offset
            .checked_add(remaining as u64)
            .ok_or_else(|| io::Error::other("Taira staged payload offset overflow"))?;
        Ok(remaining)
    }
}
impl<P: PayloadSource> TairaSequentialPayloadReader<'_, P> {
    fn finish(self) -> Result<()> {
        if self.offset != self.length {
            return Err(eyre!(
                "Taira staged payload reader consumed {} of {} bytes",
                self.offset,
                self.length
            ));
        }
        self.source
            .ensure_exhausted(self.length)
            .map_err(|error| eyre!("validate exact Taira staged payload source: {error}"))
    }
}
fn taira_streaming_directory_plan(
    root: &Path,
    member_paths: &[String],
    profile: sorafs_chunker::ChunkProfile,
) -> Result<CarBuildPlan> {
    let logical_paths = taira_stage_logical_member_paths(member_paths)?;
    let mut provisional_files = Vec::with_capacity(logical_paths.len());
    let mut file_sizes = Vec::with_capacity(logical_paths.len());
    for logical_path in &logical_paths {
        let metadata =
            taira_direct_member_metadata(root, logical_path, "Taira guest-image member")?;
        file_sizes.push(metadata.len());
        provisional_files.push(FilePlan {
            path: logical_path.clone(),
            first_chunk: 0,
            chunk_count: 0,
            size: metadata.len(),
        });
    }
    let total_bytes = taira_stage_guest_total_bytes(file_sizes.iter().copied())?;
    let mut source = DirectoryPayload::new(root, &provisional_files)
        .wrap_err("open exact Taira guest-image directory payload")?;
    let mut chunks = Vec::new();
    let mut files = Vec::with_capacity(provisional_files.len());
    let mut payload_hasher = blake3::Hasher::new();
    let mut global_offset = 0_u64;
    let mut read_buffer = vec![0_u8; TAIRA_INROU_STAGE_STREAM_BUFFER_BYTES];
    for provisional in provisional_files {
        let first_chunk = chunks.len();
        let mut local_offset = 0_u64;
        let mut emitted_offset = 0_usize;
        let mut pending = Vec::with_capacity(
            TAIRA_INROU_STAGE_STREAM_BUFFER_BYTES.saturating_add(profile.max_size),
        );
        let mut chunker = sorafs_chunker::Chunker::try_with_profile(profile)
            .wrap_err("construct Taira guest-image streaming chunker")?;
        while local_offset < provisional.size {
            let count =
                usize::try_from((provisional.size - local_offset).min(read_buffer.len() as u64))
                    .map_err(|_| eyre!("Taira guest-image member length exceeds host width"))?;
            PayloadSource::read_exact(
                &mut source,
                global_offset + local_offset,
                &mut read_buffer[..count],
            )
            .map_err(|error| eyre!("read exact Taira guest-image member bytes: {error}"))?;
            payload_hasher.update(&read_buffer[..count]);
            pending.extend_from_slice(&read_buffer[..count]);
            let mut boundaries = Vec::new();
            chunker.feed(&read_buffer[..count], |boundary| boundaries.push(boundary));
            let mut consumed = 0_usize;
            for boundary in boundaries {
                if boundary.offset != emitted_offset {
                    return Err(eyre!(
                        "Taira guest-image streaming chunker emitted non-contiguous geometry"
                    ));
                }
                let end = consumed
                    .checked_add(boundary.length)
                    .ok_or_else(|| eyre!("Taira guest-image chunk length overflow"))?;
                let bytes = pending.get(consumed..end).ok_or_else(|| {
                    eyre!("Taira guest-image chunk exceeded the bounded streaming buffer")
                })?;
                chunks.push(CarChunk {
                    offset: global_offset + boundary.offset as u64,
                    length: u32::try_from(boundary.length)
                        .map_err(|_| eyre!("Taira guest-image chunk length exceeds u32"))?,
                    digest: blake3::hash(bytes).into(),
                });
                emitted_offset = emitted_offset
                    .checked_add(boundary.length)
                    .ok_or_else(|| eyre!("Taira guest-image emitted offset overflow"))?;
                consumed = end;
            }
            if consumed != 0 {
                pending.drain(..consumed);
            }
            local_offset = local_offset
                .checked_add(count as u64)
                .ok_or_else(|| eyre!("Taira guest-image local offset overflow"))?;
        }
        if provisional.size != 0 {
            let mut boundaries = Vec::new();
            chunker.finish(|boundary| boundaries.push(boundary));
            for boundary in boundaries {
                if boundary.offset != emitted_offset || pending.len() != boundary.length {
                    return Err(eyre!(
                        "Taira guest-image streaming chunker final geometry is not canonical"
                    ));
                }
                chunks.push(CarChunk {
                    offset: global_offset + boundary.offset as u64,
                    length: u32::try_from(boundary.length)
                        .map_err(|_| eyre!("Taira guest-image chunk length exceeds u32"))?,
                    digest: blake3::hash(&pending).into(),
                });
                emitted_offset = emitted_offset
                    .checked_add(boundary.length)
                    .ok_or_else(|| eyre!("Taira guest-image emitted offset overflow"))?;
                pending.clear();
            }
        }
        if emitted_offset as u64 != provisional.size || !pending.is_empty() {
            return Err(eyre!(
                "Taira guest-image streaming chunker did not cover one member exactly"
            ));
        }
        files.push(FilePlan {
            path: provisional.path,
            first_chunk,
            chunk_count: chunks.len() - first_chunk,
            size: provisional.size,
        });
        global_offset = global_offset
            .checked_add(provisional.size)
            .ok_or_else(|| eyre!("Taira guest-image global offset overflow"))?;
    }
    PayloadSource::ensure_exhausted(&mut source, total_bytes)
        .map_err(|error| eyre!("validate exact Taira guest-image source: {error}"))?;
    let plan = CarBuildPlan {
        chunk_profile: profile,
        payload_digest: payload_hasher.finalize(),
        content_length: total_bytes,
        chunks,
        files,
    };
    plan.validate()
        .wrap_err("validate canonical Taira guest-image streaming plan")?;
    Ok(plan)
}
fn taira_stage_manifest(
    plan: &CarBuildPlan,
    payload: &[u8],
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
    description: &str,
) -> Result<BuiltSorafsManifest> {
    let descriptor = chunker_registry::default_descriptor();
    let writer_error = format!("failed to prepare staged {description} CAR writer");
    let metadata_error = format!("failed to compute staged {description} CAR metadata");
    let root_error = format!("staged {description} CAR planning produced no root CID");
    let por_error = format!("failed to compute staged {description} PoR root");
    let manifest_error = format!("failed to build staged {description} manifest");
    let governance_error = format!("failed to attach staged {description} governance proof");
    let encoding_error = format!("failed to encode staged {description} manifest");
    let digest_error = format!("failed to compute staged {description} manifest digest");
    build_sorafs_artifact_manifest(
        plan,
        payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: &writer_error,
            metadata: &metadata_error,
            root: &root_error,
            por: &por_error,
            manifest: &manifest_error,
            governance: &governance_error,
            encoding: &encoding_error,
            digest: &digest_error,
        },
    )
}
fn taira_stage_directory_manifest(
    plan: &CarBuildPlan,
    payload_dir: &Path,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
    description: &str,
) -> Result<BuiltSorafsManifest> {
    let descriptor = chunker_registry::default_descriptor();
    if plan.chunk_profile != descriptor.profile {
        return Err(eyre!(
            "staged {description} plan does not use the canonical chunking profile"
        ));
    }
    let mut car_source = DirectoryPayload::new(payload_dir, &plan.files)
        .wrap_err_with(|| format!("open staged {description} CAR source"))?;
    let mut car_reader = TairaSequentialPayloadReader::new(&mut car_source, plan.content_length);
    let car_stats = CarStreamingWriter::new(plan)
        .write_from_reader(&mut car_reader, io::sink())
        .wrap_err_with(|| format!("compute staged {description} canonical CAR metadata"))?;
    car_reader
        .finish()
        .wrap_err_with(|| format!("finish staged {description} CAR source"))?;
    let mut por_source = DirectoryPayload::new(payload_dir, &plan.files)
        .wrap_err_with(|| format!("open staged {description} PoR source"))?;
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    store
        .ingest_plan_source(plan, &mut por_source)
        .wrap_err_with(|| format!("compute staged {description} PoR root"))?;
    PayloadSource::ensure_exhausted(&mut por_source, plan.content_length)
        .map_err(|error| eyre!("validate staged {description} PoR source: {error}"))?;
    let root_cid = car_stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| eyre!("staged {description} CAR planning produced no root CID"))?;
    if car_stats.root_cids.len() != 1 {
        return Err(eyre!(
            "staged {description} CAR must produce exactly one root CID"
        ));
    }
    let manifest = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_profile(ChunkingProfileV1::from_descriptor(descriptor))
        .chunk_digest_sha3_256(compute_chunk_digest_sha3(&plan.chunks))
        .por_root(*store.por_tree().root())
        .content_length(plan.content_length)
        .car_digest(*car_stats.car_archive_digest.as_bytes())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: u16::try_from(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1)
                .expect("first-release artifact replica count fits u16"),
            storage_class: ManifestStorageClass::Hot,
            retention_epoch: release_identity.retention_epoch(),
        })
        .governance(GovernanceProofs::default())
        .build()
        .wrap_err_with(|| format!("build staged {description} manifest"))?;
    let manifest = attach_sorafs_release_governance(manifest, key_pair, release_identity)
        .wrap_err_with(|| format!("attach staged {description} governance proof"))?;
    validate_sorafs_release_identity(&manifest, release_identity, description)?;
    let policy = PinPolicyConstraints {
        require_council_signatures: true,
        ..PinPolicyConstraints::default()
    };
    validate_manifest(&manifest, &policy)
        .wrap_err_with(|| format!("validate staged {description} manifest"))?;
    let (bytes, _) = encode_sorafs_manifest_for_storage(&manifest)
        .wrap_err_with(|| format!("encode staged {description} manifest"))?;
    let digest_hex = hex::encode(
        manifest
            .digest()
            .wrap_err_with(|| format!("digest staged {description} manifest"))?
            .as_bytes(),
    );
    Ok(BuiltSorafsManifest {
        manifest,
        bytes,
        digest_hex,
    })
}
fn taira_inrou_source_workspace(
    container_path: &Path,
    service_path: &Path,
    bundle_file: &Path,
) -> Result<PathBuf> {
    let paths = [container_path, service_path, bundle_file];
    let mut workspace: Option<PathBuf> = None;
    for path in paths {
        validate_taira_path_ancestors(path, "Taira Inrou source")?;
        let metadata = fs::symlink_metadata(path)
            .wrap_err_with(|| format!("inspect Taira Inrou source {}", path.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(eyre!(
                "Taira Inrou source {} must be one direct regular file",
                path.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.nlink() != 1 {
                return Err(eyre!(
                    "Taira Inrou source {} must have exactly one hard link",
                    path.display()
                ));
            }
        }
        let parent = path.parent().ok_or_else(|| {
            eyre!(
                "Taira Inrou source {} has no workspace directory",
                path.display()
            )
        })?;
        let parent_metadata = fs::symlink_metadata(parent)
            .wrap_err_with(|| format!("inspect Taira Inrou workspace {}", parent.display()))?;
        if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
            return Err(eyre!(
                "Taira Inrou workspace {} must be one direct directory",
                parent.display()
            ));
        }
        let canonical_parent = fs::canonicalize(parent)
            .wrap_err_with(|| format!("canonicalize Taira Inrou workspace {}", parent.display()))?;
        if workspace
            .as_ref()
            .is_some_and(|expected| expected != &canonical_parent)
        {
            return Err(eyre!(
                "Taira Inrou container, service, bundle, and inrou directory must share one workspace"
            ));
        }
        workspace = Some(canonical_parent);
    }
    let workspace = workspace.ok_or_else(|| eyre!("Taira Inrou workspace is empty"))?;
    let inrou_dir = workspace.join("inrou");
    let inrou_metadata = fs::symlink_metadata(&inrou_dir).wrap_err_with(|| {
        format!(
            "inspect Taira Inrou source directory {}",
            inrou_dir.display()
        )
    })?;
    if inrou_metadata.file_type().is_symlink() || !inrou_metadata.is_dir() {
        return Err(eyre!(
            "Taira Inrou source directory {} must be one direct directory",
            inrou_dir.display()
        ));
    }
    Ok(workspace)
}
fn write_taira_stage_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: JsonSerialize + ?Sized,
{
    let bytes = json::to_vec_pretty(value)
        .wrap_err_with(|| format!("encode Taira Inrou staged JSON {}", path.display()))?;
    write_taira_stage_file(path, &bytes)
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn stage_taira_inrou_canary_deployment(
    requested_mode: crate::taira::InrouCanaryMode,
    container_path: &Path,
    service_path: &Path,
    bundle_file: &Path,
    stage_dir: &Path,
    key_pair: &KeyPair,
    sorafs_retention_epoch: NonZeroU64,
    placement_targets: BTreeSet<SoraInrouPlacementTargetV1>,
) -> Result<TairaInrouStageReceiptV1> {
    let release_identity = SorafsReleaseIdentityV1::new(sorafs_retention_epoch);
    let mode = MutationMode::from(requested_mode);
    let workspace_dir = taira_inrou_source_workspace(container_path, service_path, bundle_file)?;
    let container_bytes = taira_stage_regular_file_bytes(
        container_path,
        "Taira container manifest",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let service_bytes = taira_stage_regular_file_bytes(
        service_path,
        "Taira service manifest",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let container: UnpublishedContainerManifestV1 =
        decode_taira_stage_json(&container_bytes, container_path)?;
    let service: SoraServiceManifestV1 = decode_taira_stage_json(&service_bytes, service_path)?;
    let mut bundle = UnpublishedDeploymentBundleV1 { container, service };
    // The source carries only the canonical staging sentinel. The stage owns
    // the revision identity so callers cannot smuggle an arbitrary version
    // label into the release canary.
    validate_taira_inrou_canary_source_bundle(&bundle)?;
    if placement_targets.len() != 4 {
        return Err(eyre!(
            "Taira Inrou staging requires exactly four distinct placement targets"
        ));
    }
    for target in &placement_targets {
        target.validate()?;
    }
    bundle.service.placement_targets = placement_targets;
    validate_unpublished_deployment_source(&bundle)?;
    let bundle_bytes = taira_stage_regular_file_bytes(
        bundle_file,
        "Taira service bundle",
        INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES,
    )?;
    validate_taira_inrou_canary_bundle_payload(&bundle_bytes)?;
    let bundle_hash = Hash::new(&bundle_bytes);
    if bundle_hash != bundle.container.bundle_hash {
        return Err(eyre!(
            "Taira service bundle hash {bundle_hash} does not match admitted hash {}",
            bundle.container.bundle_hash
        ));
    }
    create_taira_stage_directory(stage_dir)?;
    let stage_result = (|| -> Result<TairaInrouStageReceiptV1> {
        let manifests_dir = stage_dir.join("manifests");
        let payloads_dir = stage_dir.join("payloads");
        let guest_payload_dir = stage_dir.join(TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1);
        let discovery_payload_dir = stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1);
        create_taira_stage_subdirectory(&manifests_dir)?;
        create_taira_stage_subdirectory(&payloads_dir)?;
        create_taira_stage_subdirectory(&guest_payload_dir)?;
        create_taira_stage_subdirectory(&discovery_payload_dir)?;
        let staged_bundle_path = stage_dir.join(TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1);
        write_taira_stage_file(&staged_bundle_path, &bundle_bytes)?;
        let bundle_plan = CarBuildPlan::single_file_with_profile(
            &bundle_bytes,
            chunker_registry::default_descriptor().profile,
        )
        .map_err(|error| eyre!("build staged Taira bundle plan: {error}"))?;
        let bundle_manifest = taira_stage_manifest(
            &bundle_plan,
            &bundle_bytes,
            key_pair,
            release_identity,
            "Taira bundle",
        )?;
        write_taira_stage_file(
            &stage_dir.join(TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1),
            &bundle_manifest.bytes,
        )?;
        let inrou_dir = workspace_dir.join("inrou");
        let image = bundle
            .container
            .inrou
            .as_ref()
            .and_then(|inrou| inrou.guest_images.get("aarch64"))
            .ok_or_else(|| eyre!("Taira AArch64 guest image disappeared during staging"))?;
        let rootfs_member_path = inrou_member_path(&image.rootfs_image_path)?;
        let rootfs_logical_path = rootfs_member_path
            .split('/')
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let member_paths = vec![
            inrou_member_path(&image.kernel_image_path)?,
            rootfs_member_path,
            inrou_member_path(
                image
                    .initrd_image_path
                    .as_deref()
                    .expect("validated Taira AArch64 image has an initrd"),
            )?,
        ];
        let logical_member_paths = taira_stage_logical_member_paths(&member_paths)?;
        let source_metadata = logical_member_paths
            .iter()
            .map(|logical_path| {
                taira_direct_member_metadata(
                    &inrou_dir,
                    logical_path,
                    "Taira guest-image source member",
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let source_sizes = source_metadata.iter().map(fs::Metadata::len);
        taira_stage_guest_total_bytes(source_sizes.clone())?;
        let rootfs_source_bytes = logical_member_paths
            .iter()
            .zip(source_sizes)
            .find_map(|(logical_path, size)| (logical_path == &rootfs_logical_path).then_some(size))
            .ok_or_else(|| eyre!("Taira rootfs disappeared from the exact guest-image layout"))?;
        validate_taira_inrou_rootfs_source_bytes(rootfs_source_bytes)?;
        for (logical_path, source_metadata) in logical_member_paths.iter().zip(&source_metadata) {
            let member_path = logical_path.join("/");
            let source = inrou_dir.join(&member_path);
            let destination = guest_payload_dir.join(&member_path);
            let parent = create_taira_stage_member_parent(&guest_payload_dir, logical_path)?;
            if destination.parent() != Some(parent.as_path()) {
                return Err(eyre!(
                    "Taira guest staging destination {} escaped its canonical parent",
                    destination.display()
                ));
            }
            let copied = copy_taira_stage_source_file(&source, &destination, source_metadata)?;
            if copied != source_metadata.len() {
                return Err(eyre!(
                    "Taira guest-image copy {} wrote {copied} of {} bytes",
                    destination.display(),
                    source_metadata.len()
                ));
            }
            set_taira_stage_permissions(&destination, 0o600)?;
        }
        let guest_plan = taira_streaming_directory_plan(
            &guest_payload_dir,
            &member_paths,
            chunker_registry::default_descriptor().profile,
        )?;
        let guest_manifest = taira_stage_directory_manifest(
            &guest_plan,
            &guest_payload_dir,
            key_pair,
            release_identity,
            "Taira guest image",
        )?;
        write_taira_stage_file(
            &stage_dir.join(TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1),
            &guest_manifest.bytes,
        )?;
        let guest_content_cid = encode_content_cid(&guest_manifest.manifest.root_cid);
        let mut bundle = bundle.into_admitted(BTreeMap::from([(
            SoraInrouGuestIsaV1::Aarch64,
            SoraPublishedInrouGuestImageArtifactV1 {
                manifest_digest_hex: guest_manifest.digest_hex.clone(),
                content_cid: guest_content_cid.clone(),
            },
        )]))?;
        install_taira_inrou_canary_service_version(&mut bundle)?;
        validate_taira_inrou_canary_bundle(&bundle)?;
        let container_manifest_hash = bundle.container_manifest_hash().to_string();
        let service_manifest_hash = bundle.service_manifest_hash().to_string();
        let (discovery, discovery_publication, discovery_artifact) =
            prepare_public_service_discovery(&bundle, key_pair, release_identity)?;
        let [discovery_file] = discovery_artifact.plan.files.as_slice() else {
            return Err(eyre!(
                "Taira public discovery must contain exactly one logical file"
            ));
        };
        if discovery_file.path != [PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT.to_owned()]
            || discovery_file.first_chunk != 0
            || discovery_file.size != discovery_artifact.plan.content_length
            || discovery_artifact.plan.content_length
                != u64::try_from(discovery_artifact.payload.len())
                    .wrap_err("Taira public discovery payload length exceeds u64")?
            || Hash::new(&discovery_artifact.payload) != discovery.document_hash
        {
            return Err(eyre!(
                "Taira public discovery is not the exact canonical one-file document"
            ));
        }
        write_taira_stage_file(
            &discovery_payload_dir.join(PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT),
            &discovery_artifact.payload,
        )?;
        write_taira_stage_file(
            &stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1),
            &discovery_artifact.built.bytes,
        )?;
        write_taira_stage_json(
            &stage_dir.join(TAIRA_INROU_STAGE_CONTAINER_FILE_V1),
            &bundle.container,
        )?;
        write_taira_stage_json(
            &stage_dir.join(TAIRA_INROU_STAGE_SERVICE_FILE_V1),
            &bundle.service,
        )?;
        let receipt = TairaInrouStageReceiptV1 {
            schema_version: TAIRA_INROU_STAGE_SCHEMA_VERSION_V1,
            sorafs_retention_epoch: release_identity.retention_epoch(),
            placement_targets: bundle.service.placement_targets.clone(),
            mutation_mode: mode.label_lowercase().to_owned(),
            service_name: bundle.service.service_name.to_string(),
            service_version: bundle.service.service_version.clone(),
            container_file: TAIRA_INROU_STAGE_CONTAINER_FILE_V1.to_owned(),
            service_file: TAIRA_INROU_STAGE_SERVICE_FILE_V1.to_owned(),
            bundle_payload_file: TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1.to_owned(),
            bundle_manifest_file: TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1.to_owned(),
            bundle_hash: bundle_hash.to_string(),
            bundle_content_cid: encode_content_cid(&bundle_manifest.manifest.root_cid),
            bundle_manifest_digest_hex: bundle_manifest.digest_hex,
            guest_isa: SoraInrouGuestIsaV1::Aarch64.as_str().to_owned(),
            guest_payload_dir: TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1.to_owned(),
            guest_manifest_file: TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1.to_owned(),
            guest_content_cid,
            guest_manifest_digest_hex: guest_manifest.digest_hex,
            discovery_payload_dir: TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1.to_owned(),
            discovery_manifest_file: TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1.to_owned(),
            discovery_document_hash: discovery.document_hash.to_string(),
            discovery_content_cid: discovery.content_cid.clone(),
            discovery_manifest_digest_hex: discovery.manifest_digest_hex.clone(),
            public_discovery_url: discovery_publication.public_discovery_url,
            public_discovery_cid_host_url: discovery_publication.public_discovery_cid_host_url,
            container_manifest_hash,
            service_manifest_hash,
        };
        write_taira_stage_json(&stage_dir.join(TAIRA_INROU_STAGE_RECEIPT_FILE_V1), &receipt)?;
        Ok(receipt)
    })();
    if stage_result.is_err() {
        let _ = fs::remove_dir_all(stage_dir);
    }
    stage_result
}

#[cfg(unix)]
const TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1: usize = 4;

#[cfg(unix)]
struct SensitiveTairaTomlTable(toml::Table);

#[cfg(unix)]
impl Drop for SensitiveTairaTomlTable {
    fn drop(&mut self) {
        zeroize_taira_toml_table(&mut self.0);
    }
}

#[cfg(unix)]
pub(crate) fn zeroize_taira_toml_table(table: &mut toml::Table) {
    table
        .iter_mut()
        .for_each(|(_, value)| zeroize_taira_toml_value(value));
}

#[cfg(unix)]
fn zeroize_taira_toml_value(value: &mut toml::Value) {
    match value {
        toml::Value::String(value) => value.zeroize(),
        toml::Value::Array(values) => values.iter_mut().for_each(zeroize_taira_toml_value),
        toml::Value::Table(table) => zeroize_taira_toml_table(table),
        toml::Value::Integer(_)
        | toml::Value::Float(_)
        | toml::Value::Boolean(_)
        | toml::Value::Datetime(_) => {}
    }
}

#[cfg(unix)]
fn taira_toml_table_at<'a>(
    root: &'a toml::Table,
    path: &[&str],
    description: &str,
) -> Result<&'a toml::Table> {
    let mut table = root;
    for component in path {
        table = table
            .get(*component)
            .and_then(toml::Value::as_table)
            .ok_or_else(|| eyre!("{description} is missing the required TOML table"))?;
    }
    Ok(table)
}

#[cfg(unix)]
fn taira_toml_required_string(table: &toml::Table, key: &str, description: &str) -> Result<String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| eyre!("{description} must be one TOML string"))
}

#[cfg(unix)]
fn taira_validator_placement_from_table(
    table: &toml::Table,
    peer_index: usize,
) -> Result<SoraInrouPlacementTargetV1> {
    let peer_id = taira_toml_required_string(
        table,
        "public_key",
        &format!("peer{peer_index}.toml top-level public_key"),
    )?;
    let signer = taira_toml_table_at(
        table,
        &["soracloud_runtime", "submission", "signer"],
        &format!("peer{peer_index}.toml Soracloud runtime signer"),
    )?;
    let validator_account = taira_toml_required_string(
        signer,
        "authority",
        &format!("peer{peer_index}.toml Soracloud runtime signer authority"),
    )?;
    let target = SoraInrouPlacementTargetV1 {
        validator_account_id: parse_canonical_inrou_validator_account(&validator_account).map_err(
            |error| eyre!("peer{peer_index}.toml has an invalid validator account: {error}"),
        )?,
        peer_id,
    };
    target.validate().map_err(|error| {
        eyre!("peer{peer_index}.toml has an invalid placement identity: {error}")
    })?;
    Ok(target)
}

#[cfg(unix)]
fn taira_toml_integer(value: u64, field: &str) -> Result<toml::Value> {
    i64::try_from(value)
        .map(toml::Value::Integer)
        .map_err(|_| eyre!("Taira Inrou {field} exceeds the TOML integer range"))
}

#[cfg(unix)]
fn taira_inrou_validator_slot(peer_index: usize) -> Result<NonZeroU32> {
    let slot = u32::try_from(peer_index)
        .ok()
        .and_then(|index| defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_BASE.checked_add(index))
        .filter(|slot| *slot < defaults::soracloud_runtime::INROU_PORTABLE_VM_ID_MAX_EXCLUSIVE)
        .ok_or_else(|| eyre!("Taira Inrou validator index {peer_index} has no V1 identity slot"))?;
    NonZeroU32::new(slot)
        .ok_or_else(|| eyre!("Taira Inrou validator index {peer_index} resolved to a zero uid/gid"))
}

#[cfg(unix)]
fn taira_inrou_validator_table(
    peer_index: usize,
    receipt: &TairaInrouStageReceiptV1,
) -> Result<toml::Table> {
    let slot = taira_inrou_validator_slot(peer_index)?;
    let mut table = toml::Table::new();
    table.insert("enabled".to_owned(), toml::Value::Boolean(true));
    table.insert(
        "portable_vm_uid".to_owned(),
        taira_toml_integer(u64::from(slot.get()), "portable_vm_uid")?,
    );
    table.insert(
        "portable_vm_gid".to_owned(),
        taira_toml_integer(u64::from(slot.get()), "portable_vm_gid")?,
    );
    table.insert(
        "trusted_guest_manifest_digest_hex".to_owned(),
        toml::Value::String(receipt.guest_manifest_digest_hex.clone()),
    );
    table.insert(
        "trusted_guest_content_cid".to_owned(),
        toml::Value::String(receipt.guest_content_cid.clone()),
    );
    table.insert(
        "guest_image_max_bytes".to_owned(),
        taira_toml_integer(
            TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1,
            "guest_image_max_bytes",
        )?,
    );
    table.insert(
        "max_cpu_millis".to_owned(),
        taira_toml_integer(
            u64::from(defaults::taira::INROU_MAX_CPU_MILLIS),
            "max_cpu_millis",
        )?,
    );
    table.insert(
        "max_memory_bytes".to_owned(),
        taira_toml_integer(defaults::taira::INROU_MAX_MEMORY_BYTES, "max_memory_bytes")?,
    );
    table.insert(
        "max_storage_bytes".to_owned(),
        taira_toml_integer(
            defaults::taira::INROU_MAX_STORAGE_BYTES,
            "max_storage_bytes",
        )?,
    );
    table.insert(
        "start_grace_ms".to_owned(),
        taira_toml_integer(
            defaults::soracloud_runtime::INROU_START_GRACE_MS,
            "start_grace_ms",
        )?,
    );
    table.insert(
        "stop_grace_ms".to_owned(),
        taira_toml_integer(
            defaults::soracloud_runtime::INROU_STOP_GRACE_MS,
            "stop_grace_ms",
        )?,
    );
    Ok(table)
}

#[cfg(unix)]
fn expected_taira_inrou_validator_config(
    peer_index: usize,
    receipt: &TairaInrouStageReceiptV1,
) -> Result<actual::SoracloudRuntimeInrou> {
    let slot = taira_inrou_validator_slot(peer_index)?;
    Ok(actual::SoracloudRuntimeInrou {
        enabled: true,
        portable_vm_uid: Some(slot),
        portable_vm_gid: Some(slot),
        trusted_guest_artifact: Some(SoraPublishedInrouGuestImageArtifactV1 {
            manifest_digest_hex: receipt.guest_manifest_digest_hex.clone(),
            content_cid: receipt.guest_content_cid.clone(),
        }),
        guest_image_max_bytes: NonZeroU64::new(TAIRA_INROU_STAGE_MAX_GUEST_BYTES_V1)
            .expect("Taira guest-image limit is nonzero"),
        max_cpu_millis: NonZeroU32::new(defaults::taira::INROU_MAX_CPU_MILLIS)
            .expect("nonzero Taira resource ceiling"),
        max_memory_bytes: NonZeroU64::new(defaults::taira::INROU_MAX_MEMORY_BYTES)
            .expect("nonzero Taira resource ceiling"),
        max_storage_bytes: NonZeroU64::new(defaults::taira::INROU_MAX_STORAGE_BYTES)
            .expect("nonzero Taira resource ceiling"),
        bundle_archive_max_compressed_bytes:
            defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_COMPRESSED_BYTES,
        bundle_archive_max_decoded_bytes:
            defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_DECODED_BYTES,
        bundle_archive_max_entries: defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_ENTRIES,
        bundle_archive_max_file_bytes:
            defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_FILE_BYTES,
        bundle_archive_max_total_file_bytes:
            defaults::soracloud_runtime::INROU_BUNDLE_ARCHIVE_MAX_TOTAL_FILE_BYTES,
        start_grace: Duration::from_millis(defaults::soracloud_runtime::INROU_START_GRACE_MS),
        stop_grace: Duration::from_millis(defaults::soracloud_runtime::INROU_STOP_GRACE_MS),
    })
}

#[cfg(unix)]
fn insert_taira_inrou_validator_table(
    root: &mut toml::Table,
    peer_index: usize,
    receipt: &TairaInrouStageReceiptV1,
) -> Result<()> {
    let runtime = root
        .get_mut("soracloud_runtime")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| {
            eyre!("peer{peer_index}.toml is missing the required [soracloud_runtime] table")
        })?;
    if runtime.contains_key("inrou") {
        return Err(eyre!(
            "peer{peer_index}.toml already contains soracloud_runtime.inrou; first-release binding never reuses or upgrades an existing Inrou profile"
        ));
    }
    runtime.insert(
        "inrou".to_owned(),
        toml::Value::Table(taira_inrou_validator_table(peer_index, receipt)?),
    );
    Ok(())
}

#[cfg(unix)]
fn validate_taira_inrou_config_receipt(receipt: &TairaInrouStageReceiptV1) -> Result<()> {
    if receipt.schema_version != TAIRA_INROU_STAGE_SCHEMA_VERSION_V1
        || receipt.placement_targets.len() != TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
    {
        return Err(eyre!(
            "Taira Inrou validator binding requires one exact four-placement V1 stage receipt"
        ));
    }
    let trusted_guest = SoraPublishedInrouGuestImageArtifactV1 {
        manifest_digest_hex: receipt.guest_manifest_digest_hex.clone(),
        content_cid: receipt.guest_content_cid.clone(),
    };
    trusted_guest.validate().map_err(|error| {
        eyre!("Taira Inrou stage has an invalid trusted guest artifact: {error}")
    })?;
    let validator_count = receipt
        .placement_targets
        .iter()
        .map(|target| &target.validator_account_id)
        .collect::<BTreeSet<_>>()
        .len();
    let peer_count = receipt
        .placement_targets
        .iter()
        .map(|target| target.peer_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    if validator_count != TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
        || peer_count != TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
    {
        return Err(eyre!(
            "Taira Inrou stage placements must bind four distinct validator accounts and peer IDs"
        ));
    }
    for target in &receipt.placement_targets {
        target
            .validate()
            .map_err(|error| eyre!("Taira Inrou stage has an invalid placement target: {error}"))?;
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TairaOwnedDirectoryIdentity {
    dev: u64,
    ino: u64,
    uid: u32,
    mode: u32,
}

#[cfg(unix)]
fn taira_owned_directory_identity(metadata: &fs::Metadata) -> TairaOwnedDirectoryIdentity {
    use std::os::unix::fs::MetadataExt as _;

    TairaOwnedDirectoryIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        uid: metadata.uid(),
        mode: metadata.mode() & 0o7777,
    }
}

#[cfg(unix)]
struct TairaValidatorConfigDirectory {
    path: PathBuf,
    file: fs::File,
    identity: TairaOwnedDirectoryIdentity,
}

#[cfg(unix)]
fn validate_taira_validator_config_directory_metadata(
    metadata: &fs::Metadata,
    description: &str,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
    {
        return Err(eyre!(
            "{description} must be one direct directory owned by the effective user with mode 0700"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn open_taira_validator_config_directory(
    config_dir: &Path,
) -> Result<TairaValidatorConfigDirectory> {
    if !config_dir.is_absolute() {
        return Err(eyre!(
            "--bind-validator-config-dir must be an absolute owner-private directory"
        ));
    }
    validate_taira_path_ancestors(config_dir, "Taira validator config directory")?;
    let named_before = fs::symlink_metadata(config_dir).wrap_err_with(|| {
        format!(
            "inspect Taira validator config directory {}",
            config_dir.display()
        )
    })?;
    validate_taira_validator_config_directory_metadata(
        &named_before,
        "Taira validator config directory",
    )?;
    let file = fs::File::from(
        rustix::fs::open(
            config_dir,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(io::Error::from)
        .wrap_err_with(|| {
            format!(
                "open Taira validator config directory {}",
                config_dir.display()
            )
        })?,
    );
    let opened = file.metadata().wrap_err_with(|| {
        format!(
            "inspect opened Taira validator config directory {}",
            config_dir.display()
        )
    })?;
    validate_taira_validator_config_directory_metadata(
        &opened,
        "opened Taira validator config directory",
    )?;
    let named_after = fs::symlink_metadata(config_dir).wrap_err_with(|| {
        format!(
            "reinspect Taira validator config directory {}",
            config_dir.display()
        )
    })?;
    validate_taira_validator_config_directory_metadata(
        &named_after,
        "Taira validator config directory",
    )?;
    let identity = taira_owned_directory_identity(&opened);
    if taira_owned_directory_identity(&named_before) != identity
        || taira_owned_directory_identity(&named_after) != identity
    {
        return Err(eyre!(
            "Taira validator config directory changed while opening its retained descriptor"
        ));
    }
    Ok(TairaValidatorConfigDirectory {
        path: config_dir.to_path_buf(),
        file,
        identity,
    })
}

#[cfg(unix)]
fn require_taira_validator_config_directory_identity(
    directory: &TairaValidatorConfigDirectory,
) -> Result<()> {
    let opened = directory.file.metadata().wrap_err_with(|| {
        format!(
            "reinspect opened Taira validator config directory {}",
            directory.path.display()
        )
    })?;
    let named = fs::symlink_metadata(&directory.path).wrap_err_with(|| {
        format!(
            "reinspect named Taira validator config directory {}",
            directory.path.display()
        )
    })?;
    validate_taira_validator_config_directory_metadata(
        &opened,
        "opened Taira validator config directory",
    )?;
    validate_taira_validator_config_directory_metadata(
        &named,
        "named Taira validator config directory",
    )?;
    if taira_owned_directory_identity(&opened) != directory.identity
        || taira_owned_directory_identity(&named) != directory.identity
    {
        return Err(eyre!(
            "Taira validator config directory changed after its descriptor was retained"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn taira_file_identity_from_stat(stat: &rustix::fs::Stat) -> Result<TairaFileIdentity> {
    Ok(TairaFileIdentity {
        dev: u64::try_from(stat.st_dev)
            .map_err(|_| eyre!("Taira validator config device identity is out of range"))?,
        ino: u64::try_from(stat.st_ino)
            .map_err(|_| eyre!("Taira validator config inode identity is out of range"))?,
        len: u64::try_from(stat.st_size)
            .map_err(|_| eyre!("Taira validator config length is out of range"))?,
        mtime: i64::try_from(stat.st_mtime)
            .map_err(|_| eyre!("Taira validator config modification time is out of range"))?,
        mtime_nsec: i64::try_from(stat.st_mtime_nsec).map_err(|_| {
            eyre!("Taira validator config modification nanoseconds are out of range")
        })?,
        ctime: i64::try_from(stat.st_ctime)
            .map_err(|_| eyre!("Taira validator config change time is out of range"))?,
        ctime_nsec: i64::try_from(stat.st_ctime_nsec)
            .map_err(|_| eyre!("Taira validator config change nanoseconds are out of range"))?,
        mode: u32::try_from(stat.st_mode)
            .map_err(|_| eyre!("Taira validator config mode is out of range"))?,
        uid: u32::try_from(stat.st_uid)
            .map_err(|_| eyre!("Taira validator config owner is out of range"))?,
        nlink: u64::try_from(stat.st_nlink)
            .map_err(|_| eyre!("Taira validator config link count is out of range"))?,
    })
}

#[cfg(unix)]
fn validate_taira_validator_config_stat(
    stat: &rustix::fs::Stat,
    description: &str,
    max_bytes: u64,
) -> Result<TairaFileIdentity> {
    let identity = taira_file_identity_from_stat(stat)?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
        || identity.uid != rustix::process::geteuid().as_raw()
        || identity.mode & 0o7777 != 0o600
        || identity.nlink != 1
        || identity.len == 0
        || identity.len > max_bytes
    {
        return Err(eyre!(
            "{description} must be a nonempty singly-linked owner-private regular file of at most {max_bytes} bytes"
        ));
    }
    Ok(identity)
}

#[cfg(unix)]
fn taira_validator_config_stat_at(
    directory: &TairaValidatorConfigDirectory,
    name: &str,
    description: &str,
    max_bytes: u64,
) -> Result<TairaFileIdentity> {
    let stat = rustix::fs::statat(&directory.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(io::Error::from)
        .wrap_err_with(|| format!("inspect {description} {name}"))?;
    validate_taira_validator_config_stat(&stat, description, max_bytes)
}

#[cfg(unix)]
#[derive(Clone, Debug)]
struct TairaValidatorConfigEntry {
    peer_index: usize,
    name: String,
    identity: TairaFileIdentity,
}

#[cfg(unix)]
fn exact_taira_validator_config_entries(
    directory: &TairaValidatorConfigDirectory,
) -> Result<[TairaValidatorConfigEntry; TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1]> {
    require_taira_validator_config_directory_identity(directory)?;
    let expected = (0..TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1)
        .map(|index| format!("peer{index}.toml"))
        .collect::<BTreeSet<_>>();
    let mut present = BTreeSet::new();
    let mut entries = rustix::fs::Dir::read_from(&directory.file)
        .map_err(io::Error::from)
        .wrap_err_with(|| {
            format!(
                "read retained Taira validator config directory {}",
                directory.path.display()
            )
        })?;
    for entry in &mut entries {
        let entry = entry.map_err(io::Error::from).wrap_err_with(|| {
            format!(
                "read entry in retained Taira validator config directory {}",
                directory.path.display()
            )
        })?;
        let name = entry.file_name();
        let bytes = name.to_bytes();
        if bytes.starts_with(b"peer") && bytes.ends_with(b".toml") {
            let name = std::str::from_utf8(bytes).map_err(|_| {
                eyre!("Taira validator config directory contains a non-UTF-8 peer TOML name")
            })?;
            present.insert(name.to_owned());
        }
    }
    if present != expected {
        return Err(eyre!(
            "Taira validator config directory must contain exactly peer0.toml through peer3.toml as peer TOML files"
        ));
    }
    let mut inode_identities = BTreeSet::new();
    let mut exact = Vec::with_capacity(TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1);
    for peer_index in 0..TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1 {
        let name = format!("peer{peer_index}.toml");
        let identity = taira_validator_config_stat_at(
            directory,
            &name,
            "Taira validator config",
            MAX_TOML_SOURCE_BYTES,
        )?;
        if !inode_identities.insert((identity.dev, identity.ino)) {
            return Err(eyre!(
                "Taira validator configs must be four distinct direct inodes under the owner-private directory"
            ));
        }
        exact.push(TairaValidatorConfigEntry {
            peer_index,
            name,
            identity,
        });
    }
    require_taira_validator_config_directory_identity(directory)?;
    exact
        .try_into()
        .map_err(|_| eyre!("Taira validator config count changed during validation"))
}

#[cfg(unix)]
fn read_sensitive_taira_validator_config_at(
    directory: &TairaValidatorConfigDirectory,
    name: &str,
    description: &str,
    max_bytes: u64,
) -> Result<(Zeroizing<Vec<u8>>, TairaFileIdentity)> {
    let named_before = taira_validator_config_stat_at(directory, name, description, max_bytes)?;
    let mut file = fs::File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(io::Error::from)
        .wrap_err_with(|| format!("open {description} {name}"))?,
    );
    let opened_before_metadata = file
        .metadata()
        .wrap_err_with(|| format!("inspect opened {description} {name}"))?;
    let opened_before = taira_metadata_identity(&opened_before_metadata);
    if opened_before != named_before {
        return Err(eyre!("{description} {name} changed while it was opened"));
    }
    let capacity = usize::try_from(opened_before.len)
        .map_err(|_| eyre!("{description} {name} length cannot be represented in memory"))?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(capacity.saturating_add(1))
        .map_err(|_| eyre!("reserve bounded sensitive buffer for {description} {name}"))?;
    std::io::Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| eyre!("read bounded sensitive bytes from {description} {name}"))?;
    let bytes_read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if bytes_read > max_bytes || bytes_read != opened_before.len {
        return Err(eyre!(
            "{description} {name} changed length during its bounded sensitive read"
        ));
    }
    let opened_after_metadata = file
        .metadata()
        .wrap_err_with(|| format!("reinspect opened {description} {name}"))?;
    let opened_after = taira_metadata_identity(&opened_after_metadata);
    let named_after = taira_validator_config_stat_at(directory, name, description, max_bytes)?;
    if opened_after != opened_before || named_after != opened_after {
        return Err(eyre!(
            "{description} {name} changed identity during its bounded sensitive read"
        ));
    }
    Ok((bytes, opened_after))
}

#[cfg(unix)]
struct PreparedTairaValidatorConfig {
    peer_index: usize,
    name: String,
    identity: TairaFileIdentity,
    rendered: Zeroizing<String>,
    temporary: Option<StagedTairaValidatorConfig>,
}

#[cfg(unix)]
struct StagedTairaValidatorConfig {
    name: String,
    file: fs::File,
    identity: TairaFileIdentity,
}

#[cfg(unix)]
fn prepare_taira_validator_config(
    directory: &TairaValidatorConfigDirectory,
    entry: TairaValidatorConfigEntry,
    receipt: &TairaInrouStageReceiptV1,
) -> Result<(SoraInrouPlacementTargetV1, PreparedTairaValidatorConfig)> {
    let TairaValidatorConfigEntry {
        peer_index,
        name,
        identity: expected_identity,
    } = entry;
    let (source, identity) = read_sensitive_taira_validator_config_at(
        directory,
        &name,
        "Taira validator config",
        MAX_TOML_SOURCE_BYTES,
    )?;
    if identity != expected_identity {
        return Err(eyre!(
            "peer{peer_index}.toml changed after exact directory validation"
        ));
    }
    let source_text = std::str::from_utf8(source.as_slice())
        .map_err(|_| eyre!("peer{peer_index}.toml is not UTF-8"))?;
    let mut table = SensitiveTairaTomlTable(
        toml::from_str(source_text)
            .map_err(|_| eyre!("peer{peer_index}.toml is not valid TOML"))?,
    );
    let placement = taira_validator_placement_from_table(&table.0, peer_index)?;
    insert_taira_inrou_validator_table(&mut table.0, peer_index, receipt)?;
    let mut rendered = Zeroizing::new(toml::to_string_pretty(&table.0).map_err(|_| {
        eyre!("failed to serialize peer{peer_index}.toml after exact V1 Inrou binding")
    })?);
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    if u64::try_from(rendered.as_bytes().len()).unwrap_or(u64::MAX) > MAX_TOML_SOURCE_BYTES {
        return Err(eyre!(
            "rendered peer{peer_index}.toml exceeds the {MAX_TOML_SOURCE_BYTES}-byte configuration-source limit"
        ));
    }
    let validation_table = toml::from_str(rendered.as_str()).map_err(|_| {
        eyre!("rendered peer{peer_index}.toml is not valid TOML after V1 Inrou binding")
    })?;
    let validated = actual::Root::from_toml_source(TomlSource::new_sensitive(
        directory.path.join(&name),
        validation_table,
        zeroize_taira_toml_table,
    ))
    .map_err(|_| {
        eyre!(
            "rendered peer{peer_index}.toml does not satisfy the current Iroha configuration schema"
        )
    })?;
    if validated.soracloud_runtime.inrou
        != expected_taira_inrou_validator_config(peer_index, receipt)?
    {
        return Err(eyre!(
            "rendered peer{peer_index}.toml does not project to the exact PortableVM V1 defaults"
        ));
    }
    Ok((
        placement,
        PreparedTairaValidatorConfig {
            peer_index,
            name,
            identity,
            rendered,
            temporary: None,
        },
    ))
}

#[cfg(unix)]
fn require_taira_validator_config_identity_at(
    directory: &TairaValidatorConfigDirectory,
    prepared: &PreparedTairaValidatorConfig,
) -> Result<()> {
    let identity = taira_validator_config_stat_at(
        directory,
        &prepared.name,
        "Taira validator config",
        MAX_TOML_SOURCE_BYTES,
    )?;
    if identity != prepared.identity {
        return Err(eyre!(
            "peer{}.toml changed before atomic V1 Inrou binding",
            prepared.peer_index
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn stage_taira_validator_config_replacement(
    directory: &TairaValidatorConfigDirectory,
    prepared: &PreparedTairaValidatorConfig,
) -> Result<StagedTairaValidatorConfig> {
    require_taira_validator_config_directory_identity(directory)?;
    for _ in 0..128 {
        let mut suffix = [0_u8; 16];
        OsRng
            .try_fill_bytes(&mut suffix)
            .map_err(|_| eyre!("OS randomness failed while staging a Taira validator config"))?;
        let name = format!(
            ".peer{}.inrou-v1-{}",
            prepared.peer_index,
            hex::encode(suffix)
        );
        let mut file = match rustix::fs::openat(
            &directory.file,
            &name,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        ) {
            Ok(file) => fs::File::from(file),
            Err(rustix::io::Errno::EXIST) => continue,
            Err(error) => {
                return Err(io::Error::from(error)).wrap_err_with(|| {
                    format!(
                        "create descriptor-relative temporary peer{}.toml",
                        prepared.peer_index
                    )
                });
            }
        };
        let created =
            taira_metadata_identity(&file.metadata().wrap_err_with(|| {
                format!("inspect new temporary peer{}.toml", prepared.peer_index)
            })?);
        let prepare = (|| -> Result<TairaFileIdentity> {
            rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR)
                .map_err(io::Error::from)
                .wrap_err_with(|| {
                    format!(
                        "set owner-only temporary peer{}.toml mode",
                        prepared.peer_index
                    )
                })?;
            let named_created_stat = rustix::fs::statat(
                &directory.file,
                &name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(io::Error::from)
            .wrap_err_with(|| format!("inspect new temporary peer{}.toml", prepared.peer_index))?;
            let named_created = taira_file_identity_from_stat(&named_created_stat)?;
            let opened_created = taira_metadata_identity(&file.metadata().wrap_err_with(|| {
                format!("reinspect new temporary peer{}.toml", prepared.peer_index)
            })?);
            if named_created != opened_created
                || opened_created.dev != created.dev
                || opened_created.ino != created.ino
                || rustix::fs::FileType::from_raw_mode(named_created_stat.st_mode)
                    != rustix::fs::FileType::RegularFile
                || opened_created.uid != rustix::process::geteuid().as_raw()
                || opened_created.mode & 0o7777 != 0o600
                || opened_created.nlink != 1
            {
                return Err(eyre!(
                    "temporary peer{}.toml changed identity while it was opened",
                    prepared.peer_index
                ));
            }
            file.write_all(prepared.rendered.as_bytes())
                .map_err(|_| eyre!("write bounded temporary peer{}.toml", prepared.peer_index))?;
            file.sync_all().map_err(|_| {
                eyre!(
                    "synchronize bounded temporary peer{}.toml",
                    prepared.peer_index
                )
            })?;
            let opened = taira_metadata_identity(&file.metadata().wrap_err_with(|| {
                format!(
                    "inspect synchronized temporary peer{}.toml",
                    prepared.peer_index
                )
            })?);
            let named = taira_validator_config_stat_at(
                directory,
                &name,
                "temporary Taira validator config",
                MAX_TOML_SOURCE_BYTES,
            )?;
            if opened != named
                || opened.dev != created.dev
                || opened.ino != created.ino
                || opened.len != u64::try_from(prepared.rendered.len()).unwrap_or(u64::MAX)
            {
                return Err(eyre!(
                    "temporary peer{}.toml did not retain its exact descriptor-bound identity",
                    prepared.peer_index
                ));
            }
            Ok(opened)
        })();
        match prepare {
            Ok(identity) => {
                return Ok(StagedTairaValidatorConfig {
                    name,
                    file,
                    identity,
                });
            }
            Err(error) => {
                remove_taira_validator_config_temp_at(directory, &name, created);
                return Err(error);
            }
        }
    }
    Err(eyre!(
        "failed to allocate an exclusive descriptor-relative Taira validator config staging file"
    ))
}

#[cfg(unix)]
fn require_staged_taira_validator_config_identity_at(
    directory: &TairaValidatorConfigDirectory,
    staged: &StagedTairaValidatorConfig,
    peer_index: usize,
) -> Result<()> {
    let opened = taira_metadata_identity(
        &staged
            .file
            .metadata()
            .wrap_err_with(|| format!("reinspect opened temporary peer{peer_index}.toml"))?,
    );
    let named = taira_validator_config_stat_at(
        directory,
        &staged.name,
        "temporary Taira validator config",
        MAX_TOML_SOURCE_BYTES,
    )?;
    if opened != staged.identity || named != opened {
        return Err(eyre!(
            "temporary peer{peer_index}.toml changed before atomic publication"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn remove_taira_validator_config_temp_at(
    directory: &TairaValidatorConfigDirectory,
    name: &str,
    expected: TairaFileIdentity,
) {
    let current = rustix::fs::statat(&directory.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW);
    if current
        .ok()
        .and_then(|stat| taira_file_identity_from_stat(&stat).ok())
        .is_some_and(|identity| identity.dev == expected.dev && identity.ino == expected.ino)
    {
        let _ = rustix::fs::unlinkat(&directory.file, name, rustix::fs::AtFlags::empty());
        let _ = directory.file.sync_all();
    }
}

#[cfg(unix)]
fn cleanup_taira_validator_config_temps(
    directory: &TairaValidatorConfigDirectory,
    prepared: &[PreparedTairaValidatorConfig],
) {
    for projected in prepared {
        if let Some(staged) = projected.temporary.as_ref() {
            remove_taira_validator_config_temp_at(directory, &staged.name, staged.identity);
        }
    }
}

/// Bind four freshly generated validator configs to one exact staged Inrou guest artifact.
///
/// This is a first-release transition: an existing Inrou table is always an error and no
/// idempotent or compatibility path is accepted.
#[cfg(unix)]
pub(crate) fn bind_taira_inrou_validator_configs(
    config_dir: &Path,
    receipt: &TairaInrouStageReceiptV1,
) -> Result<()> {
    validate_taira_inrou_config_receipt(receipt)?;
    let directory = open_taira_validator_config_directory(config_dir)?;
    let entries = exact_taira_validator_config_entries(&directory)?;
    let mut placements = BTreeSet::new();
    let mut validator_accounts = BTreeSet::new();
    let mut peer_ids = BTreeSet::new();
    let mut prepared = Vec::with_capacity(TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1);
    for entry in entries {
        let (placement, projected) = prepare_taira_validator_config(&directory, entry, receipt)?;
        validator_accounts.insert(placement.validator_account_id.clone());
        peer_ids.insert(placement.peer_id.clone());
        placements.insert(placement);
        prepared.push(projected);
    }
    if placements != receipt.placement_targets
        || validator_accounts.len() != TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
        || peer_ids.len() != TAIRA_INROU_VALIDATOR_CONFIG_COUNT_V1
    {
        return Err(eyre!(
            "Taira validator config placements do not match the exact staged Inrou placement set"
        ));
    }
    let binding = (|| -> Result<()> {
        for projected in &mut prepared {
            require_taira_validator_config_directory_identity(&directory)?;
            require_taira_validator_config_identity_at(&directory, projected)?;
            projected.temporary = Some(stage_taira_validator_config_replacement(
                &directory, projected,
            )?);
        }
        for projected in &prepared {
            require_taira_validator_config_identity_at(&directory, projected)?;
        }
        for projected in &mut prepared {
            require_taira_validator_config_directory_identity(&directory)?;
            let staged = projected
                .temporary
                .as_ref()
                .expect("every validated Taira config has one staged replacement");
            require_staged_taira_validator_config_identity_at(
                &directory,
                staged,
                projected.peer_index,
            )?;
            // Keep this identity check directly adjacent to `renameat`: the target must not
            // change between the final comparison and descriptor-relative publication.
            require_taira_validator_config_identity_at(&directory, projected)?;
            rustix::fs::renameat(
                &directory.file,
                &staged.name,
                &directory.file,
                &projected.name,
            )
            .map_err(io::Error::from)
            .wrap_err_with(|| {
                format!(
                    "atomically replace peer{}.toml with its V1 Inrou binding",
                    projected.peer_index
                )
            })?;
            let installed_opened =
                taira_metadata_identity(&staged.file.metadata().wrap_err_with(|| {
                    format!("inspect installed peer{}.toml", projected.peer_index)
                })?);
            let installed_named = taira_validator_config_stat_at(
                &directory,
                &projected.name,
                "installed Taira validator config",
                MAX_TOML_SOURCE_BYTES,
            )?;
            if installed_opened != installed_named
                || installed_opened.dev != staged.identity.dev
                || installed_opened.ino != staged.identity.ino
            {
                return Err(eyre!(
                    "installed peer{}.toml changed identity during atomic publication",
                    projected.peer_index
                ));
            }
            directory.file.sync_all().wrap_err_with(|| {
                format!(
                    "synchronize retained Taira validator config directory after peer{}.toml replacement",
                    projected.peer_index
                )
            })?;
            let (installed_bytes, installed_identity) = read_sensitive_taira_validator_config_at(
                &directory,
                &projected.name,
                "installed Taira validator config",
                MAX_TOML_SOURCE_BYTES,
            )?;
            if installed_identity != installed_opened
                || installed_bytes.as_slice() != projected.rendered.as_bytes()
            {
                return Err(eyre!(
                    "installed peer{}.toml differs from its validated V1 Inrou projection",
                    projected.peer_index
                ));
            }
        }
        Ok(())
    })();
    if binding.is_err() {
        cleanup_taira_validator_config_temps(&directory, &prepared);
    }
    binding
}

#[cfg(not(unix))]
pub(crate) fn bind_taira_inrou_validator_configs(
    _config_dir: &Path,
    _receipt: &TairaInrouStageReceiptV1,
) -> Result<()> {
    Err(eyre!(
        "Taira Inrou validator config binding requires owner-only Unix file custody"
    ))
}

fn validate_taira_stage_layout(
    receipt: &TairaInrouStageReceiptV1,
    expected_mode: MutationMode,
) -> Result<()> {
    let expected = [
        (
            "container_file",
            receipt.container_file.as_str(),
            TAIRA_INROU_STAGE_CONTAINER_FILE_V1,
        ),
        (
            "service_file",
            receipt.service_file.as_str(),
            TAIRA_INROU_STAGE_SERVICE_FILE_V1,
        ),
        (
            "bundle_payload_file",
            receipt.bundle_payload_file.as_str(),
            TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1,
        ),
        (
            "bundle_manifest_file",
            receipt.bundle_manifest_file.as_str(),
            TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1,
        ),
        (
            "guest_payload_dir",
            receipt.guest_payload_dir.as_str(),
            TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1,
        ),
        (
            "guest_manifest_file",
            receipt.guest_manifest_file.as_str(),
            TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1,
        ),
        (
            "discovery_payload_dir",
            receipt.discovery_payload_dir.as_str(),
            TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1,
        ),
        (
            "discovery_manifest_file",
            receipt.discovery_manifest_file.as_str(),
            TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1,
        ),
    ];
    for (field, actual, canonical) in expected {
        if actual != canonical {
            return Err(eyre!(
                "Taira Inrou stage receipt {field} must be canonical `{canonical}`, found `{actual}`"
            ));
        }
    }
    if receipt.schema_version != TAIRA_INROU_STAGE_SCHEMA_VERSION_V1 {
        return Err(eyre!(
            "Taira Inrou stage schema must be {}, found {}",
            TAIRA_INROU_STAGE_SCHEMA_VERSION_V1,
            receipt.schema_version
        ));
    }
    if receipt.sorafs_retention_epoch == 0 {
        return Err(eyre!(
            "Taira Inrou stage SoraFS retention epoch must be nonzero"
        ));
    }
    if receipt.placement_targets.len() != 4 {
        return Err(eyre!(
            "Taira Inrou stage receipt requires exactly four distinct placement targets"
        ));
    }
    for target in &receipt.placement_targets {
        target.validate()?;
    }
    if receipt.mutation_mode != expected_mode.label_lowercase()
        || !is_taira_inrou_canary_service_version(&receipt.service_version)
    {
        return Err(eyre!(
            "Taira Inrou stage must be an artifact-derived {} revision, found mode `{}` revision `{}`",
            expected_mode.label_lowercase(),
            receipt.mutation_mode,
            receipt.service_version
        ));
    }
    if receipt.guest_isa != SoraInrouGuestIsaV1::Aarch64.as_str() {
        return Err(eyre!(
            "Taira Inrou stage guest ISA must be `{}`",
            SoraInrouGuestIsaV1::Aarch64.as_str()
        ));
    }
    Ok(())
}
fn validate_taira_stage_owned_entry(path: &Path, directory: bool, description: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("inspect {description} {}", path.display()))?;
    let expected_kind = if directory {
        "directory"
    } else {
        "regular file"
    };
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(eyre!(
            "{description} {} must be one direct {expected_kind}",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let mode = metadata.permissions().mode() & 0o7777;
        // Prepared stages are owner-writable; authorization-bound runtime snapshots
        // are frozen owner-readable. Reading either must retain private custody.
        let accepted_mode = if directory {
            mode == 0o700
        } else {
            matches!(mode, 0o400 | 0o600)
        };
        if metadata.uid() != rustix::process::geteuid().as_raw() || !accepted_mode {
            let expected_mode = if directory { "0700" } else { "0400 or 0600" };
            return Err(eyre!(
                "{description} {} must be owned by the effective user with mode {expected_mode}",
                path.display()
            ));
        }
        if !directory && metadata.nlink() != 1 {
            return Err(eyre!(
                "{description} {} must have exactly one hard link",
                path.display()
            ));
        }
    }
    Ok(())
}
fn taira_stage_owned_file_bytes(path: &Path, description: &str, max_bytes: u64) -> Result<Vec<u8>> {
    validate_taira_stage_owned_entry(path, false, description)?;
    let bytes = taira_stage_regular_file_bytes(path, description, max_bytes)?;
    validate_taira_stage_owned_entry(path, false, description)?;
    Ok(bytes)
}
fn validate_exact_taira_stage_tree(stage_dir: &Path, member_paths: &[String]) -> Result<()> {
    let mut expected = BTreeMap::<PathBuf, bool>::new();
    for directory in [
        PathBuf::from("manifests"),
        PathBuf::from("payloads"),
        PathBuf::from(TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1),
        PathBuf::from(TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1),
    ] {
        expected.insert(directory, true);
    }
    for file in [
        TAIRA_INROU_STAGE_RECEIPT_FILE_V1,
        TAIRA_INROU_STAGE_CONTAINER_FILE_V1,
        TAIRA_INROU_STAGE_SERVICE_FILE_V1,
        TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1,
        TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1,
        TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1,
        TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1,
        TAIRA_INROU_STAGE_DISCOVERY_DOCUMENT_FILE_V1,
    ] {
        expected.insert(PathBuf::from(file), false);
    }
    for logical_path in taira_stage_logical_member_paths(member_paths)? {
        let mut relative = PathBuf::from(TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1);
        for (index, component) in logical_path.iter().enumerate() {
            relative.push(component);
            expected.insert(relative.clone(), index + 1 != logical_path.len());
        }
    }
    fn visit(
        stage_dir: &Path,
        relative_dir: &Path,
        expected: &BTreeMap<PathBuf, bool>,
        seen: &mut BTreeSet<PathBuf>,
    ) -> Result<()> {
        let absolute_dir = stage_dir.join(relative_dir);
        for entry in fs::read_dir(&absolute_dir)
            .wrap_err_with(|| format!("read Taira stage directory {}", absolute_dir.display()))?
        {
            let entry = entry.wrap_err_with(|| {
                format!("read Taira stage entry in {}", absolute_dir.display())
            })?;
            let relative = relative_dir.join(entry.file_name());
            let Some(directory) = expected.get(&relative).copied() else {
                return Err(eyre!(
                    "Taira Inrou stage contains unexpected entry `{}`",
                    relative.display()
                ));
            };
            if !seen.insert(relative.clone()) {
                return Err(eyre!(
                    "Taira Inrou stage contains duplicate entry `{}`",
                    relative.display()
                ));
            }
            validate_taira_stage_owned_entry(&entry.path(), directory, "Taira Inrou staged entry")?;
            if directory {
                visit(stage_dir, &relative, expected, seen)?;
            }
        }
        Ok(())
    }
    let mut seen = BTreeSet::new();
    visit(stage_dir, Path::new(""), &expected, &mut seen)?;
    if seen.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|path| !seen.contains(*path))
            .expect("different exact-tree cardinality has one missing entry");
        return Err(eyre!(
            "Taira Inrou stage is missing canonical entry `{}`",
            missing.display()
        ));
    }
    Ok(())
}
fn taira_release_signer_bytes(key_pair: &KeyPair) -> Result<[u8; 32]> {
    let (_, signer_bytes) = key_pair
        .public_key()
        .try_to_bytes()
        .wrap_err("Taira SoraFS release signer public key is malformed")?;
    signer_bytes.try_into().map_err(|_| {
        eyre!(
            "Taira SoraFS release signer must be a 32-byte Ed25519 key, got {} bytes",
            signer_bytes.len()
        )
    })
}
fn load_taira_stage_manifest(
    manifest_path: &Path,
    expected_digest_hex: &str,
    expected_content_cid: &str,
    expected_signer: &[u8; 32],
    description: &str,
) -> Result<BuiltSorafsManifest> {
    let bytes = taira_stage_owned_file_bytes(
        manifest_path,
        description,
        sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES as u64,
    )?;
    let manifest = sorafs_manifest::decode_manifest_v1_canonical(&bytes)
        .wrap_err_with(|| format!("decode canonical staged {description}"))?;
    let policy = PinPolicyConstraints {
        require_council_signatures: true,
        ..PinPolicyConstraints::default()
    };
    validate_manifest(&manifest, &policy)
        .wrap_err_with(|| format!("validate canonical staged {description}"))?;
    if manifest.governance.council_signatures.len() != 1
        || manifest.governance.council_signatures[0].signer != *expected_signer
    {
        return Err(eyre!(
            "staged {description} must carry exactly one governance signature from the active Taira signer"
        ));
    }
    let digest_hex = hex::encode(
        manifest
            .digest()
            .wrap_err_with(|| format!("digest staged {description}"))?
            .as_bytes(),
    );
    if digest_hex != expected_digest_hex {
        return Err(eyre!(
            "staged {description} manifest digest {digest_hex} differs from receipt {expected_digest_hex}"
        ));
    }
    let content_cid = encode_content_cid(&manifest.root_cid);
    if content_cid != expected_content_cid {
        return Err(eyre!(
            "staged {description} content CID {content_cid} differs from receipt {expected_content_cid}"
        ));
    }
    Ok(BuiltSorafsManifest {
        manifest,
        bytes,
        digest_hex,
    })
}
fn load_and_verify_taira_stage_manifest(
    manifest_path: &Path,
    plan: &CarBuildPlan,
    payload: &[u8],
    expected_digest_hex: &str,
    expected_content_cid: &str,
    expected_signer: &[u8; 32],
    description: &str,
) -> Result<BuiltSorafsManifest> {
    let built = load_taira_stage_manifest(
        manifest_path,
        expected_digest_hex,
        expected_content_cid,
        expected_signer,
        description,
    )?;
    let writer = CarWriter::new(plan, payload)
        .wrap_err_with(|| format!("construct staged {description} CAR verifier"))?;
    let mut car = Vec::new();
    writer
        .write_to(&mut car)
        .wrap_err_with(|| format!("materialize staged {description} CAR for verification"))?;
    CarVerifier::verify_full_car_with_plan(&built.manifest, plan, &car)
        .wrap_err_with(|| format!("verify staged {description} manifest against exact payload"))?;
    Ok(built)
}
fn load_and_verify_taira_stage_directory_manifest(
    manifest_path: &Path,
    plan: &CarBuildPlan,
    payload_dir: &Path,
    expected_digest_hex: &str,
    expected_content_cid: &str,
    expected_signer: &[u8; 32],
    description: &str,
) -> Result<BuiltSorafsManifest> {
    let built = load_taira_stage_manifest(
        manifest_path,
        expected_digest_hex,
        expected_content_cid,
        expected_signer,
        description,
    )?;
    let descriptor = chunker_registry::default_descriptor();
    if built.manifest.chunking != ChunkingProfileV1::from_descriptor(descriptor)
        || plan.chunk_profile != descriptor.profile
        || built.manifest.content_length != plan.content_length
        || built.manifest.chunk_digest_sha3_256 != compute_chunk_digest_sha3(&plan.chunks)
    {
        return Err(eyre!(
            "staged {description} manifest geometry does not match the exact directory plan"
        ));
    }
    let mut car_source = DirectoryPayload::new(payload_dir, &plan.files)
        .wrap_err_with(|| format!("open staged {description} CAR verification source"))?;
    let mut car_reader = TairaSequentialPayloadReader::new(&mut car_source, plan.content_length);
    let stats = CarStreamingWriter::new(plan)
        .write_from_reader(&mut car_reader, io::sink())
        .wrap_err_with(|| format!("rebuild staged {description} canonical CAR"))?;
    car_reader
        .finish()
        .wrap_err_with(|| format!("finish staged {description} CAR verification source"))?;
    if stats.root_cids != vec![built.manifest.root_cid.clone()]
        || stats.dag_codec != built.manifest.dag_codec.0
        || stats.payload_bytes != plan.content_length
        || stats.chunk_count != plan.chunks.len()
        || stats.car_size != built.manifest.car_size
        || stats.car_archive_digest.as_bytes() != &built.manifest.car_digest
    {
        return Err(eyre!(
            "staged {description} canonical CAR commitments do not match the manifest"
        ));
    }
    let mut por_source = DirectoryPayload::new(payload_dir, &plan.files)
        .wrap_err_with(|| format!("open staged {description} PoR verification source"))?;
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    store
        .ingest_plan_source(plan, &mut por_source)
        .wrap_err_with(|| format!("rebuild staged {description} PoR tree"))?;
    PayloadSource::ensure_exhausted(&mut por_source, plan.content_length)
        .map_err(|error| eyre!("validate staged {description} PoR source: {error}"))?;
    if store.por_tree().root() != &built.manifest.por_root {
        return Err(eyre!(
            "staged {description} PoR root does not match the exact directory payload"
        ));
    }
    Ok(built)
}
fn load_verified_taira_inrou_stage(
    stage_dir: &Path,
    key_pair: &KeyPair,
    expected_mode: MutationMode,
) -> Result<VerifiedTairaInrouStage> {
    validate_taira_stage_owned_entry(stage_dir, true, "Taira Inrou stage")?;
    let receipt_path = stage_dir.join(TAIRA_INROU_STAGE_RECEIPT_FILE_V1);
    let receipt_bytes = taira_stage_owned_file_bytes(
        &receipt_path,
        "Taira Inrou stage receipt",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let receipt: TairaInrouStageReceiptV1 = decode_taira_stage_json(&receipt_bytes, &receipt_path)?;
    validate_taira_stage_layout(&receipt, expected_mode)?;
    let container_path = stage_dir.join(TAIRA_INROU_STAGE_CONTAINER_FILE_V1);
    let service_path = stage_dir.join(TAIRA_INROU_STAGE_SERVICE_FILE_V1);
    let container: SoraContainerManifestV1 = decode_taira_stage_json(
        &taira_stage_owned_file_bytes(
            &container_path,
            "Taira staged container manifest",
            TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
        )?,
        &container_path,
    )?;
    let service: SoraServiceManifestV1 = decode_taira_stage_json(
        &taira_stage_owned_file_bytes(
            &service_path,
            "Taira staged service manifest",
            TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
        )?,
        &service_path,
    )?;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    validate_taira_inrou_canary_bundle(&bundle)?;
    let image = bundle
        .container
        .inrou
        .as_ref()
        .and_then(|inrou| inrou.guest_images.get(&SoraInrouGuestIsaV1::Aarch64))
        .expect("validated Taira bundle has one AArch64 image");
    let member_paths = vec![
        inrou_member_path(&image.kernel_image_path)?,
        inrou_member_path(&image.rootfs_image_path)?,
        inrou_member_path(
            image
                .initrd_image_path
                .as_deref()
                .expect("validated Taira AArch64 image has an initrd"),
        )?,
    ];
    validate_exact_taira_stage_tree(stage_dir, &member_paths)?;
    if bundle.service.placement_targets != receipt.placement_targets
        || bundle.service.service_name.as_ref() != receipt.service_name
        || bundle.service.service_version != receipt.service_version
        || bundle.container_manifest_hash().to_string() != receipt.container_manifest_hash
        || bundle.service_manifest_hash().to_string() != receipt.service_manifest_hash
    {
        return Err(eyre!(
            "Taira Inrou staged manifests do not match the exact receipt identity"
        ));
    }
    let bundle_payload = taira_stage_owned_file_bytes(
        &stage_dir.join(TAIRA_INROU_STAGE_BUNDLE_PAYLOAD_FILE_V1),
        "staged Taira bundle payload",
        INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES,
    )?;
    validate_taira_inrou_canary_bundle_payload(&bundle_payload)?;
    let bundle_hash = Hash::new(&bundle_payload);
    if bundle_hash != bundle.container.bundle_hash || bundle_hash.to_string() != receipt.bundle_hash
    {
        return Err(eyre!(
            "Taira Inrou staged bundle payload does not match its admitted and receipted hash"
        ));
    }
    let descriptor = chunker_registry::default_descriptor();
    let bundle_plan = CarBuildPlan::single_file_with_profile(&bundle_payload, descriptor.profile)
        .map_err(|error| eyre!("rebuild staged Taira bundle plan: {error}"))?;
    let expected_signer = taira_release_signer_bytes(key_pair)?;
    let bundle_manifest = load_and_verify_taira_stage_manifest(
        &stage_dir.join(TAIRA_INROU_STAGE_BUNDLE_MANIFEST_FILE_V1),
        &bundle_plan,
        &bundle_payload,
        &receipt.bundle_manifest_digest_hex,
        &receipt.bundle_content_cid,
        &expected_signer,
        "Taira bundle",
    )?;
    let release_identity = SorafsReleaseIdentityV1::new(
        NonZeroU64::new(receipt.sorafs_retention_epoch)
            .expect("validated Taira stage retention epoch is nonzero"),
    );
    validate_sorafs_release_identity(
        &bundle_manifest.manifest,
        release_identity,
        "staged Taira bundle manifest",
    )?;
    let guest_payload_dir = stage_dir.join(TAIRA_INROU_STAGE_GUEST_PAYLOAD_DIR_V1);
    let guest_plan =
        taira_streaming_directory_plan(&guest_payload_dir, &member_paths, descriptor.profile)?;
    let guest_manifest = load_and_verify_taira_stage_directory_manifest(
        &stage_dir.join(TAIRA_INROU_STAGE_GUEST_MANIFEST_FILE_V1),
        &guest_plan,
        &guest_payload_dir,
        &receipt.guest_manifest_digest_hex,
        &receipt.guest_content_cid,
        &expected_signer,
        "Taira guest image",
    )?;
    validate_sorafs_release_identity(
        &guest_manifest.manifest,
        release_identity,
        "staged Taira guest-image manifest",
    )?;
    let published = bundle
        .container
        .inrou
        .as_ref()
        .and_then(|inrou| inrou.guest_images.get(&SoraInrouGuestIsaV1::Aarch64))
        .map(|image| &image.published_artifact)
        .ok_or_else(|| eyre!("Taira Inrou staged container lacks its published AArch64 ref"))?;
    if published.manifest_digest_hex != receipt.guest_manifest_digest_hex
        || published.content_cid != receipt.guest_content_cid
    {
        return Err(eyre!(
            "Taira Inrou staged AArch64 ref does not match the exact staged manifest"
        ));
    }
    let discovery_payload_dir = stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_PAYLOAD_DIR_V1);
    let discovery_document_path = stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_DOCUMENT_FILE_V1);
    let discovery_document_bytes = taira_stage_owned_file_bytes(
        &discovery_document_path,
        "Taira staged public discovery document",
        TAIRA_INROU_STAGE_SOURCE_MANIFEST_MAX_BYTES_V1,
    )?;
    let decoded_discovery_document: SoracloudPublicServiceDiscoveryDocumentV1 =
        json::from_slice(&discovery_document_bytes)
            .wrap_err("decode canonical staged Taira public discovery document")?;
    if json::to_vec(&decoded_discovery_document)
        .wrap_err("re-encode canonical staged Taira public discovery document")?
        != discovery_document_bytes
    {
        return Err(eyre!(
            "Taira public discovery document must be canonical compact V1 JSON"
        ));
    }
    let (discovery, discovery_publication, expected_discovery_artifact) =
        prepare_public_service_discovery(&bundle, key_pair, release_identity)?;
    if expected_discovery_artifact.payload != discovery_document_bytes
        || Hash::new(&discovery_document_bytes).to_string() != receipt.discovery_document_hash
        || discovery.document_hash.to_string() != receipt.discovery_document_hash
        || discovery.content_cid != receipt.discovery_content_cid
        || discovery.manifest_digest_hex != receipt.discovery_manifest_digest_hex
        || discovery.public_discovery_url != receipt.public_discovery_url
        || discovery.public_discovery_cid_host_url != receipt.public_discovery_cid_host_url
        || discovery_publication.public_discovery_url != receipt.public_discovery_url
        || discovery_publication.public_discovery_cid_host_url
            != receipt.public_discovery_cid_host_url
    {
        return Err(eyre!(
            "Taira public discovery document or projection differs from its exact staged receipt"
        ));
    }
    let (discovery_plan, discovery_payload) =
        CarBuildPlan::from_directory_with_profile(&discovery_payload_dir, descriptor.profile)
            .map_err(|error| eyre!("rebuild staged Taira public discovery plan: {error}"))?;
    if discovery_plan != expected_discovery_artifact.plan
        || discovery_payload != discovery_document_bytes
    {
        return Err(eyre!(
            "Taira public discovery directory differs from its exact canonical one-file plan"
        ));
    }
    let discovery_manifest = load_and_verify_taira_stage_directory_manifest(
        &stage_dir.join(TAIRA_INROU_STAGE_DISCOVERY_MANIFEST_FILE_V1),
        &discovery_plan,
        &discovery_payload_dir,
        &receipt.discovery_manifest_digest_hex,
        &receipt.discovery_content_cid,
        &expected_signer,
        "Taira public discovery",
    )?;
    validate_sorafs_release_identity(
        &discovery_manifest.manifest,
        release_identity,
        "staged Taira public-discovery manifest",
    )?;
    if discovery_manifest.bytes != expected_discovery_artifact.built.bytes {
        return Err(eyre!(
            "Taira public-discovery manifest differs from the deterministic staged authority"
        ));
    }
    Ok(VerifiedTairaInrouStage {
        receipt,
        bundle,
        bundle_manifest,
        guest_manifest,
        discovery,
        discovery_manifest,
    })
}
/// Immutable identity recovered by fully revalidating one retained Taira Inrou stage.
#[derive(Clone, Debug)]
pub(crate) struct TairaInrouStageIdentity {
    pub service_name: String,
    pub service_version: String,
    pub route_host: String,
    pub route_path_prefix: String,
    pub healthcheck_path: String,
    pub stage_mode: String,
    pub bundle_hash: String,
    pub deployment_bundle_hash: String,
    pub bundle_content_cid: String,
    pub bundle_manifest_digest_hex: String,
    pub guest_content_cid: String,
    pub guest_manifest_digest_hex: String,
    pub discovery_payload_dir: String,
    pub discovery_document_hash: String,
    pub discovery_content_cid: String,
    pub discovery_manifest_digest_hex: String,
    pub public_discovery_url: String,
    pub public_discovery_cid_host_url: String,
    pub container_manifest_hash: String,
    pub service_manifest_hash: String,
    pub placement_targets: BTreeSet<SoraInrouPlacementTargetV1>,
}
/// Revalidate a retained Taira Inrou stage without registering artifacts or submitting a mutation.
pub(crate) fn load_taira_inrou_stage_identity(
    config: &ClientConfig,
    stage_dir: &Path,
    requested_mode: crate::taira::InrouCanaryMode,
) -> Result<TairaInrouStageIdentity> {
    let mode = MutationMode::from(requested_mode);
    let staged = load_verified_taira_inrou_stage(stage_dir, &config.key_pair, mode)?;
    let route = staged
        .bundle
        .service
        .route
        .as_ref()
        .expect("verified Taira Inrou stage has a public route");
    let healthcheck_path = staged
        .bundle
        .container
        .lifecycle
        .healthcheck_path
        .clone()
        .expect("verified Taira Inrou stage has /health");
    let deployment_bundle_hash = Hash::new(Encode::encode(&staged.bundle)).to_string();
    Ok(TairaInrouStageIdentity {
        service_name: staged.bundle.service.service_name.to_string(),
        service_version: staged.receipt.service_version,
        route_host: route.host.clone(),
        route_path_prefix: route.path_prefix.clone(),
        healthcheck_path,
        stage_mode: mode.label_lowercase().to_owned(),
        bundle_hash: staged.receipt.bundle_hash,
        deployment_bundle_hash,
        bundle_content_cid: staged.receipt.bundle_content_cid,
        bundle_manifest_digest_hex: staged.receipt.bundle_manifest_digest_hex,
        guest_content_cid: staged.receipt.guest_content_cid,
        guest_manifest_digest_hex: staged.receipt.guest_manifest_digest_hex,
        discovery_payload_dir: staged.receipt.discovery_payload_dir,
        discovery_document_hash: staged.receipt.discovery_document_hash,
        discovery_content_cid: staged.receipt.discovery_content_cid,
        discovery_manifest_digest_hex: staged.receipt.discovery_manifest_digest_hex,
        public_discovery_url: staged.receipt.public_discovery_url,
        public_discovery_cid_host_url: staged.receipt.public_discovery_cid_host_url,
        container_manifest_hash: staged.receipt.container_manifest_hash,
        service_manifest_hash: staged.receipt.service_manifest_hash,
        placement_targets: staged.receipt.placement_targets,
    })
}

fn derive_service_mutation_precondition(
    status: &json::Value,
    service_name: &str,
    service_version: &str,
    mode: MutationMode,
    context: &str,
) -> Result<SoraServiceMutationPreconditionV1> {
    let services = status
        .get("control_plane")
        .and_then(json::Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("{context} mutation preflight is missing control-plane services"))?;
    let mut matching = services.iter().filter(|service| {
        service.get("service_name").and_then(json::Value::as_str) == Some(service_name)
    });
    let current = matching.next();
    if matching.next().is_some() {
        return Err(eyre!(
            "{context} mutation preflight found duplicate service `{service_name}` snapshots"
        ));
    }
    match (mode, current) {
        (MutationMode::Deploy, None) => Ok(SoraServiceMutationPreconditionV1::ServiceAbsent),
        (MutationMode::Deploy, Some(_)) => Err(eyre!(
            "{context} deploy requires service `{service_name}` to be absent before artifact publication"
        )),
        (MutationMode::Upgrade, None) => Err(eyre!(
            "{context} upgrade requires service `{service_name}` to exist before artifact publication"
        )),
        (MutationMode::Upgrade, Some(service)) => {
            let current_version = service
                .get("current_version")
                .and_then(json::Value::as_str)
                .filter(|version| !version.trim().is_empty())
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no current version for service `{service_name}`"
                    )
                })?;
            if current_version == service_version {
                return Err(eyre!(
                    "{context} upgrade refuses already-current immutable revision `{service_version}` before artifact publication"
                ));
            }
            if service
                .get("active_rollout")
                .is_some_and(|rollout| !matches!(rollout, json::Value::Null))
            {
                return Err(eyre!(
                    "{context} upgrade refuses to supersede the active rollout for service `{service_name}` before artifact publication"
                ));
            }
            let revision = service
                .get("latest_revision")
                .and_then(json::Value::as_object)
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no exact current revision for service `{service_name}`"
                    )
                })?;
            let revision_version = revision
                .get("service_version")
                .and_then(json::Value::as_str)
                .filter(|version| !version.trim().is_empty())
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no exact revision version for service `{service_name}`"
                    )
                })?;
            if revision_version != current_version {
                return Err(eyre!(
                    "{context} upgrade preflight current version and latest revision disagree for service `{service_name}`"
                ));
            }
            // Status hashes use the canonical Norito JSON literal, including its
            // type tag and checksum; Hash::from_str accepts a different CLI format.
            let service_manifest_hash = json::from_value::<Hash>(
                revision.get("service_manifest_hash").cloned().ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no service manifest hash for service `{service_name}`"
                    )
                })?,
            )
            .wrap_err_with(|| {
                    format!(
                        "{context} upgrade preflight found an invalid service manifest hash for service `{service_name}`"
                    )
                })?;
            let container_manifest_hash = json::from_value::<Hash>(
                revision.get("container_manifest_hash").cloned().ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no container manifest hash for service `{service_name}`"
                    )
                })?,
            )
            .wrap_err_with(|| {
                    format!(
                        "{context} upgrade preflight found an invalid container manifest hash for service `{service_name}`"
                    )
                })?;
            let process_generation = revision
                .get("process_generation")
                .and_then(json::Value::as_u64)
                .filter(|generation| *generation > 0)
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no positive process generation for service `{service_name}`"
                    )
                })?;
            let config_generation = service
                .get("config_generation")
                .and_then(json::Value::as_u64)
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no config generation for service `{service_name}`"
                    )
                })?;
            let secret_generation = service
                .get("secret_generation")
                .and_then(json::Value::as_u64)
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no secret generation for service `{service_name}`"
                    )
                })?;
            Ok(SoraServiceMutationPreconditionV1::ExactCurrentRevision(
                SoraServiceExactCurrentRevisionPreconditionV1 {
                    service_version: current_version.to_owned(),
                    service_manifest_hash,
                    container_manifest_hash,
                    process_generation,
                    config_generation,
                    secret_generation,
                },
            ))
        }
    }
}
fn derive_app_infra_mutation_precondition(
    status: &json::Value,
    app_name: &str,
    app_version: &str,
    mode: MutationMode,
    context: &str,
) -> Result<SoraAppInfraMutationPreconditionV1> {
    let apps = status
        .get("apps")
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("{context} mutation preflight is missing authoritative apps"))?;
    let mut matching = apps
        .iter()
        .filter(|app| app.get("app_name").and_then(json::Value::as_str) == Some(app_name));
    let current = matching.next();
    if matching.next().is_some() {
        return Err(eyre!(
            "{context} mutation preflight found duplicate app `{app_name}` snapshots"
        ));
    }
    match (mode, current) {
        (MutationMode::Deploy, None) => Ok(SoraAppInfraMutationPreconditionV1::AppAbsent),
        (MutationMode::Deploy, Some(_)) => Err(eyre!(
            "{context} deploy requires app `{app_name}` to be absent before artifact publication"
        )),
        (MutationMode::Upgrade, None) => Err(eyre!(
            "{context} upgrade requires app `{app_name}` to exist before artifact publication"
        )),
        (MutationMode::Upgrade, Some(app)) => {
            let current_app_version = app
                .get("current_app_version")
                .and_then(json::Value::as_str)
                .filter(|version| !version.trim().is_empty())
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no current version for app `{app_name}`"
                    )
                })?;
            if current_app_version == app_version {
                return Err(eyre!(
                    "{context} upgrade refuses already-current app revision `{app_version}` before artifact publication"
                ));
            }
            let manifest_hash = json::from_value::<Hash>(
                app.get("current_manifest_hash").cloned().ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no current manifest hash for app `{app_name}`"
                    )
                })?,
            )
            .wrap_err_with(|| {
                    format!(
                        "{context} upgrade preflight found an invalid manifest hash for app `{app_name}`"
                    )
                })?;
            let revision_count = app
                .get("revision_count")
                .and_then(json::Value::as_u64)
                .and_then(|count| u32::try_from(count).ok())
                .filter(|count| *count > 0)
                .ok_or_else(|| {
                    eyre!(
                        "{context} upgrade preflight found no positive revision count for app `{app_name}`"
                    )
                })?;
            Ok(SoraAppInfraMutationPreconditionV1::ExactCurrentRevision(
                SoraAppInfraExactCurrentRevisionPreconditionV1 {
                    app_version: current_app_version.to_owned(),
                    manifest_hash,
                    revision_count,
                },
            ))
        }
    }
}

fn status_tagged_enum_name<'a>(value: &'a json::Value, field: &str) -> Option<&'a str> {
    value.as_object()?.get(field)?.as_str()
}
fn preflight_service_upgrade_identity(
    status: &json::Value,
    service_manifest: &SoraServiceManifestV1,
    container_runtime: SoraContainerRuntimeV1,
    mode: MutationMode,
    context: &str,
) -> Result<()> {
    if mode == MutationMode::Deploy {
        return Ok(());
    }
    let service_name = service_manifest.service_name.as_ref();
    let service = status
        .get("control_plane")
        .and_then(json::Value::as_object)
        .and_then(|control_plane| control_plane.get("services"))
        .and_then(json::Value::as_array)
        .and_then(|services| {
            services.iter().find(|service| {
                service.get("service_name").and_then(json::Value::as_str) == Some(service_name)
            })
        })
        .ok_or_else(|| {
            eyre!(
                "{context} upgrade preflight found no authoritative service identity for `{service_name}`"
            )
        })?;
    let revision = service
        .get("latest_revision")
        .and_then(json::Value::as_object)
        .ok_or_else(|| {
            eyre!(
                "{context} upgrade preflight found no authoritative route identity for service `{service_name}`"
            )
        })?;
    let expected_execution_plane = format!("{:?}", service_manifest.execution_plane);
    let expected_runtime = format!("{container_runtime:?}");
    let execution_identity_matches = revision
        .get("execution_plane")
        .and_then(|value| status_tagged_enum_name(value, "execution_plane"))
        == Some(expected_execution_plane.as_str())
        && revision
            .get("runtime")
            .and_then(|value| status_tagged_enum_name(value, "runtime"))
            == Some(expected_runtime.as_str());
    let route_identity_matches = service_manifest.route.as_ref().map_or_else(
        || {
            [
                "route_host",
                "route_path_prefix",
                "route_service_port",
                "route_visibility",
                "route_tls_mode",
            ]
            .iter()
            .all(|field| {
                revision
                    .get(*field)
                    .is_none_or(|value| matches!(value, json::Value::Null))
            })
        },
        |route| {
            let expected_visibility = format!("{:?}", route.visibility);
            let expected_tls_mode = format!("{:?}", route.tls_mode);
            revision.get("route_host").and_then(json::Value::as_str) == Some(route.host.as_str())
                && revision
                    .get("route_path_prefix")
                    .and_then(json::Value::as_str)
                    == Some(route.path_prefix.as_str())
                && revision
                    .get("route_service_port")
                    .and_then(json::Value::as_u64)
                    == Some(u64::from(route.service_port.get()))
                && revision
                    .get("route_visibility")
                    .and_then(json::Value::as_str)
                    == Some(expected_visibility.as_str())
                && revision.get("route_tls_mode").and_then(json::Value::as_str)
                    == Some(expected_tls_mode.as_str())
        },
    );
    if !execution_identity_matches || !route_identity_matches {
        return Err(eyre!(
            "{context} upgrade cannot change route identity, execution plane, or container runtime for service `{service_name}` before artifact publication"
        ));
    }
    Ok(())
}

fn preflight_taira_inrou_mutation_target(
    status: &json::Value,
    service_name: &str,
    service_version: &str,
    mode: MutationMode,
) -> Result<SoraServiceMutationPreconditionV1> {
    derive_service_mutation_precondition(status, service_name, service_version, mode, "Taira Inrou")
}

/// One exact ledger mutation in the ordered Taira Inrou canary protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TairaInrouCanaryPreparedOperationV1 {
    /// Register the canonical bundle manifest.
    BundlePin,
    /// Register the canonical guest-image manifest.
    GuestPin,
    /// Register the canonical public-discovery manifest.
    DiscoveryPin,
    /// Deploy or upgrade the canary service after all manifests are registered.
    ServiceMutation,
}

/// Exact finalized-governance readiness of one Taira Inrou canary pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TairaInrouCanaryPinReadinessV1 {
    /// The exact manifest is absent from the finalized pin registry.
    Missing,
    /// The exact manifest exists but has not reached governance approval.
    Pending,
    /// The exact manifest is approved at the bound finalized epoch.
    Approved(u64),
}

impl TairaInrouCanaryPreparedOperationV1 {
    /// Closed operation label committed by the prepared transaction.
    pub(crate) const fn operation_label(self) -> &'static str {
        match self {
            Self::BundlePin => "bundle_pin",
            Self::GuestPin => "guest_pin",
            Self::DiscoveryPin => "discovery_pin",
            Self::ServiceMutation => "service_mutation",
        }
    }

    /// Recovery-plan child kind bound into the transaction metadata.
    pub(crate) const fn mutation_kind(self) -> &'static str {
        match self {
            Self::BundlePin => "inrou_bundle_pin",
            Self::GuestPin => "inrou_guest_pin",
            Self::DiscoveryPin => "inrou_discovery_pin",
            Self::ServiceMutation => "inrou_canary",
        }
    }
}

/// Authenticate the executable identity of one prepared public-reset Inrou transaction.
///
/// This closes the retained stage identity over the actual signed instruction instead of
/// accepting a transaction merely because its metadata names the expected reset child.
pub(crate) fn verify_taira_inrou_prepared_transaction_identity_v1(
    transaction: &SignedTransaction,
    operation: TairaInrouCanaryPreparedOperationV1,
    stage: &TairaInrouStageIdentity,
    expected_idempotency_key: &str,
) -> Result<()> {
    if stage.stage_mode != MutationMode::Deploy.label_lowercase() {
        return Err(eyre!(
            "public-reset Inrou V1 accepts only the exact deploy stage"
        ));
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(eyre!(
            "prepared Inrou transaction must contain one direct instruction"
        ));
    };
    let [instruction] = instructions.as_ref() else {
        return Err(eyre!(
            "prepared Inrou transaction must contain exactly one instruction"
        ));
    };
    match operation {
        TairaInrouCanaryPreparedOperationV1::BundlePin
        | TairaInrouCanaryPreparedOperationV1::GuestPin
        | TairaInrouCanaryPreparedOperationV1::DiscoveryPin => {
            let registration = instruction
                .as_any()
                .downcast_ref::<iroha::data_model::isi::sorafs::RegisterPinManifest>()
                .ok_or_else(|| {
                    eyre!("prepared Inrou pin transaction substituted its instruction")
                })?;
            if registration.alias.is_some() || registration.successor_of.is_some() {
                return Err(eyre!(
                    "prepared Inrou pin transaction must not bind an alias or predecessor"
                ));
            }
            let manifest =
                sorafs_manifest::decode_manifest_v1_canonical(&registration.manifest_payload)
                    .wrap_err("prepared Inrou pin manifest is not canonical V1")?;
            validate_manifest(
                &manifest,
                &PinPolicyConstraints {
                    require_council_signatures: true,
                    ..PinPolicyConstraints::default()
                },
            )
            .wrap_err("prepared Inrou pin manifest failed exact policy validation")?;
            let authority = transaction
                .authority()
                .try_signatory()
                .ok_or_else(|| eyre!("prepared Inrou authority must be single-signatory"))?;
            let (authority_algorithm, authority_bytes) = authority
                .try_to_bytes()
                .wrap_err("prepared Inrou authority public key is malformed")?;
            if authority_algorithm != iroha_crypto::Algorithm::Ed25519 {
                return Err(eyre!("prepared Inrou authority must use an Ed25519 key"));
            }
            let authority_bytes: [u8; 32] = authority_bytes.try_into().map_err(|_| {
                eyre!(
                    "prepared Inrou authority must use a 32-byte Ed25519 key, found {} bytes",
                    authority_bytes.len()
                )
            })?;
            if manifest.governance.council_signatures.len() != 1
                || manifest.governance.council_signatures[0].signer != authority_bytes
            {
                return Err(eyre!(
                    "prepared Inrou pin manifest signer differs from the transaction authority"
                ));
            }
            let digest_hex = hex::encode(
                manifest
                    .digest()
                    .wrap_err("failed to digest prepared Inrou pin manifest")?
                    .as_bytes(),
            );
            let content_cid = encode_content_cid(&manifest.root_cid);
            let (expected_digest, expected_cid) = match operation {
                TairaInrouCanaryPreparedOperationV1::BundlePin => (
                    stage.bundle_manifest_digest_hex.as_str(),
                    stage.bundle_content_cid.as_str(),
                ),
                TairaInrouCanaryPreparedOperationV1::GuestPin => (
                    stage.guest_manifest_digest_hex.as_str(),
                    stage.guest_content_cid.as_str(),
                ),
                TairaInrouCanaryPreparedOperationV1::DiscoveryPin => (
                    stage.discovery_manifest_digest_hex.as_str(),
                    stage.discovery_content_cid.as_str(),
                ),
                TairaInrouCanaryPreparedOperationV1::ServiceMutation => unreachable!(),
            };
            if digest_hex != expected_digest || content_cid != expected_cid {
                return Err(eyre!(
                    "prepared Inrou pin instruction differs from its retained stage identity"
                ));
            }
        }
        TairaInrouCanaryPreparedOperationV1::ServiceMutation => {
            let deployment = instruction
                .as_any()
                .downcast_ref::<iroha::data_model::isi::soracloud::DeploySoracloudService>()
                .ok_or_else(|| {
                    eyre!("prepared Inrou service transaction is not an exact deploy instruction")
                })?;
            validate_taira_inrou_canary_bundle(&deployment.bundle)
                .wrap_err("prepared Inrou deployment bundle is not the canonical canary")?;
            if deployment.initial_service_configs.len() != 2
                || deployment
                    .initial_service_configs
                    .get("public_reset_idempotency_v1")
                    != Some(&Json::new(expected_idempotency_key.to_owned()))
                || !deployment.initial_service_secrets.is_empty()
                || deployment.precondition != SoraServiceMutationPreconditionV1::ServiceAbsent
            {
                return Err(eyre!(
                    "prepared Inrou deployment material or precondition is outside exact V1"
                ));
            }
            validate_taira_inrou_canary_public_discovery_config(
                deployment
                    .initial_service_configs
                    .get(PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME)
                    .ok_or_else(|| {
                        eyre!(
                            "prepared Inrou deployment omits its authoritative public-discovery registry"
                        )
                    })?,
                &deployment.bundle,
                stage,
            )?;
            let provenance_payload = encode_bundle_with_materials_provenance_payload(
                &deployment.bundle,
                &deployment.initial_service_configs,
                &deployment.initial_service_secrets,
                &deployment.precondition,
            )
            .wrap_err("failed to encode prepared Inrou deployment provenance")?;
            let authority = transaction
                .authority()
                .try_signatory()
                .ok_or_else(|| eyre!("prepared Inrou authority must be single-signatory"))?;
            if &deployment.provenance.signer != authority {
                return Err(eyre!(
                    "prepared Inrou deployment signer differs from the transaction authority"
                ));
            }
            deployment
                .provenance
                .signature
                .verify(&deployment.provenance.signer, &provenance_payload)
                .wrap_err("prepared Inrou deployment provenance signature is invalid")?;
            let bundle = &deployment.bundle;
            let route = bundle
                .service
                .route
                .as_ref()
                .ok_or_else(|| eyre!("prepared Inrou deployment omits its public route"))?;
            let guest = bundle
                .container
                .inrou
                .as_ref()
                .and_then(|inrou| inrou.guest_images.get(&SoraInrouGuestIsaV1::Aarch64))
                .ok_or_else(|| eyre!("prepared Inrou deployment omits its AArch64 guest"))?;
            if bundle.service.service_name.as_ref() != stage.service_name
                || bundle.service.service_version != stage.service_version
                || route.host != stage.route_host
                || route.path_prefix != stage.route_path_prefix
                || bundle.container.lifecycle.healthcheck_path.as_deref()
                    != Some(stage.healthcheck_path.as_str())
                || bundle.container.bundle_hash.to_string() != stage.bundle_hash
                || guest.published_artifact.content_cid != stage.guest_content_cid
                || guest.published_artifact.manifest_digest_hex != stage.guest_manifest_digest_hex
                || bundle.container_manifest_hash().to_string() != stage.container_manifest_hash
                || bundle.service_manifest_hash().to_string() != stage.service_manifest_hash
            {
                return Err(eyre!(
                    "prepared Inrou deploy instruction differs from its retained stage identity"
                ));
            }
        }
    }
    Ok(())
}

/// Prepare exactly one ledger transaction in the ordered Taira Inrou canary protocol.
///
/// The caller must prove and durably record the preceding child as Applied before
/// invoking this function for the next child. Nothing is submitted here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_taira_inrou_canary_operation(
    config: &ClientConfig,
    http_witness_file: Option<&Path>,
    fee_payment: FeePaymentIntent,
    binding: TairaMutationBindingV1,
    stage_dir: &Path,
    torii_url: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
    requested_mode: crate::taira::InrouCanaryMode,
    operation: TairaInrouCanaryPreparedOperationV1,
) -> Result<PreparedSoracloudTransactionV1> {
    binding.validate()?;
    if binding.kind != operation.mutation_kind() || binding.phase != "pre_edge" {
        return Err(eyre!(
            "Taira Inrou prepared operation does not match its exact pre_edge child kind"
        ));
    }
    let mode = MutationMode::from(requested_mode);
    let staged = load_verified_taira_inrou_stage(stage_dir, &config.key_pair, mode)?;
    match operation {
        TairaInrouCanaryPreparedOperationV1::BundlePin => {
            return prepare_built_sorafs_manifest_registration(
                &staged.bundle_manifest,
                operation.operation_label(),
                fee_payment,
                binding,
                torii_url,
                config,
                timeout_secs,
            );
        }
        TairaInrouCanaryPreparedOperationV1::GuestPin => {
            return prepare_built_sorafs_manifest_registration(
                &staged.guest_manifest,
                operation.operation_label(),
                fee_payment,
                binding,
                torii_url,
                config,
                timeout_secs,
            );
        }
        TairaInrouCanaryPreparedOperationV1::DiscoveryPin => {
            return prepare_built_sorafs_manifest_registration(
                &staged.discovery_manifest,
                operation.operation_label(),
                fee_payment,
                binding,
                torii_url,
                config,
                timeout_secs,
            );
        }
        TairaInrouCanaryPreparedOperationV1::ServiceMutation => {}
    }
    let initial_service_configs = BTreeMap::from([
        (
            "public_reset_idempotency_v1".to_owned(),
            Json::new(binding.idempotency_key.clone()),
        ),
        (
            PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_owned(),
            taira_inrou_canary_public_discovery_config_value(&staged.discovery)?,
        ),
    ]);
    let service_name = staged.bundle.service.service_name.to_string();
    let (_, status) =
        fetch_torii_soracloud_status(torii_url, Some(&service_name), api_token, timeout_secs)?;
    let precondition = preflight_taira_inrou_mutation_target(
        &status,
        &service_name,
        &staged.receipt.service_version,
        mode,
    )?;
    preflight_service_upgrade_identity(
        &status,
        &staged.bundle.service,
        staged.bundle.container.runtime,
        mode,
        "Taira Inrou",
    )?;
    let request = signed_bundle_request(
        staged.bundle,
        initial_service_configs,
        BTreeMap::new(),
        precondition,
        Some(&config.account),
        &config.key_pair,
    )?;
    let endpoint_path = match mode {
        MutationMode::Deploy => "v1/soracloud/deploy",
        MutationMode::Upgrade => "v1/soracloud/upgrade",
    };
    let requested = request_torii_soracloud_mutation_draft(
        torii_url,
        endpoint_path,
        &request,
        config,
        http_witness_file,
        api_token,
        timeout_secs,
    )?;
    let service_instructions = decode_soracloud_tx_instructions(&requested.draft)?;
    prepare_soracloud_draft_transaction(
        config,
        fee_payment,
        binding,
        torii_url,
        timeout_secs,
        service_instructions,
        operation.operation_label(),
    )
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum HfStorageClassArg {
    Hot,
    Warm,
    Cold,
}
impl HfStorageClassArg {
    const fn to_storage_class(self) -> StorageClass {
        match self {
            Self::Hot => StorageClass::Hot,
            Self::Warm => StorageClass::Warm,
            Self::Cold => StorageClass::Cold,
        }
    }
}
#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
enum RolloutHealth {
    #[default]
    Healthy,
    Unhealthy,
}
impl RolloutHealth {
    fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "action", content = "value")]
#[norito(deny_unknown_fields)]
enum SoracloudAction {
    Deploy,
    Upgrade,
    Rollback,
    ConfigMutation,
    SecretMutation,
    StateMutation,
    FheJobRun,
    FhePolicyRegister,
    FhePolicyRotate,
    FhePolicyRevoke,
    DecryptionRequest,
    CiphertextQuery,
    Rollout,
    LeaseUsage,
    LeaseReportingEpochRollover,
}
impl SoracloudAction {
    fn authoritative(self) -> SoraServiceLifecycleActionV1 {
        match self {
            Self::Deploy => SoraServiceLifecycleActionV1::Deploy,
            Self::Upgrade => SoraServiceLifecycleActionV1::Upgrade,
            Self::Rollback => SoraServiceLifecycleActionV1::Rollback,
            Self::ConfigMutation => SoraServiceLifecycleActionV1::ConfigMutation,
            Self::SecretMutation => SoraServiceLifecycleActionV1::SecretMutation,
            Self::StateMutation => SoraServiceLifecycleActionV1::StateMutation,
            Self::FheJobRun => SoraServiceLifecycleActionV1::FheJobRun,
            Self::FhePolicyRegister => SoraServiceLifecycleActionV1::FhePolicyRegister,
            Self::FhePolicyRotate => SoraServiceLifecycleActionV1::FhePolicyRotate,
            Self::FhePolicyRevoke => SoraServiceLifecycleActionV1::FhePolicyRevoke,
            Self::DecryptionRequest => SoraServiceLifecycleActionV1::DecryptionRequest,
            Self::CiphertextQuery => SoraServiceLifecycleActionV1::CiphertextQuery,
            Self::Rollout => SoraServiceLifecycleActionV1::Rollout,
            Self::LeaseUsage => SoraServiceLifecycleActionV1::LeaseUsage,
            Self::LeaseReportingEpochRollover => {
                SoraServiceLifecycleActionV1::LeaseReportingEpochRollover
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "stage", content = "value")]
#[norito(deny_unknown_fields)]
enum RolloutStage {
    Canary,
    Promoted,
    RolledBack,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RolloutRuntimeState {
    rollout_handle: String,
    baseline_version: String,
    candidate_version: String,
    canary_percent: u8,
    traffic_percent: u8,
    stage: RolloutStage,
    health_failures: u32,
    max_health_failures: u32,
    health_window_secs: u32,
    created_sequence: u64,
    updated_sequence: u64,
}
impl RolloutRuntimeState {
    fn authoritative(&self) -> SoraServiceRolloutStateV1 {
        SoraServiceRolloutStateV1 {
            schema_version: SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
            rollout_handle: self.rollout_handle.clone(),
            baseline_version: self.baseline_version.clone(),
            candidate_version: self.candidate_version.clone(),
            canary_percent: self.canary_percent,
            traffic_percent: self.traffic_percent,
            stage: match self.stage {
                RolloutStage::Canary => SoraRolloutStageV1::Canary,
                RolloutStage::Promoted => SoraRolloutStageV1::Promoted,
                RolloutStage::RolledBack => SoraRolloutStageV1::RolledBack,
            },
            health_failures: self.health_failures,
            max_health_failures: self.max_health_failures,
            health_window_secs: self.health_window_secs,
            created_sequence: self.created_sequence,
            updated_sequence: self.updated_sequence,
        }
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ControlPlaneServiceRevision {
    sequence: u64,
    action: SoracloudAction,
    service_version: String,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    replicas: u16,
    execution_plane: SoraServiceExecutionPlaneV1,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_service_port: Option<u16>,
    #[norito(required)]
    route_visibility: Option<String>,
    #[norito(required)]
    route_tls_mode: Option<String>,
    #[norito(required)]
    base_url: Option<String>,
    #[norito(required)]
    healthcheck_url: Option<String>,
    #[norito(required)]
    public_discovery_content_cid: Option<String>,
    #[norito(required)]
    public_discovery_url: Option<String>,
    #[norito(required)]
    public_discovery_cid_host_url: Option<String>,
    state_binding_count: u32,
    state_bindings: Vec<SoraStateBindingV1>,
    lease_volumes: Vec<SoraLeaseVolumeBindingV1>,
    allow_model_inference: bool,
    allow_model_training: bool,
    runtime: SoraContainerRuntimeV1,
    allow_state_writes: bool,
    network: SoraNetworkPolicyV1,
    cpu_millis: u32,
    memory_bytes: u64,
    ephemeral_storage_bytes: u64,
    max_open_files_per_process: u32,
    max_tasks: u16,
    start_grace_secs: u32,
    stop_grace_secs: u32,
    #[norito(required)]
    healthcheck_path: Option<String>,
    required_config_names: Vec<String>,
    required_secret_names: Vec<String>,
    config_exports: Vec<SoraConfigExportV1>,
    sandbox_profile_hash: Hash,
    process_generation: u64,
    process_started_sequence: u64,
    signed_by: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct InitOutput {
    template: String,
    container_manifest_path: String,
    service_manifest_path: String,
    container_manifest_hash: Hash,
    service_manifest_hash: Hash,
    template_artifacts: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct BundlePackOutput {
    source_file: String,
    source_size_bytes: u64,
    archive_member_path: String,
    archive_member_mode: u32,
    bundle_file: String,
    bundle_size_bytes: u64,
    bundle_hash: Hash,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SyncManifestsOutput {
    #[norito(required)]
    app_manifest_path: Option<String>,
    #[norito(required)]
    container_manifest_path: Option<String>,
    #[norito(required)]
    service_manifest_path: Option<String>,
    #[norito(required)]
    container_manifest_hash: Option<Hash>,
    #[norito(required)]
    service_manifest_hash: Option<Hash>,
    #[norito(required)]
    bundle_file: Option<String>,
    #[norito(required)]
    bundle_hash: Option<Hash>,
    services: Vec<SyncManifestEntryOutput>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SyncManifestEntryOutput {
    service_name: String,
    container_manifest_path: String,
    service_manifest_path: String,
    container_manifest_hash: Hash,
    service_manifest_hash: Hash,
    #[norito(required)]
    bundle_file: Option<String>,
    bundle_hash: Hash,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StatusOutput {
    source: String,
    #[norito(required)]
    torii_endpoint: Option<String>,
    #[norito(required)]
    schema_version: Option<u16>,
    #[norito(required)]
    service_count: Option<u32>,
    #[norito(required)]
    audit_event_count: Option<u32>,
    services: Vec<ServiceStatusOutput>,
    #[norito(required)]
    network_status: Option<norito::json::Value>,
    #[norito(required)]
    service_plan: Option<ServiceLocalPlanOutput>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceStatusOutput {
    service_name: String,
    current_version: String,
    revision_count: u32,
    config_generation: u64,
    secret_generation: u64,
    config_entry_count: u32,
    secret_entry_count: u32,
    #[norito(required)]
    service_lease: Option<ServiceLeaseStatusOutput>,
    #[norito(required)]
    public_discovery_content_cid: Option<String>,
    #[norito(required)]
    public_discovery_url: Option<String>,
    #[norito(required)]
    public_discovery_cid_host_url: Option<String>,
    #[norito(required)]
    latest_revision: Option<ControlPlaneServiceRevision>,
    #[norito(required)]
    active_rollout: Option<RolloutRuntimeState>,
    #[norito(required)]
    last_rollout: Option<RolloutRuntimeState>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceLeaseStatusOutput {
    authoritative_state: SoraServiceLeaseStateV1,
    effective_status: SoraServiceLeaseStatusV1,
    remaining_runtime_balance: Quantity,
}
#[derive(Clone, Debug, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NetworkControlPlaneSnapshotV1 {
    schema_version: u16,
    service_count: u32,
    audit_event_count: u32,
    active_inrou_hosts: Vec<SoraInrouHostCapabilityRecordV1>,
    services: Vec<ServiceStatusOutput>,
    recent_audit_events: Vec<NetworkControlPlaneAuditEventV1>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct NetworkControlPlaneAuditEventV1 {
    sequence: u64,
    action: SoracloudAction,
    service_name: String,
    #[norito(required)]
    from_version: Option<String>,
    to_version: String,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    process_generation: u64,
    config_generation: u64,
    secret_generation: u64,
    config_snapshot_hash: Hash,
    secret_snapshot_hash: Hash,
    #[norito(required)]
    binding_name: Option<String>,
    #[norito(required)]
    state_key: Option<String>,
    config_mutations: Vec<SoraServiceConfigMutationV1>,
    secret_mutations: Vec<SoraServiceSecretMutationV1>,
    #[norito(required)]
    governance_tx_hash: Option<Hash>,
    #[norito(required)]
    rollout_state: Option<RolloutRuntimeState>,
    #[norito(required)]
    policy_name: Option<String>,
    #[norito(required)]
    policy_snapshot_hash: Option<Hash>,
    #[norito(required)]
    jurisdiction_tag: Option<String>,
    #[norito(required)]
    consent_evidence_hash: Option<Hash>,
    #[norito(required)]
    break_glass: Option<bool>,
    #[norito(required)]
    break_glass_reason: Option<String>,
    #[norito(required)]
    lease_usage: Option<SoraServiceLeaseUsageAuditV1>,
    #[norito(required)]
    service_lease_commitment: Option<Hash>,
    #[norito(required)]
    lease_reporting_epoch_rollover: Option<SoraServiceLeaseReportingEpochRolloverV1>,
    signed_by: String,
}
impl NetworkControlPlaneAuditEventV1 {
    fn validate(&self) -> Result<()> {
        let parse_exact_name = |field: &str, value: &str| -> Result<Name> {
            let name = value
                .parse::<Name>()
                .wrap_err_with(|| format!("invalid Soracloud control-plane audit `{field}`"))?;
            if name.as_ref() != value {
                return Err(eyre!(
                    "Soracloud control-plane audit `{field}` must use exact canonical V1 spelling"
                ));
            }
            Ok(name)
        };
        let service_name = parse_exact_name("service_name", &self.service_name)?;
        let binding_name = self
            .binding_name
            .as_deref()
            .map(|value| parse_exact_name("binding_name", value))
            .transpose()?;
        let policy_name = self
            .policy_name
            .as_deref()
            .map(|value| parse_exact_name("policy_name", value))
            .transpose()?;
        let signer = self
            .signed_by
            .parse::<PublicKey>()
            .wrap_err("invalid Soracloud control-plane audit `signed_by`")?;
        if signer.to_string() != self.signed_by {
            return Err(eyre!(
                "Soracloud control-plane audit `signed_by` must use exact canonical V1 spelling"
            ));
        }
        SoraServiceAuditEventV1 {
            schema_version: SORA_SERVICE_AUDIT_EVENT_VERSION_V1,
            sequence: self.sequence,
            // Torii's control-plane projection intentionally omits the already-committed
            // block coordinates. Positive sentinels let the authoritative validator check
            // every invariant retained by this public projection.
            block_height: 1,
            block_timestamp_ms: 1,
            action: self.action.authoritative(),
            service_name,
            from_version: self.from_version.clone(),
            to_version: self.to_version.clone(),
            service_manifest_hash: self.service_manifest_hash,
            container_manifest_hash: self.container_manifest_hash,
            process_generation: self.process_generation,
            config_generation: self.config_generation,
            secret_generation: self.secret_generation,
            config_snapshot_hash: self.config_snapshot_hash,
            secret_snapshot_hash: self.secret_snapshot_hash,
            governance_tx_hash: self.governance_tx_hash,
            binding_name,
            state_key: self.state_key.clone(),
            config_mutations: self.config_mutations.clone(),
            secret_mutations: self.secret_mutations.clone(),
            rollout_state: self
                .rollout_state
                .as_ref()
                .map(RolloutRuntimeState::authoritative),
            policy_name,
            policy_snapshot_hash: self.policy_snapshot_hash,
            jurisdiction_tag: self.jurisdiction_tag.clone(),
            consent_evidence_hash: self.consent_evidence_hash,
            break_glass: self.break_glass,
            break_glass_reason: self.break_glass_reason.clone(),
            lease_usage: self.lease_usage.clone(),
            service_lease_commitment: self.service_lease_commitment,
            lease_reporting_epoch_rollover: self.lease_reporting_epoch_rollover.clone(),
            signer,
        }
        .validate()
        .map_err(|error| eyre!("invalid Soracloud control-plane audit event V1: {error}"))
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceConfigStatusEntryV1 {
    config_name: String,
    value_hash: Hash,
    value_json: json::Value,
    last_update_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceConfigStatusResponseV1 {
    schema_version: u16,
    service_name: Name,
    current_version: String,
    config_generation: u64,
    config_entry_count: u32,
    configs: Vec<ServiceConfigStatusEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceSecretStatusEntryV1 {
    secret_name: String,
    encryption: SecretEnvelopeEncryptionV1,
    key_id: String,
    key_version: u32,
    commitment: Hash,
    ciphertext_bytes: u64,
    last_update_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceSecretStatusResponseV1 {
    schema_version: u16,
    service_name: Name,
    current_version: String,
    secret_generation: u64,
    secret_entry_count: u32,
    secrets: Vec<ServiceSecretStatusEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct TrainingJobStatusEntryV1 {
    service_name: Name,
    model_name: Name,
    job_id: String,
    status: SoraTrainingJobStatusV1,
    worker_group_size: u16,
    target_steps: u32,
    completed_steps: u32,
    checkpoint_interval_steps: u32,
    #[norito(required)]
    last_checkpoint_step: Option<u32>,
    checkpoint_count: u32,
    retry_count: u8,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    compute_consumed_units: u64,
    compute_remaining_units: u64,
    storage_budget_bytes: u64,
    storage_consumed_bytes: u64,
    storage_remaining_bytes: u64,
    #[norito(required)]
    latest_metrics_hash: Option<Hash>,
    #[norito(required)]
    last_failure_reason: Option<String>,
    created_sequence: u64,
    updated_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct TrainingJobStatusResponseV1 {
    schema_version: u16,
    job: TrainingJobStatusEntryV1,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ModelWeightVersionEntryV1 {
    weight_version: String,
    #[norito(required)]
    parent_version: Option<String>,
    training_job_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    registered_sequence: u64,
    #[norito(required)]
    promoted_sequence: Option<u64>,
    #[norito(required)]
    gate_report_hash: Option<Hash>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ModelWeightStatusEntryV1 {
    service_name: Name,
    model_name: Name,
    #[norito(required)]
    current_version: Option<String>,
    version_count: u32,
    versions: Vec<ModelWeightVersionEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ModelWeightStatusResponseV1 {
    schema_version: u16,
    model: ModelWeightStatusEntryV1,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ModelArtifactStatusEntryV1 {
    service_name: Name,
    model_name: Name,
    artifact_id: String,
    training_job_id: String,
    #[norito(required)]
    weight_version: Option<String>,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    registered_sequence: u64,
    #[norito(required)]
    consumed_by_version: Option<String>,
    #[norito(required)]
    chunk_manifest_root: Option<Hash>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ModelArtifactStatusResponseV1 {
    schema_version: u16,
    service_name: Name,
    model_name: Name,
    artifact_count: u32,
    artifact: ModelArtifactStatusEntryV1,
    artifacts: Vec<ModelArtifactStatusEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct UploadedModelStatusResponseV1 {
    schema_version: u16,
    bundle: SoraUploadedModelBundleV1,
    #[norito(required)]
    artifact: Option<ModelArtifactStatusEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentStatusResponseV1 {
    schema_version: u16,
    apartment_count: u32,
    event_count: u32,
    apartments: Vec<AgentApartmentStatusEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentApartmentStatusEntryV1 {
    apartment_name: Name,
    manifest_hash: Hash,
    status: SoraAgentRuntimeStatusV1,
    lease_started_sequence: u64,
    lease_expires_sequence: u64,
    lease_remaining_ticks: u64,
    restart_count: u32,
    state_quota_bytes: u64,
    tool_capability_count: u32,
    policy_capability_count: u32,
    revoked_policy_capability_count: u32,
    pending_wallet_request_count: u32,
    pending_mailbox_message_count: u32,
    autonomy_budget_ceiling_units: u64,
    autonomy_budget_remaining_units: u64,
    artifact_allowlist_count: u32,
    autonomy_run_count: u32,
    process_generation: u64,
    process_started_sequence: u64,
    last_active_sequence: u64,
    #[norito(required)]
    last_checkpoint_sequence: Option<u64>,
    checkpoint_count: u32,
    persistent_state_total_bytes: u64,
    persistent_state_key_count: u32,
    spend_limit_count: u32,
    upgrade_policy: AgentUpgradePolicyV1,
    #[norito(required)]
    last_restart_sequence: Option<u64>,
    #[norito(required)]
    last_restart_reason: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentMailboxStatusResponseV1 {
    schema_version: u16,
    apartment_name: Name,
    status: SoraAgentRuntimeStatusV1,
    pending_message_count: u32,
    event_count: u32,
    messages: Vec<AgentMailboxMessageEntryV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentMailboxMessageEntryV1 {
    message_id: String,
    from_apartment: Name,
    channel: String,
    payload: String,
    payload_hash: Hash,
    enqueued_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentRuntimeReceiptRecordV1 {
    receipt_id: Hash,
    service_name: String,
    service_version: String,
    handler_name: String,
    handler_class: SoraServiceHandlerClassV1,
    request_commitment: Hash,
    result_commitment: Hash,
    certified_by: SoraCertifiedResponsePolicyV1,
    emitted_sequence: u64,
    #[norito(required)]
    execution_host: Option<SoraRuntimeDeterministicValidatorHostV1>,
    #[norito(required)]
    journal_artifact_hash: Option<Hash>,
    #[norito(required)]
    checkpoint_artifact_hash: Option<Hash>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentAutonomyExecutionAuditRecordV1 {
    sequence: u64,
    succeeded: bool,
    result_commitment: Hash,
    #[norito(required)]
    service_name: Option<String>,
    #[norito(required)]
    service_version: Option<String>,
    #[norito(required)]
    handler_name: Option<String>,
    #[norito(required)]
    runtime_receipt_id: Option<Hash>,
    #[norito(required)]
    journal_artifact_hash: Option<Hash>,
    #[norito(required)]
    checkpoint_artifact_hash: Option<Hash>,
    #[norito(required)]
    reason: Option<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentAutonomyAllowlistEntryV1 {
    artifact_hash: String,
    #[norito(required)]
    provenance_hash: Option<String>,
    added_sequence: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentAutonomyRunStatusRecordV1 {
    run_id: String,
    artifact_hash: String,
    #[norito(required)]
    provenance_hash: Option<String>,
    budget_units: u64,
    run_label: String,
    #[norito(required)]
    workflow_input_json: Option<String>,
    approved_sequence: u64,
    #[norito(required)]
    authoritative_runtime_receipt: Option<AgentRuntimeReceiptRecordV1>,
    #[norito(required)]
    authoritative_execution_audit: Option<AgentAutonomyExecutionAuditRecordV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AgentAutonomyStatusResponseV1 {
    apartment_name: Name,
    sequence: u64,
    status: SoraAgentRuntimeStatusV1,
    lease_expires_sequence: u64,
    lease_remaining_ticks: u64,
    manifest_hash: Hash,
    revoked_policy_capability_count: u32,
    budget_ceiling_units: u64,
    budget_remaining_units: u64,
    allowlist_count: u32,
    run_count: u32,
    process_generation: u64,
    process_started_sequence: u64,
    last_active_sequence: u64,
    #[norito(required)]
    last_checkpoint_sequence: Option<u64>,
    checkpoint_count: u32,
    persistent_state_total_bytes: u64,
    persistent_state_key_count: u32,
    allowlist: Vec<AgentAutonomyAllowlistEntryV1>,
    recent_runs: Vec<AgentAutonomyRunStatusRecordV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HfSharedLeaseStatusResponseV1 {
    schema_version: u16,
    source: SoraHfSourceRecordV1,
    #[norito(required)]
    pool: Option<SoraHfSharedLeasePoolV1>,
    #[norito(required)]
    member: Option<SoraHfSharedLeaseMemberV1>,
    #[norito(required)]
    latest_audit_event: Option<SoraHfSharedLeaseAuditEventV1>,
    audit_event_count: u32,
    storage_base_fee: Quantity,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppInfraStatusResponseV1 {
    schema_version: u16,
    app_count: u32,
    audit_event_count: u32,
    apps: Vec<SoraAppInfraStateV1>,
    recent_audit_events: Vec<SoraAppInfraAuditEventV1>,
}

fn exact_status_count(label: &str, advertised: u32, actual: usize) -> Result<()> {
    let actual = u32::try_from(actual)
        .wrap_err_with(|| format!("{label} exceeds the Soracloud V1 count range"))?;
    if advertised != actual {
        return Err(eyre!(
            "{label} advertises {advertised} entries but contains {actual}"
        ));
    }
    Ok(())
}

fn validate_canonical_status_text(context: &str, field: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.trim() != value {
        return Err(eyre!("{context} contains a non-canonical `{field}` value"));
    }
    Ok(())
}

fn validate_model_identifier_v1(context: &str, field: &str, value: &str) -> Result<()> {
    const MAX_IDENTIFIER_BYTES_V1: usize = 128;
    validate_canonical_status_text(context, field, value)?;
    if value.len() > MAX_IDENTIFIER_BYTES_V1
        || value.chars().any(char::is_control)
        || value.chars().any(|ch| ch.is_ascii_whitespace())
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(eyre!(
            "{context} contains an invalid Soracloud V1 `{field}` identifier"
        ));
    }
    Ok(())
}

impl ServiceConfigStatusEntryV1 {
    fn validate(&self) -> Result<()> {
        let canonical_name = parse_service_material_name_arg("config_name", &self.config_name)?;
        if canonical_name != self.config_name {
            return Err(eyre!(
                "Soracloud service-config status contains a non-canonical config_name"
            ));
        }
        SoraServiceConfigEntryV1 {
            schema_version: SORA_SERVICE_CONFIG_ENTRY_VERSION_V1,
            config_name: self.config_name.clone(),
            value_json: Json::from(self.value_json.clone()),
            value_hash: self.value_hash,
            last_update_sequence: self.last_update_sequence,
        }
        .validate()
        .wrap_err("invalid Soracloud service-config status entry")
    }
}

impl ServiceConfigStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud service-config status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        validate_canonical_status_text(
            "Soracloud service-config status",
            "current_version",
            &self.current_version,
        )?;
        exact_status_count(
            "Soracloud service-config status config_entry_count",
            self.config_entry_count,
            self.configs.len(),
        )?;
        let mut names = BTreeSet::new();
        for entry in &self.configs {
            entry.validate()?;
            if !names.insert(entry.config_name.clone()) {
                return Err(eyre!(
                    "Soracloud service-config status contains duplicate config_name `{}`",
                    entry.config_name
                ));
            }
        }
        Ok(())
    }
}

impl ServiceSecretStatusEntryV1 {
    fn validate(&self) -> Result<()> {
        let canonical_name = parse_service_material_name_arg("secret_name", &self.secret_name)?;
        if canonical_name != self.secret_name {
            return Err(eyre!(
                "Soracloud service-secret status contains a non-canonical secret_name"
            ));
        }
        validate_canonical_status_text("Soracloud service-secret status", "key_id", &self.key_id)?;
        if self.key_version == 0 || self.ciphertext_bytes == 0 || self.last_update_sequence == 0 {
            return Err(eyre!(
                "Soracloud service-secret status entry `{}` contains zero key, ciphertext, or sequence metadata",
                self.secret_name
            ));
        }
        Ok(())
    }
}

impl ServiceSecretStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud service-secret status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        validate_canonical_status_text(
            "Soracloud service-secret status",
            "current_version",
            &self.current_version,
        )?;
        exact_status_count(
            "Soracloud service-secret status secret_entry_count",
            self.secret_entry_count,
            self.secrets.len(),
        )?;
        let mut names = BTreeSet::new();
        for entry in &self.secrets {
            entry.validate()?;
            if !names.insert(entry.secret_name.clone()) {
                return Err(eyre!(
                    "Soracloud service-secret status contains duplicate secret_name `{}`",
                    entry.secret_name
                ));
            }
        }
        Ok(())
    }
}

impl TrainingJobStatusEntryV1 {
    fn validate(&self) -> Result<()> {
        validate_model_identifier_v1("Soracloud training-job status", "job_id", &self.job_id)?;
        if self.worker_group_size == 0
            || self.target_steps == 0
            || self.checkpoint_interval_steps == 0
            || self.checkpoint_interval_steps > self.target_steps
            || self.completed_steps > self.target_steps
            || self.retry_count > self.max_retries
            || self.step_compute_units == 0
            || self.compute_budget_units == 0
            || self.compute_consumed_units > self.compute_budget_units
            || self.compute_remaining_units
                != self
                    .compute_budget_units
                    .saturating_sub(self.compute_consumed_units)
            || self.storage_budget_bytes == 0
            || self.storage_consumed_bytes > self.storage_budget_bytes
            || self.storage_remaining_bytes
                != self
                    .storage_budget_bytes
                    .saturating_sub(self.storage_consumed_bytes)
            || self.created_sequence == 0
            || self.updated_sequence < self.created_sequence
        {
            return Err(eyre!(
                "Soracloud training-job status `{}` contains inconsistent progress, budget, or sequence metadata",
                self.job_id
            ));
        }
        if self
            .last_checkpoint_step
            .is_some_and(|step| step == 0 || step > self.completed_steps)
        {
            return Err(eyre!(
                "Soracloud training-job status `{}` contains an invalid last_checkpoint_step",
                self.job_id
            ));
        }
        validate_optional_canonical_status_text(
            "Soracloud training-job status",
            "last_failure_reason",
            self.last_failure_reason.as_deref(),
        )?;
        Ok(())
    }
}

impl TrainingJobStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud training-job status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        self.job.validate()
    }
}

impl ModelWeightVersionEntryV1 {
    fn validate(&self) -> Result<()> {
        validate_model_identifier_v1(
            "Soracloud model-weight status",
            "weight_version",
            &self.weight_version,
        )?;
        if let Some(parent_version) = self.parent_version.as_deref() {
            validate_model_identifier_v1(
                "Soracloud model-weight status",
                "parent_version",
                parent_version,
            )?;
            if parent_version == self.weight_version {
                return Err(eyre!(
                    "Soracloud model-weight status version `{}` is its own parent",
                    self.weight_version
                ));
            }
        }
        if !self.training_job_id.is_empty() {
            validate_model_identifier_v1(
                "Soracloud model-weight status",
                "training_job_id",
                &self.training_job_id,
            )?;
        }
        validate_canonical_status_text(
            "Soracloud model-weight status",
            "dataset_ref",
            &self.dataset_ref,
        )?;
        if self.registered_sequence == 0
            || self.promoted_sequence.is_some() != self.gate_report_hash.is_some()
            || self
                .promoted_sequence
                .is_some_and(|sequence| sequence < self.registered_sequence)
        {
            return Err(eyre!(
                "Soracloud model-weight status version `{}` contains inconsistent promotion metadata",
                self.weight_version
            ));
        }
        Ok(())
    }
}

impl ModelWeightStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud model-weight status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        exact_status_count(
            "Soracloud model-weight status version_count",
            self.model.version_count,
            self.model.versions.len(),
        )?;
        let mut previous: Option<&str> = None;
        for version in &self.model.versions {
            version.validate()?;
            if previous.is_some_and(|previous| previous >= version.weight_version.as_str()) {
                return Err(eyre!(
                    "Soracloud model-weight status versions must be strictly sorted and duplicate-free"
                ));
            }
            previous = Some(&version.weight_version);
        }
        if let Some(current_version) = self.model.current_version.as_deref() {
            validate_model_identifier_v1(
                "Soracloud model-weight status",
                "current_version",
                current_version,
            )?;
            if !self
                .model
                .versions
                .iter()
                .any(|version| version.weight_version == current_version)
            {
                return Err(eyre!(
                    "Soracloud model-weight status current_version `{current_version}` has no matching version entry"
                ));
            }
        }
        Ok(())
    }
}

impl ModelArtifactStatusEntryV1 {
    fn validate(&self) -> Result<()> {
        validate_model_identifier_v1(
            "Soracloud model-artifact status",
            "artifact_id",
            &self.artifact_id,
        )?;
        if !self.training_job_id.is_empty() {
            validate_model_identifier_v1(
                "Soracloud model-artifact status",
                "training_job_id",
                &self.training_job_id,
            )?;
        }
        for (field, value) in [
            ("weight_version", self.weight_version.as_deref()),
            ("consumed_by_version", self.consumed_by_version.as_deref()),
        ] {
            if let Some(value) = value {
                validate_model_identifier_v1("Soracloud model-artifact status", field, value)?;
            }
        }
        validate_canonical_status_text(
            "Soracloud model-artifact status",
            "dataset_ref",
            &self.dataset_ref,
        )?;
        if self.registered_sequence == 0 {
            return Err(eyre!(
                "Soracloud model-artifact status `{}` has zero registered_sequence",
                self.artifact_id
            ));
        }
        Ok(())
    }
}

impl ModelArtifactStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud model-artifact status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        exact_status_count(
            "Soracloud model-artifact status artifact_count",
            self.artifact_count,
            self.artifacts.len(),
        )?;
        if self.artifacts.first() != Some(&self.artifact) {
            return Err(eyre!(
                "Soracloud model-artifact status primary artifact does not equal the first artifact entry"
            ));
        }
        let mut artifact_ids = BTreeSet::new();
        for (index, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate()?;
            if artifact.service_name != self.service_name || artifact.model_name != self.model_name
            {
                return Err(eyre!(
                    "Soracloud model-artifact status entry `{}` does not match the response identity",
                    artifact.artifact_id
                ));
            }
            if !artifact_ids.insert(artifact.artifact_id.clone()) {
                return Err(eyre!(
                    "Soracloud model-artifact status contains duplicate artifact_id `{}`",
                    artifact.artifact_id
                ));
            }
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|previous| self.artifacts.get(previous))
                && (previous.registered_sequence < artifact.registered_sequence
                    || (previous.registered_sequence == artifact.registered_sequence
                        && previous.artifact_id >= artifact.artifact_id))
            {
                return Err(eyre!(
                    "Soracloud model-artifact status entries are not in canonical newest-first order"
                ));
            }
        }
        Ok(())
    }
}

impl UploadedModelStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud uploaded-model status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        self.bundle
            .validate()
            .wrap_err("invalid Soracloud uploaded-model status bundle")?;
        if let Some(artifact) = self.artifact.as_ref() {
            artifact.validate()?;
            if artifact.service_name != self.bundle.service_name
                || artifact.weight_version.as_deref() != Some(self.bundle.weight_version.as_str())
                || artifact.chunk_manifest_root != Some(self.bundle.chunk_manifest_root)
            {
                return Err(eyre!(
                    "Soracloud uploaded-model artifact does not bind the returned bundle"
                ));
            }
        }
        Ok(())
    }
}

impl AgentApartmentStatusEntryV1 {
    fn validate(&self) -> Result<()> {
        if self.lease_started_sequence == 0
            || self.lease_expires_sequence <= self.lease_started_sequence
        {
            return Err(eyre!(
                "agent apartment `{}` has an invalid lease sequence interval",
                self.apartment_name
            ));
        }
        if self.state_quota_bytes == 0 || self.autonomy_budget_ceiling_units == 0 {
            return Err(eyre!(
                "agent apartment `{}` has zero state quota or autonomy budget ceiling",
                self.apartment_name
            ));
        }
        if self.autonomy_budget_remaining_units > self.autonomy_budget_ceiling_units {
            return Err(eyre!(
                "agent apartment `{}` has autonomy budget remaining above its ceiling",
                self.apartment_name
            ));
        }
        if self.process_generation == 0
            || self.process_started_sequence == 0
            || self.last_active_sequence < self.process_started_sequence
        {
            return Err(eyre!(
                "agent apartment `{}` has invalid process sequence metadata",
                self.apartment_name
            ));
        }
        if self
            .last_checkpoint_sequence
            .is_some_and(|sequence| sequence == 0 || sequence > self.last_active_sequence)
        {
            return Err(eyre!(
                "agent apartment `{}` has an invalid last checkpoint sequence",
                self.apartment_name
            ));
        }
        if self.last_restart_sequence.is_some() != self.last_restart_reason.is_some() {
            return Err(eyre!(
                "agent apartment `{}` has inconsistent restart metadata",
                self.apartment_name
            ));
        }
        if let Some(sequence) = self.last_restart_sequence
            && (sequence == 0 || sequence > self.last_active_sequence)
        {
            return Err(eyre!(
                "agent apartment `{}` has an invalid last restart sequence",
                self.apartment_name
            ));
        }
        if let Some(reason) = self.last_restart_reason.as_deref() {
            validate_canonical_status_text(
                "agent apartment status",
                "last_restart_reason",
                reason,
            )?;
        }
        Ok(())
    }
}

impl AgentStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud agent status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        exact_status_count(
            "Soracloud agent status apartment_count",
            self.apartment_count,
            self.apartments.len(),
        )?;
        let mut names = BTreeSet::new();
        for apartment in &self.apartments {
            apartment.validate()?;
            if !names.insert(apartment.apartment_name.clone()) {
                return Err(eyre!(
                    "Soracloud agent status contains duplicate apartment `{}`",
                    apartment.apartment_name
                ));
            }
        }
        Ok(())
    }
}

impl AgentMailboxMessageEntryV1 {
    fn validate(&self) -> Result<()> {
        validate_canonical_status_text("agent mailbox status", "message_id", &self.message_id)?;
        validate_canonical_status_text("agent mailbox status", "channel", &self.channel)?;
        if self.enqueued_sequence == 0 {
            return Err(eyre!(
                "agent mailbox message `{}` has zero enqueued_sequence",
                self.message_id
            ));
        }
        if self.payload_hash != Hash::new(self.payload.as_bytes()) {
            return Err(eyre!(
                "agent mailbox message `{}` payload_hash does not match its payload",
                self.message_id
            ));
        }
        Ok(())
    }
}

impl AgentMailboxStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud agent mailbox schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        exact_status_count(
            "Soracloud agent mailbox pending_message_count",
            self.pending_message_count,
            self.messages.len(),
        )?;
        let mut message_ids = BTreeSet::new();
        for message in &self.messages {
            message.validate()?;
            if !message_ids.insert(message.message_id.clone()) {
                return Err(eyre!(
                    "Soracloud agent mailbox status contains duplicate message_id `{}`",
                    message.message_id
                ));
            }
        }
        Ok(())
    }
}

fn validate_optional_canonical_status_text(
    context: &str,
    field: &str,
    value: Option<&str>,
) -> Result<()> {
    if let Some(value) = value {
        validate_canonical_status_text(context, field, value)?;
    }
    Ok(())
}

impl AgentRuntimeReceiptRecordV1 {
    fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("service_name", self.service_name.as_str()),
            ("service_version", self.service_version.as_str()),
            ("handler_name", self.handler_name.as_str()),
        ] {
            validate_canonical_status_text("agent runtime receipt", field, value)?;
        }
        if self.emitted_sequence == 0 {
            return Err(eyre!("agent runtime receipt has zero emitted_sequence"));
        }
        validate_optional_canonical_status_text(
            "agent runtime receipt",
            "execution_host.peer_id",
            self.execution_host
                .as_ref()
                .map(|host| host.peer_id.as_str()),
        )?;
        Ok(())
    }
}

impl AgentAutonomyExecutionAuditRecordV1 {
    fn validate(&self) -> Result<()> {
        if self.sequence == 0 {
            return Err(eyre!("agent autonomy execution audit has zero sequence"));
        }
        for (field, value) in [
            ("service_name", self.service_name.as_deref()),
            ("service_version", self.service_version.as_deref()),
            ("handler_name", self.handler_name.as_deref()),
            ("reason", self.reason.as_deref()),
        ] {
            validate_optional_canonical_status_text(
                "agent autonomy execution audit",
                field,
                value,
            )?;
        }
        Ok(())
    }
}

impl AgentAutonomyRunStatusRecordV1 {
    fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("artifact_hash", self.artifact_hash.as_str()),
            ("run_label", self.run_label.as_str()),
        ] {
            validate_canonical_status_text("agent autonomy run status", field, value)?;
        }
        validate_optional_canonical_status_text(
            "agent autonomy run status",
            "provenance_hash",
            self.provenance_hash.as_deref(),
        )?;
        if self.budget_units == 0 || self.approved_sequence == 0 {
            return Err(eyre!(
                "agent autonomy run `{}` has zero budget or approved sequence",
                self.run_id
            ));
        }
        if let Some(workflow_input_json) = self.workflow_input_json.as_deref() {
            parse_exact_agent_workflow_input_json(workflow_input_json)?;
        }
        if let Some(receipt) = self.authoritative_runtime_receipt.as_ref() {
            receipt.validate()?;
        }
        if let Some(audit) = self.authoritative_execution_audit.as_ref() {
            audit.validate()?;
        }
        Ok(())
    }
}

impl AgentAutonomyStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        let lease_state_matches = match self.status {
            SoraAgentRuntimeStatusV1::Running => self.lease_expires_sequence > self.sequence,
            SoraAgentRuntimeStatusV1::LeaseExpired => self.lease_expires_sequence <= self.sequence,
        };
        if self.sequence == 0
            || !lease_state_matches
            || self.lease_remaining_ticks
                != self.lease_expires_sequence.saturating_sub(self.sequence)
        {
            return Err(eyre!(
                "agent autonomy status for `{}` has invalid sequence or lease metadata",
                self.apartment_name
            ));
        }
        if self.budget_ceiling_units == 0 || self.budget_remaining_units > self.budget_ceiling_units
        {
            return Err(eyre!(
                "agent autonomy status for `{}` has invalid budget metadata",
                self.apartment_name
            ));
        }
        if self.process_generation == 0
            || self.process_started_sequence == 0
            || self.last_active_sequence < self.process_started_sequence
            || self.last_active_sequence > self.sequence
        {
            return Err(eyre!(
                "agent autonomy status for `{}` has invalid process sequence metadata",
                self.apartment_name
            ));
        }
        if self
            .last_checkpoint_sequence
            .is_some_and(|sequence| sequence == 0 || sequence > self.last_active_sequence)
        {
            return Err(eyre!(
                "agent autonomy status for `{}` has an invalid checkpoint sequence",
                self.apartment_name
            ));
        }
        exact_status_count(
            "Soracloud agent autonomy allowlist_count",
            self.allowlist_count,
            self.allowlist.len(),
        )?;
        let recent_run_count = u32::try_from(self.recent_runs.len())
            .wrap_err("Soracloud agent autonomy recent run list exceeds the V1 count range")?;
        if recent_run_count > self.run_count {
            return Err(eyre!(
                "Soracloud agent autonomy recent run count exceeds run_count"
            ));
        }
        let mut artifact_hashes = BTreeSet::new();
        for rule in &self.allowlist {
            validate_canonical_status_text(
                "agent autonomy allowlist",
                "artifact_hash",
                &rule.artifact_hash,
            )?;
            validate_optional_canonical_status_text(
                "agent autonomy allowlist",
                "provenance_hash",
                rule.provenance_hash.as_deref(),
            )?;
            if rule.added_sequence == 0 || !artifact_hashes.insert(rule.artifact_hash.clone()) {
                return Err(eyre!(
                    "agent autonomy status contains an invalid or duplicate allowlist artifact `{}`",
                    rule.artifact_hash
                ));
            }
        }
        let mut run_ids = BTreeSet::new();
        for run in &self.recent_runs {
            run.validate()?;
            if !run_ids.insert(run.run_id.clone()) {
                return Err(eyre!(
                    "agent autonomy status contains duplicate run_id `{}`",
                    run.run_id
                ));
            }
        }
        Ok(())
    }
}

impl HfSharedLeaseStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud HF status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        self.source
            .validate()
            .wrap_err("invalid Soracloud HF status source")?;
        if let Some(pool) = self.pool.as_ref() {
            pool.validate()
                .wrap_err("invalid Soracloud HF status lease pool")?;
            if pool.source_id != self.source.source_id {
                return Err(eyre!(
                    "Soracloud HF status pool source_id does not match the source record"
                ));
            }
            if self.storage_base_fee != pool.base_fee {
                return Err(eyre!(
                    "Soracloud HF status storage_base_fee does not match the lease pool"
                ));
            }
        } else if !self.storage_base_fee.is_zero() {
            return Err(eyre!(
                "Soracloud HF status without a lease pool must report zero storage_base_fee"
            ));
        }
        if let Some(member) = self.member.as_ref() {
            member
                .validate()
                .wrap_err("invalid Soracloud HF status lease member")?;
            let pool = self.pool.as_ref().ok_or_else(|| {
                eyre!("Soracloud HF status contains a member without a lease pool")
            })?;
            if member.pool_id != pool.pool_id || member.source_id != self.source.source_id {
                return Err(eyre!(
                    "Soracloud HF status member identifiers do not match the source and pool"
                ));
            }
        }
        if let Some(event) = self.latest_audit_event.as_ref() {
            event
                .validate()
                .wrap_err("invalid Soracloud HF status audit event")?;
            if self.audit_event_count == 0 || event.source_id != self.source.source_id {
                return Err(eyre!(
                    "Soracloud HF status latest audit event is inconsistent with its aggregate"
                ));
            }
            if let Some(pool) = self.pool.as_ref()
                && event.pool_id != pool.pool_id
            {
                return Err(eyre!(
                    "Soracloud HF status audit event pool_id does not match the lease pool"
                ));
            }
        }
        Ok(())
    }
}

impl AppInfraStatusResponseV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud app-infra status schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_STATUS_SCHEMA_VERSION_V1
            ));
        }
        exact_status_count(
            "Soracloud app-infra status app_count",
            self.app_count,
            self.apps.len(),
        )?;
        let recent_count = u32::try_from(self.recent_audit_events.len())
            .wrap_err("Soracloud app-infra recent audit list exceeds the V1 count range")?;
        if recent_count > self.audit_event_count {
            return Err(eyre!(
                "Soracloud app-infra recent audit count {recent_count} exceeds total audit_event_count {}",
                self.audit_event_count
            ));
        }
        let mut app_names = BTreeSet::new();
        for app in &self.apps {
            app.validate()
                .wrap_err("invalid Soracloud app-infra state entry")?;
            if !app_names.insert(app.app_name.clone()) {
                return Err(eyre!(
                    "Soracloud app-infra status contains duplicate app `{}`",
                    app.app_name
                ));
            }
        }
        let mut audit_sequences = BTreeSet::new();
        for event in &self.recent_audit_events {
            event
                .validate()
                .wrap_err("invalid Soracloud app-infra audit event")?;
            if !audit_sequences.insert(event.sequence) {
                return Err(eyre!(
                    "Soracloud app-infra status contains duplicate audit sequence {}",
                    event.sequence
                ));
            }
        }
        Ok(())
    }
}

fn decode_service_config_status(payload: &json::Value) -> Result<ServiceConfigStatusResponseV1> {
    let status: ServiceConfigStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud service-config status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_service_secret_status(payload: &json::Value) -> Result<ServiceSecretStatusResponseV1> {
    let status: ServiceSecretStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud service-secret status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_training_job_status(payload: &json::Value) -> Result<TrainingJobStatusResponseV1> {
    let status: TrainingJobStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud training-job status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_model_weight_status(payload: &json::Value) -> Result<ModelWeightStatusResponseV1> {
    let status: ModelWeightStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud model-weight status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_model_artifact_status(payload: &json::Value) -> Result<ModelArtifactStatusResponseV1> {
    let status: ModelArtifactStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud model-artifact status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_uploaded_model_status(payload: &json::Value) -> Result<UploadedModelStatusResponseV1> {
    let status: UploadedModelStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud uploaded-model status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_agent_status(payload: &json::Value) -> Result<AgentStatusResponseV1> {
    let status: AgentStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud agent status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_agent_mailbox_status(payload: &json::Value) -> Result<AgentMailboxStatusResponseV1> {
    let status: AgentMailboxStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud agent mailbox status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_agent_autonomy_status(payload: &json::Value) -> Result<AgentAutonomyStatusResponseV1> {
    let status: AgentAutonomyStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud agent autonomy status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_hf_shared_lease_status(payload: &json::Value) -> Result<HfSharedLeaseStatusResponseV1> {
    let status: HfSharedLeaseStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud HF shared-lease status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_app_infra_status(payload: &json::Value) -> Result<AppInfraStatusResponseV1> {
    let status: AppInfraStatusResponseV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud app-infra status V1")?;
    status.validate()?;
    Ok(status)
}

fn decode_network_control_plane_snapshot(
    network_status: &norito::json::Value,
) -> Result<(u16, NetworkControlPlaneSnapshotV1)> {
    let schema_version = network_status
        .get("schema_version")
        .and_then(json::Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| eyre!("Soracloud network status must carry an unsigned `schema_version`"))?;
    if schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
        return Err(eyre!(
            "unsupported Soracloud status schema version {schema_version}; expected {SORACLOUD_STATUS_SCHEMA_VERSION_V1}"
        ));
    }
    let control_plane_value = network_status
        .get("control_plane")
        .cloned()
        .ok_or_else(|| eyre!("Soracloud network status is missing `control_plane`"))?;
    let control_plane: NetworkControlPlaneSnapshotV1 =
        json::from_value(control_plane_value.clone())
            .wrap_err("failed to decode canonical Soracloud `control_plane` status")?;
    if control_plane.schema_version != SORACLOUD_STATUS_SCHEMA_VERSION_V1 {
        return Err(eyre!(
            "unsupported Soracloud control-plane schema version {}; expected {}",
            control_plane.schema_version,
            SORACLOUD_STATUS_SCHEMA_VERSION_V1
        ));
    }
    let decoded_service_count = u32::try_from(control_plane.services.len())
        .wrap_err("Soracloud control-plane service list exceeds the V1 count range")?;
    if control_plane.service_count != decoded_service_count {
        return Err(eyre!(
            "Soracloud control-plane service_count {} does not match {} decoded services",
            control_plane.service_count,
            decoded_service_count
        ));
    }
    let mut previous_active_validator: Option<&AccountId> = None;
    for (index, capability) in control_plane.active_inrou_hosts.iter().enumerate() {
        capability
            .validate()
            .map_err(|error| eyre!(error))
            .wrap_err_with(|| format!("invalid active Inrou host at index {index}"))?;
        if previous_active_validator
            .is_some_and(|previous| previous >= &capability.validator_account_id)
        {
            return Err(eyre!(
                "active Inrou host capabilities must be strictly ordered by validator account"
            ));
        }
        previous_active_validator = Some(&capability.validator_account_id);
    }
    let recent_audit_count = u32::try_from(control_plane.recent_audit_events.len())
        .wrap_err("Soracloud control-plane audit list exceeds the V1 count range")?;
    if recent_audit_count > control_plane.audit_event_count {
        return Err(eyre!(
            "Soracloud control-plane recent audit count {recent_audit_count} exceeds total audit_event_count {}",
            control_plane.audit_event_count
        ));
    }
    let encoded_audit_events = control_plane_value
        .get("recent_audit_events")
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("Soracloud control-plane audit history must be a V1 array"))?;
    for (index, (event, encoded)) in control_plane
        .recent_audit_events
        .iter()
        .zip(encoded_audit_events)
        .enumerate()
    {
        event.validate().wrap_err_with(|| {
            format!("invalid Soracloud control-plane recent audit event at index {index}")
        })?;
        let canonical = json::to_value(event).wrap_err_with(|| {
            format!("failed to re-encode Soracloud control-plane audit event at index {index}")
        })?;
        if &canonical != encoded {
            return Err(eyre!(
                "Soracloud control-plane audit event at index {index} is not exact canonical V1 JSON"
            ));
        }
    }
    Ok((schema_version, control_plane))
}
impl StatusOutput {
    fn from_network(
        endpoint: String,
        network_status: norito::json::Value,
        service_plan: Option<ServiceLocalPlanOutput>,
    ) -> Result<Self> {
        let (schema_version, control_plane) =
            decode_network_control_plane_snapshot(&network_status)?;
        let service_count = control_plane.service_count;
        let audit_event_count = control_plane.audit_event_count;
        let services = control_plane.services;
        Ok(Self {
            source: "torii_control_plane".to_owned(),
            torii_endpoint: Some(endpoint),
            schema_version: Some(schema_version),
            service_count: Some(service_count),
            audit_event_count: Some(audit_event_count),
            services,
            network_status: Some(network_status),
            service_plan,
        })
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppManifestV1 {
    schema_version: u16,
    app_name: String,
    #[norito(required)]
    app_version: Option<String>,
    public_url: String,
    #[norito(required)]
    static_site: Option<SoracloudAppStaticSiteV1>,
    services: Vec<SoracloudAppServiceRefV1>,
}
impl SoracloudAppManifestV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_APP_MANIFEST_VERSION_V1 {
            return Err(eyre!(
                "unsupported app manifest schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_APP_MANIFEST_VERSION_V1
            ));
        }
        parse_exact_name_arg("app manifest field `app_name`", &self.app_name)?;
        if let Some(app_version) = self.app_version.as_deref() {
            require_exact_nonempty_arg("app manifest field `app_version`", app_version)?;
        }
        if !(self.public_url.starts_with("https://") || self.public_url.starts_with("http://")) {
            return Err(eyre!(
                "app manifest field `public_url` must start with http:// or https://"
            ));
        }
        if self.services.is_empty() {
            return Err(eyre!("app manifest must declare at least one service"));
        }
        let mut seen_service_names = BTreeSet::new();
        for service in &self.services {
            service.validate()?;
            if !seen_service_names.insert(service.service_name.clone()) {
                return Err(eyre!(
                    "duplicate app service `{}` in app manifest",
                    service.service_name
                ));
            }
        }
        if let Some(static_site) = self.static_site.as_ref() {
            static_site.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppStaticSiteV1 {
    dist_dir: String,
    mount_path: String,
    publish_mode: String,
    #[norito(required)]
    api_base_path: Option<String>,
    #[norito(required)]
    publish_label: Option<String>,
}
const APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING: &str = "RootBinding";
const APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY: &str = "CidOnly";
impl SoracloudAppStaticSiteV1 {
    fn validate(&self) -> Result<()> {
        if self.dist_dir.trim().is_empty() {
            return Err(eyre!("app static site field `dist_dir` must not be empty"));
        }
        if !self.mount_path.starts_with('/') {
            return Err(eyre!(
                "app static site field `mount_path` must start with '/'"
            ));
        }
        if self.publish_mode != APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING
            && self.publish_mode != APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY
        {
            return Err(eyre!(
                "app static site field `publish_mode` must be `{APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING}` or `{APP_STATIC_SITE_PUBLISH_MODE_CID_ONLY}`"
            ));
        }
        if let Some(api_base_path) = self.api_base_path.as_deref()
            && !api_base_path.starts_with('/')
        {
            return Err(eyre!(
                "app static site field `api_base_path` must start with '/'"
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppServiceRefV1 {
    service_name: String,
    container_manifest: String,
    service_manifest: String,
    #[norito(required)]
    bundle_file: Option<String>,
    #[norito(required)]
    initial_configs: Option<String>,
    #[norito(required)]
    initial_secrets: Option<String>,
}
impl SoracloudAppServiceRefV1 {
    fn validate(&self) -> Result<()> {
        parse_exact_name_arg("app service field `service_name`", &self.service_name)?;
        if self.container_manifest.trim().is_empty() {
            return Err(eyre!(
                "app service `{}` field `container_manifest` must not be empty",
                self.service_name
            ));
        }
        if self.service_manifest.trim().is_empty() {
            return Err(eyre!(
                "app service `{}` field `service_manifest` must not be empty",
                self.service_name
            ));
        }
        if let Some(bundle_file) = self.bundle_file.as_deref()
            && bundle_file.trim().is_empty()
        {
            return Err(eyre!(
                "app service `{}` field `bundle_file` must not be empty when provided",
                self.service_name
            ));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppInitOutput {
    template: String,
    manifest_path: String,
    public_url: String,
    service_manifest_paths: Vec<String>,
    template_artifacts: Vec<String>,
}
const SORACLOUD_APP_REPORT_SCHEMA_VERSION: &str = "soracloud.app.report.v1";
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppPhaseReportV1 {
    name: String,
    ok: bool,
    skipped: bool,
    diagnostics: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppReportServiceV1 {
    service_name: String,
    execution_plane: String,
    runtime: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudAppReportV1 {
    schema_version: String,
    app_name: String,
    manifest_path: String,
    ok: bool,
    phases: Vec<SoracloudAppPhaseReportV1>,
    #[norito(required)]
    app_infra_manifest_hash: Option<Hash>,
    routes: Vec<AppLocalRoutePlanOutput>,
    services: Vec<SoracloudAppReportServiceV1>,
    #[norito(required)]
    static_site: Option<SoracloudAppStaticSiteV1>,
    blockers: Vec<String>,
    next_action: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppMutationOutput {
    app_name: String,
    manifest_path: String,
    public_url: String,
    hostname: String,
    workspace_dir: String,
    workspace_scripts: AppLocalWorkspaceScriptsOutput,
    mode: String,
    has_mixed_planes: bool,
    hosted_http_service_count: u32,
    deterministic_service_count: u32,
    #[norito(required)]
    static_site: Option<SoracloudAppStaticSiteV1>,
    #[norito(required)]
    published_static_site: Option<AppStaticSitePublishOutput>,
    #[norito(required)]
    frontend: Option<AppLocalFrontendPlanOutput>,
    synced_manifests: Vec<SyncManifestEntryOutput>,
    #[norito(required)]
    app_infra_manifest_hash: Option<Hash>,
    #[norito(required)]
    app_infra_response: Option<norito::json::Value>,
    services: Vec<AppServiceMutationOutput>,
    routes: Vec<AppLocalRoutePlanOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppServiceMutationOutput {
    service_name: String,
    container_manifest: String,
    service_manifest: String,
    workspace_dir: String,
    workspace_scripts: AppLocalServiceWorkspaceScriptsOutput,
    execution_plane: String,
    runtime: String,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_visibility: Option<String>,
    #[norito(required)]
    published_public_discovery: Option<PublicServiceDiscoveryPublishOutput>,
    published_bundle: ServiceBundlePublishOutput,
    published_inrou_guest_images: Vec<InrouGuestImageArtifactPublishOutput>,
    response: norito::json::Value,
    notes: Vec<String>,
}
macro_rules! define_service_workspace_output {
    ($name:ident, [$($network:tt)*]) => {
        #[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
        #[norito(deny_unknown_fields)]
        struct $name {
            service_name: String,
            container_manifest_path: String,
            service_manifest_path: String,
            working_dir: String,
            script_path: String,
            script_name: String,
            mode: String,
            execution_plane: String,
            runtime: String,
            workspace_scripts: ServiceWorkspaceScriptsOutput,
            #[norito(required)]
            route_host: Option<String>,
            #[norito(required)]
            route_path_prefix: Option<String>,
            #[norito(required)]
            route_visibility: Option<String>,
            replica_count: u16,
            state_binding_count: u32,
            lease_volume_count: u32,
            handler_count: u32,
            routes: Vec<ServiceLocalRouteOutput>,
            $($network)*
            command: Vec<String>,
            #[norito(required)]
            exit_status: Option<i32>,
            notes: Vec<String>,
        }
    };
}
define_service_workspace_output!(ServiceWorkspaceScriptOutput, []);
define_service_workspace_output!(
    ServiceWorkspaceMutationScriptOutput,
    [torii_url: String, uses_api_token: bool,]
);
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceMutationOutput {
    service_name: String,
    container_manifest_path: String,
    service_manifest_path: String,
    workspace_dir: String,
    workspace_scripts: ServiceWorkspaceScriptsOutput,
    mode: String,
    execution_plane: String,
    runtime: String,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_visibility: Option<String>,
    replica_count: u16,
    state_binding_count: u32,
    lease_volume_count: u32,
    handler_count: u32,
    torii_url: String,
    uses_api_token: bool,
    routes: Vec<ServiceLocalRouteOutput>,
    #[norito(required)]
    published_public_discovery: Option<PublicServiceDiscoveryPublishOutput>,
    published_bundle: ServiceBundlePublishOutput,
    published_inrou_guest_images: Vec<InrouGuestImageArtifactPublishOutput>,
    response: norito::json::Value,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceLocalPlanOutput {
    service_name: String,
    container_manifest_path: String,
    service_manifest_path: String,
    workspace_dir: String,
    workspace_scripts: ServiceWorkspaceScriptsOutput,
    execution_plane: String,
    runtime: String,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_visibility: Option<String>,
    replica_count: u16,
    state_binding_count: u32,
    lease_volume_count: u32,
    handler_count: u32,
    routes: Vec<ServiceLocalRouteOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceWorkspaceScriptsOutput {
    #[norito(required)]
    local_dev: Option<String>,
    #[norito(required)]
    build_and_sync: Option<String>,
    #[norito(required)]
    doctor: Option<String>,
    #[norito(required)]
    release: Option<String>,
    #[norito(required)]
    deploy: Option<String>,
    #[norito(required)]
    upgrade: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceLocalRouteOutput {
    route_kind: String,
    host: String,
    path: String,
    #[norito(required)]
    handler_name: Option<String>,
    #[norito(required)]
    handler_class: Option<String>,
    #[norito(required)]
    certified_response: Option<String>,
    #[norito(required)]
    mailbox_queue: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppStatusOutput {
    report: SoracloudAppReportV1,
    app_name: String,
    manifest_path: String,
    public_url: String,
    hostname: String,
    workspace_dir: String,
    workspace_scripts: AppLocalWorkspaceScriptsOutput,
    source: String,
    #[norito(required)]
    torii_endpoint: Option<String>,
    #[norito(required)]
    static_site: Option<SoracloudAppStaticSiteV1>,
    #[norito(required)]
    frontend: Option<AppLocalFrontendPlanOutput>,
    has_mixed_planes: bool,
    hosted_http_service_count: u32,
    deterministic_service_count: u32,
    #[norito(required)]
    app_infra_status: Option<norito::json::Value>,
    services: Vec<AppServiceStatusOutput>,
    routes: Vec<AppLocalRoutePlanOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppDoctorCheckOutput {
    name: String,
    status: String,
    detail: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppDoctorOutput {
    report: SoracloudAppReportV1,
    app_name: String,
    manifest_path: String,
    public_url: String,
    hostname: String,
    workspace_dir: String,
    workspace_scripts: AppLocalWorkspaceScriptsOutput,
    ok: bool,
    has_mixed_planes: bool,
    hosted_http_service_count: u32,
    deterministic_service_count: u32,
    #[norito(required)]
    frontend: Option<AppLocalFrontendPlanOutput>,
    services: Vec<AppLocalServicePlanOutput>,
    routes: Vec<AppLocalRoutePlanOutput>,
    checks: Vec<AppDoctorCheckOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLiveVerificationOutput {
    label: String,
    url: String,
    status_code: u16,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppReleaseOutput {
    report: SoracloudAppReportV1,
    mode: String,
    release_mode: String,
    torii_url: String,
    uses_api_token: bool,
    plan: AppLocalPlanOutput,
    build_and_sync: AppBuildAndSyncOutput,
    #[norito(required)]
    release_response: Option<AppMutationOutput>,
    #[norito(required)]
    status_response: Option<AppStatusOutput>,
    live_verifications: Vec<AppLiveVerificationOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppSimulateOutput {
    report: SoracloudAppReportV1,
    mode: String,
    plan: AppLocalPlanOutput,
    synced_manifests: Vec<SyncManifestEntryOutput>,
    #[norito(required)]
    planned_static_site: Option<AppStaticSitePublishOutput>,
    #[norito(required)]
    app_infra_request: Option<norito::json::Value>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppServiceStatusOutput {
    service_name: String,
    container_manifest_path: String,
    service_manifest_path: String,
    workspace_dir: String,
    workspace_scripts: AppLocalServiceWorkspaceScriptsOutput,
    execution_plane: String,
    runtime: String,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_visibility: Option<String>,
    present_in_control_plane: bool,
    #[norito(required)]
    status: Option<norito::json::Value>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalPlanOutput {
    app_name: String,
    manifest_path: String,
    public_url: String,
    hostname: String,
    has_mixed_planes: bool,
    hosted_http_service_count: u32,
    deterministic_service_count: u32,
    #[norito(required)]
    frontend: Option<AppLocalFrontendPlanOutput>,
    workspace_dir: String,
    workspace_scripts: AppLocalWorkspaceScriptsOutput,
    services: Vec<AppLocalServicePlanOutput>,
    routes: Vec<AppLocalRoutePlanOutput>,
    notes: Vec<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalWorkspaceScriptsOutput {
    #[norito(required)]
    local_dev: Option<String>,
    #[norito(required)]
    build_and_sync: Option<String>,
    #[norito(required)]
    doctor: Option<String>,
    #[norito(required)]
    release: Option<String>,
}
macro_rules! define_app_workspace_output {
    ($(#[$meta:meta])* $name:ident, [$($before_mode:tt)*], [$($after_mode:tt)*]) => {
        $(#[$meta])*
        #[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
        #[norito(deny_unknown_fields)]
        struct $name {
            app_name: String,
            public_url: String,
            hostname: String,
            manifest_path: String,
            workspace_dir: String,
            workspace_scripts: AppLocalWorkspaceScriptsOutput,
            working_dir: String,
            script_path: String,
            $($before_mode)*
            mode: String,
            $($after_mode)*
            has_mixed_planes: bool,
            hosted_http_service_count: u32,
            deterministic_service_count: u32,
            #[norito(required)]
            frontend: Option<AppLocalFrontendPlanOutput>,
            services: Vec<AppLocalServicePlanOutput>,
            routes: Vec<AppLocalRoutePlanOutput>,
            command: Vec<String>,
            #[norito(required)]
            exit_status: Option<i32>,
            notes: Vec<String>,
        }
    };
}
define_app_workspace_output!(AppLocalDevOutput, [], []);
define_app_workspace_output!(AppBuildAndSyncOutput, [], []);
define_app_workspace_output!(
    #[cfg(test)]
    AppReleaseWorkspaceScriptOutput,
    [script_name: String,],
    [torii_url: String, uses_api_token: bool,]
);
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalFrontendPlanOutput {
    dist_dir: String,
    mount_path: String,
    publish_mode: String,
    #[norito(required)]
    api_base_path: Option<String>,
    #[norito(required)]
    cid_gateway_url_template: Option<String>,
    #[norito(required)]
    root_binding_url: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalServicePlanOutput {
    service_name: String,
    container_manifest_path: String,
    service_manifest_path: String,
    workspace_dir: String,
    workspace_scripts: AppLocalServiceWorkspaceScriptsOutput,
    execution_plane: String,
    runtime: String,
    #[norito(required)]
    route_host: Option<String>,
    #[norito(required)]
    route_path_prefix: Option<String>,
    #[norito(required)]
    route_visibility: Option<String>,
    replica_count: u16,
    state_binding_count: u32,
    lease_volume_count: u32,
    handler_count: u32,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalServiceWorkspaceScriptsOutput {
    #[norito(required)]
    dev: Option<String>,
    #[norito(required)]
    build: Option<String>,
    #[norito(required)]
    verify_build: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppLocalRoutePlanOutput {
    service_name: String,
    route_kind: String,
    host: String,
    path: String,
    #[norito(required)]
    handler_name: Option<String>,
    #[norito(required)]
    handler_class: Option<String>,
    #[norito(required)]
    certified_response: Option<String>,
    #[norito(required)]
    mailbox_queue: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppStaticSiteBindingV1 {
    schema_version: u16,
    app_name: String,
    public_url: String,
    hostname: String,
    mount_path: String,
    index_document: String,
    spa_fallback: bool,
    manifest_digest_hex: String,
    #[norito(required)]
    api_base_path: Option<String>,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AppStaticSitePublishOutput {
    hostname: String,
    public_url: String,
    cid_gateway_url: String,
    content_cid: String,
    manifest_digest_hex: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct InrouGuestImageArtifactPublishOutput {
    service_name: String,
    guest_isa: String,
    source_dir: String,
    hydrate_mount_path: String,
    member_paths: Vec<String>,
    content_cid: String,
    manifest_digest_hex: String,
    note: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceBundlePublishOutput {
    service_name: String,
    bundle_file: String,
    content_cid: String,
    manifest_digest_hex: String,
    bundle_hash: String,
    note: String,
}
#[derive(Clone, Debug)]
struct PublishedSorafsDirectoryArtifact {
    content_cid: String,
    manifest_digest_hex: String,
}
#[derive(Clone, Debug)]
struct PublishedSorafsFileArtifact {
    content_cid: String,
    manifest_digest_hex: String,
    payload_hash: Hash,
}
#[derive(Clone, Debug)]
struct AppStaticSiteRootBindingPlan {
    target_host: String,
    binding_value: Json,
}
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudPublicServiceDiscoveryDocumentV1 {
    schema_version: u16,
    service_name: String,
    service_version: String,
    execution_plane: String,
    runtime: String,
    route_host: String,
    path_prefix: String,
    base_url: String,
    #[norito(required)]
    healthcheck_path: Option<String>,
    #[norito(required)]
    healthcheck_url: Option<String>,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    deployment_bundle_hash: Hash,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudPublicServiceDiscoveryV1 {
    schema_version: u16,
    service_name: String,
    service_version: String,
    execution_plane: String,
    runtime: String,
    route_host: String,
    path_prefix: String,
    base_url: String,
    #[norito(required)]
    healthcheck_path: Option<String>,
    #[norito(required)]
    healthcheck_url: Option<String>,
    service_manifest_hash: Hash,
    container_manifest_hash: Hash,
    deployment_bundle_hash: Hash,
    document_hash: Hash,
    content_cid: String,
    public_discovery_url: String,
    public_discovery_cid_host_url: String,
    manifest_digest_hex: String,
}
fn public_service_discovery_document_from_projection(
    discovery: &SoracloudPublicServiceDiscoveryV1,
) -> SoracloudPublicServiceDiscoveryDocumentV1 {
    SoracloudPublicServiceDiscoveryDocumentV1 {
        schema_version: discovery.schema_version,
        service_name: discovery.service_name.clone(),
        service_version: discovery.service_version.clone(),
        execution_plane: discovery.execution_plane.clone(),
        runtime: discovery.runtime.clone(),
        route_host: discovery.route_host.clone(),
        path_prefix: discovery.path_prefix.clone(),
        base_url: discovery.base_url.clone(),
        healthcheck_path: discovery.healthcheck_path.clone(),
        healthcheck_url: discovery.healthcheck_url.clone(),
        service_manifest_hash: discovery.service_manifest_hash,
        container_manifest_hash: discovery.container_manifest_hash,
        deployment_bundle_hash: discovery.deployment_bundle_hash,
    }
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudPublicServiceDiscoveryRegistryV1 {
    schema_version: u16,
    service_name: String,
    current_version: String,
    revisions: BTreeMap<String, SoracloudPublicServiceDiscoveryV1>,
}

fn taira_inrou_canary_public_discovery_config_value(
    discovery: &SoracloudPublicServiceDiscoveryV1,
) -> Result<Json> {
    let registry = SoracloudPublicServiceDiscoveryRegistryV1 {
        schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
        service_name: discovery.service_name.clone(),
        current_version: discovery.service_version.clone(),
        revisions: BTreeMap::from([(discovery.service_version.clone(), discovery.clone())]),
    };
    Ok(Json::from(json::to_value(&registry).wrap_err(
        "encode canonical Taira public-discovery registry config",
    )?))
}

fn validate_taira_inrou_canary_public_discovery_config(
    config: &Json,
    bundle: &SoraDeploymentBundleV1,
    stage: &TairaInrouStageIdentity,
) -> Result<()> {
    let registry: SoracloudPublicServiceDiscoveryRegistryV1 = json::from_str(config.as_ref())
        .wrap_err("decode exact Taira public-discovery registry config")?;
    if registry.schema_version != PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1
        || registry.service_name != stage.service_name
        || registry.current_version != stage.service_version
        || registry.revisions.len() != 1
    {
        return Err(eyre!(
            "Taira public-discovery registry must contain exactly the current canary revision"
        ));
    }
    let discovery = registry
        .revisions
        .get(&stage.service_version)
        .ok_or_else(|| {
            eyre!("Taira public-discovery registry omits the current canary revision")
        })?;
    let route = bundle
        .service
        .route
        .as_ref()
        .ok_or_else(|| eyre!("Taira public-discovery bundle omits its public route"))?;
    let base_url = normalize_public_service_base_url(bundle)?;
    let healthcheck_path = bundle.container.lifecycle.healthcheck_path.clone();
    let healthcheck_url = healthcheck_path.as_ref().map(|path| {
        let mut url = base_url.clone();
        url.set_path(&join_service_route_path(&route.path_prefix, path));
        url.set_query(None);
        url.set_fragment(None);
        url.to_string()
    });
    let expected_document = SoracloudPublicServiceDiscoveryDocumentV1 {
        schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
        service_name: bundle.service.service_name.to_string(),
        service_version: bundle.service.service_version.clone(),
        execution_plane: format!("{:?}", bundle.service.execution_plane),
        runtime: format!("{:?}", bundle.container.runtime),
        route_host: route.host.clone(),
        path_prefix: route.path_prefix.clone(),
        base_url: base_url.to_string(),
        healthcheck_path,
        healthcheck_url,
        service_manifest_hash: bundle.service_manifest_hash(),
        container_manifest_hash: bundle.container_manifest_hash(),
        deployment_bundle_hash: Hash::new(Encode::encode(bundle)),
    };
    let expected_document_hash = Hash::new(
        json::to_vec(&expected_document)
            .wrap_err("encode exact Taira public-discovery document")?,
    );
    let mut expected_public_url = base_url.clone();
    expected_public_url.set_path(&format!(
        "/sorafs/cid/{}/{PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT}",
        stage.discovery_content_cid
    ));
    let cid_host_suffix = sorafs_cid_host_suffix_for_hostname(&route.host);
    if cid_host_suffix != "sorafs.taira.sora.org" {
        return Err(eyre!(
            "Taira public-discovery route did not resolve to the authoritative public CID-host suffix"
        ));
    }
    let expected_cid_host_url = format!(
        "https://{}.{cid_host_suffix}/{PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT}",
        stage.discovery_content_cid
    );
    if public_service_discovery_document_from_projection(discovery) != expected_document
        || expected_document.deployment_bundle_hash.to_string() != stage.deployment_bundle_hash
        || discovery.document_hash != expected_document_hash
        || discovery.document_hash.to_string() != stage.discovery_document_hash
        || discovery.content_cid != stage.discovery_content_cid
        || discovery.manifest_digest_hex != stage.discovery_manifest_digest_hex
        || discovery.public_discovery_url != stage.public_discovery_url
        || discovery.public_discovery_url != expected_public_url.to_string()
        || discovery.public_discovery_cid_host_url != stage.public_discovery_cid_host_url
        || discovery.public_discovery_cid_host_url != expected_cid_host_url
    {
        return Err(eyre!(
            "Taira public-discovery registry differs from the exact retained canary stage"
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PublicServiceDiscoveryPublishOutput {
    service_name: String,
    service_version: String,
    route_host: String,
    base_url: String,
    #[norito(required)]
    healthcheck_url: Option<String>,
    document_hash: Hash,
    content_cid: String,
    public_discovery_url: String,
    public_discovery_cid_host_url: String,
    manifest_digest_hex: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ServiceConfigSetPayload {
    service_name: String,
    config_name: String,
    value_json: Json,
}
macro_rules! signed_request_types {
    ($($request:ident => $payload:ident;)+) => {
        $(
            #[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
            #[norito(deny_unknown_fields)]
            struct $request {
                payload: $payload,
                provenance: ManifestProvenance,
            }
        )+
    };
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ServiceConfigDeletePayload {
    service_name: String,
    config_name: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ServiceSecretSetPayload {
    service_name: String,
    secret_name: String,
    secret: SecretEnvelopeV1,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ServiceSecretDeletePayload {
    service_name: String,
    secret_name: String,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignedBundleRequest {
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    precondition: SoraServiceMutationPreconditionV1,
    provenance: ManifestProvenance,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignedAppInfraRequest {
    deploy_services: Vec<SignedBundleRequest>,
    upgrade_services: Vec<SignedBundleRequest>,
    manifest: SoraAppInfraManifestV1,
    precondition: SoraAppInfraMutationPreconditionV1,
    provenance: ManifestProvenance,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct RollbackPayload {
    service_name: String,
    target_version: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct RolloutAdvancePayload {
    service_name: String,
    rollout_handle: String,
    healthy: bool,
    #[norito(required)]
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentDeployPayload {
    manifest: AgentApartmentManifestV1,
    lease_ticks: u64,
    autonomy_budget_units: u64,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentLeaseRenewPayload {
    apartment_name: String,
    lease_ticks: u64,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct HfSharedLeaseJoinPayload {
    repo_id: String,
    revision: String,
    service_name: String,
    #[norito(required)]
    apartment_name: Option<String>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: AssetDefinitionId,
    base_fee: Quantity,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignedHfSharedLeaseJoinRequest {
    payload: HfSharedLeaseJoinPayload,
    provenance: ManifestProvenance,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct HfLeaseLeavePayload {
    repo_id: String,
    revision: String,
    storage_class: StorageClass,
    lease_term_ms: u64,
    #[norito(required)]
    service_name: Option<String>,
    #[norito(required)]
    apartment_name: Option<String>,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct HfLeaseRenewPayload {
    repo_id: String,
    revision: String,
    service_name: String,
    #[norito(required)]
    apartment_name: Option<String>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition_id: AssetDefinitionId,
    base_fee: Quantity,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignedHfLeaseRenewRequest {
    payload: HfLeaseRenewPayload,
    provenance: ManifestProvenance,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentRestartPayload {
    apartment_name: String,
    reason: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentPolicyRevokePayload {
    apartment_name: String,
    capability: String,
    #[norito(required)]
    reason: Option<String>,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentWalletSpendPayload {
    apartment_name: String,
    request_id: String,
    asset_definition: String,
    amount: Quantity,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentWalletApprovePayload {
    apartment_name: String,
    request_id: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentMessageSendPayload {
    from_apartment: String,
    to_apartment: String,
    channel: String,
    payload: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentMessageAckPayload {
    apartment_name: String,
    message_id: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct AgentArtifactAllowPayload {
    apartment_name: String,
    artifact_hash: String,
    #[norito(required)]
    provenance_hash: Option<String>,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct TrainingJobStartPayload {
    service_name: String,
    model_name: String,
    job_id: String,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct TrainingJobCheckpointPayload {
    service_name: String,
    job_id: String,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct TrainingJobRetryPayload {
    service_name: String,
    job_id: String,
    reason: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ModelArtifactRegisterPayload {
    service_name: String,
    model_name: String,
    training_job_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ModelWeightRegisterPayload {
    service_name: String,
    model_name: String,
    weight_version: String,
    training_job_id: String,
    #[norito(required)]
    parent_version: Option<String>,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ModelWeightPromotePayload {
    service_name: String,
    model_name: String,
    weight_version: String,
    gate_approved: bool,
    gate_report_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct ModelWeightRollbackPayload {
    service_name: String,
    model_name: String,
    target_version: String,
    reason: String,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct UploadedModelFinalizePayload {
    service_name: String,
    model_name: String,
    model_id: String,
    artifact_id: String,
    weight_version: String,
    bundle_root: Hash,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}
#[derive(
    Clone,
    Debug,
    JsonSerialize,
    JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
#[norito(deny_unknown_fields)]
struct UploadedModelRegisterPayload {
    bundle: SoraUploadedModelBundleV1,
    model_name: String,
    artifact_id: String,
    weight_artifact_hash: Hash,
    dataset_ref: String,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
}
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SignedUploadedModelRegisterRequest {
    payload: UploadedModelRegisterPayload,
    bundle_provenance: ManifestProvenance,
    finalize_provenance: ManifestProvenance,
}
signed_request_types! {
    SignedServiceConfigSetRequest => ServiceConfigSetPayload;
    SignedServiceConfigDeleteRequest => ServiceConfigDeletePayload;
    SignedServiceSecretSetRequest => ServiceSecretSetPayload;
    SignedServiceSecretDeleteRequest => ServiceSecretDeletePayload;
    SignedRollbackRequest => RollbackPayload;
    SignedRolloutAdvanceRequest => RolloutAdvancePayload;
    SignedAgentDeployRequest => AgentDeployPayload;
    SignedAgentLeaseRenewRequest => AgentLeaseRenewPayload;
    SignedHfLeaseLeaveRequest => HfLeaseLeavePayload;
    SignedAgentRestartRequest => AgentRestartPayload;
    SignedAgentPolicyRevokeRequest => AgentPolicyRevokePayload;
    SignedAgentWalletSpendRequest => AgentWalletSpendPayload;
    SignedAgentWalletApproveRequest => AgentWalletApprovePayload;
    SignedAgentMessageSendRequest => AgentMessageSendPayload;
    SignedAgentMessageAckRequest => AgentMessageAckPayload;
    SignedAgentArtifactAllowRequest => AgentArtifactAllowPayload;
    SignedTrainingJobStartRequest => TrainingJobStartPayload;
    SignedTrainingJobCheckpointRequest => TrainingJobCheckpointPayload;
    SignedTrainingJobRetryRequest => TrainingJobRetryPayload;
    SignedModelArtifactRegisterRequest => ModelArtifactRegisterPayload;
    SignedModelWeightRegisterRequest => ModelWeightRegisterPayload;
    SignedModelWeightPromoteRequest => ModelWeightPromotePayload;
    SignedModelWeightRollbackRequest => ModelWeightRollbackPayload;
}
struct SoracloudTempDir {
    path: PathBuf,
}
impl SoracloudTempDir {
    fn new(prefix: &str) -> Result<Self> {
        Self::new_with_rng(prefix, &mut OsRng)
    }
    fn new_with_rng<R: TryCryptoRng + ?Sized>(prefix: &str, rng: &mut R) -> Result<Self> {
        for _ in 0..8 {
            let mut suffix = [0_u8; 8];
            rng.try_fill_bytes(&mut suffix).map_err(|error| {
                eyre!("Soracloud temporary directory suffix OS RNG failed: {error}")
            })?;
            let path = std::env::temp_dir().join(format!("{prefix}-{}", hex::encode(suffix)));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .wrap_err_with(|| format!("failed to create `{}`", path.display()));
                }
            }
        }
        Err(eyre!(
            "failed to allocate a unique Soracloud temporary directory"
        ))
    }
    fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for SoracloudTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
fn run_service_bundle_mutation(
    mode: MutationMode,
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    precondition: SoraServiceMutationPreconditionV1,
    torii_url: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
    authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<norito::json::Value> {
    let request = signed_bundle_request(
        bundle,
        initial_service_configs,
        initial_service_secrets,
        precondition,
        Some(authority),
        key_pair,
    )?;
    run_signed_service_bundle_mutation(mode, &request, torii_url, api_token, timeout_secs)
}
fn run_signed_service_bundle_mutation(
    mode: MutationMode,
    request: &SignedBundleRequest,
    torii_url: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<norito::json::Value> {
    let service_name = request.bundle.service.service_name.to_string();
    let endpoint_path = match mode {
        MutationMode::Deploy => "v1/soracloud/deploy",
        MutationMode::Upgrade => "v1/soracloud/upgrade",
    };
    let (_, payload) =
        post_torii_soracloud_mutation(torii_url, endpoint_path, request, api_token, timeout_secs)?;
    let (_, status_payload) =
        fetch_torii_soracloud_status(torii_url, Some(&service_name), api_token, timeout_secs)?;
    build_service_mutation_output(
        payload,
        &status_payload,
        &service_name,
        match mode {
            MutationMode::Deploy => "Deploy",
            MutationMode::Upgrade => "Upgrade",
        },
    )
}
fn run_app_infra_mutation(
    mode: MutationMode,
    request: &SignedAppInfraRequest,
    torii_url: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<norito::json::Value> {
    let endpoint_path = match mode {
        MutationMode::Deploy => "v1/soracloud/apps/deploy",
        MutationMode::Upgrade => "v1/soracloud/apps/upgrade",
    };
    let (endpoint, payload) =
        post_torii_soracloud_mutation(torii_url, endpoint_path, request, api_token, timeout_secs)?;
    decode_soracloud_submission_receipt(&payload)?;
    Ok(norito::json!({
        "app_infra_endpoint": endpoint,
        "app_infra_manifest_hash": (request.manifest.manifest_hash()),
        "submission": payload,
    }))
}
fn build_app_static_site_binding_value(
    app_name: &str,
    _public_url: &str,
    publication: &AppStaticSitePublishOutput,
    static_site: &SoracloudAppStaticSiteV1,
) -> Result<Json> {
    let binding = AppStaticSiteBindingV1 {
        schema_version: APP_STATIC_SITE_BINDING_SCHEMA_VERSION_V1,
        app_name: app_name.to_owned(),
        public_url: publication.public_url.clone(),
        hostname: publication.hostname.clone(),
        mount_path: static_site.mount_path.clone(),
        index_document: APP_STATIC_SITE_INDEX_DOCUMENT.to_owned(),
        spa_fallback: true,
        manifest_digest_hex: publication.manifest_digest_hex.clone(),
        api_base_path: static_site.api_base_path.clone(),
    };
    Ok(Json::from(json::to_value(&binding).wrap_err(
        "failed to encode app static site binding JSON",
    )?))
}
fn plan_app_static_site_root_binding(
    app_name: &str,
    public_url: &str,
    static_site: Option<&SoracloudAppStaticSiteV1>,
    publication: Option<&AppStaticSitePublishOutput>,
) -> Result<Option<AppStaticSiteRootBindingPlan>> {
    match (static_site, publication) {
        (Some(static_site), Some(publication))
            if static_site.publish_mode == APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING =>
        {
            Ok(Some(AppStaticSiteRootBindingPlan {
                target_host: publication.hostname.clone(),
                binding_value: build_app_static_site_binding_value(
                    app_name,
                    public_url,
                    publication,
                    static_site,
                )?,
            }))
        }
        _ => Ok(None),
    }
}
fn apply_app_static_site_root_binding(
    service_name: &str,
    service_manifest: &SoraServiceManifestV1,
    initial_service_configs: &mut BTreeMap<String, Json>,
    binding_plan: Option<&AppStaticSiteRootBindingPlan>,
    binding_already_attached: bool,
) -> Result<bool> {
    if initial_service_configs.contains_key(APP_STATIC_SITE_CONFIG_NAME) {
        return Err(eyre!(
            "app service `{}` initial configs may not set reserved config `{APP_STATIC_SITE_CONFIG_NAME}`",
            service_name
        ));
    }
    let Some(binding_plan) = binding_plan else {
        return Ok(false);
    };
    if binding_already_attached {
        return Ok(false);
    }
    let route_matches_static_site = service_manifest.route.as_ref().is_some_and(|route| {
        route.visibility == SoraRouteVisibilityV1::Public
            && route
                .host
                .eq_ignore_ascii_case(binding_plan.target_host.as_str())
    });
    if !route_matches_static_site {
        return Ok(false);
    }
    initial_service_configs.insert(
        APP_STATIC_SITE_CONFIG_NAME.to_owned(),
        binding_plan.binding_value.clone(),
    );
    Ok(true)
}
fn ensure_app_static_site_root_binding_attached(
    binding_plan: Option<&AppStaticSiteRootBindingPlan>,
    binding_attached: bool,
) -> Result<()> {
    if let Some(binding_plan) = binding_plan
        && !binding_attached
    {
        return Err(eyre!(
            "app static site host `{}` has no matching public service route in the app manifest",
            binding_plan.target_host
        ));
    }
    Ok(())
}
fn normalize_app_public_origin_url(public_url: &str, manifest_path: &Path) -> Result<reqwest::Url> {
    let mut public_origin = reqwest::Url::parse(public_url).wrap_err_with(|| {
        format!(
            "app manifest `{}` field `public_url` is not a valid URL: {public_url}",
            manifest_path.display()
        )
    })?;
    public_origin.set_query(None);
    public_origin.set_fragment(None);
    public_origin.set_path("/");
    if public_origin.host_str().is_none() {
        return Err(eyre!(
            "app manifest `{}` field `public_url` must include a hostname",
            manifest_path.display()
        ));
    }
    Ok(public_origin)
}
fn join_service_route_path(path_prefix: &str, route_path: &str) -> String {
    if route_path == "/" {
        return format!("{}/", path_prefix.trim_end_matches('/'));
    }
    let trimmed_prefix = path_prefix.trim_end_matches('/');
    if trimmed_prefix.is_empty() {
        return route_path.to_owned();
    }
    format!("{trimmed_prefix}/{}", route_path.trim_start_matches('/'))
}

#[derive(Clone, Debug)]
struct AppLiveVerificationTarget {
    label: String,
    url: reqwest::Url,
}

fn app_live_verification_targets(manifest_path: &Path) -> Result<Vec<AppLiveVerificationTarget>> {
    let manifest: SoracloudAppManifestV1 = load_json(manifest_path)?;
    manifest.validate()?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let public_origin = normalize_app_public_origin_url(&manifest.public_url, manifest_path)?;
    let public_hostname = public_origin
        .host_str()
        .ok_or_else(|| eyre!("app public URL is missing its hostname"))?;
    let mut targets = BTreeMap::<String, String>::new();

    for service_ref in &manifest.services {
        let container_path = resolve_manifest_path(manifest_dir, &service_ref.container_manifest);
        let service_path = resolve_manifest_path(manifest_dir, &service_ref.service_manifest);
        let container: UnpublishedContainerManifestV1 = load_json(&container_path)?;
        let service: SoraServiceManifestV1 = load_json(&service_path)?;
        ensure_app_service_ref_matches_manifest_name(
            &service_ref.service_name,
            &service_path,
            &service,
        )?;
        if service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
            || container.runtime != SoraContainerRuntimeV1::Inrou
        {
            continue;
        }
        let route = service.route.as_ref().ok_or_else(|| {
            eyre!(
                "hosted app service `{}` is missing its public route",
                service_ref.service_name
            )
        })?;
        if route.visibility != SoraRouteVisibilityV1::Public {
            return Err(eyre!(
                "hosted app service `{}` must expose a public route for release verification",
                service_ref.service_name
            ));
        }
        if !route.host.eq_ignore_ascii_case(public_hostname) {
            return Err(eyre!(
                "hosted app service `{}` route host `{}` does not match app public host `{public_hostname}`",
                service_ref.service_name,
                route.host
            ));
        }
        let healthcheck_path = container.lifecycle.healthcheck_path.as_deref().ok_or_else(|| {
            eyre!(
                "hosted app service `{}` must declare a healthcheck_path for live release verification",
                service_ref.service_name
            )
        })?;
        let mut url = public_origin.clone();
        url.set_path(&join_service_route_path(
            &route.path_prefix,
            healthcheck_path,
        ));
        url.set_query(None);
        url.set_fragment(None);
        targets.insert(
            url.to_string(),
            format!("service `{}` healthcheck", service_ref.service_name),
        );
    }

    if manifest
        .static_site
        .as_ref()
        .is_some_and(|site| site.publish_mode == APP_STATIC_SITE_PUBLISH_MODE_ROOT_BINDING)
    {
        targets.insert(
            public_origin.to_string(),
            "root-bound static site".to_owned(),
        );
    }
    if targets.is_empty() {
        return Err(eyre!(
            "app release has no live verification target; V1 requires a public Inrou healthcheck or a root-bound static site"
        ));
    }
    targets
        .into_iter()
        .map(|(url, label)| {
            Ok(AppLiveVerificationTarget {
                label,
                url: reqwest::Url::parse(&url)
                    .wrap_err("failed to retain canonical app verification URL")?,
            })
        })
        .collect()
}

fn verify_app_live_targets(
    targets: &[AppLiveVerificationTarget],
    timeout_secs: u64,
) -> Result<Vec<AppLiveVerificationOutput>> {
    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .wrap_err("failed to build exact app live-verification HTTP client")?;
    let mut verified = Vec::with_capacity(targets.len());
    for target in targets {
        let deadline = Instant::now() + timeout;
        loop {
            let last_observation = match client
                .get(target.url.clone())
                .header(header::ACCEPT, HeaderValue::from_static("*/*"))
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    verified.push(AppLiveVerificationOutput {
                        label: target.label.clone(),
                        url: target.url.to_string(),
                        status_code: response.status().as_u16(),
                    });
                    break;
                }
                Ok(response) => format!("HTTP {}", response.status()),
                Err(error) => error.to_string(),
            };
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "live verification for {} at `{}` did not return a 2xx response within {} second(s): {last_observation}",
                    target.label,
                    target.url,
                    timeout_secs.max(1)
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    Ok(verified)
}

fn service_uses_public_inrou_http_route(bundle: &SoraDeploymentBundleV1) -> bool {
    bundle.service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
        && bundle.container.runtime == SoraContainerRuntimeV1::Inrou
        && bundle
            .service
            .route
            .as_ref()
            .is_some_and(|route| route.visibility == SoraRouteVisibilityV1::Public)
}
fn normalize_public_service_base_url(bundle: &SoraDeploymentBundleV1) -> Result<reqwest::Url> {
    let route = bundle.service.route.as_ref().ok_or_else(|| {
        eyre!(
            "service `{}` does not declare a public route",
            bundle.service.service_name
        )
    })?;
    let scheme = match route.tls_mode {
        SoraTlsModeV1::Disabled => "http",
        SoraTlsModeV1::Optional | SoraTlsModeV1::Required => "https",
    };
    let mut base_url = reqwest::Url::parse(&format!("{scheme}://{}", route.host))
        .wrap_err_with(|| format!("invalid public route host `{}`", route.host))?;
    let route_root = if route.path_prefix.trim().is_empty() {
        "/".to_owned()
    } else if route.path_prefix.ends_with('/') {
        route.path_prefix.clone()
    } else {
        format!("{}/", route.path_prefix)
    };
    base_url.set_path(&route_root);
    base_url.set_query(None);
    base_url.set_fragment(None);
    Ok(base_url)
}
fn sorafs_cid_host_suffix_for_hostname(hostname: &str) -> String {
    let hostname = hostname.trim().trim_end_matches('.').to_ascii_lowercase();
    if hostname == "taira.sora.org" || hostname.ends_with(".taira.sora.org") {
        return "sorafs.taira.sora.org".to_owned();
    }
    if hostname == "sora.org" || hostname.ends_with(".sora.org") {
        return "sorafs.sora.org".to_owned();
    }
    if let Some((_, suffix)) = hostname.split_once('.') {
        return format!("sorafs.{suffix}");
    }
    format!("sorafs.{hostname}")
}
fn fetch_existing_public_service_discovery_registry(
    torii_url: &str,
    service_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<Option<SoracloudPublicServiceDiscoveryRegistryV1>> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/service/config/status")
        .wrap_err("failed to derive /v1/soracloud/service/config/status URL from --torii-url")?;
    {
        let mut query = endpoint.query_pairs_mut();
        query.append_pair("service_name", service_name.as_str());
        query.append_pair("config_name", PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME);
    }
    let (_, status, content_type, body) = send_torii_soracloud_authenticated_get(
        endpoint,
        api_token,
        timeout_secs,
        "Torii public-discovery config status",
    )?;
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let payload = decode_exact_torii_json_success(
        status,
        content_type.as_ref(),
        &body,
        "Torii public-discovery config status",
    )?;
    let status = decode_service_config_status(&payload)?;
    if status.service_name.to_string() != service_name
        || status.configs.len() != 1
        || status.configs[0].config_name != PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME
    {
        return Err(eyre!(
            "successful public-discovery config status must contain exactly the requested config entry"
        ));
    }
    let registry: SoracloudPublicServiceDiscoveryRegistryV1 =
        json::from_value(status.configs[0].value_json.clone())
            .wrap_err("failed to decode public service discovery registry JSON")?;
    if registry.schema_version != PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1
        || registry.service_name != service_name
        || registry.current_version.trim() != registry.current_version
        || registry.current_version.is_empty()
        || !registry.revisions.contains_key(&registry.current_version)
    {
        return Err(eyre!(
            "public service discovery registry contains inconsistent V1 identity or revision metadata"
        ));
    }
    Ok(Some(registry))
}
fn encode_sorafs_manifest_for_storage(manifest: &ManifestV1) -> Result<(Vec<u8>, blake3::Hash)> {
    let manifest_bytes = manifest
        .encode()
        .wrap_err("failed to encode SoraFS manifest")?;
    let manifest_digest = blake3::hash(&manifest_bytes);
    Ok((manifest_bytes, manifest_digest))
}
fn sign_soracloud_payload(key_pair: &KeyPair, payload: &[u8]) -> Result<Signature> {
    Signature::try_new(key_pair.private_key(), payload)
        .wrap_err("failed to sign Soracloud CLI payload")
}
fn signed_manifest_provenance(key_pair: &KeyPair, payload: &[u8]) -> Result<ManifestProvenance> {
    Ok(ManifestProvenance {
        signer: key_pair.public_key().clone(),
        signature: sign_soracloud_payload(key_pair, payload)?,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SorafsReleaseIdentityV1 {
    retention_epoch: NonZeroU64,
}
impl SorafsReleaseIdentityV1 {
    const fn new(retention_epoch: NonZeroU64) -> Self {
        Self { retention_epoch }
    }
    const fn retention_epoch(self) -> u64 {
        self.retention_epoch.get()
    }
}
fn attach_sorafs_release_governance(
    mut manifest: ManifestV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<ManifestV1> {
    manifest.metadata.push(MetadataEntry {
        key: SORAFS_RELEASE_RETENTION_EPOCH_METADATA_KEY_V1.to_owned(),
        value: release_identity.retention_epoch().to_string(),
    });
    let (_unsigned_manifest_bytes, unsigned_manifest_digest) =
        encode_sorafs_manifest_for_storage(&manifest)?;
    let (_, signer_bytes) = key_pair
        .public_key()
        .try_to_bytes()
        .wrap_err("SoraFS release governance signer public key is malformed")?;
    let signer: [u8; 32] = signer_bytes.try_into().map_err(|_| {
        eyre!(
            "SoraFS release governance proof requires a 32-byte Ed25519 public key, got {} bytes",
            signer_bytes.len()
        )
    })?;
    let signature = sign_soracloud_payload(key_pair, unsigned_manifest_digest.as_bytes())?;
    manifest.governance = GovernanceProofs {
        council_signatures: vec![CouncilSignature {
            signer,
            signature: signature.payload().to_vec(),
        }],
    };
    Ok(manifest)
}
fn validate_sorafs_release_identity(
    manifest: &ManifestV1,
    release_identity: SorafsReleaseIdentityV1,
    description: &str,
) -> Result<()> {
    if manifest.pin_policy.retention_epoch != release_identity.retention_epoch() {
        return Err(eyre!(
            "{description} retention epoch {} differs from the release identity {}",
            manifest.pin_policy.retention_epoch,
            release_identity.retention_epoch()
        ));
    }
    let expected = release_identity.retention_epoch().to_string();
    let [identity] = manifest.metadata.as_slice() else {
        return Err(eyre!(
            "{description} must carry only the deterministic `{SORAFS_RELEASE_RETENTION_EPOCH_METADATA_KEY_V1}` metadata entry"
        ));
    };
    if identity.key != SORAFS_RELEASE_RETENTION_EPOCH_METADATA_KEY_V1 || identity.value != expected
    {
        return Err(eyre!(
            "{description} must carry the deterministic `{SORAFS_RELEASE_RETENTION_EPOCH_METADATA_KEY_V1}` metadata value {expected}"
        ));
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExpectedSorafsPinManifest {
    digest: ManifestDigest,
    root_cid: ManifestRootCid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SorafsPinManifestReadiness {
    Missing,
    Pending,
    Approved(u64),
}

fn expected_sorafs_pin_manifest(
    args: iroha::client::SorafsPinRegisterArgs<'_>,
    manifest_digest_hex: &str,
) -> Result<ExpectedSorafsPinManifest> {
    let manifest = sorafs_manifest::decode_manifest_v1_canonical(args.manifest_payload)
        .wrap_err("failed to decode the exact SoraFS manifest before pin registration")?;
    let digest = ManifestDigest::from_manifest(&manifest)
        .wrap_err("failed to derive the exact SoraFS manifest digest before pin registration")?;
    let canonical_digest_hex = hex::encode(digest.as_bytes());
    if manifest_digest_hex != canonical_digest_hex {
        return Err(eyre!(
            "SoraFS pin registration digest `{manifest_digest_hex}` does not match the exact manifest digest `{canonical_digest_hex}`"
        ));
    }
    let root_cid = ManifestRootCid::try_from_slice(&manifest.root_cid)
        .wrap_err("failed to decode the exact SoraFS manifest root CID before pin registration")?;
    Ok(ExpectedSorafsPinManifest { digest, root_cid })
}

fn decode_sorafs_pin_manifest_readiness(
    response_body: &[u8],
    expected: ExpectedSorafsPinManifest,
) -> Result<SorafsPinManifestReadiness> {
    let finalized: PinManifestFinalizedRecordV1 = json::from_slice(response_body)
        .wrap_err("failed to decode the finalized SoraFS pin registry record")?;
    if finalized.manifest.digest != expected.digest {
        return Err(eyre!(
            "finalized SoraFS pin registry record returned digest {}, expected {}",
            hex::encode(finalized.manifest.digest.as_bytes()),
            hex::encode(expected.digest.as_bytes())
        ));
    }
    if finalized.manifest.root_cid != expected.root_cid {
        return Err(eyre!(
            "finalized SoraFS pin registry record returned a root CID that does not match manifest {}",
            hex::encode(expected.digest.as_bytes())
        ));
    }
    match finalized.manifest.status {
        PinStatus::Pending => {
            if finalized.manifest.approved_epoch.is_some() {
                return Err(eyre!(
                    "pending SoraFS pin registry record {} carries an approval epoch",
                    hex::encode(expected.digest.as_bytes())
                ));
            }
            Ok(SorafsPinManifestReadiness::Pending)
        }
        PinStatus::Approved(approved_epoch) => {
            if finalized.manifest.approved_epoch != Some(approved_epoch) {
                return Err(eyre!(
                    "approved SoraFS pin registry record {} does not bind status epoch {approved_epoch} to approved_epoch",
                    hex::encode(expected.digest.as_bytes())
                ));
            }
            Ok(SorafsPinManifestReadiness::Approved(approved_epoch))
        }
        PinStatus::Retired(retired_epoch) => Err(eyre!(
            "SoraFS pin registry record {} retired at epoch {retired_epoch} and is not replication-eligible",
            hex::encode(expected.digest.as_bytes())
        )),
    }
}

fn sorafs_pin_manifest_readiness(
    client: &Client,
    expected: ExpectedSorafsPinManifest,
) -> Result<SorafsPinManifestReadiness> {
    let manifest_digest_hex = hex::encode(expected.digest.as_bytes());
    let response = client
        .client()
        .get_sorafs_pin_manifest(&manifest_digest_hex)
        .wrap_err_with(|| {
            format!("failed to query SoraFS pin registry for {manifest_digest_hex}")
        })?;
    match response.status() {
        iroha::http::StatusCode::OK => {
            decode_sorafs_pin_manifest_readiness(response.body(), expected)
        }
        iroha::http::StatusCode::NOT_FOUND => Ok(SorafsPinManifestReadiness::Missing),
        status => Err(eyre!(
            "failed to query SoraFS pin registry for {manifest_digest_hex}: {} {}",
            status,
            std::str::from_utf8(response.body()).unwrap_or("")
        )),
    }
}

/// Revalidate a retained Taira stage and read exact finalized pin-governance readiness.
///
/// A globally applied registration transaction is deliberately insufficient: only a
/// finalized record whose digest, root CID, status epoch, and `approved_epoch` all match
/// the retained stage is returned as [`TairaInrouCanaryPinReadinessV1::Approved`].
pub(crate) fn taira_inrou_canary_pin_readiness_v1(
    config: &ClientConfig,
    stage_dir: &Path,
    torii_url: &str,
    request_timeout: Duration,
    requested_mode: crate::taira::InrouCanaryMode,
    operation: TairaInrouCanaryPreparedOperationV1,
) -> Result<TairaInrouCanaryPinReadinessV1> {
    let staged = load_verified_taira_inrou_stage(
        stage_dir,
        &config.key_pair,
        MutationMode::from(requested_mode),
    )?;
    let built = match operation {
        TairaInrouCanaryPreparedOperationV1::BundlePin => &staged.bundle_manifest,
        TairaInrouCanaryPreparedOperationV1::GuestPin => &staged.guest_manifest,
        TairaInrouCanaryPreparedOperationV1::DiscoveryPin => &staged.discovery_manifest,
        TairaInrouCanaryPreparedOperationV1::ServiceMutation => {
            return Err(eyre!(
                "Taira Inrou service mutation has no SoraFS pin-governance readiness"
            ));
        }
    };
    let expected = expected_sorafs_pin_manifest(
        iroha::client::SorafsPinRegisterArgs {
            manifest_payload: &built.bytes,
            alias: None,
            successor_of: None,
        },
        &built.digest_hex,
    )?;
    if request_timeout.is_zero() {
        return Ok(TairaInrouCanaryPinReadinessV1::Missing);
    }
    let mut bounded_config = config.clone();
    bounded_config.torii_api_url = url::Url::parse(torii_url)?;
    bounded_config.torii_request_timeout = request_timeout;
    let client = Client::new(bounded_config)?;
    Ok(match sorafs_pin_manifest_readiness(&client, expected)? {
        SorafsPinManifestReadiness::Missing => TairaInrouCanaryPinReadinessV1::Missing,
        SorafsPinManifestReadiness::Pending => TairaInrouCanaryPinReadinessV1::Pending,
        SorafsPinManifestReadiness::Approved(epoch) => {
            TairaInrouCanaryPinReadinessV1::Approved(epoch)
        }
    })
}

fn wait_for_sorafs_pin_manifest(
    client: &Client,
    expected: ExpectedSorafsPinManifest,
    description: &str,
    timeout_secs: u64,
) -> Result<()> {
    let manifest_digest_hex = hex::encode(expected.digest.as_bytes());
    let timeout = Duration::from_secs(timeout_secs.max(1));
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(
            sorafs_pin_manifest_readiness(client, expected)?,
            SorafsPinManifestReadiness::Approved(_)
        ) {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(eyre!(
                "timed out waiting for {description} pin registry record {manifest_digest_hex} to reach Approved"
            ));
        }
        std::thread::sleep((deadline - now).min(Duration::from_secs(2)));
    }
}
fn register_sorafs_pin_manifest_and_wait(
    client: &Client,
    args: iroha::client::SorafsPinRegisterArgs<'_>,
    manifest_digest_hex: &str,
    description: &str,
    timeout_secs: u64,
) -> Result<()> {
    let expected = expected_sorafs_pin_manifest(args, manifest_digest_hex)?;
    match sorafs_pin_manifest_readiness(client, expected)? {
        SorafsPinManifestReadiness::Approved(_) => return Ok(()),
        SorafsPinManifestReadiness::Pending => {}
        SorafsPinManifestReadiness::Missing => {
            client
                .post_sorafs_pin_register(args)
                .wrap_err_with(|| format!("failed to register {description} manifest"))?;
        }
    }
    wait_for_sorafs_pin_manifest(client, expected, description, timeout_secs)
}
struct SorafsManifestBuildLabels<'a> {
    writer: &'a str,
    metadata: &'a str,
    root: &'a str,
    por: &'a str,
    manifest: &'a str,
    governance: &'a str,
    encoding: &'a str,
    digest: &'a str,
}
struct BuiltSorafsManifest {
    manifest: ManifestV1,
    bytes: Vec<u8>,
    digest_hex: String,
}

struct PreparedSorafsArtifact {
    description: String,
    plan: CarBuildPlan,
    payload: Vec<u8>,
    built: BuiltSorafsManifest,
}

const INROU_PRESEED_RECEIPT_MAX_BYTES: u64 = 1024 * 1024;
const INROU_PRESEED_STDERR_MAX_BYTES: u64 = 1024 * 1024;
const INROU_PRESEED_HELPER_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const INROU_PRESEED_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);

fn parse_canonical_inrou_validator_account(value: &str) -> std::result::Result<AccountId, String> {
    if value.trim() != value {
        return Err("validator account must not contain surrounding whitespace".to_owned());
    }
    let account = AccountId::parse_encoded(value)
        .map_err(|error| format!("invalid validator account: {error}"))?;
    if account.to_string() != value {
        return Err("validator account must use its exact canonical V1 spelling".to_owned());
    }
    Ok(account)
}

/// One CLI-supplied validator/peer/store binding for offline Inrou preseed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InrouOperatorPreseedTargetArg {
    validator_account_id: String,
    peer_id: String,
    data_dir: PathBuf,
}

impl InrouOperatorPreseedTargetArg {
    fn canonical_placement(&self) -> std::result::Result<SoraInrouPlacementTargetV1, String> {
        let placement = SoraInrouPlacementTargetV1 {
            validator_account_id: parse_canonical_inrou_validator_account(
                &self.validator_account_id,
            )?,
            peer_id: self.peer_id.clone(),
        };
        placement
            .validate()
            .map_err(|error| format!("invalid validator/peer binding: {error}"))?;
        Ok(placement)
    }
}

impl FromStr for InrouOperatorPreseedTargetArg {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let mut fields = value.splitn(3, ',');
        let validator_account_id = fields
            .next()
            .filter(|field| !field.is_empty())
            .ok_or_else(|| {
                "expected <validator-account-id>,<peer-id>,<absolute-data-dir>".to_owned()
            })?
            .to_owned();
        let peer_id = fields
            .next()
            .filter(|field| !field.is_empty())
            .ok_or_else(|| {
                "expected <validator-account-id>,<peer-id>,<absolute-data-dir>".to_owned()
            })?
            .to_owned();
        let data_dir = fields
            .next()
            .filter(|field| !field.is_empty())
            .ok_or_else(|| {
                "expected <validator-account-id>,<peer-id>,<absolute-data-dir>".to_owned()
            })?;
        Ok(Self {
            validator_account_id,
            peer_id,
            data_dir: PathBuf::from(data_dir),
        })
    }
}

pub(crate) fn parse_inrou_placement_target_identity(
    value: &str,
) -> std::result::Result<SoraInrouPlacementTargetV1, String> {
    let mut fields = value.split(',');
    let validator_account_id = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "expected <validator-account-id>,<peer-id>".to_owned())
        .and_then(parse_canonical_inrou_validator_account)?;
    let peer_id = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "expected <validator-account-id>,<peer-id>".to_owned())?
        .to_owned();
    if fields.next().is_some() {
        return Err("expected exactly <validator-account-id>,<peer-id>".to_owned());
    }
    let target = SoraInrouPlacementTargetV1 {
        validator_account_id,
        peer_id,
    };
    target
        .validate()
        .map_err(|error| format!("invalid validator/peer binding: {error}"))?;
    Ok(target)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValidatedInrouOperatorPreseedTarget {
    placement: SoraInrouPlacementTargetV1,
    store_root: PathBuf,
}

struct ValidatedInrouOperatorPreseed {
    targets: Vec<ValidatedInrouOperatorPreseedTarget>,
    max_capacity_bytes: NonZeroU64,
    helper_path: PathBuf,
    helper_sha256: String,
}

impl ValidatedInrouOperatorPreseed {
    fn placement_targets(&self) -> BTreeSet<SoraInrouPlacementTargetV1> {
        self.targets
            .iter()
            .map(|target| target.placement.clone())
            .collect()
    }
}

fn bind_exact_inrou_placement_targets(
    service: &mut SoraServiceManifestV1,
    qualified_targets: &BTreeSet<SoraInrouPlacementTargetV1>,
) -> Result<()> {
    if service.placement_targets.is_empty() {
        service.placement_targets = qualified_targets.clone();
        return Ok(());
    }
    if &service.placement_targets != qualified_targets {
        return Err(eyre!(
            "source service placement_targets differ from the exact durable Inrou qualification"
        ));
    }
    Ok(())
}

/// Offline helper process holding every target store lock until qualification is durable.
///
/// The originating CLI timeout independently bounds readiness, release, and cleanup phases.
struct InrouOperatorPreseedSession {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    stderr_bytes: Vec<u8>,
    stdout_eof: bool,
    stderr_eof: bool,
    timeout: Duration,
    receipt: OperatorPreseedSessionReceiptV1,
    _stage: SoracloudTempDir,
}

fn inrou_preseed_phase_deadline(timeout: Duration, phase: &str) -> Result<Instant> {
    if timeout.is_zero() {
        return Err(eyre!(
            "Inrou operator-preseed {phase} timeout must be positive"
        ));
    }
    Instant::now().checked_add(timeout).ok_or_else(|| {
        eyre!("Inrou operator-preseed {phase} timeout exceeds the monotonic clock domain")
    })
}

#[cfg(unix)]
fn make_inrou_preseed_pipe_nonblocking(pipe: &impl std::os::fd::AsFd, label: &str) -> Result<()> {
    let descriptor = pipe.as_fd();
    let flags = rustix::fs::fcntl_getfl(descriptor)
        .wrap_err_with(|| format!("failed to inspect Inrou operator-preseed {label}"))?;
    rustix::fs::fcntl_setfl(descriptor, flags | rustix::fs::OFlags::NONBLOCK)
        .wrap_err_with(|| format!("failed to make Inrou operator-preseed {label} nonblocking"))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_inrou_preseed_pipe_nonblocking<T>(_pipe: &T, _label: &str) -> Result<()> {
    Err(eyre!(
        "offline Inrou operator-preseed sessions require Unix nonblocking pipes"
    ))
}

fn drain_inrou_preseed_pipe(
    pipe: &mut impl io::Read,
    output: &mut Vec<u8>,
    eof: &mut bool,
    label: &str,
    max_bytes: u64,
) -> Result<()> {
    if *eof {
        return Ok(());
    }
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => {
                *eof = true;
                return Ok(());
            }
            Ok(count) => {
                let next_len = output
                    .len()
                    .checked_add(count)
                    .ok_or_else(|| eyre!("Inrou operator-preseed {label} length overflow"))?;
                if u64::try_from(next_len).unwrap_or(u64::MAX) > max_bytes {
                    return Err(eyre!(
                        "Inrou operator-preseed {label} exceeded its V1 byte limit"
                    ));
                }
                output.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(error)
                    .wrap_err_with(|| format!("failed to read Inrou operator-preseed {label}"));
            }
        }
    }
}

fn drain_inrou_preseed_stdout(
    stdout: &mut ChildStdout,
    output: &mut Vec<u8>,
    eof: &mut bool,
) -> Result<()> {
    drain_inrou_preseed_pipe(
        stdout,
        output,
        eof,
        "stdout",
        INROU_PRESEED_RECEIPT_MAX_BYTES,
    )
}

fn drain_inrou_preseed_stderr(
    stderr: &mut ChildStderr,
    output: &mut Vec<u8>,
    eof: &mut bool,
) -> Result<()> {
    drain_inrou_preseed_pipe(
        stderr,
        output,
        eof,
        "stderr",
        INROU_PRESEED_STDERR_MAX_BYTES,
    )
}

fn wait_for_inrou_preseed_child_exit(child: &mut Child, deadline: Instant) -> Result<()> {
    loop {
        match child
            .try_wait()
            .wrap_err("failed to poll Inrou operator-preseed helper during cleanup")?
        {
            Some(_) => return Ok(()),
            None if Instant::now() >= deadline => {
                return Err(eyre!(
                    "Inrou operator-preseed helper did not exit before its cleanup deadline"
                ));
            }
            None => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(INROU_PRESEED_PROCESS_POLL_INTERVAL.min(remaining));
            }
        }
    }
}

#[cfg(unix)]
fn signal_inrou_preseed_process_group(child: &mut Child) -> Result<()> {
    let process_group = rustix::process::Pid::from_child(child);
    match rustix::process::kill_process_group(process_group, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(error) => {
            let _ = child.kill();
            Err(error).wrap_err("failed to terminate owned Inrou operator-preseed process group")
        }
    }
}

#[cfg(not(unix))]
fn signal_inrou_preseed_process_group(child: &mut Child) -> Result<()> {
    child
        .kill()
        .wrap_err("failed to terminate owned Inrou operator-preseed helper")
}

fn terminate_inrou_preseed_child(child: &mut Child, timeout: Duration, phase: &str) -> Result<()> {
    let signal_error = signal_inrou_preseed_process_group(child).err();
    let wait_result = inrou_preseed_phase_deadline(timeout, phase)
        .and_then(|deadline| wait_for_inrou_preseed_child_exit(child, deadline));
    match (signal_error, wait_result) {
        (None, Ok(())) => Ok(()),
        (Some(error), Ok(())) => Err(error),
        (None, Err(error)) => Err(error),
        (Some(signal_error), Err(wait_error)) => Err(eyre!(
            "{wait_error:#}; process-group termination also failed: {signal_error:#}"
        )),
    }
}

fn inrou_preseed_error_after_cleanup(
    mut child: Child,
    timeout: Duration,
    phase: &str,
    error: Report,
) -> Report {
    match terminate_inrou_preseed_child(&mut child, timeout, phase) {
        Ok(()) => error,
        Err(cleanup_error) => {
            eyre!("{error:#}; Inrou operator-preseed cleanup also failed: {cleanup_error:#}")
        }
    }
}

impl InrouOperatorPreseedSession {
    #[cfg(test)]
    fn ensure_alive(&mut self) -> Result<()> {
        let mut trailing = Vec::new();
        drain_inrou_preseed_stdout(
            self.stdout
                .as_mut()
                .expect("live preseed session owns its stdout"),
            &mut trailing,
            &mut self.stdout_eof,
        )?;
        drain_inrou_preseed_stderr(
            self.stderr
                .as_mut()
                .expect("live preseed session owns its stderr"),
            &mut self.stderr_bytes,
            &mut self.stderr_eof,
        )?;
        if !trailing.is_empty() {
            return Err(eyre!(
                "offline Inrou operator-preseed helper emitted trailing stdout before lock release"
            ));
        }
        if let Some(status) = self
            .child
            .as_mut()
            .expect("live preseed session owns its child")
            .try_wait()
            .wrap_err("failed to query offline Inrou operator-preseed helper")?
        {
            return Err(eyre!(
                "offline Inrou operator-preseed helper exited with {status} before lock release was authorized"
            ));
        }
        Ok(())
    }

    fn finish(mut self) -> Result<OperatorPreseedSessionReceiptV1> {
        let deadline = inrou_preseed_phase_deadline(self.timeout, "release")?;
        drop(self.stdin.take());
        let mut trailing = Vec::new();
        let mut status = None;
        let result = (|| -> Result<()> {
            loop {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "Inrou operator-preseed helper exceeded its release deadline"
                    ));
                }
                drain_inrou_preseed_stdout(
                    self.stdout
                        .as_mut()
                        .expect("live preseed session owns its stdout"),
                    &mut trailing,
                    &mut self.stdout_eof,
                )?;
                drain_inrou_preseed_stderr(
                    self.stderr
                        .as_mut()
                        .expect("live preseed session owns its stderr"),
                    &mut self.stderr_bytes,
                    &mut self.stderr_eof,
                )?;
                if !OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1.starts_with(&trailing) {
                    return Err(eyre!(
                        "offline Inrou operator-preseed helper emitted a noncanonical release acknowledgment"
                    ));
                }
                if status.is_none() {
                    status = self
                        .child
                        .as_mut()
                        .expect("live preseed session owns its child")
                        .try_wait()
                        .wrap_err("failed to poll offline Inrou operator-preseed helper")?;
                }
                if status.is_some() && self.stdout_eof && self.stderr_eof {
                    break;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                std::thread::sleep(INROU_PRESEED_PROCESS_POLL_INTERVAL.min(remaining));
            }
            let status = status.expect("preseed release completes only after child exit");
            if !status.success() {
                return Err(eyre!(
                    "offline Inrou operator-preseed helper exited with {status} after lock release"
                ));
            }
            if trailing != OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1 {
                return Err(eyre!(
                    "offline Inrou operator-preseed helper did not emit the exact EOF release acknowledgment"
                ));
            }
            Ok(())
        })();
        drop(self.stdout.take());
        drop(self.stderr.take());
        let child = self
            .child
            .take()
            .expect("live preseed session owns its child");
        match result {
            Ok(()) => Ok(self.receipt.clone()),
            Err(error) => Err(inrou_preseed_error_after_cleanup(
                child,
                self.timeout,
                "release cleanup",
                error,
            )),
        }
    }
}

impl Drop for InrouOperatorPreseedSession {
    fn drop(&mut self) {
        drop(self.stdin.take());
        drop(self.stdout.take());
        drop(self.stderr.take());
        if let Some(mut child) = self.child.take()
            && let Err(error) =
                terminate_inrou_preseed_child(&mut child, self.timeout, "drop cleanup")
        {
            let _ = writeln!(
                io::stderr().lock(),
                "failed to clean up Inrou operator-preseed helper: {error:#}"
            );
        }
    }
}

fn required_inrou_preseed_store_count<B>(bundles: impl IntoIterator<Item = B>) -> Option<usize>
where
    B: std::borrow::Borrow<UnpublishedDeploymentBundleV1>,
{
    bundles
        .into_iter()
        .filter(|bundle| {
            let bundle = <B as std::borrow::Borrow<UnpublishedDeploymentBundleV1>>::borrow(bundle);
            bundle.service.execution_plane == SoraServiceExecutionPlaneV1::HttpService
                && bundle.container.runtime == SoraContainerRuntimeV1::Inrou
        })
        .map(|bundle| {
            let bundle = <B as std::borrow::Borrow<UnpublishedDeploymentBundleV1>>::borrow(&bundle);
            usize::from(bundle.service.replicas.get())
        })
        .max()
        .map(|replicas: usize| replicas.max(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1))
}

fn validate_inrou_preseed_artifact_count<B>(bundles: impl IntoIterator<Item = B>) -> Result<()>
where
    B: std::borrow::Borrow<UnpublishedDeploymentBundleV1>,
{
    let mut artifact_count = 0_usize;
    for bundle in bundles {
        let bundle = <B as std::borrow::Borrow<UnpublishedDeploymentBundleV1>>::borrow(&bundle);
        if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
            || bundle.container.runtime != SoraContainerRuntimeV1::Inrou
        {
            continue;
        }
        let guest_image_count = bundle
            .container
            .inrou
            .as_ref()
            .ok_or_else(|| {
                eyre!(
                    "service `{}` selects Inrou without its mandatory guest-image manifest",
                    bundle.service.service_name
                )
            })?
            .guest_images
            .len();
        let public_discovery_count = usize::from(
            bundle
                .service
                .route
                .as_ref()
                .is_some_and(|route| route.visibility == SoraRouteVisibilityV1::Public),
        );
        artifact_count = artifact_count
            .checked_add(1)
            .and_then(|count| count.checked_add(guest_image_count))
            .and_then(|count| count.checked_add(public_discovery_count))
            .ok_or_else(|| eyre!("Inrou operator-preseed artifact count overflow"))?;
        if artifact_count > OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1 {
            return Err(eyre!(
                "HttpService + Inrou publication requires {artifact_count} artifacts, exceeding the V1 operator-preseed session limit of {OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1}"
            ));
        }
    }
    Ok(())
}

fn distinct_prepared_sorafs_artifacts<'a>(
    artifacts: impl IntoIterator<Item = &'a PreparedSorafsArtifact>,
) -> Vec<&'a PreparedSorafsArtifact> {
    let mut manifest_digests = BTreeSet::new();
    let mut artifacts = artifacts
        .into_iter()
        .filter(|artifact| manifest_digests.insert(artifact.built.digest_hex.clone()))
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| left.built.digest_hex.cmp(&right.built.digest_hex));
    artifacts
}

fn canonical_inrou_preseed_target(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(eyre!(
            "--inrou-preseed-target data-dir must be an absolute path, got `{}`",
            path.display()
        ));
    }
    let metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to inspect --inrou-preseed-target data-dir `{}`",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(eyre!(
            "--inrou-preseed-target data-dir `{}` must be one existing real directory",
            path.display()
        ));
    }
    fs::canonicalize(path).wrap_err_with(|| {
        format!(
            "failed to canonicalize --inrou-preseed-target data-dir `{}`",
            path.display()
        )
    })
}

fn sha256_file(path: &Path, label: &str, max_bytes: u64) -> Result<String> {
    use sha2::{Digest as _, Sha256};

    let metadata = fs::metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(eyre!(
            "{label} `{}` must be a nonempty regular file no larger than {max_bytes} bytes",
            path.display()
        ));
    }
    let mut file = fs::File::open(path)
        .wrap_err_with(|| format!("failed to open {label} `{}`", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .wrap_err_with(|| format!("failed to hash {label} `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn canonical_inrou_preseed_helper(path: &Path, expected_sha256: &str) -> Result<(PathBuf, String)> {
    if !path.is_absolute() {
        return Err(eyre!(
            "--inrou-preseed-helper must be an absolute path, got `{}`",
            path.display()
        ));
    }
    let metadata = fs::symlink_metadata(path).wrap_err_with(|| {
        format!(
            "failed to inspect --inrou-preseed-helper `{}`",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(eyre!(
            "--inrou-preseed-helper `{}` must be one existing real file",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(eyre!(
                "--inrou-preseed-helper `{}` must be executable",
                path.display()
            ));
        }
    }
    let canonical = fs::canonicalize(path).wrap_err_with(|| {
        format!(
            "failed to canonicalize --inrou-preseed-helper `{}`",
            path.display()
        )
    })?;
    let decoded = hex::decode(expected_sha256).map_err(|_| {
        eyre!("--inrou-preseed-helper-sha256 must be exactly 32 lowercase hex bytes")
    })?;
    if decoded.len() != 32 || hex::encode(decoded) != expected_sha256 {
        return Err(eyre!(
            "--inrou-preseed-helper-sha256 must be exactly 32 lowercase hex bytes"
        ));
    }
    let actual = sha256_file(
        &canonical,
        "offline Inrou operator-preseed helper",
        INROU_PRESEED_HELPER_MAX_BYTES,
    )?;
    if actual != expected_sha256 {
        return Err(eyre!(
            "offline Inrou operator-preseed helper SHA-256 mismatch: expected {expected_sha256}, got {actual}"
        ));
    }
    Ok((canonical, actual))
}

fn validate_inrou_operator_preseed(
    required_count: Option<usize>,
    target_args: &[InrouOperatorPreseedTargetArg],
    max_capacity_bytes: Option<NonZeroU64>,
    helper_path: Option<&Path>,
    helper_sha256: Option<&str>,
) -> Result<Option<ValidatedInrouOperatorPreseed>> {
    let Some(required_count) = required_count else {
        if !target_args.is_empty()
            || max_capacity_bytes.is_some()
            || helper_path.is_some()
            || helper_sha256.is_some()
        {
            return Err(eyre!(
                "Inrou operator-preseed arguments are valid only for an HttpService + Inrou mutation"
            ));
        }
        return Ok(None);
    };
    let max_capacity_bytes = max_capacity_bytes.ok_or_else(|| {
        eyre!(
            "HttpService + Inrou publication requires --inrou-preseed-max-capacity-bytes matching every selected validator store"
        )
    })?;
    if target_args.len() < required_count {
        return Err(eyre!(
            "HttpService + Inrou publication requires at least {required_count} explicit --inrou-preseed-target validator/peer/store bindings for its replica/pin policy; found {}",
            target_args.len()
        ));
    }
    if target_args.len() > OPERATOR_PRESEED_SESSION_MAX_STORES_V1 {
        return Err(eyre!(
            "HttpService + Inrou publication admits at most {OPERATOR_PRESEED_SESSION_MAX_STORES_V1} explicit --inrou-preseed-target bindings in V1; found {}",
            target_args.len()
        ));
    }
    let mut canonical_targets = Vec::with_capacity(target_args.len());
    let mut distinct_roots = BTreeSet::new();
    let mut distinct_validators = BTreeSet::new();
    let mut distinct_peers = BTreeSet::new();
    for target in target_args {
        let placement = target
            .canonical_placement()
            .map_err(|error| eyre!(error))
            .wrap_err("invalid --inrou-preseed-target validator/peer binding")?;
        if !distinct_validators.insert(placement.validator_account_id.clone())
            || !distinct_peers.insert(placement.peer_id.clone())
        {
            return Err(eyre!(
                "--inrou-preseed-target validator and peer identities must each be distinct"
            ));
        }
        let canonical = canonical_inrou_preseed_target(&target.data_dir)?;
        if !distinct_roots.insert(canonical.clone()) {
            return Err(eyre!(
                "--inrou-preseed-target values must resolve to distinct storage roots; `{}` is repeated",
                canonical.display()
            ));
        }
        if let Some(overlap) =
            canonical_targets
                .iter()
                .find(|existing: &&ValidatedInrouOperatorPreseedTarget| {
                    canonical.starts_with(existing.store_root.as_path())
                        || existing.store_root.starts_with(&canonical)
                })
        {
            return Err(eyre!(
                "--inrou-preseed-target storage roots must not overlap; `{}` and `{}` have an ancestor/descendant relationship",
                overlap.store_root.display(),
                canonical.display()
            ));
        }
        canonical_targets.push(ValidatedInrouOperatorPreseedTarget {
            placement,
            store_root: canonical,
        });
    }
    canonical_targets.sort_by(|left, right| {
        (
            left.placement.validator_account_id.to_string(),
            left.placement.peer_id.as_str(),
            left.store_root.as_path(),
        )
            .cmp(&(
                right.placement.validator_account_id.to_string(),
                right.placement.peer_id.as_str(),
                right.store_root.as_path(),
            ))
    });
    let helper_path = helper_path
        .ok_or_else(|| eyre!("HttpService + Inrou publication requires --inrou-preseed-helper"))?;
    let helper_sha256 = helper_sha256.ok_or_else(|| {
        eyre!("HttpService + Inrou publication requires --inrou-preseed-helper-sha256")
    })?;
    let (helper_path, helper_sha256) = canonical_inrou_preseed_helper(helper_path, helper_sha256)?;
    Ok(Some(ValidatedInrouOperatorPreseed {
        targets: canonical_targets,
        max_capacity_bytes,
        helper_path,
        helper_sha256,
    }))
}

enum StagedInrouPreseedPayload {
    File(PathBuf),
    Directory(PathBuf),
}

fn stage_inrou_preseed_artifact(
    stage_root: &Path,
    index: usize,
    artifact: &PreparedSorafsArtifact,
) -> Result<(PathBuf, StagedInrouPreseedPayload)> {
    if artifact.plan.content_length
        != u64::try_from(artifact.payload.len())
            .wrap_err("Inrou preseed payload length exceeds u64")?
    {
        return Err(eyre!(
            "{} plan length does not match its exact prepared payload",
            artifact.description
        ));
    }
    let manifest_path = stage_root.join(format!("artifact-{index}.manifest.norito"));
    fs::write(&manifest_path, &artifact.built.bytes).wrap_err_with(|| {
        format!(
            "failed to stage {} manifest at `{}`",
            artifact.description,
            manifest_path.display()
        )
    })?;
    if artifact.plan.files.len() == 1 && artifact.plan.files[0].path.is_empty() {
        let payload_path = stage_root.join(format!("artifact-{index}.payload"));
        fs::write(&payload_path, &artifact.payload).wrap_err_with(|| {
            format!(
                "failed to stage exact {} payload at `{}`",
                artifact.description,
                payload_path.display()
            )
        })?;
        return Ok((manifest_path, StagedInrouPreseedPayload::File(payload_path)));
    }
    let payload_dir = stage_root.join(format!("artifact-{index}.payload-dir"));
    fs::create_dir(&payload_dir).wrap_err_with(|| {
        format!(
            "failed to create exact {} payload directory `{}`",
            artifact.description,
            payload_dir.display()
        )
    })?;
    let mut offset = 0_usize;
    for file in &artifact.plan.files {
        if file.path.is_empty() {
            return Err(eyre!(
                "{} mixes an empty logical file path into a directory artifact",
                artifact.description
            ));
        }
        let size = usize::try_from(file.size)
            .wrap_err_with(|| format!("convert {} file size", artifact.description))?;
        let end = offset
            .checked_add(size)
            .ok_or_else(|| eyre!("{} staged file range overflow", artifact.description))?;
        let bytes = artifact.payload.get(offset..end).ok_or_else(|| {
            eyre!(
                "{} staged logical file exceeds its exact payload",
                artifact.description
            )
        })?;
        let mut path = payload_dir.clone();
        for component in &file.path {
            path.push(component);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!(
                    "failed to create exact {} logical directory `{}`",
                    artifact.description,
                    parent.display()
                )
            })?;
        }
        fs::write(&path, bytes).wrap_err_with(|| {
            format!(
                "failed to stage exact {} logical file `{}`",
                artifact.description,
                path.display()
            )
        })?;
        offset = end;
    }
    if offset != artifact.payload.len() {
        return Err(eyre!(
            "{} logical files do not cover its exact prepared payload",
            artifact.description
        ));
    }
    Ok((
        manifest_path,
        StagedInrouPreseedPayload::Directory(payload_dir),
    ))
}

fn expected_inrou_preseed_receipt(
    config: &ValidatedInrouOperatorPreseed,
    artifacts: &[&PreparedSorafsArtifact],
) -> Result<OperatorPreseedSessionReceiptV1> {
    expected_inrou_preseed_receipt_from_parts(&config.targets, config.max_capacity_bytes, artifacts)
}

fn expected_inrou_preseed_receipt_from_parts(
    targets: &[ValidatedInrouOperatorPreseedTarget],
    max_capacity_bytes: NonZeroU64,
    artifacts: &[&PreparedSorafsArtifact],
) -> Result<OperatorPreseedSessionReceiptV1> {
    let store_count =
        u32::try_from(targets.len()).wrap_err("Inrou preseed store count exceeds u32")?;
    let mut artifacts = artifacts
        .iter()
        .map(|artifact| OperatorPreseedArtifactReceiptV1 {
            manifest_digest_blake3: artifact.built.digest_hex.clone(),
            payload_digest_blake3: hex::encode(artifact.plan.payload_digest.as_bytes()),
            content_length: artifact.plan.content_length,
            store_count,
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.manifest_digest_blake3
            .cmp(&right.manifest_digest_blake3)
    });
    let targets = targets
        .iter()
        .map(|target| {
            target
                .store_root
                .to_str()
                .map(|store_root| OperatorPreseedTargetReceiptV1 {
                    validator_account_id: target.placement.validator_account_id.to_string(),
                    peer_id: target.placement.peer_id.clone(),
                    store_root: store_root.to_owned(),
                })
                .ok_or_else(|| eyre!("Inrou preseed store roots must be valid UTF-8"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(OperatorPreseedSessionReceiptV1 {
        schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
        status: "ready".to_owned(),
        mode: "ingest".to_owned(),
        max_capacity_bytes: max_capacity_bytes.get(),
        targets,
        artifacts,
    })
}

#[derive(Clone)]
struct LoadedInrouPreseedQualification {
    path: PathBuf,
    receipt: OperatorPreseedSessionReceiptV1,
    bytes: Vec<u8>,
    targets: Vec<ValidatedInrouOperatorPreseedTarget>,
    max_capacity_bytes: NonZeroU64,
}

impl LoadedInrouPreseedQualification {
    fn placement_targets(&self) -> BTreeSet<SoraInrouPlacementTargetV1> {
        self.targets
            .iter()
            .map(|target| target.placement.clone())
            .collect()
    }

    fn require_exact_artifacts(&self, artifacts: &[&PreparedSorafsArtifact]) -> Result<()> {
        let expected = expected_inrou_preseed_receipt_from_parts(
            &self.targets,
            self.max_capacity_bytes,
            artifacts,
        )?;
        if self.receipt != expected {
            return Err(eyre!(
                "durable Inrou preseed qualification does not bind the exact prepared targets, capacity, and artifacts"
            ));
        }
        Ok(())
    }

    fn revalidate(&self) -> Result<()> {
        let loaded = read_inrou_preseed_qualification_file(&self.path)?;
        if loaded != self.bytes {
            return Err(eyre!(
                "durable Inrou preseed qualification changed before online publication"
            ));
        }
        Ok(())
    }
}

fn require_active_inrou_qualification_targets(
    qualification: &LoadedInrouPreseedQualification,
    active_hosts: &[SoraInrouHostCapabilityRecordV1],
) -> Result<()> {
    let mut active_by_validator = BTreeMap::new();
    let mut previous_validator: Option<&AccountId> = None;
    for (index, capability) in active_hosts.iter().enumerate() {
        capability
            .validate()
            .map_err(|error| eyre!(error))
            .wrap_err_with(|| format!("invalid active Inrou host at index {index}"))?;
        if previous_validator.is_some_and(|previous| previous >= &capability.validator_account_id) {
            return Err(eyre!(
                "active Inrou host capabilities must be strictly ordered by validator account"
            ));
        }
        previous_validator = Some(&capability.validator_account_id);
        if active_by_validator
            .insert(
                capability.validator_account_id.clone(),
                capability.peer_id.as_str(),
            )
            .is_some()
        {
            return Err(eyre!(
                "active Inrou host capabilities repeat a validator account"
            ));
        }
    }
    for target in &qualification.targets {
        let active_peer = active_by_validator
            .get(&target.placement.validator_account_id)
            .ok_or_else(|| {
                eyre!(
                    "qualified Inrou target `{}` is not an active advertised validator host",
                    target.placement.validator_account_id
                )
            })?;
        if *active_peer != target.placement.peer_id {
            return Err(eyre!(
                "qualified Inrou target `{}` binds peer `{}`, but the active capability binds `{}`",
                target.placement.validator_account_id,
                target.placement.peer_id,
                active_peer
            ));
        }
    }
    Ok(())
}

fn read_inrou_preseed_qualification_file(path: &Path) -> Result<Vec<u8>> {
    if !path.is_absolute() {
        return Err(eyre!(
            "--inrou-preseed-receipt must be an absolute path, got `{}`",
            path.display()
        ));
    }
    validate_taira_stage_owned_entry(path, false, "Inrou preseed qualification")?;
    taira_stage_owned_file_bytes(
        path,
        "Inrou preseed qualification",
        INROU_PRESEED_RECEIPT_MAX_BYTES,
    )
}

fn load_inrou_preseed_qualification(
    required_count: Option<usize>,
    receipt_path: Option<&Path>,
) -> Result<Option<LoadedInrouPreseedQualification>> {
    let Some(required_count) = required_count else {
        if receipt_path.is_some() {
            return Err(eyre!(
                "--inrou-preseed-receipt is valid only for an HttpService + Inrou publication"
            ));
        }
        return Ok(None);
    };
    let path = receipt_path.ok_or_else(|| {
        eyre!(
            "HttpService + Inrou publication requires the exact durable --inrou-preseed-receipt produced by the separate offline preseed command"
        )
    })?;
    let path = fs::canonicalize(path).wrap_err_with(|| {
        format!(
            "failed to resolve --inrou-preseed-receipt `{}`",
            path.display()
        )
    })?;
    let bytes = read_inrou_preseed_qualification_file(&path)?;
    let receipt: OperatorPreseedSessionReceiptV1 = json::from_slice(&bytes)
        .wrap_err("failed to decode durable Inrou preseed qualification")?;
    receipt
        .validate()
        .map_err(|error| eyre!(error))
        .wrap_err("invalid durable Inrou preseed qualification")?;
    if receipt.mode != "ingest"
        || json::to_vec(&receipt).wrap_err("re-encode Inrou preseed qualification")? != bytes
    {
        return Err(eyre!(
            "durable Inrou preseed qualification must be canonical JSON from an ingest session"
        ));
    }
    if receipt.targets.len() < required_count {
        return Err(eyre!(
            "HttpService + Inrou publication requires at least {required_count} qualified targets; receipt contains {}",
            receipt.targets.len()
        ));
    }
    let max_capacity_bytes = NonZeroU64::new(receipt.max_capacity_bytes)
        .ok_or_else(|| eyre!("Inrou preseed qualification capacity must be nonzero"))?;
    let targets = receipt
        .targets
        .iter()
        .map(|target| {
            let placement = SoraInrouPlacementTargetV1 {
                validator_account_id: parse_canonical_inrou_validator_account(
                    &target.validator_account_id,
                )
                .map_err(|error| eyre!(error))?,
                peer_id: target.peer_id.clone(),
            };
            placement
                .validate()
                .map_err(|error| eyre!(error))
                .wrap_err("invalid placement target in Inrou preseed qualification")?;
            Ok(ValidatedInrouOperatorPreseedTarget {
                placement,
                // The online phase never opens or canonicalizes target stores. The exact path is
                // retained only to reconstruct and compare the offline qualification bytes.
                store_root: PathBuf::from(&target.store_root),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(LoadedInrouPreseedQualification {
        path,
        receipt,
        bytes,
        targets,
        max_capacity_bytes,
    }))
}

fn write_inrou_preseed_qualification_file(
    destination: &Path,
    receipt: &OperatorPreseedSessionReceiptV1,
) -> Result<(PathBuf, Vec<u8>)> {
    if !destination.is_absolute() {
        return Err(eyre!(
            "--receipt-out must be an absolute path, got `{}`",
            destination.display()
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("--receipt-out must have a parent directory"))?;
    validate_taira_stage_owned_entry(parent, true, "Inrou qualification output directory")?;
    let bytes = json::to_vec(receipt).wrap_err("encode canonical Inrou preseed qualification")?;
    let stage = parent.join(format!(
        ".inrou-preseed-receipt-{}.tmp",
        hex::encode(Hash::new(&bytes).as_ref())
    ));
    if stage.exists() {
        validate_taira_stage_owned_entry(&stage, false, "staged Inrou qualification")?;
        let staged = taira_stage_owned_file_bytes(
            &stage,
            "staged Inrou qualification",
            INROU_PRESEED_RECEIPT_MAX_BYTES,
        )?;
        if staged != bytes {
            return Err(eyre!(
                "stale Inrou qualification staging file {} has different bytes",
                stage.display()
            ));
        }
        if let Ok(existing) = read_inrou_preseed_qualification_file(destination) {
            if existing != bytes {
                return Err(eyre!(
                    "--receipt-out `{}` already exists with different bytes",
                    destination.display()
                ));
            }
            fs::remove_file(&stage).wrap_err("remove recovered Inrou qualification stage")?;
        } else {
            #[cfg(unix)]
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &stage,
                rustix::fs::CWD,
                destination,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .wrap_err("recover immutable Inrou qualification without replacement")?;
            #[cfg(not(unix))]
            return Err("Inrou qualification crash recovery requires Unix".into());
        }
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .wrap_err("synchronize recovered Inrou qualification output directory")?;
        let installed = read_inrou_preseed_qualification_file(destination)?;
        if installed != bytes {
            return Err(eyre!(
                "recovered Inrou preseed qualification differs from its canonical bytes"
            ));
        }
        return Ok((destination.to_path_buf(), bytes));
    }
    if let Ok(existing) = read_inrou_preseed_qualification_file(destination) {
        if existing == bytes {
            return Ok((destination.to_path_buf(), bytes));
        }
        return Err(eyre!(
            "--receipt-out `{}` already exists with different bytes",
            destination.display()
        ));
    }
    write_taira_stage_file(&stage, &bytes)?;
    #[cfg(unix)]
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        &stage,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        eyre!(
            "failed to install immutable Inrou qualification {}: {error}",
            destination.display()
        )
    })?;
    #[cfg(not(unix))]
    return Err("Inrou qualification installation requires Unix".into());
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .wrap_err("synchronize Inrou qualification output directory")?;
    let installed = read_inrou_preseed_qualification_file(destination)?;
    if installed != bytes {
        return Err(eyre!(
            "installed Inrou preseed qualification differs from its canonical bytes"
        ));
    }
    Ok((destination.to_path_buf(), bytes))
}

#[cfg(unix)]
fn stage_verified_inrou_preseed_helper(
    config: &ValidatedInrouOperatorPreseed,
    stage_root: &Path,
) -> Result<PathBuf> {
    let captured = fs::symlink_metadata(&config.helper_path).wrap_err_with(|| {
        format!(
            "failed to inspect offline Inrou operator-preseed helper `{}` for descriptor-bound staging",
            config.helper_path.display()
        )
    })?;
    if captured.file_type().is_symlink()
        || !captured.is_file()
        || captured.len() == 0
        || captured.len() > INROU_PRESEED_HELPER_MAX_BYTES
    {
        return Err(eyre!(
            "offline Inrou operator-preseed helper `{}` changed before descriptor-bound staging",
            config.helper_path.display()
        ));
    }
    let staged = stage_root.join("sorafs-node-preseed-helper");
    copy_taira_stage_source_file(&config.helper_path, &staged, &captured)
        .wrap_err("failed to create the descriptor-bound Inrou preseed helper copy")?;
    set_taira_stage_permissions(&staged, 0o500)?;
    let staged_file = fs::File::open(&staged).wrap_err_with(|| {
        format!(
            "failed to reopen staged Inrou operator-preseed helper `{}`",
            staged.display()
        )
    })?;
    staged_file.sync_all().wrap_err_with(|| {
        format!(
            "failed to synchronize staged Inrou operator-preseed helper `{}`",
            staged.display()
        )
    })?;
    fs::File::open(stage_root)
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| {
            format!(
                "failed to synchronize Inrou preseed stage `{}`",
                stage_root.display()
            )
        })?;
    let staged_sha256 = sha256_file(
        &staged,
        "staged offline Inrou operator-preseed helper",
        INROU_PRESEED_HELPER_MAX_BYTES,
    )?;
    if staged_sha256 != config.helper_sha256 {
        return Err(eyre!(
            "staged offline Inrou operator-preseed helper SHA-256 mismatch: expected {}, got {staged_sha256}",
            config.helper_sha256
        ));
    }
    Ok(staged)
}

#[cfg(not(unix))]
fn stage_verified_inrou_preseed_helper(
    _config: &ValidatedInrouOperatorPreseed,
    _stage_root: &Path,
) -> Result<PathBuf> {
    Err(eyre!(
        "offline Inrou operator-preseed helper staging requires owner-only Unix file permissions"
    ))
}

fn start_inrou_operator_preseed_session(
    config: Option<&ValidatedInrouOperatorPreseed>,
    artifacts: &[&PreparedSorafsArtifact],
    timeout_secs: u64,
) -> Result<Option<InrouOperatorPreseedSession>> {
    let timeout = Duration::from_secs(timeout_secs);
    if timeout.is_zero() {
        return Err(eyre!(
            "Inrou operator-preseed helper timeout must be positive"
        ));
    }
    let Some(config) = config else {
        if !artifacts.is_empty() {
            return Err(eyre!(
                "Inrou artifacts require a validated operator-preseed session"
            ));
        }
        return Ok(None);
    };
    if artifacts.is_empty() {
        return Err(eyre!(
            "validated Inrou operator-preseed session has no artifacts"
        ));
    }
    let stage = SoracloudTempDir::new("iroha-inrou-preseed-session")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(stage.path(), fs::Permissions::from_mode(0o700)).wrap_err_with(
            || {
                format!(
                    "failed to restrict Inrou preseed stage `{}`",
                    stage.path().display()
                )
            },
        )?;
    }
    let staged_helper = stage_verified_inrou_preseed_helper(config, stage.path())?;
    let expected = expected_inrou_preseed_receipt(config, artifacts)?;
    let mut command = ProcessCommand::new(&staged_helper);
    command.arg("preseed-session").arg(format!(
        "--max-capacity-bytes={}",
        config.max_capacity_bytes
    ));
    for target in &config.targets {
        let root = target
            .store_root
            .to_str()
            .ok_or_else(|| eyre!("Inrou preseed store roots must be valid UTF-8"))?;
        command.arg(format!(
            "--target={},{},{root}",
            target.placement.validator_account_id, target.placement.peer_id
        ));
    }
    for (index, artifact) in artifacts.iter().enumerate() {
        let (manifest_path, payload) = stage_inrou_preseed_artifact(stage.path(), index, artifact)?;
        let manifest_path = manifest_path
            .to_str()
            .ok_or_else(|| eyre!("Inrou preseed stage paths must be valid UTF-8"))?;
        command.arg(format!("--manifest={manifest_path}"));
        match payload {
            StagedInrouPreseedPayload::File(path) => {
                let path = path
                    .to_str()
                    .ok_or_else(|| eyre!("Inrou preseed stage paths must be valid UTF-8"))?;
                command.arg(format!("--payload={path}"));
            }
            StagedInrouPreseedPayload::Directory(path) => {
                let path = path
                    .to_str()
                    .ok_or_else(|| eyre!("Inrou preseed stage paths must be valid UTF-8"))?;
                command.arg(format!("--payload-dir={path}"));
            }
        }
    }
    command
        .current_dir(stage.path())
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        command.process_group(0);
    }
    #[cfg(test)]
    command.env(
        "IROHA_TEST_INROU_PRESEED_RECEIPT",
        String::from_utf8(
            json::to_vec(&expected).wrap_err("encode test Inrou preseed ready receipt")?,
        )
        .wrap_err("test Inrou preseed ready receipt must be UTF-8")?,
    );
    #[cfg(test)]
    command.env(
        "IROHA_TEST_INROU_PRESEED_RELEASE_ACK",
        std::str::from_utf8(OPERATOR_PRESEED_SESSION_RELEASE_ACK_V1)
            .expect("operator-preseed release acknowledgment is UTF-8"),
    );
    let staged_helper_sha256 = sha256_file(
        &staged_helper,
        "staged offline Inrou operator-preseed helper",
        INROU_PRESEED_HELPER_MAX_BYTES,
    )?;
    if staged_helper_sha256 != config.helper_sha256 {
        return Err(eyre!(
            "staged offline Inrou operator-preseed helper changed before starting the session"
        ));
    }
    // Descriptor-bound helper/artifact staging above uses regular-file copy, fsync, and
    // write operations governed by fixed V1 byte/count limits. The caller timeout starts
    // here and bounds only the spawned helper's readiness protocol; those synchronous
    // local staging operations are intentionally outside this nonblocking process deadline.
    let readiness_deadline = inrou_preseed_phase_deadline(timeout, "readiness")?;
    let mut child = command.spawn().wrap_err_with(|| {
        format!(
            "failed to start exact offline Inrou operator-preseed helper `{}`",
            staged_helper.display()
        )
    })?;
    let mut stdin = match child.stdin.take() {
        Some(stdin) => Some(stdin),
        None => {
            let error = eyre!("Inrou preseed helper stdin pipe was not created");
            return Err(inrou_preseed_error_after_cleanup(
                child,
                timeout,
                "startup cleanup",
                error,
            ));
        }
    };
    let mut stdout = match child.stdout.take() {
        Some(stdout) => Some(stdout),
        None => {
            drop(stdin.take());
            let error = eyre!("Inrou preseed helper stdout pipe was not created");
            return Err(inrou_preseed_error_after_cleanup(
                child,
                timeout,
                "startup cleanup",
                error,
            ));
        }
    };
    let mut stderr = match child.stderr.take() {
        Some(stderr) => Some(stderr),
        None => {
            drop(stdin.take());
            drop(stdout.take());
            let error = eyre!("Inrou preseed helper stderr pipe was not created");
            return Err(inrou_preseed_error_after_cleanup(
                child,
                timeout,
                "startup cleanup",
                error,
            ));
        }
    };
    let nonblocking_result = make_inrou_preseed_pipe_nonblocking(
        stdout
            .as_ref()
            .expect("preseed startup owns its stdout before readiness"),
        "stdout",
    )
    .and_then(|()| {
        make_inrou_preseed_pipe_nonblocking(
            stderr
                .as_ref()
                .expect("preseed startup owns its stderr before readiness"),
            "stderr",
        )
    });
    if let Err(error) = nonblocking_result {
        drop(stdin.take());
        drop(stdout.take());
        drop(stderr.take());
        return Err(inrou_preseed_error_after_cleanup(
            child,
            timeout,
            "startup cleanup",
            error,
        ));
    }
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut stderr_bytes = Vec::new();
    let readiness = (|| -> Result<OperatorPreseedSessionReceiptV1> {
        let mut receipt_bytes = Vec::new();
        loop {
            if Instant::now() >= readiness_deadline {
                return Err(eyre!(
                    "Inrou preseed helper exceeded its deadline before the ready receipt"
                ));
            }
            drain_inrou_preseed_stdout(
                stdout
                    .as_mut()
                    .expect("live preseed startup owns its stdout"),
                &mut receipt_bytes,
                &mut stdout_eof,
            )?;
            drain_inrou_preseed_stderr(
                stderr
                    .as_mut()
                    .expect("live preseed startup owns its stderr"),
                &mut stderr_bytes,
                &mut stderr_eof,
            )?;
            let status = child
                .try_wait()
                .wrap_err("failed to poll Inrou preseed helper before readiness")?;
            if let Some(newline) = receipt_bytes.iter().position(|byte| *byte == b'\n') {
                if newline + 1 != receipt_bytes.len()
                    || receipt_bytes[..newline].contains(&b'\r')
                    || status.is_some()
                {
                    return Err(eyre!(
                        "Inrou preseed helper did not retain exactly one bounded newline-terminated ready receipt"
                    ));
                }
                break;
            }
            if stdout_eof || status.is_some() {
                return Err(eyre!(
                    "Inrou preseed helper exited before emitting its ready receipt"
                ));
            }
            let remaining = readiness_deadline.saturating_duration_since(Instant::now());
            std::thread::sleep(INROU_PRESEED_PROCESS_POLL_INTERVAL.min(remaining));
        }
        if receipt_bytes.pop() != Some(b'\n')
            || receipt_bytes.is_empty()
            || receipt_bytes.contains(&b'\n')
            || receipt_bytes.contains(&b'\r')
        {
            return Err(eyre!(
                "Inrou preseed helper did not emit exactly one bounded newline-terminated ready receipt"
            ));
        }
        let receipt: OperatorPreseedSessionReceiptV1 = json::from_slice(&receipt_bytes)
            .wrap_err("failed to decode canonical Inrou preseed ready receipt")?;
        receipt
            .validate()
            .map_err(|error| eyre!(error))
            .wrap_err("invalid Inrou preseed ready receipt")?;
        if json::to_vec(&receipt).wrap_err("re-encode Inrou preseed receipt")? != receipt_bytes {
            return Err(eyre!("Inrou preseed ready receipt was not canonical JSON"));
        }
        if receipt != expected {
            return Err(eyre!(
                "Inrou preseed ready receipt did not bind the exact requested stores, capacity, and artifacts"
            ));
        }
        if Instant::now() >= readiness_deadline {
            return Err(eyre!(
                "Inrou preseed helper exceeded its deadline while validating the ready receipt"
            ));
        }
        let mut trailing = Vec::new();
        drain_inrou_preseed_stdout(
            stdout
                .as_mut()
                .expect("live preseed startup owns its stdout"),
            &mut trailing,
            &mut stdout_eof,
        )?;
        drain_inrou_preseed_stderr(
            stderr
                .as_mut()
                .expect("live preseed startup owns its stderr"),
            &mut stderr_bytes,
            &mut stderr_eof,
        )?;
        if !trailing.is_empty() {
            return Err(eyre!(
                "Inrou preseed helper emitted trailing stdout while its ready receipt was validated"
            ));
        }
        if let Some(status) = child
            .try_wait()
            .wrap_err("failed to query Inrou preseed helper readiness")?
        {
            return Err(eyre!(
                "Inrou preseed helper exited with {status} instead of holding store locks"
            ));
        }
        Ok(receipt)
    })();
    let receipt = match readiness {
        Ok(receipt) => receipt,
        Err(error) => {
            drop(stdin.take());
            drop(stdout.take());
            drop(stderr.take());
            return Err(inrou_preseed_error_after_cleanup(
                child,
                timeout,
                "startup cleanup",
                error,
            ));
        }
    };
    Ok(Some(InrouOperatorPreseedSession {
        child: Some(child),
        stdin,
        stdout,
        stderr,
        stderr_bytes,
        stdout_eof,
        stderr_eof,
        timeout,
        receipt,
        _stage: stage,
    }))
}

fn register_prepared_sorafs_artifact(
    artifact: &PreparedSorafsArtifact,
    torii_url: &str,
    authority: &AccountId,
    key_pair: &KeyPair,
    timeout_secs: u64,
) -> Result<()> {
    register_built_sorafs_manifest(
        &artifact.built,
        &artifact.description,
        torii_url,
        authority,
        key_pair,
        timeout_secs,
    )
}

fn register_prepared_sorafs_artifacts<A>(
    artifacts: impl IntoIterator<Item = A>,
    torii_url: &str,
    authority: &AccountId,
    key_pair: &KeyPair,
    timeout_secs: u64,
) -> Result<()>
where
    A: std::borrow::Borrow<PreparedSorafsArtifact>,
{
    for artifact in artifacts {
        let artifact = <A as std::borrow::Borrow<PreparedSorafsArtifact>>::borrow(&artifact);
        register_prepared_sorafs_artifact(artifact, torii_url, authority, key_pair, timeout_secs)?;
    }
    Ok(())
}
fn build_sorafs_artifact_manifest(
    plan: &CarBuildPlan,
    payload: &[u8],
    descriptor: &chunker_registry::ChunkerProfileDescriptor,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
    labels: SorafsManifestBuildLabels<'_>,
) -> Result<BuiltSorafsManifest> {
    let writer = CarWriter::new(plan, payload).wrap_err_with(|| labels.writer.to_owned())?;
    let mut sink = io::sink();
    let car_stats = writer
        .write_to(&mut sink)
        .wrap_err_with(|| labels.metadata.to_owned())?;
    let root_cid = car_stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| eyre!(labels.root.to_owned()))?;
    let car_archive_digest = *car_stats.car_archive_digest.as_bytes();
    let chunk_digest_sha3_256 = compute_chunk_digest_sha3(&plan.chunks);
    let manifest = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_profile(ChunkingProfileV1::from_descriptor(descriptor))
        .chunk_digest_sha3_256(chunk_digest_sha3_256)
        .por_root(compute_por_root(payload, plan).wrap_err_with(|| labels.por.to_owned())?)
        .content_length(plan.content_length)
        .car_digest(car_archive_digest)
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: u16::try_from(SORACLOUD_ARTIFACT_MIN_REPLICAS_V1)
                .expect("first-release artifact replica count fits u16"),
            storage_class: ManifestStorageClass::Hot,
            retention_epoch: release_identity.retention_epoch(),
        })
        .governance(GovernanceProofs::default())
        .build()
        .wrap_err_with(|| labels.manifest.to_owned())?;
    let manifest = attach_sorafs_release_governance(manifest, key_pair, release_identity)
        .wrap_err_with(|| labels.governance.to_owned())?;
    validate_sorafs_release_identity(&manifest, release_identity, labels.manifest)?;
    let (bytes, _) = encode_sorafs_manifest_for_storage(&manifest)
        .wrap_err_with(|| labels.encoding.to_owned())?;
    let digest_hex = hex::encode(
        manifest
            .digest()
            .wrap_err_with(|| labels.digest.to_owned())?
            .as_bytes(),
    );
    Ok(BuiltSorafsManifest {
        manifest,
        bytes,
        digest_hex,
    })
}
fn register_built_sorafs_manifest(
    built: &BuiltSorafsManifest,
    description: &str,
    torii_url: &str,
    authority: &AccountId,
    key_pair: &KeyPair,
    timeout_secs: u64,
) -> Result<()> {
    let mut config = soracloud_submission_config()?;
    config.torii_api_url = url::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?;
    config.torii_request_timeout = Duration::from_secs(timeout_secs.max(1));
    config.account = authority.clone();
    config.key_pair = key_pair.clone();
    register_sorafs_pin_manifest_and_wait(
        &Client::new(config).wrap_err("failed to initialize blocking SoraFS client")?,
        iroha::client::SorafsPinRegisterArgs {
            manifest_payload: &built.bytes,
            alias: None,
            successor_of: None,
        },
        &built.digest_hex,
        description,
        timeout_secs,
    )
}

fn prepare_built_sorafs_manifest_registration(
    built: &BuiltSorafsManifest,
    operation: &str,
    requested_fee_payment: FeePaymentIntent,
    binding: TairaMutationBindingV1,
    torii_url: &str,
    config: &ClientConfig,
    timeout_secs: u64,
) -> Result<PreparedSoracloudTransactionV1> {
    let mut config = config.clone();
    config.torii_api_url = url::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?;
    config.torii_request_timeout = Duration::from_secs(timeout_secs.max(1));
    let client = Client::new(config).wrap_err("failed to initialize blocking Soracloud client")?;
    let instruction =
        iroha::data_model::isi::sorafs::RegisterPinManifest::new(built.bytes.clone(), None, None);
    let payload = client
        .account_client()
        .prepare_transaction(iroha::client::AccountTransactionDraft::new(
            [InstructionBox::from(instruction)],
            requested_fee_payment.clone(),
            binding.metadata(operation)?,
        ))
        .wrap_err("failed to build exact SoraFS pin-registration payload")?;
    let quote = client
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .wrap_err("failed to quote exact SoraFS pin-registration payload")?;
    let mut payload = payload;
    if !requested_fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        return Err(eyre!(
            "SoraFS pin-registration fee quote changed payer or gas bound"
        ));
    }
    quote
        .intent
        .validate()
        .wrap_err("SoraFS pin-registration fee quote is invalid")?;
    payload.fee_payment = quote.intent.clone();
    let transaction = client
        .account_client()
        .sign_transaction(payload)
        .wrap_err("failed to sign exact SoraFS pin-registration payload")?;
    PreparedSoracloudTransactionV1::from_signed(operation, binding, quote, transaction)
}
fn prepare_public_service_discovery(
    bundle: &SoraDeploymentBundleV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<(
    SoracloudPublicServiceDiscoveryV1,
    PublicServiceDiscoveryPublishOutput,
    PreparedSorafsArtifact,
)> {
    let base_url = normalize_public_service_base_url(bundle)?;
    let route = bundle.service.route.as_ref().ok_or_else(|| {
        eyre!(
            "service `{}` does not declare a route",
            bundle.service.service_name
        )
    })?;
    let healthcheck_path = bundle.container.lifecycle.healthcheck_path.clone();
    let healthcheck_url = healthcheck_path.as_ref().map(|path| {
        let mut url = base_url.clone();
        url.set_path(&join_service_route_path(route.path_prefix.as_str(), path));
        url.set_query(None);
        url.set_fragment(None);
        url.to_string()
    });
    let tempdir = SoracloudTempDir::new("iroha-public-service-discovery")
        .wrap_err("failed to create temporary public discovery dir")?;
    let discovery_document = SoracloudPublicServiceDiscoveryDocumentV1 {
        schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
        service_name: bundle.service.service_name.to_string(),
        service_version: bundle.service.service_version.clone(),
        execution_plane: format!("{:?}", bundle.service.execution_plane),
        runtime: format!("{:?}", bundle.container.runtime),
        route_host: route.host.clone(),
        path_prefix: route.path_prefix.clone(),
        base_url: base_url.to_string(),
        healthcheck_path: healthcheck_path.clone(),
        healthcheck_url: healthcheck_url.clone(),
        service_manifest_hash: bundle.service_manifest_hash(),
        container_manifest_hash: bundle.container_manifest_hash(),
        deployment_bundle_hash: Hash::new(Encode::encode(bundle)),
    };
    let discovery_document_bytes = json::to_vec(&discovery_document)
        .wrap_err("failed to encode canonical public discovery document")?;
    let document_hash = Hash::new(&discovery_document_bytes);
    fs::write(
        tempdir.path().join(PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT),
        discovery_document_bytes,
    )
    .wrap_err("failed to write canonical public discovery document")?;
    let descriptor = chunker_registry::default_descriptor();
    let (plan, payload) =
        CarBuildPlan::from_directory_with_profile(tempdir.path(), descriptor.profile).map_err(
            |err| {
                eyre!(
                    "failed to package public discovery `{}`: {err}",
                    tempdir.path().display()
                )
            },
        )?;
    let built = build_sorafs_artifact_manifest(
        &plan,
        &payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: "failed to prepare public discovery CAR writer",
            metadata: "failed to compute public discovery CAR metadata",
            root: "public discovery CAR planning produced no root CID",
            por: "failed to compute public discovery PoR root",
            manifest: "failed to build public discovery manifest",
            governance: "failed to attach public discovery governance proof",
            encoding: "failed to encode public discovery manifest",
            digest: "failed to compute public discovery canonical manifest digest",
        },
    )?;
    let content_cid = encode_content_cid(&built.manifest.root_cid);
    let manifest_digest_hex = built.digest_hex.clone();
    let mut public_discovery_url = base_url.clone();
    public_discovery_url.set_path(&format!(
        "/sorafs/cid/{content_cid}/{PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT}"
    ));
    let cid_host_suffix = sorafs_cid_host_suffix_for_hostname(route.host.as_str());
    let public_discovery_cid_host_url = format!(
        "https://{content_cid}.{cid_host_suffix}/{PUBLIC_SERVICE_DISCOVERY_INDEX_DOCUMENT}"
    );
    let discovery = SoracloudPublicServiceDiscoveryV1 {
        schema_version: discovery_document.schema_version,
        service_name: discovery_document.service_name,
        service_version: discovery_document.service_version,
        execution_plane: discovery_document.execution_plane,
        runtime: discovery_document.runtime,
        route_host: discovery_document.route_host,
        path_prefix: discovery_document.path_prefix,
        base_url: discovery_document.base_url,
        healthcheck_path: discovery_document.healthcheck_path,
        healthcheck_url: discovery_document.healthcheck_url,
        service_manifest_hash: discovery_document.service_manifest_hash,
        container_manifest_hash: discovery_document.container_manifest_hash,
        deployment_bundle_hash: discovery_document.deployment_bundle_hash,
        document_hash,
        content_cid: content_cid.clone(),
        public_discovery_url: public_discovery_url.to_string(),
        public_discovery_cid_host_url: public_discovery_cid_host_url.clone(),
        manifest_digest_hex: manifest_digest_hex.clone(),
    };
    let output = PublicServiceDiscoveryPublishOutput {
        service_name: discovery.service_name.clone(),
        service_version: discovery.service_version.clone(),
        route_host: discovery.route_host.clone(),
        base_url: discovery.base_url.clone(),
        healthcheck_url: discovery.healthcheck_url.clone(),
        document_hash,
        content_cid,
        public_discovery_url: public_discovery_url.to_string(),
        public_discovery_cid_host_url,
        manifest_digest_hex,
    };
    Ok((
        discovery,
        output,
        PreparedSorafsArtifact {
            description: "public discovery".to_owned(),
            plan,
            payload,
            built,
        },
    ))
}

struct PreparedPublicServiceDiscovery {
    artifact: PreparedSorafsArtifact,
    config_value: Json,
    publication: PublicServiceDiscoveryPublishOutput,
}

fn prepare_public_service_discovery_config(
    bundle: &SoraDeploymentBundleV1,
    initial_service_configs: &BTreeMap<String, Json>,
    torii_url: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<Option<PreparedPublicServiceDiscovery>> {
    if initial_service_configs.contains_key(PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME) {
        return Err(eyre!(
            "service `{}` initial configs may not set reserved config `{PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME}`",
            bundle.service.service_name
        ));
    }
    if !service_uses_public_inrou_http_route(bundle) {
        return Ok(None);
    }
    let existing_registry = fetch_existing_public_service_discovery_registry(
        torii_url,
        bundle.service.service_name.as_ref(),
        api_token,
        timeout_secs,
    )?;
    let (discovery, publication, artifact) =
        prepare_public_service_discovery(bundle, key_pair, release_identity)?;
    let mut revisions = existing_registry
        .as_ref()
        .map(|registry| registry.revisions.clone())
        .unwrap_or_default();
    revisions.insert(discovery.service_version.clone(), discovery.clone());
    let registry = SoracloudPublicServiceDiscoveryRegistryV1 {
        schema_version: PUBLIC_SERVICE_DISCOVERY_SCHEMA_VERSION_V1,
        service_name: discovery.service_name.clone(),
        current_version: discovery.service_version.clone(),
        revisions,
    };
    let config_value = Json::from(
        json::to_value(&registry)
            .wrap_err("failed to encode public service discovery registry JSON")?,
    );
    Ok(Some(PreparedPublicServiceDiscovery {
        artifact,
        config_value,
        publication,
    }))
}

fn attach_prepared_public_service_discovery_config(
    initial_service_configs: &mut BTreeMap<String, Json>,
    prepared: Option<PreparedPublicServiceDiscovery>,
) -> Result<Option<PublicServiceDiscoveryPublishOutput>> {
    let Some(prepared) = prepared else {
        return Ok(None);
    };
    if initial_service_configs
        .insert(
            PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME.to_owned(),
            prepared.config_value,
        )
        .is_some()
    {
        return Err(eyre!(
            "reserved config `{PUBLIC_SERVICE_DISCOVERY_CONFIG_NAME}` was inserted after public discovery preparation"
        ));
    }
    Ok(Some(prepared.publication))
}
fn prepare_app_static_site(
    app_manifest: &SoracloudAppManifestV1,
    manifest_dir: &Path,
    static_site: &SoracloudAppStaticSiteV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<(AppStaticSitePublishOutput, PreparedSorafsArtifact)> {
    let planned = plan_app_static_site_publication(
        app_manifest,
        manifest_dir,
        static_site,
        key_pair,
        release_identity,
    )?;
    let dist_dir = resolve_manifest_path(manifest_dir, &static_site.dist_dir);
    let descriptor = chunker_registry::default_descriptor();
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(&dist_dir, descriptor.profile)
        .map_err(|err| {
            eyre!(
                "failed to package static site `{}`: {err}",
                dist_dir.display()
            )
        })?;
    let built = build_sorafs_artifact_manifest(
        &plan,
        &payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: "failed to prepare site CAR writer",
            metadata: "failed to compute site CAR metadata",
            root: "site CAR planning produced no root CID",
            por: "failed to compute app static site PoR root",
            manifest: "failed to build app static site manifest",
            governance: "failed to attach app static site governance proof",
            encoding: "failed to encode app static site manifest",
            digest: "failed to compute app static site canonical manifest digest",
        },
    )?;
    Ok((
        planned,
        PreparedSorafsArtifact {
            description: "app static site".to_owned(),
            plan,
            payload,
            built,
        },
    ))
}
fn plan_app_static_site_publication(
    app_manifest: &SoracloudAppManifestV1,
    manifest_dir: &Path,
    static_site: &SoracloudAppStaticSiteV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<AppStaticSitePublishOutput> {
    if static_site.mount_path != "/" {
        return Err(eyre!(
            "app static site mount_path `{}` is not supported yet; only `/` is supported for published app static sites",
            static_site.mount_path
        ));
    }
    let mut public_url = reqwest::Url::parse(&app_manifest.public_url).wrap_err_with(|| {
        format!(
            "app manifest field `public_url` is not a valid URL: {}",
            app_manifest.public_url
        )
    })?;
    if public_url.path() != "/" {
        return Err(eyre!(
            "app manifest field `public_url` must target the host origin when static_site is enabled; got path `{}`",
            public_url.path()
        ));
    }
    public_url.set_query(None);
    public_url.set_fragment(None);
    let hostname = public_url
        .host_str()
        .ok_or_else(|| eyre!("app manifest field `public_url` must include a hostname"))?
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if hostname.is_empty() {
        return Err(eyre!(
            "app manifest field `public_url` resolved to an empty hostname"
        ));
    }
    let dist_dir = resolve_manifest_path(manifest_dir, &static_site.dist_dir);
    let metadata = fs::metadata(&dist_dir).wrap_err_with(|| {
        format!(
            "failed to access app static site dist_dir `{}`",
            dist_dir.display()
        )
    })?;
    if !metadata.is_dir() {
        return Err(eyre!(
            "app static site dist_dir `{}` must be a directory",
            dist_dir.display()
        ));
    }
    let descriptor = chunker_registry::default_descriptor();
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(&dist_dir, descriptor.profile)
        .map_err(|err| {
            eyre!(
                "failed to package static site `{}`: {err}",
                dist_dir.display()
            )
        })?;
    let built = build_sorafs_artifact_manifest(
        &plan,
        &payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: "failed to prepare site CAR writer",
            metadata: "failed to compute site CAR metadata",
            root: "site CAR planning produced no root CID",
            por: "failed to compute app static site PoR root",
            manifest: "failed to build app static site manifest",
            governance: "failed to attach app static site governance proof",
            encoding: "failed to encode app static site manifest",
            digest: "failed to compute app static site canonical manifest digest",
        },
    )?;
    let content_cid = encode_content_cid(&built.manifest.root_cid);
    let manifest_digest_hex = built.digest_hex;
    let mut cid_gateway_url = public_url.clone();
    cid_gateway_url.set_path(&format!("/sorafs/cid/{content_cid}"));
    let canonical_public_url = public_url
        .as_str()
        .strip_suffix('/')
        .ok_or_else(|| eyre!("canonical app public origin is missing its root path"))?
        .to_owned();
    Ok(AppStaticSitePublishOutput {
        hostname,
        public_url: canonical_public_url,
        cid_gateway_url: cid_gateway_url.to_string(),
        content_cid,
        manifest_digest_hex,
    })
}
fn prepare_sorafs_directory_artifact(
    input_dir: &Path,
    description: &str,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<(PreparedSorafsArtifact, PublishedSorafsDirectoryArtifact)> {
    let metadata = fs::metadata(input_dir)
        .wrap_err_with(|| format!("failed to access {description} `{}`", input_dir.display()))?;
    if !metadata.is_dir() {
        return Err(eyre!(
            "{description} `{}` must be a directory",
            input_dir.display()
        ));
    }
    let descriptor = chunker_registry::default_descriptor();
    let (plan, payload) = CarBuildPlan::from_directory_with_profile(input_dir, descriptor.profile)
        .map_err(|err| {
            eyre!(
                "failed to package {description} `{}`: {err}",
                input_dir.display()
            )
        })?;
    let writer_error = format!("failed to prepare {description} CAR writer");
    let metadata_error = format!("failed to compute {description} CAR metadata");
    let root_error = format!("{description} CAR planning produced no root CID");
    let por_error = format!("failed to compute {description} PoR root");
    let manifest_error = format!("failed to build {description} manifest");
    let governance_error = format!("failed to attach {description} governance proof");
    let encoding_error = format!("failed to encode {description} manifest");
    let digest_error = format!("failed to compute {description} canonical manifest digest");
    let built = build_sorafs_artifact_manifest(
        &plan,
        &payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: &writer_error,
            metadata: &metadata_error,
            root: &root_error,
            por: &por_error,
            manifest: &manifest_error,
            governance: &governance_error,
            encoding: &encoding_error,
            digest: &digest_error,
        },
    )?;
    let content_cid = encode_content_cid(&built.manifest.root_cid);
    let published = PublishedSorafsDirectoryArtifact {
        content_cid,
        manifest_digest_hex: built.digest_hex.clone(),
    };
    Ok((
        PreparedSorafsArtifact {
            description: description.to_owned(),
            plan,
            payload,
            built,
        },
        published,
    ))
}
fn prepare_sorafs_file_artifact(
    input_file: &Path,
    description: &str,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<(PreparedSorafsArtifact, PublishedSorafsFileArtifact)> {
    let metadata = fs::metadata(input_file)
        .wrap_err_with(|| format!("failed to access {description} `{}`", input_file.display()))?;
    if !metadata.is_file() {
        return Err(eyre!(
            "{description} `{}` must be a file",
            input_file.display()
        ));
    }
    let payload = fs::read(input_file)
        .wrap_err_with(|| format!("failed to read {description} `{}`", input_file.display()))?;
    let payload_hash = Hash::new(&payload);
    let descriptor = chunker_registry::default_descriptor();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, descriptor.profile).map_err(|err| {
            eyre!(
                "failed to package {description} `{}`: {err}",
                input_file.display()
            )
        })?;
    let writer_error = format!("failed to prepare {description} CAR writer");
    let metadata_error = format!("failed to compute {description} CAR metadata");
    let root_error = format!("{description} CAR planning produced no root CID");
    let por_error = format!("failed to compute {description} PoR root");
    let manifest_error = format!("failed to build {description} manifest");
    let governance_error = format!("failed to attach {description} governance proof");
    let encoding_error = format!("failed to encode {description} manifest");
    let digest_error = format!("failed to compute {description} canonical manifest digest");
    let built = build_sorafs_artifact_manifest(
        &plan,
        &payload,
        descriptor,
        key_pair,
        release_identity,
        SorafsManifestBuildLabels {
            writer: &writer_error,
            metadata: &metadata_error,
            root: &root_error,
            por: &por_error,
            manifest: &manifest_error,
            governance: &governance_error,
            encoding: &encoding_error,
            digest: &digest_error,
        },
    )?;
    let content_cid = encode_content_cid(&built.manifest.root_cid);
    let published = PublishedSorafsFileArtifact {
        content_cid,
        manifest_digest_hex: built.digest_hex.clone(),
        payload_hash,
    };
    Ok((
        PreparedSorafsArtifact {
            description: description.to_owned(),
            plan,
            payload,
            built,
        },
        published,
    ))
}
fn inrou_member_path(path: &str) -> Result<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(eyre!("Inrou guest image member path must not be empty"));
    }
    trimmed
        .strip_prefix("/inrou/")
        .filter(|relative| !relative.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            eyre!(
                "Inrou guest image path `{path}` must live under `/inrou/` so it can hydrate from the published guest-image artifact"
            )
        })
}
fn validate_local_inrou_member(inrou_dir: &Path, member_path: &str) -> Result<String> {
    let relative_member = inrou_member_path(member_path)?;
    let local_path = inrou_dir.join(&relative_member);
    let metadata = fs::metadata(&local_path).wrap_err_with(|| {
        format!(
            "missing Inrou guest image member `{member_path}` at `{}`; stage real guest images before release",
            local_path.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(eyre!(
            "Inrou guest image member `{member_path}` resolved to `{}` but it is not a nonempty regular file",
            local_path.display()
        ));
    }
    Ok(relative_member)
}
fn validate_local_inrou_guest_image_sources(
    service_workspace_dir: Option<&Path>,
    bundle: &UnpublishedDeploymentBundleV1,
) -> Result<()> {
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
        || bundle.container.runtime != SoraContainerRuntimeV1::Inrou
    {
        return Ok(());
    }
    let inrou = bundle.container.inrou.as_ref().ok_or_else(|| {
        eyre!(
            "service `{}` selects the Inrou runtime without mandatory guest-image source metadata",
            bundle.service.service_name
        )
    })?;
    let workspace_dir = service_workspace_dir.ok_or_else(|| {
        eyre!(
            "service `{}` uses Inrou but its container and service manifests are not in a shared workspace directory",
            bundle.service.service_name
        )
    })?;
    let inrou_dir = workspace_dir.join("inrou");
    for image in inrou.guest_images.values() {
        validate_local_inrou_member(&inrou_dir, &image.kernel_image_path)?;
        validate_local_inrou_member(&inrou_dir, &image.rootfs_image_path)?;
        if let Some(initrd_image_path) = image.initrd_image_path.as_deref() {
            validate_local_inrou_member(&inrou_dir, initrd_image_path)?;
        }
    }
    Ok(())
}
struct PreparedServiceArtifacts {
    admitted_bundle: SoraDeploymentBundleV1,
    published_bundle: ServiceBundlePublishOutput,
    inrou_guest_images: Vec<InrouGuestImageArtifactPublishOutput>,
    artifacts: Vec<PreparedSorafsArtifact>,
}

struct PreparedAppServiceMutation<'a> {
    service: &'a SoracloudAppServiceRefV1,
    container_manifest: PathBuf,
    service_manifest: PathBuf,
    precondition: SoraServiceMutationPreconditionV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    is_hosted_http: bool,
    is_deterministic: bool,
    service_artifacts: PreparedServiceArtifacts,
    public_discovery: Option<PreparedPublicServiceDiscovery>,
}

fn prepare_service_artifacts(
    bundle_file: &Path,
    service_workspace_dir: Option<&Path>,
    bundle: UnpublishedDeploymentBundleV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<PreparedServiceArtifacts> {
    validate_unpublished_deployment_source(&bundle)?;
    validate_local_inrou_guest_image_sources(service_workspace_dir, &bundle)?;
    let (bundle_artifact, published) = prepare_sorafs_file_artifact(
        bundle_file,
        &format!("Soracloud service bundle ({})", bundle.service.service_name),
        key_pair,
        release_identity,
    )?;
    if published.payload_hash != bundle.container.bundle_hash {
        return Err(eyre!(
            "published service bundle `{}` hash {} did not match admitted bundle hash {}",
            bundle_file.display(),
            published.payload_hash,
            bundle.container.bundle_hash
        ));
    }
    let published_bundle = ServiceBundlePublishOutput {
        service_name: bundle.service.service_name.to_string(),
        bundle_file: bundle_file.to_string_lossy().into_owned(),
        content_cid: published.content_cid,
        manifest_digest_hex: published.manifest_digest_hex,
        bundle_hash: published.payload_hash.to_string(),
        note: "service bundle bytes were published to SoraFS for runtime hydration".to_owned(),
    };
    let (inrou_guest_images, published_inrou_artifacts, guest_artifacts) =
        prepare_inrou_guest_image_artifacts(
            service_workspace_dir,
            &bundle,
            key_pair,
            release_identity,
        )?;
    let admitted_bundle = bundle.into_admitted(published_inrou_artifacts)?;
    let mut artifacts = Vec::with_capacity(1 + guest_artifacts.len());
    artifacts.push(bundle_artifact);
    artifacts.extend(guest_artifacts);
    Ok(PreparedServiceArtifacts {
        admitted_bundle,
        published_bundle,
        inrou_guest_images,
        artifacts,
    })
}
fn prepare_inrou_guest_image_artifacts(
    service_workspace_dir: Option<&Path>,
    bundle: &UnpublishedDeploymentBundleV1,
    key_pair: &KeyPair,
    release_identity: SorafsReleaseIdentityV1,
) -> Result<(
    Vec<InrouGuestImageArtifactPublishOutput>,
    BTreeMap<SoraInrouGuestIsaV1, SoraPublishedInrouGuestImageArtifactV1>,
    Vec<PreparedSorafsArtifact>,
)> {
    if bundle.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService
        || bundle.container.runtime != SoraContainerRuntimeV1::Inrou
    {
        return Ok((Vec::new(), BTreeMap::new(), Vec::new()));
    }
    let inrou = bundle.container.inrou.as_ref().ok_or_else(|| {
        eyre!(
            "service `{}` selects the Inrou runtime without mandatory guest-image source metadata",
            bundle.service.service_name
        )
    })?;
    let service_name = bundle.service.service_name.to_string();
    let workspace_dir = service_workspace_dir.ok_or_else(|| {
        eyre!(
            "service `{service_name}` uses Inrou but its container and service manifests are not in a shared workspace directory"
        )
    })?;
    let inrou_dir = workspace_dir.join("inrou");
    let mut outputs = Vec::new();
    let mut published_artifacts = BTreeMap::new();
    let mut prepared_artifacts = Vec::new();
    for (guest_isa_name, image) in &inrou.guest_images {
        let guest_isa = parse_unpublished_inrou_guest_isa(guest_isa_name)?;
        let mut member_paths = vec![
            inrou_member_path(&image.kernel_image_path)?,
            inrou_member_path(&image.rootfs_image_path)?,
        ];
        if let Some(initrd_image_path) = image.initrd_image_path.as_deref() {
            member_paths.push(inrou_member_path(initrd_image_path)?);
        }
        member_paths = member_paths
            .into_iter()
            .map(|member_path| {
                validate_local_inrou_member(&inrou_dir, &format!("/inrou/{member_path}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let staged_artifact = SoracloudTempDir::new("iroha-inrou-guest-image-artifact")
            .wrap_err_with(|| {
                format!(
                    "failed to stage guest-image artifact for {}",
                    guest_isa.as_str()
                )
            })?;
        for member_path in &member_paths {
            let source_path = inrou_dir.join(member_path);
            let destination_path = staged_artifact.path().join(member_path);
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent).wrap_err_with(|| {
                    format!(
                        "failed to create staged guest-image artifact directory `{}`",
                        parent.display()
                    )
                })?;
            }
            fs::copy(&source_path, &destination_path).wrap_err_with(|| {
                format!(
                    "failed to stage guest-image artifact member `{}` into `{}`",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
        let (prepared, published) = prepare_sorafs_directory_artifact(
            staged_artifact.path(),
            &format!("Inrou guest-image artifact ({})", guest_isa.as_str()),
            key_pair,
            release_identity,
        )?;
        let artifact = SoraPublishedInrouGuestImageArtifactV1 {
            manifest_digest_hex: published.manifest_digest_hex.clone(),
            content_cid: published.content_cid.clone(),
        };
        if published_artifacts.insert(guest_isa, artifact).is_some() {
            return Err(eyre!(
                "unpublished Inrou workspace repeats guest ISA `{}`",
                guest_isa.as_str()
            ));
        }
        outputs.push(InrouGuestImageArtifactPublishOutput {
            service_name: service_name.clone(),
            guest_isa: guest_isa.as_str().to_owned(),
            source_dir: inrou_dir.join(guest_isa.as_str()).to_string_lossy().into_owned(),
            hydrate_mount_path: "/inrou".to_owned(),
            member_paths,
            content_cid: published.content_cid.clone(),
            manifest_digest_hex: published.manifest_digest_hex.clone(),
            note: "hosts hydrate these members from the exact authenticated SoraFS artifact reference in the admitted manifest".to_owned(),
        });
        prepared_artifacts.push(prepared);
    }
    Ok((outputs, published_artifacts, prepared_artifacts))
}
fn compute_chunk_digest_sha3(chunks: &[CarChunk]) -> [u8; 32] {
    let mut hasher = Sha3::v256();
    for chunk in chunks {
        hasher.update(&chunk.offset.to_le_bytes());
        hasher.update(&u64::from(chunk.length).to_le_bytes());
        hasher.update(&chunk.digest);
    }
    let mut digest = [0u8; 32];
    hasher.finalize(&mut digest);
    digest
}
fn encode_content_cid(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    if bytes.is_empty() {
        return "b".to_owned();
    }
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity((bytes.len() * 8).div_ceil(5) + 1);
    out.push(b'b');
    for byte in bytes {
        acc = (acc << 8) | (*byte as u32);
        bits += 8;
        while bits >= 5 {
            let index = ((acc >> (bits - 5)) & 0x1f) as usize;
            out.push(ALPHABET[index]);
            bits -= 5;
        }
    }
    if bits > 0 {
        let index = ((acc << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[index]);
    }
    String::from_utf8(out).expect("lowercase base32 CID should be valid UTF-8")
}
fn signed_bundle_request(
    bundle: SoraDeploymentBundleV1,
    initial_service_configs: BTreeMap<String, Json>,
    initial_service_secrets: BTreeMap<String, SecretEnvelopeV1>,
    precondition: SoraServiceMutationPreconditionV1,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedBundleRequest> {
    bundle
        .validate_for_admission()
        .wrap_err("deployment bundle failed canonical admission validation")?;
    if let SoraNetworkPolicyV1::Allowlist(entries) = &bundle.container.capabilities.network {
        for entry in entries {
            if entry.host.is_empty()
                || entry.host.trim() != entry.host
                || entry.host.chars().any(char::is_control)
                || entry.host.chars().any(char::is_whitespace)
            {
                return Err(eyre!(
                    "container capability network allowlist host must use exact canonical V1 bytes"
                ));
            }
        }
    }
    for config_name in initial_service_configs.keys() {
        parse_service_material_name_arg("initial config name", config_name)?;
    }
    for secret_name in initial_service_secrets.keys() {
        parse_service_material_name_arg("initial secret name", secret_name)?;
    }
    let payload = encode_bundle_with_materials_provenance_payload(
        &bundle,
        &initial_service_configs,
        &initial_service_secrets,
        &precondition,
    )
    .wrap_err("failed to encode deployment bundle payload for signing")?;
    Ok(SignedBundleRequest {
        bundle,
        initial_service_configs,
        initial_service_secrets,
        precondition,
        provenance: signed_manifest_provenance(key_pair, &payload)?,
    })
}
fn signed_app_infra_request(
    mode: MutationMode,
    manifest: SoraAppInfraManifestV1,
    services: Vec<SignedBundleRequest>,
    precondition: SoraAppInfraMutationPreconditionV1,
    key_pair: &KeyPair,
) -> Result<SignedAppInfraRequest> {
    let payload = encode_app_infra_provenance_payload(&manifest, &precondition)
        .wrap_err("failed to encode app infra manifest payload for signing")?;
    let (deploy_services, upgrade_services) = match mode {
        MutationMode::Deploy => (services, Vec::new()),
        MutationMode::Upgrade => (Vec::new(), services),
    };
    Ok(SignedAppInfraRequest {
        deploy_services,
        upgrade_services,
        manifest,
        precondition,
        provenance: signed_manifest_provenance(key_pair, &payload)?,
    })
}
fn build_app_infra_manifest(
    app_manifest: &SoracloudAppManifestV1,
    static_site_publication: Option<&AppStaticSitePublishOutput>,
    bundles: &[SoraDeploymentBundleV1],
) -> Result<SoraAppInfraManifestV1> {
    let mut services = Vec::with_capacity(bundles.len());
    for bundle in bundles {
        services.push(build_app_infra_service_ref(bundle)?);
    }
    let app_version = derive_app_infra_version(app_manifest, &services);
    let manifest = SoraAppInfraManifestV1 {
        schema_version: SORA_APP_INFRA_MANIFEST_VERSION_V1,
        app_name: parse_exact_name_arg("app manifest field `app_name`", &app_manifest.app_name)?
            .parse()
            .wrap_err("app manifest field `app_name` is not a valid Iroha name")?,
        app_version,
        public_url: app_manifest.public_url.clone(),
        static_site: build_app_infra_static_site_binding(app_manifest, static_site_publication),
        services,
    };
    manifest
        .validate()
        .wrap_err("app infra manifest failed canonical validation")?;
    Ok(manifest)
}
fn derive_app_infra_version(
    app_manifest: &SoracloudAppManifestV1,
    services: &[SoraAppInfraServiceRefV1],
) -> String {
    if let Some(app_version) = app_manifest.app_version.as_deref() {
        return app_version.to_owned();
    }
    let service_versions = services
        .iter()
        .map(|service| service.service_version.as_str())
        .collect::<BTreeSet<_>>();
    if service_versions.len() == 1
        && let Some(version) = service_versions.iter().next()
    {
        return (*version).to_owned();
    }
    format!("services-{}", Hash::new(Encode::encode(&services.to_vec())))
}
fn build_app_infra_static_site_binding(
    app_manifest: &SoracloudAppManifestV1,
    static_site_publication: Option<&AppStaticSitePublishOutput>,
) -> Option<SoraAppStaticSiteBindingV1> {
    app_manifest
        .static_site
        .as_ref()
        .map(|static_site| SoraAppStaticSiteBindingV1 {
            schema_version: SORA_APP_STATIC_SITE_BINDING_VERSION_V1,
            public_url: static_site_publication
                .map(|publication| publication.public_url.clone())
                .unwrap_or_else(|| app_manifest.public_url.clone()),
            content_cid: static_site_publication.map(|publication| publication.content_cid.clone()),
            manifest_digest_hex: static_site_publication
                .map(|publication| publication.manifest_digest_hex.clone()),
            mount_path: static_site.mount_path.clone(),
            api_base_path: static_site.api_base_path.clone(),
        })
}
fn build_app_infra_service_ref(
    bundle: &SoraDeploymentBundleV1,
) -> Result<SoraAppInfraServiceRefV1> {
    let service = &bundle.service;
    let routes = service
        .route
        .as_ref()
        .map(|route| {
            vec![SoraAppRouteProjectionV1 {
                schema_version: SORA_APP_ROUTE_PROJECTION_VERSION_V1,
                public_host: (route.visibility == SoraRouteVisibilityV1::Public)
                    .then(|| route.host.clone()),
                path_prefix: route.path_prefix.clone(),
                internal_url: Some(format!(
                    "soracloud://{}:{}{}",
                    service.service_name, route.service_port, route.path_prefix
                )),
            }]
        })
        .unwrap_or_default();
    let lease_volumes = service
        .lease_volumes
        .iter()
        .map(|volume| volume.volume_name.clone())
        .collect::<Vec<_>>();
    Ok(SoraAppInfraServiceRefV1 {
        schema_version: SORA_APP_INFRA_SERVICE_REF_VERSION_V1,
        service_name: service.service_name.clone(),
        service_version: service.service_version.clone(),
        service_manifest_hash: bundle.service_manifest_hash(),
        container_manifest_hash: bundle.container_manifest_hash(),
        execution_plane: service.execution_plane,
        runtime: bundle.container.runtime,
        routes,
        lease_volumes,
        shard: app_service_shard_label(&bundle.container),
    })
}
fn app_service_shard_label(container: &SoraContainerManifestV1) -> Option<String> {
    let mut entries = [
        "HAYAHI_CRAWLER_SHARD_ID",
        "HAYAHI_CRAWLER_SHARD_COUNT",
        "SORACLOUD_SHARD_ID",
        "SORACLOUD_SHARD_COUNT",
    ]
    .into_iter()
    .filter_map(|key| container.env.get(key).map(|value| format!("{key}={value}")))
    .collect::<Vec<_>>();
    if entries.is_empty() {
        None
    } else {
        entries.sort();
        Some(entries.join(";"))
    }
}
fn parse_service_material_name_arg(flag_name: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Err(eyre!("{flag_name} must not be empty"));
    }
    if value.trim() != value {
        return Err(eyre!("{flag_name} must not contain surrounding whitespace"));
    }
    if value.len() > 256 {
        return Err(eyre!("{flag_name} exceeds max bytes (256)"));
    }
    if value.starts_with('/') || value.contains("..") {
        return Err(eyre!(
            "{flag_name} must not contain leading `/` or parent path segments"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(eyre!("{flag_name} must not contain control characters"));
    }
    Ok(value.to_owned())
}
fn parse_exact_name_arg(flag_name: &str, value: &str) -> Result<String> {
    if value.trim() != value {
        return Err(eyre!(
            "invalid {flag_name}: must not contain surrounding whitespace"
        ));
    }
    let parsed: Name = value
        .parse()
        .wrap_err_with(|| format!("invalid {flag_name}"))?;
    if parsed.as_ref() != value {
        return Err(eyre!(
            "{flag_name} must use its exact canonical V1 spelling"
        ));
    }
    Ok(value.to_owned())
}
fn require_exact_nonempty_arg(flag_name: &str, value: &str) -> Result<String> {
    if value.is_empty() {
        return Err(eyre!("{flag_name} must not be empty"));
    }
    if value.trim() != value {
        return Err(eyre!("{flag_name} must not contain surrounding whitespace"));
    }
    if value.chars().any(char::is_control) {
        return Err(eyre!("{flag_name} must not contain control characters"));
    }
    Ok(value.to_owned())
}
fn parse_training_identifier_arg(flag_name: &str, value: &str) -> Result<String> {
    let value = require_exact_nonempty_arg(flag_name, value)?;
    if value.len() > 128 {
        return Err(eyre!("{flag_name} exceeds max bytes (128)"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(eyre!(
            "{flag_name} must use only ASCII letters, digits, or [- _ . : #]"
        ));
    }
    Ok(value)
}
fn load_initial_service_configs(path: Option<&Path>) -> Result<BTreeMap<String, Json>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let configs: BTreeMap<String, Json> = load_json(path)?;
    configs
        .into_iter()
        .map(|(config_name, value_json)| {
            Ok((
                parse_service_material_name_arg("--initial-configs key", &config_name)?,
                value_json,
            ))
        })
        .collect()
}
fn load_initial_service_secrets(path: Option<&Path>) -> Result<BTreeMap<String, SecretEnvelopeV1>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let secrets: BTreeMap<String, SecretEnvelopeV1> = load_json(path)?;
    secrets
        .into_iter()
        .map(|(secret_name, envelope)| {
            Ok((
                parse_service_material_name_arg("--initial-secrets key", &secret_name)?,
                envelope,
            ))
        })
        .collect()
}
fn load_service_config_value(
    value_json: Option<&str>,
    value_file: Option<&Path>,
) -> Result<norito::json::Value> {
    match (value_json, value_file) {
        (Some(_), Some(_)) => Err(eyre!("specify exactly one of --value-json or --value-file")),
        (None, None) => Err(eyre!("specify exactly one of --value-json or --value-file")),
        (Some(value_json), None) => {
            json::from_str(value_json).wrap_err("failed to decode --value-json")
        }
        (None, Some(path)) => {
            let bytes = fs::read(path)
                .wrap_err_with(|| format!("failed to read config value file {}", path.display()))?;
            json::from_slice(&bytes)
                .wrap_err_with(|| format!("failed to decode {}", path.display()))
        }
    }
}
fn signed_service_config_set_request(
    service_name: &str,
    config_name: &str,
    value_json: norito::json::Value,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedServiceConfigSetRequest> {
    let payload = ServiceConfigSetPayload {
        service_name: parse_exact_name_arg("--service-name", service_name)?,
        config_name: parse_service_material_name_arg("--config-name", config_name)?,
        value_json: Json::from(value_json),
    };
    let encoded = encode_set_service_config_provenance_payload(
        payload.service_name.as_str(),
        payload.config_name.as_str(),
        &payload.value_json,
    )
    .wrap_err("failed to encode service config payload for signing")?;
    Ok(SignedServiceConfigSetRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_service_config_delete_request(
    service_name: &str,
    config_name: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedServiceConfigDeleteRequest> {
    let payload = ServiceConfigDeletePayload {
        service_name: parse_exact_name_arg("--service-name", service_name)?,
        config_name: parse_service_material_name_arg("--config-name", config_name)?,
    };
    let encoded = encode_delete_service_config_provenance_payload(
        payload.service_name.as_str(),
        payload.config_name.as_str(),
    )
    .wrap_err("failed to encode service config delete payload for signing")?;
    Ok(SignedServiceConfigDeleteRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_service_secret_set_request(
    service_name: &str,
    secret_name: &str,
    secret: SecretEnvelopeV1,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedServiceSecretSetRequest> {
    let payload = ServiceSecretSetPayload {
        service_name: parse_exact_name_arg("--service-name", service_name)?,
        secret_name: parse_service_material_name_arg("--secret-name", secret_name)?,
        secret,
    };
    let encoded = encode_set_service_secret_provenance_payload(
        payload.service_name.as_str(),
        payload.secret_name.as_str(),
        &payload.secret,
    )
    .wrap_err("failed to encode service secret payload for signing")?;
    Ok(SignedServiceSecretSetRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_service_secret_delete_request(
    service_name: &str,
    secret_name: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedServiceSecretDeleteRequest> {
    let payload = ServiceSecretDeletePayload {
        service_name: parse_exact_name_arg("--service-name", service_name)?,
        secret_name: parse_service_material_name_arg("--secret-name", secret_name)?,
    };
    let encoded = encode_delete_service_secret_provenance_payload(
        payload.service_name.as_str(),
        payload.secret_name.as_str(),
    )
    .wrap_err("failed to encode service secret delete payload for signing")?;
    Ok(SignedServiceSecretDeleteRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_rollback_request(
    service_name: &str,
    target_version: &str,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedRollbackRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let target_version = require_exact_nonempty_arg("--target-version", target_version)?;
    let payload = RollbackPayload {
        service_name,
        target_version,
    };
    let encoded = encode_rollback_signature_payload(&payload)
        .wrap_err("failed to encode rollback payload for signing")?;
    Ok(SignedRollbackRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn encode_rollback_signature_payload(payload: &RollbackPayload) -> Result<Vec<u8>> {
    encode_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.target_version.as_str(),
    )
    .wrap_err("failed to encode rollback signature payload tuple")
}
fn signed_rollout_request(
    service_name: &str,
    rollout_handle: &str,
    healthy: bool,
    promote_to_percent: Option<u8>,
    governance_tx_hash: Hash,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedRolloutAdvanceRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let rollout_handle = require_exact_nonempty_arg("--rollout-handle", rollout_handle)?;
    if promote_to_percent.is_some_and(|value| value > 100) {
        return Err(eyre!("--promote-to-percent must be within 0..=100"));
    }
    match (healthy, promote_to_percent) {
        (true, None) => {
            return Err(eyre!("healthy rollout steps require --promote-to-percent"));
        }
        (false, Some(_)) => {
            return Err(eyre!("unhealthy rollout steps forbid --promote-to-percent"));
        }
        (true, Some(_)) | (false, None) => {}
    }
    let payload = RolloutAdvancePayload {
        service_name,
        rollout_handle,
        healthy,
        promote_to_percent,
        governance_tx_hash,
    };
    let encoded = encode_rollout_signature_payload(&payload)
        .wrap_err("failed to encode rollout payload for signing")?;
    Ok(SignedRolloutAdvanceRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_deploy_request(
    manifest: AgentApartmentManifestV1,
    lease_ticks: u64,
    autonomy_budget_units: u64,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentDeployRequest> {
    let payload = AgentDeployPayload {
        manifest,
        lease_ticks,
        autonomy_budget_units,
    };
    let encoded = encode_agent_deploy_signature_payload(&payload)
        .wrap_err("failed to encode agent deploy payload for signing")?;
    Ok(SignedAgentDeployRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_lease_renew_request(
    apartment_name: &str,
    lease_ticks: u64,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentLeaseRenewRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    if lease_ticks == 0 {
        return Err(eyre!("--lease-ticks must be greater than zero"));
    }
    let payload = AgentLeaseRenewPayload {
        apartment_name,
        lease_ticks,
    };
    let encoded = encode_agent_lease_renew_signature_payload(&payload)
        .wrap_err("failed to encode agent lease renew payload for signing")?;
    Ok(SignedAgentLeaseRenewRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn parse_hf_repo_id_arg(repo_id: &str) -> Result<String> {
    if !is_canonical_hf_repo_id_v1(repo_id) {
        return Err(eyre!(
            "--repo-id must be one exact fully-qualified `namespace/repository` identifier"
        ));
    }
    Ok(repo_id.to_owned())
}
fn parse_hf_revision_arg(revision: &str) -> Result<String> {
    if !is_canonical_hf_commit_oid_v1(revision) {
        return Err(eyre!(
            "--revision must be the full 40-character lowercase hexadecimal commit OID"
        ));
    }
    Ok(revision.to_owned())
}
fn parse_hf_service_name_arg(service_name: &str) -> Result<String> {
    let parsed = service_name
        .parse::<Name>()
        .wrap_err("invalid --service-name")?;
    if parsed.as_ref() != service_name {
        return Err(eyre!(
            "--service-name must use its exact canonical V1 spelling"
        ));
    }
    Ok(service_name.to_owned())
}
fn parse_hf_apartment_name_arg(apartment_name: Option<&str>) -> Result<Option<String>> {
    apartment_name
        .map(|name| {
            let parsed = name.parse::<Name>().wrap_err("invalid --apartment-name")?;
            if parsed.as_ref() != name {
                return Err(eyre!(
                    "--apartment-name must use its exact canonical V1 spelling"
                ));
            }
            Ok(name.to_owned())
        })
        .transpose()
}
fn parse_hf_account_id_arg(account_id: Option<&str>) -> Result<Option<String>> {
    account_id
        .map(|literal| {
            if literal.trim() != literal {
                return Err(eyre!(
                    "--account-id must not contain surrounding whitespace"
                ));
            }
            let parsed = AccountId::parse_encoded(literal).wrap_err("invalid --account-id")?;
            if parsed.to_string() != literal {
                return Err(eyre!(
                    "--account-id must use its exact canonical V1 spelling"
                ));
            }
            Ok(literal.to_owned())
        })
        .transpose()
}
fn parse_asset_definition_arg(
    flag_name: &str,
    asset_definition: &str,
) -> Result<AssetDefinitionId> {
    if asset_definition.trim() != asset_definition {
        return Err(eyre!("{flag_name} must not contain surrounding whitespace"));
    }
    let parsed: AssetDefinitionId = asset_definition
        .parse()
        .wrap_err_with(|| format!("invalid {flag_name}"))?;
    if parsed.to_string() != asset_definition {
        return Err(eyre!(
            "{flag_name} must use its exact canonical V1 spelling"
        ));
    }
    Ok(parsed)
}
fn parse_positive_quantity(value: &str) -> std::result::Result<Quantity, String> {
    let quantity = value
        .parse::<Quantity>()
        .map_err(|error| format!("must be a canonical non-negative quantity: {error}"))?;
    if quantity.to_string() != value {
        return Err(format!("must use canonical quantity spelling `{quantity}`"));
    }
    if quantity.is_zero() {
        return Err("quantity must be greater than zero".to_owned());
    }
    Ok(quantity)
}
fn parse_agent_wallet_asset_definition(value: &str) -> std::result::Result<String, String> {
    if value.trim() != value {
        return Err("must not contain surrounding whitespace".to_owned());
    }
    let parsed = value
        .parse::<AssetDefinitionId>()
        .map_err(|error| format!("must be a canonical asset definition: {error}"))?;
    if parsed.to_string() != value {
        return Err(format!(
            "must use canonical asset definition spelling `{parsed}`"
        ));
    }
    Ok(value.to_owned())
}
fn parse_agent_wallet_request_id(value: &str) -> std::result::Result<String, String> {
    if !is_canonical_agent_wallet_request_id_v1(value) {
        return Err(
            "must be a canonical V1 wallet request id (1..=128 bytes, no surrounding whitespace or control characters)"
                .to_owned(),
        );
    }
    Ok(value.to_owned())
}
#[allow(clippy::too_many_arguments)]
fn signed_hf_shared_lease_join_request(
    repo_id: &str,
    revision: &str,
    service_name: &str,
    apartment_name: Option<&str>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition: &str,
    base_fee: &Quantity,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedHfSharedLeaseJoinRequest> {
    if lease_term_ms == 0 {
        return Err(eyre!("--lease-term-ms must be greater than zero"));
    }
    if base_fee.is_zero() {
        return Err(eyre!("--base-fee must be greater than zero"));
    }
    let repo_id = parse_hf_repo_id_arg(repo_id)?;
    let resolved_revision = parse_hf_revision_arg(revision)?;
    let service_name = parse_hf_service_name_arg(service_name)?;
    let apartment_name = parse_hf_apartment_name_arg(apartment_name)?;
    let payload = HfSharedLeaseJoinPayload {
        repo_id: repo_id.clone(),
        revision: resolved_revision.clone(),
        service_name: service_name.clone(),
        apartment_name: apartment_name.clone(),
        storage_class,
        lease_term_ms,
        lease_asset_definition_id: parse_asset_definition_arg(
            "--lease-asset-definition",
            lease_asset_definition,
        )?,
        base_fee: base_fee.clone(),
    };
    let encoded = encode_hf_shared_lease_join_signature_payload(&payload)
        .wrap_err("failed to encode hf shared-lease join payload for signing")?;
    Ok(SignedHfSharedLeaseJoinRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_hf_lease_leave_request(
    repo_id: &str,
    revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    service_name: Option<&str>,
    apartment_name: Option<&str>,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedHfLeaseLeaveRequest> {
    if lease_term_ms == 0 {
        return Err(eyre!("--lease-term-ms must be greater than zero"));
    }
    let payload = HfLeaseLeavePayload {
        repo_id: parse_hf_repo_id_arg(repo_id)?,
        revision: parse_hf_revision_arg(revision)?,
        storage_class,
        lease_term_ms,
        service_name: service_name.map(parse_hf_service_name_arg).transpose()?,
        apartment_name: parse_hf_apartment_name_arg(apartment_name)?,
    };
    let encoded = encode_hf_lease_leave_signature_payload(&payload)
        .wrap_err("failed to encode hf lease leave payload for signing")?;
    Ok(SignedHfLeaseLeaveRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
#[allow(clippy::too_many_arguments)]
fn signed_hf_lease_renew_request(
    repo_id: &str,
    revision: &str,
    service_name: &str,
    apartment_name: Option<&str>,
    storage_class: StorageClass,
    lease_term_ms: u64,
    lease_asset_definition: &str,
    base_fee: &Quantity,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedHfLeaseRenewRequest> {
    if lease_term_ms == 0 {
        return Err(eyre!("--lease-term-ms must be greater than zero"));
    }
    if base_fee.is_zero() {
        return Err(eyre!("--base-fee must be greater than zero"));
    }
    let repo_id = parse_hf_repo_id_arg(repo_id)?;
    let resolved_revision = parse_hf_revision_arg(revision)?;
    let service_name = parse_hf_service_name_arg(service_name)?;
    let apartment_name = parse_hf_apartment_name_arg(apartment_name)?;
    let payload = HfLeaseRenewPayload {
        repo_id: repo_id.clone(),
        revision: resolved_revision.clone(),
        service_name: service_name.clone(),
        apartment_name: apartment_name.clone(),
        storage_class,
        lease_term_ms,
        lease_asset_definition_id: parse_asset_definition_arg(
            "--lease-asset-definition",
            lease_asset_definition,
        )?,
        base_fee: base_fee.clone(),
    };
    let encoded = encode_hf_lease_renew_signature_payload(&payload)
        .wrap_err("failed to encode hf lease renew payload for signing")?;
    Ok(SignedHfLeaseRenewRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_restart_request(
    apartment_name: &str,
    reason: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentRestartRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    let reason = require_exact_nonempty_arg("--reason", reason)?;
    let payload = AgentRestartPayload {
        apartment_name,
        reason,
    };
    let encoded = encode_agent_restart_signature_payload(&payload)
        .wrap_err("failed to encode agent restart payload for signing")?;
    Ok(SignedAgentRestartRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_policy_revoke_request(
    apartment_name: &str,
    capability: &str,
    reason: Option<&str>,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentPolicyRevokeRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    let capability = require_exact_nonempty_arg("--capability", capability)?;
    let reason = reason
        .map(|value| require_exact_nonempty_arg("--reason", value))
        .transpose()?;
    let payload = AgentPolicyRevokePayload {
        apartment_name,
        capability,
        reason,
    };
    let encoded = encode_agent_policy_revoke_signature_payload(&payload)
        .wrap_err("failed to encode agent policy revoke payload for signing")?;
    Ok(SignedAgentPolicyRevokeRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_wallet_spend_request(
    apartment_name: &str,
    request_id: &str,
    asset_definition: &str,
    amount: &Quantity,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentWalletSpendRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    parse_agent_wallet_request_id(request_id).map_err(|error| eyre!("--request-id {error}"))?;
    let asset_definition = parse_agent_wallet_asset_definition(asset_definition)
        .map_err(|error| eyre!("invalid --asset-definition: {error}"))?;
    if amount.is_zero() {
        return Err(eyre!("--amount must be greater than zero"));
    }
    let payload = AgentWalletSpendPayload {
        apartment_name,
        request_id: request_id.to_owned(),
        asset_definition,
        amount: amount.clone(),
    };
    let encoded = encode_agent_wallet_spend_signature_payload(&payload)
        .wrap_err("failed to encode agent wallet spend payload for signing")?;
    Ok(SignedAgentWalletSpendRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_wallet_approve_request(
    apartment_name: &str,
    request_id: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentWalletApproveRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    parse_agent_wallet_request_id(request_id).map_err(|error| eyre!("--request-id {error}"))?;
    let payload = AgentWalletApprovePayload {
        apartment_name,
        request_id: request_id.to_owned(),
    };
    let encoded = encode_agent_wallet_approve_signature_payload(&payload)
        .wrap_err("failed to encode agent wallet approve payload for signing")?;
    Ok(SignedAgentWalletApproveRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_message_send_request(
    from_apartment: &str,
    to_apartment: &str,
    channel: &str,
    payload: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentMessageSendRequest> {
    let from_apartment = parse_exact_name_arg("--from-apartment", from_apartment)?;
    let to_apartment = parse_exact_name_arg("--to-apartment", to_apartment)?;
    let channel = require_exact_nonempty_arg("--channel", channel)?;
    let message_payload = require_exact_nonempty_arg("--payload", payload)?;
    let payload = AgentMessageSendPayload {
        from_apartment,
        to_apartment,
        channel,
        payload: message_payload,
    };
    let encoded = encode_agent_message_send_signature_payload(&payload)
        .wrap_err("failed to encode agent message send payload for signing")?;
    Ok(SignedAgentMessageSendRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_agent_message_ack_request(
    apartment_name: &str,
    message_id: &str,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentMessageAckRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    let message_id = require_exact_nonempty_arg("--message-id", message_id)?;
    let payload = AgentMessageAckPayload {
        apartment_name,
        message_id,
    };
    let encoded = encode_agent_message_ack_signature_payload(&payload)
        .wrap_err("failed to encode agent message ack payload for signing")?;
    Ok(SignedAgentMessageAckRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn validate_hash_like_value(flag_name: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(eyre!("{flag_name} must not be empty"));
    }
    if value.len() > AGENT_AUTONOMY_MAX_HASH_BYTES {
        return Err(eyre!(
            "{flag_name} exceeds max bytes ({AGENT_AUTONOMY_MAX_HASH_BYTES})"
        ));
    }
    if value.chars().any(|ch| ch.is_ascii_whitespace()) {
        return Err(eyre!("{flag_name} must not contain whitespace"));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '-' | '_' | '.' | '#'))
    {
        return Err(eyre!(
            "{flag_name} must use only ASCII letters, digits, or [: - _ . #]"
        ));
    }
    Ok(())
}
fn parse_exact_agent_workflow_input_json(workflow_input_json: &str) -> Result<String> {
    if workflow_input_json.is_empty() {
        return Err(eyre!("workflow input JSON must not be empty"));
    }
    if workflow_input_json.trim() != workflow_input_json {
        return Err(eyre!(
            "workflow input JSON must not contain surrounding whitespace"
        ));
    }
    if workflow_input_json.len() > AGENT_AUTONOMY_MAX_REQUEST_BYTES {
        return Err(eyre!(
            "workflow input JSON exceeds max bytes ({AGENT_AUTONOMY_MAX_REQUEST_BYTES})"
        ));
    }
    let parsed: norito::json::Value =
        json::from_str(workflow_input_json).wrap_err("workflow input JSON must be valid JSON")?;
    let canonical =
        json::to_json(&parsed).wrap_err("failed to canonicalize workflow input JSON")?;
    if canonical != workflow_input_json {
        return Err(eyre!(
            "workflow input JSON must already use the exact canonical Norito JSON spelling `{canonical}`"
        ));
    }
    Ok(workflow_input_json.to_owned())
}
fn signed_agent_artifact_allow_request(
    apartment_name: &str,
    artifact_hash: &str,
    provenance_hash: Option<&str>,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedAgentArtifactAllowRequest> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    validate_hash_like_value("--artifact-hash", artifact_hash)?;
    if let Some(provenance_hash) = provenance_hash {
        validate_hash_like_value("--provenance-hash", provenance_hash)?;
    }
    let payload = AgentArtifactAllowPayload {
        apartment_name,
        artifact_hash: artifact_hash.to_owned(),
        provenance_hash: provenance_hash.map(ToOwned::to_owned),
    };
    let encoded = encode_agent_artifact_allow_signature_payload(&payload)
        .wrap_err("failed to encode agent autonomy allow payload for signing")?;
    Ok(SignedAgentArtifactAllowRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_training_job_start_request(
    service_name: &str,
    model_name: &str,
    job_id: &str,
    worker_group_size: u16,
    target_steps: u32,
    checkpoint_interval_steps: u32,
    max_retries: u8,
    step_compute_units: u64,
    compute_budget_units: u64,
    storage_budget_bytes: u64,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobStartRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let job_id = parse_training_identifier_arg("--job-id", job_id)?;
    if worker_group_size == 0 {
        return Err(eyre!("--worker-group-size must be greater than zero"));
    }
    if target_steps == 0 {
        return Err(eyre!("--target-steps must be greater than zero"));
    }
    if checkpoint_interval_steps == 0 {
        return Err(eyre!(
            "--checkpoint-interval-steps must be greater than zero"
        ));
    }
    if step_compute_units == 0 {
        return Err(eyre!("--step-compute-units must be greater than zero"));
    }
    if compute_budget_units == 0 {
        return Err(eyre!("--compute-budget-units must be greater than zero"));
    }
    if storage_budget_bytes == 0 {
        return Err(eyre!("--storage-budget-bytes must be greater than zero"));
    }
    let payload = TrainingJobStartPayload {
        service_name,
        model_name,
        job_id,
        worker_group_size,
        target_steps,
        checkpoint_interval_steps,
        max_retries,
        step_compute_units,
        compute_budget_units,
        storage_budget_bytes,
    };
    let encoded = encode_training_job_start_signature_payload(&payload)
        .wrap_err("failed to encode training job start payload for signing")?;
    Ok(SignedTrainingJobStartRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_training_job_checkpoint_request(
    service_name: &str,
    job_id: &str,
    completed_step: u32,
    checkpoint_size_bytes: u64,
    metrics_hash: Hash,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobCheckpointRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let job_id = parse_training_identifier_arg("--job-id", job_id)?;
    if completed_step == 0 {
        return Err(eyre!("--completed-step must be greater than zero"));
    }
    if checkpoint_size_bytes == 0 {
        return Err(eyre!("--checkpoint-size-bytes must be greater than zero"));
    }
    let payload = TrainingJobCheckpointPayload {
        service_name,
        job_id,
        completed_step,
        checkpoint_size_bytes,
        metrics_hash,
    };
    let encoded = encode_training_job_checkpoint_signature_payload(&payload)
        .wrap_err("failed to encode training job checkpoint payload for signing")?;
    Ok(SignedTrainingJobCheckpointRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_training_job_retry_request(
    service_name: &str,
    job_id: &str,
    reason: &str,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedTrainingJobRetryRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let job_id = parse_training_identifier_arg("--job-id", job_id)?;
    let reason = require_exact_nonempty_arg("--reason", reason)?;
    let payload = TrainingJobRetryPayload {
        service_name,
        job_id,
        reason,
    };
    let encoded = encode_training_job_retry_signature_payload(&payload)
        .wrap_err("failed to encode training job retry payload for signing")?;
    Ok(SignedTrainingJobRetryRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
#[allow(clippy::too_many_arguments)]
fn signed_model_artifact_register_request(
    service_name: &str,
    model_name: &str,
    training_job_id: &str,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedModelArtifactRegisterRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let training_job_id = parse_training_identifier_arg("--training-job-id", training_job_id)?;
    let dataset_ref = require_exact_nonempty_arg("--dataset-ref", dataset_ref)?;
    let payload = ModelArtifactRegisterPayload {
        service_name,
        model_name,
        training_job_id,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    };
    let encoded = encode_model_artifact_register_signature_payload(&payload)
        .wrap_err("failed to encode model artifact register payload for signing")?;
    Ok(SignedModelArtifactRegisterRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
#[allow(clippy::too_many_arguments)]
fn signed_model_weight_register_request(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    training_job_id: &str,
    parent_version: Option<&str>,
    weight_artifact_hash: Hash,
    dataset_ref: &str,
    training_config_hash: Hash,
    reproducibility_hash: Hash,
    provenance_attestation_hash: Hash,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightRegisterRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let weight_version = parse_training_identifier_arg("--weight-version", weight_version)?;
    let training_job_id = parse_training_identifier_arg("--training-job-id", training_job_id)?;
    let parent_version = parent_version
        .map(|value| parse_training_identifier_arg("--parent-version", value))
        .transpose()?;
    let dataset_ref = require_exact_nonempty_arg("--dataset-ref", dataset_ref)?;
    let payload = ModelWeightRegisterPayload {
        service_name,
        model_name,
        weight_version,
        training_job_id,
        parent_version,
        weight_artifact_hash,
        dataset_ref,
        training_config_hash,
        reproducibility_hash,
        provenance_attestation_hash,
    };
    let encoded = encode_model_weight_register_signature_payload(&payload)
        .wrap_err("failed to encode model weight register payload for signing")?;
    Ok(SignedModelWeightRegisterRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_model_weight_promote_request(
    service_name: &str,
    model_name: &str,
    weight_version: &str,
    gate_approved: bool,
    gate_report_hash: Hash,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightPromoteRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let weight_version = parse_training_identifier_arg("--weight-version", weight_version)?;
    let payload = ModelWeightPromotePayload {
        service_name,
        model_name,
        weight_version,
        gate_approved,
        gate_report_hash,
    };
    let encoded = encode_model_weight_promote_signature_payload(&payload)
        .wrap_err("failed to encode model weight promote payload for signing")?;
    Ok(SignedModelWeightPromoteRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_model_weight_rollback_request(
    service_name: &str,
    model_name: &str,
    target_version: &str,
    reason: &str,
    _authority: Option<&AccountId>,
    key_pair: &KeyPair,
) -> Result<SignedModelWeightRollbackRequest> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let target_version = parse_training_identifier_arg("--target-version", target_version)?;
    let reason = require_exact_nonempty_arg("--reason", reason)?;
    let payload = ModelWeightRollbackPayload {
        service_name,
        model_name,
        target_version,
        reason,
    };
    let encoded = encode_model_weight_rollback_signature_payload(&payload)
        .wrap_err("failed to encode model weight rollback payload for signing")?;
    Ok(SignedModelWeightRollbackRequest {
        payload,
        provenance: signed_manifest_provenance(key_pair, &encoded)?,
    })
}
fn signed_uploaded_model_register_request(
    bundle: SoraUploadedModelBundleV1,
    finalize: UploadedModelFinalizePayload,
    _authority: &AccountId,
    key_pair: &KeyPair,
) -> Result<SignedUploadedModelRegisterRequest> {
    bundle
        .validate()
        .wrap_err("invalid uploaded model bundle")?;
    let service_name = parse_exact_name_arg("register service_name", &finalize.service_name)?;
    if service_name != bundle.service_name.as_ref() {
        return Err(eyre!(
            "register finalize service_name `{}` must match bundle service_name `{}`",
            finalize.service_name,
            bundle.service_name
        ));
    }
    if finalize.model_id != bundle.model_id {
        return Err(eyre!(
            "register finalize model_id `{}` must match bundle model_id `{}`",
            finalize.model_id,
            bundle.model_id
        ));
    }
    if finalize.weight_version != bundle.weight_version {
        return Err(eyre!(
            "register finalize weight_version `{}` must match bundle weight_version `{}`",
            finalize.weight_version,
            bundle.weight_version
        ));
    }
    if finalize.bundle_root != bundle.bundle_root {
        return Err(eyre!(
            "register finalize bundle_root must match bundle bundle_root"
        ));
    }
    let model_name = parse_exact_name_arg("register model_name", &finalize.model_name)?;
    let artifact_id = parse_training_identifier_arg("register artifact_id", &finalize.artifact_id)?;
    let dataset_ref = require_exact_nonempty_arg("register dataset_ref", &finalize.dataset_ref)?;
    let payload = UploadedModelRegisterPayload {
        bundle,
        model_name,
        artifact_id,
        weight_artifact_hash: finalize.weight_artifact_hash,
        dataset_ref,
        training_config_hash: finalize.training_config_hash,
        reproducibility_hash: finalize.reproducibility_hash,
        provenance_attestation_hash: finalize.provenance_attestation_hash,
    };
    let bundle_encoded =
        encode_uploaded_model_bundle_register_provenance_payload(payload.bundle.clone())
            .wrap_err("failed to encode uploaded model register bundle signature payload")?;
    let finalize_encoded = encode_uploaded_model_finalize_provenance_payload(
        payload.bundle.service_name.as_ref(),
        payload.model_name.as_str(),
        payload.bundle.model_id.as_str(),
        payload.artifact_id.as_str(),
        payload.bundle.weight_version.as_str(),
        payload.bundle.bundle_root,
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .wrap_err("failed to encode uploaded model register finalize signature payload")?;
    Ok(SignedUploadedModelRegisterRequest {
        payload,
        bundle_provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: sign_soracloud_payload(key_pair, &bundle_encoded)?,
        },
        finalize_provenance: ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature: sign_soracloud_payload(key_pair, &finalize_encoded)?,
        },
    })
}
fn encode_rollout_signature_payload(payload: &RolloutAdvancePayload) -> Result<Vec<u8>> {
    encode_rollout_provenance_payload(
        payload.service_name.as_str(),
        payload.rollout_handle.as_str(),
        payload.healthy,
        payload.promote_to_percent,
        payload.governance_tx_hash.clone(),
    )
    .wrap_err("failed to encode rollout signature payload tuple")
}
fn encode_agent_deploy_signature_payload(payload: &AgentDeployPayload) -> Result<Vec<u8>> {
    encode_agent_deploy_provenance_payload(
        payload.manifest.clone(),
        payload.lease_ticks,
        payload.autonomy_budget_units,
    )
    .wrap_err("failed to encode agent deploy signature payload tuple")
}
fn encode_agent_lease_renew_signature_payload(payload: &AgentLeaseRenewPayload) -> Result<Vec<u8>> {
    encode_agent_lease_renew_provenance_payload(
        payload.apartment_name.as_str(),
        payload.lease_ticks,
    )
    .wrap_err("failed to encode agent lease renew signature payload tuple")
}
fn encode_agent_restart_signature_payload(payload: &AgentRestartPayload) -> Result<Vec<u8>> {
    encode_agent_restart_provenance_payload(
        payload.apartment_name.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode agent restart signature payload tuple")
}
fn encode_agent_policy_revoke_signature_payload(
    payload: &AgentPolicyRevokePayload,
) -> Result<Vec<u8>> {
    encode_agent_policy_revoke_provenance_payload(
        payload.apartment_name.as_str(),
        payload.capability.as_str(),
        payload.reason.as_deref(),
    )
    .wrap_err("failed to encode agent policy revoke signature payload tuple")
}
fn encode_agent_wallet_spend_signature_payload(
    payload: &AgentWalletSpendPayload,
) -> Result<Vec<u8>> {
    encode_agent_wallet_spend_provenance_payload(
        payload.apartment_name.as_str(),
        payload.request_id.as_str(),
        payload.asset_definition.as_str(),
        &payload.amount,
    )
    .wrap_err("failed to encode agent wallet spend signature payload tuple")
}
fn encode_agent_wallet_approve_signature_payload(
    payload: &AgentWalletApprovePayload,
) -> Result<Vec<u8>> {
    encode_agent_wallet_approve_provenance_payload(
        payload.apartment_name.as_str(),
        payload.request_id.as_str(),
    )
    .wrap_err("failed to encode agent wallet approve signature payload tuple")
}
fn encode_agent_message_send_signature_payload(
    payload: &AgentMessageSendPayload,
) -> Result<Vec<u8>> {
    encode_agent_message_send_provenance_payload(
        payload.from_apartment.as_str(),
        payload.to_apartment.as_str(),
        payload.channel.as_str(),
        payload.payload.as_str(),
    )
    .wrap_err("failed to encode agent message send signature payload tuple")
}
fn encode_agent_message_ack_signature_payload(payload: &AgentMessageAckPayload) -> Result<Vec<u8>> {
    encode_agent_message_ack_provenance_payload(
        payload.apartment_name.as_str(),
        payload.message_id.as_str(),
    )
    .wrap_err("failed to encode agent message ack signature payload tuple")
}
fn encode_agent_artifact_allow_signature_payload(
    payload: &AgentArtifactAllowPayload,
) -> Result<Vec<u8>> {
    encode_agent_artifact_allow_provenance_payload(
        payload.apartment_name.as_str(),
        payload.artifact_hash.as_str(),
        payload.provenance_hash.as_deref(),
    )
    .wrap_err("failed to encode agent artifact allow signature payload tuple")
}
fn encode_hf_shared_lease_join_signature_payload(
    payload: &HfSharedLeaseJoinPayload,
) -> Result<Vec<u8>> {
    let repo_id = parse_hf_repo_id_arg(&payload.repo_id)?;
    let resolved_revision = parse_hf_revision_arg(&payload.revision)?;
    let service_name = parse_hf_service_name_arg(&payload.service_name)?;
    let apartment_name = parse_hf_apartment_name_arg(payload.apartment_name.as_deref())?;
    if payload.lease_term_ms == 0 {
        return Err(eyre!("lease_term_ms must be greater than zero"));
    }
    if payload.base_fee.is_zero() {
        return Err(eyre!("base_fee must be greater than zero"));
    }
    encode_hf_shared_lease_join_provenance_payload(
        repo_id.as_str(),
        resolved_revision.as_str(),
        service_name.as_str(),
        apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        &payload.lease_asset_definition_id,
        &payload.base_fee,
    )
    .wrap_err("failed to encode hf shared-lease join signature payload tuple")
}
fn encode_hf_lease_leave_signature_payload(payload: &HfLeaseLeavePayload) -> Result<Vec<u8>> {
    let repo_id = parse_hf_repo_id_arg(&payload.repo_id)?;
    let resolved_revision = parse_hf_revision_arg(&payload.revision)?;
    let service_name = payload
        .service_name
        .as_deref()
        .map(parse_hf_service_name_arg)
        .transpose()?;
    let apartment_name = parse_hf_apartment_name_arg(payload.apartment_name.as_deref())?;
    if payload.lease_term_ms == 0 {
        return Err(eyre!("lease_term_ms must be greater than zero"));
    }
    encode_hf_shared_lease_leave_provenance_payload(
        repo_id.as_str(),
        resolved_revision.as_str(),
        payload.storage_class,
        payload.lease_term_ms,
        service_name.as_deref(),
        apartment_name.as_deref(),
    )
    .wrap_err("failed to encode hf lease leave signature payload tuple")
}
fn encode_hf_lease_renew_signature_payload(payload: &HfLeaseRenewPayload) -> Result<Vec<u8>> {
    let repo_id = parse_hf_repo_id_arg(&payload.repo_id)?;
    let resolved_revision = parse_hf_revision_arg(&payload.revision)?;
    let service_name = parse_hf_service_name_arg(&payload.service_name)?;
    let apartment_name = parse_hf_apartment_name_arg(payload.apartment_name.as_deref())?;
    if payload.lease_term_ms == 0 {
        return Err(eyre!("lease_term_ms must be greater than zero"));
    }
    if payload.base_fee.is_zero() {
        return Err(eyre!("base_fee must be greater than zero"));
    }
    encode_hf_shared_lease_renew_provenance_payload(
        repo_id.as_str(),
        resolved_revision.as_str(),
        service_name.as_str(),
        apartment_name.as_deref(),
        payload.storage_class,
        payload.lease_term_ms,
        &payload.lease_asset_definition_id,
        &payload.base_fee,
    )
    .wrap_err("failed to encode hf lease renew signature payload tuple")
}
fn encode_training_job_start_signature_payload(
    payload: &TrainingJobStartPayload,
) -> Result<Vec<u8>> {
    encode_training_job_start_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.job_id.as_str(),
        payload.worker_group_size,
        payload.target_steps,
        payload.checkpoint_interval_steps,
        payload.max_retries,
        payload.step_compute_units,
        payload.compute_budget_units,
        payload.storage_budget_bytes,
    )
    .wrap_err("failed to encode training job start signature payload tuple")
}
fn encode_training_job_checkpoint_signature_payload(
    payload: &TrainingJobCheckpointPayload,
) -> Result<Vec<u8>> {
    encode_training_job_checkpoint_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.completed_step,
        payload.checkpoint_size_bytes,
        payload.metrics_hash,
    )
    .wrap_err("failed to encode training job checkpoint signature payload tuple")
}
fn encode_training_job_retry_signature_payload(
    payload: &TrainingJobRetryPayload,
) -> Result<Vec<u8>> {
    encode_training_job_retry_provenance_payload(
        payload.service_name.as_str(),
        payload.job_id.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode training job retry signature payload tuple")
}
fn encode_model_artifact_register_signature_payload(
    payload: &ModelArtifactRegisterPayload,
) -> Result<Vec<u8>> {
    encode_model_artifact_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.training_job_id.as_str(),
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .wrap_err("failed to encode model artifact register signature payload tuple")
}
fn encode_model_weight_register_signature_payload(
    payload: &ModelWeightRegisterPayload,
) -> Result<Vec<u8>> {
    encode_model_weight_register_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.training_job_id.as_str(),
        payload.parent_version.as_deref(),
        payload.weight_artifact_hash,
        payload.dataset_ref.as_str(),
        payload.training_config_hash,
        payload.reproducibility_hash,
        payload.provenance_attestation_hash,
    )
    .wrap_err("failed to encode model weight register signature payload tuple")
}
fn encode_model_weight_promote_signature_payload(
    payload: &ModelWeightPromotePayload,
) -> Result<Vec<u8>> {
    encode_model_weight_promote_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.weight_version.as_str(),
        payload.gate_approved,
        payload.gate_report_hash,
    )
    .wrap_err("failed to encode model weight promote signature payload tuple")
}
fn encode_model_weight_rollback_signature_payload(
    payload: &ModelWeightRollbackPayload,
) -> Result<Vec<u8>> {
    encode_model_weight_rollback_provenance_payload(
        payload.service_name.as_str(),
        payload.model_name.as_str(),
        payload.target_version.as_str(),
        payload.reason.as_str(),
    )
    .wrap_err("failed to encode model weight rollback signature payload tuple")
}
fn soracloud_invocation_context() -> Result<SoracloudInvocationContext> {
    SORACLOUD_INVOCATION_CONTEXT.with(|slot| {
        slot.borrow()
            .clone()
            .ok_or_else(|| eyre!("Soracloud invocation context is not initialized"))
    })
}
fn soracloud_submission_config() -> Result<ClientConfig> {
    soracloud_invocation_context().map(|context| context.submission_config)
}
fn soracloud_fee_payment() -> Result<FeePaymentIntent> {
    soracloud_invocation_context()?
        .fee_payment
        .map_err(eyre::Report::msg)
}
fn load_soracloud_http_witness(path: &Path) -> Result<CanonicalRequestWitnessV1> {
    let mut file = fs::File::open(path)
        .wrap_err_with(|| format!("failed to open Soracloud witness file `{}`", path.display()))?;
    let file_bytes = file
        .metadata()
        .wrap_err_with(|| {
            format!(
                "failed to inspect Soracloud witness file `{}`",
                path.display()
            )
        })?
        .len();
    if file_bytes > SORACLOUD_HTTP_WITNESS_FILE_MAX_BYTES_V1 {
        return Err(eyre!(
            "Soracloud witness file `{}` exceeds the V1 limit of {SORACLOUD_HTTP_WITNESS_FILE_MAX_BYTES_V1} bytes",
            path.display()
        ));
    }
    let file_bytes = usize::try_from(file_bytes)
        .wrap_err("Soracloud witness file length exceeds platform capacity")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(file_bytes)
        .wrap_err_with(|| format!("failed to reserve {file_bytes} Soracloud witness bytes"))?;
    bytes.resize(file_bytes, 0);
    file.read_exact(&mut bytes)
        .wrap_err_with(|| format!("failed to read Soracloud witness file `{}`", path.display()))?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).wrap_err_with(|| {
        format!(
            "failed to finish Soracloud witness file `{}`",
            path.display()
        )
    })? != 0
    {
        return Err(eyre!(
            "Soracloud witness file `{}` changed while it was being read",
            path.display()
        ));
    }
    let witness: CanonicalRequestWitnessV1 = json::from_slice(&bytes).wrap_err_with(|| {
        format!(
            "failed to decode Soracloud witness file `{}`",
            path.display()
        )
    })?;
    if witness.schema_version != CANONICAL_REQUEST_WITNESS_VERSION_V1 {
        return Err(eyre!(
            "unsupported Soracloud witness schema_version `{}` in `{}`",
            witness.schema_version,
            path.display()
        ));
    }
    Ok(witness)
}
#[derive(Debug)]
struct SoracloudMutationAuth {
    headers: Vec<(&'static str, String)>,
    expected_response_signers: Vec<PublicKey>,
}
fn build_soracloud_mutation_auth_headers(
    submission_config: &ClientConfig,
    http_witness_file: Option<&Path>,
    endpoint: &reqwest::Url,
    body: &[u8],
) -> Result<SoracloudMutationAuth> {
    build_soracloud_mutation_auth_headers_with_rng(
        submission_config,
        http_witness_file,
        endpoint,
        body,
        &mut OsRng,
    )
}
fn build_soracloud_mutation_auth_headers_with_rng<R: TryCryptoRng>(
    submission_config: &ClientConfig,
    http_witness_file: Option<&Path>,
    endpoint: &reqwest::Url,
    body: &[u8],
    rng: &mut R,
) -> Result<SoracloudMutationAuth> {
    if let Some(witness_file) = http_witness_file {
        let witness = load_soracloud_http_witness(witness_file)?;
        if witness.subject_account != submission_config.account {
            return Err(eyre!(
                "Soracloud witness subject_account `{}` does not match configured account `{}`",
                witness.subject_account,
                submission_config.account
            ));
        }
        let expected_hash = canonical_network_request_hash(
            &submission_config.network_id,
            &reqwest::Method::POST,
            endpoint,
            body,
        )?;
        if witness.canonical_request_hash != expected_hash {
            return Err(eyre!(
                "Soracloud witness canonical_request_hash does not match the POST {} request",
                endpoint.path()
            ));
        }
        let witness_header = canonical_request_witness_header_value(&witness)
            .wrap_err("failed to encode Soracloud witness header")?;
        if witness.signatures.is_empty() {
            return Err(eyre!(
                "Soracloud canonical request witness must contain at least one signature"
            ));
        }
        let expected_response_signers = witness
            .signatures
            .iter()
            .map(|signature| signature.signer.clone())
            .collect();
        let mut headers = Vec::new();
        headers
            .try_reserve_exact(1)
            .wrap_err("failed to reserve Soracloud witness header")?;
        headers.push((HEADER_IROHA_WITNESS, witness_header));
        return Ok(SoracloudMutationAuth {
            headers,
            expected_response_signers,
        });
    }
    let expected_response_signers = vec![submission_config.key_pair.public_key().clone()];
    let headers = build_soracloud_signature_auth_headers_with_rng(
        submission_config,
        "POST",
        endpoint,
        body,
        rng,
    )?;
    Ok(SoracloudMutationAuth {
        headers,
        expected_response_signers,
    })
}
fn build_soracloud_read_auth_headers(
    submission_config: &ClientConfig,
    endpoint: &reqwest::Url,
) -> Result<Vec<(&'static str, String)>> {
    build_soracloud_signature_auth_headers_with_rng(
        submission_config,
        "GET",
        endpoint,
        &[],
        &mut OsRng,
    )
}
fn soracloud_signature_timestamp_ms_from_elapsed(elapsed: Duration) -> Result<u64> {
    u64::try_from(elapsed.as_millis())
        .wrap_err("Soracloud request signature timestamp exceeds u64 milliseconds")
}
fn soracloud_signature_timestamp_ms(now: SystemTime) -> Result<u64> {
    let elapsed = now
        .duration_since(UNIX_EPOCH)
        .wrap_err("Soracloud request signature clock precedes the Unix epoch")?;
    soracloud_signature_timestamp_ms_from_elapsed(elapsed)
}
fn build_soracloud_signature_auth_headers_with_rng<R: TryCryptoRng>(
    submission_config: &ClientConfig,
    method: &str,
    endpoint: &reqwest::Url,
    body: &[u8],
    rng: &mut R,
) -> Result<Vec<(&'static str, String)>> {
    let timestamp_ms = soracloud_signature_timestamp_ms(SystemTime::now())?;
    let mut nonce_bytes = [0_u8; 16];
    rng.try_fill_bytes(&mut nonce_bytes)
        .map_err(|error| eyre!("Soracloud request signature nonce OS RNG failed: {error}"))?;
    let nonce_encoded_bytes = nonce_bytes
        .len()
        .checked_mul(2)
        .ok_or_else(|| eyre!("Soracloud request signature nonce length exceeds capacity"))?;
    let mut nonce_encoded = Vec::new();
    nonce_encoded
        .try_reserve_exact(nonce_encoded_bytes)
        .wrap_err("failed to reserve Soracloud request signature nonce")?;
    nonce_encoded.resize(nonce_encoded_bytes, 0);
    hex::encode_to_slice(nonce_bytes, &mut nonce_encoded)
        .wrap_err("failed to encode Soracloud request signature nonce")?;
    let nonce = String::from_utf8(nonce_encoded)
        .wrap_err("Soracloud request signature nonce is not UTF-8")?;
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .wrap_err("invalid Soracloud canonical request method")?;
    let message = canonical_network_request_signature_message(
        &submission_config.network_id,
        &method,
        endpoint,
        body,
        timestamp_ms,
        &nonce,
    )?;
    let signature = sign_soracloud_payload(&submission_config.key_pair, &message)?;
    let account = canonical_request_account_header_value(&submission_config.account)?;
    let signature = canonical_request_signature_header_value(&signature)?;
    let timestamp = canonical_request_timestamp_header_value(timestamp_ms)?;
    let mut headers = Vec::new();
    headers
        .try_reserve_exact(4)
        .wrap_err("failed to reserve Soracloud canonical auth headers")?;
    headers.push((HEADER_IROHA_ACCOUNT, account));
    headers.push((HEADER_IROHA_SIGNATURE, signature));
    headers.push((HEADER_IROHA_TIMESTAMP_MS, timestamp));
    headers.push((HEADER_IROHA_NONCE, nonce));
    Ok(headers)
}
const SORACLOUD_MUTATION_SUBMISSION_RECEIPT_VERSION_V1: u16 = 1;
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SoracloudMutationSubmissionReceiptV1 {
    schema_version: u16,
    authority: AccountId,
    signed_by: PublicKey,
    instruction_count: NonZeroU32,
    submitted_tx_hash: Hash,
}
impl SoracloudMutationSubmissionReceiptV1 {
    fn validate(&self) -> Result<()> {
        if self.schema_version != SORACLOUD_MUTATION_SUBMISSION_RECEIPT_VERSION_V1 {
            return Err(eyre!(
                "unsupported Soracloud mutation submission receipt schema version {}; expected {}",
                self.schema_version,
                SORACLOUD_MUTATION_SUBMISSION_RECEIPT_VERSION_V1
            ));
        }
        Ok(())
    }
}
fn decode_soracloud_submission_receipt(
    payload: &json::Value,
) -> Result<SoracloudMutationSubmissionReceiptV1> {
    let receipt: SoracloudMutationSubmissionReceiptV1 = json::from_value(payload.clone())
        .wrap_err("failed to decode exact Soracloud mutation submission receipt")?;
    receipt.validate()?;
    Ok(receipt)
}
fn decode_soracloud_tx_instructions(
    draft: &SoracloudMutationDraftResponse,
) -> Result<Vec<InstructionBox>> {
    draft
        .validate()
        .map_err(|error| eyre!("invalid Soracloud mutation draft response: {error}"))?;
    let mut decoded = Vec::with_capacity(draft.tx_instructions.len());
    for entry in &draft.tx_instructions {
        let payload_bytes = hex::decode(&entry.payload_hex)
            .wrap_err("failed to decode Soracloud tx instruction hex payload")?;
        let instruction = decode_instruction_from_pair(&entry.wire_id, &payload_bytes)
            .wrap_err("failed to decode Soracloud instruction skeleton")?;
        decoded.push(instruction);
    }
    Ok(decoded)
}
/// One canonical signed transaction prepared for durable, byte-identical replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedSoracloudTransactionV1 {
    /// Closed operation label bound by the caller's durable mutation plan.
    pub operation: String,
    /// Exact fixed-V1 Norito bytes submitted to Torii.
    pub wire: Vec<u8>,
    /// Canonical lowercase hexadecimal Iroha hash of the signed transaction.
    pub tx_hash_hex: String,
    /// Exact signature-bound fee intent carried by `wire`.
    pub fee_payment: FeePaymentIntent,
    /// Exact Torii quote whose intent was signed into `wire`.
    pub fee_quote: FeeQuoteResponse,
    /// Reset authorization and mutation identity duplicated in transaction metadata.
    pub binding: TairaMutationBindingV1,
}

/// Exact public-reset identity signed into every prepared ledger transaction.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(crate) struct TairaMutationBindingV1 {
    pub authorization_sha256: String,
    pub authorization_nonce: String,
    pub kind: String,
    pub phase: String,
    pub idempotency_key: String,
    pub execution_expires_at_unix_ms: u64,
}

impl PreparedSoracloudTransactionV1 {
    fn from_signed(
        operation: &str,
        binding: TairaMutationBindingV1,
        fee_quote: FeeQuoteResponse,
        transaction: SignedTransaction,
    ) -> Result<Self> {
        validate_prepared_soracloud_operation(operation)?;
        binding.validate()?;
        transaction
            .verify_signature()
            .wrap_err("prepared Soracloud transaction signature is invalid")?;
        let wire = transaction
            .encode_wire_v1()
            .wrap_err("failed to encode canonical prepared Soracloud transaction")?;
        let prepared = Self {
            operation: operation.to_owned(),
            tx_hash_hex: hex::encode(transaction.hash().as_ref()),
            fee_payment: transaction.fee_payment_intent().clone(),
            fee_quote,
            binding,
            wire,
        };
        prepared.decode_and_validate()?;
        Ok(prepared)
    }

    pub(crate) fn decode_and_validate(&self) -> Result<SignedTransaction> {
        validate_prepared_soracloud_operation(&self.operation)?;
        if self.tx_hash_hex.len() != 64
            || !self
                .tx_hash_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || !self.tx_hash_hex.as_bytes().last().is_some_and(|byte| {
                matches!(byte, b'1' | b'3' | b'5' | b'7' | b'9' | b'b' | b'd' | b'f')
            })
        {
            return Err(eyre!(
                "prepared Soracloud transaction hash must be exactly 64 lowercase hexadecimal characters with the Iroha hash marker set"
            ));
        }
        let transaction = SignedTransaction::decode_all_versioned(&self.wire)
            .wrap_err("failed to decode exact prepared Soracloud transaction wire")?;
        transaction
            .verify_signature()
            .wrap_err("prepared Soracloud transaction signature is invalid")?;
        let canonical = transaction
            .encode_wire_v1()
            .wrap_err("failed to re-encode prepared Soracloud transaction")?;
        if canonical != self.wire {
            return Err(eyre!(
                "prepared Soracloud transaction wire is not canonical fixed-V1 Norito"
            ));
        }
        let actual_hash = hex::encode(transaction.hash().as_ref());
        if actual_hash != self.tx_hash_hex {
            return Err(eyre!(
                "prepared Soracloud transaction hash does not match its exact wire bytes"
            ));
        }

        if transaction.fee_payment_intent() != &self.fee_payment {
            return Err(eyre!(
                "prepared Soracloud transaction fee identity does not match its exact wire bytes"
            ));
        }
        if self.fee_quote.intent != self.fee_payment {
            return Err(eyre!(
                "prepared Soracloud fee quote does not match the signed fee identity"
            ));
        }
        self.fee_quote
            .validate_for_signed_payload(transaction.payload())
            .map_err(|error| {
                eyre!("prepared Soracloud fee quote is semantically invalid: {error}")
            })?;
        let expected_metadata = self.binding.metadata(&self.operation)?;
        if transaction.metadata() != &expected_metadata {
            return Err(eyre!(
                "prepared Soracloud transaction metadata does not exactly bind the reset authorization and mutation"
            ));
        }
        Ok(transaction)
    }
}

impl TairaMutationBindingV1 {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.authorization_sha256.len() != 64
            || !self
                .authorization_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.idempotency_key.len() != 64
            || !self
                .idempotency_key
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.authorization_nonce.len() != 32
            || !self.authorization_nonce.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
            || self.kind.is_empty()
            || self.phase.is_empty()
            || !self.kind.bytes().chain(self.phase.bytes()).all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            })
            || self.execution_expires_at_unix_ms == 0
        {
            return Err(eyre!(
                "Taira mutation binding is outside the exact first-release grammar"
            ));
        }
        Ok(())
    }

    fn metadata(&self, operation: &str) -> Result<Metadata> {
        self.validate()?;
        validate_prepared_soracloud_operation(operation)?;
        let binding_value = json::to_value(self)
            .wrap_err("failed to encode exact Taira mutation binding metadata")?;
        let binding_json = Json::from_norito_value_ref(&binding_value)
            .wrap_err("failed to canonicalize exact Taira mutation binding metadata")?;
        let entries = [
            ("taira_public_reset_binding", binding_json),
            (
                "taira_public_reset_authorization_sha256",
                Json::new(self.authorization_sha256.clone()),
            ),
            (
                "taira_public_reset_authorization_nonce",
                Json::new(self.authorization_nonce.clone()),
            ),
            (
                "taira_public_reset_mutation_kind",
                Json::new(self.kind.clone()),
            ),
            (
                "taira_public_reset_mutation_phase",
                Json::new(self.phase.clone()),
            ),
            (
                "taira_public_reset_idempotency_key",
                Json::new(self.idempotency_key.clone()),
            ),
            (
                "taira_public_reset_execution_expires_at_unix_ms",
                Json::new(self.execution_expires_at_unix_ms),
            ),
            (
                "taira_public_reset_mutation_operation",
                Json::new(operation.to_owned()),
            ),
        ];
        let mut metadata = Metadata::default();
        for (key, value) in entries {
            let key: Name = key
                .parse()
                .wrap_err("Taira mutation metadata key is invalid")?;
            if metadata.insert(key, value).is_some() {
                return Err(eyre!("duplicate Taira mutation metadata key"));
            }
        }
        Ok(metadata)
    }
}

fn validate_prepared_soracloud_operation(operation: &str) -> Result<()> {
    match operation {
        "bundle_pin" | "guest_pin" | "discovery_pin" | "service_mutation" => Ok(()),
        _ => Err(eyre!(
            "prepared Soracloud transaction operation must be bundle_pin, guest_pin, discovery_pin, or service_mutation"
        )),
    }
}

fn soracloud_transaction_client(
    config: &ClientConfig,
    torii_url: &str,
    timeout_secs: u64,
) -> Result<Client> {
    let mut config = config.clone();
    config.torii_api_url = url::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?;
    config.torii_request_timeout = Duration::from_secs(timeout_secs.max(1));
    Client::new(config)
}

/// Quote and sign one exact Soracloud draft without submitting it.
pub(crate) fn prepare_soracloud_draft_transaction(
    config: &ClientConfig,
    requested_fee_payment: FeePaymentIntent,
    binding: TairaMutationBindingV1,
    torii_url: &str,
    timeout_secs: u64,
    instructions: Vec<InstructionBox>,
    operation: &str,
) -> Result<PreparedSoracloudTransactionV1> {
    if instructions.is_empty() {
        return Err(eyre!(
            "Soracloud mutation draft must contain at least one transaction instruction"
        ));
    }
    let client = soracloud_transaction_client(config, torii_url, timeout_secs)?;
    let executable = Executable::Instructions(instructions.into());
    let mut payload = client
        .account_client()
        .prepare_transaction(iroha::client::AccountTransactionDraft::new(
            executable,
            requested_fee_payment.clone(),
            binding.metadata(operation)?,
        ))
        .wrap_err("failed to build exact unsigned Soracloud mutation payload")?;
    let quote = client
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .wrap_err("failed to quote exact Soracloud mutation fees")?;
    if !requested_fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        return Err(eyre!(
            "fee quote changed the selected payer, sponsor revision, or gas bound"
        ));
    }
    quote
        .intent
        .validate()
        .wrap_err("fee quote returned an invalid Soracloud payment intent")?;
    payload.fee_payment = quote.intent.clone();
    let transaction = client
        .account_client()
        .sign_transaction(payload)
        .wrap_err("failed to sign the exact quoted Soracloud mutation payload")?;
    PreparedSoracloudTransactionV1::from_signed(operation, binding, quote, transaction)
}

/// Submit one already prepared canonical Soracloud transaction verbatim.
pub(crate) fn submit_prepared_soracloud_transaction(
    config: &ClientConfig,
    torii_url: &str,
    deadline: Instant,
    prepared: &PreparedSoracloudTransactionV1,
) -> Result<Hash> {
    let transaction = prepared.decode_and_validate()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if (remaining / 2).is_zero() {
        return Err(eyre!("prepared Soracloud submission deadline is exhausted"));
    }
    let mut bounded_config = config.clone();
    bounded_config.torii_api_url = url::Url::parse(torii_url)?;
    // Compatibility and one exact POST share the existing caller budget.
    bounded_config.torii_request_timeout = remaining / 2;
    let client = Client::new(bounded_config)?;
    client
        .submit_transaction(&transaction)
        .map(Into::into)
        .wrap_err("failed to submit exact prepared Soracloud mutation transaction")
}

/// Read-only finality classification of one exact prepared Soracloud transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreparedSoracloudRecoveryV1 {
    /// The exact transaction hash is not yet globally observable.
    Absent,
    /// The exact transaction is globally state-resolved `Applied` and its canonical wire is proven.
    Applied {
        /// Positive committed block height.
        block_height: u64,
        /// Lowercase transaction hash proving the committed exact envelope.
        evidence_sha256: String,
    },
    /// The exact transaction exists but is not terminal.
    Pending {
        /// Canonical pipeline state.
        terminal_kind: String,
    },
    /// The exact transaction reached a terminal non-Applied state.
    Rejected {
        /// Canonical rejection or expiry class.
        terminal_kind: String,
    },
}

/// Classify one exact prepared Soracloud transaction without submitting it.
///
/// Applied classification includes the authenticated exact transaction-details route;
/// it never requests global transaction inventory.
pub(crate) fn recover_prepared_soracloud_transaction(
    config: &ClientConfig,
    torii_url: &str,
    deadline: Instant,
    prepared: &PreparedSoracloudTransactionV1,
) -> Result<PreparedSoracloudRecoveryV1> {
    let expected = prepared.decode_and_validate()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    let request_budget = remaining / 3;
    if request_budget.is_zero() {
        return Ok(PreparedSoracloudRecoveryV1::Pending {
            terminal_kind: "ObservationBudgetExhausted".to_owned(),
        });
    }
    let mut bounded_config = config.clone();
    bounded_config.torii_api_url = url::Url::parse(torii_url)?;
    bounded_config.torii_request_timeout = request_budget;
    let client = Client::new(bounded_config)?;
    let Some(status) = client
        .client()
        .get_transaction_status_response_global(expected.hash())
        .wrap_err("read-only exact Soracloud transaction status lookup failed")?
    else {
        return Ok(PreparedSoracloudRecoveryV1::Absent);
    };
    if status.hash != prepared.tx_hash_hex || status.scope != "global" {
        return Err(eyre!(
            "Soracloud transaction status differs from the exact prepared identity"
        ));
    }
    match status.status.kind.as_str() {
        "Applied" if soracloud_status_is_final_applied(&status) => {
            let block_height = status
                .status
                .block_height
                .filter(|height| *height > 0)
                .ok_or_else(|| eyre!("Applied Soracloud transaction omits its block height"))?;
            match verify_committed_prepared_soracloud_transaction(&client, prepared, &expected) {
                Ok(()) => {}
                Err(error) if crate::taira::exact_transaction_details_not_found(&error) => {
                    return Ok(PreparedSoracloudRecoveryV1::Pending {
                        terminal_kind: "AppliedEvidencePending".to_owned(),
                    });
                }
                Err(error) => return Err(error),
            }
            Ok(PreparedSoracloudRecoveryV1::Applied {
                block_height,
                evidence_sha256: prepared.tx_hash_hex.clone(),
            })
        }
        "Rejected" | "Expired" if soracloud_status_is_final_failure(&status) => {
            Ok(PreparedSoracloudRecoveryV1::Rejected {
                terminal_kind: status.status.kind,
            })
        }
        "Queued" | "Approved" | "Committed" | "Applied" | "Rejected" | "Expired" => {
            Ok(PreparedSoracloudRecoveryV1::Pending {
                terminal_kind: status.status.kind,
            })
        }
        other => Err(eyre!(
            "Soracloud transaction status has unsupported kind `{other}`"
        )),
    }
}

fn soracloud_status_is_final_applied(status: &PipelineTransactionStatusResponse) -> bool {
    status.status.kind == "Applied" && status.scope == "global" && status.resolved_from == "state"
}

fn soracloud_status_is_final_failure(status: &PipelineTransactionStatusResponse) -> bool {
    matches!(status.status.kind.as_str(), "Rejected" | "Expired")
        && status.scope == "global"
        && status.resolved_from == "state"
}

fn verify_committed_prepared_soracloud_transaction(
    client: &Client,
    prepared: &PreparedSoracloudTransactionV1,
    expected: &SignedTransaction,
) -> Result<()> {
    let entrypoint_hash = expected.hash_as_entrypoint();
    let details = client
        .client()
        .get_transaction_details(entrypoint_hash)
        .wrap_err("exact committed Soracloud transaction proof query failed")?;
    let committed = &details.transaction;
    if committed.result().is_err() {
        return Err(eyre!(
            "Applied Soracloud status resolves to a failed committed transaction"
        ));
    }
    let TransactionEntrypoint::External(transaction) = committed.entrypoint() else {
        return Err(eyre!(
            "Applied Soracloud status resolves to a non-external entrypoint"
        ));
    };
    let wire = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to encode committed Soracloud transaction: {error}"))?;
    if wire != prepared.wire
        || transaction.hash() != expected.hash()
        || transaction.hash_as_entrypoint() != entrypoint_hash
        || transaction.metadata() != expected.metadata()
    {
        return Err(eyre!(
            "committed Soracloud transaction differs from the prepared exact envelope"
        ));
    }
    Ok(())
}

fn submit_soracloud_draft_transaction(
    torii_url: &str,
    timeout_secs: u64,
    instructions: Vec<InstructionBox>,
) -> Result<Hash> {
    #[cfg(test)]
    if let Some(hash) = SORACLOUD_TEST_SUBMITTED_TX_HASH.with(|slot| *slot.borrow()) {
        return Ok(hash);
    }
    let config = soracloud_submission_config()?;
    let client = soracloud_transaction_client(&config, torii_url, timeout_secs)?;
    let requested_fee_payment = soracloud_fee_payment()?;
    let executable = Executable::Instructions(instructions.into());
    let mut payload = client
        .account_client()
        .prepare_transaction(iroha::client::AccountTransactionDraft::new(
            executable,
            requested_fee_payment.clone(),
            Metadata::default(),
        ))
        .wrap_err("failed to build exact unsigned Soracloud mutation payload")?;
    let quote = client
        .quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload })
        .wrap_err("failed to quote exact Soracloud mutation fees")?;
    if !requested_fee_payment.has_same_payer_and_gas_bound(&quote.intent) {
        return Err(eyre!(
            "fee quote changed the selected payer, sponsor revision, or gas bound"
        ));
    }
    quote
        .intent
        .validate()
        .wrap_err("fee quote returned an invalid Soracloud payment intent")?;
    payload.fee_payment = quote.intent;
    let transaction = client
        .account_client()
        .sign_transaction(payload)
        .wrap_err("failed to sign the exact quoted Soracloud mutation payload")?;
    client
        .submit_transaction_and_wait(&transaction)
        .map(Into::into)
        .wrap_err("failed to submit Soracloud mutation transaction")
}
fn decode_exact_torii_json_success(
    status: reqwest::StatusCode,
    content_type: Option<&HeaderValue>,
    body: &[u8],
    response_context: &str,
) -> Result<json::Value> {
    if status != reqwest::StatusCode::OK {
        return Err(eyre!(
            "{response_context} returned HTTP {status}; expected exactly 200 OK: {}",
            String::from_utf8_lossy(body)
        ));
    }
    let content_type = content_type.ok_or_else(|| {
        eyre!("{response_context} 200 OK response is missing Content-Type: application/json")
    })?;
    if content_type.as_bytes() != b"application/json" {
        return Err(eyre!(
            "{response_context} 200 OK response has Content-Type `{}`; expected exactly `application/json`",
            String::from_utf8_lossy(content_type.as_bytes())
        ));
    }
    if body.is_empty() {
        return Err(eyre!(
            "{response_context} 200 OK response has an empty JSON body"
        ));
    }
    json::from_slice(body)
        .wrap_err_with(|| format!("failed to decode exact {response_context} JSON body"))
}
struct RequestedSoracloudMutationDraft {
    endpoint: String,
    draft: SoracloudMutationDraftResponse,
    instruction_count: NonZeroU32,
}

fn request_torii_soracloud_mutation_draft<T>(
    torii_url: &str,
    endpoint_path: &str,
    request_payload: &T,
    submission_config: &ClientConfig,
    http_witness_file: Option<&Path>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<RequestedSoracloudMutationDraft>
where
    T: JsonSerialize + ?Sized,
{
    let endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join(endpoint_path)
        .wrap_err_with(|| format!("failed to derive /{endpoint_path} URL from --torii-url"))?;
    let body = json::to_vec(request_payload)
        .wrap_err("failed to encode soracloud mutation request payload")?;
    let auth = build_soracloud_mutation_auth_headers(
        submission_config,
        http_witness_file,
        &endpoint,
        &body,
    )?;
    let timeout = Duration::from_secs(timeout_secs.max(1));
    let client = BlockingHttpClient::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .wrap_err("failed to build HTTP client for soracloud mutation")?;
    let mut request = client
        .post(endpoint.clone())
        .header(header::ACCEPT, HeaderValue::from_static("application/json"))
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )
        .body(body);
    for (name, value) in auth.headers {
        request = request.header(name, value);
    }
    if let Some(token) = api_token {
        request = request.header("x-api-token", token);
    }
    let response = request
        .send()
        .wrap_err_with(|| format!("failed to call `{}`", endpoint.as_str()))?;
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let body = response
        .bytes()
        .wrap_err("failed to read Torii mutation response body")?;
    let payload = decode_exact_torii_json_success(
        status,
        content_type.as_ref(),
        &body,
        &format!("Torii /{endpoint_path} mutation draft"),
    )?;
    let draft: SoracloudMutationDraftResponse = json::from_value(payload)
        .wrap_err("failed to decode exact Torii Soracloud mutation draft response")?;
    draft
        .validate()
        .wrap_err("Torii returned an invalid Soracloud mutation draft response")?;
    if draft.authority != submission_config.account {
        return Err(eyre!(
            "Torii Soracloud mutation draft authority `{}` does not match configured transaction authority `{}`",
            draft.authority,
            submission_config.account
        ));
    }
    if !auth
        .expected_response_signers
        .iter()
        .any(|signer| signer == &draft.signed_by)
    {
        return Err(eyre!(
            "Torii Soracloud mutation draft signer `{}` is not one of the authenticated request signers",
            draft.signed_by
        ));
    }
    let instruction_count = NonZeroU32::new(
        u32::try_from(draft.tx_instructions.len())
            .wrap_err("Soracloud mutation draft instruction count exceeds the V1 range")?,
    )
    .ok_or_else(|| eyre!("Soracloud mutation draft must contain at least one instruction"))?;
    Ok(RequestedSoracloudMutationDraft {
        endpoint: endpoint.to_string(),
        draft,
        instruction_count,
    })
}

fn post_torii_soracloud_mutation<T>(
    torii_url: &str,
    endpoint_path: &str,
    request_payload: &T,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)>
where
    T: JsonSerialize + ?Sized,
{
    let invocation = soracloud_invocation_context()?;
    let requested = request_torii_soracloud_mutation_draft(
        torii_url,
        endpoint_path,
        request_payload,
        &invocation.submission_config,
        invocation.http_witness_file.as_deref(),
        api_token,
        timeout_secs,
    )?;
    let instructions = decode_soracloud_tx_instructions(&requested.draft)?;
    let submitted_tx_hash =
        submit_soracloud_draft_transaction(torii_url, timeout_secs, instructions)?;
    let receipt = SoracloudMutationSubmissionReceiptV1 {
        schema_version: SORACLOUD_MUTATION_SUBMISSION_RECEIPT_VERSION_V1,
        authority: requested.draft.authority,
        signed_by: requested.draft.signed_by,
        instruction_count: requested.instruction_count,
        submitted_tx_hash,
    };
    Ok((
        requested.endpoint,
        json::to_value(&receipt).wrap_err("failed to encode Soracloud submission receipt")?,
    ))
}
fn merge_submission_metadata(
    target: &mut json::Value,
    mutation_payload: &json::Value,
) -> Result<()> {
    let Some(root) = target.as_object_mut() else {
        return Err(eyre!(
            "expected JSON object when merging Soracloud submission metadata"
        ));
    };
    let receipt = decode_soracloud_submission_receipt(mutation_payload)?;
    root.insert(
        "submission".to_owned(),
        json::to_value(&receipt).wrap_err("failed to encode Soracloud submission receipt")?,
    );
    Ok(())
}
fn control_plane_service_from_status(
    payload: &json::Value,
    service_name: &str,
) -> Result<ServiceStatusOutput> {
    let (_, snapshot) = decode_network_control_plane_snapshot(payload)?;
    let mut matches = snapshot
        .services
        .into_iter()
        .filter(|service| service.service_name == service_name);
    let service = matches.next().ok_or_else(|| {
        eyre!("service `{service_name}` not found in authoritative Soracloud control-plane status")
    })?;
    if matches.next().is_some() {
        return Err(eyre!(
            "authoritative Soracloud control-plane status contains duplicate service `{service_name}`"
        ));
    }
    Ok(service)
}
fn agent_apartment_from_status(
    payload: &json::Value,
    apartment_name: &str,
) -> Result<AgentApartmentStatusEntryV1> {
    let expected = parse_exact_name_arg("apartment status name", apartment_name)?
        .parse::<Name>()
        .wrap_err("invalid apartment name for authoritative Soracloud agent status lookup")?;
    let status = decode_agent_status(payload)?;
    let mut matches = status
        .apartments
        .into_iter()
        .filter(|apartment| apartment.apartment_name == expected);
    let apartment = matches.next().ok_or_else(|| {
        eyre!("apartment `{apartment_name}` not found in authoritative Soracloud agent status")
    })?;
    if matches.next().is_some() {
        return Err(eyre!(
            "authoritative Soracloud agent status contains duplicate apartment `{apartment_name}`"
        ));
    }
    Ok(apartment)
}

fn mailbox_message_ids(payload: &json::Value) -> Result<BTreeSet<String>> {
    Ok(decode_agent_mailbox_status(payload)?
        .messages
        .into_iter()
        .map(|message| message.message_id)
        .collect())
}

fn mailbox_message_by_id(
    payload: &json::Value,
    message_id: &str,
) -> Result<AgentMailboxMessageEntryV1> {
    let status = decode_agent_mailbox_status(payload)?;
    let mut matches = status
        .messages
        .into_iter()
        .filter(|message| message.message_id == message_id);
    let message = matches.next().ok_or_else(|| {
        eyre!("mailbox message `{message_id}` was not found in authoritative status")
    })?;
    if matches.next().is_some() {
        return Err(eyre!(
            "authoritative mailbox status contains duplicate message_id `{message_id}`"
        ));
    }
    Ok(message)
}

fn new_mailbox_message_from_status(
    payload: &json::Value,
    known_message_ids: &BTreeSet<String>,
    from_apartment: &str,
    channel: &str,
    message_payload: &str,
) -> Result<AgentMailboxMessageEntryV1> {
    let expected_from = parse_exact_name_arg("sender apartment name", from_apartment)?
        .parse::<Name>()
        .wrap_err("invalid sender apartment name for mailbox status lookup")?;
    let status = decode_agent_mailbox_status(payload)?;
    let mut matches = status.messages.into_iter().filter(|message| {
        !known_message_ids.contains(&message.message_id)
            && message.from_apartment == expected_from
            && message.channel == channel
            && message.payload == message_payload
    });
    let message = matches.next().ok_or_else(|| {
        eyre!(
            "authoritative mailbox status did not return one newly enqueued message from `{from_apartment}` on channel `{channel}`"
        )
    })?;
    if let Some(ambiguous) = matches.next() {
        return Err(eyre!(
            "authoritative mailbox status returned multiple newly enqueued matching messages (`{}` and `{}`)",
            message.message_id,
            ambiguous.message_id
        ));
    }
    Ok(message)
}
fn build_service_mutation_output(
    mutation_payload: json::Value,
    status_payload: &json::Value,
    service_name: &str,
    action_label: &str,
) -> Result<json::Value> {
    let service = control_plane_service_from_status(status_payload, service_name)?;
    let mut output = json::to_value(&service).wrap_err("encode exact service status output")?;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("service snapshot must be a JSON object"));
    };
    root.insert(
        "action".to_owned(),
        json::Value::String(action_label.to_owned()),
    );
    merge_submission_metadata(&mut output, &mutation_payload)?;
    Ok(output)
}
fn build_agent_mutation_output(
    mutation_payload: json::Value,
    status_payload: &json::Value,
    apartment_name: &str,
    action_label: &str,
) -> Result<json::Value> {
    let apartment = agent_apartment_from_status(status_payload, apartment_name)?;
    let mut output =
        json::to_value(&apartment).wrap_err("failed to encode exact agent apartment status V1")?;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("agent apartment snapshot must be a JSON object"));
    };
    root.insert(
        "action".to_owned(),
        json::Value::String(action_label.to_owned()),
    );
    merge_submission_metadata(&mut output, &mutation_payload)?;
    Ok(output)
}
fn build_hf_mutation_output(
    mutation_payload: json::Value,
    status_payload: &json::Value,
    action_label: &str,
) -> Result<json::Value> {
    let status = decode_hf_shared_lease_status(status_payload)?;
    let mut output = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud HF shared-lease status V1")?;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("hf shared-lease status must be a JSON object"));
    };
    root.insert(
        "action".to_owned(),
        json::Value::String(action_label.to_owned()),
    );
    merge_submission_metadata(&mut output, &mutation_payload)?;
    Ok(output)
}
fn build_wallet_spend_output(
    mutation_payload: json::Value,
    status_payload: &json::Value,
    apartment_name: &str,
    request_id: &str,
) -> Result<json::Value> {
    parse_agent_wallet_request_id(request_id).map_err(|error| eyre!("request_id {error}"))?;
    let mut output = build_agent_mutation_output(
        mutation_payload,
        status_payload,
        apartment_name,
        "WalletSpendSubmitted",
    )?;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("wallet spend output must be a JSON object"));
    };
    root.insert(
        "request_id".to_owned(),
        json::Value::String(request_id.to_owned()),
    );
    Ok(output)
}
fn build_message_send_output(
    mutation_payload: json::Value,
    mailbox_status_payload: &json::Value,
    known_message_ids: &BTreeSet<String>,
    from_apartment: &str,
    channel: &str,
    message_payload: &str,
) -> Result<json::Value> {
    let mailbox_status = decode_agent_mailbox_status(mailbox_status_payload)?;
    let mut output = json::to_value(&mailbox_status)
        .wrap_err("failed to encode exact Soracloud agent mailbox status V1")?;
    let message_id = new_mailbox_message_from_status(
        mailbox_status_payload,
        known_message_ids,
        from_apartment,
        channel,
        message_payload,
    )?
    .message_id;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("mailbox status must be a JSON object"));
    };
    root.insert(
        "action".to_owned(),
        json::Value::String("MessageEnqueued".to_owned()),
    );
    root.insert("message_id".to_owned(), json::Value::String(message_id));
    merge_submission_metadata(&mut output, &mutation_payload)?;
    Ok(output)
}
fn build_message_ack_output(
    mutation_payload: json::Value,
    mailbox_status_before: &json::Value,
    mailbox_status_payload: &json::Value,
    message_id: &str,
) -> Result<json::Value> {
    mailbox_message_by_id(mailbox_status_before, message_id)?;
    let status_after = decode_agent_mailbox_status(mailbox_status_payload)?;
    if status_after
        .messages
        .iter()
        .any(|message| message.message_id == message_id)
    {
        return Err(eyre!(
            "authoritative mailbox status still contains acknowledged message `{message_id}`"
        ));
    }
    let mut output = json::to_value(&status_after)
        .wrap_err("failed to encode exact Soracloud agent mailbox status V1")?;
    let Some(root) = output.as_object_mut() else {
        return Err(eyre!("mailbox status must be a JSON object"));
    };
    root.insert(
        "action".to_owned(),
        json::Value::String("MessageAcknowledged".to_owned()),
    );
    root.insert(
        "message_id".to_owned(),
        json::Value::String(message_id.to_owned()),
    );
    merge_submission_metadata(&mut output, &mutation_payload)?;
    Ok(output)
}
fn send_torii_soracloud_authenticated_get(
    endpoint: reqwest::Url,
    api_token: Option<&str>,
    timeout_secs: u64,
    response_context: &'static str,
) -> Result<(
    reqwest::Url,
    reqwest::StatusCode,
    Option<HeaderValue>,
    Vec<u8>,
)> {
    let submission_config = soracloud_submission_config()
        .wrap_err("Soracloud protected GET requires an initialized local account signer")?;
    let auth_headers = build_soracloud_read_auth_headers(&submission_config, &endpoint)?;
    let client = BlockingHttpClient::builder()
        .timeout(Duration::from_secs(timeout_secs.max(1)))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .wrap_err("failed to build exact-auth Soracloud HTTP client")?;
    let mut request = client
        .get(endpoint.clone())
        .header(header::ACCEPT, HeaderValue::from_static("application/json"));
    for (name, value) in auth_headers {
        request = request.header(name, value);
    }
    if let Some(token) = api_token {
        request = request.header("x-api-token", token);
    }
    let response = request
        .send()
        .wrap_err_with(|| format!("failed to fetch `{}`", endpoint.as_str()))?;
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let body = response
        .bytes()
        .wrap_err_with(|| format!("failed to read {response_context} response body"))?
        .to_vec();
    Ok((endpoint, status, content_type, body))
}
fn fetch_torii_soracloud_authenticated_json(
    endpoint: reqwest::Url,
    api_token: Option<&str>,
    timeout_secs: u64,
    response_context: &'static str,
) -> Result<(String, norito::json::Value)> {
    let (endpoint, status, content_type, body) = send_torii_soracloud_authenticated_get(
        endpoint,
        api_token,
        timeout_secs,
        response_context,
    )?;
    let payload =
        decode_exact_torii_json_success(status, content_type.as_ref(), &body, response_context)?;
    Ok((endpoint.to_string(), payload))
}
fn fetch_torii_soracloud_status(
    torii_url: &str,
    service_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/status")
        .wrap_err("failed to derive /v1/soracloud/status URL from --torii-url")?;
    let (endpoint, mut payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud status",
    )?;
    filter_soracloud_status_payload(&mut payload, service_name)?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_app_infra_status(
    torii_url: &str,
    app_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?;
    let app_name = app_name
        .map(|name| parse_exact_name_arg("--app-name", name))
        .transpose()?;
    if let Some(app_name) = app_name.as_deref() {
        endpoint = endpoint
            .join(&format!("v1/soracloud/apps/{app_name}/status"))
            .wrap_err(
                "failed to derive /v1/soracloud/apps/{app_name}/status URL from --torii-url",
            )?;
    } else {
        endpoint = endpoint
            .join("v1/soracloud/apps/status")
            .wrap_err("failed to derive /v1/soracloud/apps/status URL from --torii-url")?;
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud app-infra status",
    )?;
    let status = decode_app_infra_status(&payload)?;
    if let Some(expected_app_name) = app_name.as_deref() {
        let expected_app_name = expected_app_name
            .parse::<Name>()
            .wrap_err("invalid expected app name")?;
        if status.apps.len() != 1 || status.apps[0].app_name != expected_app_name {
            return Err(eyre!(
                "Soracloud app-infra status did not return exactly app `{expected_app_name}`"
            ));
        }
        if status
            .recent_audit_events
            .iter()
            .any(|event| event.app_name != expected_app_name)
        {
            return Err(eyre!(
                "Soracloud app-infra status returned audit events for an unexpected app"
            ));
        }
    }
    let payload =
        json::to_value(&status).wrap_err("failed to encode exact Soracloud app-infra status V1")?;
    Ok((endpoint, payload))
}
fn filter_soracloud_status_payload(
    payload: &mut norito::json::Value,
    service_name: Option<&str>,
) -> Result<()> {
    decode_network_control_plane_snapshot(payload)?;
    let Some(service_name) = service_name else {
        return Ok(());
    };
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let root = payload
        .as_object_mut()
        .ok_or_else(|| eyre!("Soracloud status must be a JSON object"))?;
    let control_plane = root
        .get_mut("control_plane")
        .ok_or_else(|| eyre!("Soracloud network status is missing `control_plane`"))?;
    let control_plane_map = control_plane
        .as_object_mut()
        .ok_or_else(|| eyre!("Soracloud `control_plane` status must be a JSON object"))?;
    let filtered_service_count = {
        let services = control_plane_map
            .get_mut("services")
            .and_then(norito::json::Value::as_array_mut)
            .ok_or_else(|| eyre!("Soracloud control-plane status is missing `services`"))?;
        services.retain(|entry| {
            entry
                .as_object()
                .and_then(|service| service.get("service_name"))
                .and_then(norito::json::Value::as_str)
                == Some(service_name.as_str())
        });
        u64::try_from(services.len())
            .wrap_err("filtered Soracloud service count exceeds the V1 range")?
    };
    let service_count = control_plane_map
        .get_mut("service_count")
        .ok_or_else(|| eyre!("Soracloud control-plane status is missing `service_count`"))?;
    *service_count = norito::json::Value::from(filtered_service_count);
    let audit_events = control_plane_map
        .get_mut("recent_audit_events")
        .and_then(norito::json::Value::as_array_mut)
        .ok_or_else(|| eyre!("Soracloud control-plane status is missing `recent_audit_events`"))?;
    audit_events.retain(|entry| {
        entry
            .as_object()
            .and_then(|event| event.get("service_name"))
            .and_then(norito::json::Value::as_str)
            == Some(service_name.as_str())
    });
    Ok(())
}
fn fetch_torii_soracloud_service_config_status(
    torii_url: &str,
    service_name: &str,
    config_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let config_name = config_name
        .map(|value| parse_service_material_name_arg("--config-name", value))
        .transpose()?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/service/config/status")
        .wrap_err("failed to derive /v1/soracloud/service/config/status URL from --torii-url")?;
    {
        let mut query = endpoint.query_pairs_mut();
        query.append_pair("service_name", service_name.as_str());
        if let Some(config_name) = config_name.as_deref() {
            query.append_pair("config_name", config_name);
        }
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud service-config status",
    )?;
    let status = decode_service_config_status(&payload)?;
    if status.service_name.to_string() != service_name {
        return Err(eyre!(
            "Soracloud service-config status returned an unexpected service"
        ));
    }
    if let Some(expected_config) = config_name.as_deref()
        && (status.configs.len() != 1 || status.configs[0].config_name != expected_config)
    {
        return Err(eyre!(
            "Soracloud service-config status did not return exactly the requested config `{expected_config}`"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud service-config status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_service_secret_status(
    torii_url: &str,
    service_name: &str,
    secret_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let secret_name = secret_name
        .map(|value| parse_service_material_name_arg("--secret-name", value))
        .transpose()?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/service/secret/status")
        .wrap_err("failed to derive /v1/soracloud/service/secret/status URL from --torii-url")?;
    {
        let mut query = endpoint.query_pairs_mut();
        query.append_pair("service_name", service_name.as_str());
        if let Some(secret_name) = secret_name.as_deref() {
            query.append_pair("secret_name", secret_name);
        }
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud service-secret status",
    )?;
    let status = decode_service_secret_status(&payload)?;
    if status.service_name.to_string() != service_name {
        return Err(eyre!(
            "Soracloud service-secret status returned an unexpected service"
        ));
    }
    if let Some(expected_secret) = secret_name.as_deref()
        && (status.secrets.len() != 1 || status.secrets[0].secret_name != expected_secret)
    {
        return Err(eyre!(
            "Soracloud service-secret status did not return exactly the requested secret `{expected_secret}`"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud service-secret status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_agent_status(
    torii_url: &str,
    apartment_name: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/status")
        .wrap_err("failed to derive /v1/soracloud/agent/status URL from --torii-url")?;
    let apartment_name = apartment_name
        .map(|name| parse_exact_name_arg("--apartment-name", name))
        .transpose()?;
    if let Some(apartment_name) = apartment_name.as_deref() {
        endpoint
            .query_pairs_mut()
            .append_pair("apartment_name", apartment_name);
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud agent status",
    )?;
    let status = decode_agent_status(&payload)?;
    if let Some(expected_apartment) = apartment_name.as_deref() {
        let expected_apartment = expected_apartment
            .parse::<Name>()
            .wrap_err("invalid expected apartment name")?;
        if status.apartments.len() > 1
            || status
                .apartments
                .first()
                .is_some_and(|entry| entry.apartment_name != expected_apartment)
        {
            return Err(eyre!(
                "Soracloud agent status filter returned an unexpected apartment"
            ));
        }
    }
    let payload =
        json::to_value(&status).wrap_err("failed to encode exact Soracloud agent status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_agent_mailbox_status(
    torii_url: &str,
    apartment_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/mailbox/status")
        .wrap_err("failed to derive /v1/soracloud/agent/mailbox/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("apartment_name", apartment_name.as_str());
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud agent-mailbox status",
    )?;
    let status = decode_agent_mailbox_status(&payload)?;
    let expected_apartment = apartment_name
        .parse::<Name>()
        .wrap_err("invalid expected mailbox apartment name")?;
    if status.apartment_name != expected_apartment {
        return Err(eyre!(
            "Soracloud mailbox status returned apartment `{}` instead of `{expected_apartment}`",
            status.apartment_name
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud agent mailbox status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_agent_autonomy_status(
    torii_url: &str,
    apartment_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let apartment_name = parse_exact_name_arg("--apartment-name", apartment_name)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/agent/autonomy/status")
        .wrap_err("failed to derive /v1/soracloud/agent/autonomy/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("apartment_name", apartment_name.as_str());
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud agent-autonomy status",
    )?;
    let status = decode_agent_autonomy_status(&payload)?;
    let expected_apartment = apartment_name
        .parse::<Name>()
        .wrap_err("invalid expected autonomy apartment name")?;
    if status.apartment_name != expected_apartment {
        return Err(eyre!(
            "Soracloud agent autonomy status returned apartment `{}` instead of `{expected_apartment}`",
            status.apartment_name
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud agent autonomy status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_training_job_status(
    torii_url: &str,
    service_name: &str,
    job_id: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let job_id = parse_training_identifier_arg("--job-id", job_id)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/training/job/status")
        .wrap_err("failed to derive /v1/soracloud/training/job/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name.as_str())
        .append_pair("job_id", job_id.as_str());
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud training-job status",
    )?;
    let status = decode_training_job_status(&payload)?;
    if status.job.service_name.to_string() != service_name || status.job.job_id != job_id {
        return Err(eyre!(
            "Soracloud training-job status returned an unexpected service or job"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud training-job status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_model_artifact_status(
    torii_url: &str,
    service_name: &str,
    training_job_id: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let training_job_id = parse_training_identifier_arg("--training-job-id", training_job_id)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/model/artifact/status")
        .wrap_err("failed to derive /v1/soracloud/model/artifact/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name.as_str())
        .append_pair("training_job_id", training_job_id.as_str());
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud model-artifact status",
    )?;
    let status = decode_model_artifact_status(&payload)?;
    if status.service_name.to_string() != service_name
        || status
            .artifacts
            .iter()
            .any(|artifact| artifact.training_job_id != training_job_id)
    {
        return Err(eyre!(
            "Soracloud model-artifact status returned an unexpected service or training job"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud model-artifact status V1")?;
    Ok((endpoint, payload))
}
fn fetch_torii_soracloud_model_weight_status(
    torii_url: &str,
    service_name: &str,
    model_name: &str,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let model_name = parse_exact_name_arg("--model-name", model_name)?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/model/weight/status")
        .wrap_err("failed to derive /v1/soracloud/model/weight/status URL from --torii-url")?;
    endpoint
        .query_pairs_mut()
        .append_pair("service_name", service_name.as_str())
        .append_pair("model_name", model_name.as_str());
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud model-weight status",
    )?;
    let status = decode_model_weight_status(&payload)?;
    if status.model.service_name.to_string() != service_name
        || status.model.model_name.to_string() != model_name
    {
        return Err(eyre!(
            "Soracloud model-weight status returned an unexpected service or model"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud model-weight status V1")?;
    Ok((endpoint, payload))
}
#[allow(clippy::too_many_arguments)]
fn fetch_torii_soracloud_uploaded_model_status(
    torii_url: &str,
    endpoint_path: &str,
    service_name: &str,
    weight_version: &str,
    model_id: Option<&str>,
    model_name: Option<&str>,
    bundle_root: Option<Hash>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    let service_name = parse_exact_name_arg("--service-name", service_name)?;
    let weight_version = parse_training_identifier_arg("--weight-version", weight_version)?;
    if model_id.is_none() && model_name.is_none() {
        return Err(eyre!("--model-id or --model-name is required"));
    }
    let model_id = model_id
        .map(|model_id| parse_training_identifier_arg("--model-id", model_id))
        .transpose()?;
    let model_name = model_name
        .map(|model_name| parse_exact_name_arg("--model-name", model_name))
        .transpose()?;
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join(endpoint_path)
        .wrap_err_with(|| format!("failed to derive /{endpoint_path} URL from --torii-url"))?;
    {
        let mut query = endpoint.query_pairs_mut();
        query.append_pair("service_name", service_name.as_str());
        query.append_pair("weight_version", weight_version.as_str());
        if let Some(model_id) = model_id.as_deref() {
            query.append_pair("model_id", model_id);
        }
        if let Some(model_name) = model_name.as_deref() {
            query.append_pair("model_name", model_name);
        }
        if let Some(bundle_root) = bundle_root {
            query.append_pair("bundle_root", &json::to_json(&bundle_root)?);
        }
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud uploaded-model status",
    )?;
    let status = decode_uploaded_model_status(&payload)?;
    if status.bundle.service_name.to_string() != service_name
        || status.bundle.weight_version != weight_version
        || model_id
            .as_deref()
            .is_some_and(|expected| status.bundle.model_id != expected)
        || bundle_root.is_some_and(|expected| status.bundle.bundle_root != expected)
    {
        return Err(eyre!(
            "Soracloud uploaded-model status returned an unexpected service, model, version, or bundle"
        ));
    }
    if let Some(expected_model_name) = model_name.as_deref()
        && status
            .artifact
            .as_ref()
            .is_none_or(|artifact| artifact.model_name.as_ref() != expected_model_name)
    {
        return Err(eyre!(
            "Soracloud uploaded-model status did not bind requested model_name `{expected_model_name}`"
        ));
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud uploaded-model status V1")?;
    Ok((endpoint, payload))
}
const fn storage_class_query_label(storage_class: StorageClass) -> &'static str {
    match storage_class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}
fn fetch_torii_soracloud_hf_status(
    torii_url: &str,
    repo_id: &str,
    revision: &str,
    storage_class: StorageClass,
    lease_term_ms: u64,
    account_id: Option<&str>,
    api_token: Option<&str>,
    timeout_secs: u64,
) -> Result<(String, norito::json::Value)> {
    if lease_term_ms == 0 {
        return Err(eyre!("--lease-term-ms must be greater than zero"));
    }
    let repo_id = parse_hf_repo_id_arg(repo_id)?;
    let revision = parse_hf_revision_arg(revision)?;
    let account_id = parse_hf_account_id_arg(account_id)?;
    let storage_class_label = storage_class_query_label(storage_class);
    let mut endpoint = reqwest::Url::parse(torii_url)
        .wrap_err_with(|| format!("invalid --torii-url `{torii_url}`"))?
        .join("v1/soracloud/hf/lease/status")
        .wrap_err("failed to derive /v1/soracloud/hf/lease/status URL from --torii-url")?;
    {
        let mut query = endpoint.query_pairs_mut();
        query
            .append_pair("repo_id", repo_id.as_str())
            .append_pair("revision", revision.as_str())
            .append_pair("storage_class", storage_class_label)
            .append_pair("lease_term_ms", &lease_term_ms.to_string());
        if let Some(account_id) = account_id.as_deref() {
            query.append_pair("account_id", account_id);
        }
    }
    let (endpoint, payload) = fetch_torii_soracloud_authenticated_json(
        endpoint,
        api_token,
        timeout_secs,
        "Torii Soracloud HF status",
    )?;
    let status = decode_hf_shared_lease_status(&payload)?;
    if status.source.repo_id != repo_id || status.source.resolved_revision != revision {
        return Err(eyre!(
            "Soracloud HF status source does not match the requested repository and revision"
        ));
    }
    if let Some(pool) = status.pool.as_ref()
        && (pool.storage_class != storage_class || pool.lease_term_ms != lease_term_ms)
    {
        return Err(eyre!(
            "Soracloud HF status pool does not match the requested storage class and lease term"
        ));
    }
    match (account_id.as_deref(), status.member.as_ref()) {
        (Some(expected), Some(member)) if member.account_id.to_string() != expected => {
            return Err(eyre!(
                "Soracloud HF status member does not match the requested account"
            ));
        }
        (None, Some(_)) => {
            return Err(eyre!(
                "Soracloud HF status returned an account member without an account filter"
            ));
        }
        _ => {}
    }
    let payload = json::to_value(&status)
        .wrap_err("failed to encode exact Soracloud HF shared-lease status V1")?;
    Ok((endpoint, payload))
}
fn require_torii_url<'a>(torii_url: Option<&'a str>) -> Result<&'a str> {
    torii_url
        .ok_or_else(|| eyre!("--torii-url is required for Soracloud live control-plane access"))
}
fn resolve_manifest_path(base_dir: &Path, path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}
fn relative_path_string(from_file: &Path, to_path: &Path) -> String {
    let base_dir = from_file.parent().unwrap_or_else(|| Path::new("."));
    pathdiff::diff_paths(to_path, base_dir)
        .unwrap_or_else(|| to_path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}
fn ensure_app_service_ref_matches_manifest_name(
    expected_service_name: &str,
    service_path: &Path,
    service_manifest: &SoraServiceManifestV1,
) -> Result<()> {
    let manifest_service_name = service_manifest.service_name.to_string();
    if expected_service_name != manifest_service_name {
        return Err(eyre!(
            "app service `{}` points to `{}` but the referenced service manifest declares `{}`",
            expected_service_name,
            service_path.display(),
            manifest_service_name
        ));
    }
    Ok(())
}
fn is_app_service_name_mismatch_error(error: &Report) -> bool {
    format!("{error:#}").contains("referenced service manifest declares")
}
fn sync_manifest_pair(
    container_path: &Path,
    service_path: &Path,
    bundle_file: Option<&Path>,
    service_name_override: Option<&str>,
) -> Result<SyncManifestEntryOutput> {
    let (entry, bundle) = project_sync_manifest_pair(
        container_path,
        service_path,
        bundle_file,
        service_name_override,
    )?;
    write_json(container_path, &bundle.container)?;
    write_json(service_path, &bundle.service)?;
    Ok(entry)
}
fn project_sync_manifest_pair(
    container_path: &Path,
    service_path: &Path,
    bundle_file: Option<&Path>,
    service_name_override: Option<&str>,
) -> Result<(SyncManifestEntryOutput, UnpublishedDeploymentBundleV1)> {
    let mut container: UnpublishedContainerManifestV1 = load_json(container_path)?;
    let mut service: SoraServiceManifestV1 = load_json(service_path)?;
    if let Some(expected_service_name) = service_name_override {
        ensure_app_service_ref_matches_manifest_name(
            expected_service_name,
            service_path,
            &service,
        )?;
    }
    if let Some(bundle_file) = bundle_file {
        let bundle_bytes = fs::read(bundle_file).wrap_err_with(|| {
            format!(
                "failed to read Soracloud bundle file {}",
                bundle_file.display()
            )
        })?;
        container.bundle_hash = Hash::new(&bundle_bytes);
    }
    service.container.manifest_hash = container.workspace_hash()?;
    service.container.expected_schema_version = container.schema_version;
    let bundle = UnpublishedDeploymentBundleV1 {
        container: container.clone(),
        service: service.clone(),
    };
    validate_unpublished_deployment_source(&bundle)?;
    let entry = SyncManifestEntryOutput {
        service_name: service_name_override
            .map(str::to_owned)
            .unwrap_or_else(|| service.service_name.to_string()),
        container_manifest_path: container_path.to_string_lossy().into_owned(),
        service_manifest_path: service_path.to_string_lossy().into_owned(),
        container_manifest_hash: service.container.manifest_hash,
        service_manifest_hash: Hash::new(Encode::encode(&service)),
        bundle_file: bundle_file.map(|path| path.to_string_lossy().into_owned()),
        bundle_hash: container.bundle_hash,
    };
    Ok((entry, bundle))
}
fn project_app_manifest_service_refs(
    manifest: &SoracloudAppManifestV1,
    manifest_dir: &Path,
) -> Result<(
    Vec<SyncManifestEntryOutput>,
    Vec<UnpublishedDeploymentBundleV1>,
)> {
    let mut outputs = Vec::with_capacity(manifest.services.len());
    let mut bundles = Vec::with_capacity(manifest.services.len());
    for service in &manifest.services {
        let container_path = resolve_manifest_path(manifest_dir, &service.container_manifest);
        let service_path = resolve_manifest_path(manifest_dir, &service.service_manifest);
        let bundle_file = service
            .bundle_file
            .as_deref()
            .map(|path| resolve_manifest_path(manifest_dir, path));
        let (entry, bundle) = project_sync_manifest_pair(
            &container_path,
            &service_path,
            bundle_file.as_deref(),
            Some(&service.service_name),
        )?;
        outputs.push(entry);
        bundles.push(bundle);
    }
    Ok((outputs, bundles))
}
fn sync_app_manifest_service_refs(
    manifest: &SoracloudAppManifestV1,
    manifest_dir: &Path,
) -> Result<Vec<SyncManifestEntryOutput>> {
    let mut outputs = Vec::with_capacity(manifest.services.len());
    for service in &manifest.services {
        let container_path = resolve_manifest_path(manifest_dir, &service.container_manifest);
        let service_path = resolve_manifest_path(manifest_dir, &service.service_manifest);
        let bundle_file = service
            .bundle_file
            .as_deref()
            .map(|path| resolve_manifest_path(manifest_dir, path));
        outputs.push(sync_manifest_pair(
            &container_path,
            &service_path,
            bundle_file.as_deref(),
            Some(&service.service_name),
        )?);
    }
    Ok(outputs)
}
fn load_json<T>(path: &Path) -> Result<T>
where
    T: JsonDeserialize,
{
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read JSON file {}", path.display()))?;
    let text = String::from_utf8(bytes)
        .wrap_err_with(|| format!("failed to decode {} as UTF-8", path.display()))?;
    json::from_str(&text).wrap_err_with(|| format!("failed to decode {}", path.display()))
}
fn write_json<T>(path: &Path, value: &T) -> Result<()>
where
    T: JsonSerialize + ?Sized,
{
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create directory {}", parent.display()))?;
    }
    let bytes = json::to_vec_pretty(value).wrap_err("failed to encode JSON")?;
    fs::write(path, bytes).wrap_err_with(|| format!("failed to write {}", path.display()))
}
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BundlePackFileSnapshot {
    device: u64,
    inode: u64,
    links: u64,
    size: u64,
    is_file: bool,
    is_directory: bool,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}
#[cfg(unix)]
impl BundlePackFileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            links: metadata.nlink(),
            size: metadata.len(),
            is_file: metadata.is_file(),
            is_directory: metadata.is_dir(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
    fn same_identity(self, other: Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
    fn unchanged(self, other: Self) -> bool {
        self == other
    }
    fn is_regular_single_file(self) -> bool {
        self.is_file && self.links == 1
    }
    fn is_direct_directory(self) -> bool {
        self.is_directory
    }
    const fn is_reparse_point(self) -> bool {
        false
    }
    const fn size(self) -> u64 {
        self.size
    }
}
#[cfg(windows)]
const WINDOWS_FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
#[cfg(windows)]
const WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
#[cfg(windows)]
const WINDOWS_FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
#[cfg(windows)]
const WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
#[cfg(windows)]
const WINDOWS_FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;
#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct BundlePackWindowsFileTime {
    low: u32,
    high: u32,
}
#[cfg(windows)]
#[repr(C)]
struct BundlePackWindowsByHandleFileInformation {
    file_attributes: u32,
    creation_time: BundlePackWindowsFileTime,
    _last_access_time: BundlePackWindowsFileTime,
    last_write_time: BundlePackWindowsFileTime,
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}
#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BundlePackFileSnapshot {
    volume_serial_number: u32,
    file_index: u64,
    links: u32,
    size: u64,
    file_attributes: u32,
    creation_time: u64,
    last_write_time: u64,
}
#[cfg(windows)]
impl BundlePackFileSnapshot {
    fn same_identity(self, other: Self) -> bool {
        self.volume_serial_number == other.volume_serial_number
            && self.file_index == other.file_index
    }
    fn unchanged(self, other: Self) -> bool {
        self == other
    }
    fn is_regular_single_file(self) -> bool {
        self.links == 1
            && self.file_attributes
                & (WINDOWS_FILE_ATTRIBUTE_DIRECTORY | WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT)
                == 0
    }
    fn is_direct_directory(self) -> bool {
        self.file_attributes & WINDOWS_FILE_ATTRIBUTE_DIRECTORY != 0
            && self.file_attributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT == 0
    }
    fn is_reparse_point(self) -> bool {
        self.file_attributes & WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    const fn size(self) -> u64 {
        self.size
    }
}
#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BundlePackFileSnapshot;
#[cfg(not(any(unix, windows)))]
impl BundlePackFileSnapshot {
    const fn same_identity(self, _other: Self) -> bool {
        false
    }
    const fn unchanged(self, _other: Self) -> bool {
        false
    }
    const fn is_regular_single_file(self) -> bool {
        false
    }
    const fn is_direct_directory(self) -> bool {
        false
    }
    const fn is_reparse_point(self) -> bool {
        false
    }
    const fn size(self) -> u64 {
        0
    }
}
#[cfg(windows)]
#[link(name = "kernel32")]
#[allow(unsafe_code)]
unsafe extern "system" {
    #[link_name = "GetFileInformationByHandle"]
    fn bundle_pack_get_file_information_by_handle(
        file: *mut std::ffi::c_void,
        information: *mut BundlePackWindowsByHandleFileInformation,
    ) -> i32;
    #[link_name = "MoveFileExW"]
    fn bundle_pack_move_file_ex(
        existing_path: *const u16,
        replacement_path: *const u16,
        flags: u32,
    ) -> i32;
}
#[cfg(unix)]
fn snapshot_bundle_pack_handle(file: &fs::File) -> Result<BundlePackFileSnapshot> {
    file.metadata()
        .map(|metadata| BundlePackFileSnapshot::from_metadata(&metadata))
        .wrap_err("failed to snapshot Inrou bundle file handle")
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn snapshot_bundle_pack_handle(file: &fs::File) -> Result<BundlePackFileSnapshot> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle as _};
    let mut information = MaybeUninit::<BundlePackWindowsByHandleFileInformation>::uninit();
    // SAFETY: `file` owns a valid kernel handle for the duration of the call,
    // and `information` points to writable storage with the exact Win32 ABI
    // layout required by `GetFileInformationByHandle`.
    let succeeded = unsafe {
        bundle_pack_get_file_information_by_handle(file.as_raw_handle(), information.as_mut_ptr())
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error())
            .wrap_err("GetFileInformationByHandle failed for Inrou bundle path");
    }
    // SAFETY: a nonzero return initializes every field of
    // BY_HANDLE_FILE_INFORMATION.
    let information = unsafe { information.assume_init() };
    let combine = |high: u32, low: u32| u64::from(high) << 32 | u64::from(low);
    Ok(BundlePackFileSnapshot {
        volume_serial_number: information.volume_serial_number,
        file_index: combine(information.file_index_high, information.file_index_low),
        links: information.number_of_links,
        size: combine(information.file_size_high, information.file_size_low),
        file_attributes: information.file_attributes,
        creation_time: combine(
            information.creation_time.high,
            information.creation_time.low,
        ),
        last_write_time: combine(
            information.last_write_time.high,
            information.last_write_time.low,
        ),
    })
}
#[cfg(not(any(unix, windows)))]
fn snapshot_bundle_pack_handle(_file: &fs::File) -> Result<BundlePackFileSnapshot> {
    Err(eyre!(
        "this platform does not expose a stable direct-file identity for Inrou bundle packing"
    ))
}
#[cfg(unix)]
fn open_direct_bundle_pack_file(path: &Path) -> Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| {
        format!(
            "failed to securely open Inrou bundle path `{}`",
            path.display()
        )
    })?;
    Ok(fs::File::from(descriptor))
}
#[cfg(windows)]
fn open_direct_bundle_pack_file(path: &Path) -> Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .share_mode(WINDOWS_FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path).wrap_err_with(|| {
        format!(
            "failed to securely open Inrou bundle path `{}`",
            path.display()
        )
    })
}
#[cfg(not(any(unix, windows)))]
fn open_direct_bundle_pack_file(path: &Path) -> Result<fs::File> {
    Err(eyre!(
        "Inrou bundle path `{}` cannot be opened because this platform does not expose a stable direct-file identity",
        path.display()
    ))
}
#[cfg(unix)]
fn open_direct_bundle_pack_directory(path: &Path) -> Result<fs::File> {
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::DIRECTORY,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| {
        format!(
            "failed to securely open Inrou bundle output directory `{}`",
            path.display()
        )
    })?;
    Ok(fs::File::from(descriptor))
}
#[cfg(windows)]
fn open_direct_bundle_pack_directory(path: &Path) -> Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    let mut options = fs::OpenOptions::new();
    options
        .access_mode(0)
        .share_mode(WINDOWS_FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(WINDOWS_FILE_FLAG_BACKUP_SEMANTICS | WINDOWS_FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path).wrap_err_with(|| {
        format!(
            "failed to securely open Inrou bundle output directory `{}`",
            path.display()
        )
    })
}
#[cfg(not(any(unix, windows)))]
fn open_direct_bundle_pack_directory(path: &Path) -> Result<fs::File> {
    Err(eyre!(
        "Inrou bundle output directory `{}` cannot be opened because this platform does not expose a stable direct-file identity",
        path.display()
    ))
}
struct BundlePackSource {
    file: fs::File,
    snapshot: BundlePackFileSnapshot,
    payload: Vec<u8>,
}
impl BundlePackSource {
    fn revalidate(&self, path: &Path) -> Result<()> {
        let handle_snapshot = snapshot_bundle_pack_handle(&self.file)?;
        if !self.snapshot.unchanged(handle_snapshot) {
            return Err(eyre!(
                "Inrou bundle source `{}` changed while it was retained",
                path.display()
            ));
        }
        let path_file = open_direct_bundle_pack_file(path)?;
        let path_snapshot = snapshot_bundle_pack_handle(&path_file)?;
        if !self.snapshot.unchanged(path_snapshot) {
            return Err(eyre!(
                "Inrou bundle source `{}` was substituted after it was read",
                path.display()
            ));
        }
        Ok(())
    }
}
fn read_stable_bundle_pack_source(path: &Path) -> Result<BundlePackSource> {
    let path_metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect Inrou bundle source `{}`", path.display()))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
        return Err(eyre!(
            "Inrou bundle source `{}` must be a regular non-reparse file with a stable single-link identity",
            path.display()
        ));
    }
    let mut file = open_direct_bundle_pack_file(path)?;
    let snapshot = snapshot_bundle_pack_handle(&file)?;
    if !snapshot.is_regular_single_file() {
        return Err(eyre!(
            "Inrou bundle source `{}` must be a regular non-reparse file with a stable single-link identity",
            path.display()
        ));
    }
    let source_size_bytes = snapshot.size();
    if source_size_bytes > INROU_BUNDLE_PACK_MAX_SOURCE_BYTES {
        return Err(eyre!(
            "Inrou bundle source `{}` exceeds the {} byte packing limit",
            path.display(),
            INROU_BUNDLE_PACK_MAX_SOURCE_BYTES
        ));
    }
    let expected_len = usize::try_from(source_size_bytes).map_err(|_| {
        eyre!(
            "Inrou bundle source `{}` size cannot be represented on this host",
            path.display()
        )
    })?;
    let mut payload = Vec::new();
    payload.try_reserve_exact(expected_len).map_err(|error| {
        eyre!(
            "failed to reserve {} bytes for Inrou bundle source `{}`: {error}",
            source_size_bytes,
            path.display()
        )
    })?;
    (&mut file)
        .take(source_size_bytes.saturating_add(1))
        .read_to_end(&mut payload)
        .wrap_err_with(|| format!("failed to read Inrou bundle source `{}`", path.display()))?;
    if payload.len() != expected_len {
        return Err(eyre!(
            "Inrou bundle source `{}` changed length while it was read",
            path.display()
        ));
    }
    let source = BundlePackSource {
        file,
        snapshot,
        payload,
    };
    source.revalidate(path)?;
    Ok(source)
}
fn bundle_pack_paths_lexically_equal(left: &Path, right: &Path) -> bool {
    left.components().eq(right.components())
}
fn reject_bundle_pack_source_output_alias(
    source_path: &Path,
    output: &Path,
    source: &BundlePackSource,
) -> Result<()> {
    if bundle_pack_paths_lexically_equal(source_path, output) {
        return Err(eyre!(
            "Inrou bundle output `{}` must not replace its source file",
            output.display()
        ));
    }
    source.revalidate(source_path)?;
    let output_metadata = match fs::symlink_metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).wrap_err_with(|| {
                format!(
                    "failed to inspect Inrou bundle output `{}`",
                    output.display()
                )
            });
        }
    };
    if output_metadata.file_type().is_symlink() {
        return Ok(());
    }
    if output_metadata.is_dir() {
        return Err(eyre!(
            "Inrou bundle output `{}` must not be an existing directory",
            output.display()
        ));
    }
    if !output_metadata.is_file() {
        return Err(eyre!(
            "Inrou bundle output `{}` must be a regular file or replaceable reparse path",
            output.display()
        ));
    }
    let output_file = open_direct_bundle_pack_file(output)?;
    let output_snapshot = snapshot_bundle_pack_handle(&output_file)?;
    if output_snapshot.is_reparse_point() {
        return Ok(());
    }
    if !output_snapshot.is_regular_single_file() {
        return Err(eyre!(
            "Inrou bundle output `{}` must be a regular single-link file or replaceable reparse path",
            output.display()
        ));
    }
    if source.snapshot.same_identity(output_snapshot) {
        return Err(eyre!(
            "Inrou bundle output `{}` aliases its source file",
            output.display()
        ));
    }
    Ok(())
}
struct BundlePackParentGuard {
    path: PathBuf,
    file: fs::File,
    snapshot: BundlePackFileSnapshot,
}
impl BundlePackParentGuard {
    fn open(output: &Path) -> Result<Self> {
        let path = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        fs::create_dir_all(&path).wrap_err_with(|| {
            format!(
                "failed to create Inrou bundle output directory `{}`",
                path.display()
            )
        })?;
        let path_metadata = fs::symlink_metadata(&path).wrap_err_with(|| {
            format!(
                "failed to inspect Inrou bundle output directory `{}`",
                path.display()
            )
        })?;
        if path_metadata.file_type().is_symlink() || !path_metadata.is_dir() {
            return Err(eyre!(
                "Inrou bundle output parent `{}` must be a direct directory",
                path.display()
            ));
        }
        let file = open_direct_bundle_pack_directory(&path)?;
        let snapshot = snapshot_bundle_pack_handle(&file)?;
        if !snapshot.is_direct_directory() {
            return Err(eyre!(
                "Inrou bundle output parent `{}` must be a direct non-reparse directory",
                path.display()
            ));
        }
        Ok(Self {
            path,
            file,
            snapshot,
        })
    }
    fn revalidate(&self) -> Result<()> {
        let handle_snapshot = snapshot_bundle_pack_handle(&self.file)?;
        if !self.snapshot.same_identity(handle_snapshot) || !handle_snapshot.is_direct_directory() {
            return Err(eyre!(
                "retained Inrou bundle output directory `{}` changed identity",
                self.path.display()
            ));
        }
        let path_file = open_direct_bundle_pack_directory(&self.path)?;
        let path_snapshot = snapshot_bundle_pack_handle(&path_file)?;
        if !self.snapshot.same_identity(path_snapshot) || !path_snapshot.is_direct_directory() {
            return Err(eyre!(
                "Inrou bundle output directory `{}` was substituted",
                self.path.display()
            ));
        }
        Ok(())
    }
    #[cfg(unix)]
    fn sync(&self) -> Result<()> {
        self.file.sync_all().wrap_err_with(|| {
            format!(
                "failed to synchronize Inrou bundle output directory `{}`",
                self.path.display()
            )
        })
    }
    #[cfg(not(unix))]
    fn sync(&self) -> Result<()> {
        let snapshot = snapshot_bundle_pack_handle(&self.file)?;
        if !self.snapshot.same_identity(snapshot) || !snapshot.is_direct_directory() {
            return Err(eyre!(
                "Inrou bundle output directory `{}` changed before durability confirmation",
                self.path.display()
            ));
        }
        Ok(())
    }
}
struct BundlePackArchiveWriter<'a> {
    inner: &'a mut fs::File,
    written_bytes: u64,
    max_bytes: u64,
}
impl<'a> BundlePackArchiveWriter<'a> {
    const fn new(inner: &'a mut fs::File, max_bytes: u64) -> Self {
        Self {
            inner,
            written_bytes: 0,
            max_bytes,
        }
    }
    const fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
}
impl io::Write for BundlePackArchiveWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = u64::try_from(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Inrou bundle archive write size does not fit u64",
            )
        })?;
        let requested_total = self.written_bytes.checked_add(requested).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Inrou bundle archive byte counter overflow",
            )
        })?;
        if requested_total > self.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "canonical Inrou bundle archive exceeds the {} byte limit",
                    self.max_bytes
                ),
            ));
        }
        let written = self.inner.write(bytes)?;
        let written_u64 = u64::try_from(written).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Inrou bundle archive write length does not fit u64",
            )
        })?;
        self.written_bytes = self.written_bytes.checked_add(written_u64).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Inrou bundle archive byte counter overflow",
            )
        })?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
fn ensure_bundle_pack_archive_size_within_limit(size: u64) -> Result<()> {
    if size > INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES {
        return Err(eyre!(
            "canonical Inrou bundle archive size {size} exceeds the {} byte limit",
            INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES
        ));
    }
    Ok(())
}
fn remove_bundle_pack_file_if_owned(path: &Path, identity: BundlePackFileSnapshot) {
    let Ok(file) = open_direct_bundle_pack_file(path) else {
        return;
    };
    let Ok(snapshot) = snapshot_bundle_pack_handle(&file) else {
        return;
    };
    if snapshot.is_regular_single_file() && identity.same_identity(snapshot) {
        drop(file);
        let _ = fs::remove_file(path);
    }
}
struct BundlePackAtomicOutput {
    path: PathBuf,
    snapshot: BundlePackFileSnapshot,
    file: Option<fs::File>,
    parent: BundlePackParentGuard,
    installed: bool,
}
struct BundlePackStagingCreationGuard {
    path: PathBuf,
    file: Option<fs::File>,
}
impl BundlePackStagingCreationGuard {
    fn file(&self) -> Result<&fs::File> {
        self.file
            .as_ref()
            .ok_or_else(|| eyre!("staged Inrou bundle file is already closed"))
    }
    fn take_file(&mut self) -> Result<fs::File> {
        self.file
            .take()
            .ok_or_else(|| eyre!("staged Inrou bundle file is already closed"))
    }
}
impl Drop for BundlePackStagingCreationGuard {
    fn drop(&mut self) {
        let Some(file) = self.file.take() else {
            return;
        };
        let Ok(snapshot) = snapshot_bundle_pack_handle(&file) else {
            return;
        };
        drop(file);
        remove_bundle_pack_file_if_owned(&self.path, snapshot);
    }
}
impl BundlePackAtomicOutput {
    fn create(output: &Path) -> Result<Self> {
        if output.file_name().is_none() {
            return Err(eyre!(
                "Inrou bundle output `{}` must name a file",
                output.display()
            ));
        }
        let parent = BundlePackParentGuard::open(output)?;
        let mut rng = OsRng;
        for _ in 0..INROU_BUNDLE_PACK_TEMP_ATTEMPTS {
            let mut suffix = [0_u8; 16];
            rng.try_fill_bytes(&mut suffix)
                .map_err(|error| eyre!("Inrou bundle staging-name OS RNG failed: {error}"))?;
            let path = parent
                .path
                .join(format!(".inrou-bundle-pack-{}.tmp", hex::encode(suffix)));
            let mut options = fs::OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                options.mode(0o600);
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt as _;
                options.share_mode(WINDOWS_FILE_SHARE_READ_WRITE_DELETE);
            }
            match options.open(&path) {
                Ok(file) => {
                    let mut staging = BundlePackStagingCreationGuard {
                        path: path.clone(),
                        file: Some(file),
                    };
                    let snapshot = snapshot_bundle_pack_handle(staging.file()?)?;
                    if !snapshot.is_regular_single_file() {
                        return Err(eyre!(
                            "staged Inrou bundle `{}` lacks a stable single-file identity",
                            path.display()
                        ));
                    }
                    let path_file = open_direct_bundle_pack_file(&path)?;
                    let path_snapshot = snapshot_bundle_pack_handle(&path_file)?;
                    if !snapshot.same_identity(path_snapshot) {
                        return Err(eyre!(
                            "staged Inrou bundle `{}` was substituted after creation",
                            path.display()
                        ));
                    }
                    let file = staging.take_file()?;
                    return Ok(Self {
                        path,
                        snapshot,
                        file: Some(file),
                        parent,
                        installed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).wrap_err_with(|| {
                        format!(
                            "failed to create staged Inrou bundle in `{}`",
                            parent.path.display()
                        )
                    });
                }
            }
        }
        Err(eyre!(
            "failed to allocate an exclusive staged Inrou bundle in `{}`",
            parent.path.display()
        ))
    }
    fn file_mut(&mut self) -> Result<&mut fs::File> {
        self.file
            .as_mut()
            .ok_or_else(|| eyre!("staged Inrou bundle file is already closed"))
    }
    fn finish_and_install(
        mut self,
        output: &Path,
        source_path: &Path,
        source: &BundlePackSource,
        expected_archive_bytes: u64,
    ) -> Result<(Hash, u64)> {
        let staged_file = self
            .file
            .as_mut()
            .ok_or_else(|| eyre!("staged Inrou bundle file is already closed"))?;
        staged_file
            .flush()
            .wrap_err("failed to flush staged Inrou bundle")?;
        staged_file
            .sync_all()
            .wrap_err("failed to synchronize staged Inrou bundle")?;
        let written_snapshot = snapshot_bundle_pack_handle(staged_file)?;
        if !written_snapshot.is_regular_single_file()
            || !self.snapshot.same_identity(written_snapshot)
            || written_snapshot.size() != expected_archive_bytes
        {
            return Err(eyre!(
                "staged Inrou bundle `{}` was substituted or changed size while it was encoded",
                self.path.display()
            ));
        }
        ensure_bundle_pack_archive_size_within_limit(written_snapshot.size())?;
        let staged_path_file = open_direct_bundle_pack_file(&self.path)?;
        let staged_path_snapshot = snapshot_bundle_pack_handle(&staged_path_file)?;
        if !written_snapshot.unchanged(staged_path_snapshot) {
            return Err(eyre!(
                "staged Inrou bundle `{}` path no longer matches its retained handle",
                self.path.display()
            ));
        }
        staged_file
            .seek(SeekFrom::Start(0))
            .wrap_err("failed to rewind staged Inrou bundle")?;
        let (bundle_hash, bundle_size_bytes) =
            Hash::new_from_reader_bounded(&mut *staged_file, INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES)
                .wrap_err("failed to hash staged Inrou bundle")?;
        if bundle_size_bytes != written_snapshot.size() {
            return Err(eyre!(
                "staged Inrou bundle size changed while it was hashed"
            ));
        }
        let hashed_snapshot = snapshot_bundle_pack_handle(staged_file)?;
        let hashed_path_file = open_direct_bundle_pack_file(&self.path)?;
        let hashed_path_snapshot = snapshot_bundle_pack_handle(&hashed_path_file)?;
        if !written_snapshot.unchanged(hashed_snapshot)
            || !written_snapshot.unchanged(hashed_path_snapshot)
        {
            return Err(eyre!(
                "staged Inrou bundle `{}` changed while it was hashed",
                self.path.display()
            ));
        }
        source.revalidate(source_path)?;
        self.parent.revalidate()?;
        atomic_replace_bundle_pack_file(&self.path, output).wrap_err_with(|| {
            format!(
                "failed to atomically install Inrou bundle `{}`",
                output.display()
            )
        })?;
        self.installed = true;
        let post_commit = (|| -> Result<()> {
            let retained_snapshot = snapshot_bundle_pack_handle(
                self.file
                    .as_ref()
                    .ok_or_else(|| eyre!("staged Inrou bundle handle was closed early"))?,
            )?;
            if !self.snapshot.same_identity(retained_snapshot)
                || !retained_snapshot.is_regular_single_file()
                || retained_snapshot.size() != bundle_size_bytes
            {
                return Err(eyre!(
                    "retained staging handle no longer identifies the installed archive"
                ));
            }
            let mut installed_file = open_direct_bundle_pack_file(output)?;
            let installed_snapshot = snapshot_bundle_pack_handle(&installed_file)?;
            if !retained_snapshot.same_identity(installed_snapshot)
                || !installed_snapshot.is_regular_single_file()
                || installed_snapshot.size() != bundle_size_bytes
            {
                return Err(eyre!(
                    "installed path does not identify the promoted staging file"
                ));
            }
            installed_file
                .seek(SeekFrom::Start(0))
                .wrap_err("failed to rewind installed Inrou bundle")?;
            let (installed_hash, installed_size) = Hash::new_from_reader_bounded(
                &mut installed_file,
                INROU_BUNDLE_PACK_MAX_ARCHIVE_BYTES,
            )
            .wrap_err("failed to hash installed Inrou bundle")?;
            if installed_size != bundle_size_bytes || installed_hash != bundle_hash {
                return Err(eyre!(
                    "installed Inrou bundle bytes do not match the staged hash and size"
                ));
            }
            let installed_after_hash = snapshot_bundle_pack_handle(&installed_file)?;
            let installed_path_file = open_direct_bundle_pack_file(output)?;
            let installed_path_snapshot = snapshot_bundle_pack_handle(&installed_path_file)?;
            if !installed_snapshot.unchanged(installed_after_hash)
                || !installed_snapshot.unchanged(installed_path_snapshot)
            {
                return Err(eyre!(
                    "installed Inrou bundle changed during post-commit verification"
                ));
            }
            self.parent.revalidate()?;
            self.parent.sync()?;
            Ok(())
        })();
        if let Err(error) = post_commit {
            return Err(error).wrap_err_with(|| {
                format!(
                    "Inrou bundle archive was installed at `{}`, but post-commit verification or durability failed",
                    output.display()
                )
            });
        }
        Ok((bundle_hash, bundle_size_bytes))
    }
}
impl Drop for BundlePackAtomicOutput {
    fn drop(&mut self) {
        if self.installed {
            return;
        }
        drop(self.file.take());
        remove_bundle_pack_file_if_owned(&self.path, self.snapshot);
    }
}
#[cfg(unix)]
fn atomic_replace_bundle_pack_file(staged: &Path, output: &Path) -> Result<()> {
    fs::rename(staged, output).wrap_err_with(|| {
        format!(
            "failed to replace `{}` with staged file `{}`",
            output.display(),
            staged.display()
        )
    })
}
#[cfg(windows)]
fn windows_bundle_pack_wide_path(path: &Path) -> Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt as _;
    let mut wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
    if wide.contains(&0) {
        return Err(eyre!(
            "Windows Inrou bundle path `{}` contains an interior NUL",
            path.display()
        ));
    }
    wide.push(0);
    Ok(wide)
}
#[cfg(windows)]
#[allow(unsafe_code)]
fn atomic_replace_bundle_pack_file(staged: &Path, output: &Path) -> Result<()> {
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;
    let staged_wide = windows_bundle_pack_wide_path(staged)?;
    let output_wide = windows_bundle_pack_wide_path(output)?;
    // SAFETY: both path buffers are NUL-terminated and remain alive for the
    // duration of the call. The flags request an atomic same-volume replace
    // and synchronous metadata flush.
    let succeeded = unsafe {
        bundle_pack_move_file_ex(
            staged_wide.as_ptr(),
            output_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        return Err(io::Error::last_os_error()).wrap_err_with(|| {
            format!(
                "MoveFileExW failed to replace `{}` with `{}`",
                output.display(),
                staged.display()
            )
        });
    }
    Ok(())
}
#[cfg(not(any(unix, windows)))]
fn atomic_replace_bundle_pack_file(_staged: &Path, _output: &Path) -> Result<()> {
    Err(eyre!(
        "atomic Inrou bundle replacement is unsupported on this platform"
    ))
}
fn ensure_can_write(path: &Path, overwrite: bool) -> Result<()> {
    if !overwrite && path.exists() {
        return Err(eyre!(
            "file {} already exists (use --overwrite to replace it)",
            path.display()
        ));
    }
    Ok(())
}
fn workspace_fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}
fn service_handler(
    handler_name: &str,
    class: SoraServiceHandlerClassV1,
    entrypoint: &str,
    route_path: Option<&str>,
    certified_response: SoraCertifiedResponsePolicyV1,
    mailbox: Option<(&str, u32, u64, u32)>,
) -> SoraServiceHandlerV1 {
    SoraServiceHandlerV1 {
        handler_name: handler_name.parse().expect("literal handler name is valid"),
        class,
        entrypoint: entrypoint.to_owned(),
        route_path: route_path.map(ToOwned::to_owned),
        certified_response,
        mailbox: mailbox.map(
            |(queue_name, max_pending_messages, max_message_bytes, retention_blocks)| {
                SoraMailboxContractV1 {
                    queue_name: queue_name.parse().expect("literal queue name is valid"),
                    max_pending_messages: NonZeroU32::new(max_pending_messages)
                        .expect("nonzero literal"),
                    max_message_bytes: NonZeroU64::new(max_message_bytes).expect("nonzero literal"),
                    retention_blocks: NonZeroU32::new(retention_blocks).expect("nonzero literal"),
                }
            },
        ),
    }
}
fn service_artifact(
    kind: SoraArtifactKindV1,
    artifact_path: &str,
    handler_name: Option<&str>,
) -> SoraArtifactRefV1 {
    SoraArtifactRefV1 {
        kind,
        artifact_hash: Hash::new(artifact_path.as_bytes()),
        artifact_path: artifact_path.to_owned(),
        handler_name: handler_name.map(|name| name.parse().expect("literal handler name is valid")),
    }
}
macro_rules! state_binding {
    ($name:expr, $scope:ident, $mutability:ident, $encryption:ident, $prefix:expr, $item:literal, $total:literal) => {
        SoraStateBindingV1 {
            schema_version: SORA_STATE_BINDING_VERSION_V1,
            binding_name: $name.parse().expect("literal binding name is valid"),
            scope: SoraStateScopeV1::$scope,
            mutability: SoraStateMutabilityV1::$mutability,
            encryption: SoraStateEncryptionV1::$encryption,
            key_prefix: $prefix.to_owned(),
            max_item_bytes: NonZeroU64::new($item).expect("nonzero literal"),
            max_total_bytes: NonZeroU64::new($total).expect("nonzero literal"),
        }
    };
}
fn auth_state_binding(name: &str, prefix: &str) -> SoraStateBindingV1 {
    state_binding!(
        name,
        ServiceState,
        ReadWrite,
        ClientCiphertext,
        prefix,
        8_192,
        4_194_304
    )
}
fn default_inrou_manifest() -> UnpublishedInrouManifestWorkspaceV1 {
    UnpublishedInrouManifestWorkspaceV1 {
        schema_version: SORA_INROU_MANIFEST_VERSION_V1,
        guest_images: std::collections::BTreeMap::from([
            (
                "x86_64".to_owned(),
                UnpublishedInrouGuestImageWorkspaceV1 {
                    kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    published_artifact: (),
                },
            ),
            (
                "aarch64".to_owned(),
                UnpublishedInrouGuestImageWorkspaceV1 {
                    kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                    rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                    initrd_image_path: None,
                    published_artifact: (),
                },
            ),
        ]),
    }
}
fn default_generic_http_service_lease_volumes() -> Vec<SoraLeaseVolumeBindingV1> {
    vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero literal"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "app_data".parse().expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/app_data".to_owned(),
            max_total_bytes: NonZeroU64::new(536_870_912).expect("nonzero literal"),
        },
    ]
}
fn default_split_app_live_lease_volumes() -> Vec<SoraLeaseVolumeBindingV1> {
    vec![
        SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: NonZeroU64::new(8 * 1024 * 1024 * 1024).expect("nonzero literal"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "shared_cache"
                .parse()
                .expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Hot,
            mount_path: "/var/lib/soracloud/volumes/shared_cache".to_owned(),
            max_total_bytes: NonZeroU64::new(536_870_912).expect("nonzero literal"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "search_sessions"
                .parse()
                .expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/search_sessions".to_owned(),
            max_total_bytes: NonZeroU64::new(268_435_456).expect("nonzero literal"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "collector_state"
                .parse()
                .expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/collector_state".to_owned(),
            max_total_bytes: NonZeroU64::new(268_435_456).expect("nonzero literal"),
        },
        SoraLeaseVolumeBindingV1 {
            volume_name: "runtime_cache"
                .parse()
                .expect("literal volume name is valid"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: StorageClass::Warm,
            mount_path: "/var/lib/soracloud/volumes/runtime_cache".to_owned(),
            max_total_bytes: NonZeroU64::new(268_435_456).expect("nonzero literal"),
        },
    ]
}
fn build_split_app_live_service_bundle(
    app_name: &str,
    host: &str,
    app_version: &str,
) -> Result<UnpublishedDeploymentBundleV1> {
    let mut container = UnpublishedContainerManifestV1::from_non_inrou_manifest(load_json::<
        SoraContainerManifestV1,
    >(
        &workspace_fixture(DEFAULT_CONTAINER_MANIFEST),
    )?)?;
    let mut service =
        load_json::<SoraServiceManifestV1>(&workspace_fixture(DEFAULT_SERVICE_MANIFEST))?;
    let service_name: Name = format!("{app_name}_live")
        .parse()
        .wrap_err("invalid split-app live service name")?;
    container.runtime = SoraContainerRuntimeV1::Inrou;
    container.bundle_path = "/app/server.mjs".to_owned();
    container.entrypoint = "/app/server.mjs".to_owned();
    container.args.clear();
    container.inrou = Some(default_inrou_manifest());
    container
        .env
        .insert("SORACLOUD_TEMPLATE".to_owned(), "split-app-live".to_owned());
    container
        .env
        .insert("SORACLOUD_HTTP_PORT".to_owned(), "8787".to_owned());
    container.capabilities.network = SoraNetworkPolicyV1::Isolated;
    container.capabilities.allow_state_writes = false;
    container.capabilities.allow_model_inference = false;
    container.capabilities.allow_model_training = false;
    container.lifecycle.healthcheck_path = Some("/health".to_owned());
    service.service_name = service_name;
    service.service_version = app_version.to_owned();
    service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
    service.route = Some(SoraRouteTargetV1 {
        host: host.to_owned(),
        path_prefix: "/api/v1".to_owned(),
        service_port: NonZeroU16::new(8787).expect("nonzero literal"),
        visibility: SoraRouteVisibilityV1::Public,
        tls_mode: SoraTlsModeV1::Required,
    });
    service.replicas = NonZeroU16::new(1).expect("nonzero literal");
    service.state_bindings.clear();
    service.lease_volumes = default_split_app_live_lease_volumes();
    service.handlers.clear();
    service.artifacts.clear();
    service.container.manifest_hash = container.workspace_hash()?;
    service.container.expected_schema_version = container.schema_version;
    let bundle = UnpublishedDeploymentBundleV1 { container, service };
    validate_unpublished_deployment_source(&bundle)?;
    Ok(bundle)
}
fn build_split_app_vault_service_bundle(
    app_name: &str,
    host: &str,
    app_version: &str,
) -> Result<SoraDeploymentBundleV1> {
    let mut container =
        load_json::<SoraContainerManifestV1>(&workspace_fixture(DEFAULT_CONTAINER_MANIFEST))?;
    let mut service =
        load_json::<SoraServiceManifestV1>(&workspace_fixture(DEFAULT_SERVICE_MANIFEST))?;
    let service_name: Name = format!("{app_name}_vault")
        .parse()
        .wrap_err("invalid split-app vault service name")?;
    container.runtime = SoraContainerRuntimeV1::Ivm;
    container.bundle_path = "/bundles/vault-api.to".to_owned();
    container.entrypoint = "main".to_owned();
    container.args = vec!["--http".to_owned(), "--port=8788".to_owned()];
    container.env.insert(
        "SORACLOUD_TEMPLATE".to_owned(),
        "split-app-vault".to_owned(),
    );
    container
        .env
        .insert("AUTH_MODE".to_owned(), "strict".to_owned());
    container
        .env
        .insert("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned());
    container
        .env
        .insert("AUTH_CHALLENGE_TTL_SECS".to_owned(), "120".to_owned());
    container
        .env
        .insert("AUTH_CAPABILITY_MAP_JSON".to_owned(), "{}".to_owned());
    container
        .env
        .insert("PUBLIC_BASE_URL".to_owned(), format!("https://{host}"));
    container.capabilities.network = SoraNetworkPolicyV1::Allowlist(vec![
        SoraNetworkAllowlistEntryV1::new("torii.sora.internal", [443]),
        SoraNetworkAllowlistEntryV1::new("wallet.sora.internal", [443]),
    ]);
    container.capabilities.allow_state_writes = true;
    container.capabilities.allow_model_inference = false;
    container.capabilities.allow_model_training = false;
    container.lifecycle.healthcheck_path = Some("/api/auth/me".to_owned());
    service.service_name = service_name;
    service.service_version = app_version.to_owned();
    service.execution_plane = SoraServiceExecutionPlaneV1::DeterministicService;
    service.route = Some(SoraRouteTargetV1 {
        host: host.to_owned(),
        path_prefix: "/api".to_owned(),
        service_port: NonZeroU16::new(8788).expect("nonzero literal"),
        visibility: SoraRouteVisibilityV1::Public,
        tls_mode: SoraTlsModeV1::Required,
    });
    service.replicas = NonZeroU16::new(2).expect("nonzero literal");
    service.state_bindings = vec![
        auth_state_binding("auth_challenges", "/state/auth/challenges"),
        auth_state_binding("auth_sessions", "/state/auth/sessions"),
        state_binding!(
            "user_preferences",
            ConfidentialState,
            ReadWrite,
            FheCiphertext,
            "/state/users/preferences",
            8_192,
            4_194_304
        ),
        state_binding!(
            "user_saved_searches",
            ConfidentialState,
            ReadWrite,
            FheCiphertext,
            "/state/users/saved_searches",
            32_768,
            8_388_608
        ),
    ];
    service.lease_volumes.clear();
    service.handlers = vec![
        service_handler(
            "auth_me",
            SoraServiceHandlerClassV1::Query,
            "serve_auth_me",
            Some("/auth/me"),
            SoraCertifiedResponsePolicyV1::AuditReceipt,
            None,
        ),
        service_handler(
            "user_preferences_get",
            SoraServiceHandlerClassV1::Query,
            "serve_user_preferences",
            Some("/v1/user/preferences"),
            SoraCertifiedResponsePolicyV1::AuditReceipt,
            None,
        ),
        service_handler(
            "saved_searches_list",
            SoraServiceHandlerClassV1::Query,
            "serve_saved_searches",
            Some("/v1/user/saved-searches"),
            SoraCertifiedResponsePolicyV1::AuditReceipt,
            None,
        ),
        service_handler(
            "auth_challenge",
            SoraServiceHandlerClassV1::Update,
            "issue_auth_challenge",
            Some("/auth/challenge"),
            SoraCertifiedResponsePolicyV1::None,
            Some(("auth_updates", 512, 32_768, 1_440)),
        ),
        service_handler(
            "auth_login",
            SoraServiceHandlerClassV1::Update,
            "complete_auth_login",
            Some("/auth/login"),
            SoraCertifiedResponsePolicyV1::None,
            Some(("auth_updates", 256, 131_072, 2_880)),
        ),
        service_handler(
            "auth_logout",
            SoraServiceHandlerClassV1::Update,
            "close_auth_session",
            Some("/auth/logout"),
            SoraCertifiedResponsePolicyV1::None,
            Some(("auth_updates", 256, 131_072, 2_880)),
        ),
        service_handler(
            "user_preferences_put",
            SoraServiceHandlerClassV1::Update,
            "store_user_preferences",
            Some("/v1/user/preferences"),
            SoraCertifiedResponsePolicyV1::None,
            Some(("user_updates", 256, 131_072, 2_880)),
        ),
        service_handler(
            "saved_searches_post",
            SoraServiceHandlerClassV1::Update,
            "store_saved_search",
            Some("/v1/user/saved-searches"),
            SoraCertifiedResponsePolicyV1::None,
            Some(("user_updates", 256, 131_072, 2_880)),
        ),
    ];
    service.artifacts = vec![
        service_artifact(
            SoraArtifactKindV1::Journal,
            "/journals/vault-api.journal",
            Some("auth_challenge"),
        ),
        service_artifact(
            SoraArtifactKindV1::Checkpoint,
            "/checkpoints/vault-api.chk",
            Some("user_preferences_put"),
        ),
    ];
    service.container.manifest_hash = Hash::new(Encode::encode(&container));
    service.container.expected_schema_version = container.schema_version;
    let bundle = SoraDeploymentBundleV1 {
        schema_version: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
        container,
        service,
    };
    bundle.validate_for_admission()?;
    Ok(bundle)
}
fn apply_init_template_defaults(
    template: InitTemplate,
    service_name: &Name,
    service: &mut SoraServiceManifestV1,
    container: &mut UnpublishedContainerManifestV1,
) -> Result<()> {
    let dns_label = normalized_service_label(service_name.as_ref());
    let host = format!("{dns_label}.sora");
    match template {
        InitTemplate::Baseline => Ok(()),
        InitTemplate::HttpService => {
            container.runtime = SoraContainerRuntimeV1::Inrou;
            container.bundle_path = "/app/server.mjs".to_owned();
            container.entrypoint = "/app/server.mjs".to_owned();
            container.args.clear();
            container.inrou = Some(default_inrou_manifest());
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "http-service".to_owned());
            container.env.insert(
                "SORACLOUD_HTTP_SERVICE_NAME".to_owned(),
                service_name.to_string(),
            );
            container
                .env
                .insert("SORACLOUD_HTTP_PORT".to_owned(), "8787".to_owned());
            container.capabilities.network = SoraNetworkPolicyV1::Isolated;
            container.capabilities.allow_state_writes = false;
            container.capabilities.allow_model_inference = false;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/health".to_owned());
            service.execution_plane = SoraServiceExecutionPlaneV1::HttpService;
            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/api/v1".to_owned(),
                service_port: NonZeroU16::new(8787).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(1).expect("nonzero literal");
            service.state_bindings.clear();
            service.lease_volumes = default_generic_http_service_lease_volumes();
            service.handlers.clear();
            service.artifacts.clear();
            Ok(())
        }
        InitTemplate::Site => {
            container.runtime = SoraContainerRuntimeV1::Ivm;
            container.bundle_path = "/bundles/site-static.to".to_owned();
            container.entrypoint = "main".to_owned();
            container.args = vec!["--http".to_owned(), "--port=8080".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "site".to_owned());
            container.capabilities.network =
                SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                    "torii.sora.internal",
                    [443],
                )]);
            container.capabilities.allow_state_writes = false;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/healthz".to_owned());
            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/".to_owned(),
                service_port: NonZeroU16::new(8080).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(2).expect("nonzero literal");
            service.state_bindings.clear();
            service.handlers = vec![service_handler(
                "assets",
                SoraServiceHandlerClassV1::Asset,
                "serve_assets",
                Some("/"),
                SoraCertifiedResponsePolicyV1::StateCommitment,
                None,
            )];
            service.artifacts = vec![service_artifact(
                SoraArtifactKindV1::StaticAsset,
                "/app/dist/index.html",
                Some("assets"),
            )];
            Ok(())
        }
        InitTemplate::Webapp => {
            container.runtime = SoraContainerRuntimeV1::Ivm;
            container.bundle_path = "/bundles/webapp-api.to".to_owned();
            container.entrypoint = "main".to_owned();
            container.args = vec!["--http".to_owned(), "--port=8787".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "webapp".to_owned());
            container
                .env
                .insert("AUTH_MODE".to_owned(), "strict".to_owned());
            container
                .env
                .insert("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned());
            container
                .env
                .insert("AUTH_CHALLENGE_TTL_SECS".to_owned(), "120".to_owned());
            container
                .env
                .insert("AUTH_CAPABILITY_MAP_JSON".to_owned(), "{}".to_owned());
            container
                .env
                .insert("PUBLIC_BASE_URL".to_owned(), format!("https://{host}"));
            container.capabilities.network = SoraNetworkPolicyV1::Allowlist(vec![
                SoraNetworkAllowlistEntryV1::new("torii.sora.internal", [443]),
                SoraNetworkAllowlistEntryV1::new("wallet.sora.internal", [443]),
            ]);
            container.capabilities.allow_state_writes = true;
            container.capabilities.allow_model_inference = false;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/api/v1/health".to_owned());
            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/api".to_owned(),
                service_port: NonZeroU16::new(8787).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.state_bindings = vec![
                auth_state_binding("auth_challenges", "/state/auth/challenges"),
                auth_state_binding("auth_sessions", "/state/auth/sessions"),
            ];
            service.handlers = vec![
                service_handler(
                    "query",
                    SoraServiceHandlerClassV1::Query,
                    "serve_query",
                    Some("/query"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "update",
                    SoraServiceHandlerClassV1::Update,
                    "apply_update",
                    Some("/update"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("updates", 1024, 65_536, 1_440)),
                ),
            ];
            service.artifacts = vec![service_artifact(
                SoraArtifactKindV1::Journal,
                "/journals/webapp.journal",
                Some("update"),
            )];
            Ok(())
        }
        InitTemplate::PiiApp => {
            container.runtime = SoraContainerRuntimeV1::Ivm;
            container.bundle_path = "/bundles/pii-app-api.to".to_owned();
            container.entrypoint = "main".to_owned();
            container.args = vec!["--http".to_owned(), "--port=8788".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "pii-app".to_owned());
            container
                .env
                .insert("AUTH_MODE".to_owned(), "strict".to_owned());
            container
                .env
                .insert("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned());
            container
                .env
                .insert("AUTH_CHALLENGE_TTL_SECS".to_owned(), "120".to_owned());
            container
                .env
                .insert("AUTH_CAPABILITY_MAP_JSON".to_owned(), "{}".to_owned());
            container
                .env
                .insert("PUBLIC_BASE_URL".to_owned(), format!("https://{host}"));
            container
                .env
                .insert("PII_DATA_CATEGORY_EXAMPLE".to_owned(), "health".to_owned());
            container.env.insert(
                "CONSENT_POLICY_NAMESPACE".to_owned(),
                "pii.consent.v1".to_owned(),
            );
            container.capabilities.network =
                SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
                    "torii.sora.internal",
                    [443],
                )]);
            container.capabilities.allow_state_writes = true;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/pii/api/healthz".to_owned());
            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/pii/api".to_owned(),
                service_port: NonZeroU16::new(8788).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(3).expect("nonzero literal");
            service.state_bindings = vec![
                state_binding!(
                    "pii_records",
                    ConfidentialState,
                    AppendOnly,
                    FheCiphertext,
                    "/state/pii/records",
                    65_536,
                    33_554_432
                ),
                state_binding!(
                    "pii_consent_events",
                    ServiceState,
                    AppendOnly,
                    ClientCiphertext,
                    "/state/pii/consent",
                    8_192,
                    8_388_608
                ),
                state_binding!(
                    "pii_retention_jobs",
                    ServiceState,
                    ReadWrite,
                    ClientCiphertext,
                    "/state/pii/retention",
                    4_096,
                    2_097_152
                ),
                auth_state_binding("auth_challenges", "/state/auth/challenges"),
                auth_state_binding("auth_sessions", "/state/auth/sessions"),
            ];
            service.handlers = vec![
                service_handler(
                    "query",
                    SoraServiceHandlerClassV1::Query,
                    "serve_query",
                    Some("/query"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "update",
                    SoraServiceHandlerClassV1::Update,
                    "apply_update",
                    Some("/update"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("updates", 512, 65_536, 1_440)),
                ),
                service_handler(
                    "ciphertext_update",
                    SoraServiceHandlerClassV1::Update,
                    "apply_ciphertext_update",
                    Some("/ciphertext/update"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("ciphertext_updates", 256, 131_072, 2_880)),
                ),
            ];
            service.artifacts = vec![
                service_artifact(
                    SoraArtifactKindV1::Journal,
                    "/journals/pii.journal",
                    Some("update"),
                ),
                service_artifact(
                    SoraArtifactKindV1::Checkpoint,
                    "/checkpoints/pii.chk",
                    Some("ciphertext_update"),
                ),
            ];
            Ok(())
        }
        InitTemplate::HayahiApp => {
            container.runtime = SoraContainerRuntimeV1::Ivm;
            container.bundle_path = "/bundles/hayahi-app-api.to".to_owned();
            container.entrypoint = "main".to_owned();
            container.args = vec!["--http".to_owned(), "--port=8787".to_owned()];
            container
                .env
                .insert("SORACLOUD_TEMPLATE".to_owned(), "hayahi-app".to_owned());
            container
                .env
                .insert("AUTH_MODE".to_owned(), "strict".to_owned());
            container
                .env
                .insert("AUTH_SESSION_TTL_SECS".to_owned(), "900".to_owned());
            container
                .env
                .insert("AUTH_CHALLENGE_TTL_SECS".to_owned(), "120".to_owned());
            container
                .env
                .insert("AUTH_CAPABILITY_MAP_JSON".to_owned(), "{}".to_owned());
            container
                .env
                .insert("PUBLIC_BASE_URL".to_owned(), format!("https://{host}"));
            container.env.insert(
                "HAYAHI_SHARED_STATE_NAMESPACE".to_owned(),
                "hayahi.v1".to_owned(),
            );
            container.env.insert(
                "HAYAHI_COLLECTOR_MODE".to_owned(),
                "soracloud_workers".to_owned(),
            );
            container.capabilities.network = SoraNetworkPolicyV1::Allowlist(vec![
                SoraNetworkAllowlistEntryV1::new("torii.sora.internal", [443]),
                SoraNetworkAllowlistEntryV1::new("wallet.sora.internal", [443]),
            ]);
            container.capabilities.allow_state_writes = true;
            container.capabilities.allow_model_training = false;
            container.lifecycle.healthcheck_path = Some("/api/healthz".to_owned());
            service.route = Some(SoraRouteTargetV1 {
                host,
                path_prefix: "/api".to_owned(),
                service_port: NonZeroU16::new(8787).expect("nonzero literal"),
                visibility: SoraRouteVisibilityV1::Public,
                tls_mode: SoraTlsModeV1::Required,
            });
            service.replicas = NonZeroU16::new(3).expect("nonzero literal");
            service.state_bindings = vec![
                state_binding!(
                    "search_sessions",
                    ServiceState,
                    ReadWrite,
                    ClientCiphertext,
                    "/state/hayahi/search/sessions",
                    131_072,
                    67_108_864
                ),
                state_binding!(
                    "search_cache",
                    ServiceState,
                    ReadWrite,
                    ClientCiphertext,
                    "/state/hayahi/search/cache",
                    524_288,
                    134_217_728
                ),
                state_binding!(
                    "collector_jobs",
                    ServiceState,
                    AppendOnly,
                    ClientCiphertext,
                    "/state/hayahi/collectors/jobs",
                    65_536,
                    33_554_432
                ),
                state_binding!(
                    "collector_results",
                    ServiceState,
                    AppendOnly,
                    ClientCiphertext,
                    "/state/hayahi/collectors/results",
                    262_144,
                    134_217_728
                ),
                auth_state_binding("auth_challenges", "/state/auth/challenges"),
                auth_state_binding("auth_sessions", "/state/auth/sessions"),
                state_binding!(
                    "user_saved_searches",
                    ConfidentialState,
                    ReadWrite,
                    FheCiphertext,
                    "/state/hayahi/users/saved_searches",
                    32_768,
                    8_388_608
                ),
                state_binding!(
                    "user_preferences",
                    ConfidentialState,
                    ReadWrite,
                    FheCiphertext,
                    "/state/hayahi/users/preferences",
                    8_192,
                    4_194_304
                ),
            ];
            service.handlers = vec![
                service_handler(
                    "health",
                    SoraServiceHandlerClassV1::Query,
                    "serve_health",
                    Some("/v1/health"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "state_overview",
                    SoraServiceHandlerClassV1::Query,
                    "serve_state_overview",
                    Some("/v1/state/overview"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "collector_status",
                    SoraServiceHandlerClassV1::Query,
                    "serve_collector_status",
                    Some("/v1/collector/status"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "auth_me",
                    SoraServiceHandlerClassV1::Query,
                    "serve_auth_me",
                    Some("/auth/me"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "user_preferences_get",
                    SoraServiceHandlerClassV1::Query,
                    "serve_user_preferences",
                    Some("/v1/user/preferences"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "saved_searches_list",
                    SoraServiceHandlerClassV1::Query,
                    "serve_saved_searches",
                    Some("/v1/user/saved-searches"),
                    SoraCertifiedResponsePolicyV1::AuditReceipt,
                    None,
                ),
                service_handler(
                    "auth_challenge",
                    SoraServiceHandlerClassV1::Update,
                    "issue_auth_challenge",
                    Some("/auth/challenge"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("auth_updates", 512, 32_768, 1_440)),
                ),
                service_handler(
                    "search_create",
                    SoraServiceHandlerClassV1::Update,
                    "enqueue_search_request",
                    Some("/v1/search"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("search_updates", 1024, 131_072, 1_440)),
                ),
                service_handler(
                    "auth_login",
                    SoraServiceHandlerClassV1::Update,
                    "complete_auth_login",
                    Some("/auth/login"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("auth_updates", 256, 131_072, 2_880)),
                ),
                service_handler(
                    "auth_logout",
                    SoraServiceHandlerClassV1::Update,
                    "close_auth_session",
                    Some("/auth/logout"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("auth_updates", 256, 131_072, 2_880)),
                ),
                service_handler(
                    "user_preferences_put",
                    SoraServiceHandlerClassV1::Update,
                    "store_user_preferences",
                    Some("/v1/user/preferences"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("user_updates", 256, 131_072, 2_880)),
                ),
                service_handler(
                    "saved_searches_create",
                    SoraServiceHandlerClassV1::Update,
                    "store_saved_search",
                    Some("/v1/user/saved-searches"),
                    SoraCertifiedResponsePolicyV1::None,
                    Some(("user_updates", 256, 131_072, 2_880)),
                ),
            ];
            service.artifacts = vec![
                service_artifact(
                    SoraArtifactKindV1::Journal,
                    "/journals/hayahi.journal",
                    Some("search_create"),
                ),
                service_artifact(
                    SoraArtifactKindV1::Checkpoint,
                    "/checkpoints/hayahi.chk",
                    Some("user_preferences_put"),
                ),
            ];
            Ok(())
        }
    }
}
fn scaffold_init_template(
    template: InitTemplate,
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    match template {
        InitTemplate::Baseline => Ok(Vec::new()),
        InitTemplate::HttpService => {
            scaffold_http_service_template(output_dir, service_name, overwrite)
        }
        InitTemplate::Site => scaffold_site_template(output_dir, service_name, overwrite),
        InitTemplate::Webapp => scaffold_webapp_template(output_dir, service_name, overwrite),
        InitTemplate::PiiApp => scaffold_pii_app_template(output_dir, service_name, overwrite),
        InitTemplate::HayahiApp => {
            scaffold_hayahi_app_template(output_dir, service_name, overwrite)
        }
    }
}
fn scaffold_site_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("site");
    let package_name = normalized_service_label(service_name);
    let dns_host = format!("{package_name}.sora");
    let files = vec![
        (
            project_dir.join("package.json"),
            site_package_json(&package_name),
        ),
        (
            project_dir.join("tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("vite.config.ts"),
            site_vite_config().to_owned(),
        ),
        (project_dir.join("index.html"), site_index_html().to_owned()),
        (project_dir.join("src/main.ts"), site_main_ts().to_owned()),
        (project_dir.join("src/App.vue"), site_app_vue(service_name)),
        (
            project_dir.join(".gitignore"),
            "node_modules/\ndist/\n".to_owned(),
        ),
        (
            project_dir.join("README.md"),
            site_readme(service_name, &dns_host),
        ),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_single_api_app_template(
    output_dir: &Path,
    app_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let web_dir = output_dir.join("web");
    let api_dir = output_dir.join("services").join("api");
    let package_name = normalized_service_label(app_name);
    let files = vec![
        (
            web_dir.join("package.json"),
            site_package_json(&package_name),
        ),
        (
            web_dir.join("tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            web_dir.join("vite.config.ts"),
            single_api_frontend_vite_config().to_owned(),
        ),
        (web_dir.join("index.html"), site_index_html().to_owned()),
        (web_dir.join("src/main.ts"), site_main_ts().to_owned()),
        (
            web_dir.join("src/App.vue"),
            single_api_frontend_app_vue(app_name),
        ),
        (
            web_dir.join(".gitignore"),
            "node_modules/\ndist/\n".to_owned(),
        ),
        (
            api_dir.join("contract/api_service.ko"),
            single_api_contract_ko(app_name),
        ),
        (
            api_dir.join("dev-server.mjs"),
            single_api_api_dev_server_mjs(app_name),
        ),
        (api_dir.join("dev.sh"), single_api_api_dev_sh().to_owned()),
        (api_dir.join("build.sh"), single_api_api_build_sh()),
        (
            api_dir.join("verify-build.sh"),
            single_api_api_verify_build_sh(),
        ),
        (api_dir.join(".gitignore"), "build/\ntmp/\n".to_owned()),
        (api_dir.join("README.md"), single_api_api_readme(app_name)),
        (
            output_dir.join("dev.sh"),
            single_api_local_dev_sh().to_owned(),
        ),
        (
            output_dir.join("build-and-sync.sh"),
            single_api_build_and_sync_sh(),
        ),
        (output_dir.join("doctor.sh"), single_api_doctor_sh()),
        (output_dir.join("release.sh"), single_api_release_sh()),
        (
            output_dir.join(".gitignore"),
            "web/node_modules/\nweb/dist/\nservices/api/build/\nservices/api/tmp/\n".to_owned(),
        ),
        (
            output_dir.join("README.md"),
            single_api_app_readme(app_name, &package_name),
        ),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_http_service_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("http-service");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("app/server.mjs"),
            http_service_server_mjs(service_name),
        ),
        (
            project_dir.join("inrou/README.md"),
            http_service_inrou_assets_readme(),
        ),
        (project_dir.join("dev.sh"), http_service_dev_sh().to_owned()),
        (
            project_dir.join("build.sh"),
            http_service_build_sh("http-service.tgz"),
        ),
        (project_dir.join(".gitignore"), "build/\ntmp/\n".to_owned()),
        (
            project_dir.join("README.md"),
            http_service_readme(service_name, &package_name),
        ),
        (
            output_dir.join("dev.sh"),
            http_service_local_dev_sh().to_owned(),
        ),
        (
            output_dir.join("build-and-sync.sh"),
            http_service_build_and_sync_sh("http-service.tgz"),
        ),
        (output_dir.join("doctor.sh"), http_service_doctor_sh()),
        (output_dir.join("release.sh"), http_service_release_sh()),
        (output_dir.join("deploy.sh"), http_service_deploy_sh()),
        (output_dir.join("upgrade.sh"), http_service_upgrade_sh()),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_webapp_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("webapp");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("package.json"),
            webapp_root_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/package.json"),
            webapp_frontend_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("frontend/vite.config.ts"),
            webapp_frontend_vite_config().to_owned(),
        ),
        (
            project_dir.join("frontend/index.html"),
            site_index_html().to_owned(),
        ),
        (
            project_dir.join("frontend/src/main.ts"),
            site_main_ts().to_owned(),
        ),
        (
            project_dir.join("frontend/src/App.vue"),
            webapp_frontend_app_vue(service_name),
        ),
        (project_dir.join("api/server.mjs"), webapp_api_server_mjs()),
        (project_dir.join("README.md"), webapp_readme(service_name)),
        (
            project_dir.join(".gitignore"),
            "node_modules/\nfrontend/node_modules/\nfrontend/dist/\n".to_owned(),
        ),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_pii_app_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("pii-app");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("package.json"),
            pii_app_root_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/package.json"),
            pii_app_frontend_package_json(&package_name),
        ),
        (
            project_dir.join("frontend/tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            project_dir.join("frontend/vite.config.ts"),
            pii_app_frontend_vite_config().to_owned(),
        ),
        (
            project_dir.join("frontend/index.html"),
            site_index_html().to_owned(),
        ),
        (
            project_dir.join("frontend/src/main.ts"),
            site_main_ts().to_owned(),
        ),
        (
            project_dir.join("frontend/src/App.vue"),
            pii_app_frontend_app_vue(service_name),
        ),
        (project_dir.join("api/server.mjs"), pii_app_api_server_mjs()),
        (
            project_dir.join("policy/consent_policy_template.json"),
            pii_app_consent_policy_template(),
        ),
        (
            project_dir.join("policy/retention_policy_template.json"),
            pii_app_retention_policy_template(),
        ),
        (
            project_dir.join("policy/deletion_workflow_template.json"),
            pii_app_deletion_workflow_template(),
        ),
        (
            project_dir.join(".gitignore"),
            "node_modules/\nfrontend/node_modules/\nfrontend/dist/\n".to_owned(),
        ),
        (project_dir.join("README.md"), pii_app_readme(service_name)),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_hayahi_app_template(
    output_dir: &Path,
    service_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let project_dir = output_dir.join("hayahi-app");
    let package_name = normalized_service_label(service_name);
    let files = vec![
        (
            project_dir.join("package.json"),
            hayahi_app_root_package_json(&package_name),
        ),
        (
            project_dir.join("contract/hayahi_api.ko"),
            hayahi_app_contract_ko(service_name),
        ),
        (project_dir.join("build.sh"), hayahi_app_build_sh()),
        (project_dir.join(".gitignore"), "build/\n".to_owned()),
        (
            project_dir.join("README.md"),
            hayahi_app_readme(service_name),
        ),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_split_app_template(
    output_dir: &Path,
    app_name: &str,
    overwrite: bool,
    existing_repo: bool,
) -> Result<Vec<String>> {
    if existing_repo {
        return scaffold_split_app_existing_repo_template(output_dir, app_name, overwrite);
    }
    let frontend_dir = output_dir.join("frontend");
    let live_dir = output_dir.join("services").join("live");
    let vault_dir = output_dir.join("services").join("vault");
    let package_name = normalized_service_label(app_name);
    let files = vec![
        (
            frontend_dir.join("package.json"),
            split_app_frontend_package_json(&package_name),
        ),
        (
            frontend_dir.join("tsconfig.json"),
            site_tsconfig_json().to_owned(),
        ),
        (
            frontend_dir.join("vite.config.ts"),
            split_app_frontend_vite_config().to_owned(),
        ),
        (
            frontend_dir.join("scripts/validate-production-env.mjs"),
            split_app_frontend_validate_production_env_mjs().to_owned(),
        ),
        (
            frontend_dir.join("index.html"),
            site_index_html().to_owned(),
        ),
        (frontend_dir.join("src/main.ts"), site_main_ts().to_owned()),
        (
            frontend_dir.join("src/App.vue"),
            split_app_frontend_app_vue(app_name),
        ),
        (
            live_dir.join("app/server.mjs"),
            split_app_live_server_mjs(&format!("{app_name} live")),
        ),
        (
            live_dir.join("inrou/README.md"),
            http_service_inrou_assets_readme(),
        ),
        (live_dir.join("inrou/x86_64/.gitkeep"), String::new()),
        (live_dir.join("inrou/aarch64/.gitkeep"), String::new()),
        (live_dir.join("dev.sh"), http_service_dev_sh().to_owned()),
        (
            live_dir.join("build.sh"),
            http_service_build_sh("live-api.tgz"),
        ),
        (live_dir.join(".gitignore"), "build/\ntmp/\n".to_owned()),
        (live_dir.join("README.md"), split_app_live_readme(app_name)),
        (
            vault_dir.join("contract/vault_api.ko"),
            split_app_vault_contract_ko(app_name),
        ),
        (
            vault_dir.join("dev-server.mjs"),
            split_app_vault_dev_server_mjs(app_name),
        ),
        (vault_dir.join("dev.sh"), split_app_vault_dev_sh().to_owned()),
        (vault_dir.join("build.sh"), split_app_vault_build_sh()),
        (
            vault_dir.join("verify-build.sh"),
            split_app_vault_verify_build_sh(),
        ),
        (vault_dir.join(".gitignore"), "build/\ntmp/\n".to_owned()),
        (
            vault_dir.join("README.md"),
            split_app_vault_readme(app_name),
        ),
        (
            output_dir.join("dev.sh"),
            split_app_local_dev_sh().to_owned(),
        ),
        (
            output_dir.join("build-and-sync.sh"),
            split_app_build_and_sync_sh(),
        ),
        (output_dir.join("doctor.sh"), split_app_doctor_sh()),
        (output_dir.join("release.sh"), split_app_release_sh()),
        (
            output_dir.join(".gitignore"),
            "frontend/node_modules/\nfrontend/dist/\nservices/live/build/\nservices/live/tmp/\nservices/vault/build/\nservices/vault/tmp/\n"
                .to_owned(),
        ),
        (
            output_dir.join("README.md"),
            split_app_readme(app_name, &package_name),
        ),
    ];
    write_template_files(files, overwrite)
}
fn scaffold_split_app_existing_repo_template(
    output_dir: &Path,
    app_name: &str,
    overwrite: bool,
) -> Result<Vec<String>> {
    let files = vec![
        (
            output_dir.join("build-and-sync.sh"),
            split_app_existing_repo_build_and_sync_sh(),
        ),
        (output_dir.join("doctor.sh"), split_app_doctor_sh()),
        (output_dir.join("release.sh"), split_app_release_sh()),
        (
            output_dir.join(".gitignore"),
            "services/live/build/\nservices/live/tmp/\nservices/vault/build/\nservices/vault/tmp/\n"
                .to_owned(),
        ),
        (
            output_dir.join("README.md"),
            split_app_existing_repo_readme(app_name),
        ),
    ];
    write_template_files(files, overwrite)
}
fn write_template_files(files: Vec<(PathBuf, String)>, overwrite: bool) -> Result<Vec<String>> {
    let mut written = Vec::with_capacity(files.len());
    for (path, body) in files {
        write_template_file(&path, &body, overwrite)?;
        written.push(path.to_string_lossy().into_owned());
    }
    Ok(written)
}
fn write_template_file(path: &Path, body: &str, overwrite: bool) -> Result<()> {
    ensure_can_write(path, overwrite)?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create template directory {}", parent.display())
        })?;
    }
    let body = normalize_template_shell_vars(body);
    fs::write(path, body)
        .wrap_err_with(|| format!("failed to write template file {}", path.display()))?;
    mark_template_file_executable(path)?;
    Ok(())
}
fn normalize_template_shell_vars(body: &str) -> String {
    body.replace("${{BASH_SOURCE[0]}}", "${BASH_SOURCE[0]}")
}
fn mark_template_file_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let extension = path.extension().and_then(|value| value.to_str());
        if extension == Some("sh") {
            let mut permissions = fs::metadata(path)
                .wrap_err_with(|| format!("failed to stat template file {}", path.display()))?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).wrap_err_with(|| {
                format!("failed to mark template file executable {}", path.display())
            })?;
        }
    }
    Ok(())
}
fn normalized_service_label(service_name: &str) -> String {
    let mut out = String::new();
    for ch in service_name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch == '-' || ch == '_') && !out.ends_with('-') {
            out.push('-');
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "sora-app".to_owned()
    } else {
        out
    }
}
fn normalized_contract_identifier(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    while out.starts_with('_') {
        out.remove(0);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "sora_contract".to_owned()
    } else if out
        .chars()
        .next()
        .expect("nonempty contract identifier")
        .is_ascii_digit()
    {
        format!("sora_{out}")
    } else {
        out
    }
}

const TEMPLATE_PACKAGE_NAME: &str = "__SORACLOUD_PACKAGE_NAME__";
const TEMPLATE_SERVICE_NAME: &str = "__SORACLOUD_SERVICE_NAME__";
const TEMPLATE_SERVICE_NAME_DEBUG: &str = "__SORACLOUD_SERVICE_NAME_DEBUG__";
const TEMPLATE_APP_NAME: &str = "__SORACLOUD_APP_NAME__";
const TEMPLATE_APP_NAME_DEBUG: &str = "__SORACLOUD_APP_NAME_DEBUG__";
const TEMPLATE_BUNDLE_NAME: &str = "__SORACLOUD_BUNDLE_NAME__";
const TEMPLATE_SHELL_PRELUDE: &str = "__SORACLOUD_SHELL_PRELUDE__";
const TEMPLATE_SEIYAKU_NAME: &str = "__SORACLOUD_SEIYAKU_NAME__";
const TEMPLATE_DNS_HOST: &str = "__SORACLOUD_DNS_HOST__";

fn render_template(template: &str, substitutions: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut remaining = template;
    loop {
        let mut next_substitution = None;
        for &(placeholder, replacement) in substitutions {
            if let Some(offset) = remaining.find(placeholder)
                && next_substitution.is_none_or(|(best_offset, _, _)| offset < best_offset)
            {
                next_substitution = Some((offset, placeholder, replacement));
            }
        }
        let Some((offset, placeholder, replacement)) = next_substitution else {
            rendered.push_str(remaining);
            return rendered;
        };
        rendered.push_str(&remaining[..offset]);
        rendered.push_str(replacement);
        remaining = &remaining[offset + placeholder.len()..];
    }
}

fn site_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/site_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn webapp_root_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/webapp_root_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn webapp_frontend_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/webapp_frontend_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn pii_app_root_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/pii_app_root_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn pii_app_frontend_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/pii_app_frontend_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn hayahi_app_root_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/hayahi_app_root_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn site_tsconfig_json() -> &'static str {
    include_str!("soracloud/templates/v1/site_tsconfig.json")
}
fn site_vite_config() -> &'static str {
    include_str!("soracloud/templates/v1/site_vite.config.ts")
}
fn single_api_frontend_vite_config() -> &'static str {
    include_str!("soracloud/templates/v1/single_api_frontend_vite.config.ts")
}
fn webapp_frontend_vite_config() -> &'static str {
    include_str!("soracloud/templates/v1/webapp_frontend_vite.config.ts")
}
fn pii_app_frontend_vite_config() -> &'static str {
    include_str!("soracloud/templates/v1/pii_app_frontend_vite.config.ts")
}
fn site_index_html() -> &'static str {
    include_str!("soracloud/templates/v1/site_index.html")
}
fn site_main_ts() -> &'static str {
    include_str!("soracloud/templates/v1/site_main.ts")
}
fn site_app_vue(service_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/site_app_vue.tmpl"),
        &[(TEMPLATE_SERVICE_NAME, service_name)],
    )
}
fn single_api_frontend_app_vue(app_name: &str) -> String {
    include_str!("soracloud/templates/v1/static/single_api_frontend_app.vue")
        .replace("__APP_NAME__", app_name)
}
fn webapp_frontend_app_vue(service_name: &str) -> String {
    include_str!("soracloud/templates/v1/static/webapp_frontend_app.vue")
        .replace("__SERVICE_NAME__", service_name)
}
fn single_api_contract_ko(app_name: &str) -> String {
    let seiyaku_name = format!("{}_api_service", normalized_contract_identifier(app_name));
    include_str!("soracloud/templates/v1/static/single_api_contract.ko")
        .replace("__CONTRACT_NAME__", &seiyaku_name)
        .replace("__APP_NAME__", app_name)
}
fn pii_app_frontend_app_vue(service_name: &str) -> String {
    include_str!("soracloud/templates/v1/static/pii_app_frontend_app.vue")
        .replace("__SERVICE_NAME__", service_name)
}
fn hayahi_app_contract_ko(service_name: &str) -> String {
    include_str!("soracloud/templates/v1/static/hayahi_app_contract.ko")
        .replace("__CONTRACT_NAME__", "HayahiSoracloudCore")
        .replace("__SERVICE_NAME__", service_name)
}
fn soracloud_auth_core_mjs() -> &'static str {
    include_str!("soracloud/templates/v1/soracloud_auth_core.mjs")
}
const WEBAPP_API_TAIL_V1: &str = include_str!("soracloud/assets/v1/webapp_api_tail.mjs");
const PII_API_TAIL_V1: &str = include_str!("soracloud/assets/v1/pii_api_tail.mjs");
fn webapp_api_server_mjs() -> String {
    let mut script = String::from(soracloud_auth_core_mjs());
    script.push_str(WEBAPP_API_TAIL_V1);
    script
}
fn pii_app_api_server_mjs() -> String {
    let mut script = String::from(soracloud_auth_core_mjs());
    script.push_str(PII_API_TAIL_V1);
    script
}
fn single_api_api_build_sh() -> String {
    include_str!("soracloud/templates/v1/single_api_api_build.sh").to_owned()
}
fn single_api_api_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/single_api_api_dev.sh")
}
fn single_api_api_dev_server_mjs(app_name: &str) -> String {
    let app_name_debug = format!("{app_name:?}");
    render_template(
        include_str!("soracloud/assets/v1/single_api_api_dev_server_mjs.tmpl"),
        &[(TEMPLATE_APP_NAME_DEBUG, &app_name_debug)],
    )
}
fn single_api_api_verify_build_sh() -> String {
    include_str!("soracloud/templates/v1/single_api_api_verify_build.sh").to_owned()
}
fn hayahi_app_build_sh() -> String {
    include_str!("soracloud/templates/v1/hayahi_app_build.sh").to_owned()
}
fn http_service_build_sh(bundle_name: &str) -> String {
    let prelude = iroha_shell_command_prelude();
    render_template(
        include_str!("soracloud/assets/v1/http_service_build_sh.tmpl"),
        &[
            (TEMPLATE_BUNDLE_NAME, bundle_name),
            (TEMPLATE_SHELL_PRELUDE, prelude),
        ],
    )
}
fn http_service_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/http_service_dev.sh")
}
fn http_service_local_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/http_service_local_dev.sh")
}
fn iroha_shell_command_prelude() -> &'static str {
    include_str!("soracloud/templates/v1/iroha_shell_command_prelude.sh")
}
fn http_service_build_and_sync_sh(bundle_name: &str) -> String {
    let prelude = iroha_shell_command_prelude();
    render_template(
        include_str!("soracloud/assets/v1/http_service_build_and_sync_sh.tmpl"),
        &[
            (TEMPLATE_BUNDLE_NAME, bundle_name),
            (TEMPLATE_SHELL_PRELUDE, prelude),
        ],
    )
}
fn http_service_doctor_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/http_service_doctor.sh")
        .replace("{prelude}", prelude)
}
fn http_service_release_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/http_service_release.sh")
        .replace("{prelude}", prelude)
}
fn http_service_deploy_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/http_service_deploy.sh")
        .replace("{prelude}", prelude)
}
fn http_service_upgrade_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/http_service_upgrade.sh")
        .replace("{prelude}", prelude)
}
fn http_service_server_mjs(service_name: &str) -> String {
    let service_name_debug = format!("{service_name:?}");
    let rendered = render_template(
        include_str!("soracloud/assets/v1/http_service_server_mjs.tmpl"),
        &[(TEMPLATE_SERVICE_NAME_DEBUG, &service_name_debug)],
    );
    let declaration_anchor = format!("const SERVICE_NAME = {service_name_debug};");
    assert_eq!(
        rendered.matches(&declaration_anchor).count(),
        1,
        "HTTP service template must contain exactly one service-name declaration anchor"
    );
    let declaration_replacement = format!(
        "{declaration_anchor}\nconst SERVICE_VERSION = process.env.SORACLOUD_SERVICE_VERSION;\nif (typeof SERVICE_VERSION !== \"string\" || SERVICE_VERSION.trim().length === 0) {{\n  throw new Error(\"SORACLOUD_SERVICE_VERSION is required\");\n}}"
    );
    let rendered = rendered.replacen(&declaration_anchor, &declaration_replacement, 1);
    const HEALTH_ANCHOR: &str = "      service: SERVICE_NAME,\n      runtime: \"Inrou\",";
    assert_eq!(
        rendered.matches(HEALTH_ANCHOR).count(),
        1,
        "HTTP service template must contain exactly one health-identity anchor"
    );
    rendered.replacen(
        HEALTH_ANCHOR,
        "      service: SERVICE_NAME,\n      service_version: SERVICE_VERSION,\n      runtime: \"Inrou\",",
        1,
    )
}
fn split_app_live_server_mjs(service_name: &str) -> String {
    let service_name_debug = format!("{service_name:?}");
    render_template(
        include_str!("soracloud/assets/v1/split_app_live_server_mjs.tmpl"),
        &[(TEMPLATE_SERVICE_NAME_DEBUG, &service_name_debug)],
    )
}
fn http_service_readme(service_name: &str, package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/http_service_readme.tmpl"),
        &[
            (TEMPLATE_SERVICE_NAME, service_name),
            (TEMPLATE_PACKAGE_NAME, package_name),
        ],
    )
}
fn http_service_inrou_assets_readme() -> String {
    include_str!("soracloud/templates/v1/http_service_inrou_assets_readme.md").to_owned()
}
fn split_app_frontend_package_json(package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_frontend_package_json.tmpl"),
        &[(TEMPLATE_PACKAGE_NAME, package_name)],
    )
}
fn split_app_frontend_validate_production_env_mjs() -> &'static str {
    include_str!("soracloud/templates/v1/split_app_frontend_validate_production_env.mjs")
}
fn split_app_frontend_vite_config() -> &'static str {
    include_str!("soracloud/templates/v1/split_app_frontend_vite.config.ts")
}
fn split_app_frontend_app_vue(app_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_frontend_app_vue.tmpl"),
        &[(TEMPLATE_APP_NAME, app_name)],
    )
}
fn split_app_vault_build_sh() -> String {
    include_str!("soracloud/templates/v1/split_app_vault_build.sh").to_owned()
}
fn split_app_vault_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/split_app_vault_dev.sh")
}
fn split_app_vault_verify_build_sh() -> String {
    include_str!("soracloud/templates/v1/split_app_vault_verify_build.sh").to_owned()
}
fn split_app_vault_dev_server_mjs(app_name: &str) -> String {
    let app_name_debug = format!("{app_name:?}");
    render_template(
        include_str!("soracloud/assets/v1/split_app_vault_dev_server_mjs.tmpl"),
        &[(TEMPLATE_APP_NAME_DEBUG, &app_name_debug)],
    )
}
fn split_app_vault_contract_ko(app_name: &str) -> String {
    let seiyaku_name = format!("{}_vault_api", normalized_contract_identifier(app_name));
    render_template(
        include_str!("soracloud/assets/v1/split_app_vault_contract_ko.tmpl"),
        &[(TEMPLATE_SEIYAKU_NAME, &seiyaku_name)],
    )
}
fn split_app_live_readme(app_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_live_readme.tmpl"),
        &[(TEMPLATE_APP_NAME, app_name)],
    )
}
fn split_app_vault_readme(app_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_vault_readme.tmpl"),
        &[(TEMPLATE_APP_NAME, app_name)],
    )
}
fn split_app_local_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/split_app_local_dev.sh")
}
fn split_app_build_and_sync_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/split_app_build_and_sync.sh")
        .replace("{prelude}", prelude)
}
fn split_app_existing_repo_build_and_sync_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/split_app_existing_repo_build_and_sync.sh")
        .replace("{prelude}", prelude)
}
fn split_app_doctor_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/split_app_doctor.sh").replace("{prelude}", prelude)
}
fn split_app_release_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/split_app_release.sh").replace("{prelude}", prelude)
}
fn split_app_readme(app_name: &str, package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_readme.tmpl"),
        &[
            (TEMPLATE_APP_NAME, app_name),
            (TEMPLATE_PACKAGE_NAME, package_name),
        ],
    )
}
fn split_app_existing_repo_readme(app_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/split_app_existing_repo_readme.tmpl"),
        &[(TEMPLATE_APP_NAME, app_name)],
    )
}
fn pii_app_consent_policy_template() -> String {
    include_str!("soracloud/templates/v1/pii_app_consent_policy.json").to_owned()
}
fn pii_app_retention_policy_template() -> String {
    include_str!("soracloud/templates/v1/pii_app_retention_policy.json").to_owned()
}
fn pii_app_deletion_workflow_template() -> String {
    include_str!("soracloud/templates/v1/pii_app_deletion_workflow.json").to_owned()
}
fn site_readme(service_name: &str, dns_host: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/site_readme.tmpl"),
        &[
            (TEMPLATE_SERVICE_NAME, service_name),
            (TEMPLATE_DNS_HOST, dns_host),
        ],
    )
}
fn single_api_api_readme(app_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/single_api_api_readme.tmpl"),
        &[(TEMPLATE_APP_NAME, app_name)],
    )
}
fn single_api_local_dev_sh() -> &'static str {
    include_str!("soracloud/templates/v1/single_api_local_dev.sh")
}
fn single_api_build_and_sync_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/single_api_build_and_sync.sh")
        .replace("{prelude}", prelude)
}
fn single_api_doctor_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/single_api_doctor.sh").replace("{prelude}", prelude)
}
fn single_api_release_sh() -> String {
    let prelude = iroha_shell_command_prelude();
    include_str!("soracloud/templates/v1/static/single_api_release.sh")
        .replace("{prelude}", prelude)
}
fn single_api_app_readme(app_name: &str, package_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/single_api_app_readme.tmpl"),
        &[
            (TEMPLATE_APP_NAME, app_name),
            (TEMPLATE_PACKAGE_NAME, package_name),
        ],
    )
}
fn webapp_readme(service_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/webapp_readme.tmpl"),
        &[(TEMPLATE_SERVICE_NAME, service_name)],
    )
}
fn pii_app_readme(service_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/pii_app_readme.tmpl"),
        &[(TEMPLATE_SERVICE_NAME, service_name)],
    )
}
fn hayahi_app_readme(service_name: &str) -> String {
    render_template(
        include_str!("soracloud/assets/v1/hayahi_app_readme.tmpl"),
        &[(TEMPLATE_SERVICE_NAME, service_name)],
    )
}
#[cfg(test)]
#[path = "soracloud/tests.rs"]
mod tests;
