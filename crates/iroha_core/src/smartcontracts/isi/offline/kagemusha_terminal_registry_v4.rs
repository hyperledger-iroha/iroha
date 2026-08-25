//! Authenticated Kagemusha V4 verifier catalog and activation replay guards.
//!
//! V4 has its own state namespaces, release-record schema, KRV4 framing, and
//! verifier identity. Nothing in this module accepts or upgrades the V3
//! registry representation. Release policy comes from canonical configured
//! Norito; consensus state can select material, but cannot select its signers.
use super::{Error, StateTransaction, kagemusha_v2_marker, labeled_invariant};
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
use crate::zk::kagemusha_artifact_v4::kagemusha_artifact_descriptor_v4;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
use crate::zk::kagemusha_recursion_adapter::{
    KAGEMUSHA_PK_STREAM_AUTHENTICATION_BUFFER_BYTES_V5, kagemusha_artifact_encoding_sizes_v4,
};
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
use crate::zk::{
    kagemusha_artifact_source_v4::{
        KagemushaArtifactReadSeekV4, KagemushaAuthenticatedArtifactSourceV4,
        qualify_kagemusha_authenticated_artifact_source_v4,
    },
    kagemusha_artifact_v4::{
        KagemushaAuthenticatedArtifactInspectionV4, inspect_kagemusha_pasta_cycle_artifact_v4,
        read_kagemusha_pasta_cycle_artifact_v4,
    },
    kagemusha_recursion_adapter::{
        KagemushaQualificationMemoryContractV4, verify_candidate_recursive_step_two_receipt_v4,
    },
};
use crate::zk::{
    kagemusha_artifact_source_v4::{
        KagemushaQualifiedArtifactSourceV4, KagemushaQualifiedParityMetadataV4,
    },
    kagemusha_v2::KagemushaPastaCycleOpaqueVerifierV4,
};
use iroha_crypto::Hash;
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4, KagemushaRecursiveSpendCandidateV4,
    KagemushaRecursiveSpendQualificationReceiptV4,
};
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
    KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
    KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4, KagemushaPastaCycleArtifactKindV4,
    KagemushaRecursiveSpendReleaseAttestationV4, KagemushaRecursiveSpendReleasePolicyV1,
};
use iroha_data_model::{
    confidential::ConfidentialStatus,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4, KAGEMUSHA_VERIFIER_NAMESPACE,
        KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactV4,
        KagemushaPastaCycleParityV1, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendReleaseActivationV4,
        KagemushaStepCircuitParamsV4, kagemusha_recursive_spend_verifier_owner_manifest_id_v4,
        kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4,
    },
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
    state_path::StatePath,
    zk::BackendTag,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
use rustix::fs::{
    AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, fcntl_getfl, open, openat, statat,
};
use sha2::{Digest as _, Sha256};
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
use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{Seek as _, SeekFrom},
    os::unix::fs::MetadataExt as _,
    sync::Mutex,
};
pub(crate) const TERMINAL_RELEASE_STATE_KEY_PREFIX_V4: &str = "kagemusha_terminal_release_v4_";
const KAGEMUSHA_V4_PROMOTION_ID_DOMAIN: &str = "kagemusha-v4-promotion-id";
const VERIFIER_OWNER_MANIFEST_PREFIX_V4: &str = "kagemusha-v4-";
const STEP_EQ_VERIFIER_CURVE_V4: &str = "vesta";
const STEP_EP_VERIFIER_CURVE_V4: &str = "pallas";
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MAX_POLICY_BYTES: usize = 64 * 1024;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MAX_ATTESTATION_BYTES: usize = 1024 * 1024;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MANIFEST_FILE_NAME_V4: &str = "manifest.norito";
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MANIFEST_JSON_FILE_NAME_V4: &str = "manifest.json";
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const MANIFEST_SHA256_FILE_NAME_V4: &str = "manifest.norito.sha256";
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const PROMOTION_RECORD_FILE_NAME_V4: &str = "promotion-record-v4.norito";
const KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.len();
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_SCHEMA_V1: &str =
    "iroha.kagemusha.catalog_qualification_seal.v1";
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_VERSION_V1: u16 = 1;
/// Maximum canonical bytes accepted for one root-trusted catalog qualification seal.
pub const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_PATHS_V1: usize = 1024;
const KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_RELEASES_V1: usize = 16;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
///
/// 272 MiB is the next 16 MiB boundary above the conservative 279,192,800-byte
/// fixed-K17 two-profile qualification estimate.
pub const DEFAULT_KAGEMUSHA_CATALOG_MAX_DECODED_BYTES_V4: u64 =
    iroha_config::parameters::defaults::settlement::offline::KAGEMUSHA_MAX_DECODED_BYTES;
/// `ParamsIPA` retains two vectors of 64-byte Pasta affine points per domain row.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const PARSED_PARAMS_BYTES_PER_ROW_V4: u64 = 2 * 64;
/// Conservative expansion from compressed verifier-key bytes to parsed points.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const PARSED_VERIFYING_KEY_EXPANSION_V4: u64 = 2;
/// Conservative retained cost of Halo2's verifier-key evaluation domain.
///
/// The vendored domain owns forward and inverse FFT twiddle tables for the base and extended
/// domains. Charging 512 bytes per base-domain row covers those tables, their vector metadata, and
/// construction scratch without pretending that the tiny serialized VK is representative of its
/// decoded footprint.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const PARSED_VERIFYING_KEY_DOMAIN_BYTES_PER_ROW_V4: u64 = 512;
/// Small authenticated objects retained in several catalog/verifier owners.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const CATALOG_RELEASE_METADATA_PERSISTENT_BYTES_V4: u64 = (3 * MAX_MANIFEST_BYTES
    + MAX_ATTESTATION_BYTES
    + KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1
    + 2 * KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1
    + KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4)
    as u64;
/// Metadata parsing scratch that can overlap verifier parsing.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const CATALOG_RELEASE_METADATA_TRANSIENT_BYTES_V4: u64 = (3 * MAX_MANIFEST_BYTES
    + MAX_POLICY_BYTES
    + MAX_ATTESTATION_BYTES
    + KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1
    + KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4
    + KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_PROMOTION_BYTES_V4)
    as u64;
/// Extra allocator/metadata headroom applied to decoded catalog estimates.
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const DECODED_ESTIMATE_HEADROOM_NUMERATOR_V4: u64 = 5;
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
const DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4: u64 = 4;
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
    qualification_receipt_file_name: String,
    qualification_receipt_sha256: [u8; 32],
    qualified_candidate_sha256: [u8; 32],
    internal_validation_receipt_sha256: [u8; 32],
    source_commit: String,
    source_tree_sha256: [u8; 32],
    reviewed_source_closure_descriptor_sha256: [u8; 32],
    authenticated_source_seal_projection_sha256: [u8; 32],
    reviewed_cargo_binary_sha256: [u8; 32],
    reviewed_rustc_binary_sha256: [u8; 32],
    generator_binary_sha256: [u8; 32],
    sealed_candidate_build_report_sha256: [u8; 32],
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
/// The canonical Norito value contains an explicit schema and fixed V1 layout. It is not
/// self-authenticating: production loading accepts it only from a root-owned, single-link,
/// non-writable descriptor-relative path (also extended-ACL-free on macOS) and compares every
/// sealed source and executable identity before trusting its qualified Eq/Ep facts.
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

    /// Verify exact externally retained canonical bytes for this same-load seal.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` is empty, exceeds the fixed seal ceiling,
    /// or differs byte-for-byte from this authenticated seal's canonical form.
    pub fn verify_exact_canonical_bytes(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_CATALOG_QUALIFICATION_SEAL_MAX_BYTES_V1 {
            return Err(
                "Kagemusha catalog qualification seal bytes violate the fixed bound".to_owned(),
            );
        }
        let canonical = self.canonical_bytes()?;
        if canonical != bytes {
            return Err(
                "configured Kagemusha catalog qualification seal differs from the same-load seal"
                    .to_owned(),
            );
        }
        Ok(())
    }
}
impl KagemushaCatalogSealedParityQualificationV1 {
    #[cfg(any(
        test,
        all(
            unix,
            not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
        )
    ))]
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
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    qualification_receipt_sha256: [u8; 32],
    #[cfg(all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    ))]
    qualified_candidate_sha256: [u8; 32],
    resolved: ResolvedKagemushaTerminalVerifierV4,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
struct KagemushaCatalogReleaseConsensusIdentityV1 {
    manifest_sha256: [u8; 32],
    release_record_sha256: [u8; 32],
    qualification_receipt_sha256: [u8; 32],
    qualified_candidate_sha256: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
struct KagemushaCatalogConsensusIdentityV1 {
    version: u16,
    configured_policy_sha256: [u8; 32],
    releases: Vec<KagemushaCatalogReleaseConsensusIdentityV1>,
}
/// Immutable startup catalog keyed by canonical V4 manifest digest.
///
/// The catalog owns qualified pinned read-only artifact handles and one source-backed verifier
/// facade per release. Consensus execution performs map lookups and reads only those already-opened
/// inodes; it never reopens an artifact by path or caches two-parity Halo2 material.
#[derive(Clone, Default)]
pub struct KagemushaReleaseCatalogV4 {
    configured_policy_sha256: Option<[u8; 32]>,
    consensus_policy_digest: Option<[u8; 32]>,
    releases: BTreeMap<[u8; 32], Arc<KagemushaCachedReleaseV4>>,
}
include!("kagemusha_terminal_registry_v4_release_catalog_impl.rs");
include!("kagemusha_terminal_registry_v4_validator_qualification.rs");
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
            cached.qualification_receipt_sha256,
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
    qualification_receipt_sha256: [u8; 32],
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
    let mut captured_qualification_receipt = false;
    for file_name in release_directory.entry_names("release inventory")? {
        let mut opened = release_directory.open_file(
            &file_name,
            &format!("release file `{directory_name}/{file_name}`"),
        )?;
        opened.verify_trusted(trusted_uid)?;
        if file_name == KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4 {
            if captured_qualification_receipt
                || hash_catalog_opened_file_v1(&mut opened)? != qualification_receipt_sha256
            {
                return Err(
                    "Kagemusha V4 qualification-seal capture changed the qualification receipt"
                        .to_owned(),
                );
            }
            captured_qualification_receipt = true;
        }
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
    if captured_roles.len() != KAGEMUSHA_CATALOG_ARTIFACT_COUNT_V4
        || !captured_qualification_receipt
    {
        return Err(
            "Kagemusha V4 qualification-seal capture omitted an artifact role or qualification receipt"
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
        if authenticated.manifest_sha256() != *manifest_sha256
            || cached.qualification_receipt_sha256 != manifest.qualification_receipt_sha256
            || cached.qualified_candidate_sha256 != manifest.qualified_candidate_sha256
        {
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
            qualification_receipt_file_name:
                KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.to_owned(),
            qualification_receipt_sha256: cached.qualification_receipt_sha256,
            qualified_candidate_sha256: cached.qualified_candidate_sha256,
            internal_validation_receipt_sha256: manifest.internal_validation_receipt_sha256,
            source_commit: manifest.source_commit.clone(),
            source_tree_sha256: manifest.source_tree_sha256,
            reviewed_source_closure_descriptor_sha256: manifest
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: manifest
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: manifest.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: manifest.reviewed_rustc_binary_sha256,
            generator_binary_sha256: manifest.generator_binary_sha256,
            sealed_candidate_build_report_sha256: manifest.sealed_candidate_build_report_sha256,
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
                || release.qualification_receipt_file_name
                    != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4
                || release.qualification_receipt_sha256 == [0; 32]
                || release.qualified_candidate_sha256 == [0; 32]
                || release.internal_validation_receipt_sha256 == [0; 32]
                || release.source_commit.is_empty()
                || release.source_tree_sha256 == [0; 32]
                || release.reviewed_source_closure_descriptor_sha256 == [0; 32]
                || release.authenticated_source_seal_projection_sha256 == [0; 32]
                || release.reviewed_cargo_binary_sha256 == [0; 32]
                || release.reviewed_rustc_binary_sha256 == [0; 32]
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
/// Every handle is opened relative to the already pinned release directory and is never reopened by
/// path. A source-wide permit prevents Eq/Ep or role readers from overlapping; each file also owns
/// its cursor mutex so clones of the source cannot race a rewind. Full qualification retains one
/// complete-frame inspection per role. A sealed restart retains only the root-trusted inode
/// identity and reauthenticates every byte when a later parser actually consumes the role.
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1.to_owned(),
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4.to_owned(),
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
    if expected.len() != 18 {
        return Err(
            "Kagemusha V4 release inventory does not describe exactly eighteen unique files"
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
fn checked_decoded_estimate_headroom_v4(bytes: u64) -> Result<u64, String> {
    bytes
        .checked_mul(DECODED_ESTIMATE_HEADROOM_NUMERATOR_V4)
        .and_then(|value| {
            value.checked_add(DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4.saturating_sub(1))
        })
        .and_then(|value| value.checked_div(DECODED_ESTIMATE_HEADROOM_DENOMINATOR_V4))
        .ok_or_else(|| "Kagemusha V4 decoded catalog memory estimate overflowed".to_owned())
}
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
fn estimate_catalog_release_memory_v4(
    manifest: &iroha_data_model::offline::KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<KagemushaCatalogMemoryEstimateV4, String> {
    let pk_stream_scratch_bytes = u64::try_from(KAGEMUSHA_PK_STREAM_AUTHENTICATION_BUFFER_BYTES_V5)
        .map_err(|_| "Kagemusha V4 PK stream scratch does not fit u64".to_owned())?;
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
        .and_then(|value| value.checked_add(pk_stream_scratch_bytes))
        .ok_or_else(|| "Kagemusha V4 peak memory estimate overflowed".to_owned())?;
    Ok(KagemushaCatalogMemoryEstimateV4 {
        persistent_bytes: checked_decoded_estimate_headroom_v4(persistent_bytes)?,
        peak_load_bytes: checked_decoded_estimate_headroom_v4(peak_load_bytes)?,
    })
}
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
fn validate_sealed_release_qualification_v1(
    sealed: &KagemushaCatalogSealedReleaseQualificationV1,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    promotion_bytes: &[u8],
    qualification_receipt_sha256: [u8; 32],
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
        || sealed.qualification_receipt_file_name
            != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4
        || sealed.qualification_receipt_sha256 != qualification_receipt_sha256
        || sealed.qualification_receipt_sha256 != manifest.qualification_receipt_sha256
        || sealed.qualified_candidate_sha256 != manifest.qualified_candidate_sha256
        || sealed.internal_validation_receipt_sha256 != manifest.internal_validation_receipt_sha256
        || sealed.source_commit != manifest.source_commit
        || sealed.source_tree_sha256 != manifest.source_tree_sha256
        || sealed.reviewed_source_closure_descriptor_sha256
            != manifest.reviewed_source_closure_descriptor_sha256
        || sealed.authenticated_source_seal_projection_sha256
            != manifest.authenticated_source_seal_projection_sha256
        || sealed.reviewed_cargo_binary_sha256 != manifest.reviewed_cargo_binary_sha256
        || sealed.reviewed_rustc_binary_sha256 != manifest.reviewed_rustc_binary_sha256
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
#[cfg(all(
    unix,
    not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
))]
fn verify_catalog_qualification_receipt_v4(
    directory: &CatalogDirectory,
    authenticated: &KagemushaAuthenticatedReleaseV4,
    candidate: &KagemushaRecursiveSpendCandidateV4,
    receipt: &KagemushaRecursiveSpendQualificationReceiptV4,
    max_decoded_bytes: u64,
) -> Result<(), String> {
    let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
    let candidate_manifest_bytes =
        norito::encode_canonical(&candidate.manifest).map_err(|error| {
            format!("failed to encode Kagemusha V4 qualification candidate manifest: {error}")
        })?;
    let candidate_manifest_sha256: [u8; 32] = Sha256::digest(candidate_manifest_bytes).into();
    let step_eq_proving_key = kagemusha_artifact_descriptor_v4(
        &candidate.manifest,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )?;
    let step_ep_proving_key = kagemusha_artifact_descriptor_v4(
        &candidate.manifest,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ProvingKey,
    )?;
    let step_eq_opened = directory.open_file(
        &step_eq_proving_key.file_name,
        "qualification receipt Eq proving key",
    )?;
    let step_ep_opened = directory.open_file(
        &step_ep_proving_key.file_name,
        "qualification receipt Ep proving key",
    )?;
    step_eq_opened.verify_unchanged()?;
    step_ep_opened.verify_unchanged()?;
    let step_eq_proving_key_file = step_eq_opened.file.try_clone().map_err(|error| {
        format!("failed to duplicate Kagemusha V4 Eq proving-key handle: {error}")
    })?;
    let step_ep_proving_key_file = step_ep_opened.file.try_clone().map_err(|error| {
        format!("failed to duplicate Kagemusha V4 Ep proving-key handle: {error}")
    })?;
    let qualification_memory_contract =
        KagemushaQualificationMemoryContractV4::for_runtime_catalog(max_decoded_bytes)?;
    verify_candidate_recursive_step_two_receipt_v4(
        candidate,
        candidate_sha256,
        candidate_manifest_sha256,
        receipt,
        &qualification_memory_contract,
        step_eq_proving_key_file,
        step_ep_proving_key_file,
        |parity, kind| {
            if kind == KagemushaPastaCycleArtifactKindV4::ProvingKey {
                return Err(
                    "Kagemusha V4 bounded qualification loader requested a proving key".to_owned(),
                );
            }
            let descriptor =
                kagemusha_artifact_descriptor_v4(authenticated.manifest(), parity, kind)?;
            let mut opened = directory.open_file(
                &descriptor.file_name,
                &format!("qualification receipt {parity:?} {kind:?} artifact"),
            )?;
            let payload = read_kagemusha_pasta_cycle_artifact_v4(
                &mut opened.file,
                authenticated,
                descriptor,
            )?;
            opened.verify_unchanged()?;
            Ok(payload)
        },
    )?;
    step_eq_opened.verify_unchanged()?;
    step_ep_opened.verify_unchanged()?;
    directory.verify_path_identity()
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
    let candidate = manifest
        .immutable_candidate()
        .map_err(|error| format!("Kagemusha V4 qualification candidate is invalid: {error}"))?;
    let qualification_receipt_bytes = read_bounded_directory_file(
        directory,
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
        "recursive-step-two qualification receipt",
    )?;
    let qualification_receipt =
        KagemushaRecursiveSpendQualificationReceiptV4::decode_canonical_against_candidate(
            &qualification_receipt_bytes,
            &candidate,
        )
        .map_err(|error| format!("Kagemusha V4 qualification receipt is invalid: {error}"))?;
    let qualification_receipt_sha256 = qualification_receipt
        .canonical_sha256_against_candidate(&candidate)
        .map_err(|error| format!("Kagemusha V4 qualification receipt is invalid: {error}"))?;
    let qualified_candidate_sha256 = qualification_receipt
        .qualified_candidate_sha256(&candidate)
        .map_err(|error| format!("Kagemusha V4 qualification receipt is invalid: {error}"))?;
    if qualification_receipt_sha256 != manifest.qualification_receipt_sha256
        || qualified_candidate_sha256 != manifest.qualified_candidate_sha256
    {
        return Err(
            "Kagemusha V4 manifest does not bind the exact qualification receipt".to_owned(),
        );
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
    let internal_validation_receipt = read_bounded_directory_file(
        directory,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        "internal-validation receipt",
    )?;
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
        &internal_validation_receipt,
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
        validate_sealed_release_qualification_v1(
            sealed,
            &authenticated,
            &promotion_bytes,
            qualification_receipt_sha256,
        )?;
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
    // A root-owned seal may cache expensive artifact qualification metadata,
    // but it cannot stand in for the proof-bearing continuity receipt. Always
    // authenticate every candidate role and terminally verify the exact stored
    // initialization and one-parent child proof pairs before constructing a
    // production cache entry.
    verify_catalog_qualification_receipt_v4(
        directory,
        &authenticated,
        &candidate,
        &qualification_receipt,
        remaining_decoded_bytes,
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
                internal_validation_receipt,
                physical_device_benchmark_summary,
                cryptographic_review_summary,
                promotion_record,
            },
            qualification_receipt_sha256,
            qualified_candidate_sha256,
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
/// Plan consumption of one unused promotion identity without mutating state.
pub(super) fn plan_v4_promotion_id(
    promotion_id: [u8; 32],
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<Hash, Error> {
    if promotion_id == [0; 32] {
        return Err(labeled_invariant(
            "recursive_release_invalid",
            "Kagemusha V4 promotion id must be nonzero",
        )
        .into());
    }
    let marker = kagemusha_v2_marker(KAGEMUSHA_V4_PROMOTION_ID_DOMAIN, &[&promotion_id]);
    if state_transaction
        .world
        .kagemusha_replay_keys
        .get(&marker)
        .is_some()
    {
        return Err(labeled_invariant(
            "promotion_replay",
            "Kagemusha V4 promotion id was already consumed by a committed activation",
        )
        .into());
    }
    Ok(marker)
}

/// Commit one previously validated promotion identity in the activation overlay.
pub(super) fn commit_v4_promotion_id(
    marker: Hash,
    state_transaction: &mut StateTransaction<'_, '_>,
) {
    state_transaction
        .world
        .kagemusha_replay_keys
        .insert(marker, ());
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
    Ok(kagemusha_recursive_spend_verifier_owner_manifest_id_v4(
        binding.manifest_sha256,
    ))
}
/// Derive the release- and layout-bound public-input identity stored in a V4
/// [`VerifyingKeyRecord`].
pub(crate) fn verifier_public_inputs_schema_hash(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String> {
    kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4(manifest, parity)
        .map_err(|error| error.to_string())
}
#[cfg(any(
    test,
    all(
        unix,
        not(any(target_os = "espidf", target_os = "horizon", target_os = "redox"))
    )
))]
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
#[path = "kagemusha_terminal_registry_v4/inline_tests.rs"]
mod tests;
