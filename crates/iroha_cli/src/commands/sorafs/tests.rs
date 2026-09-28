use super::*;
use crate::CliOutputFormat;
use blake3::Hasher as Blake3Hasher;
use ed25519_dalek::{SigningKey, VerifyingKey as Ed25519VerifyingKey};
use iroha::{
    config::{self, Config},
    crypto::KeyPair,
    data_model::prelude::AccountId,
};
use iroha_config::{
    base::{read::ConfigReader, toml::TomlSource},
    parameters::user::Sorafs as UserSorafsConfig,
};
use iroha_crypto::{
    Algorithm, PublicKey,
    soranet::{
        certificate::{
            CapabilityToggle, RelayCapabilityFlagsV1, RelayCertificateV2, RelayEndpointV2,
            RelayRolesV2,
        },
        directory::{
            GuardDirectoryIssuerV1, GuardDirectoryRelayEntryV2, GuardDirectorySnapshotV2,
            compute_issuer_fingerprint,
        },
        handshake::HandshakeSuite,
        token::{self, AdmissionTokenVerifier, InMemoryTokenStore, TokenStore, TokenStoreLimits},
    },
};
use iroha_data_model::{
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, TransferBox},
    soranet::incentives::{
        RelayBondLedgerEntryV1, RelayBondPolicyV1, RelayComplianceStatusV1, RelayEpochMetricsV1,
        RelayRewardDisputeV1, RelayRewardInstructionV1,
    },
};
use iroha_i18n::{Bundle, Language, Localizer};
use iroha_model_base::chain::ChainId;
use iroha_model_base::metadata::Metadata;
use iroha_primitives::numeric::Quantity;
use norito::json::{Map, Value};
use norito::{decode_from_bytes, json::JsonSerialize, to_bytes};
use rand::{
    RngCore, SeedableRng,
    rand_core::{TryCryptoRng, TryRngCore},
    rngs::StdRng,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1, DagCodecId, ManifestBuilder, PinPolicy,
    ProfileId, StorageClass as ManifestStorageClass,
};
use sorafs_orchestrator::soranet::EndpointTag;
use sorafs_orchestrator::{incentives::RewardConfig, treasury::ExpectedLedgerTransfer};
use soranet_pq::{MlDsaSuite, generate_mldsa_keypair_from_os as generate_mldsa_keypair};
use std::{
    fmt::{self, Display},
    fs,
    io::Write,
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tempfile::{NamedTempFile, TempDir};
use url::Url;

include!("tests/part_01.rs");
include!("tests/part_02.rs");
include!("tests/part_03.rs");
