//! Startup catalog for authenticated Kagemusha V4 verifier material.
//!
//! V4 has its own state namespaces, release-record schema, KRV4 framing, and
//! verifier identity. Nothing in this module accepts or upgrades the V3
//! registry representation. Release policy comes from canonical configured
//! Norito; consensus state can select material, but cannot select its signers.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
use rustix::fs::{
    AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, fcntl_getfl, open, openat, statat,
};
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Seek as _, SeekFrom},
    os::unix::fs::MetadataExt as _,
    sync::Mutex,
};

use iroha_crypto::Hash;
use iroha_data_model::{
    confidential::ConfidentialStatus,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4, KAGEMUSHA_VERIFIER_NAMESPACE,
        KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactKindV4,
        KagemushaPastaCycleArtifactV4, KagemushaPastaCycleParityV1,
        KagemushaRecursiveSpendArtifactBindingV4, KagemushaRecursiveSpendArtifactManifestV4,
        KagemushaRecursiveSpendReleaseActivationV4, KagemushaRecursiveSpendReleaseAttestationV4,
        KagemushaRecursiveSpendReleasePolicyV1, KagemushaStepCircuitParamsV4,
    },
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
    state_path::StatePath,
    zk::BackendTag,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use crate::zk::{
    kagemusha_artifact_source_v4::{
        KagemushaArtifactReadSeekV4, KagemushaAuthenticatedArtifactSourceV4,
        KagemushaQualifiedArtifactSourceV4, KagemushaQualifiedParityMetadataV4,
        qualify_kagemusha_authenticated_artifact_source_v4,
    },
    kagemusha_artifact_v4::{
        KagemushaAuthenticatedArtifactInspectionV4, inspect_kagemusha_pasta_cycle_artifact_v4,
        kagemusha_artifact_descriptor_v4,
    },
    kagemusha_recursion_adapter::kagemusha_artifact_encoding_sizes_v4,
    kagemusha_v2::KagemushaPastaCycleOpaqueVerifierV4,
};

pub(crate) const TERMINAL_RELEASE_STATE_KEY_PREFIX_V4: &str = "kagemusha_terminal_release_v4_";
const VERIFIER_OWNER_MANIFEST_PREFIX_V4: &str = "kagemusha-v4-";
const VERIFIER_IDENTITY_SCHEMA_V4: &str = "kagemusha.offline.recursive_spend.verifier_identity.v4";
const VERIFIER_IDENTITY_VERSION_V4: u16 = 4;
const STEP_EQ_VERIFIER_CURVE_V4: &str = "vesta";
const STEP_EP_VERIFIER_CURVE_V4: &str = "pallas";
const MAX_POLICY_BYTES: usize = 64 * 1024;
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
const MANIFEST_FILE_NAME_V4: &str = "manifest.norito";
const MANIFEST_JSON_FILE_NAME_V4: &str = "manifest.json";
const MANIFEST_SHA256_FILE_NAME_V4: &str = "manifest.norito.sha256";
const PROMOTION_RECORD_FILE_NAME_V4: &str = "promotion-record-v4.norito";
const KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len();
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1: &str =
    "iroha.kagemusha.catalog_qualification_seal.v1";
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1: u16 = 1;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1: usize = 1024;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_RELEASES_V1: usize = 16;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_BUILD_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:catalog-qualification-seal:build:v1\0";
#[cfg(test)]
std::thread_local! {
    static KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
const MAX_CATALOG_DIRECTORY_ENTRIES: usize = 1024;
/// Maximum number of historic authenticated releases retained in one runtime.
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
const MAX_CATALOG_RELEASES_V4: usize = 16;
/// Maximum on-disk bytes described by all exact release inventories combined.
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
const MAX_CATALOG_AGGREGATE_BYTES_V4: u64 = 12 * 1024 * 1024 * 1024;
/// Default decoded-resident ceiling used by non-daemon catalog callers.
pub const DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4: u64 = 256 * 1024 * 1024;
/// `ParamsIPA` retains two vectors of 64-byte Pasta affine points per domain row.
const PARSED_PARAMS_BYTES_PER_ROW_V4: u64 = 2 * 64;
/// Conservative expansion from compressed verifier-key bytes to parsed points.
const PARSED_VERIFYING_KEY_EXPANSION_V4: u64 = 2;
/// Conservative retained cost of Halo2's verifier-key evaluation domain.
///
/// The vendored domain owns forward and inverse FFT twiddle tables for the
/// base and extended domains. Charging 512 bytes per base-domain row covers
/// those tables, their vector metadata, and construction scratch without
/// pretending that the tiny serialized VK is representative of its decoded
/// footprint.
const PARSED_VERIFYING_KEY_DOMAIN_BYTES_PER_ROW_V4: u64 = 512;
/// Small authenticated objects retained in several catalog/verifier owners.
const CATALOG_RELEASE_METADATA_PERSISTENT_BYTES_V4: u64 = (3 * MAX_MANIFEST_BYTES
    + MAX_ATTESTATION_BYTES
    + 2 * KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    + KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4)
    as u64;
/// Metadata parsing scratch that can overlap verifier parsing.
const CATALOG_RELEASE_METADATA_TRANSIENT_BYTES_V4: u64 = (3 * MAX_MANIFEST_BYTES
    + MAX_POLICY_BYTES
    + MAX_ATTESTATION_BYTES
    + KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4)
    as u64;
/// Extra allocator/metadata headroom applied to decoded catalog estimates.
const DECODED_ESTIMATE_HEADROOM_NUMERATOR_V4: u64 = 5;
const DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4: u64 = 4;

/// Canonical identity committed by each V4 verifier registry record.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaTerminalVerifierIdentityV4 {
    schema: String,
    version: u16,
    manifest_sha256: [u8; 32],
    parity: KagemushaPastaCycleParityV1,
    circuit_id: String,
    circuit_params_sha256: [u8; 32],
    compiled_protocol_structure_sha256: [u8; 32],
    public_input_limbs: u32,
}

/// Readiness-safe identity derived only from an authenticated V4 release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaAuthenticatedArtifactSetV4 {
    pub(crate) generation: String,
    pub(crate) manifest_sha256: [u8; 32],
    pub(crate) release_policy_sha256: [u8; 32],
    pub(crate) release_attestation_sha256: [u8; 32],
    pub(crate) activation_height: u64,
    pub(crate) withdrawal_height: u64,
    pub(crate) max_proof_bytes: u32,
    pub(crate) asset_scale: u32,
}

/// Qualified exact-eight source and source-backed terminal verifier.
///
/// No domain-sized Halo2 parameters or keys are retained here. The qualified
/// source owns the pinned release and light per-parity identities; the opaque
/// facade loads and drops one parity for each terminal decision.
pub(crate) struct ResolvedKagemushaTerminalVerifierV4 {
    qualified_source: Arc<KagemushaQualifiedArtifactSourceV4>,
    verifier: Arc<KagemushaPastaCycleOpaqueVerifierV4>,
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    pinned_source: Arc<KagemushaCatalogPinnedArtifactSourceV4>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KagemushaCatalogMemoryEstimateV4 {
    persistent_bytes: u64,
    peak_load_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
enum KagemushaCatalogSealedPathKindV1 {
    Directory,
    File,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCatalogSealedStatV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCatalogSealedPathV1 {
    canonical_path: String,
    kind: KagemushaCatalogSealedPathKindV1,
    stat: KagemushaCatalogSealedStatV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCatalogSealedArtifactDigestV1 {
    parity: KagemushaPastaCycleParityV1,
    artifact: KagemushaPastaCycleArtifactV4,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCatalogSealedParityQualificationV1 {
    parity: KagemushaPastaCycleParityV1,
    circuit_params: KagemushaStepCircuitParamsV4,
    compiled_protocol_structure_sha256: [u8; 32],
    compiled_protocol_identity_sha256: [u8; 32],
    processed_verifying_key_len: u64,
    processed_verifying_key_sha256: [u8; 32],
    verifying_key_commitment: [u8; 32],
    proving_key_embedded_verifying_key_sha256: [u8; 32],
    proving_key_fixed_columns: u64,
    proving_key_permutation_columns: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct KagemushaCatalogSealedReleaseQualificationV1 {
    manifest_sha256: [u8; 32],
    release_attestation_sha256: [u8; 32],
    source_commit: String,
    source_tree_sha256: [u8; 32],
    reviewed_source_closure_descriptor_sha256: [u8; 32],
    benchmark_evidence_sha256: [u8; 32],
    cryptographic_review_sha256: [u8; 32],
    promotion_record_sha256: [u8; 32],
    artifacts: Vec<KagemushaCatalogSealedArtifactDigestV1>,
    step_eq: KagemushaCatalogSealedParityQualificationV1,
    step_ep: KagemushaCatalogSealedParityQualificationV1,
}

/// Root-trusted proof that one exact Kagemusha catalog completed full release
/// and proving-key qualification.
///
/// The canonical Norito value contains an explicit schema and fixed V1 layout.
/// It is not self-authenticating: production loading accepts it only from a
/// root-owned, single-link, non-writable descriptor-relative path (also
/// extended-ACL-free on macOS) and compares every sealed source and executable
/// identity before trusting its qualified Eq/Ep facts.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaCatalogQualificationSealV1 {
    schema: String,
    version: u16,
    canonical_policy_path: String,
    canonical_artifact_dir: String,
    canonical_executable_path: String,
    build_fingerprint_sha256: [u8; 32],
    executable_sha256: [u8; 32],
    configured_policy_sha256: [u8; 32],
    paths: Vec<KagemushaCatalogSealedPathV1>,
    releases: Vec<KagemushaCatalogSealedReleaseQualificationV1>,
}

impl KagemushaCatalogQualificationSealV1 {
    /// Encode the fixed V1 seal layout canonically.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito cannot encode the seal or the encoded
    /// representation exceeds the bounded qualification-seal corridor.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_layout()?;
        let bytes = norito::encode_canonical(self).map_err(|error| {
            format!("failed to encode Kagemusha catalog qualification seal: {error}")
        })?;
        if bytes.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1 {
            return Err(format!(
                "Kagemusha catalog qualification seal exceeds the {}-byte limit",
                KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1
            ));
        }
        Ok(bytes)
    }
}

impl KagemushaCatalogSealedParityQualificationV1 {
    fn from_qualified(
        qualified: &KagemushaQualifiedParityMetadataV4,
        compiled_protocol_structure_sha256: [u8; 32],
    ) -> Result<Self, String> {
        if compiled_protocol_structure_sha256 == [0; 32]
            || compiled_protocol_structure_sha256 == qualified.compiled_protocol_identity_sha256()
        {
            return Err(
                "Kagemusha V4 compiled-protocol structure and qualified identity must be non-zero and distinct"
                    .to_owned(),
            );
        }
        Ok(Self {
            parity: qualified.parity(),
            circuit_params: qualified.circuit_params().clone(),
            compiled_protocol_structure_sha256,
            compiled_protocol_identity_sha256: qualified.compiled_protocol_identity_sha256(),
            processed_verifying_key_len: qualified.processed_verifying_key_len(),
            processed_verifying_key_sha256: qualified.processed_verifying_key_sha256(),
            verifying_key_commitment: qualified.verifying_key_commitment(),
            proving_key_embedded_verifying_key_sha256: qualified
                .proving_key_embedded_verifying_key_sha256(),
            proving_key_fixed_columns: u64::try_from(qualified.proving_key_fixed_columns())
                .map_err(|_| {
                    "Kagemusha V4 proving-key fixed-column count does not fit u64".to_owned()
                })?,
            proving_key_permutation_columns: u64::try_from(
                qualified.proving_key_permutation_columns(),
            )
            .map_err(|_| {
                "Kagemusha V4 proving-key permutation-column count does not fit u64".to_owned()
            })?,
        })
    }

    fn to_qualified(&self) -> Result<KagemushaQualifiedParityMetadataV4, String> {
        KagemushaQualifiedParityMetadataV4::new(
            self.parity,
            self.circuit_params.clone(),
            self.compiled_protocol_identity_sha256,
            self.processed_verifying_key_len,
            self.processed_verifying_key_sha256,
            self.verifying_key_commitment,
            self.proving_key_embedded_verifying_key_sha256,
            usize::try_from(self.proving_key_fixed_columns).map_err(|_| {
                "sealed Kagemusha V4 proving-key fixed-column count does not fit usize".to_owned()
            })?,
            usize::try_from(self.proving_key_permutation_columns).map_err(|_| {
                "sealed Kagemusha V4 proving-key permutation-column count does not fit usize"
                    .to_owned()
            })?,
        )
    }
}

/// One startup-authenticated ABI-21 release retained for consensus execution.
pub(crate) struct KagemushaCachedReleaseV4 {
    release_record: iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    resolved: ResolvedKagemushaTerminalVerifierV4,
}

/// Immutable startup catalog keyed by canonical V4 manifest digest.
///
/// The catalog owns qualified pinned read-only artifact handles and one
/// source-backed verifier facade per release. Consensus execution performs map
/// lookups and reads only those already-opened inodes; it never reopens an
/// artifact by path or caches two-parity Halo2 material.
#[derive(Default)]
pub struct KagemushaReleaseCatalogV4 {
    configured_policy_sha256: Option<[u8; 32]>,
    releases: BTreeMap<[u8; 32], Arc<KagemushaCachedReleaseV4>>,
}

impl KagemushaReleaseCatalogV4 {
    /// Return an unconfigured, always-unready catalog.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether a canonical policy and artifact directory were configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        self.configured_policy_sha256.is_some()
    }

    /// Digest of the configured canonical Norito policy, when configured.
    #[must_use]
    pub const fn configured_policy_sha256(&self) -> Option<[u8; 32]> {
        self.configured_policy_sha256
    }

    /// Load an optional immutable verifier cache.
    ///
    /// An omitted policy/artifact pair produces the explicit empty catalog. The
    /// cache is not an offline-capability switch and is not an asset catalog;
    /// every deployment and asset retains the protocol primitives when it is
    /// empty. A partially configured pair or authentication failure is rejected
    /// only when an operator explicitly configures this cache.
    ///
    /// # Errors
    ///
    /// Returns an error when only one catalog path is configured or when the configured catalog
    /// or qualification seal cannot be authenticated.
    pub fn from_offline_config(
        config: &iroha_config::parameters::actual::Offline,
    ) -> Result<Self, String> {
        match (
            config.kagemusha_release_policy_path.as_deref(),
            config.kagemusha_artifact_dir.as_deref(),
        ) {
            (None, None) => Ok(Self::empty()),
            (Some(policy_path), Some(artifact_dir)) => {
                if let Some(seal_path) = config.kagemusha_catalog_qualification_seal_path.as_deref()
                {
                    Self::load_with_decoded_budget_and_qualification_seal(
                        policy_path,
                        artifact_dir,
                        config.kagemusha_max_decoded_bytes,
                        seal_path,
                    )
                } else {
                    Self::load_with_decoded_budget(
                        policy_path,
                        artifact_dir,
                        config.kagemusha_max_decoded_bytes,
                    )
                }
            }
            _ => Err(
                "Kagemusha V4 release policy and artifact directory must be configured together"
                    .to_owned(),
            ),
        }
    }

    pub(crate) fn get(&self, manifest_sha256: &[u8; 32]) -> Option<&Arc<KagemushaCachedReleaseV4>> {
        self.releases.get(manifest_sha256)
    }

    /// Number of authenticated releases retained by this process.
    #[must_use]
    pub fn len(&self) -> usize {
        self.releases.len()
    }

    /// Whether this catalog contains no authenticated releases.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.releases.is_empty()
    }

    /// Deterministically authenticate every manifest-digest subdirectory.
    ///
    /// Both configured paths must be canonical absolute paths. Every directory
    /// component is opened relative to its already pinned parent and symlinks
    /// are rejected at every level.
    ///
    /// All filesystem access, hashing, framing checks, Halo2 verifier parsing,
    /// and allocation-free proving-key structural validation complete before
    /// the returned immutable catalog is published. Full proving-key parsing is
    /// deferred until an actual prover operation needs that parity.
    pub fn load(policy_path: &Path, artifact_dir: &Path) -> Result<Self, String> {
        Self::load_with_decoded_budget(
            policy_path,
            artifact_dir,
            DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
        )
    }

    /// Authenticate a catalog under an explicit decoded-resident memory ceiling.
    pub fn load_with_decoded_budget(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<Self, String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_descriptor_relative(policy_path, artifact_dir, max_decoded_bytes)
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes);
            Err(
                "Kagemusha V4 descriptor-relative catalog loading is unsupported on this platform"
                    .to_owned(),
            )
        }
    }

    /// Fully authenticate a catalog and produce its root-trusted restart seal.
    ///
    /// This constructor always executes complete artifact hashing and Eq/Ep
    /// proving-key structural qualification before it emits a seal. It also
    /// requires the configured inputs and current executable to be rooted in
    /// root-owned, non-writable, symlink-free path chains.
    pub fn load_and_build_qualification_seal(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<(Self, KagemushaCatalogQualificationSealV1), String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_and_build_qualification_seal_for_trusted_uid(
                policy_path,
                artifact_dir,
                max_decoded_bytes,
                0,
            )
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes);
            Err("Kagemusha V4 qualification seals are unsupported on this platform".to_owned())
        }
    }

    /// Load a fully qualified catalog through a persistent root-trusted seal.
    ///
    /// Seal absence or any path, stat, build, digest, inventory, or qualified
    /// metadata mismatch fails closed. The fast path never refreshes the seal
    /// and never streams a proving-key payload.
    pub fn load_with_decoded_budget_and_qualification_seal(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        seal_path: &Path,
    ) -> Result<Self, String> {
        validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes)?;
        #[cfg(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        ))]
        {
            Self::load_with_qualification_seal_for_trusted_uid(
                policy_path,
                artifact_dir,
                max_decoded_bytes,
                seal_path,
                0,
            )
        }
        #[cfg(not(all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )))]
        {
            let _ = (policy_path, artifact_dir, max_decoded_bytes, seal_path);
            Err("Kagemusha V4 qualification seals are unsupported on this platform".to_owned())
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_descriptor_relative(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
    ) -> Result<Self, String> {
        Self::load_descriptor_relative_with_qualification(
            policy_path,
            artifact_dir,
            max_decoded_bytes,
            None,
        )
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_descriptor_relative_with_qualification(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        qualification_seal: Option<&KagemushaCatalogQualificationSealV1>,
    ) -> Result<Self, String> {
        let (policy_parent_path, policy_file_name) =
            absolute_file_parent_and_name(policy_path, "release policy")?;
        let policy_parent =
            CatalogDirectory::open_path(policy_parent_path, "release policy parent")?;
        let mut policy_file = policy_parent.open_file(policy_file_name, "release policy")?;
        let policy_bytes =
            read_bounded_opened_file(&mut policy_file, MAX_POLICY_BYTES, "release policy")?;
        let policy = decode_trusted_policy(&policy_bytes)?;
        let policy_sha256: [u8; 32] = Sha256::digest(&policy_bytes).into();
        if qualification_seal.is_some_and(|seal| seal.configured_policy_sha256 != policy_sha256) {
            return Err(
                "Kagemusha V4 qualification seal configured-policy digest mismatch".to_owned(),
            );
        }

        let artifact_root = CatalogDirectory::open_path(artifact_dir, "artifact root")?;
        let directory_names = artifact_root.entry_names("artifact root")?;
        ensure_catalog_release_count(directory_names.len())?;
        if let Some(seal) = qualification_seal {
            let sealed_names = seal
                .releases
                .iter()
                .map(|release| hex::encode(release.manifest_sha256))
                .collect::<Vec<_>>();
            if sealed_names != directory_names {
                return Err("Kagemusha V4 qualification seal release inventory mismatch".to_owned());
            }
        }

        let mut releases = BTreeMap::new();
        let mut aggregate_catalog_bytes = 0_u64;
        let mut aggregate_decoded_bytes = 0_u64;
        for directory_name in &directory_names {
            let file_name = directory_name.as_str();
            let manifest_sha256 = parse_manifest_directory_name(file_name)?;
            let directory = artifact_root
                .open_directory(directory_name, &format!("release directory `{file_name}`"))?;
            let remaining_catalog_bytes = MAX_CATALOG_AGGREGATE_BYTES_V4
                .checked_sub(aggregate_catalog_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 catalog aggregate byte accounting overflowed".to_owned()
                })?;
            let remaining_decoded_bytes = max_decoded_bytes
                .checked_sub(aggregate_decoded_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 decoded catalog memory accounting overflowed".to_owned()
                })?;
            let sealed_release = qualification_seal.and_then(|seal| {
                seal.releases
                    .iter()
                    .find(|release| release.manifest_sha256 == manifest_sha256)
            });
            if qualification_seal.is_some() && sealed_release.is_none() {
                return Err("Kagemusha V4 qualification seal omits a catalog release".to_owned());
            }
            let (release, release_bytes, release_decoded_bytes) = load_release_directory(
                &directory,
                manifest_sha256,
                &policy,
                policy_sha256,
                remaining_catalog_bytes,
                remaining_decoded_bytes,
                sealed_release,
            )?;
            aggregate_catalog_bytes =
                add_catalog_release_bytes(aggregate_catalog_bytes, release_bytes)?;
            aggregate_decoded_bytes = aggregate_decoded_bytes
                .checked_add(release_decoded_bytes)
                .ok_or_else(|| {
                    "Kagemusha V4 decoded catalog memory accounting overflowed".to_owned()
                })?;
            artifact_root.verify_directory_entry(directory_name, &directory)?;
            if releases
                .insert(manifest_sha256, Arc::new(release))
                .is_some()
            {
                return Err("Kagemusha V4 artifact catalog repeats a manifest digest".to_owned());
            }
        }
        if artifact_root.entry_names("artifact root")? != directory_names {
            return Err("Kagemusha V4 artifact inventory changed while it was loaded".to_owned());
        }
        artifact_root.verify_path_identity()?;
        policy_file.verify_unchanged()?;
        policy_parent.verify_path_identity()?;
        if releases.is_empty() {
            return Err(
                "configured Kagemusha V4 artifact directory contains no releases".to_owned(),
            );
        }
        Ok(Self {
            configured_policy_sha256: Some(policy_sha256),
            releases,
        })
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_and_build_qualification_seal_for_trusted_uid(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        trusted_uid: u32,
    ) -> Result<(Self, KagemushaCatalogQualificationSealV1), String> {
        let effective_uid = rustix::process::geteuid().as_raw();
        if effective_uid != trusted_uid {
            return Err(format!(
                "Kagemusha V4 qualification seal creation requires effective uid {trusted_uid}, found {effective_uid}"
            ));
        }
        let catalog = Self::load_descriptor_relative(policy_path, artifact_dir, max_decoded_bytes)?;
        let seal = build_kagemusha_catalog_qualification_seal_v1(
            policy_path,
            artifact_dir,
            &catalog,
            trusted_uid,
        )?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        Ok((catalog, seal))
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn load_with_qualification_seal_for_trusted_uid(
        policy_path: &Path,
        artifact_dir: &Path,
        max_decoded_bytes: u64,
        seal_path: &Path,
        trusted_uid: u32,
    ) -> Result<Self, String> {
        let seal =
            read_root_trusted_kagemusha_catalog_qualification_seal_v1(seal_path, trusted_uid)?;
        seal.validate_for_configured_runtime(policy_path, artifact_dir)?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        let catalog = Self::load_descriptor_relative_with_qualification(
            policy_path,
            artifact_dir,
            max_decoded_bytes,
            Some(&seal),
        )?;
        verify_kagemusha_catalog_sealed_paths_v1(&seal.paths, trusted_uid)?;
        Ok(catalog)
    }

    /// Build the exact governed activation payload for one authenticated release.
    ///
    /// This is the only production constructor for the consensus payload. It
    /// projects both inline verifier records from the immutable, qualified
    /// pinned startup source, so an operator cannot substitute release fields,
    /// key bytes, commitments, schemas, activation heights, or policy identity.
    /// Consensus still enforces that `verifier_version` is the next atomic
    /// Eq/Ep version when the resulting instruction is executed.
    pub fn build_activation(
        &self,
        manifest_sha256: [u8; 32],
        verifier_version: u32,
    ) -> Result<KagemushaRecursiveSpendReleaseActivationV4, String> {
        if verifier_version == 0 {
            return Err("Kagemusha V4 verifier version must be nonzero".to_owned());
        }
        let configured_policy_sha256 = self.configured_policy_sha256.ok_or_else(|| {
            "Kagemusha V4 activation requires a configured release policy".to_owned()
        })?;
        let cached = self.get(&manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 activation release is absent from the authenticated catalog".to_owned()
        })?;
        let release = cached.resolved.release();
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: release.manifest().generation.clone(),
            manifest_sha256,
        };
        let step_eq_verifier_record = cached.activation_record(
            &binding,
            KagemushaPastaCycleParityV1::StepEq,
            verifier_version,
        )?;
        let step_ep_verifier_record = cached.activation_record(
            &binding,
            KagemushaPastaCycleParityV1::StepEp,
            verifier_version,
        )?;
        let activation = KagemushaRecursiveSpendReleaseActivationV4 {
            release_record: cached.release_record.clone(),
            configured_policy_sha256,
            step_eq_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    KagemushaPastaCycleParityV1::StepEq,
                    manifest_sha256,
                ),
            step_eq_verifier_record,
            step_ep_verifier_key_id:
                iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                    KagemushaPastaCycleParityV1::StepEp,
                    manifest_sha256,
                ),
            step_ep_verifier_record,
        };
        activation
            .validate_structure()
            .map_err(|error| format!("constructed Kagemusha V4 activation is invalid: {error}"))?;
        Ok(activation)
    }

    pub(crate) fn resolve_binding(
        &self,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
    ) -> Result<&Arc<KagemushaCachedReleaseV4>, String> {
        binding
            .validate()
            .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
        let cached = self.get(&binding.manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 release is not present in the immutable startup catalog".to_owned()
        })?;
        if cached.resolved.release().manifest().generation != binding.generation {
            return Err("Kagemusha V4 release generation and digest disagree".to_owned());
        }
        Ok(cached)
    }

    pub(crate) fn resolve_activation_records(
        &self,
        step_eq_record: &VerifyingKeyRecord,
        step_ep_record: &VerifyingKeyRecord,
    ) -> Result<&Arc<KagemushaCachedReleaseV4>, String> {
        let manifest_sha256 = activation_manifest_sha256(step_eq_record, step_ep_record)?;
        let cached = self.get(&manifest_sha256).ok_or_else(|| {
            "active Kagemusha V4 release is absent from the immutable startup catalog".to_owned()
        })?;
        cached.validate_verifier_records(step_eq_record, step_ep_record)?;
        Ok(cached)
    }
}

impl KagemushaCachedReleaseV4 {
    pub(crate) fn release_record(
        &self,
    ) -> &iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4 {
        &self.release_record
    }

    pub(crate) fn resolved(&self) -> &ResolvedKagemushaTerminalVerifierV4 {
        &self.resolved
    }

    pub(crate) fn verifier(&self) -> &KagemushaPastaCycleOpaqueVerifierV4 {
        self.resolved.verifier.as_ref()
    }

    pub(crate) fn issuance_active_at(&self, block_height: u64) -> bool {
        let manifest = self.resolved.release().manifest();
        block_height >= manifest.activation_height && block_height < manifest.withdrawal_height
    }

    fn activation_record(
        &self,
        binding: &KagemushaRecursiveSpendArtifactBindingV4,
        parity: KagemushaPastaCycleParityV1,
        verifier_version: u32,
    ) -> Result<VerifyingKeyRecord, String> {
        let release = self.resolved.release();
        let manifest = release.manifest();
        let proof_profile = profile(manifest, parity)?;
        let curve = match parity {
            KagemushaPastaCycleParityV1::StepEq => STEP_EQ_VERIFIER_CURVE_V4,
            KagemushaPastaCycleParityV1::StepEp => STEP_EP_VERIFIER_CURVE_V4,
        };
        let authenticated_vk = self.resolved.authenticated_verifying_key(parity)?;
        let vk_len = u32::try_from(authenticated_vk.len())
            .map_err(|_| "Kagemusha V4 verifier key length exceeds u32".to_owned())?;
        let key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            authenticated_vk,
        );
        let commitment = crate::zk::hash_vk(&key);
        let mut record = VerifyingKeyRecord::new_with_owner(
            verifier_version,
            proof_profile.circuit_id.clone(),
            Some(verifier_owner_manifest_id(binding)?),
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            curve,
            verifier_public_inputs_schema_hash(manifest, parity)?,
            commitment,
        );
        record.vk_len = vk_len;
        record.max_proof_bytes = manifest.max_proof_bytes;
        record.activation_height = Some(manifest.activation_height);
        // Withdrawal closes issuance only. Historic notes must retain a live
        // terminal verifier after the release stops creating new notes.
        record.withdraw_height = None;
        record.key = Some(key);
        record.status = ConfidentialStatus::Active;
        ensure_activation_record(
            &record,
            binding,
            release,
            parity,
            self.resolved.parity_metadata(parity),
            manifest.activation_height,
        )?;
        Ok(record)
    }

    pub(crate) fn validate_verifier_records(
        &self,
        step_eq_record: &VerifyingKeyRecord,
        step_ep_record: &VerifyingKeyRecord,
    ) -> Result<(), String> {
        if step_eq_record.version == 0 || step_eq_record.version != step_ep_record.version {
            return Err("Kagemusha V4 Eq/Ep activation versions are not atomic".to_owned());
        }
        let release = self.resolved.release();
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: release.manifest().generation.clone(),
            manifest_sha256: release.manifest_sha256(),
        };
        ensure_activation_record(
            step_eq_record,
            &binding,
            release,
            KagemushaPastaCycleParityV1::StepEq,
            self.resolved
                .parity_metadata(KagemushaPastaCycleParityV1::StepEq),
            release.manifest().activation_height,
        )?;
        ensure_activation_record(
            step_ep_record,
            &binding,
            release,
            KagemushaPastaCycleParityV1::StepEp,
            self.resolved
                .parity_metadata(KagemushaPastaCycleParityV1::StepEp),
            release.manifest().activation_height,
        )?;
        if step_eq_record.commitment == step_ep_record.commitment {
            return Err("Kagemusha V4 Eq/Ep verifier record identities collide".to_owned());
        }
        Ok(())
    }
}

fn activation_manifest_sha256(
    step_eq_record: &VerifyingKeyRecord,
    step_ep_record: &VerifyingKeyRecord,
) -> Result<[u8; 32], String> {
    let step_eq_manifest_sha256 = verifier_owner_manifest_sha256(step_eq_record, "Eq")?;
    let step_ep_manifest_sha256 = verifier_owner_manifest_sha256(step_ep_record, "Ep")?;
    if step_eq_manifest_sha256 != step_ep_manifest_sha256 {
        return Err("Kagemusha V4 Eq/Ep activation records select different releases".to_owned());
    }
    Ok(step_eq_manifest_sha256)
}

/// Parse the canonical owner-manifest digest committed by one V4 verifier record.
pub(crate) fn verifier_owner_manifest_sha256(
    record: &VerifyingKeyRecord,
    role: &str,
) -> Result<[u8; 32], String> {
    let owner = record.owner_manifest_id.as_deref().ok_or_else(|| {
        format!("Kagemusha V4 {role} verifier has no authenticated release owner")
    })?;
    let manifest_hex = owner
        .strip_prefix(VERIFIER_OWNER_MANIFEST_PREFIX_V4)
        .ok_or_else(|| "Kagemusha V4 verifier owner namespace is invalid".to_owned())?;
    parse_manifest_directory_name(manifest_hex)
}

fn parse_manifest_directory_name(name: &str) -> Result<[u8; 32], String> {
    if name.len() != 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!(
            "Kagemusha V4 release directory `{name}` is not a lowercase manifest digest"
        ));
    }
    hex::decode(name)
        .map_err(|_| "Kagemusha V4 manifest directory digest is malformed".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha V4 manifest directory digest has the wrong length".to_owned())
}

fn validate_kagemusha_catalog_decoded_budget_v4(max_decoded_bytes: u64) -> Result<(), String> {
    if max_decoded_bytes == 0 {
        return Err(
            "Kagemusha V4 decoded catalog memory budget must be greater than zero".to_owned(),
        );
    }
    if max_decoded_bytes > DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 decoded catalog memory budget cannot exceed the non-raiseable {DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4}-byte safety ceiling"
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn ensure_catalog_release_count(count: usize) -> Result<(), String> {
    if count > MAX_CATALOG_RELEASES_V4 {
        return Err(format!(
            "Kagemusha V4 artifact catalog contains {count} releases; at most {MAX_CATALOG_RELEASES_V4} are retained"
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn add_catalog_release_bytes(current: u64, release_bytes: u64) -> Result<u64, String> {
    let total = current
        .checked_add(release_bytes)
        .ok_or_else(|| "Kagemusha V4 catalog aggregate byte accounting overflowed".to_owned())?;
    if total > MAX_CATALOG_AGGREGATE_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 artifact catalog exceeds the aggregate byte limit of {MAX_CATALOG_AGGREGATE_BYTES_V4}"
        ));
    }
    Ok(total)
}

fn validate_absolute_catalog_path(path: &Path, label: &str) -> Result<(), String> {
    let normalized = path.components().collect::<PathBuf>();
    let mut components = path.components();
    let has_root = matches!(components.next(), Some(Component::RootDir));
    let has_forbidden_component =
        components.any(|component| !matches!(component, Component::Normal(_)));
    if !has_root
        || path.as_os_str().is_empty()
        || normalized.as_os_str() != path.as_os_str()
        || has_forbidden_component
    {
        return Err(format!(
            "Kagemusha V4 {label} `{}` must be an absolute path with one canonical spelling",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn absolute_file_parent_and_name<'path>(
    path: &'path Path,
    label: &str,
) -> Result<(&'path Path, &'path str), String> {
    validate_absolute_catalog_path(path, label)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Kagemusha V4 {label} path has no UTF-8 file name"))?;
    let parent = path.parent().ok_or_else(|| {
        format!(
            "Kagemusha V4 {label} `{}` has no absolute parent directory",
            path.display()
        )
    })?;
    Ok((parent, file_name))
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn canonical_catalog_path_string_v1(path: &Path, label: &str) -> Result<String, String> {
    validate_absolute_catalog_path(path, label)?;
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("Kagemusha V4 {label} path is not canonical UTF-8"))
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn insert_sealed_catalog_path_v1(
    paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
    entry: KagemushaCatalogSealedPathV1,
) -> Result<(), String> {
    if let Some(previous) = paths.insert(entry.canonical_path.clone(), entry.clone())
        && previous != entry
    {
        return Err(format!(
            "Kagemusha V4 sealed path `{}` changed identity while the seal was built",
            entry.canonical_path
        ));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl CatalogFileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogFileSnapshot {
    identity: CatalogFileIdentity,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl CatalogFileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        (metadata.is_file() && metadata.nlink() == 1).then(|| Self {
            identity: CatalogFileIdentity::from_metadata(metadata),
            mode: metadata.mode(),
            owner_uid: metadata.uid(),
            owner_gid: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Option<Self> {
        if RustixFileType::from_raw_mode(stat.st_mode) != RustixFileType::RegularFile {
            return None;
        }
        let links = u64::try_from(stat.st_nlink).ok()?;
        if links != 1 {
            return None;
        }
        Some(Self {
            identity: CatalogFileIdentity::from_stat(stat),
            mode: u32::try_from(stat.st_mode).ok()?,
            owner_uid: u32::try_from(stat.st_uid).ok()?,
            owner_gid: u32::try_from(stat.st_gid).ok()?,
            links,
            length: u64::try_from(stat.st_size).ok()?,
            modified_seconds: i64::try_from(stat.st_mtime).ok()?,
            modified_nanoseconds: i64::try_from(stat.st_mtime_nsec).ok()?,
            changed_seconds: i64::try_from(stat.st_ctime).ok()?,
            changed_nanoseconds: i64::try_from(stat.st_ctime_nsec).ok()?,
        })
    }

    fn matches_stat(self, stat: &rustix::fs::Stat) -> bool {
        Self::from_stat(stat) == Some(self)
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogDirectorySnapshot {
    identity: CatalogFileIdentity,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl CatalogDirectorySnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        metadata.is_dir().then(|| Self {
            identity: CatalogFileIdentity::from_metadata(metadata),
            mode: metadata.mode(),
            owner_uid: metadata.uid(),
            owner_gid: metadata.gid(),
            links: metadata.nlink(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Option<Self> {
        if RustixFileType::from_raw_mode(stat.st_mode) != RustixFileType::Directory {
            return None;
        }
        Some(Self {
            identity: CatalogFileIdentity::from_stat(stat),
            mode: u32::try_from(stat.st_mode).ok()?,
            owner_uid: u32::try_from(stat.st_uid).ok()?,
            owner_gid: u32::try_from(stat.st_gid).ok()?,
            links: u64::try_from(stat.st_nlink).ok()?,
            length: u64::try_from(stat.st_size).ok()?,
            modified_seconds: i64::try_from(stat.st_mtime).ok()?,
            modified_nanoseconds: i64::try_from(stat.st_mtime_nsec).ok()?,
            changed_seconds: i64::try_from(stat.st_ctime).ok()?,
            changed_nanoseconds: i64::try_from(stat.st_ctime_nsec).ok()?,
        })
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl From<CatalogFileSnapshot> for KagemushaCatalogSealedStatV1 {
    fn from(snapshot: CatalogFileSnapshot) -> Self {
        Self {
            device: snapshot.identity.device,
            inode: snapshot.identity.inode,
            mode: snapshot.mode,
            owner_uid: snapshot.owner_uid,
            owner_gid: snapshot.owner_gid,
            links: snapshot.links,
            length: snapshot.length,
            modified_seconds: snapshot.modified_seconds,
            modified_nanoseconds: snapshot.modified_nanoseconds,
            changed_seconds: snapshot.changed_seconds,
            changed_nanoseconds: snapshot.changed_nanoseconds,
        }
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl From<CatalogDirectorySnapshot> for KagemushaCatalogSealedStatV1 {
    fn from(snapshot: CatalogDirectorySnapshot) -> Self {
        Self {
            device: snapshot.identity.device,
            inode: snapshot.identity.inode,
            mode: snapshot.mode,
            owner_uid: snapshot.owner_uid,
            owner_gid: snapshot.owner_gid,
            links: snapshot.links,
            length: snapshot.length,
            modified_seconds: snapshot.modified_seconds,
            modified_nanoseconds: snapshot.modified_nanoseconds,
            changed_seconds: snapshot.changed_seconds,
            changed_nanoseconds: snapshot.changed_nanoseconds,
        }
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn ensure_root_trusted_stat_v1(
    stat: KagemushaCatalogSealedStatV1,
    trusted_uid: u32,
    label: &str,
) -> Result<(), String> {
    let owner_is_trusted =
        stat.owner_uid == trusted_uid || (cfg!(test) && trusted_uid != 0 && stat.owner_uid == 0);
    if !owner_is_trusted {
        return Err(format!(
            "Kagemusha V4 {label} owner uid {} is not the trusted uid {trusted_uid}",
            stat.owner_uid
        ));
    }
    if stat.mode & 0o022 != 0 {
        return Err(format!(
            "Kagemusha V4 {label} must not be group- or world-writable"
        ));
    }
    if stat.links == 0 {
        return Err(format!("Kagemusha V4 {label} has no filesystem link"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
const KAGEMUSHA_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES_V1: usize = 64 * 1024;
#[cfg(target_os = "macos")]
const KAGEMUSHA_MACOS_ACL_CACHE_MAX_ENTRIES_V1: usize = 4 * 1024;

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct KagemushaMacosAclSnapshotKeyV1 {
    path: PathBuf,
    device: u64,
    inode: u64,
    mode: u32,
    owner_uid: u32,
    owner_gid: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(target_os = "macos")]
impl KagemushaMacosAclSnapshotKeyV1 {
    fn new(path: &Path, stat: KagemushaCatalogSealedStatV1) -> Self {
        Self {
            path: path.to_path_buf(),
            device: stat.device,
            inode: stat.inode,
            mode: stat.mode,
            owner_uid: stat.owner_uid,
            owner_gid: stat.owner_gid,
            links: stat.links,
            length: stat.length,
            modified_seconds: stat.modified_seconds,
            modified_nanoseconds: stat.modified_nanoseconds,
            changed_seconds: stat.changed_seconds,
            changed_nanoseconds: stat.changed_nanoseconds,
        }
    }
}

#[cfg(target_os = "macos")]
fn ensure_no_macos_extended_acl_v1(
    path: &Path,
    stat: KagemushaCatalogSealedStatV1,
    label: &str,
    revalidate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    static ACL_FREE_SNAPSHOTS: std::sync::OnceLock<
        Mutex<BTreeSet<KagemushaMacosAclSnapshotKeyV1>>,
    > = std::sync::OnceLock::new();

    let key = KagemushaMacosAclSnapshotKeyV1::new(path, stat);
    let cache = ACL_FREE_SNAPSHOTS.get_or_init(|| Mutex::new(BTreeSet::new()));
    let cached = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&key);
    if !cached {
        let output = std::process::Command::new("/bin/ls")
            .arg("-ldeq")
            .arg(path)
            .env_clear()
            .env("LC_ALL", "C")
            .output()
            .map_err(|error| {
                format!(
                    "failed to inspect macOS extended ACL for Kagemusha V4 {label} `{}`: {error}",
                    path.display()
                )
            })?;
        if output.stdout.len() > KAGEMUSHA_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES_V1
            || output.stderr.len() > KAGEMUSHA_MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES_V1
        {
            return Err(format!(
                "macOS extended ACL inspection output exceeded its bound for Kagemusha V4 {label} `{}`",
                path.display()
            ));
        }
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "macOS extended ACL inspection failed for Kagemusha V4 {label} `{}`: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let newline_count = output.stdout.iter().filter(|byte| **byte == b'\n').count();
        if !output.stdout.ends_with(b"\n") || newline_count != 1 {
            return Err(format!(
                "Kagemusha V4 {label} `{}` must not have an extended ACL",
                path.display()
            ));
        }
    }

    // ACL edits update ctime. Revalidate the already-pinned stat immediately
    // after the path-based query so removal/restoration races fail closed.
    revalidate()?;
    if !cached {
        let mut cache = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache.len() >= KAGEMUSHA_MACOS_ACL_CACHE_MAX_ENTRIES_V1 {
            cache.clear();
        }
        cache.insert(key);
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn ensure_no_macos_extended_acl_v1(
    _path: &Path,
    _stat: KagemushaCatalogSealedStatV1,
    _label: &str,
    revalidate: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    // POSIX mode/owner validation remains unchanged on non-macOS Unix.
    revalidate()
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
struct CatalogDirectory {
    display_path: PathBuf,
    file: File,
    snapshot: CatalogDirectorySnapshot,
    path_chain: Vec<(PathBuf, CatalogDirectorySnapshot)>,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl CatalogDirectory {
    fn open_path(path: &Path, label: &str) -> Result<Self, String> {
        validate_absolute_catalog_path(path, label)?;
        let root_path = Path::new("/");
        let before = fs::symlink_metadata(root_path).map_err(|error| {
            format!("failed to inspect Kagemusha V4 filesystem root for {label}: {error}")
        })?;
        let before = CatalogDirectorySnapshot::from_metadata(&before).ok_or_else(|| {
            format!("Kagemusha V4 filesystem root for {label} must be a real directory")
        })?;
        let file = File::from(
            open(
                root_path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| {
                format!("failed to open Kagemusha V4 filesystem root for {label}: {error}")
            })?,
        );
        let opened = file.metadata().map_err(|error| {
            format!("failed to inspect opened Kagemusha V4 filesystem root: {error}")
        })?;
        let after = fs::symlink_metadata(root_path).map_err(|error| {
            format!("failed to re-inspect Kagemusha V4 filesystem root: {error}")
        })?;
        if CatalogDirectorySnapshot::from_metadata(&opened) != Some(before)
            || CatalogDirectorySnapshot::from_metadata(&after) != Some(before)
        {
            return Err("Kagemusha V4 filesystem root changed while it was opened".to_owned());
        }
        let mut directory = Self {
            display_path: root_path.to_path_buf(),
            file,
            snapshot: before,
            path_chain: vec![(root_path.to_path_buf(), before)],
        };
        for component in path.components().skip(1) {
            let Component::Normal(name) = component else {
                return Err(format!(
                    "Kagemusha V4 {label} `{}` has a non-canonical path component",
                    path.display()
                ));
            };
            let component_path = directory.display_path.join(name);
            directory = directory.open_directory_os(
                name,
                &format!("{label} component `{}`", component_path.display()),
            )?;
        }
        Ok(directory)
    }

    fn verify_trusted_path_chain(&self, trusted_uid: u32, label: &str) -> Result<(), String> {
        self.verify_path_identity()?;
        for (path, snapshot) in &self.path_chain {
            let path_label = format!("{label} directory `{}`", path.display());
            ensure_root_trusted_stat_v1((*snapshot).into(), trusted_uid, &path_label)?;
            ensure_no_macos_extended_acl_v1(path, (*snapshot).into(), &path_label, || {
                let current = fs::symlink_metadata(path).map_err(|error| {
                    format!(
                        "failed to re-inspect Kagemusha V4 {path_label} after ACL validation: {error}"
                    )
                })?;
                if CatalogDirectorySnapshot::from_metadata(&current) != Some(*snapshot) {
                    return Err(format!(
                        "Kagemusha V4 {path_label} changed during ACL validation"
                    ));
                }
                Ok(())
            })?;
        }
        self.verify_path_identity()
    }

    fn append_sealed_path_chain(
        &self,
        paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
    ) -> Result<(), String> {
        for (path, snapshot) in &self.path_chain {
            insert_sealed_catalog_path_v1(
                paths,
                KagemushaCatalogSealedPathV1 {
                    canonical_path: canonical_catalog_path_string_v1(path, "directory")?,
                    kind: KagemushaCatalogSealedPathKindV1::Directory,
                    stat: (*snapshot).into(),
                },
            )?;
        }
        Ok(())
    }

    fn validate_entry_os_name(name: &OsStr, label: &str) -> Result<(), String> {
        let mut components = Path::new(name).components();
        if !matches!(components.next(), Some(Component::Normal(component)) if component == name)
            || components.next().is_some()
        {
            return Err(format!(
                "Kagemusha V4 {label} name must be one normal path component"
            ));
        }
        Ok(())
    }

    fn validate_entry_name(name: &str, label: &str) -> Result<(), String> {
        Self::validate_entry_os_name(OsStr::new(name), label)
    }

    fn stat_entry_os(&self, name: &OsStr, label: &str) -> Result<rustix::fs::Stat, String> {
        Self::validate_entry_os_name(name, label)?;
        statat(&self.file, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            format!(
                "failed to inspect Kagemusha V4 {label} `{}`: {error}",
                self.display_path.join(name).display()
            )
        })
    }

    fn stat_entry(&self, name: &str, label: &str) -> Result<rustix::fs::Stat, String> {
        self.stat_entry_os(OsStr::new(name), label)
    }

    fn entry_names(&self, label: &str) -> Result<Vec<String>, String> {
        self.verify_opened_snapshot()?;
        let mut stream = Dir::read_from(&self.file).map_err(|error| {
            format!(
                "failed to enumerate Kagemusha V4 {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        let mut names = Vec::new();
        for entry in &mut stream {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read Kagemusha V4 {label} `{}`: {error}",
                    self.display_path.display()
                )
            })?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            let name = std::str::from_utf8(bytes)
                .map_err(|_| format!("Kagemusha V4 {label} entry name is not UTF-8"))?
                .to_owned();
            Self::validate_entry_name(&name, label)?;
            names.push(name);
            if names.len() > MAX_CATALOG_DIRECTORY_ENTRIES {
                return Err(format!(
                    "Kagemusha V4 {label} contains too many directory entries"
                ));
            }
        }
        names.sort();
        self.verify_opened_snapshot()?;
        Ok(names)
    }

    fn open_directory(&self, name: &str, label: &str) -> Result<Self, String> {
        self.open_directory_os(OsStr::new(name), label)
    }

    fn open_directory_os(&self, name: &OsStr, label: &str) -> Result<Self, String> {
        let before = self.stat_entry_os(name, label)?;
        let before = CatalogDirectorySnapshot::from_stat(&before)
            .ok_or_else(|| format!("Kagemusha V4 {label} is not a real directory"))?;
        let file = File::from(
            openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| format!("failed to open Kagemusha V4 {label}: {error}"))?,
        );
        let opened = file
            .metadata()
            .map_err(|error| format!("failed to inspect opened Kagemusha V4 {label}: {error}"))?;
        let snapshot = CatalogDirectorySnapshot::from_metadata(&opened)
            .filter(|snapshot| *snapshot == before)
            .ok_or_else(|| format!("Kagemusha V4 {label} changed while it was opened"))?;
        let after = self.stat_entry_os(name, label)?;
        if CatalogDirectorySnapshot::from_stat(&after) != Some(snapshot) {
            return Err(format!("Kagemusha V4 {label} changed while it was opened"));
        }
        let display_path = self.display_path.join(name);
        let mut path_chain = self.path_chain.clone();
        path_chain.push((display_path.clone(), snapshot));
        Ok(Self {
            display_path,
            file,
            snapshot,
            path_chain,
        })
    }

    fn open_file(&self, name: &str, label: &str) -> Result<CatalogOpenedFile<'_>, String> {
        let before = self.stat_entry(name, label)?;
        let before = CatalogFileSnapshot::from_stat(&before).ok_or_else(|| {
            format!("Kagemusha V4 {label} is not a direct single-link regular file")
        })?;
        let file = File::from(
            openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|error| format!("failed to open Kagemusha V4 {label}: {error}"))?,
        );
        let metadata = file
            .metadata()
            .map_err(|error| format!("failed to inspect opened Kagemusha V4 {label}: {error}"))?;
        let snapshot = CatalogFileSnapshot::from_metadata(&metadata)
            .filter(|snapshot| *snapshot == before)
            .ok_or_else(|| {
                format!(
                    "Kagemusha V4 {label} changed or ceased to be a single-link regular file while it was opened"
                )
            })?;
        let after = self.stat_entry(name, label)?;
        if !snapshot.matches_stat(&after) {
            return Err(format!("Kagemusha V4 {label} changed while it was opened"));
        }
        Ok(CatalogOpenedFile {
            directory: self,
            name: name.to_owned(),
            label: label.to_owned(),
            file,
            snapshot,
        })
    }

    fn verify_opened_snapshot(&self) -> Result<(), String> {
        let metadata = self.file.metadata().map_err(|error| {
            format!(
                "failed to re-inspect Kagemusha V4 directory `{}`: {error}",
                self.display_path.display()
            )
        })?;
        if CatalogDirectorySnapshot::from_metadata(&metadata) != Some(self.snapshot) {
            return Err(format!(
                "Kagemusha V4 directory `{}` changed while the catalog was loaded",
                self.display_path.display()
            ));
        }
        Ok(())
    }

    fn verify_path_identity(&self) -> Result<(), String> {
        self.verify_opened_snapshot()?;
        let reopened = Self::open_path(&self.display_path, "configured directory revalidation")?;
        if reopened.snapshot != self.snapshot || reopened.path_chain != self.path_chain {
            return Err(format!(
                "Kagemusha V4 directory `{}` changed identity while the catalog was loaded",
                self.display_path.display()
            ));
        }
        Ok(())
    }

    fn verify_directory_entry(&self, name: &str, directory: &Self) -> Result<(), String> {
        directory.verify_opened_snapshot()?;
        let stat = self.stat_entry(name, "release directory")?;
        if CatalogDirectorySnapshot::from_stat(&stat) != Some(directory.snapshot) {
            return Err(format!(
                "Kagemusha V4 release directory `{name}` changed while it was loaded"
            ));
        }
        Ok(())
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
struct CatalogOpenedFile<'directory> {
    directory: &'directory CatalogDirectory,
    name: String,
    label: String,
    file: File,
    snapshot: CatalogFileSnapshot,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl CatalogOpenedFile<'_> {
    fn verify_unchanged(&self) -> Result<(), String> {
        let metadata = self.file.metadata().map_err(|error| {
            format!(
                "failed to re-inspect opened Kagemusha V4 {}: {error}",
                self.label
            )
        })?;
        let stat = self.directory.stat_entry(&self.name, &self.label)?;
        if CatalogFileSnapshot::from_metadata(&metadata) != Some(self.snapshot)
            || !self.snapshot.matches_stat(&stat)
        {
            return Err(format!(
                "Kagemusha V4 {} changed while it was read",
                self.label
            ));
        }
        Ok(())
    }

    fn verify_trusted(&self, trusted_uid: u32) -> Result<(), String> {
        self.verify_unchanged()?;
        ensure_root_trusted_stat_v1(self.snapshot.into(), trusted_uid, &self.label)?;
        let path = self.directory.display_path.join(&self.name);
        ensure_no_macos_extended_acl_v1(&path, self.snapshot.into(), &self.label, || {
            self.verify_unchanged()
        })
    }

    fn append_sealed_path(
        &self,
        paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
    ) -> Result<(), String> {
        self.verify_unchanged()?;
        self.directory.append_sealed_path_chain(paths)?;
        insert_sealed_catalog_path_v1(
            paths,
            KagemushaCatalogSealedPathV1 {
                canonical_path: canonical_catalog_path_string_v1(
                    &self.directory.display_path.join(&self.name),
                    &self.label,
                )?,
                kind: KagemushaCatalogSealedPathKindV1::File,
                stat: self.snapshot.into(),
            },
        )
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn current_kagemusha_catalog_executable_path_v1() -> Result<PathBuf, String> {
    let current = std::env::current_exe()
        .map_err(|error| format!("failed to resolve the current Iroha executable: {error}"))?;
    let canonical = fs::canonicalize(&current).map_err(|error| {
        format!(
            "failed to canonicalize current Iroha executable `{}`: {error}",
            current.display()
        )
    })?;
    validate_absolute_catalog_path(&canonical, "current executable")?;
    Ok(canonical)
}

fn current_kagemusha_catalog_build_fingerprint_v1() -> [u8; 32] {
    fn update_framed(hasher: &mut Sha256, value: &[u8]) {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(value);
    }

    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_BUILD_DOMAIN_V1);
    for value in [
        env!("CARGO_PKG_VERSION"),
        option_env!("GIT_COMMIT_HASH").unwrap_or("unknown"),
        option_env!("IROHA_GIT_COMMIT_HASH").unwrap_or("unknown"),
        option_env!("KAGEMUSHA_BUILD_SOURCE_COMMIT").unwrap_or("unknown"),
        option_env!("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256").unwrap_or("unknown"),
        option_env!("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256").unwrap_or("unknown"),
        std::env::consts::ARCH,
        std::env::consts::OS,
        std::env::consts::FAMILY,
        if cfg!(debug_assertions) {
            "debug-assertions"
        } else {
            "release-assertions"
        },
        if cfg!(feature = "circuit-params") {
            "circuit-params"
        } else {
            "no-circuit-params"
        },
        if cfg!(feature = "zk-stark") {
            "zk-stark"
        } else {
            "no-zk-stark"
        },
        if cfg!(feature = "fastpq-gpu") {
            "fastpq-gpu"
        } else {
            "no-fastpq-gpu"
        },
        if cfg!(feature = "privacy-release-evidence") {
            "privacy-release-evidence"
        } else {
            "no-privacy-release-evidence"
        },
        if cfg!(feature = "iroha-core-tests") {
            "iroha-core-tests"
        } else {
            "no-iroha-core-tests"
        },
    ] {
        update_framed(&mut hasher, value.as_bytes());
    }
    hasher.finalize().into()
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn hash_catalog_opened_file_v1(opened: &mut CatalogOpenedFile<'_>) -> Result<[u8; 32], String> {
    opened
        .file
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("failed to rewind Kagemusha V4 {}: {error}", opened.label))?;
    let mut hasher = Sha256::new();
    let mut read_bytes = 0_u64;
    let mut scratch = [0_u8; 64 * 1024];
    loop {
        let read = opened
            .file
            .read(&mut scratch)
            .map_err(|error| format!("failed to hash Kagemusha V4 {}: {error}", opened.label))?;
        if read == 0 {
            break;
        }
        read_bytes = read_bytes
            .checked_add(u64::try_from(read).map_err(|_| "file read length does not fit u64")?)
            .ok_or_else(|| "Kagemusha V4 file hash length overflowed".to_owned())?;
        if read_bytes > opened.snapshot.length {
            return Err(format!(
                "Kagemusha V4 {} grew while it was hashed",
                opened.label
            ));
        }
        hasher.update(&scratch[..read]);
    }
    opened.verify_unchanged()?;
    if read_bytes != opened.snapshot.length {
        return Err(format!(
            "Kagemusha V4 {} changed length while it was hashed",
            opened.label
        ));
    }
    opened.file.seek(SeekFrom::Start(0)).map_err(|error| {
        format!(
            "failed to restore Kagemusha V4 {} cursor: {error}",
            opened.label
        )
    })?;
    Ok(hasher.finalize().into())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn capture_trusted_catalog_file_v1(
    path: &Path,
    label: &str,
    trusted_uid: u32,
    paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
    hash_contents: bool,
) -> Result<Option<[u8; 32]>, String> {
    let (parent_path, file_name) = absolute_file_parent_and_name(path, label)?;
    let parent = CatalogDirectory::open_path(parent_path, &format!("{label} parent"))?;
    parent.verify_trusted_path_chain(trusted_uid, &format!("{label} parent"))?;
    let mut opened = parent.open_file(file_name, label)?;
    opened.verify_trusted(trusted_uid)?;
    opened.append_sealed_path(paths)?;
    let digest = hash_contents
        .then(|| hash_catalog_opened_file_v1(&mut opened))
        .transpose()?;
    opened.verify_unchanged()?;
    parent.verify_path_identity()?;
    Ok(digest)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn capture_trusted_catalog_inventory_v1(
    artifact_dir: &Path,
    catalog: &KagemushaReleaseCatalogV4,
    trusted_uid: u32,
    paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
) -> Result<(), String> {
    let artifact_root = CatalogDirectory::open_path(artifact_dir, "artifact root")?;
    artifact_root.verify_trusted_path_chain(trusted_uid, "artifact root")?;
    artifact_root.append_sealed_path_chain(paths)?;
    let directory_names = artifact_root.entry_names("artifact root")?;
    let expected_names = catalog.releases.keys().map(hex::encode).collect::<Vec<_>>();
    if directory_names != expected_names {
        return Err(
            "Kagemusha V4 catalog changed before its qualification seal was captured".to_owned(),
        );
    }
    for directory_name in &directory_names {
        let manifest_sha256 = parse_manifest_directory_name(directory_name)?;
        let cached = catalog.releases.get(&manifest_sha256).ok_or_else(|| {
            "Kagemusha V4 catalog changed before its qualification seal was captured".to_owned()
        })?;
        let release_directory = artifact_root.open_directory(
            directory_name,
            &format!("release directory `{directory_name}`"),
        )?;
        release_directory.verify_trusted_path_chain(
            trusted_uid,
            &format!("release directory `{directory_name}`"),
        )?;
        release_directory.append_sealed_path_chain(paths)?;
        capture_trusted_catalog_release_inventory_v1(
            &release_directory,
            directory_name,
            manifest_sha256,
            &cached.resolved.pinned_source,
            trusted_uid,
            paths,
        )?;
        artifact_root.verify_directory_entry(directory_name, &release_directory)?;
    }
    artifact_root.verify_path_identity()
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn capture_trusted_catalog_release_inventory_v1(
    release_directory: &CatalogDirectory,
    directory_name: &str,
    manifest_sha256: [u8; 32],
    pinned_source: &KagemushaCatalogPinnedArtifactSourceV4,
    trusted_uid: u32,
    paths: &mut BTreeMap<String, KagemushaCatalogSealedPathV1>,
) -> Result<(), String> {
    pinned_source.validate_snapshot()?;
    if pinned_source.manifest_sha256 != manifest_sha256 {
        return Err(
            "Kagemusha V4 qualification-seal capture selected a different authenticated release"
                .to_owned(),
        );
    }
    let expected_artifacts = pinned_source
        .authenticated_release()
        .manifest()
        .profiles
        .iter()
        .flat_map(|profile| {
            profile
                .artifacts
                .iter()
                .map(move |descriptor| (profile.parity, descriptor))
        })
        .collect::<Vec<_>>();
    let mut captured_roles = BTreeSet::new();
    for file_name in release_directory.entry_names("release inventory")? {
        let opened = release_directory.open_file(
            &file_name,
            &format!("release file `{directory_name}/{file_name}`"),
        )?;
        opened.verify_trusted(trusted_uid)?;
        let pinned_artifact = expected_artifacts
            .iter()
            .find(|(_, descriptor)| descriptor.file_name == file_name);
        if let Some((parity, descriptor)) = pinned_artifact {
            if !captured_roles.insert((*parity, descriptor.kind)) {
                return Err(
                    "Kagemusha V4 qualification-seal capture repeats a pinned artifact role"
                        .to_owned(),
                );
            }
            pinned_source.validate_reopened_artifact_for_seal(*parity, descriptor, &opened)?;
        }
        opened.append_sealed_path(paths)?;
        if let Some((parity, descriptor)) = pinned_artifact {
            pinned_source.validate_reopened_artifact_for_seal(*parity, descriptor, &opened)?;
        }
    }
    if captured_roles.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4 {
        return Err(
            "Kagemusha V4 qualification-seal capture omitted a fully qualified artifact role"
                .to_owned(),
        );
    }
    pinned_source.validate_snapshot()?;
    release_directory.verify_path_identity()
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn build_kagemusha_catalog_qualification_seal_v1(
    policy_path: &Path,
    artifact_dir: &Path,
    catalog: &KagemushaReleaseCatalogV4,
    trusted_uid: u32,
) -> Result<KagemushaCatalogQualificationSealV1, String> {
    let configured_policy_sha256 = catalog
        .configured_policy_sha256
        .ok_or_else(|| "cannot seal an unconfigured Kagemusha V4 release catalog".to_owned())?;
    if catalog.releases.is_empty()
        || catalog.releases.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_RELEASES_V1
    {
        return Err(
            "Kagemusha V4 qualification seal requires a bounded nonempty catalog".to_owned(),
        );
    }

    let canonical_policy_path = canonical_catalog_path_string_v1(policy_path, "release policy")?;
    let canonical_artifact_dir = canonical_catalog_path_string_v1(artifact_dir, "artifact root")?;
    let canonical_executable = current_kagemusha_catalog_executable_path_v1()?;
    let canonical_executable_path =
        canonical_catalog_path_string_v1(&canonical_executable, "current executable")?;

    let mut paths = BTreeMap::new();
    capture_trusted_catalog_file_v1(
        policy_path,
        "release policy",
        trusted_uid,
        &mut paths,
        false,
    )?;
    capture_trusted_catalog_inventory_v1(artifact_dir, catalog, trusted_uid, &mut paths)?;
    let executable_sha256 = capture_trusted_catalog_file_v1(
        &canonical_executable,
        "current executable",
        trusted_uid,
        &mut paths,
        true,
    )?
    .ok_or_else(|| "current executable digest was not captured".to_owned())?;
    if paths.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1 {
        return Err(format!(
            "Kagemusha V4 qualification seal contains {} paths; at most {} are allowed",
            paths.len(),
            KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1
        ));
    }

    let mut releases = Vec::with_capacity(catalog.releases.len());
    for (manifest_sha256, cached) in &catalog.releases {
        let authenticated = cached.resolved.release();
        let manifest = authenticated.manifest();
        if authenticated.manifest_sha256() != *manifest_sha256 {
            return Err("Kagemusha V4 catalog release identity changed before sealing".to_owned());
        }
        let promotion_bytes = norito::encode_canonical(&cached.release_record.promotion_record)
            .map_err(|error| {
                format!(
                    "failed to encode Kagemusha V4 promotion record for qualification seal: {error}"
                )
            })?;
        let artifacts = manifest
            .profiles
            .iter()
            .flat_map(|profile| {
                profile.artifacts.iter().cloned().map(move |artifact| {
                    KagemushaCatalogSealedArtifactDigestV1 {
                        parity: profile.parity,
                        artifact,
                    }
                })
            })
            .collect();
        releases.push(KagemushaCatalogSealedReleaseQualificationV1 {
            manifest_sha256: *manifest_sha256,
            release_attestation_sha256: authenticated.release_attestation_sha256(),
            source_commit: manifest.source_commit.clone(),
            source_tree_sha256: manifest.source_tree_sha256,
            reviewed_source_closure_descriptor_sha256: manifest
                .reviewed_source_closure_descriptor_sha256,
            benchmark_evidence_sha256: manifest.benchmark_evidence_sha256,
            cryptographic_review_sha256: manifest.cryptographic_review_sha256,
            promotion_record_sha256: Sha256::digest(promotion_bytes).into(),
            artifacts,
            step_eq: KagemushaCatalogSealedParityQualificationV1::from_qualified(
                cached
                    .resolved
                    .parity_metadata(KagemushaPastaCycleParityV1::StepEq),
                profile(manifest, KagemushaPastaCycleParityV1::StepEq)?
                    .compiled_protocol_structure_sha256,
            )?,
            step_ep: KagemushaCatalogSealedParityQualificationV1::from_qualified(
                cached
                    .resolved
                    .parity_metadata(KagemushaPastaCycleParityV1::StepEp),
                profile(manifest, KagemushaPastaCycleParityV1::StepEp)?
                    .compiled_protocol_structure_sha256,
            )?,
        });
    }
    let seal = KagemushaCatalogQualificationSealV1 {
        schema: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1.to_owned(),
        version: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1,
        canonical_policy_path,
        canonical_artifact_dir,
        canonical_executable_path,
        build_fingerprint_sha256: current_kagemusha_catalog_build_fingerprint_v1(),
        executable_sha256,
        configured_policy_sha256,
        paths: paths.into_values().collect(),
        releases,
    };
    seal.validate_for_configured_runtime(policy_path, artifact_dir)?;
    Ok(seal)
}

impl KagemushaCatalogQualificationSealV1 {
    fn validate_layout(&self) -> Result<(), String> {
        if self.schema != KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1
            || self.version != KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1
            || self.build_fingerprint_sha256 == [0; 32]
            || self.executable_sha256 == [0; 32]
            || self.configured_policy_sha256 == [0; 32]
            || self.paths.is_empty()
            || self.paths.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1
            || self.releases.is_empty()
            || self.releases.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_RELEASES_V1
        {
            return Err("Kagemusha V4 qualification seal header or bounds are invalid".to_owned());
        }
        for (path, label) in [
            (&self.canonical_policy_path, "sealed release policy"),
            (&self.canonical_artifact_dir, "sealed artifact root"),
            (&self.canonical_executable_path, "sealed current executable"),
        ] {
            validate_absolute_catalog_path(Path::new(path), label)?;
        }
        let mut previous_path: Option<&str> = None;
        for path in &self.paths {
            validate_absolute_catalog_path(
                Path::new(&path.canonical_path),
                "qualification-seal path",
            )?;
            if previous_path.is_some_and(|previous| previous >= path.canonical_path.as_str())
                || path.stat.links == 0
                || !(0..1_000_000_000).contains(&path.stat.modified_nanoseconds)
                || !(0..1_000_000_000).contains(&path.stat.changed_nanoseconds)
            {
                return Err(
                    "Kagemusha V4 qualification seal path inventory is not canonical".to_owned(),
                );
            }
            previous_path = Some(&path.canonical_path);
        }
        for required in [
            (
                self.canonical_policy_path.as_str(),
                KagemushaCatalogSealedPathKindV1::File,
            ),
            (
                self.canonical_artifact_dir.as_str(),
                KagemushaCatalogSealedPathKindV1::Directory,
            ),
            (
                self.canonical_executable_path.as_str(),
                KagemushaCatalogSealedPathKindV1::File,
            ),
        ] {
            if !self
                .paths
                .iter()
                .any(|path| (path.canonical_path.as_str(), path.kind) == required)
            {
                return Err(
                    "Kagemusha V4 qualification seal omits a configured path identity".to_owned(),
                );
            }
        }
        let mut previous_release = None;
        for release in &self.releases {
            if release.manifest_sha256 == [0; 32]
                || release.release_attestation_sha256 == [0; 32]
                || release.source_commit.is_empty()
                || release.source_tree_sha256 == [0; 32]
                || release.reviewed_source_closure_descriptor_sha256 == [0; 32]
                || release.benchmark_evidence_sha256 == [0; 32]
                || release.cryptographic_review_sha256 == [0; 32]
                || release.promotion_record_sha256 == [0; 32]
                || previous_release.is_some_and(|previous| previous >= release.manifest_sha256)
                || release.artifacts.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
            {
                return Err(
                    "Kagemusha V4 qualification seal release inventory is invalid".to_owned(),
                );
            }
            previous_release = Some(release.manifest_sha256);
            let mut roles = BTreeSet::new();
            let mut file_names = BTreeSet::new();
            for artifact in &release.artifacts {
                artifact
                    .artifact
                    .validate()
                    .map_err(|error| format!("invalid sealed Kagemusha artifact: {error}"))?;
                if !roles.insert((artifact.parity, artifact.artifact.kind))
                    || !file_names.insert(artifact.artifact.file_name.as_str())
                {
                    return Err(
                        "Kagemusha V4 qualification seal repeats an artifact role or file"
                            .to_owned(),
                    );
                }
            }
            if release.step_eq.compiled_protocol_structure_sha256 == [0; 32]
                || release.step_ep.compiled_protocol_structure_sha256 == [0; 32]
                || release.step_eq.compiled_protocol_structure_sha256
                    == release.step_eq.compiled_protocol_identity_sha256
                || release.step_ep.compiled_protocol_structure_sha256
                    == release.step_ep.compiled_protocol_identity_sha256
            {
                return Err(
                    "Kagemusha V4 qualification seal compiled-protocol bindings are invalid"
                        .to_owned(),
                );
            }
            release.step_eq.to_qualified()?;
            release.step_ep.to_qualified()?;
        }
        Ok(())
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn validate_for_configured_runtime(
        &self,
        policy_path: &Path,
        artifact_dir: &Path,
    ) -> Result<(), String> {
        self.validate_layout()?;
        if self.canonical_policy_path
            != canonical_catalog_path_string_v1(policy_path, "release policy")?
            || self.canonical_artifact_dir
                != canonical_catalog_path_string_v1(artifact_dir, "artifact root")?
            || self.build_fingerprint_sha256 != current_kagemusha_catalog_build_fingerprint_v1()
        {
            return Err(
                "Kagemusha V4 qualification seal is stale for the configured paths or build"
                    .to_owned(),
            );
        }
        let executable = current_kagemusha_catalog_executable_path_v1()?;
        if self.canonical_executable_path
            != canonical_catalog_path_string_v1(&executable, "current executable")?
        {
            return Err(
                "Kagemusha V4 qualification seal is stale for the current executable path"
                    .to_owned(),
            );
        }
        Ok(())
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn read_root_trusted_kagemusha_catalog_qualification_seal_v1(
    seal_path: &Path,
    trusted_uid: u32,
) -> Result<KagemushaCatalogQualificationSealV1, String> {
    let (parent_path, file_name) =
        absolute_file_parent_and_name(seal_path, "catalog qualification seal")?;
    let parent = CatalogDirectory::open_path(parent_path, "catalog qualification seal parent")?;
    parent.verify_trusted_path_chain(trusted_uid, "catalog qualification seal parent")?;
    let mut opened = parent.open_file(file_name, "catalog qualification seal")?;
    opened.verify_trusted(trusted_uid)?;
    let bytes = read_bounded_opened_file(
        &mut opened,
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1,
        "catalog qualification seal",
    )?;
    let limits = norito::core::DecodeLimits::new(
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1,
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1,
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1.saturating_mul(16),
        KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1.saturating_mul(2),
        64,
    );
    let seal: KagemushaCatalogQualificationSealV1 =
        norito::decode_canonical_with_limits(&bytes, limits).map_err(|error| {
            format!("failed to decode canonical Kagemusha catalog qualification seal: {error}")
        })?;
    seal.validate_layout()?;
    opened.verify_unchanged()?;
    parent.verify_path_identity()?;
    Ok(seal)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn verify_kagemusha_catalog_sealed_paths_v1(
    paths: &[KagemushaCatalogSealedPathV1],
    trusted_uid: u32,
) -> Result<(), String> {
    if paths.is_empty() || paths.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1 {
        return Err("Kagemusha V4 qualification seal has an invalid path count".to_owned());
    }
    for sealed in paths {
        let path = Path::new(&sealed.canonical_path);
        match sealed.kind {
            KagemushaCatalogSealedPathKindV1::Directory => {
                let directory =
                    CatalogDirectory::open_path(path, "sealed catalog directory revalidation")?;
                directory.verify_trusted_path_chain(
                    trusted_uid,
                    "sealed catalog directory revalidation",
                )?;
                if KagemushaCatalogSealedStatV1::from(directory.snapshot) != sealed.stat {
                    return Err(format!(
                        "Kagemusha V4 sealed directory `{}` changed identity",
                        path.display()
                    ));
                }
            }
            KagemushaCatalogSealedPathKindV1::File => {
                let (parent_path, file_name) =
                    absolute_file_parent_and_name(path, "sealed catalog file revalidation")?;
                let parent = CatalogDirectory::open_path(
                    parent_path,
                    "sealed catalog file parent revalidation",
                )?;
                parent.verify_trusted_path_chain(
                    trusted_uid,
                    "sealed catalog file parent revalidation",
                )?;
                let opened = parent.open_file(file_name, "sealed catalog file revalidation")?;
                opened.verify_trusted(trusted_uid)?;
                if KagemushaCatalogSealedStatV1::from(opened.snapshot) != sealed.stat {
                    return Err(format!(
                        "Kagemusha V4 sealed file `{}` changed identity",
                        path.display()
                    ));
                }
                parent.verify_path_identity()?;
            }
        }
    }
    Ok(())
}

/// One exact manifest role retained as the descriptor-relative inode opened at
/// catalog startup.  The descriptor is kept beside the handle so role lookup
/// cannot be redirected by a later path replacement.
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[derive(Debug)]
struct KagemushaCatalogPinnedArtifactV4 {
    parity: KagemushaPastaCycleParityV1,
    descriptor: KagemushaPastaCycleArtifactV4,
    file: Mutex<File>,
    snapshot: CatalogFileSnapshot,
    authenticated_inspection: Option<KagemushaAuthenticatedArtifactInspectionV4>,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl KagemushaCatalogPinnedArtifactV4 {
    fn validate_locked_file(&self, file: &File) -> Result<(), String> {
        let metadata = file.metadata().map_err(|error| {
            format!(
                "failed to inspect pinned Kagemusha V4 artifact `{}`: {error}",
                self.descriptor.file_name
            )
        })?;
        let flags = fcntl_getfl(file).map_err(|error| {
            format!(
                "failed to inspect pinned Kagemusha V4 artifact access mode `{}`: {error}",
                self.descriptor.file_name
            )
        })?;
        if CatalogFileSnapshot::from_metadata(&metadata) != Some(self.snapshot)
            || self.snapshot.identity != CatalogFileIdentity::from_metadata(&metadata)
            || self.snapshot.length != self.descriptor.size_bytes
            || flags & OFlags::ACCMODE != OFlags::RDONLY
        {
            return Err(format!(
                "pinned Kagemusha V4 artifact `{}` changed identity, bytes, or read-only access mode",
                self.descriptor.file_name
            ));
        }
        Ok(())
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn lock_kagemusha_catalog_source_mutex_v4<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        // These mutexes serialize immutable, read-only pinned artifacts. Every
        // use rewinds, validates the descriptor snapshot, and is fully
        // reauthenticated by core, so a caught panic must not brick the source.
        Err(poisoned) => {
            mutex.clear_poison();
            poisoned.into_inner()
        }
    }
}

/// Exact-eight, read-only source retained by one authenticated catalog release.
///
/// Every handle is opened relative to the already pinned release directory and
/// is never reopened by path. A source-wide permit prevents Eq/Ep or role
/// readers from overlapping; each file also owns its cursor mutex so clones of
/// the source cannot race a rewind. Full qualification retains one
/// complete-frame inspection per role. A sealed restart retains only the
/// root-trusted inode identity and reauthenticates every byte when a later
/// parser actually consumes the role.
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
#[derive(Debug)]
pub(crate) struct KagemushaCatalogPinnedArtifactSourceV4 {
    release: KagemushaAuthenticatedReleaseV4,
    manifest_sha256: [u8; 32],
    artifacts: Vec<KagemushaCatalogPinnedArtifactV4>,
    access_permit: Mutex<()>,
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl KagemushaCatalogPinnedArtifactSourceV4 {
    fn open(
        directory: &CatalogDirectory,
        release: KagemushaAuthenticatedReleaseV4,
    ) -> Result<Self, String> {
        let mut source = Self::open_pinned(directory, release)?;
        source.authenticate_inventory_once()?;
        Ok(source)
    }

    fn open_pinned(
        directory: &CatalogDirectory,
        release: KagemushaAuthenticatedReleaseV4,
    ) -> Result<Self, String> {
        release
            .manifest()
            .validate()
            .map_err(|error| error.to_string())?;
        let manifest_sha256 = release.manifest_sha256();
        let expected = release
            .manifest()
            .profiles
            .iter()
            .flat_map(|profile| {
                profile
                    .artifacts
                    .iter()
                    .map(move |descriptor| (profile.parity, descriptor))
            })
            .collect::<Vec<_>>();
        let unique_roles = expected
            .iter()
            .map(|(parity, descriptor)| (*parity, descriptor.kind))
            .collect::<BTreeSet<_>>();
        let unique_descriptors = expected
            .iter()
            .map(|(parity, descriptor)| (*parity, (*descriptor).clone()))
            .collect::<BTreeSet<_>>();
        let unique_file_names = expected
            .iter()
            .map(|(_, descriptor)| descriptor.file_name.as_str())
            .collect::<BTreeSet<_>>();
        if manifest_sha256 == [0; 32]
            || expected.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
            || unique_roles.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
            || unique_descriptors.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
            || unique_file_names.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
        {
            return Err(
                "Kagemusha V4 pinned catalog source requires one exact-eight authenticated release"
                    .to_owned(),
            );
        }

        let mut artifacts = Vec::with_capacity(expected.len());
        for (parity, descriptor) in expected {
            let opened = directory.open_file(
                &descriptor.file_name,
                &format!("artifact `{}`", descriptor.file_name),
            )?;
            opened.verify_unchanged()?;
            let artifact = KagemushaCatalogPinnedArtifactV4 {
                parity,
                descriptor: descriptor.clone(),
                snapshot: opened.snapshot,
                file: Mutex::new(opened.file),
                authenticated_inspection: None,
            };
            {
                let file = lock_kagemusha_catalog_source_mutex_v4(&artifact.file);
                artifact.validate_locked_file(&file)?;
            }
            artifacts.push(artifact);
        }

        let source = Self {
            release,
            manifest_sha256,
            artifacts,
            access_permit: Mutex::new(()),
        };
        source.validate_snapshot()?;
        Ok(source)
    }

    fn artifact(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Result<&KagemushaCatalogPinnedArtifactV4, String> {
        let descriptor = kagemusha_artifact_descriptor_v4(self.release.manifest(), parity, kind)?;
        self.artifacts
            .iter()
            .find(|artifact| {
                artifact.parity == parity
                    && artifact.descriptor.kind == kind
                    && artifact.descriptor == *descriptor
            })
            .ok_or_else(|| {
                "pinned Kagemusha V4 catalog source returned no exact artifact role".to_owned()
            })
    }

    fn validate_snapshot(&self) -> Result<(), String> {
        let expected = self.release.manifest().profiles.iter().flat_map(|profile| {
            profile
                .artifacts
                .iter()
                .map(move |descriptor| (profile.parity, descriptor))
        });
        if self.manifest_sha256 != self.release.manifest_sha256()
            || self.artifacts.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
        {
            return Err("pinned Kagemusha V4 catalog source identity changed".to_owned());
        }
        for (artifact, (parity, descriptor)) in self.artifacts.iter().zip(expected) {
            if artifact.parity != parity || artifact.descriptor != *descriptor {
                return Err("pinned Kagemusha V4 catalog source role inventory changed".to_owned());
            }
            let file = lock_kagemusha_catalog_source_mutex_v4(&artifact.file);
            artifact.validate_locked_file(&file)?;
        }
        Ok(())
    }

    fn validate_reopened_artifact_for_seal(
        &self,
        parity: KagemushaPastaCycleParityV1,
        descriptor: &KagemushaPastaCycleArtifactV4,
        reopened: &CatalogOpenedFile<'_>,
    ) -> Result<(), String> {
        self.validate_snapshot()?;
        let pinned = self.artifact(parity, descriptor.kind)?;
        if pinned.parity != parity
            || pinned.descriptor != *descriptor
            || pinned.snapshot != reopened.snapshot
        {
            return Err(format!(
                "Kagemusha V4 qualification-seal capture reopened artifact `{}` on an inode different from the fully qualified pinned source",
                descriptor.file_name
            ));
        }
        {
            let file = lock_kagemusha_catalog_source_mutex_v4(&pinned.file);
            pinned.validate_locked_file(&file)?;
        }
        reopened.verify_unchanged()?;
        self.validate_snapshot()
    }

    fn authenticate_inventory_once(&mut self) -> Result<(), String> {
        self.validate_snapshot()?;
        for artifact in &mut self.artifacts {
            #[cfg(test)]
            if KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(std::cell::Cell::get) {
                return Err(format!(
                    "Kagemusha V4 test sentinel rejected a {:?} payload read",
                    artifact.descriptor.kind
                ));
            }
            let mut file = lock_kagemusha_catalog_source_mutex_v4(&artifact.file);
            artifact.validate_locked_file(&file)?;
            file.seek(SeekFrom::Start(0)).map_err(|error| {
                format!("failed to rewind pinned Kagemusha V4 artifact: {error}")
            })?;
            let inspection = inspect_kagemusha_pasta_cycle_artifact_v4(
                &mut *file,
                &self.release,
                &artifact.descriptor,
            )?;
            file.seek(SeekFrom::Start(0)).map_err(|error| {
                format!("failed to restore pinned Kagemusha V4 artifact cursor: {error}")
            })?;
            artifact.validate_locked_file(&file)?;
            drop(file);
            artifact.authenticated_inspection = Some(inspection);
        }
        Ok(())
    }

    fn with_selected_file<T>(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
        consume: impl FnOnce(&mut File) -> Result<T, String>,
    ) -> Result<T, String> {
        let _access = lock_kagemusha_catalog_source_mutex_v4(&self.access_permit);
        let artifact = self.artifact(parity, kind)?;
        #[cfg(test)]
        if KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(std::cell::Cell::get) {
            return Err(format!(
                "Kagemusha V4 test sentinel rejected a {kind:?} payload read"
            ));
        }
        let mut file = lock_kagemusha_catalog_source_mutex_v4(&artifact.file);
        artifact.validate_locked_file(&file)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind pinned Kagemusha V4 artifact: {error}"))?;
        let outcome = consume(&mut file);
        let rewind = file.seek(SeekFrom::Start(0)).map(|_| ()).map_err(|error| {
            format!("failed to restore pinned Kagemusha V4 artifact cursor: {error}")
        });
        let snapshot = artifact.validate_locked_file(&file);
        match (outcome, rewind, snapshot) {
            (Err(error), _, _) => Err(error),
            (Ok(_), Err(error), _) | (Ok(_), Ok(()), Err(error)) => Err(error),
            (Ok(value), Ok(()), Ok(())) => Ok(value),
        }
    }
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
impl KagemushaAuthenticatedArtifactSourceV4 for KagemushaCatalogPinnedArtifactSourceV4 {
    fn authenticated_release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.release
    }

    fn with_framed_artifact(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
        consume: &mut dyn FnMut(&mut dyn KagemushaArtifactReadSeekV4) -> Result<(), String>,
    ) -> Result<(), String> {
        self.with_selected_file(parity, kind, |file| consume(file))
    }

    fn authenticated_inspection(
        &self,
        parity: KagemushaPastaCycleParityV1,
        kind: KagemushaPastaCycleArtifactKindV4,
    ) -> Result<Option<KagemushaAuthenticatedArtifactInspectionV4>, String> {
        let artifact = self.artifact(parity, kind)?;
        let file = lock_kagemusha_catalog_source_mutex_v4(&artifact.file);
        artifact.validate_locked_file(&file)?;
        Ok(artifact.authenticated_inspection.clone())
    }
}

#[cfg(all(
    test,
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, String> {
    let (parent_path, file_name) = absolute_file_parent_and_name(path, label)?;
    let parent = CatalogDirectory::open_path(parent_path, &format!("{label} parent"))?;
    let mut opened = parent.open_file(file_name, label)?;
    let bytes = read_bounded_opened_file(&mut opened, maximum, label)?;
    parent.verify_path_identity()?;
    Ok(bytes)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn read_bounded_directory_file(
    directory: &CatalogDirectory,
    file_name: &str,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let mut opened = directory.open_file(file_name, label)?;
    read_bounded_opened_file(&mut opened, maximum, label)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn read_bounded_opened_file(
    opened: &mut CatalogOpenedFile<'_>,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let size = usize::try_from(opened.snapshot.length)
        .map_err(|_| format!("Kagemusha V4 {label} length does not fit usize"))?;
    if size == 0 || size > maximum {
        return Err(format!(
            "Kagemusha V4 {label} is not a bounded regular file"
        ));
    }
    let limit = u64::try_from(maximum).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(size);
    Read::by_ref(&mut opened.file)
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 {label}: {error}"))?;
    opened.verify_unchanged()?;
    if bytes.len() != size {
        return Err(format!("Kagemusha V4 {label} changed while it was read"));
    }
    Ok(bytes)
}

fn decode_canonical_manifest(
    bytes: &[u8],
) -> Result<KagemushaRecursiveSpendArtifactManifestV4, String> {
    let manifest: KagemushaRecursiveSpendArtifactManifestV4 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha V4 manifest is malformed".to_owned())?;
    if norito::to_bytes(&manifest)
        .map_err(|error| format!("failed to encode Kagemusha V4 manifest: {error}"))?
        != bytes
    {
        return Err("Kagemusha V4 manifest is not canonical Norito".to_owned());
    }
    manifest.validate().map_err(|error| error.to_string())?;
    Ok(manifest)
}

fn decode_canonical_attestation(
    bytes: &[u8],
) -> Result<KagemushaRecursiveSpendReleaseAttestationV4, String> {
    let attestation: KagemushaRecursiveSpendReleaseAttestationV4 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha V4 release attestation is malformed".to_owned())?;
    if norito::to_bytes(&attestation)
        .map_err(|error| format!("failed to encode Kagemusha V4 attestation: {error}"))?
        != bytes
    {
        return Err("Kagemusha V4 release attestation is not canonical Norito".to_owned());
    }
    Ok(attestation)
}

fn decode_canonical_promotion(
    bytes: &[u8],
) -> Result<iroha_data_model::offline::KagemushaRecursiveSpendPromotedReleaseV4, String> {
    let promotion: iroha_data_model::offline::KagemushaRecursiveSpendPromotedReleaseV4 =
        norito::decode_from_bytes(bytes)
            .map_err(|_| "Kagemusha V4 promotion record is malformed".to_owned())?;
    if norito::to_bytes(&promotion)
        .map_err(|error| format!("failed to encode Kagemusha V4 promotion record: {error}"))?
        != bytes
    {
        return Err("Kagemusha V4 promotion record is not canonical Norito".to_owned());
    }
    promotion.validate().map_err(|error| error.to_string())?;
    Ok(promotion)
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn verify_file_descriptor(
    directory: &CatalogDirectory,
    file_name: &str,
    expected_size: u64,
    expected_sha256: [u8; 32],
    maximum: u64,
    label: &str,
) -> Result<(), String> {
    let mut opened = directory.open_file(file_name, label)?;
    if opened.snapshot.length != expected_size || expected_size == 0 || expected_size > maximum {
        return Err(format!("Kagemusha V4 {label} size or file type mismatch"));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        let count = opened
            .file
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash Kagemusha V4 {label}: {error}"))?;
        if count == 0 {
            break;
        }
        read = read
            .checked_add(u64::try_from(count).map_err(|_| "artifact read length overflow")?)
            .ok_or_else(|| "Kagemusha V4 artifact read length overflow".to_owned())?;
        if read > expected_size {
            return Err(format!("Kagemusha V4 {label} grew while it was read"));
        }
        hasher.update(&buffer[..count]);
    }
    let actual_sha256: [u8; 32] = hasher.finalize().into();
    opened.verify_unchanged()?;
    if read != expected_size || actual_sha256 != expected_sha256 {
        return Err(format!("Kagemusha V4 {label} digest mismatch"));
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn verify_exact_release_inventory_v4(
    directory: &CatalogDirectory,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<u64, String> {
    let mut expected = BTreeSet::from([
        MANIFEST_FILE_NAME_V4.to_owned(),
        MANIFEST_JSON_FILE_NAME_V4.to_owned(),
        MANIFEST_SHA256_FILE_NAME_V4.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1.to_owned(),
        PROMOTION_RECORD_FILE_NAME_V4.to_owned(),
        manifest.topup_finality_roster_artifact.file_name.clone(),
    ]);
    expected.extend(
        manifest
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .map(|artifact| artifact.file_name.clone()),
    );
    if expected.len() != 16 {
        return Err(
            "Kagemusha V4 release inventory does not describe exactly sixteen unique files"
                .to_owned(),
        );
    }

    let mut observed = BTreeSet::new();
    let mut aggregate_bytes = 0_u64;
    for name in directory.entry_names("release inventory")? {
        let stat = directory.stat_entry(&name, &format!("release file `{name}`"))?;
        let snapshot = CatalogFileSnapshot::from_stat(&stat).ok_or_else(|| {
            format!("Kagemusha V4 release entry `{name}` is not a direct single-link regular file")
        })?;
        aggregate_bytes = aggregate_bytes
            .checked_add(snapshot.length)
            .ok_or_else(|| "Kagemusha V4 release inventory byte count overflowed".to_owned())?;
        if aggregate_bytes > MAX_CATALOG_AGGREGATE_BYTES_V4 {
            return Err(format!(
                "Kagemusha V4 release inventory exceeds the aggregate byte limit of {MAX_CATALOG_AGGREGATE_BYTES_V4}"
            ));
        }
        observed.insert(name);
    }
    if observed != expected {
        return Err(format!(
            "Kagemusha V4 release inventory is not exact (missing={:?}, unexpected={:?})",
            expected.difference(&observed).collect::<Vec<_>>(),
            observed.difference(&expected).collect::<Vec<_>>()
        ));
    }
    Ok(aggregate_bytes)
}

fn checked_decoded_estimate_headroom_v4(bytes: u64) -> Result<u64, String> {
    bytes
        .checked_mul(DECODED_ESTIMATE_HEADROOM_NUMERATOR_V4)
        .and_then(|value| {
            value.checked_add(DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4.saturating_sub(1))
        })
        .and_then(|value| value.checked_div(DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4))
        .ok_or_else(|| "Kagemusha V4 decoded catalog memory estimate overflowed".to_owned())
}

fn profile_artifact_payload_bytes_v4(
    profile: &iroha_data_model::offline::KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<u64, String> {
    profile
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .map(|artifact| artifact.payload_size_bytes)
        .ok_or_else(|| {
            format!(
                "Kagemusha V4 profile is missing the {kind:?} artifact required for memory accounting"
            )
        })
}

fn validate_catalog_artifact_encoding_sizes_v4(
    manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<(), String> {
    for profile in &manifest.profiles {
        let sizes = kagemusha_artifact_encoding_sizes_v4(&profile.circuit_params, profile.parity)?;
        for (kind, label, expected_bytes) in [
            (
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                "parameters",
                sizes.parameters_bytes,
            ),
            (
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
                "proving key",
                sizes.proving_key_bytes,
            ),
            (
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
                "verifier key",
                sizes.verifying_key_bytes,
            ),
        ] {
            if expected_bytes == 0
                || expected_bytes >= KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
            {
                return Err(format!(
                    "Kagemusha V4 {label} length {expected_bytes} violates the fixed {}-byte artifact-size corridor",
                    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
                ));
            }
            let declared_bytes = profile_artifact_payload_bytes_v4(profile, kind)?;
            if declared_bytes != expected_bytes {
                return Err(format!(
                    "Kagemusha V4 {label} descriptor length {declared_bytes} does not match the exact authenticated shape length {expected_bytes}"
                ));
            }
        }
    }
    Ok(())
}

fn estimate_catalog_release_memory_v4(
    manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<KagemushaCatalogMemoryEstimateV4, String> {
    let mut persistent_bytes = CATALOG_RELEASE_METADATA_PERSISTENT_BYTES_V4;
    let mut largest_transient_payload_bytes = 0_u64;
    for profile in &manifest.profiles {
        let rows = 1_u64
            .checked_shl(profile.ipa_k)
            .ok_or_else(|| "Kagemusha V4 parameter row estimate overflowed".to_owned())?;
        let parsed_params_bytes = rows
            .checked_mul(PARSED_PARAMS_BYTES_PER_ROW_V4)
            .ok_or_else(|| "Kagemusha V4 parsed parameter estimate overflowed".to_owned())?;
        let parsed_verifying_key_domain_bytes = rows
            .checked_mul(PARSED_VERIFYING_KEY_DOMAIN_BYTES_PER_ROW_V4)
            .ok_or_else(|| "Kagemusha V4 parsed verifier domain estimate overflowed".to_owned())?;
        let params_payload_bytes = profile_artifact_payload_bytes_v4(
            profile,
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
        )?;
        let verifying_key_payload_bytes = profile_artifact_payload_bytes_v4(
            profile,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        )?;
        let bootstrap_payload_bytes = profile_artifact_payload_bytes_v4(
            profile,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        )?;
        let retained_and_parsed_verifying_key_bytes = verifying_key_payload_bytes
            .checked_mul(1 + PARSED_VERIFYING_KEY_EXPANSION_V4)
            .ok_or_else(|| "Kagemusha V4 verifier-key estimate overflowed".to_owned())?;
        persistent_bytes = persistent_bytes
            .checked_add(parsed_params_bytes)
            .and_then(|value| value.checked_add(parsed_verifying_key_domain_bytes))
            .and_then(|value| value.checked_add(retained_and_parsed_verifying_key_bytes))
            .ok_or_else(|| "Kagemusha V4 persistent memory estimate overflowed".to_owned())?;
        // The descriptor-relative loader opens and drops one raw role before
        // requesting the next. Only the largest payload overlaps the final
        // parsed verifier set; summing six raw payloads would model the retired
        // all-Vec loader rather than production behavior.
        largest_transient_payload_bytes = largest_transient_payload_bytes.max(
            params_payload_bytes
                .max(verifying_key_payload_bytes)
                .max(bootstrap_payload_bytes),
        );
    }
    let peak_load_bytes = persistent_bytes
        .checked_add(CATALOG_RELEASE_METADATA_TRANSIENT_BYTES_V4)
        .and_then(|value| value.checked_add(largest_transient_payload_bytes))
        .ok_or_else(|| "Kagemusha V4 peak memory estimate overflowed".to_owned())?;
    Ok(KagemushaCatalogMemoryEstimateV4 {
        persistent_bytes: checked_decoded_estimate_headroom_v4(persistent_bytes)?,
        peak_load_bytes: checked_decoded_estimate_headroom_v4(peak_load_bytes)?,
    })
}

fn validate_sealed_release_qualification_v1(
    sealed: &KagemushaCatalogSealedReleaseQualificationV1,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    promotion_bytes: &[u8],
) -> Result<(), String> {
    let manifest = authenticated.manifest();
    let artifacts = manifest
        .profiles
        .iter()
        .flat_map(|profile| {
            profile.artifacts.iter().cloned().map(move |artifact| {
                KagemushaCatalogSealedArtifactDigestV1 {
                    parity: profile.parity,
                    artifact,
                }
            })
        })
        .collect::<Vec<_>>();
    if sealed.manifest_sha256 != authenticated.manifest_sha256()
        || sealed.release_attestation_sha256 != authenticated.release_attestation_sha256()
        || sealed.source_commit != manifest.source_commit
        || sealed.source_tree_sha256 != manifest.source_tree_sha256
        || sealed.reviewed_source_closure_descriptor_sha256
            != manifest.reviewed_source_closure_descriptor_sha256
        || sealed.benchmark_evidence_sha256 != manifest.benchmark_evidence_sha256
        || sealed.cryptographic_review_sha256 != manifest.cryptographic_review_sha256
        || sealed.promotion_record_sha256 != <[u8; 32]>::from(Sha256::digest(promotion_bytes))
        || sealed.artifacts != artifacts
    {
        return Err(
            "Kagemusha V4 sealed release identity or artifact digest inventory mismatch".to_owned(),
        );
    }
    let eq_profile = profile(manifest, KagemushaPastaCycleParityV1::StepEq)?;
    let ep_profile = profile(manifest, KagemushaPastaCycleParityV1::StepEp)?;
    let eq_vk = kagemusha_artifact_descriptor_v4(
        manifest,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let ep_vk = kagemusha_artifact_descriptor_v4(
        manifest,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    if sealed.step_eq.parity != KagemushaPastaCycleParityV1::StepEq
        || sealed.step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        || sealed.step_eq.circuit_params != eq_profile.circuit_params
        || sealed.step_ep.circuit_params != ep_profile.circuit_params
        || sealed.step_eq.compiled_protocol_structure_sha256
            != eq_profile.compiled_protocol_structure_sha256
        || sealed.step_ep.compiled_protocol_structure_sha256
            != ep_profile.compiled_protocol_structure_sha256
        || sealed.step_eq.processed_verifying_key_len != eq_vk.payload_size_bytes
        || sealed.step_ep.processed_verifying_key_len != ep_vk.payload_size_bytes
        || sealed.step_eq.processed_verifying_key_sha256 != eq_vk.payload_sha256
        || sealed.step_ep.processed_verifying_key_sha256 != ep_vk.payload_sha256
    {
        return Err("Kagemusha V4 sealed Eq/Ep qualification metadata mismatch".to_owned());
    }
    // A compiled-protocol identity deliberately includes the separately sealed
    // value-free structure digest plus the final VK's preprocessed points and
    // transcript state. It therefore must not equal the structure digest.
    // The root-trusted seal binds both values to the exact executable,
    // manifest, and eight artifact descriptors; `to_qualified` rejects an
    // empty or internally inconsistent full identity.
    sealed.step_eq.to_qualified()?;
    sealed.step_ep.to_qualified()?;
    if sealed.step_eq.compiled_protocol_identity_sha256
        == sealed.step_ep.compiled_protocol_identity_sha256
        || sealed.step_eq.verifying_key_commitment == sealed.step_ep.verifying_key_commitment
    {
        return Err("Kagemusha V4 sealed Eq/Ep qualified identities are not distinct".to_owned());
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn open_qualified_kagemusha_catalog_source_v4(
    directory: &CatalogDirectory,
    authenticated: KagemushaAuthenticatedReleaseV4,
    sealed_qualification: Option<&KagemushaCatalogSealedReleaseQualificationV1>,
) -> Result<
    (
        Arc<KagemushaCatalogPinnedArtifactSourceV4>,
        Arc<KagemushaQualifiedArtifactSourceV4>,
    ),
    String,
> {
    let pinned_source = Arc::new(if sealed_qualification.is_some() {
        KagemushaCatalogPinnedArtifactSourceV4::open_pinned(directory, authenticated.clone())?
    } else {
        KagemushaCatalogPinnedArtifactSourceV4::open(directory, authenticated.clone())?
    });
    let source: Arc<dyn KagemushaAuthenticatedArtifactSourceV4> = pinned_source.clone();
    let qualified_source = Arc::new(if let Some(sealed) = sealed_qualification {
        KagemushaQualifiedArtifactSourceV4::new(
            source,
            authenticated,
            sealed.step_eq.to_qualified()?,
            sealed.step_ep.to_qualified()?,
        )?
    } else {
        qualify_kagemusha_authenticated_artifact_source_v4(source)?
    });
    Ok((pinned_source, qualified_source))
}

#[allow(clippy::too_many_lines)]
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn load_release_directory(
    directory: &CatalogDirectory,
    expected_manifest_sha256: [u8; 32],
    policy: &KagemushaRecursiveSpendReleasePolicyV1,
    policy_sha256: [u8; 32],
    remaining_catalog_bytes: u64,
    remaining_decoded_bytes: u64,
    sealed_qualification: Option<&KagemushaCatalogSealedReleaseQualificationV1>,
) -> Result<(KagemushaCachedReleaseV4, u64, u64), String> {
    let manifest_bytes = read_bounded_directory_file(
        directory,
        MANIFEST_FILE_NAME_V4,
        MAX_MANIFEST_BYTES,
        "manifest",
    )?;
    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
    if manifest_sha256 != expected_manifest_sha256 {
        return Err("Kagemusha V4 manifest digest does not match its directory".to_owned());
    }
    let manifest = decode_canonical_manifest(&manifest_bytes)?;
    validate_catalog_artifact_encoding_sizes_v4(&manifest)?;
    let memory_estimate = estimate_catalog_release_memory_v4(&manifest)?;
    if memory_estimate.peak_load_bytes > remaining_decoded_bytes {
        return Err(format!(
            "Kagemusha V4 decoded catalog memory estimate {} exceeds the remaining budget {remaining_decoded_bytes}",
            memory_estimate.peak_load_bytes
        ));
    }
    let inventory_bytes = verify_exact_release_inventory_v4(directory, &manifest)?;
    if inventory_bytes > remaining_catalog_bytes {
        return Err(format!(
            "Kagemusha V4 artifact catalog exceeds the aggregate byte limit of {MAX_CATALOG_AGGREGATE_BYTES_V4}"
        ));
    }
    let manifest_json = read_bounded_directory_file(
        directory,
        MANIFEST_JSON_FILE_NAME_V4,
        MAX_MANIFEST_BYTES,
        "manifest JSON sidecar",
    )?;
    let mut expected_manifest_json =
        norito::json::to_string_pretty(&manifest).map_err(|error| {
            format!("failed to render canonical Kagemusha V4 manifest JSON: {error}")
        })?;
    expected_manifest_json.push('\n');
    if manifest_json != expected_manifest_json.as_bytes() {
        return Err("Kagemusha V4 manifest JSON sidecar is not canonical or is stale".to_owned());
    }
    let manifest_sha256_sidecar = read_bounded_directory_file(
        directory,
        MANIFEST_SHA256_FILE_NAME_V4,
        65,
        "manifest SHA-256 sidecar",
    )?;
    let expected_manifest_sha256_sidecar = format!("{}\n", hex::encode(manifest_sha256));
    if manifest_sha256_sidecar != expected_manifest_sha256_sidecar.as_bytes() {
        return Err("Kagemusha V4 manifest SHA-256 sidecar is stale".to_owned());
    }
    let attestation_bytes = read_bounded_directory_file(
        directory,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
        MAX_ATTESTATION_BYTES,
        "release attestation",
    )?;
    let release_attestation = decode_canonical_attestation(&attestation_bytes)?;
    let physical_device_benchmark_summary = read_bounded_directory_file(
        directory,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        "physical-device benchmark summary",
    )?;
    let cryptographic_review_summary = read_bounded_directory_file(
        directory,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        "cryptographic-review summary",
    )?;
    let promotion_bytes = read_bounded_directory_file(
        directory,
        PROMOTION_RECORD_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4,
        "promotion record",
    )?;
    let promotion_record = decode_canonical_promotion(&promotion_bytes)?;
    let authenticated = KagemushaAuthenticatedReleaseV4::verify(
        &manifest,
        policy,
        &release_attestation,
        &physical_device_benchmark_summary,
        &cryptographic_review_summary,
    )
    .map_err(|error| format!("Kagemusha V4 release authentication failed: {error}"))?;
    if authenticated.manifest_sha256() != expected_manifest_sha256
        || authenticated.release_policy_sha256() != policy_sha256
    {
        return Err(
            "Kagemusha V4 release identity or configured-policy digest mismatch".to_owned(),
        );
    }
    promotion_record
        .validate_against_authenticated_release(&authenticated)
        .map_err(|error| format!("Kagemusha V4 promotion record mismatch: {error}"))?;
    if let Some(sealed) = sealed_qualification {
        validate_sealed_release_qualification_v1(sealed, &authenticated, &promotion_bytes)?;
    }

    if manifest
        .profiles
        .iter()
        .map(|profile| profile.artifacts.len())
        .sum::<usize>()
        != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
    {
        return Err("Kagemusha V4 manifest does not contain exactly eight artifacts".to_owned());
    }
    let roster = &manifest.topup_finality_roster_artifact;
    verify_file_descriptor(
        directory,
        &roster.file_name,
        roster.size_bytes,
        roster.sha256,
        iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        "top-up finality roster",
    )?;

    // Retain every exact descriptor-relative inode. The qualified wrapper is
    // the sole production owner and the source-backed facade loads only one
    // parity's heavy verifier material at a time.
    let (pinned_source, qualified_source) = open_qualified_kagemusha_catalog_source_v4(
        directory,
        authenticated.clone(),
        sealed_qualification,
    )?;
    let verifier = Arc::new(
        KagemushaPastaCycleOpaqueVerifierV4::from_qualified_artifact_source(Arc::clone(
            &qualified_source,
        ))?,
    );
    let resolved = ResolvedKagemushaTerminalVerifierV4 {
        qualified_source,
        verifier,
        pinned_source,
    };
    resolved.pinned_source.validate_snapshot()?;
    if verify_exact_release_inventory_v4(directory, &manifest)? != inventory_bytes {
        return Err("Kagemusha V4 release inventory byte count changed while loading".to_owned());
    }
    Ok((
        KagemushaCachedReleaseV4 {
            release_record: iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4 {
                manifest,
                release_attestation,
                physical_device_benchmark_summary,
                cryptographic_review_summary,
                promotion_record,
            },
            resolved,
        },
        inventory_bytes,
        memory_estimate.persistent_bytes,
    ))
}

impl ResolvedKagemushaTerminalVerifierV4 {
    pub(crate) fn release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        self.qualified_source.authenticated_release()
    }

    fn parity_metadata(
        &self,
        parity: KagemushaPastaCycleParityV1,
    ) -> &KagemushaQualifiedParityMetadataV4 {
        match parity {
            KagemushaPastaCycleParityV1::StepEq => self.qualified_source.step_eq(),
            KagemushaPastaCycleParityV1::StepEp => self.qualified_source.step_ep(),
        }
    }

    fn authenticated_verifying_key(
        &self,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Vec<u8>, String> {
        let metadata = self.parity_metadata(parity);
        if metadata.parity() != parity
            || metadata.processed_verifying_key_len() == 0
            || metadata.processed_verifying_key_len()
                > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        {
            return Err("qualified Kagemusha V4 verifier-key metadata is invalid".to_owned());
        }
        self.qualified_source
            .with_authenticated_processed_verifying_key(parity, |reader, expected_len| {
                if expected_len != metadata.processed_verifying_key_len() {
                    return Err(
                        "qualified Kagemusha V4 verifier-key length changed during projection"
                            .to_owned(),
                    );
                }
                let capacity = usize::try_from(expected_len).map_err(|_| {
                    "Kagemusha V4 verifier-key length does not fit usize".to_owned()
                })?;
                let mut bytes = Vec::with_capacity(capacity);
                reader
                    .take(expected_len.saturating_add(1))
                    .read_to_end(&mut bytes)
                    .map_err(|error| {
                        format!("failed to read qualified Kagemusha V4 verifier key: {error}")
                    })?;
                if bytes.len() != capacity
                    || <[u8; 32]>::from(Sha256::digest(&bytes))
                        != metadata.processed_verifying_key_sha256()
                {
                    return Err(
                        "qualified Kagemusha V4 verifier-key payload identity changed".to_owned(),
                    );
                }
                let key = VerifyingKeyBox::new(
                    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
                    bytes,
                );
                if crate::zk::hash_vk(&key) != metadata.verifying_key_commitment() {
                    return Err("qualified Kagemusha V4 verifier-key commitment changed".to_owned());
                }
                Ok(key.bytes)
            })
    }

    pub(crate) fn artifact_set(&self) -> KagemushaAuthenticatedArtifactSetV4 {
        let release = self.release();
        let manifest = release.manifest();
        KagemushaAuthenticatedArtifactSetV4 {
            generation: manifest.generation.clone(),
            manifest_sha256: release.manifest_sha256(),
            release_policy_sha256: release.release_policy_sha256(),
            release_attestation_sha256: release.release_attestation_sha256(),
            activation_height: manifest.activation_height,
            withdrawal_height: manifest.withdrawal_height,
            max_proof_bytes: manifest.max_proof_bytes,
            asset_scale: manifest.asset_scale,
        }
    }
}

/// Deterministic V4-only state key for an authenticated release record.
pub(crate) fn release_state_key(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<StatePath, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    format!(
        "{TERMINAL_RELEASE_STATE_KEY_PREFIX_V4}{}",
        hex::encode(binding.manifest_sha256)
    )
    .parse()
    .map_err(|_| "Kagemusha V4 terminal release state key is invalid".to_owned())
}

/// Exact owner-manifest identifier required on V4 verifier records.
pub(crate) fn verifier_owner_manifest_id(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<String, String> {
    binding
        .validate()
        .map_err(|error| format!("invalid Kagemusha V4 artifact binding: {error}"))?;
    Ok(format!(
        "{VERIFIER_OWNER_MANIFEST_PREFIX_V4}{}",
        hex::encode(binding.manifest_sha256)
    ))
}

/// Derive the release- and layout-bound public-input identity stored in a V4
/// [`VerifyingKeyRecord`].
pub(crate) fn verifier_public_inputs_schema_hash(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String> {
    manifest.validate().map_err(|error| error.to_string())?;
    let manifest_bytes = norito::to_bytes(manifest)
        .map_err(|error| format!("failed to encode Kagemusha V4 manifest: {error}"))?;
    let profile = profile(manifest, parity)?;
    let identity = KagemushaTerminalVerifierIdentityV4 {
        schema: VERIFIER_IDENTITY_SCHEMA_V4.to_owned(),
        version: VERIFIER_IDENTITY_VERSION_V4,
        manifest_sha256: Sha256::digest(manifest_bytes).into(),
        parity,
        circuit_id: profile.circuit_id.clone(),
        circuit_params_sha256: profile
            .circuit_params_sha256()
            .map_err(|error| error.to_string())?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        public_input_limbs: profile.circuit_params.public_input_limbs,
    };
    let bytes = norito::to_bytes(&identity)
        .map_err(|error| format!("failed to encode Kagemusha V4 verifier identity: {error}"))?;
    Ok(Hash::new(bytes).into())
}

fn decode_trusted_policy(bytes: &[u8]) -> Result<KagemushaRecursiveSpendReleasePolicyV1, String> {
    if bytes.is_empty() || bytes.len() > MAX_POLICY_BYTES || bytes.iter().all(|byte| *byte == 0) {
        return Err("Kagemusha V4 trusted release policy is empty or exceeds its bound".to_owned());
    }
    let policy: KagemushaRecursiveSpendReleasePolicyV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| "Kagemusha V4 trusted release policy is malformed".to_owned())?;
    if norito::to_bytes(&policy)
        .map_err(|error| format!("failed to encode Kagemusha V4 trusted policy: {error}"))?
        != bytes
    {
        return Err("Kagemusha V4 trusted release policy is not canonical".to_owned());
    }
    policy
        .validate()
        .map_err(|error| format!("Kagemusha V4 trusted release policy is invalid: {error}"))?;
    Ok(policy)
}

fn profile(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<&iroha_data_model::offline::KagemushaPastaCycleProofProfileV4, String> {
    manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .ok_or_else(|| "Kagemusha V4 terminal verifier parity is absent".to_owned())
}

fn ensure_activation_record(
    record: &VerifyingKeyRecord,
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
    release: &KagemushaAuthenticatedReleaseV4,
    parity: KagemushaPastaCycleParityV1,
    qualified: &KagemushaQualifiedParityMetadataV4,
    block_height: u64,
) -> Result<(), String> {
    let manifest = release.manifest();
    let profile = profile(manifest, parity)?;
    let (role, expected_curve) = match parity {
        KagemushaPastaCycleParityV1::StepEq => ("Eq", STEP_EQ_VERIFIER_CURVE_V4),
        KagemushaPastaCycleParityV1::StepEp => ("Ep", STEP_EP_VERIFIER_CURVE_V4),
    };
    let expected_owner = verifier_owner_manifest_id(binding)?;
    let expected_schema_hash = verifier_public_inputs_schema_hash(manifest, parity)?;
    if record.version == 0
        || record.circuit_id != profile.circuit_id
        || record.owner_manifest_id.as_deref() != Some(expected_owner.as_str())
        || record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE
        || record.backend != BackendTag::Halo2IpaPasta
        || record.curve != expected_curve
        || record.public_inputs_schema_hash != expected_schema_hash
        || qualified.parity() != parity
        || qualified.circuit_params() != &profile.circuit_params
        || record.commitment != qualified.verifying_key_commitment()
        || u64::from(record.vk_len) != qualified.processed_verifying_key_len()
        || record.max_proof_bytes != manifest.max_proof_bytes
        || record.activation_height != Some(manifest.activation_height)
        // Release withdrawal ends issuance, not terminal verification. Keeping
        // the verifier record active prevents already-issued escrow from being
        // stranded after the issuance window closes.
        || record.withdraw_height.is_some()
        || !record.is_active_at(block_height)
    {
        return Err(format!(
            "Kagemusha V4 {role} verifier activation metadata or release identity mismatch"
        ));
    }
    let state_vk = record
        .key
        .as_ref()
        .ok_or_else(|| format!("Kagemusha V4 {role} verifier key is not available inline"))?;
    if state_vk.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
        || state_vk.bytes.is_empty()
        || u64::try_from(state_vk.bytes.len()).ok() != Some(qualified.processed_verifying_key_len())
        || <[u8; 32]>::from(Sha256::digest(&state_vk.bytes))
            != qualified.processed_verifying_key_sha256()
        || crate::zk::hash_vk(state_vk) != qualified.verifying_key_commitment()
    {
        return Err(format!(
            "Kagemusha V4 {role} state verifier key does not equal the authenticated release payload"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "kagemusha_terminal_registry_v4/candidate_profile.rs"]
mod test_support;

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        asset::AssetDefinitionId,
        domain::DomainId,
        offline::{
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
            KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
            KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1, KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2,
            KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4,
            KagemushaRecursiveSpendArtifactManifestV4,
            KagemushaRecursiveSpendCryptographicReviewApprovalV4,
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4,
            KagemushaRecursiveSpendCryptographicReviewPayloadV4,
            KagemushaRecursiveSpendPromotedReleaseV4, KagemushaRecursiveSpendReleaseApprovalRoleV1,
            KagemushaRecursiveSpendReleaseApprovalV4, KagemushaRecursiveSpendReleaseAttestationV4,
            KagemushaRecursiveSpendReleaseRolePolicyV1, KagemushaReleaseVerificationError,
            KagemushaReviewedSourceClosureV1, KagemushaTopUpFinalityRosterArtifactReferenceV4,
        },
    };

    use super::{test_support::candidate_binding_profile, *};

    #[cfg(target_os = "macos")]
    struct MacosAclGuard {
        path: PathBuf,
    }

    #[cfg(target_os = "macos")]
    impl Drop for MacosAclGuard {
        fn drop(&mut self) {
            let _ = std::process::Command::new("/bin/chmod")
                .arg("-N")
                .arg(&self.path)
                .status();
        }
    }

    #[cfg(target_os = "macos")]
    fn add_macos_acl(path: &Path, entry: &str) -> MacosAclGuard {
        let output = std::process::Command::new("/bin/chmod")
            .arg("+a")
            .arg(entry)
            .arg(path)
            .output()
            .expect("run macOS chmod");
        assert!(
            output.status.success(),
            "chmod +a failed for {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        MacosAclGuard {
            path: path.to_path_buf(),
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn catalog_source_mutex_recovers_after_caught_panic() {
        let mutex = Mutex::new(());
        let panicked = std::panic::catch_unwind(|| {
            let _guard = lock_kagemusha_catalog_source_mutex_v4(&mutex);
            panic!("catalog source mutex poison fixture");
        });
        assert!(panicked.is_err(), "fixture must poison the mutex once");

        let _guard = lock_kagemusha_catalog_source_mutex_v4(&mutex);
        assert!(!mutex.is_poisoned());
    }

    fn candidate_binding_reviewed_source_closure(
        source_commit: &str,
        source_tree_sha256: [u8; 32],
    ) -> (KagemushaReviewedSourceClosureV1, [u8; 32]) {
        let tracked_binary_diff_sha256 = Sha256::digest([0x91; 32]).into();
        let untracked_path_mode_blob_oid_manifest_sha256 = Sha256::digest([]).into();
        let mut combined = Sha256::new();
        combined.update(b"iroha-source-diff-v1\0");
        combined.update(b"tracked-binary-diff-sha256\0");
        combined.update(tracked_binary_diff_sha256);
        combined.update(b"untracked-path-blob-manifest-sha256\0");
        combined.update(untracked_path_mode_blob_oid_manifest_sha256);
        let closure = KagemushaReviewedSourceClosureV1 {
            schema: KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V1.to_owned(),
            base_commit: source_commit.to_owned(),
            source_commit: source_commit.to_owned(),
            source_repo_dirty: true,
            source_tree_sha256,
            tracked_binary_diff_sha256,
            untracked_file_count: 0,
            untracked_path_mode_blob_oid_manifest: Vec::new(),
            untracked_path_mode_blob_oid_manifest_sha256,
            ignored_cargo_lock_size_bytes: 1,
            ignored_cargo_lock_sha256: Sha256::digest([0x92]).into(),
            combined_source_fingerprint_sha256: combined.finalize().into(),
        };
        let descriptor_sha256 = closure
            .canonical_descriptor_sha256()
            .expect("candidate-binding reviewed source closure");
        (closure, descriptor_sha256)
    }

    fn authenticated_candidate_binding_release() -> (
        KagemushaAuthenticatedReleaseV4,
        KagemushaRecursiveSpendPromotedReleaseV4,
    ) {
        let benchmark = b"signed candidate-binding device benchmark";
        let source_commit = "0123456789abcdef0123456789abcdef01234567";
        let source_tree_sha256 = [0x61; 32];
        let (reviewed_source_closure, reviewed_source_closure_descriptor_sha256) =
            candidate_binding_reviewed_source_closure(source_commit, source_tree_sha256);
        let mut manifest = KagemushaRecursiveSpendArtifactManifestV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4,
            bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            generation: "candidate-binding-release".to_owned(),
            source_commit: source_commit.to_owned(),
            source_tree_sha256,
            source_repo_dirty: true,
            reviewed_source_closure,
            reviewed_source_closure_descriptor_sha256,
            chain_id: ChainId::from("candidate-binding-chain"),
            asset: AssetDefinitionId::new(
                DomainId::try_new("candidate", "binding").expect("candidate-binding domain"),
                "asset".parse().expect("candidate-binding asset name"),
            ),
            asset_scale: 2,
            activation_height: 1,
            withdrawal_height: 100,
            max_proof_bytes: 9_000,
            profiles: vec![
                candidate_binding_profile(KagemushaPastaCycleParityV1::StepEq, 0x10),
                candidate_binding_profile(KagemushaPastaCycleParityV1::StepEp, 0x20),
            ],
            topup_finality_roster_artifact: KagemushaTopUpFinalityRosterArtifactReferenceV4 {
                file_name: KAGEMUSHA_TOPUP_FINALITY_ROSTER_FILE_NAME_V4.to_owned(),
                size_bytes: 128,
                sha256: [0x31; 32],
                artifact_generation: "candidate-binding-release".to_owned(),
                circuit_id: KAGEMUSHA_TOPUP_FINALITY_CIRCUIT_ID_V2.to_owned(),
                purpose: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_PURPOSE_V2.to_owned(),
                artifact_type: KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_TYPE_V2.to_owned(),
                required_bridge_abi_version:
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            },
            benchmark_evidence_sha256: Sha256::digest(benchmark).into(),
            cryptographic_review_sha256: [0x63; 32],
            release_attestation_sha256: [0x62; 32],
        };
        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let key_pairs = [
            KeyPair::from_seed(vec![0x71; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![0x72; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![0x73; 32], Algorithm::Ed25519),
        ];
        let candidate = manifest
            .immutable_candidate()
            .expect("candidate-binding immutable candidate");
        let review_payload = KagemushaRecursiveSpendCryptographicReviewPayloadV4::approved(
            &candidate,
            [0x81; 32],
            [
                [0x82; 32], [0x83; 32], [0x84; 32], [0x85; 32], [0x86; 32], [0x87; 32],
            ],
        )
        .expect("candidate-binding review payload");
        let review = norito::to_bytes(&KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4,
            approvals: vec![KagemushaRecursiveSpendCryptographicReviewApprovalV4 {
                public_key: key_pairs[1].public_key().clone(),
                signature: SignatureOf::try_new(key_pairs[1].private_key(), &review_payload)
                    .expect("candidate-binding review signature"),
            }],
            payload: review_payload,
        })
        .expect("candidate-binding canonical signed review");
        manifest.cryptographic_review_sha256 = Sha256::digest(&review).into();
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "candidate-binding-policy".to_owned(),
            roles: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseRolePolicyV1 {
                        role,
                        threshold: 1,
                        authorized_signers: vec![key_pair.public_key().clone()],
                    },
                )
                .collect(),
        };
        let subject = manifest
            .release_attestation_subject()
            .expect("candidate-binding release subject");
        let attestation = KagemushaRecursiveSpendReleaseAttestationV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            subject: subject.clone(),
            approvals: roles
                .iter()
                .zip(&key_pairs)
                .map(
                    |(&role, key_pair)| KagemushaRecursiveSpendReleaseApprovalV4 {
                        role,
                        public_key: key_pair.public_key().clone(),
                        signature: SignatureOf::try_new(
                            key_pair.private_key(),
                            &subject.approval_payload(role),
                        )
                        .expect("candidate-binding release signature"),
                    },
                )
                .collect(),
        };
        manifest.release_attestation_sha256 =
            Sha256::digest(norito::to_bytes(&attestation).expect("candidate-binding attestation"))
                .into();
        let authenticated = KagemushaAuthenticatedReleaseV4::verify(
            &manifest,
            &policy,
            &attestation,
            benchmark,
            &review,
        )
        .expect("authenticated candidate-binding release");
        let candidate_sha256 = manifest
            .immutable_candidate()
            .and_then(|candidate| candidate.sha256())
            .expect("canonical candidate binding");
        let promotion = KagemushaRecursiveSpendPromotedReleaseV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4,
            generation: manifest.generation.clone(),
            candidate_sha256,
            manifest_sha256: authenticated.manifest_sha256(),
            release_attestation_sha256: authenticated.release_attestation_sha256(),
            release_policy_sha256: authenticated.release_policy_sha256(),
            approved_signers: authenticated.approved_signers().to_vec(),
            artifact_inventory_verified: true,
            bridge_abi_version:
                iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            max_proof_bytes: manifest.max_proof_bytes,
        };
        (authenticated, promotion)
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn sealed_parity_fixture(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        parity: KagemushaPastaCycleParityV1,
        commitment_tag: u8,
    ) -> KagemushaCatalogSealedParityQualificationV1 {
        let profile = profile(manifest, parity).expect("fixture parity profile");
        let verifying_key = kagemusha_artifact_descriptor_v4(
            manifest,
            parity,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        )
        .expect("fixture verifier-key descriptor");
        KagemushaCatalogSealedParityQualificationV1 {
            parity,
            circuit_params: profile.circuit_params.clone(),
            compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
            // The production seal captures the full protocol identity, which
            // intentionally differs from the value-free structure digest.
            compiled_protocol_identity_sha256: [commitment_tag ^ 0x5a; 32],
            processed_verifying_key_len: verifying_key.payload_size_bytes,
            processed_verifying_key_sha256: verifying_key.payload_sha256,
            verifying_key_commitment: [commitment_tag; 32],
            proving_key_embedded_verifying_key_sha256: verifying_key.payload_sha256,
            proving_key_fixed_columns: 1,
            proving_key_permutation_columns: 1,
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn qualification_seal_fixture(
        policy_path: &Path,
        artifact_dir: &Path,
    ) -> KagemushaCatalogQualificationSealV1 {
        let (authenticated, promotion) = authenticated_candidate_binding_release();
        let manifest = authenticated.manifest();
        let executable =
            current_kagemusha_catalog_executable_path_v1().expect("fixture executable path");
        let mut paths = BTreeMap::new();
        let fixture_stat = |inode, mode| KagemushaCatalogSealedStatV1 {
            device: 1,
            inode,
            mode,
            owner_uid: 0,
            owner_gid: 0,
            links: 1,
            length: 1,
            modified_seconds: 1,
            modified_nanoseconds: 1,
            changed_seconds: 1,
            changed_nanoseconds: 1,
        };
        for (path, kind, stat) in [
            (
                policy_path,
                KagemushaCatalogSealedPathKindV1::File,
                fixture_stat(1, 0o100440),
            ),
            (
                artifact_dir,
                KagemushaCatalogSealedPathKindV1::Directory,
                fixture_stat(2, 0o040550),
            ),
            (
                executable.as_path(),
                KagemushaCatalogSealedPathKindV1::File,
                fixture_stat(3, 0o100550),
            ),
        ] {
            let canonical_path =
                canonical_catalog_path_string_v1(path, "qualification seal fixture path")
                    .expect("canonical fixture path");
            paths.insert(
                canonical_path.clone(),
                KagemushaCatalogSealedPathV1 {
                    canonical_path,
                    kind,
                    stat,
                },
            );
        }
        let artifacts = manifest
            .profiles
            .iter()
            .flat_map(|profile| {
                profile.artifacts.iter().cloned().map(move |artifact| {
                    KagemushaCatalogSealedArtifactDigestV1 {
                        parity: profile.parity,
                        artifact,
                    }
                })
            })
            .collect();
        KagemushaCatalogQualificationSealV1 {
            schema: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1,
            canonical_policy_path: canonical_catalog_path_string_v1(
                policy_path,
                "fixture release policy",
            )
            .expect("fixture policy path"),
            canonical_artifact_dir: canonical_catalog_path_string_v1(
                artifact_dir,
                "fixture artifact root",
            )
            .expect("fixture artifact path"),
            canonical_executable_path: canonical_catalog_path_string_v1(
                &executable,
                "fixture executable",
            )
            .expect("fixture executable path"),
            build_fingerprint_sha256: current_kagemusha_catalog_build_fingerprint_v1(),
            executable_sha256: [0xa1; 32],
            configured_policy_sha256: authenticated.release_policy_sha256(),
            paths: paths.into_values().collect(),
            releases: vec![KagemushaCatalogSealedReleaseQualificationV1 {
                manifest_sha256: authenticated.manifest_sha256(),
                release_attestation_sha256: authenticated.release_attestation_sha256(),
                source_commit: manifest.source_commit.clone(),
                source_tree_sha256: manifest.source_tree_sha256,
                reviewed_source_closure_descriptor_sha256: manifest
                    .reviewed_source_closure_descriptor_sha256,
                benchmark_evidence_sha256: manifest.benchmark_evidence_sha256,
                cryptographic_review_sha256: manifest.cryptographic_review_sha256,
                promotion_record_sha256: Sha256::digest(
                    norito::encode_canonical(&promotion)
                        .expect("canonical fixture promotion record"),
                )
                .into(),
                artifacts,
                step_eq: sealed_parity_fixture(manifest, KagemushaPastaCycleParityV1::StepEq, 0xb1),
                step_ep: sealed_parity_fixture(manifest, KagemushaPastaCycleParityV1::StepEp, 0xb2),
            }],
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_is_canonical_versioned_norito_and_rejects_tamper() {
        let policy = Path::new("/sealed-fixture/policy.norito");
        let artifacts = Path::new("/sealed-fixture/artifacts");
        let seal = qualification_seal_fixture(policy, artifacts);
        let sealed_eq = &seal.releases[0].step_eq;
        let qualified_eq = sealed_eq.to_qualified().expect("qualified Eq fixture");
        assert_eq!(
            KagemushaCatalogSealedParityQualificationV1::from_qualified(
                &qualified_eq,
                sealed_eq.compiled_protocol_structure_sha256,
            )
            .expect("separately bound structure and identity"),
            sealed_eq.clone()
        );
        assert!(
            KagemushaCatalogSealedParityQualificationV1::from_qualified(&qualified_eq, [0; 32],)
                .is_err()
        );
        assert!(
            KagemushaCatalogSealedParityQualificationV1::from_qualified(
                &qualified_eq,
                qualified_eq.compiled_protocol_identity_sha256(),
            )
            .is_err()
        );
        let bytes = seal.canonical_bytes().expect("canonical seal bytes");
        let decoded: KagemushaCatalogQualificationSealV1 =
            norito::decode_canonical(&bytes).expect("decode canonical seal");
        assert_eq!(decoded, seal);

        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            norito::decode_canonical::<KagemushaCatalogQualificationSealV1>(&trailing).is_err(),
            "trailing seal bytes must not decode canonically"
        );

        let mut wrong_schema = seal.clone();
        wrong_schema.schema.push_str(".tampered");
        assert!(wrong_schema.canonical_bytes().is_err());
        let mut wrong_version = seal;
        wrong_version.version = wrong_version.version.saturating_add(1);
        assert!(wrong_version.canonical_bytes().is_err());
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_rejects_stale_build_executable_and_source_facts() {
        let policy = Path::new("/sealed-fixture/policy.norito");
        let artifacts = Path::new("/sealed-fixture/artifacts");
        let seal = qualification_seal_fixture(policy, artifacts);
        seal.validate_for_configured_runtime(policy, artifacts)
            .expect("matching runtime binding");

        let mut stale_build = seal.clone();
        stale_build.build_fingerprint_sha256[0] ^= 1;
        assert!(
            stale_build
                .validate_for_configured_runtime(policy, artifacts)
                .is_err()
        );

        let mut stale_executable = seal.clone();
        stale_executable.canonical_executable_path = "/sealed-fixture/other-irohad".to_owned();
        assert!(
            stale_executable
                .validate_for_configured_runtime(policy, artifacts)
                .is_err()
        );

        let (authenticated, promotion) = authenticated_candidate_binding_release();
        let promotion_bytes =
            norito::encode_canonical(&promotion).expect("canonical promotion fixture");
        for parity in [
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleParityV1::StepEp,
        ] {
            let sealed_parity = match parity {
                KagemushaPastaCycleParityV1::StepEq => &seal.releases[0].step_eq,
                KagemushaPastaCycleParityV1::StepEp => &seal.releases[0].step_ep,
            };
            assert_ne!(
                sealed_parity.compiled_protocol_identity_sha256,
                profile(authenticated.manifest(), parity)
                    .expect("fixture parity profile")
                    .compiled_protocol_structure_sha256,
                "the sealed full protocol identity must not be mistaken for its value-free structure digest"
            );
            assert_eq!(
                sealed_parity.compiled_protocol_structure_sha256,
                profile(authenticated.manifest(), parity)
                    .expect("fixture parity profile")
                    .compiled_protocol_structure_sha256,
                "the value-free structure digest must be sealed separately"
            );
        }
        validate_sealed_release_qualification_v1(
            &seal.releases[0],
            &authenticated,
            &promotion_bytes,
        )
        .expect("matching sealed source facts");
        let mut stale_source = seal.releases[0].clone();
        stale_source.source_tree_sha256[0] ^= 1;
        assert!(
            validate_sealed_release_qualification_v1(
                &stale_source,
                &authenticated,
                &promotion_bytes,
            )
            .is_err()
        );
        let mut tampered_artifact = seal.releases[0].clone();
        tampered_artifact.artifacts[0].artifact.payload_sha256[0] ^= 1;
        assert!(
            validate_sealed_release_qualification_v1(
                &tampered_artifact,
                &authenticated,
                &promotion_bytes,
            )
            .is_err()
        );
        let mut tampered_structure = seal.releases[0].clone();
        tampered_structure
            .step_eq
            .compiled_protocol_structure_sha256[0] ^= 1;
        assert!(
            validate_sealed_release_qualification_v1(
                &tampered_structure,
                &authenticated,
                &promotion_bytes,
            )
            .is_err()
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_missing_or_malformed_file_fails_closed() {
        let temporary = tempfile::tempdir().expect("ACL-free temporary seal root");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let policy = temporary.path().join("policy.norito");
        let artifacts = temporary.path().join("artifacts");
        let missing = temporary.path().join("missing-seal.norito");
        let missing_error =
            KagemushaReleaseCatalogV4::load_with_qualification_seal_for_trusted_uid(
                &policy,
                &artifacts,
                DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
                &missing,
                trusted_uid,
            )
            .err()
            .expect("missing qualification seal must fail closed");
        assert!(
            missing_error.contains("qualification seal")
                || missing_error.contains("failed to inspect")
        );

        let malformed = temporary.path().join("malformed-seal.norito");
        std::fs::write(&malformed, b"not canonical Norito")
            .expect("write malformed qualification seal");
        let malformed =
            std::fs::canonicalize(malformed).expect("canonical malformed qualification seal");
        let malformed_error =
            KagemushaReleaseCatalogV4::load_with_qualification_seal_for_trusted_uid(
                &policy,
                &artifacts,
                DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4,
                &malformed,
                trusted_uid,
            )
            .err()
            .expect("malformed qualification seal must fail closed");
        assert!(malformed_error.contains("decode") || malformed_error.contains("seal"));
    }

    #[cfg(all(
        unix,
        not(target_os = "macos"),
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_executable_stat_tamper_fails_without_rehashing() {
        let executable =
            current_kagemusha_catalog_executable_path_v1().expect("current test executable");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let mut captured = BTreeMap::new();
        let digest = capture_trusted_catalog_file_v1(
            &executable,
            "current test executable",
            trusted_uid,
            &mut captured,
            true,
        )
        .expect("capture test executable")
        .expect("test executable digest");
        assert_ne!(digest, [0; 32]);
        let paths = captured.into_values().collect::<Vec<_>>();
        verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid)
            .expect("matching executable stat identity");
        let mut stale = paths;
        let executable_entry = stale
            .iter_mut()
            .find(|entry| entry.canonical_path == executable.to_string_lossy())
            .expect("sealed executable path");
        executable_entry.stat.changed_seconds =
            executable_entry.stat.changed_seconds.saturating_add(1);
        assert!(
            verify_kagemusha_catalog_sealed_paths_v1(&stale, trusted_uid).is_err(),
            "fast startup must reject stale executable stat without hashing the binary"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_trust_rejects_third_party_owner_and_writable_mode() {
        let trusted_uid = 501;
        let base = KagemushaCatalogSealedStatV1 {
            device: 1,
            inode: 2,
            mode: 0o100440,
            owner_uid: trusted_uid,
            owner_gid: 20,
            links: 1,
            length: 3,
            modified_seconds: 4,
            modified_nanoseconds: 5,
            changed_seconds: 6,
            changed_nanoseconds: 7,
        };
        ensure_root_trusted_stat_v1(base, trusted_uid, "fixture")
            .expect("test fixture owner is trusted");
        let mut third_party = base;
        third_party.owner_uid = trusted_uid + 1;
        assert!(ensure_root_trusted_stat_v1(third_party, trusted_uid, "fixture").is_err());
        let mut writable = base;
        writable.mode |= 0o020;
        assert!(ensure_root_trusted_stat_v1(writable, trusted_uid, "fixture").is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn qualification_seal_trust_rejects_extended_acl_write_grants() {
        use std::os::unix::fs::MetadataExt as _;

        let temporary = tempfile::tempdir().expect("ACL fixture root");
        let source_path = temporary.path().join("source.bin");
        std::fs::write(&source_path, b"trusted source").expect("write trusted source fixture");
        let canonical_source =
            std::fs::canonicalize(&source_path).expect("canonical trusted source fixture");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let source_error = {
            let _acl = add_macos_acl(&canonical_source, "everyone allow write");
            let metadata =
                std::fs::symlink_metadata(&canonical_source).expect("inspect ACL source fixture");
            assert_eq!(
                metadata.mode() & 0o022,
                0,
                "ACL grant must not rely on writable POSIX mode bits"
            );
            capture_trusted_catalog_file_v1(
                &canonical_source,
                "ACL source fixture",
                trusted_uid,
                &mut BTreeMap::new(),
                false,
            )
            .expect_err("an ACL-writable trusted source must fail closed")
        };
        assert!(source_error.contains("extended ACL"));

        let seal_path = temporary.path().join("seal.norito");
        std::fs::write(&seal_path, b"not decoded").expect("write seal ACL fixture");
        let canonical_seal = std::fs::canonicalize(&seal_path).expect("canonical seal ACL fixture");
        let seal_error = {
            let _acl = add_macos_acl(&canonical_seal, "everyone allow write");
            read_root_trusted_kagemusha_catalog_qualification_seal_v1(&canonical_seal, trusted_uid)
                .expect_err("an ACL-writable root-trusted seal must fail before decoding")
        };
        assert!(seal_error.contains("extended ACL"));
    }

    #[test]
    fn decoded_catalog_estimate_accounts_for_params_and_vk_expansion() {
        let (authenticated, _) = authenticated_candidate_binding_release();
        let estimate = estimate_catalog_release_memory_v4(authenticated.manifest())
            .expect("candidate-binding memory estimate");
        let rows = 1_u64 << KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4;
        let params = rows * PARSED_PARAMS_BYTES_PER_ROW_V4 * 2;
        let verifier_domains = rows * PARSED_VERIFYING_KEY_DOMAIN_BYTES_PER_ROW_V4 * 2;
        let retained_and_parsed_vk = 64 * (1 + PARSED_VERIFYING_KEY_EXPANSION_V4) * 2;
        let expected_persistent = checked_decoded_estimate_headroom_v4(
            CATALOG_RELEASE_METADATA_PERSISTENT_BYTES_V4
                + params
                + verifier_domains
                + retained_and_parsed_vk,
        )
        .expect("expected persistent estimate");

        assert_eq!(estimate.persistent_bytes, expected_persistent);
        assert!(estimate.peak_load_bytes > estimate.persistent_bytes);
        assert!(estimate.peak_load_bytes <= DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4);
    }

    #[test]
    fn decoded_catalog_headroom_rounds_up() {
        assert_eq!(
            checked_decoded_estimate_headroom_v4(1).expect("one-byte estimate"),
            2
        );
        assert_eq!(
            checked_decoded_estimate_headroom_v4(4).expect("aligned estimate"),
            5
        );
    }

    #[test]
    fn catalog_preflight_rejects_inexact_proving_key_before_halo_parsing() {
        let (authenticated, _) = authenticated_candidate_binding_release();
        let mut manifest = authenticated.manifest().clone();
        for profile in &mut manifest.profiles {
            let sizes =
                kagemusha_artifact_encoding_sizes_v4(&profile.circuit_params, profile.parity)
                    .expect("fixture artifact encoding sizes");
            for descriptor in &mut profile.artifacts {
                match descriptor.kind {
                    KagemushaPastaCycleArtifactKindV4::ParamsIpa => {
                        descriptor.payload_size_bytes = sizes.parameters_bytes;
                    }
                    KagemushaPastaCycleArtifactKindV4::VerifyingKey => {
                        descriptor.payload_size_bytes = sizes.verifying_key_bytes;
                    }
                    KagemushaPastaCycleArtifactKindV4::ProvingKey
                    | KagemushaPastaCycleArtifactKindV4::BootstrapWitness => {}
                }
            }
        }
        let error = validate_catalog_artifact_encoding_sizes_v4(&manifest)
            .expect_err("an inexact proving-key descriptor must fail before parsing");

        assert!(error.contains("proving key descriptor length 64"));
        assert!(error.contains("exact authenticated shape length"));
    }

    #[test]
    fn decoded_catalog_estimate_rejects_shift_overflow() {
        let (authenticated, _) = authenticated_candidate_binding_release();
        let mut manifest = authenticated.manifest().clone();
        manifest.profiles[0].ipa_k = u64::BITS;

        assert!(estimate_catalog_release_memory_v4(&manifest).is_err());
    }

    #[test]
    fn decoded_catalog_loader_rejects_zero_budget_before_filesystem_access() {
        let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
            Path::new("missing-policy"),
            Path::new("missing-artifacts"),
            0,
        )
        .err()
        .expect("zero decoded budget must fail first");

        assert!(error.contains("must be greater than zero"));
    }

    #[test]
    fn decoded_catalog_loader_rejects_budget_above_safety_ceiling() {
        let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
            Path::new("missing-policy"),
            Path::new("missing-artifacts"),
            DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4 + 1,
        )
        .err()
        .expect("an over-ceiling decoded budget must fail before filesystem access");

        assert!(error.contains("non-raiseable"));
        assert!(error.contains(&DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4.to_string()));
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn decoded_catalog_loader_enforces_budget_before_artifact_reads() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        let (authenticated, _) = authenticated_candidate_binding_release();
        let mut manifest = authenticated.manifest().clone();
        for profile in &mut manifest.profiles {
            let sizes =
                kagemusha_artifact_encoding_sizes_v4(&profile.circuit_params, profile.parity)
                    .expect("compact artifact encoding sizes");
            for descriptor in &mut profile.artifacts {
                let payload_size = match descriptor.kind {
                    KagemushaPastaCycleArtifactKindV4::ParamsIpa => sizes.parameters_bytes,
                    KagemushaPastaCycleArtifactKindV4::ProvingKey => sizes.proving_key_bytes,
                    KagemushaPastaCycleArtifactKindV4::VerifyingKey => sizes.verifying_key_bytes,
                    KagemushaPastaCycleArtifactKindV4::BootstrapWitness => {
                        descriptor.payload_size_bytes
                    }
                };
                descriptor.payload_size_bytes = payload_size;
                descriptor.size_bytes = payload_size
                    .checked_add(4_096)
                    .expect("compact framed artifact size");
            }
        }
        let manifest_bytes = norito::to_bytes(&manifest).expect("canonical compact manifest");
        let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
        let release = artifacts.join(hex::encode(manifest_sha256));
        std::fs::create_dir_all(&release).expect("create compact release directory");
        std::fs::write(release.join(MANIFEST_FILE_NAME_V4), manifest_bytes)
            .expect("write compact manifest");

        let estimate =
            estimate_catalog_release_memory_v4(&manifest).expect("compact catalog memory estimate");
        assert!(estimate.peak_load_bytes <= DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4);
        let error = KagemushaReleaseCatalogV4::load_with_decoded_budget(
            &policy,
            &artifacts,
            estimate.peak_load_bytes - 1,
        )
        .err()
        .expect("a one-byte-short decoded budget must fail before inventory reads");
        assert!(
            error.contains("decoded catalog memory estimate"),
            "unexpected error: {error}"
        );

        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("the intentionally incomplete inventory must still fail closed");
        assert!(
            !error.contains("decoded catalog memory estimate") && error.contains("inventory"),
            "default loader rejected before the bounded inventory check: {error}"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn canonical_temporary_root(temporary: &tempfile::TempDir) -> PathBuf {
        std::fs::canonicalize(temporary.path()).expect("canonical temporary catalog root")
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn pinned_source_fixture() -> (
        tempfile::TempDir,
        PathBuf,
        KagemushaCatalogPinnedArtifactSourceV4,
    ) {
        let temporary = tempfile::tempdir().expect("temporary pinned-source root");
        let root = canonical_temporary_root(&temporary);
        let release_directory = root.join("release");
        std::fs::create_dir(&release_directory).expect("create pinned-source release directory");
        let (authenticated, _) = authenticated_candidate_binding_release();
        for (index, descriptor) in authenticated
            .manifest()
            .profiles
            .iter()
            .flat_map(|profile| profile.artifacts.iter())
            .enumerate()
        {
            let length = usize::try_from(descriptor.size_bytes)
                .expect("test artifact length must fit usize");
            let tag = u8::try_from(index + 1).expect("test artifact tag");
            std::fs::write(
                release_directory.join(&descriptor.file_name),
                vec![tag; length],
            )
            .expect("write pinned-source artifact");
        }
        let pinned_directory =
            CatalogDirectory::open_path(&release_directory, "pinned-source release")
                .expect("pin source release directory");
        let source =
            KagemushaCatalogPinnedArtifactSourceV4::open_pinned(&pinned_directory, authenticated)
                .expect("open exact-eight pinned source fixture");
        (temporary, release_directory, source)
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn sealed_fast_source_open_never_reads_any_artifact_payload() {
        KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(true));
        let fixture = std::panic::catch_unwind(pinned_source_fixture);
        KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(false));
        let (_temporary, release_directory, source) =
            fixture.expect("sealed open must not touch any artifact payload");
        source
            .validate_snapshot()
            .expect("metadata-only pinned source remains valid");
        assert_eq!(source.artifacts.len(), KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4);
        for artifact in &source.artifacts {
            assert!(
                artifact.authenticated_inspection.is_none(),
                "the sealed fast path must not retain a content inspection"
            );
            let bytes = std::fs::read(release_directory.join(&artifact.descriptor.file_name))
                .expect("read deliberately invalid proving-key fixture");
            assert_ne!(
                <[u8; 32]>::from(Sha256::digest(bytes)),
                artifact.descriptor.sha256,
                "fixture must prove open_pinned accepted an unread digest-mismatched artifact"
            );
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn sealed_catalog_source_construction_never_reads_any_artifact_payload() {
        let (temporary, release_directory, source) = pinned_source_fixture();
        let authenticated = source.release.clone();
        drop(source);
        let directory = CatalogDirectory::open_path(
            &release_directory,
            "sealed catalog source construction fixture",
        )
        .expect("pin sealed catalog release directory");
        let mut seal = qualification_seal_fixture(
            Path::new("/sealed-fixture/policy.norito"),
            Path::new("/sealed-fixture/artifacts"),
        );
        let sealed_release = seal.releases.remove(0);

        KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(true));
        let construction = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            open_qualified_kagemusha_catalog_source_v4(
                &directory,
                authenticated,
                Some(&sealed_release),
            )
        }));
        KAGEMUSHA_TEST_FORBID_ARTIFACT_PAYLOAD_READ_V1.with(|forbid| forbid.set(false));
        let (pinned, qualified) = construction
            .expect("sealed source construction must not panic")
            .expect("sealed source construction must not touch any artifact payload");

        assert!(
            qualified.authenticated_release() == pinned.authenticated_release(),
            "sealed qualified and pinned sources must retain the same release"
        );
        pinned
            .validate_snapshot()
            .expect("sealed source construction retains exact read-only handles");
        assert!(
            pinned
                .artifacts
                .iter()
                .all(|artifact| artifact.authenticated_inspection.is_none()),
            "sealed source construction must not retain a content inspection"
        );
        drop(temporary);
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn qualification_seal_capture_rejects_replaced_release_after_full_qualification() {
        let (temporary, qualified_release_directory, pinned_source) = pinned_source_fixture();
        let replacement_release_directory =
            canonical_temporary_root(&temporary).join("replacement-release");
        std::fs::create_dir(&replacement_release_directory)
            .expect("create replacement release directory");
        for artifact in &pinned_source.artifacts {
            std::fs::copy(
                qualified_release_directory.join(&artifact.descriptor.file_name),
                replacement_release_directory.join(&artifact.descriptor.file_name),
            )
            .expect("copy replacement artifact");
        }
        let replacement = CatalogDirectory::open_path(
            &replacement_release_directory,
            "replacement release after full qualification",
        )
        .expect("open replacement release directory");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let mut paths = BTreeMap::new();

        let error = capture_trusted_catalog_release_inventory_v1(
            &replacement,
            "replacement-release",
            pinned_source.manifest_sha256,
            &pinned_source,
            trusted_uid,
            &mut paths,
        )
        .expect_err("seal capture must reject inodes not used by full qualification");
        assert!(
            error.contains("different from the fully qualified pinned source"),
            "unexpected replacement error: {error}"
        );
        pinned_source
            .validate_snapshot()
            .expect("the originally qualified pinned source remains unchanged");
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn sealed_path_revalidation_rejects_path_stat_owner_mode_and_time_tamper() {
        let temporary = tempfile::tempdir().expect("ACL-free temporary sealed-path root");
        let file_path = temporary.path().join("sealed.bin");
        std::fs::write(&file_path, b"sealed identity").expect("write sealed-path fixture");
        let canonical_file = std::fs::canonicalize(&file_path).expect("canonical fixture file");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let mut captured = BTreeMap::new();
        capture_trusted_catalog_file_v1(
            &canonical_file,
            "sealed-path fixture",
            trusted_uid,
            &mut captured,
            false,
        )
        .expect("capture trusted fixture path");
        let paths = captured.into_values().collect::<Vec<_>>();
        verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid)
            .expect("unchanged fixture path");
        let file_index = paths
            .iter()
            .position(|entry| entry.canonical_path == canonical_file.to_string_lossy())
            .expect("sealed fixture file entry");

        let mut changed_path = paths.clone();
        changed_path[file_index]
            .canonical_path
            .push_str(".replacement");
        assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_path, trusted_uid).is_err());

        let mut changed_inode = paths.clone();
        changed_inode[file_index].stat.inode =
            changed_inode[file_index].stat.inode.saturating_add(1);
        assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_inode, trusted_uid).is_err());

        let mut changed_owner = paths.clone();
        changed_owner[file_index].stat.owner_uid =
            changed_owner[file_index].stat.owner_uid.saturating_add(1);
        assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_owner, trusted_uid).is_err());

        let mut changed_mode = paths.clone();
        changed_mode[file_index].stat.mode ^= 0o100;
        assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_mode, trusted_uid).is_err());

        let mut changed_time = paths;
        changed_time[file_index].stat.changed_nanoseconds =
            (changed_time[file_index].stat.changed_nanoseconds + 1) % 1_000_000_000;
        assert!(verify_kagemusha_catalog_sealed_paths_v1(&changed_time, trusted_uid).is_err());
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn sealed_path_revalidation_rejects_content_replacement_and_writable_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = tempfile::tempdir().expect("ACL-free temporary sealed-path root");
        let file_path = temporary.path().join("sealed.bin");
        std::fs::write(&file_path, b"original bytes").expect("write sealed-path fixture");
        let canonical_file = std::fs::canonicalize(&file_path).expect("canonical fixture file");
        let trusted_uid = rustix::process::geteuid().as_raw();
        let mut captured = BTreeMap::new();
        capture_trusted_catalog_file_v1(
            &canonical_file,
            "sealed-path fixture",
            trusted_uid,
            &mut captured,
            false,
        )
        .expect("capture trusted fixture path");
        let paths = captured.into_values().collect::<Vec<_>>();

        std::fs::write(&canonical_file, b"tampered bytes").expect("tamper fixture in place");
        assert!(
            verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
            "in-place byte mutation must change the sealed stat identity"
        );

        let mut permissions = std::fs::metadata(&canonical_file)
            .expect("inspect fixture permissions")
            .permissions();
        permissions.set_mode(0o664);
        std::fs::set_permissions(&canonical_file, permissions)
            .expect("make fixture group-writable");
        assert!(
            verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
            "a group-writable sealed file must fail root-trust validation"
        );

        std::fs::remove_file(&canonical_file).expect("remove original fixture inode");
        std::fs::write(&canonical_file, b"replacement obj").expect("replace fixture inode");
        assert!(
            verify_kagemusha_catalog_sealed_paths_v1(&paths, trusted_uid).is_err(),
            "path replacement must fail the inode seal"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    fn write_test_policy(root: &Path) -> std::path::PathBuf {
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1,
            KagemushaRecursiveSpendReleaseApprovalRoleV1,
            KagemushaRecursiveSpendReleaseRolePolicyV1,
        };

        let roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        let policy = KagemushaRecursiveSpendReleasePolicyV1 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1,
            policy_id: "catalog-loader-test-policy".to_owned(),
            roles: roles
                .into_iter()
                .enumerate()
                .map(|(index, role)| {
                    let seed = u8::try_from(index + 1).expect("small signer index");
                    KagemushaRecursiveSpendReleaseRolePolicyV1 {
                        role,
                        threshold: 1,
                        authorized_signers: vec![
                            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                                .public_key()
                                .clone(),
                        ],
                    }
                })
                .collect(),
        };
        policy.validate().expect("valid catalog test policy");
        let path = root.join("policy.norito");
        std::fs::write(
            &path,
            norito::to_bytes(&policy).expect("canonical catalog test policy"),
        )
        .expect("write catalog test policy");
        path
    }

    fn verifier_record_for_manifest(manifest_sha256: [u8; 32]) -> VerifyingKeyRecord {
        VerifyingKeyRecord::new_with_owner(
            1,
            "catalog-test-circuit",
            Some(format!(
                "{VERIFIER_OWNER_MANIFEST_PREFIX_V4}{}",
                hex::encode(manifest_sha256)
            )),
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            STEP_EQ_VERIFIER_CURVE_V4,
            [0x31; 32],
            [0x32; 32],
        )
    }

    #[test]
    fn empty_catalog_is_explicitly_unconfigured() {
        let catalog = KagemushaReleaseCatalogV4::empty();
        assert!(!catalog.is_configured());
        assert_eq!(catalog.configured_policy_sha256(), None);
        assert!(catalog.is_empty());
    }

    #[test]
    fn production_catalog_has_no_eager_artifact_materializer_path() {
        let module = include_str!("kagemusha_terminal_registry_v4.rs");
        let owner_body = |name: &str| {
            let start = module
                .find(name)
                .unwrap_or_else(|| panic!("missing production owner `{name}`"));
            let tail = &module[start..];
            let end = tail
                .find("\n}")
                .unwrap_or_else(|| panic!("unterminated production owner `{name}`"));
            &tail[..end]
        };
        let resolved = owner_body("pub(crate) struct ResolvedKagemushaTerminalVerifierV4");
        let cached = owner_body("pub(crate) struct KagemushaCachedReleaseV4");
        let catalog = owner_body("pub struct KagemushaReleaseCatalogV4");
        for owner in [resolved, cached, catalog] {
            assert!(!owner.contains(concat!("KagemushaPastaCycleVerifier", "ArtifactsV4")));
        }
        assert!(resolved.contains("qualified_source: Arc<KagemushaQualifiedArtifactSourceV4>"));
        assert!(resolved.contains("verifier: Arc<KagemushaPastaCycleOpaqueVerifierV4>"));
        assert!(module.contains("from_qualified_artifact_source"));
        for forbidden in [
            concat!("KagemushaPastaCycleVerifier", "ArtifactsV4"),
            concat!("KagemushaValidatedArtifact", "PayloadV4"),
            concat!("read_kagemusha_pasta_cycle_", "artifact_v4"),
            concat!("from_", "authenticated_artifacts"),
        ] {
            assert!(
                !module.contains(forbidden),
                "production catalog contains forbidden eager symbol `{forbidden}`"
            );
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_source_is_exact_read_only_and_rewinds_once_per_callback() {
        let (_temporary, _release_directory, source) = pinned_source_fixture();
        assert_eq!(source.artifacts.len(), KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4);
        source
            .validate_snapshot()
            .expect("all exact handles remain read-only");

        let mut callback_count = 0_u8;
        for _ in 0..2 {
            let mut callback = |reader: &mut dyn KagemushaArtifactReadSeekV4| {
                callback_count = callback_count.saturating_add(1);
                let mut bytes = Vec::new();
                reader
                    .read_to_end(&mut bytes)
                    .map_err(|error| error.to_string())?;
                if bytes.len() != 128 || bytes.iter().any(|byte| *byte != 1) {
                    return Err("pinned source did not rewind to the original Eq params".to_owned());
                }
                Ok(())
            };
            source
                .with_framed_artifact(
                    KagemushaPastaCycleParityV1::StepEq,
                    KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                    &mut callback,
                )
                .expect("lend one exact pinned file");
        }
        assert_eq!(callback_count, 2);
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_source_rejects_wrong_role_without_invoking_callback() {
        let (_temporary, _release_directory, mut source) = pinned_source_fixture();
        source.artifacts[0].parity = KagemushaPastaCycleParityV1::StepEp;
        let mut invoked = false;
        let mut callback = |_reader: &mut dyn KagemushaArtifactReadSeekV4| {
            invoked = true;
            Ok(())
        };
        let error = source
            .with_framed_artifact(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                &mut callback,
            )
            .expect_err("a role-substituted source must fail closed");
        assert!(error.contains("no exact artifact role"));
        assert!(!invoked);
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_source_rejects_in_place_tamper_and_trailing_growth() {
        use std::io::Write as _;

        let (_temporary, release_directory, source) = pinned_source_fixture();
        let file_name = source.artifacts[0].descriptor.file_name.clone();
        std::fs::write(release_directory.join(&file_name), vec![0xa5; 128])
            .expect("tamper pinned artifact in place");
        let mut invoked = false;
        let tamper_error = source
            .with_selected_file(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                |_file| {
                    invoked = true;
                    Ok(())
                },
            )
            .expect_err("in-place tamper must invalidate the pinned snapshot");
        assert!(tamper_error.contains("changed identity, bytes, or read-only"));
        assert!(!invoked);

        let (_temporary, release_directory, source) = pinned_source_fixture();
        let file_name = source.artifacts[0].descriptor.file_name.clone();
        std::fs::OpenOptions::new()
            .append(true)
            .open(release_directory.join(&file_name))
            .and_then(|mut file| file.write_all(b"trailing"))
            .expect("append trailing bytes to pinned artifact");
        let trailing_error = source
            .with_selected_file(
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
                |_file| Ok(()),
            )
            .expect_err("trailing growth must invalidate the pinned snapshot");
        assert!(trailing_error.contains("changed identity, bytes, or read-only"));
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_source_global_permit_serializes_all_roles() {
        use std::{
            sync::{
                Barrier,
                atomic::{AtomicUsize, Ordering},
            },
            time::Duration,
        };

        let (_temporary, _release_directory, source) = pinned_source_fixture();
        let source = Arc::new(source);
        let barrier = Arc::new(Barrier::new(3));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for kind in [
            KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            KagemushaPastaCycleArtifactKindV4::VerifyingKey,
        ] {
            let source = Arc::clone(&source);
            let barrier = Arc::clone(&barrier);
            let active = Arc::clone(&active);
            let maximum = Arc::clone(&maximum);
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                source
                    .with_selected_file(KagemushaPastaCycleParityV1::StepEq, kind, |_file| {
                        let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(25));
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(())
                    })
                    .expect("serialized pinned source access");
            }));
        }
        barrier.wait();
        for thread in threads {
            thread.join().expect("pinned-source worker");
        }
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn manifest_directory_names_are_canonical_lowercase_sha256() {
        let digest = [0xab; 32];
        let encoded = hex::encode(digest);
        assert_eq!(parse_manifest_directory_name(&encoded), Ok(digest));
        assert!(parse_manifest_directory_name(&encoded.to_uppercase()).is_err());
        assert!(parse_manifest_directory_name(&encoded[..63]).is_err());
    }

    #[test]
    fn release_state_key_is_manifest_content_addressed() {
        let binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "release-1".to_owned(),
            manifest_sha256: [0x5a; 32],
        };
        assert_eq!(
            release_state_key(&binding)
                .expect("valid V4 release key")
                .to_string(),
            format!(
                "{TERMINAL_RELEASE_STATE_KEY_PREFIX_V4}{}",
                hex::encode(binding.manifest_sha256)
            )
        );
    }

    #[test]
    fn runtime_promotion_validation_rejects_candidate_digest_substitution() {
        let (authenticated, promotion) = authenticated_candidate_binding_release();
        promotion
            .validate_against_authenticated_release(&authenticated)
            .expect("exact reconstructed candidate binding");

        let mut substituted = promotion;
        substituted.candidate_sha256[0] ^= 1;
        substituted
            .validate()
            .expect("substituted candidate digest remains structurally valid");
        assert_eq!(
            substituted.validate_against_authenticated_release(&authenticated),
            Err(KagemushaReleaseVerificationError::InvalidPromotionRecord)
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_malformed_policy_before_publication() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = root.join("policy.norito");
        let artifacts = root.join("artifacts");
        std::fs::write(&policy, b"not canonical norito").expect("write malformed policy");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("malformed configured policy must fail closed");
        assert!(error.contains("policy") || error.contains("malformed"));
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_empty_release_inventory() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("an empty configured catalog must fail closed");
        assert!(
            error.contains("contains no releases"),
            "unexpected error: {error}"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_release_count_above_retention_bound() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        for index in 0..=MAX_CATALOG_RELEASES_V4 {
            std::fs::create_dir(artifacts.join(format!("{index:064x}")))
                .expect("create bounded release directory");
        }
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("a catalog above the release-retention bound must fail closed");
        assert!(
            error.contains("at most") && error.contains(&MAX_CATALOG_RELEASES_V4.to_string()),
            "unexpected error: {error}"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_aggregate_byte_accounting_is_bounded() {
        const CORRECTED_EQ_EP_PROVING_KEYS_BYTES: u64 = 2 * 5_347_763_078;

        assert_eq!(MAX_CATALOG_AGGREGATE_BYTES_V4, 12 * 1024 * 1024 * 1024);
        assert!(
            CORRECTED_EQ_EP_PROVING_KEYS_BYTES < MAX_CATALOG_AGGREGATE_BYTES_V4,
            "the authenticated Eq/Ep proving keys must fit below the internal catalog ceiling"
        );
        assert_eq!(
            add_catalog_release_bytes(0, CORRECTED_EQ_EP_PROVING_KEYS_BYTES),
            Ok(CORRECTED_EQ_EP_PROVING_KEYS_BYTES)
        );
        assert_eq!(
            add_catalog_release_bytes(MAX_CATALOG_AGGREGATE_BYTES_V4 - 1, 1),
            Ok(MAX_CATALOG_AGGREGATE_BYTES_V4)
        );
        let error = add_catalog_release_bytes(MAX_CATALOG_AGGREGATE_BYTES_V4, 1)
            .expect_err("an aggregate catalog above the byte bound must fail closed");
        assert!(
            error.contains("aggregate byte limit"),
            "unexpected error: {error}"
        );
        let error = add_catalog_release_bytes(u64::MAX, 1)
            .expect_err("aggregate byte accounting overflow must fail closed");
        assert!(error.contains("overflowed"), "unexpected error: {error}");
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_manifest_directory_digest_substitution() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        let release = artifacts.join(hex::encode([0x55; 32]));
        std::fs::create_dir_all(&release).expect("create substituted release directory");
        std::fs::write(
            release.join(MANIFEST_FILE_NAME_V4),
            b"different manifest bytes",
        )
        .expect("write substituted manifest");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("a manifest under a different digest must fail closed");
        assert!(
            error.contains("manifest digest does not match its directory"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn activation_records_require_one_exact_v4_manifest_owner() {
        let manifest_sha256 = [0x61; 32];
        let step_eq = verifier_record_for_manifest(manifest_sha256);
        let step_ep = verifier_record_for_manifest(manifest_sha256);
        assert_eq!(
            activation_manifest_sha256(&step_eq, &step_ep),
            Ok(manifest_sha256)
        );

        let other = verifier_record_for_manifest([0x62; 32]);
        let error = activation_manifest_sha256(&step_eq, &other)
            .expect_err("cross-manifest Eq/Ep records must fail closed");
        assert!(error.contains("select different releases"));

        let mut retired = step_ep;
        retired.owner_manifest_id = Some(format!("kagemusha-v3-{}", hex::encode(manifest_sha256)));
        let error = activation_manifest_sha256(&step_eq, &retired)
            .expect_err("a retired owner namespace must fail closed");
        assert!(error.contains("owner namespace is invalid"));
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_symlinked_policy_and_artifact_roots() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        let policy_link = root.join("policy-link.norito");
        symlink(&policy, &policy_link).expect("create policy symlink");
        let error = read_bounded_regular_file(&policy_link, MAX_POLICY_BYTES, "release policy")
            .err()
            .expect("a symlinked policy leaf must fail before artifact scanning");
        assert!(
            error.contains("direct single-link regular file"),
            "unexpected error: {error}"
        );

        let artifact_link = root.join("artifact-link");
        symlink(&artifacts, &artifact_link).expect("create artifact-root symlink");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifact_link)
            .err()
            .expect("a symlinked artifact root must fail closed");
        assert!(error.contains("not a real directory"));

        for suffix in ["/", "/."] {
            let mut spelling = artifact_link.as_os_str().to_os_string();
            spelling.push(suffix);
            let error = CatalogDirectory::open_path(
                Path::new(&spelling),
                "non-canonical symlinked artifact root",
            )
            .err()
            .expect("a trailing component must not turn the final symlink into an intermediate");
            assert!(error.contains("canonical"), "unexpected error: {error}");
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_paths_reject_intermediate_symlinks() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let real_parent = root.join("real-parent");
        let artifacts = real_parent.join("artifacts");
        let policy = real_parent.join("policy.norito");
        std::fs::create_dir(&real_parent).expect("create real parent");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        std::fs::write(&policy, b"policy bytes").expect("write policy leaf");

        let intermediate = root.join("intermediate-link");
        symlink(&real_parent, &intermediate).expect("create intermediate symlink");

        let artifact_error = CatalogDirectory::open_path(
            &intermediate.join("artifacts"),
            "intermediate-symlink artifact root",
        )
        .err()
        .expect("an intermediate artifact-root symlink must fail closed");
        assert!(
            artifact_error.contains("not a real directory"),
            "unexpected error: {artifact_error}"
        );

        let policy_error = read_bounded_regular_file(
            &intermediate.join("policy.norito"),
            MAX_POLICY_BYTES,
            "intermediate-symlink release policy",
        )
        .err()
        .expect("an intermediate policy symlink must fail closed");
        assert!(
            policy_error.contains("not a real directory"),
            "unexpected error: {policy_error}"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_paths_must_be_absolute() {
        let error = CatalogDirectory::open_path(Path::new("relative-artifacts"), "artifact root")
            .err()
            .expect("a relative configured catalog path must fail closed");
        assert!(error.contains("absolute"), "unexpected error: {error}");
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn configured_catalog_rejects_symlinked_release_directory_and_manifest_leaf() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = write_test_policy(&root);
        let artifacts = root.join("artifacts");
        let external_release = root.join("external-release");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        std::fs::create_dir(&external_release).expect("create external release");
        let release_name = hex::encode([0x71; 32]);
        symlink(&external_release, artifacts.join(&release_name))
            .expect("create release-directory symlink");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("a symlinked release directory must fail closed");
        assert!(
            error.contains("not a real directory"),
            "unexpected error: {error}"
        );

        std::fs::remove_file(artifacts.join(&release_name))
            .expect("remove release-directory symlink");
        let release = artifacts.join(&release_name);
        std::fs::create_dir(&release).expect("create real release directory");
        let external_manifest = root.join("external-manifest.norito");
        std::fs::write(&external_manifest, b"substituted manifest")
            .expect("write external manifest");
        symlink(&external_manifest, release.join(MANIFEST_FILE_NAME_V4))
            .expect("create manifest symlink");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("a symlinked manifest leaf must fail before decoding");
        assert!(
            error.contains("direct single-link regular file"),
            "unexpected error: {error}"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_directory_reads_original_object_and_rejects_path_replacement() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let temporary_root = canonical_temporary_root(&temporary);
        let artifacts = temporary_root.join("artifacts");
        let displaced = temporary_root.join("artifacts-displaced");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        std::fs::write(artifacts.join("original.bin"), b"original")
            .expect("write original artifact");
        let pinned = CatalogDirectory::open_path(&artifacts, "test artifact root")
            .expect("pin original artifact root");

        std::fs::rename(&artifacts, &displaced).expect("displace original artifact root");
        std::fs::create_dir(&artifacts).expect("install replacement artifact root");
        std::fs::write(artifacts.join("original.bin"), b"replacement")
            .expect("write replacement artifact");

        let mut opened = pinned
            .open_file("original.bin", "test artifact")
            .expect("open through pinned original directory");
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut opened.file, &mut bytes)
            .expect("read pinned original artifact");
        opened.verify_unchanged().expect("original file is stable");
        assert_eq!(bytes, b"original");
        assert!(
            pinned.verify_path_identity().is_err(),
            "publication must reject a replaced configured path"
        );
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn retained_policy_handle_rejects_post_read_mutation() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let root = canonical_temporary_root(&temporary);
        let policy = root.join("policy.norito");
        std::fs::write(&policy, b"initial-policy").expect("write initial policy");
        let parent =
            CatalogDirectory::open_path(&root, "release policy parent").expect("pin policy parent");
        let mut opened = parent
            .open_file("policy.norito", "release policy")
            .expect("pin policy file");
        let bytes = read_bounded_opened_file(&mut opened, MAX_POLICY_BYTES, "release policy")
            .expect("read stable initial policy");
        assert_eq!(bytes, b"initial-policy");

        std::fs::write(&policy, b"changed-policy-with-a-different-length")
            .expect("mutate policy after read");
        let error = opened
            .verify_unchanged()
            .expect_err("the retained policy handle must detect post-read mutation");
        assert!(error.contains("changed"), "unexpected error: {error}");
    }

    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    #[test]
    fn pinned_release_reads_original_object_and_rejects_entry_replacement() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let temporary_root = canonical_temporary_root(&temporary);
        let root = temporary_root.join("artifacts");
        let release_name = hex::encode([0x72; 32]);
        let release = root.join(&release_name);
        let displaced = root.join("displaced-release");
        std::fs::create_dir_all(&release).expect("create release directory");
        std::fs::write(release.join("original.bin"), b"original").expect("write original artifact");
        let pinned_root =
            CatalogDirectory::open_path(&root, "test artifact root").expect("pin artifact root");
        let pinned_release = pinned_root
            .open_directory(&release_name, "test release")
            .expect("pin release directory");

        std::fs::rename(&release, &displaced).expect("displace original release");
        std::fs::create_dir(&release).expect("install replacement release");
        std::fs::write(release.join("original.bin"), b"replacement")
            .expect("write replacement artifact");

        let mut opened = pinned_release
            .open_file("original.bin", "test release artifact")
            .expect("open through pinned original release");
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut opened.file, &mut bytes)
            .expect("read pinned original release artifact");
        opened.verify_unchanged().expect("original file is stable");
        assert_eq!(bytes, b"original");
        assert!(
            pinned_root
                .verify_directory_entry(&release_name, &pinned_release)
                .is_err(),
            "publication must reject a replaced release entry"
        );
    }
}
