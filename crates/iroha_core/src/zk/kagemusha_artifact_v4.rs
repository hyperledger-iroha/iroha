//! Authenticated framing and role-safe carriers for Kagemusha ABI-21 artifacts.
//!
//! V4 packages are selector-free and intentionally reject any pre-release
//! format. Every release-sized allocation is preceded by a fixed upper-bound,
//! descriptor, and framing check. Framed bytes are first authenticated against
//! the canonical manifest; production constructors then require a separately
//! authenticated [`KagemushaAuthenticatedReleaseV4`].

use std::{
    collections::BTreeSet,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};

#[cfg(feature = "kagemusha-candidate-evidence-lab")]
use iroha_data_model::offline::KagemushaRecursiveSpendCandidateV4;
use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4,
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
    KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4, KagemushaAuthenticatedReleaseV4,
    KagemushaPastaCycleArtifactKindV4, KagemushaPastaCycleArtifactV4,
    KagemushaPastaCycleFramedArtifactHeaderV4, KagemushaPastaCycleParityV1,
    KagemushaPastaCycleProofProfileV4, KagemushaRecursiveSpendArtifactManifestV4,
};
use sha2::{Digest as _, Sha256};

/// Framing magic for a streamed ABI-21 artifact.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4: &[u8; 8] =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_KEY_MAGIC_V4;
/// Defensive limit checked before allocating an encoded V4 header.
pub const KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4: usize =
    KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_MAX_BYTES_V4 as usize;

/// Bounded public carrier used by the core-owned KRV4 framing and parser.
pub type KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 =
    KagemushaPastaCycleFramedArtifactHeaderV4;

static NEXT_EXPORT_TEMP_ID_V4: AtomicU64 = AtomicU64::new(0);

/// Return the canonical file name for one of the eight V4 artifact roles.
#[must_use]
pub const fn kagemusha_artifact_file_name_v4(
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> &'static str {
    match (parity, kind) {
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PARAMS_IPA_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::ProvingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_PROVING_KEY_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEq, KagemushaPastaCycleArtifactKindV4::VerifyingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFYING_KEY_FILE_NAME_V4
        }
        (
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ) => KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_BOOTSTRAP_FILE_NAME_V4,
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::ParamsIpa) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PARAMS_IPA_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::ProvingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_PROVING_KEY_FILE_NAME_V4
        }
        (KagemushaPastaCycleParityV1::StepEp, KagemushaPastaCycleArtifactKindV4::VerifyingKey) => {
            KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFYING_KEY_FILE_NAME_V4
        }
        (
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
        ) => KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_BOOTSTRAP_FILE_NAME_V4,
    }
}

fn validate_header_v4(
    header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
) -> Result<(), String> {
    header.validate().map_err(|error| error.to_string())
}

fn validate_header_against_manifest_v4(
    header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<(), String> {
    header
        .validate_against_manifest(manifest, descriptor)
        .map_err(|error| error.to_string())
}

fn export_header_v4(
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaRecursiveSpendPastaCycleArtifactHeaderV4, String> {
    export_header_from_identity_v4(
        generation,
        profile,
        kind,
        u64::try_from(payload.len())
            .map_err(|_| "Kagemusha V4 payload length does not fit u64".to_owned())?,
        Sha256::digest(payload).into(),
    )
}

fn export_header_from_identity_v4(
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload_size_bytes: u64,
    payload_sha256: [u8; 32],
) -> Result<KagemushaRecursiveSpendPastaCycleArtifactHeaderV4, String> {
    profile
        .circuit_params
        .validate()
        .map_err(|error| error.to_string())?;
    if profile.ipa_k != profile.circuit_params.k
        || profile.compiled_protocol_structure_sha256 == [0; 32]
        || profile.step_proof_size_bytes != profile.circuit_params.max_parent_proof_bytes
        || payload_size_bytes == 0
        || payload_size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        || payload_sha256 == [0; 32]
    {
        return Err("Kagemusha V4 export profile or payload is invalid".to_owned());
    }
    let header = KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_HEADER_VERSION_V4,
        manifest_schema: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4.to_owned(),
        bridge_abi_version: KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        generation: generation.to_owned(),
        parity: profile.parity,
        circuit_id: profile.circuit_id.clone(),
        parameter_generation: profile.parameter_generation.clone(),
        ipa_k: profile.ipa_k,
        circuit_params_sha256: profile
            .circuit_params
            .sha256()
            .map_err(|error| error.to_string())?,
        compiled_protocol_structure_sha256: profile.compiled_protocol_structure_sha256,
        step_proof_size_bytes: profile.step_proof_size_bytes,
        kind,
        payload_size_bytes,
        payload_sha256,
    };
    validate_header_v4(&header)?;
    Ok(header)
}

/// Stream one canonical KRV4 package and return its exact manifest descriptor.
///
/// The supplied profile is the measured profile being assembled; its inline
/// circuit parameters are bound by the bounded header. The caller must insert
/// the returned descriptor into that profile
/// and validate the completed manifest before release signing.
pub fn write_kagemusha_pasta_cycle_artifact_v4<W: Write>(
    writer: &mut W,
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaPastaCycleArtifactV4, String> {
    let header = export_header_v4(generation, profile, kind, payload)?;
    let header_bytes = norito::to_bytes(&header)
        .map_err(|error| format!("failed to encode Kagemusha V4 artifact header: {error}"))?;
    if header_bytes.is_empty()
        || header_bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
    {
        return Err("Kagemusha V4 encoded header exceeds its bound".to_owned());
    }
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "Kagemusha V4 header length does not fit u32".to_owned())?;
    let header_len_bytes = header_len.to_le_bytes();
    let size_bytes = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len())
        .ok()
        .and_then(|size| size.checked_add(u64::try_from(header_len_bytes.len()).ok()?))
        .and_then(|size| size.checked_add(u64::from(header_len)))
        .and_then(|size| size.checked_add(header.payload_size_bytes))
        .ok_or_else(|| "Kagemusha V4 framed artifact size overflow".to_owned())?;
    if size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 framed artifact size {size_bytes} exceeds explicit ceiling {}",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        ));
    }

    let mut framed_hasher = Sha256::new();
    for bytes in [
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.as_slice(),
        header_len_bytes.as_slice(),
        header_bytes.as_slice(),
        payload,
    ] {
        writer
            .write_all(bytes)
            .map_err(|error| format!("failed to write Kagemusha V4 artifact: {error}"))?;
        framed_hasher.update(bytes);
    }
    let descriptor = KagemushaPastaCycleArtifactV4 {
        kind,
        file_name: kagemusha_artifact_file_name_v4(profile.parity, kind).to_owned(),
        size_bytes,
        sha256: framed_hasher.finalize().into(),
        payload_size_bytes: header.payload_size_bytes,
        payload_sha256: header.payload_sha256,
    };
    descriptor.validate().map_err(|error| error.to_string())?;
    Ok(descriptor)
}

/// Stream a pre-authenticated payload into one canonical KRV4 package.
///
/// The declared identity must come from the same bounded writer that produced
/// `payload`. This routine rereads in 1 MiB chunks, rejects early EOF and
/// trailing bytes, and independently verifies the digest while framing, so a
/// release-sized proving key never needs a second in-memory byte vector.
pub fn write_kagemusha_pasta_cycle_artifact_from_reader_v4<W: Write, R: Read>(
    writer: &mut W,
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &mut R,
    payload_size_bytes: u64,
    payload_sha256: [u8; 32],
) -> Result<KagemushaPastaCycleArtifactV4, String> {
    let header = export_header_from_identity_v4(
        generation,
        profile,
        kind,
        payload_size_bytes,
        payload_sha256,
    )?;
    let header_bytes = norito::to_bytes(&header)
        .map_err(|error| format!("failed to encode Kagemusha V4 artifact header: {error}"))?;
    if header_bytes.is_empty()
        || header_bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
    {
        return Err("Kagemusha V4 encoded header exceeds its bound".to_owned());
    }
    let header_len = u32::try_from(header_bytes.len())
        .map_err(|_| "Kagemusha V4 header length does not fit u32".to_owned())?;
    let header_len_bytes = header_len.to_le_bytes();
    let size_bytes = u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len())
        .ok()
        .and_then(|size| size.checked_add(u64::try_from(header_len_bytes.len()).ok()?))
        .and_then(|size| size.checked_add(u64::from(header_len)))
        .and_then(|size| size.checked_add(payload_size_bytes))
        .ok_or_else(|| "Kagemusha V4 framed artifact size overflow".to_owned())?;
    if size_bytes > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4 {
        return Err(format!(
            "Kagemusha V4 framed artifact size {size_bytes} exceeds explicit ceiling {}",
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
        ));
    }

    let mut framed_hasher = Sha256::new();
    for bytes in [
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.as_slice(),
        header_len_bytes.as_slice(),
        header_bytes.as_slice(),
    ] {
        writer
            .write_all(bytes)
            .map_err(|error| format!("failed to write Kagemusha V4 artifact: {error}"))?;
        framed_hasher.update(bytes);
    }

    let mut payload_hasher = Sha256::new();
    let mut remaining = payload_size_bytes;
    let mut buffer = vec![0_u8; 1024 * 1024];
    while remaining > 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| "Kagemusha V4 payload chunk length does not fit usize".to_owned())?;
        let read = payload
            .read(&mut buffer[..requested])
            .map_err(|error| format!("failed to read Kagemusha V4 payload: {error}"))?;
        if read == 0 {
            return Err("Kagemusha V4 payload ended before its declared length".to_owned());
        }
        let bytes = &buffer[..read];
        writer
            .write_all(bytes)
            .map_err(|error| format!("failed to write Kagemusha V4 artifact: {error}"))?;
        payload_hasher.update(bytes);
        framed_hasher.update(bytes);
        remaining -= u64::try_from(read)
            .map_err(|_| "Kagemusha V4 payload read length does not fit u64".to_owned())?;
    }
    let mut trailing = [0_u8; 1];
    if payload
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha V4 payload boundary: {error}"))?
        != 0
    {
        return Err("Kagemusha V4 payload exceeds its declared length".to_owned());
    }
    if <[u8; 32]>::from(payload_hasher.finalize()) != payload_sha256 {
        return Err("Kagemusha V4 payload digest changed while framing".to_owned());
    }

    let descriptor = KagemushaPastaCycleArtifactV4 {
        kind,
        file_name: kagemusha_artifact_file_name_v4(profile.parity, kind).to_owned(),
        size_bytes,
        sha256: framed_hasher.finalize().into(),
        payload_size_bytes,
        payload_sha256,
    };
    descriptor.validate().map_err(|error| error.to_string())?;
    Ok(descriptor)
}

/// One atomically published KRV4 file and its exact manifest descriptor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaExportedArtifactV4 {
    path: PathBuf,
    descriptor: KagemushaPastaCycleArtifactV4,
}

impl KagemushaExportedArtifactV4 {
    /// Final canonical file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Descriptor to insert into the corresponding V4 manifest profile.
    #[must_use]
    pub const fn descriptor(&self) -> &KagemushaPastaCycleArtifactV4 {
        &self.descriptor
    }
}

fn ensure_private_export_directory_v4(directory: &Path) -> Result<(), String> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err("Kagemusha V4 export path must be a real directory".to_owned());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(directory).map_err(|error| {
                format!("failed to create Kagemusha V4 export directory: {error}")
            })?;
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect Kagemusha V4 export directory: {error}"
            ));
        }
    }
    #[cfg(unix)]
    {
        let mode = fs::metadata(directory)
            .map_err(|error| format!("failed to inspect Kagemusha V4 directory mode: {error}"))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(
                "Kagemusha V4 export directory must not grant group/other permissions".to_owned(),
            );
        }
    }
    Ok(())
}

fn create_private_temp_v4(directory: &Path, final_name: &str) -> Result<(PathBuf, File), String> {
    for _ in 0..128 {
        let id = NEXT_EXPORT_TEMP_ID_V4.fetch_add(1, Ordering::Relaxed);
        let temp_path = directory.join(format!(".{final_name}.tmp.{}.{}", std::process::id(), id));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&temp_path) {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "failed to create private Kagemusha V4 temporary file: {error}"
                ));
            }
        }
    }
    Err("failed to reserve a unique Kagemusha V4 temporary file".to_owned())
}

/// Atomically publish one owner-only KRV4 file without overwriting any path.
///
/// The output directory is created with mode `0700` on Unix, or rejected when
/// an existing directory grants group/other permissions. The file is fully
/// written and synced at mode `0600`, then atomically linked to its canonical
/// name; an existing destination always fails closed.
pub fn export_kagemusha_pasta_cycle_artifact_v4(
    output_directory: &Path,
    generation: &str,
    profile: &KagemushaPastaCycleProofProfileV4,
    kind: KagemushaPastaCycleArtifactKindV4,
    payload: &[u8],
) -> Result<KagemushaExportedArtifactV4, String> {
    ensure_private_export_directory_v4(output_directory)?;
    let final_name = kagemusha_artifact_file_name_v4(profile.parity, kind);
    let final_path = output_directory.join(final_name);
    if fs::symlink_metadata(&final_path).is_ok() {
        return Err(format!(
            "refusing to overwrite existing Kagemusha V4 artifact `{final_name}`"
        ));
    }
    let (temp_path, mut temp_file) = create_private_temp_v4(output_directory, final_name)?;
    let descriptor = match write_kagemusha_pasta_cycle_artifact_v4(
        &mut temp_file,
        generation,
        profile,
        kind,
        payload,
    ) {
        Ok(descriptor) => descriptor,
        Err(error) => {
            drop(temp_file);
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
    };
    if let Err(error) = temp_file.sync_all() {
        drop(temp_file);
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to sync Kagemusha V4 temporary artifact: {error}"
        ));
    }
    drop(temp_file);
    if let Err(error) = fs::hard_link(&temp_path, &final_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "failed to atomically publish Kagemusha V4 artifact without overwrite: {error}"
        ));
    }
    if let Err(error) = fs::remove_file(&temp_path) {
        let _ = fs::remove_file(&final_path);
        return Err(format!(
            "failed to remove Kagemusha V4 temporary link: {error}"
        ));
    }
    File::open(output_directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("failed to sync Kagemusha V4 export directory: {error}"))?;
    Ok(KagemushaExportedArtifactV4 {
        path: final_path,
        descriptor,
    })
}

/// Fully authenticated unframed bytes from one V4 artifact role.
///
/// Construction is private to the bounded framed reader, preventing callers
/// from labeling arbitrary bytes as release-authenticated material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaValidatedArtifactPayloadV4 {
    header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
    payload: Vec<u8>,
}

/// Trust mode attached to one parsed ABI-21 inventory.
///
/// The candidate variant is compiled only into the explicitly requested
/// non-shipping evidence harness. Keeping the mode in the carrier prevents a
/// candidate payload from being relabelled as release-authenticated material
/// while still allowing both modes to exercise the same prover and verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
enum KagemushaArtifactManifestBindingV4 {
    AuthenticatedRelease(KagemushaAuthenticatedReleaseV4),
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    CandidateEvidence {
        candidate: KagemushaRecursiveSpendCandidateV4,
        candidate_sha256: [u8; 32],
        manifest_sha256: [u8; 32],
    },
}

impl KagemushaArtifactManifestBindingV4 {
    fn authenticated_release(release: &KagemushaAuthenticatedReleaseV4) -> Self {
        Self::AuthenticatedRelease(release.clone())
    }

    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    fn candidate_evidence(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
    ) -> Result<Self, String> {
        candidate.validate().map_err(|error| error.to_string())?;
        let candidate_sha256 = candidate.sha256().map_err(|error| error.to_string())?;
        let manifest_bytes = norito::to_bytes(&candidate.manifest).map_err(|error| {
            format!("failed to encode Kagemusha V4 candidate manifest: {error}")
        })?;
        let manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
        if candidate_sha256 == [0; 32]
            || manifest_sha256 == [0; 32]
            || candidate_sha256 != expected_candidate_sha256
            || manifest_sha256 != expected_manifest_sha256
        {
            return Err("Kagemusha V4 candidate identity mismatch".to_owned());
        }
        Ok(Self::CandidateEvidence {
            candidate: candidate.clone(),
            candidate_sha256,
            manifest_sha256,
        })
    }

    fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence { candidate, .. } => &candidate.manifest,
        }
    }

    fn manifest_sha256(&self) -> [u8; 32] {
        match self {
            Self::AuthenticatedRelease(release) => release.manifest_sha256(),
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence {
                manifest_sha256, ..
            } => *manifest_sha256,
        }
    }

    fn is_candidate_evidence_lab(&self) -> bool {
        #[cfg(feature = "kagemusha-candidate-evidence-lab")]
        if matches!(self, Self::CandidateEvidence { .. }) {
            return true;
        }
        false
    }

    fn validate(&self) -> Result<(), String> {
        match self {
            Self::AuthenticatedRelease(release) => {
                release
                    .manifest()
                    .validate()
                    .map_err(|error| error.to_string())?;
                if release.manifest_sha256() == [0; 32] {
                    return Err("Kagemusha V4 authenticated release digest is zero".to_owned());
                }
                Ok(())
            }
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence {
                candidate,
                candidate_sha256,
                manifest_sha256,
            } => {
                candidate.validate().map_err(|error| error.to_string())?;
                let observed_candidate_sha256 =
                    candidate.sha256().map_err(|error| error.to_string())?;
                let manifest_bytes = norito::to_bytes(&candidate.manifest).map_err(|error| {
                    format!("failed to encode Kagemusha V4 candidate manifest: {error}")
                })?;
                let observed_manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
                if *candidate_sha256 == [0; 32]
                    || *manifest_sha256 == [0; 32]
                    || observed_candidate_sha256 != *candidate_sha256
                    || observed_manifest_sha256 != *manifest_sha256
                {
                    return Err("Kagemusha V4 candidate binding changed".to_owned());
                }
                Ok(())
            }
        }
    }

    fn validate_header(
        &self,
        header: &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4,
        descriptor: &KagemushaPastaCycleArtifactV4,
    ) -> Result<(), String> {
        match self {
            Self::AuthenticatedRelease(release) => {
                validate_header_against_manifest_v4(header, release.manifest(), descriptor)
            }
            #[cfg(feature = "kagemusha-candidate-evidence-lab")]
            Self::CandidateEvidence { candidate, .. } => {
                candidate.validate().map_err(|error| error.to_string())?;
                header
                    .validate_against_candidate_manifest(&candidate.manifest, descriptor)
                    .map_err(|error| error.to_string())
            }
        }
    }
}

impl KagemushaValidatedArtifactPayloadV4 {
    /// Return the authenticated role header.
    #[must_use]
    pub fn header(&self) -> &KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 {
        &self.header
    }

    /// Return the exact authenticated unframed payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn validate_payload(&self) -> Result<(), String> {
        validate_header_v4(&self.header)?;
        if u64::try_from(self.payload.len())
            .ok()
            .is_none_or(|len| len != self.header.payload_size_bytes)
            || <[u8; 32]>::from(Sha256::digest(&self.payload)) != self.header.payload_sha256
        {
            return Err("Kagemusha V4 authenticated artifact payload mismatch".to_owned());
        }
        Ok(())
    }
}

/// Locate one exact role in a validated Eq-then-Ep V4 manifest inventory.
pub fn kagemusha_artifact_descriptor_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<&KagemushaPastaCycleArtifactV4, String> {
    manifest.validate().map_err(|error| error.to_string())?;
    manifest
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .and_then(|profile| {
            profile
                .artifacts
                .iter()
                .find(|descriptor| descriptor.kind == kind)
        })
        .ok_or_else(|| "Kagemusha V4 artifact manifest role is absent".to_owned())
}

/// Read and authenticate one complete framed V4 artifact from a pinned handle.
///
/// Header and payload lengths are checked against hard limits and the exact
/// manifest descriptor before allocation. The reader must contain exactly one
/// artifact and no trailing bytes.
pub fn read_kagemusha_pasta_cycle_artifact_v4<R: Read>(
    reader: &mut R,
    release: &KagemushaAuthenticatedReleaseV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let binding = KagemushaArtifactManifestBindingV4::authenticated_release(release);
    read_kagemusha_pasta_cycle_artifact_with_binding_v4(reader, &binding, descriptor)
}

/// Parse one exact KRV4 artifact against a clean, canonical pre-promotion
/// candidate. This API exists only in explicitly feature-selected evidence-lab
/// builds and does not manufacture an authenticated production release.
#[cfg(feature = "kagemusha-candidate-evidence-lab")]
pub fn read_kagemusha_pasta_cycle_candidate_artifact_v4<R: Read>(
    reader: &mut R,
    candidate: &KagemushaRecursiveSpendCandidateV4,
    expected_candidate_sha256: [u8; 32],
    expected_manifest_sha256: [u8; 32],
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    let binding = KagemushaArtifactManifestBindingV4::candidate_evidence(
        candidate,
        expected_candidate_sha256,
        expected_manifest_sha256,
    )?;
    read_kagemusha_pasta_cycle_artifact_with_binding_v4(reader, &binding, descriptor)
}

fn read_kagemusha_pasta_cycle_artifact_with_binding_v4<R: Read>(
    reader: &mut R,
    binding: &KagemushaArtifactManifestBindingV4,
    descriptor: &KagemushaPastaCycleArtifactV4,
) -> Result<KagemushaValidatedArtifactPayloadV4, String> {
    binding.validate()?;
    descriptor.validate().map_err(|error| error.to_string())?;

    let mut framed_hasher = Sha256::new();
    let mut magic = [0_u8; KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact magic: {error}"))?;
    framed_hasher.update(magic);
    if &magic != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_ARTIFACT_MAGIC_V4 {
        return Err("Kagemusha V4 artifact magic mismatch".to_owned());
    }

    let mut header_len_bytes = [0_u8; 4];
    reader
        .read_exact(&mut header_len_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 header length: {error}"))?;
    framed_hasher.update(header_len_bytes);
    let header_len = usize::try_from(u32::from_le_bytes(header_len_bytes))
        .map_err(|_| "Kagemusha V4 header length does not fit usize".to_owned())?;
    let prefix_len = magic
        .len()
        .checked_add(header_len_bytes.len())
        .and_then(|len| len.checked_add(header_len))
        .ok_or_else(|| "Kagemusha V4 artifact prefix length overflow".to_owned())?;
    if header_len == 0
        || header_len > KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_MAX_HEADER_BYTES_V4
        || u64::try_from(prefix_len)
            .ok()
            .is_none_or(|prefix| prefix >= descriptor.size_bytes)
    {
        return Err("Kagemusha V4 artifact header length is invalid".to_owned());
    }

    let mut header_bytes = vec![0_u8; header_len];
    reader
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact header: {error}"))?;
    framed_hasher.update(&header_bytes);
    let header: KagemushaRecursiveSpendPastaCycleArtifactHeaderV4 =
        norito::decode_from_bytes(&header_bytes)
            .map_err(|_| "Kagemusha V4 artifact header is malformed".to_owned())?;
    if norito::to_bytes(&header)
        .map_err(|error| format!("failed to re-encode Kagemusha V4 header: {error}"))?
        != header_bytes
    {
        return Err("Kagemusha V4 artifact header is not canonical".to_owned());
    }
    binding.validate_header(&header, descriptor)?;
    if u64::try_from(prefix_len)
        .ok()
        .and_then(|prefix| prefix.checked_add(header.payload_size_bytes))
        != Some(descriptor.size_bytes)
    {
        return Err("Kagemusha V4 artifact payload length mismatch".to_owned());
    }

    let payload_len = usize::try_from(header.payload_size_bytes)
        .map_err(|_| "Kagemusha V4 payload length does not fit usize".to_owned())?;
    if payload_len == 0
        || u64::try_from(payload_len)
            .ok()
            .is_none_or(|len| len > KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4)
    {
        return Err("Kagemusha V4 artifact payload length is invalid".to_owned());
    }
    let mut payload = vec![0_u8; payload_len];
    reader
        .read_exact(&mut payload)
        .map_err(|error| format!("failed to read Kagemusha V4 artifact payload: {error}"))?;
    let payload_sha256: [u8; 32] = Sha256::digest(&payload).into();
    framed_hasher.update(&payload);
    let framed_sha256: [u8; 32] = framed_hasher.finalize().into();
    let mut trailing = [0_u8; 1];
    if reader
        .read(&mut trailing)
        .map_err(|error| format!("failed to check Kagemusha V4 trailing bytes: {error}"))?
        != 0
        || payload_sha256 != descriptor.payload_sha256
        || framed_sha256 != descriptor.sha256
    {
        return Err("Kagemusha V4 artifact content digest mismatch".to_owned());
    }
    Ok(KagemushaValidatedArtifactPayloadV4 { header, payload })
}

fn validate_role(
    binding: &KagemushaArtifactManifestBindingV4,
    artifact: &KagemushaValidatedArtifactPayloadV4,
    parity: KagemushaPastaCycleParityV1,
    kind: KagemushaPastaCycleArtifactKindV4,
) -> Result<(), String> {
    binding.validate()?;
    artifact.validate_payload()?;
    if artifact.header.parity != parity || artifact.header.kind != kind {
        return Err("Kagemusha V4 artifact carrier role mismatch".to_owned());
    }
    let descriptor = binding
        .manifest()
        .profiles
        .iter()
        .find(|profile| profile.parity == parity)
        .and_then(|profile| {
            profile
                .artifacts
                .iter()
                .find(|descriptor| descriptor.kind == kind)
        })
        .ok_or_else(|| "Kagemusha V4 artifact manifest role is absent".to_owned())?;
    binding.validate_header(&artifact.header, descriptor)
}

/// Exact six-role verifier material bound to one authenticated V4 release.
///
/// Bootstrap witnesses remain opaque here; the recursion adapter is the sole
/// owner of their canonical typed representation and validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleVerifierArtifactsV4 {
    binding: KagemushaArtifactManifestBindingV4,
    step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
    step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
    step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
    step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
    step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
}

impl KagemushaPastaCycleVerifierArtifactsV4 {
    /// Bind all six verifier roles to one authenticated release.
    pub fn new(
        release: &KagemushaAuthenticatedReleaseV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::authenticated_release(release),
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    /// Bind all six verifier roles to one clean candidate in a non-shipping
    /// evidence-lab build without relabelling it as a promoted release.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    #[allow(clippy::too_many_arguments)]
    pub fn new_candidate_evidence_lab(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::candidate_evidence(
                candidate,
                expected_candidate_sha256,
                expected_manifest_sha256,
            )?,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_binding(
        binding: KagemushaArtifactManifestBindingV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        binding.validate()?;
        let artifacts = [
            (
                &step_eq_parameters,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                &step_eq_verifying_key,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                &step_eq_bootstrap_witness,
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
            (
                &step_ep_parameters,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                &step_ep_verifying_key,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                &step_ep_bootstrap_witness,
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
        ];
        let mut digests = BTreeSet::new();
        for (artifact, parity, kind) in artifacts {
            validate_role(&binding, artifact, parity, kind)?;
            if !digests.insert(artifact.header.payload_sha256) {
                return Err("Kagemusha V4 verifier payloads are not distinct".to_owned());
            }
        }
        Ok(Self {
            binding,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        })
    }

    /// SHA-256 of the exact authenticated manifest selecting every role.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.binding.manifest_sha256()
    }

    /// Authenticated manifest selecting every verifier role.
    #[must_use]
    pub(crate) fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        self.binding.manifest()
    }

    #[must_use]
    pub(crate) fn is_candidate_evidence_lab(&self) -> bool {
        self.binding.is_candidate_evidence_lab()
    }

    /// Exact release-specific proof-pair cap.
    #[must_use]
    pub fn max_proof_bytes(&self) -> u32 {
        self.binding.manifest().max_proof_bytes
    }

    pub(crate) fn step_eq_parameters(&self) -> &[u8] {
        self.step_eq_parameters.payload()
    }

    pub(crate) fn step_eq_verifying_key(&self) -> &[u8] {
        self.step_eq_verifying_key.payload()
    }

    pub(crate) fn step_eq_bootstrap_witness(&self) -> &[u8] {
        self.step_eq_bootstrap_witness.payload()
    }

    pub(crate) fn step_ep_parameters(&self) -> &[u8] {
        self.step_ep_parameters.payload()
    }

    pub(crate) fn step_ep_verifying_key(&self) -> &[u8] {
        self.step_ep_verifying_key.payload()
    }

    pub(crate) fn step_ep_bootstrap_witness(&self) -> &[u8] {
        self.step_ep_bootstrap_witness.payload()
    }

    fn payload_digests(&self) -> [[u8; 32]; 6] {
        [
            self.step_eq_parameters.header.payload_sha256,
            self.step_eq_verifying_key.header.payload_sha256,
            self.step_eq_bootstrap_witness.header.payload_sha256,
            self.step_ep_parameters.header.payload_sha256,
            self.step_ep_verifying_key.header.payload_sha256,
            self.step_ep_bootstrap_witness.header.payload_sha256,
        ]
    }
}

/// Exact eight-role prover material bound to one authenticated V4 release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPastaCycleProverArtifactsV4 {
    verifier: KagemushaPastaCycleVerifierArtifactsV4,
    step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
    step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
}

impl KagemushaPastaCycleProverArtifactsV4 {
    /// Bind the complete eight-artifact inventory to one authenticated release.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        release: &KagemushaAuthenticatedReleaseV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::authenticated_release(release),
            step_eq_parameters,
            step_eq_proving_key,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_proving_key,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    /// Bind the exact eight-role inventory to one clean candidate in an
    /// explicitly selected, non-shipping evidence-lab build.
    #[cfg(feature = "kagemusha-candidate-evidence-lab")]
    #[allow(clippy::too_many_arguments)]
    pub fn new_candidate_evidence_lab(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        expected_candidate_sha256: [u8; 32],
        expected_manifest_sha256: [u8; 32],
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        Self::new_with_binding(
            KagemushaArtifactManifestBindingV4::candidate_evidence(
                candidate,
                expected_candidate_sha256,
                expected_manifest_sha256,
            )?,
            step_eq_parameters,
            step_eq_proving_key,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_proving_key,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_binding(
        binding: KagemushaArtifactManifestBindingV4,
        step_eq_parameters: KagemushaValidatedArtifactPayloadV4,
        step_eq_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_eq_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
        step_ep_parameters: KagemushaValidatedArtifactPayloadV4,
        step_ep_proving_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_verifying_key: KagemushaValidatedArtifactPayloadV4,
        step_ep_bootstrap_witness: KagemushaValidatedArtifactPayloadV4,
    ) -> Result<Self, String> {
        binding.validate()?;
        validate_role(
            &binding,
            &step_eq_proving_key,
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
        )?;
        validate_role(
            &binding,
            &step_ep_proving_key,
            KagemushaPastaCycleParityV1::StepEp,
            KagemushaPastaCycleArtifactKindV4::ProvingKey,
        )?;
        let verifier = KagemushaPastaCycleVerifierArtifactsV4::new_with_binding(
            binding,
            step_eq_parameters,
            step_eq_verifying_key,
            step_eq_bootstrap_witness,
            step_ep_parameters,
            step_ep_verifying_key,
            step_ep_bootstrap_witness,
        )?;
        let mut digests: BTreeSet<_> = verifier.payload_digests().into_iter().collect();
        if !digests.insert(step_eq_proving_key.header.payload_sha256)
            || !digests.insert(step_ep_proving_key.header.payload_sha256)
        {
            return Err("Kagemusha V4 prover payloads are not distinct".to_owned());
        }
        Ok(Self {
            verifier,
            step_eq_proving_key,
            step_ep_proving_key,
        })
    }

    /// SHA-256 of the exact authenticated manifest selecting all eight roles.
    #[must_use]
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.verifier.manifest_sha256()
    }

    /// Exact release-specific proof-pair cap.
    #[must_use]
    pub fn max_proof_bytes(&self) -> u32 {
        self.verifier.max_proof_bytes()
    }

    #[must_use]
    pub(crate) fn verifier(&self) -> &KagemushaPastaCycleVerifierArtifactsV4 {
        &self.verifier
    }
}
