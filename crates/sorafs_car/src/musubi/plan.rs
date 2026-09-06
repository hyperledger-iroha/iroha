//! Canonical Musubi CAR plan admission and phase-specific resource policies.
//!
//! Plan admission validates geometry and the declared archive commitment. It does not read a CAR,
//! verify payload bytes or PoR, or attest to semantic bundle contents. Those checks belong to
//! [`super::MusubiBundleVerifierV1`]. The closed contexts keep ingress witness admission separate
//! from the stricter provider verification phase without exposing caller-selected budget knobs.

use super::{
    MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1, MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1,
    MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1, MusubiBundleIntegritySurfaceV1,
    MusubiBundleVerificationErrorV1, SOURCE_TREE_DOMAIN_V1, frame_length,
};
use crate::{
    CarBuildPlan, DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES, ProfileId,
    compute_chunk_plan_digest_sha3, sorafs_chunker::ChunkProfile,
};
use iroha_data_model::musubi::{
    MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1, MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1,
    MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_CHUNKS_V1,
    MUSUBI_MAX_FILES_V1, MusubiArchiveCommitmentV1, validate_musubi_portable_path_set_v1,
};

/// Provider verification heap/RSS qualification target used to size individual phase controls.
///
/// The controls below are not a proof that the complete process remains under this target.
pub(super) const PROVIDER_FETCH_MEMORY_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
/// Plan/PoR construction is a separate phase and may consume at most half the gate.
pub(super) const PROVIDER_FETCH_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES_V1: usize =
    PROVIDER_FETCH_MEMORY_MAX_BYTES_V1 / 2;
/// Maximum aggregate bytes captured for all three mandatory metadata files.
pub(super) const BUNDLE_METADATA_TOTAL_MAX_BYTES_V1: u64 =
    2 * MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1 + MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1;
/// Maximum normalized source-tree transcript retained during semantic verification.
pub(super) const SOURCE_TREE_TRANSCRIPT_MAX_BYTES_V1: usize = 18 * 1024 * 1024;

const BUNDLE_METADATA_FILE_COUNT: usize = 3;

/// Fixed admission context for a Musubi CAR plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPlanValidationContextV1 {
    /// Admit a wire witness under the canonical ingest heap limit; do not apply provider-only
    /// metadata-capture or source-transcript phase limits.
    SeedIngress,
    /// Admit a plan for complete provider verification under its existing bounded phase limits.
    ProviderFetch,
}
impl MusubiPlanValidationContextV1 {
    const fn heap_limit(self) -> usize {
        match self {
            Self::SeedIngress => DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            Self::ProviderFetch => PROVIDER_FETCH_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES_V1,
        }
    }
}

/// Resolve the exact registered chunker selected by a valid archive commitment.
///
/// # Errors
///
/// Returns an archive-commitment error for invalid commitment shape, an unknown profile, or any
/// mismatch in its namespace, name, version, or multihash code. No chunker aliases are accepted.
pub fn resolve_chunk_profile_v1(
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<ChunkProfile, MusubiBundleVerificationErrorV1> {
    commitment.validate().map_err(|_| archive_error())?;
    registered_chunk_profile(commitment)
}

fn archive_error() -> MusubiBundleVerificationErrorV1 {
    MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment)
}

fn registered_chunk_profile(
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<ChunkProfile, MusubiBundleVerificationErrorV1> {
    let descriptor = crate::chunker_registry::lookup(ProfileId(commitment.chunker.profile_id))
        .ok_or_else(archive_error)?;
    if descriptor.namespace != commitment.chunker.namespace
        || descriptor.name != commitment.chunker.name
        || descriptor.semver != commitment.chunker.semver
        || descriptor.multihash_code != commitment.chunker.multihash_code
    {
        return Err(archive_error());
    }
    Ok(descriptor.profile)
}

/// Validate one complete CAR plan against its immutable Musubi commitment and admission context.
///
/// Structural validation and the context's heap bound run before joined-path allocation. Provider
/// admission additionally checks metadata capture and source-transcript capacity before portable
/// path identity, preserving its failure precedence. Neither context verifies actual CAR bytes.
///
/// # Errors
///
/// Portable path-set failures and the provider source-transcript limit report `SourceTree`;
/// malformed geometry, resource bounds, chunker identity and commitment bindings report
/// `ArchiveCommitment`. No attacker-controlled text is retained in the error.
pub fn validate_plan_commitment_v1(
    plan: &CarBuildPlan,
    commitment: &MusubiArchiveCommitmentV1,
    context: MusubiPlanValidationContextV1,
) -> Result<(), MusubiBundleVerificationErrorV1> {
    commitment.validate().map_err(|_| archive_error())?;
    let maximum_files = usize::try_from(MUSUBI_MAX_FILES_V1)
        .unwrap_or(usize::MAX)
        .saturating_add(BUNDLE_METADATA_FILE_COUNT);
    if plan.content_length == 0
        || plan.content_length > MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1
        || commitment.car_size > MUSUBI_MAX_CAR_BYTES_V1
        || plan.chunks.is_empty()
        || plan.chunks.len() > usize::try_from(MUSUBI_MAX_CHUNKS_V1).unwrap_or(usize::MAX)
        || plan.files.len() < BUNDLE_METADATA_FILE_COUNT + 1
        || plan.files.len() > maximum_files
    {
        return Err(archive_error());
    }
    plan.validate_for_ingest_with_limit(context.heap_limit())
        .map_err(|_| archive_error())?;
    if context == MusubiPlanValidationContextV1::ProviderFetch {
        validate_bundle_metadata_capture_geometry(plan)?;
        if source_material_capacity_v1(plan)? > SOURCE_TREE_TRANSCRIPT_MAX_BYTES_V1 {
            return Err(MusubiBundleVerificationErrorV1::at(
                MusubiBundleIntegritySurfaceV1::SourceTree,
            ));
        }
    }
    validate_musubi_portable_path_set_v1(plan.files.iter().map(|file| file.path.as_slice()))
        .map_err(|_| {
            MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::SourceTree)
        })?;
    if registered_chunk_profile(commitment)? != plan.chunk_profile
        || plan.content_length != commitment.content_length
        || plan.chunks.len()
            != usize::try_from(commitment.chunk_count).map_err(|_| archive_error())?
        || compute_chunk_plan_digest_sha3(&plan.chunks) != *commitment.chunk_plan_digest.as_bytes()
    {
        return Err(archive_error());
    }
    let expected_source_files =
        usize::try_from(commitment.file_count).map_err(|_| archive_error())?;
    let expected_files = expected_source_files
        .checked_add(BUNDLE_METADATA_FILE_COUNT)
        .ok_or_else(archive_error)?;
    if plan.files.len() != expected_files {
        return Err(archive_error());
    }
    let mut source_files = 0_usize;
    let mut release_files = 0_u8;
    let mut descriptor_files = 0_u8;
    let mut lock_files = 0_u8;
    for file in &plan.files {
        match file.path.join("/").as_str() {
            MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1 => {
                release_files = release_files.saturating_add(1)
            }
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1 => {
                descriptor_files = descriptor_files.saturating_add(1)
            }
            MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1 => lock_files = lock_files.saturating_add(1),
            path if path.starts_with(".musubi/") => return Err(archive_error()),
            _ => source_files = source_files.saturating_add(1),
        }
    }
    if source_files != expected_source_files
        || release_files != 1
        || descriptor_files != 1
        || lock_files != 1
    {
        return Err(archive_error());
    }
    Ok(())
}

pub(super) fn validate_bundle_metadata_capture_geometry(
    plan: &CarBuildPlan,
) -> Result<(), MusubiBundleVerificationErrorV1> {
    let archive_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment);
    let captured_metadata_bytes = plan.files.iter().try_fold(0_u64, |total, file| {
        let path = file.path.join("/");
        let individual_max = match path.as_str() {
            MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1 | MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1 => {
                MUSUBI_MAX_BUNDLE_METADATA_FILE_BYTES_V1
            }
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1 => MUSUBI_MAX_ARTIFACT_DESCRIPTOR_BYTES_V1,
            _ => return Ok(total),
        };
        if file.size == 0 || file.size > individual_max {
            return Err(archive_error());
        }
        total.checked_add(file.size).ok_or_else(archive_error)
    })?;
    if captured_metadata_bytes > BUNDLE_METADATA_TOTAL_MAX_BYTES_V1 {
        return Err(archive_error());
    }
    Ok(())
}
pub(super) fn source_material_capacity_v1(
    plan: &CarBuildPlan,
) -> Result<usize, MusubiBundleVerificationErrorV1> {
    let archive_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment);
    let source_material_length = plan.files.iter().try_fold(
        frame_length(u64::try_from(SOURCE_TREE_DOMAIN_V1.len()).map_err(|_| archive_error())?)
            .and_then(|length| length.checked_add(4))
            .ok_or_else(archive_error)?,
        |total, file| {
            let path = file.path.join("/");
            if path.starts_with(".musubi/") {
                return Ok(total);
            }
            total
                .checked_add(
                    frame_length(u64::try_from(path.len()).map_err(|_| archive_error())?)
                        .ok_or_else(archive_error)?,
                )
                .and_then(|length| length.checked_add(8 + 32))
                .ok_or_else(archive_error)
        },
    )?;
    usize::try_from(source_material_length).map_err(|_| archive_error())
}
