//! Startup catalog for authenticated Kagemusha V4 verifier material.
//!
//! V4 has its own state namespaces, release-record schema, KRV4 framing, and
//! verifier identity. Nothing in this module accepts or upgrades the V3
//! registry representation. Release policy comes from canonical configured
//! Norito; consensus state can select material, but cannot select its signers.

use std::{collections::BTreeMap, path::Path, sync::Arc};

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
use rustix::fs::{AtFlags, Dir, FileType as RustixFileType, Mode, OFlags, open, openat, statat};
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt as _,
    path::{Component, PathBuf},
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
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
const MAX_CATALOG_DIRECTORY_ENTRIES: usize = 1024;
/// Maximum number of historic authenticated releases retained in one runtime.
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
const MAX_CATALOG_RELEASES_V4: usize = 16;
/// Maximum on-disk bytes described by all exact release inventories combined.
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
const MAX_CATALOG_AGGREGATE_BYTES_V4: u64 = 8 * 1024 * 1024 * 1024;

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
    /// Both configured paths must be canonical absolute paths. Every directory
    /// component is opened relative to its already pinned parent and symlinks
    /// are rejected at every level.
    ///
    /// All filesystem access, hashing, framing checks, and Halo2 verifier
    /// parsing complete before the returned immutable catalog is published.
    pub fn load(policy_path: &Path, artifact_dir: &Path) -> Result<Self, String> {
        #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
        {
            Self::load_descriptor_relative(policy_path, artifact_dir)
        }
        #[cfg(not(all(unix, not(any(target_os = "espidf", target_os = "redox")))))]
        {
            let _ = (policy_path, artifact_dir);
            Err(
                "Kagemusha V4 descriptor-relative catalog loading is unsupported on this platform"
                    .to_owned(),
            )
        }
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    fn load_descriptor_relative(policy_path: &Path, artifact_dir: &Path) -> Result<Self, String> {
        let (policy_parent_path, policy_file_name) =
            absolute_file_parent_and_name(policy_path, "release policy")?;
        let policy_parent =
            CatalogDirectory::open_path(policy_parent_path, "release policy parent")?;
        let mut policy_file = policy_parent.open_file(policy_file_name, "release policy")?;
        let policy_bytes =
            read_bounded_opened_file(&mut policy_file, MAX_POLICY_BYTES, "release policy")?;
        let policy = decode_trusted_policy(&policy_bytes)?;
        let policy_sha256: [u8; 32] = Sha256::digest(&policy_bytes).into();

        let artifact_root = CatalogDirectory::open_path(artifact_dir, "artifact root")?;
        let directory_names = artifact_root.entry_names("artifact root")?;
        ensure_catalog_release_count(directory_names.len())?;

        let mut releases = BTreeMap::new();
        let mut aggregate_catalog_bytes = 0_u64;
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
            let (release, release_bytes) = load_release_directory(
                &directory,
                manifest_sha256,
                &policy,
                policy_sha256,
                remaining_catalog_bytes,
            )?;
            aggregate_catalog_bytes =
                add_catalog_release_bytes(aggregate_catalog_bytes, release_bytes)?;
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn ensure_catalog_release_count(count: usize) -> Result<(), String> {
    if count > MAX_CATALOG_RELEASES_V4 {
        return Err(format!(
            "Kagemusha V4 artifact catalog contains {count} releases; at most {MAX_CATALOG_RELEASES_V4} are retained"
        ));
    }
    Ok(())
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogFileSnapshot {
    identity: CatalogFileIdentity,
    mode: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
impl CatalogFileSnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        (metadata.is_file() && metadata.nlink() == 1).then(|| Self {
            identity: CatalogFileIdentity::from_metadata(metadata),
            mode: metadata.mode(),
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CatalogDirectorySnapshot {
    identity: CatalogFileIdentity,
    mode: u32,
    links: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
impl CatalogDirectorySnapshot {
    fn from_metadata(metadata: &fs::Metadata) -> Option<Self> {
        metadata.is_dir().then(|| Self {
            identity: CatalogFileIdentity::from_metadata(metadata),
            mode: metadata.mode(),
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
            links: u64::try_from(stat.st_nlink).ok()?,
            length: u64::try_from(stat.st_size).ok()?,
            modified_seconds: i64::try_from(stat.st_mtime).ok()?,
            modified_nanoseconds: i64::try_from(stat.st_mtime_nsec).ok()?,
            changed_seconds: i64::try_from(stat.st_ctime).ok()?,
            changed_nanoseconds: i64::try_from(stat.st_ctime_nsec).ok()?,
        })
    }
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
struct CatalogDirectory {
    display_path: PathBuf,
    file: File,
    snapshot: CatalogDirectorySnapshot,
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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
        Ok(Self {
            display_path: self.display_path.join(name),
            file,
            snapshot,
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
        if reopened.snapshot != self.snapshot {
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
struct CatalogOpenedFile<'directory> {
    directory: &'directory CatalogDirectory,
    name: String,
    label: String,
    file: File,
    snapshot: CatalogFileSnapshot,
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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
}

#[cfg(all(test, unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn read_bounded_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, String> {
    let (parent_path, file_name) = absolute_file_parent_and_name(path, label)?;
    let parent = CatalogDirectory::open_path(parent_path, &format!("{label} parent"))?;
    let mut opened = parent.open_file(file_name, label)?;
    let bytes = read_bounded_opened_file(&mut opened, maximum, label)?;
    parent.verify_path_identity()?;
    Ok(bytes)
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn read_bounded_directory_file(
    directory: &CatalogDirectory,
    file_name: &str,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let mut opened = directory.open_file(file_name, label)?;
    read_bounded_opened_file(&mut opened, maximum, label)
}

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn read_release_role(
    directory: &CatalogDirectory,
    release: &KagemushaAuthenticatedReleaseV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let descriptor = kagemusha_artifact_descriptor_v4(release.manifest(), parity, kind)?;
    let mut opened = directory.open_file(
        &descriptor.file_name,
        &format!("artifact `{}`", descriptor.file_name),
    )?;
    let payload = read_kagemusha_pasta_cycle_artifact_v4(&mut opened.file, release, descriptor)?;
    opened.verify_unchanged()?;
    Ok(payload)
}

#[allow(clippy::too_many_lines)]
#[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
fn load_release_directory(
    directory: &CatalogDirectory,
    expected_manifest_sha256: [u8; 32],
    policy: &KagemushaRecursiveSpendReleasePolicyV1,
    policy_sha256: [u8; 32],
    remaining_catalog_bytes: u64,
) -> Result<(KagemushaCachedReleaseV4, u64), String> {
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
            directory,
            &descriptor.file_name,
            descriptor.size_bytes,
            descriptor.sha256,
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
            &format!("artifact `{}`", descriptor.file_name),
        )?;
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
            verifier,
        },
        inventory_bytes,
    ))
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
    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    fn canonical_temporary_root(temporary: &tempfile::TempDir) -> PathBuf {
        std::fs::canonicalize(temporary.path()).expect("canonical temporary catalog root")
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    #[test]
    fn configured_catalog_aggregate_byte_accounting_is_bounded() {
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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
        assert!(error.contains("must be a real directory"));

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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
    #[test]
    fn configured_catalog_paths_must_be_absolute() {
        let error = CatalogDirectory::open_path(Path::new("relative-artifacts"), "artifact root")
            .err()
            .expect("a relative configured catalog path must fail closed");
        assert!(error.contains("absolute"), "unexpected error: {error}");
    }

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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

    #[cfg(all(unix, not(any(target_os = "espidf", target_os = "redox"))))]
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
