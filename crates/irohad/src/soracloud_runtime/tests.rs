//! Tests for the embedded Soracloud runtime manager.
use super::*;
use eyre::Result;
use iroha_core::{kura::Kura, query::store::LiveQueryStore, smartcontracts::Execute, state::World};
use iroha_crypto::{Algorithm, BlsNormal, KeyGenOption, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::{
    Level,
    block::BlockHeader,
    isi::Log,
    smart_contract::manifest::EntryPointKind,
    soracloud::{
        AgentApartmentManifestV1, SECRET_ENVELOPE_VERSION_V1,
        SORA_AGENT_APARTMENT_RECORD_VERSION_V1, SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
        SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1, SORA_SERVICE_ROLLOUT_STATE_VERSION_V1,
        SORA_SERVICE_RUNTIME_STATE_VERSION_V1, SecretEnvelopeEncryptionV1, SecretEnvelopeV1,
        SoraAgentPersistentStateV1, SoraContainerRuntimeV1, SoraDeploymentBundleV1,
        SoraInrouGuestImageV1, SoraPublishedInrouGuestImageArtifactV1, SoraRolloutStageV1,
        SoraServiceConfigEntryV1, SoraServiceDeploymentStateV1, SoraServiceHandlerClassV1,
        SoraServiceHealthStatusV1, SoraServiceLeaseClockV1, SoraServiceMailboxMessageV1,
        SoraServiceRolloutStateV1, SoraServiceRuntimeStateV1, SoraServiceSecretEntryV1,
    },
    sorafs::pin_registry::{
        ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinFeePayment, PinManifestRecord,
        PinPolicy, ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus,
    },
};
use iroha_futures::supervisor::Supervisor;
use iroha_model_base::metadata::Metadata;
use iroha_primitives::{json::Json, numeric::Quantity};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID};
use iroha_torii::sorafs::AdmissionRegistry;
use sorafs_car::{
    CAR_PLAN_MAX_CHUNKS, CarBuildPlan, CarChunk, CarStreamingWriter, ChunkStore, FileEntry,
    FilePlan,
    bundle_archive::{BundleArchiveFile, write_gzip_ustar},
    compute_chunk_plan_digest_sha3,
};
use sorafs_chunker::{ChunkProfile, Chunker};
use sorafs_manifest::{
    AdvertEndpoint, AvailabilityTier, BLAKE3_256_MULTIHASH_CODE, CapabilityTlv, CapabilityType,
    CouncilSignature, DagCodecId, EndpointAdmissionV1, EndpointAttestationKind,
    EndpointAttestationV1, EndpointMetadata, EndpointMetadataKey, ManifestBuilder,
    PROVIDER_ADMISSION_ENVELOPE_VERSION_V1, PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
    PROVIDER_ADVERT_VERSION_V1, PathDiversityPolicy, PinPolicy as ManifestPinPolicy,
    ProviderAdmissionCouncilPolicy, ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1,
    ProviderAdvertBodyV1, ProviderAdvertV1, ProviderCapabilityRangeV1, ProviderVrfPublicKeyV1,
    QosHints, RendezvousTopic, SignatureAlgorithm, StakePointer, StreamBudgetV1, TransportHintV1,
    XorQuantity, compute_advert_body_digest, compute_envelope_authorization_digest,
    compute_proposal_digest,
};
use std::{
    io::{BufReader, Read},
    net::TcpListener,
    num::NonZeroU64,
    sync::{Arc, Mutex, OnceLock, mpsc},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

include!("tests/part_01.rs");
include!("tests/part_02.rs");
include!("tests/part_03.rs");
include!("tests/part_04.rs");
include!("tests/part_05.rs");
include!("tests/part_06.rs");
