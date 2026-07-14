//! Startup catalog for authenticated Kagemusha V4 verifier material.
//!
//! V4 has its own state namespaces, release-record schema, KRV4 framing, and
//! verifier identity. Nothing in this module accepts or upgrades the V3
//! registry representation. Release policy comes from canonical configured
//! Norito; consensus state can select material, but cannot select its signers.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read as _,
    path::Path,
    sync::Arc,
};

use iroha_crypto::Hash;
use iroha_data_model::{
    confidential::ConfidentialStatus,
    name::Name,
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4,
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1, KAGEMUSHA_VERIFIER_NAMESPACE,
        KagemushaAuthenticatedReleaseV4, KagemushaPastaCycleArtifactKindV4,
        KagemushaPastaCycleParityV1, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendArtifactManifestV4, KagemushaRecursiveSpendReleaseActivationV4,
        KagemushaRecursiveSpendReleaseAttestationV4, KagemushaRecursiveSpendReleasePolicyV1,
    },
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
    zk::BackendTag,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use crate::zk::{
    kagemusha_artifact_v4::{
        KagemushaPastaCycleVerifierArtifactsV4, KagemushaValidatedArtifactPayloadV4,
        kagemusha_artifact_descriptor_v4, read_kagemusha_pasta_cycle_artifact_v4,
    },
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
const MAX_PROMOTION_RECORD_BYTES: usize = 1024 * 1024;

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

/// Authenticated verifier material from an exact-eight V4 release.
///
/// The release binds four artifacts per parity, while the runtime resolver
/// installs only the six verifier-side artifacts: parameters, verifying keys,
/// and bootstrap witnesses. Circuit profiles stay inline in the authenticated
/// manifest, and proving keys remain prover-only. The cryptographic parser is
/// deliberately exposed as a fallible constructor: registry authentication and
/// expensive Halo2 key/bootstrap parsing remain separate fail-closed stages,
/// while production callers cannot obtain a V4 verifier without passing
/// through both.
#[derive(Debug)]
pub(crate) struct ResolvedKagemushaTerminalVerifierV4 {
    release: KagemushaAuthenticatedReleaseV4,
    artifacts: KagemushaPastaCycleVerifierArtifactsV4,
}

/// One startup-authenticated ABI-20 release retained for consensus execution.
pub(crate) struct KagemushaCachedReleaseV4 {
    release_record: iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4,
    resolved: ResolvedKagemushaTerminalVerifierV4,
    verifier: Arc<KagemushaPastaCycleOpaqueVerifierV4>,
}

/// Immutable startup catalog keyed by canonical V4 manifest digest.
///
/// The catalog owns parsed verifier-side material. Consensus execution only
/// performs map lookups and never reaches the filesystem.
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
    /// All filesystem access, hashing, framing checks, and Halo2 verifier
    /// parsing complete before the returned immutable catalog is published.
    pub fn load(policy_path: &Path, artifact_dir: &Path) -> Result<Self, String> {
        let policy_bytes =
            read_bounded_regular_file(policy_path, MAX_POLICY_BYTES, "release policy")?;
        let policy = decode_trusted_policy(&policy_bytes)?;
        let policy_sha256: [u8; 32] = Sha256::digest(&policy_bytes).into();

        let root_metadata = fs::symlink_metadata(artifact_dir).map_err(|error| {
            format!(
                "failed to inspect Kagemusha V4 artifact directory `{}`: {error}",
                artifact_dir.display()
            )
        })?;
        if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
            return Err("Kagemusha V4 artifact path must be a real directory".to_owned());
        }

        let mut directories = fs::read_dir(artifact_dir)
            .map_err(|error| {
                format!(
                    "failed to scan Kagemusha V4 artifact directory `{}`: {error}",
                    artifact_dir.display()
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to enumerate Kagemusha V4 releases: {error}"))?;
        directories.sort_by_key(fs::DirEntry::file_name);

        let mut releases = BTreeMap::new();
        for directory in directories {
            let file_name = directory
                .file_name()
                .into_string()
                .map_err(|_| "Kagemusha V4 release directory name is not UTF-8".to_owned())?;
            let manifest_sha256 = parse_manifest_directory_name(&file_name)?;
            let file_type = directory
                .file_type()
                .map_err(|error| format!("failed to inspect Kagemusha V4 release: {error}"))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                return Err(format!(
                    "Kagemusha V4 release `{file_name}` is not a real directory"
                ));
            }
            let release =
                load_release_directory(&directory.path(), manifest_sha256, &policy, policy_sha256)?;
            if releases
                .insert(manifest_sha256, Arc::new(release))
                .is_some()
            {
                return Err("Kagemusha V4 artifact catalog repeats a manifest digest".to_owned());
            }
        }
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

    /// Build the exact governed activation payload for one authenticated release.
    ///
    /// This is the only production constructor for the consensus payload. It
    /// projects both inline verifier records from the immutable, fully parsed
    /// startup catalog, so an operator cannot substitute release fields, key
    /// bytes, commitments, schemas, activation heights, or policy identity.
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
        &self.verifier
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
        let (curve, authenticated_vk) = match parity {
            KagemushaPastaCycleParityV1::StepEq => (
                STEP_EQ_VERIFIER_CURVE_V4,
                self.resolved.artifacts().step_eq_verifying_key(),
            ),
            KagemushaPastaCycleParityV1::StepEp => (
                STEP_EP_VERIFIER_CURVE_V4,
                self.resolved.artifacts().step_ep_verifying_key(),
            ),
        };
        let key = VerifyingKeyBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            authenticated_vk.to_vec(),
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
        record.vk_len = u32::try_from(authenticated_vk.len())
            .map_err(|_| "Kagemusha V4 verifier key length exceeds u32".to_owned())?;
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
            authenticated_vk,
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
            self.resolved.artifacts().step_eq_verifying_key(),
            release.manifest().activation_height,
        )?;
        ensure_activation_record(
            step_ep_record,
            &binding,
            release,
            KagemushaPastaCycleParityV1::StepEp,
            self.resolved.artifacts().step_ep_verifying_key(),
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

fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect Kagemusha V4 {label} `{}`: {error}",
            path.display()
        )
    })?;
    let size = usize::try_from(metadata.len())
        .map_err(|_| format!("Kagemusha V4 {label} length does not fit usize"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || size == 0 || size > maximum {
        return Err(format!(
            "Kagemusha V4 {label} is not a bounded regular file"
        ));
    }
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read Kagemusha V4 {label} `{}`: {error}",
            path.display()
        )
    })?;
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

fn verify_file_descriptor(
    path: &Path,
    expected_size: u64,
    expected_sha256: [u8; 32],
    maximum: u64,
    label: &str,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect Kagemusha V4 {label}: {error}"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() != expected_size
        || expected_size == 0
        || expected_size > maximum
    {
        return Err(format!("Kagemusha V4 {label} size or file type mismatch"));
    }
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open Kagemusha V4 {label}: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        let count = file
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
    if read != expected_size || actual_sha256 != expected_sha256 {
        return Err(format!("Kagemusha V4 {label} digest mismatch"));
    }
    Ok(())
}

fn verify_exact_release_inventory_v4(
    directory: &Path,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
) -> Result<(), String> {
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
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("failed to enumerate Kagemusha V4 release inventory: {error}"))?
    {
        let entry = entry.map_err(|error| {
            format!("failed to inspect Kagemusha V4 release inventory: {error}")
        })?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Kagemusha V4 release file name is not UTF-8".to_owned())?;
        let file_type = entry.file_type().map_err(|error| {
            format!("failed to inspect Kagemusha V4 release file `{name}`: {error}")
        })?;
        if !file_type.is_file() {
            return Err(format!(
                "Kagemusha V4 release entry `{name}` is not a regular file"
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
    Ok(())
}

fn read_release_role(
    directory: &Path,
    release: &KagemushaAuthenticatedReleaseV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let descriptor = kagemusha_artifact_descriptor_v4(release.manifest(), parity, kind)?;
    let path = directory.join(&descriptor.file_name);
    let mut file = File::open(&path).map_err(|error| {
        format!(
            "failed to open Kagemusha V4 artifact `{}`: {error}",
            descriptor.file_name
        )
    })?;
    read_kagemusha_pasta_cycle_artifact_v4(&mut file, release, descriptor)
}

#[allow(clippy::too_many_lines)]
fn load_release_directory(
    directory: &Path,
    expected_manifest_sha256: [u8; 32],
    policy: &KagemushaRecursiveSpendReleasePolicyV1,
    policy_sha256: [u8; 32],
) -> Result<KagemushaCachedReleaseV4, String> {
    let manifest_bytes = read_bounded_regular_file(
        &directory.join(MANIFEST_FILE_NAME_V4),
        MAX_MANIFEST_BYTES,
        "manifest",
    )?;
    let manifest_sha256: [u8; 32] = Sha256::digest(&manifest_bytes).into();
    if manifest_sha256 != expected_manifest_sha256 {
        return Err("Kagemusha V4 manifest digest does not match its directory".to_owned());
    }
    let manifest = decode_canonical_manifest(&manifest_bytes)?;
    verify_exact_release_inventory_v4(directory, &manifest)?;
    let manifest_json = read_bounded_regular_file(
        &directory.join(MANIFEST_JSON_FILE_NAME_V4),
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
    let manifest_sha256_sidecar = read_bounded_regular_file(
        &directory.join(MANIFEST_SHA256_FILE_NAME_V4),
        65,
        "manifest SHA-256 sidecar",
    )?;
    let expected_manifest_sha256_sidecar = format!("{}\n", hex::encode(manifest_sha256));
    if manifest_sha256_sidecar != expected_manifest_sha256_sidecar.as_bytes() {
        return Err("Kagemusha V4 manifest SHA-256 sidecar is stale".to_owned());
    }
    let attestation_bytes = read_bounded_regular_file(
        &directory.join(KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_FILE_NAME_V4),
        MAX_ATTESTATION_BYTES,
        "release attestation",
    )?;
    let release_attestation = decode_canonical_attestation(&attestation_bytes)?;
    let physical_device_benchmark_summary = read_bounded_regular_file(
        &directory.join(KAGEMUSHA_RECURSIVE_SPEND_BENCHMARK_EVIDENCE_FILE_NAME_V1),
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        "physical-device benchmark summary",
    )?;
    let cryptographic_review_summary = read_bounded_regular_file(
        &directory.join(KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_FILE_NAME_V1),
        KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
        "cryptographic-review summary",
    )?;
    let promotion_bytes = read_bounded_regular_file(
        &directory.join(PROMOTION_RECORD_FILE_NAME_V4),
        MAX_PROMOTION_RECORD_BYTES,
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

    let descriptors = manifest
        .profiles
        .iter()
        .flat_map(|profile| profile.artifacts.iter())
        .collect::<Vec<_>>();
    if descriptors.len() != 8 {
        return Err("Kagemusha V4 manifest does not contain exactly eight artifacts".to_owned());
    }
    for descriptor in descriptors {
        verify_file_descriptor(
            &directory.join(&descriptor.file_name),
            descriptor.size_bytes,
            descriptor.sha256,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
            &format!("artifact `{}`", descriptor.file_name),
        )?;
    }
    let roster = &manifest.topup_finality_roster_artifact;
    verify_file_descriptor(
        &directory.join(&roster.file_name),
        roster.size_bytes,
        roster.sha256,
        iroha_data_model::offline::KAGEMUSHA_TOPUP_FINALITY_ROSTER_ARTIFACT_MAX_BYTES_V2,
        "top-up finality roster",
    )?;

    let step_eq_parameters = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    )?;
    let step_eq_verifying_key = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let step_eq_bootstrap_witness = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEq,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    )?;
    let step_ep_parameters = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::ParamsIpa,
    )?;
    let step_ep_verifying_key = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::VerifyingKey,
    )?;
    let step_ep_bootstrap_witness = read_release_role(
        directory,
        &authenticated,
        KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
    )?;
    let artifacts = KagemushaPastaCycleVerifierArtifactsV4::new(
        &authenticated,
        step_eq_parameters,
        step_eq_verifying_key,
        step_eq_bootstrap_witness,
        step_ep_parameters,
        step_ep_verifying_key,
        step_ep_bootstrap_witness,
    )?;
    let resolved = ResolvedKagemushaTerminalVerifierV4 {
        release: authenticated,
        artifacts,
    };
    let verifier = Arc::new(resolved.verifier()?);
    Ok(KagemushaCachedReleaseV4 {
        release_record: iroha_data_model::offline::KagemushaRecursiveSpendReleaseRecordV4 {
            manifest,
            release_attestation,
            physical_device_benchmark_summary,
            cryptographic_review_summary,
            promotion_record,
        },
        resolved,
        verifier,
    })
}

impl ResolvedKagemushaTerminalVerifierV4 {
    pub(crate) fn release(&self) -> &KagemushaAuthenticatedReleaseV4 {
        &self.release
    }

    pub(crate) fn artifacts(&self) -> &KagemushaPastaCycleVerifierArtifactsV4 {
        &self.artifacts
    }

    pub(crate) fn artifact_set(&self) -> KagemushaAuthenticatedArtifactSetV4 {
        let manifest = self.release.manifest();
        KagemushaAuthenticatedArtifactSetV4 {
            generation: manifest.generation.clone(),
            manifest_sha256: self.release.manifest_sha256(),
            release_policy_sha256: self.release.release_policy_sha256(),
            release_attestation_sha256: self.release.release_attestation_sha256(),
            activation_height: manifest.activation_height,
            withdrawal_height: manifest.withdrawal_height,
            max_proof_bytes: manifest.max_proof_bytes,
            asset_scale: manifest.asset_scale,
        }
    }

    pub(crate) fn verifier(&self) -> Result<KagemushaPastaCycleOpaqueVerifierV4, String> {
        ensure_serialized_parameter_degree(
            self.artifacts.step_eq_parameters(),
            self.artifacts.step_eq_profile().ipa_k,
            "Eq",
        )?;
        ensure_serialized_parameter_degree(
            self.artifacts.step_ep_parameters(),
            self.artifacts.step_ep_profile().ipa_k,
            "Ep",
        )?;
        KagemushaPastaCycleOpaqueVerifierV4::from_authenticated_artifacts(&self.artifacts)
    }
}

fn ensure_serialized_parameter_degree(
    bytes: &[u8],
    expected_k: u32,
    role: &str,
) -> Result<(), String> {
    let encoded_k = bytes
        .get(..4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| format!("Kagemusha V4 {role} parameter payload is truncated"))?;
    if encoded_k != expected_k {
        return Err(format!(
            "Kagemusha V4 {role} parameter payload degree does not equal the authenticated profile"
        ));
    }
    Ok(())
}

/// Deterministic V4-only state key for an authenticated release record.
pub(crate) fn release_state_key(
    binding: &KagemushaRecursiveSpendArtifactBindingV4,
) -> Result<Name, String> {
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
    authenticated_vk: &[u8],
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
        || record.commitment == [0; 32]
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
        || state_vk.bytes.as_slice() != authenticated_vk
        || u32::try_from(authenticated_vk.len()).ok() != Some(record.vk_len)
        || crate::zk::hash_vk(state_vk) != record.commitment
    {
        return Err(format!(
            "Kagemusha V4 {role} state verifier key does not equal the authenticated release payload"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_catalog_is_explicitly_unconfigured() {
        let catalog = KagemushaReleaseCatalogV4::empty();
        assert!(!catalog.is_configured());
        assert_eq!(catalog.configured_policy_sha256(), None);
        assert!(catalog.is_empty());
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
    fn configured_catalog_rejects_malformed_policy_before_publication() {
        let temporary = tempfile::tempdir().expect("temporary catalog root");
        let policy = temporary.path().join("policy.norito");
        let artifacts = temporary.path().join("artifacts");
        std::fs::write(&policy, b"not canonical norito").expect("write malformed policy");
        std::fs::create_dir(&artifacts).expect("create artifact root");
        let error = KagemushaReleaseCatalogV4::load(&policy, &artifacts)
            .err()
            .expect("malformed configured policy must fail closed");
        assert!(error.contains("policy") || error.contains("malformed"));
    }
}
