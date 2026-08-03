//! Provider-grade verification of canonical Musubi V1 source bundles.
//!
//! A provider must parse and verify the complete semantic bundle before it may attest to an
//! archive. This module centralizes that fail-closed check so seed ingress, storage providers, and
//! later cache integration use one canonical transcript implementation.

use std::{fmt, io::Read};

use iroha_data_model::musubi::{
    MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1, MUSUBI_MAX_CAR_BYTES_V1, MUSUBI_MAX_CHUNKS_V1,
    MUSUBI_MAX_FILES_V1, MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1, MusubiArchiveCommitmentV1,
    MusubiArtifactDescriptorV1, MusubiContentDigestV1, MusubiSemanticReleaseManifestV1,
    MusubiVerificationLockV1, validate_musubi_portable_path_set_v1,
};
use norito::{
    DecodeLimits,
    codec::{Decode as _, Encode as _},
};

use crate::{
    CarBuildPlan, CarVerifier, CarWriteStats, ChunkStore,
    DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES, ProfileId, compute_chunk_plan_digest_sha3,
};

/// Canonical bundle path of the archive-independent semantic release manifest.
pub const MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1: &str = ".musubi/semantic-release.norito";
/// Canonical bundle path of the typed artifact descriptor.
pub const MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1: &str = ".musubi/artifact-descriptor.norito";
/// Canonical bundle path of the normalized exact verification lock.
pub const MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1: &str = ".musubi/verification-lock.norito";

const SOURCE_TREE_DOMAIN_V1: &[u8] = b"musubi-source-tree-v1\0";
const ARTIFACT_DESCRIPTOR_DOMAIN_V1: &[u8] = b"musubi-artifact-descriptor-v1\0";
const BUNDLE_DOMAIN_V1: &[u8] = b"musubi-bundle-v1\0";
const DESCRIPTOR_MAX_BYTES_V1: u64 = 64 * 1024;
const DESCRIPTOR_MAX_BYTES_USIZE_V1: usize = 64 * 1024;
const BUNDLE_METADATA_MAX_BYTES_V1: u64 = MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1;
const BUNDLE_METADATA_MAX_BYTES_USIZE_V1: usize = 64 * 1024 * 1024;
const IO_BUFFER_BYTES: usize = 64 * 1024;
const BUNDLE_METADATA_FILE_COUNT: usize = 3;

const DESCRIPTOR_DECODE_LIMITS_V1: DecodeLimits =
    DecodeLimits::new(32, DESCRIPTOR_MAX_BYTES_USIZE_V1, 256, 128 * 1024, 32);
const SEMANTIC_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    1_024,
    BUNDLE_METADATA_MAX_BYTES_USIZE_V1,
    100_000,
    128 * 1024 * 1024,
    64,
);
// A dense valid graph can carry 1,024 nodes with 256 edges each and up to 16 requirement
// comparators per edge. These fixed cumulative limits cover that public maximum.
const LOCK_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    1_024,
    BUNDLE_METADATA_MAX_BYTES_USIZE_V1,
    8_000_000,
    256 * 1024 * 1024,
    64,
);

/// Closed, payload-free integrity surface reported by the Musubi bundle verifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiBundleIntegritySurfaceV1 {
    /// Archive commitment, plan, canonical CAR, payload, chunk plan, or PoR.
    ArchiveCommitment,
    /// Complete semantic bundle or semantic release manifest.
    Bundle,
    /// Typed artifact descriptor or its transcript binding.
    Descriptor,
    /// Portable source inventory or normalized source-tree transcript.
    SourceTree,
    /// Normalized exact verification lock or its cross-bindings.
    VerificationLock,
}

impl MusubiBundleIntegritySurfaceV1 {
    /// Return the stable low-cardinality label for this integrity surface.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArchiveCommitment => "archive_commitment",
            Self::Bundle => "bundle",
            Self::Descriptor => "descriptor",
            Self::SourceTree => "source_tree",
            Self::VerificationLock => "verification_lock",
        }
    }
}

/// Redacted failure returned by [`MusubiBundleVerifierV1`].
///
/// The error deliberately retains no codec text, file path, payload bytes, provider URL, or
/// other attacker-controlled material. Callers may safely map its closed surface into bounded
/// telemetry and public error codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusubiBundleVerificationErrorV1 {
    surface: MusubiBundleIntegritySurfaceV1,
}

impl MusubiBundleVerificationErrorV1 {
    const fn at(surface: MusubiBundleIntegritySurfaceV1) -> Self {
        Self { surface }
    }

    /// Return the bounded integrity surface at which verification failed.
    #[must_use]
    pub const fn surface(self) -> MusubiBundleIntegritySurfaceV1 {
        self.surface
    }
}

impl fmt::Display for MusubiBundleVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Musubi bundle verification failed at {}",
            self.surface.as_str()
        )
    }
}

impl std::error::Error for MusubiBundleVerificationErrorV1 {}

/// Parsed evidence produced only after every Musubi V1 bundle commitment is verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedMusubiBundleV1 {
    car_stats: CarWriteStats,
    descriptor: MusubiArtifactDescriptorV1,
    semantic_release: MusubiSemanticReleaseManifestV1,
    verification_lock: MusubiVerificationLockV1,
    source_file_count: u32,
    source_bytes: u64,
}

impl VerifiedMusubiBundleV1 {
    /// Return canonical CAR statistics reproduced from the exact input bytes.
    #[must_use]
    pub const fn car_stats(&self) -> &CarWriteStats {
        &self.car_stats
    }

    /// Return the parsed and commitment-bound artifact descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &MusubiArtifactDescriptorV1 {
        &self.descriptor
    }

    /// Return the parsed and commitment-bound semantic release manifest.
    #[must_use]
    pub const fn semantic_release(&self) -> &MusubiSemanticReleaseManifestV1 {
        &self.semantic_release
    }

    /// Return the parsed and commitment-bound normalized verification lock.
    #[must_use]
    pub const fn verification_lock(&self) -> &MusubiVerificationLockV1 {
        &self.verification_lock
    }

    /// Return the verified number of regular source files.
    #[must_use]
    pub const fn source_file_count(&self) -> u32 {
        self.source_file_count
    }

    /// Return the verified total number of regular source bytes.
    #[must_use]
    pub const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}

/// Stateless verifier for one complete canonical Musubi V1 bundle.
#[derive(Clone, Copy, Debug, Default)]
pub struct MusubiBundleVerifierV1;

impl MusubiBundleVerifierV1 {
    /// Verify a canonical CAR, its authenticated payload, and all nested Musubi commitments.
    ///
    /// `plan` is still revalidated at this trust boundary even when its caller has already
    /// validated it. The CAR must be the byte-for-byte canonical encoding of that plan. Exactly
    /// one descriptor, semantic release manifest, and verification lock must be present; all
    /// other files form the normalized source tree.
    ///
    /// # Errors
    ///
    /// Returns a payload-free error identifying only the bounded integrity surface that failed.
    #[allow(
        clippy::too_many_lines,
        reason = "one auditable sequence binds the canonical CAR and every nested bundle transcript"
    )]
    pub fn verify(
        plan: &CarBuildPlan,
        canonical_car: &[u8],
        commitment: &MusubiArchiveCommitmentV1,
    ) -> Result<VerifiedMusubiBundleV1, MusubiBundleVerificationErrorV1> {
        let archive_error = || {
            MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment)
        };
        validate_plan_commitment(plan, commitment)?;
        if canonical_car.is_empty()
            || u64::try_from(canonical_car.len()).map_err(|_| archive_error())?
                != commitment.car_size
            || blake3::hash(canonical_car).as_bytes() != commitment.car_digest.as_bytes()
        {
            return Err(archive_error());
        }

        let verified = CarVerifier::verify_canonical_car_with_plan_retained(plan, canonical_car)
            .map_err(|_| archive_error())?;
        let stats = verified.stats();
        if stats.car_size != commitment.car_size
            || stats.car_archive_digest.as_bytes() != commitment.car_digest.as_bytes()
            || stats.payload_bytes != commitment.content_length
            || stats.chunk_count != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
            || stats.chunk_profile != plan.chunk_profile
            || stats.root_cids.len() != 1
            || stats.root_cids[0].as_slice() != commitment.root_cid.as_bytes()
        {
            return Err(archive_error());
        }

        if payload_digest(verified.payload_reader()).map_err(|_| archive_error())?
            != plan.payload_digest
        {
            return Err(archive_error());
        }

        let mut chunk_store = ChunkStore::with_profile_and_heap_limit(
            plan.chunk_profile,
            DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
        )
        .map_err(|_| archive_error())?;
        let mut payload_reader = verified.payload_reader();
        chunk_store
            .ingest_plan_stream(plan, &mut payload_reader)
            .map_err(|_| archive_error())?;
        if chunk_store.payload_digest() != &plan.payload_digest
            || chunk_store.por_tree().root() != commitment.por_root.as_bytes()
        {
            return Err(archive_error());
        }

        let parsed = verify_bundle_payload(plan, verified.payload_reader(), commitment)?;
        Ok(VerifiedMusubiBundleV1 {
            car_stats: stats.clone(),
            descriptor: parsed.descriptor,
            semantic_release: parsed.semantic_release,
            verification_lock: parsed.verification_lock,
            source_file_count: parsed.source_file_count,
            source_bytes: parsed.source_bytes,
        })
    }
}

fn validate_plan_commitment(
    plan: &CarBuildPlan,
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<(), MusubiBundleVerificationErrorV1> {
    let archive_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment);
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
        || plan
            .chunks
            .iter()
            .any(|chunk| chunk.taikai_segment_hint.is_some())
    {
        return Err(archive_error());
    }
    plan.validate_for_ingest_with_limit(DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES)
        .map_err(|_| archive_error())?;
    validate_musubi_portable_path_set_v1(plan.files.iter().map(|file| file.path.as_slice()))
        .map_err(|_| {
            MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::SourceTree)
        })?;

    let descriptor = crate::chunker_registry::lookup(ProfileId(commitment.chunker.profile_id))
        .ok_or_else(archive_error)?;
    if descriptor.namespace != commitment.chunker.namespace
        || descriptor.name != commitment.chunker.name
        || descriptor.semver != commitment.chunker.semver
        || descriptor.multihash_code != commitment.chunker.multihash_code
        || descriptor.profile != plan.chunk_profile
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
                release_files = release_files.saturating_add(1);
            }
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1 => {
                descriptor_files = descriptor_files.saturating_add(1);
            }
            MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1 => {
                lock_files = lock_files.saturating_add(1);
            }
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

struct ParsedBundlePayloadV1 {
    descriptor: MusubiArtifactDescriptorV1,
    semantic_release: MusubiSemanticReleaseManifestV1,
    verification_lock: MusubiVerificationLockV1,
    source_file_count: u32,
    source_bytes: u64,
}

#[allow(
    clippy::too_many_lines,
    reason = "the complete bounded payload transcript stays contiguous for auditability"
)]
fn verify_bundle_payload(
    plan: &CarBuildPlan,
    mut payload: impl Read,
    commitment: &MusubiArchiveCommitmentV1,
) -> Result<ParsedBundlePayloadV1, MusubiBundleVerificationErrorV1> {
    let archive_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::ArchiveCommitment);
    let bundle_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::Bundle);
    let descriptor_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::Descriptor);
    let source_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::SourceTree);
    let lock_error =
        || MusubiBundleVerificationErrorV1::at(MusubiBundleIntegritySurfaceV1::VerificationLock);

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
    let source_material_capacity =
        usize::try_from(source_material_length).map_err(|_| archive_error())?;
    let mut source_count = 0_u32;
    let mut source_bytes = 0_u64;
    let mut source_entries = Vec::new();
    source_entries
        .try_reserve_exact(usize::try_from(commitment.file_count).map_err(|_| archive_error())?)
        .map_err(|_| archive_error())?;
    let mut semantic_release_bytes = None;
    let mut descriptor_bytes = None;
    let mut verification_lock_bytes = None;
    let mut io_buffer = vec![0_u8; IO_BUFFER_BYTES];
    for file in &plan.files {
        let path = file.path.join("/");
        let capture_bound = match path.as_str() {
            MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1 | MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1 => {
                Some(BUNDLE_METADATA_MAX_BYTES_V1)
            }
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1 => Some(DESCRIPTOR_MAX_BYTES_V1),
            _ => None,
        };
        if let Some(bound) = capture_bound {
            if file.size == 0 || file.size > bound {
                return Err(archive_error());
            }
            let size = usize::try_from(file.size).map_err(|_| archive_error())?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(size).map_err(|_| archive_error())?;
            bytes.resize(size, 0);
            payload
                .read_exact(&mut bytes)
                .map_err(|_| archive_error())?;
            match path.as_str() {
                MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1 => {
                    semantic_release_bytes = Some(bytes);
                }
                MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1 => {
                    descriptor_bytes = Some(bytes);
                }
                MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1 => {
                    verification_lock_bytes = Some(bytes);
                }
                _ => unreachable!("capture bounds exist only for mandatory metadata"),
            }
            continue;
        }

        source_count = source_count.checked_add(1).ok_or_else(source_error)?;
        source_bytes = source_bytes
            .checked_add(file.size)
            .ok_or_else(source_error)?;
        let mut file_hasher = blake3::Hasher::new();
        let mut remaining = file.size;
        while remaining != 0 {
            let length = io_buffer
                .len()
                .min(usize::try_from(remaining).unwrap_or(usize::MAX));
            payload
                .read_exact(&mut io_buffer[..length])
                .map_err(|_| archive_error())?;
            file_hasher.update(&io_buffer[..length]);
            remaining -= u64::try_from(length).expect("bounded read length fits u64");
        }
        source_entries.push((path, file.size, *file_hasher.finalize().as_bytes()));
    }
    let mut trailing = [0_u8; 1];
    if payload.read(&mut trailing).map_err(|_| archive_error())? != 0
        || source_count != commitment.file_count
    {
        return Err(archive_error());
    }

    // `CarBuildPlan` uses structural component-vector ordering. The package commitment uses the
    // joined portable path bytes, which differ for pairs such as `a-` and `a/z`.
    let source_material =
        source_material(source_entries, source_material_capacity).map_err(|_| source_error())?;
    let source_digest =
        domain_digest(SOURCE_TREE_DOMAIN_V1, &source_material).map_err(|_| source_error())?;
    if source_digest != commitment.source_tree_digest {
        return Err(source_error());
    }

    let semantic_release_bytes = semantic_release_bytes.ok_or_else(archive_error)?;
    let descriptor_bytes = descriptor_bytes.ok_or_else(archive_error)?;
    let verification_lock_bytes = verification_lock_bytes.ok_or_else(archive_error)?;

    let mut descriptor_input = descriptor_bytes.as_slice();
    let descriptor = norito::with_decode_limits(DESCRIPTOR_DECODE_LIMITS_V1, || {
        MusubiArtifactDescriptorV1::decode(&mut descriptor_input)
    })
    .map_err(|_| descriptor_error())?;
    descriptor.validate().map_err(|_| descriptor_error())?;
    if !descriptor_input.is_empty()
        || descriptor.encode() != descriptor_bytes
        || descriptor.source_tree_digest != source_digest
        || descriptor.source_file_count != source_count
        || descriptor.source_bytes != source_bytes
    {
        return Err(descriptor_error());
    }

    let mut semantic_input = semantic_release_bytes.as_slice();
    let semantic_release = norito::with_decode_limits(SEMANTIC_DECODE_LIMITS_V1, || {
        MusubiSemanticReleaseManifestV1::decode(&mut semantic_input)
    })
    .map_err(|_| bundle_error())?;
    semantic_release.validate().map_err(|_| bundle_error())?;
    if !semantic_input.is_empty()
        || semantic_release.encode() != semantic_release_bytes
        || descriptor.semantic_release_manifest_digest != semantic_release.semantic_digest()
    {
        return Err(bundle_error());
    }

    let mut lock_input = verification_lock_bytes.as_slice();
    let verification_lock = norito::with_decode_limits(LOCK_DECODE_LIMITS_V1, || {
        MusubiVerificationLockV1::decode(&mut lock_input)
    })
    .map_err(|_| lock_error())?;
    verification_lock.validate().map_err(|_| lock_error())?;
    let verification_lock_digest = verification_lock.digest();
    if !lock_input.is_empty()
        || verification_lock.encode() != verification_lock_bytes
        || descriptor.verification_lock_digest != verification_lock_digest
    {
        return Err(lock_error());
    }
    semantic_release
        .validate_verification_lock(&verification_lock)
        .map_err(|_| lock_error())?;

    let descriptor_material =
        descriptor_material(&descriptor_bytes).map_err(|_| descriptor_error())?;
    if domain_digest(ARTIFACT_DESCRIPTOR_DOMAIN_V1, &descriptor_material)
        .map_err(|_| descriptor_error())?
        != commitment.descriptor_digest
    {
        return Err(descriptor_error());
    }

    let bundle_material_length = [
        BUNDLE_DOMAIN_V1.len(),
        semantic_release_bytes.len(),
        descriptor_material.len(),
        source_material.len(),
        verification_lock_bytes.len(),
    ]
    .into_iter()
    .try_fold(0_u64, |total, length| {
        total
            .checked_add(
                frame_length(u64::try_from(length).map_err(|_| bundle_error())?)
                    .ok_or_else(bundle_error)?,
            )
            .ok_or_else(bundle_error)
    })?;
    let mut bundle_hasher = blake3::Hasher::new();
    bundle_hasher.update(BUNDLE_DOMAIN_V1);
    bundle_hasher.update(&bundle_material_length.to_be_bytes());
    update_frame(&mut bundle_hasher, BUNDLE_DOMAIN_V1).map_err(|_| bundle_error())?;
    update_frame(&mut bundle_hasher, &semantic_release_bytes).map_err(|_| bundle_error())?;
    update_frame(&mut bundle_hasher, &descriptor_material).map_err(|_| bundle_error())?;
    update_frame(&mut bundle_hasher, &source_material).map_err(|_| bundle_error())?;
    update_frame(&mut bundle_hasher, &verification_lock_bytes).map_err(|_| bundle_error())?;
    if MusubiContentDigestV1::new(*bundle_hasher.finalize().as_bytes()) != commitment.bundle_digest
    {
        return Err(bundle_error());
    }

    Ok(ParsedBundlePayloadV1 {
        descriptor,
        semantic_release,
        verification_lock,
        source_file_count: source_count,
        source_bytes,
    })
}

fn payload_digest(mut payload: impl Read) -> std::io::Result<blake3::Hash> {
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; IO_BUFFER_BYTES];
    loop {
        let read = payload.read(&mut buffer)?;
        if read == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buffer[..read]);
    }
}

fn source_material(
    mut entries: Vec<(String, u64, [u8; 32])>,
    expected_length: usize,
) -> Result<Vec<u8>, ()> {
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    if entries.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(());
    }
    let count = u32::try_from(entries.len()).map_err(|_| ())?;
    let mut material = Vec::new();
    material
        .try_reserve_exact(expected_length)
        .map_err(|_| ())?;
    append_frame(&mut material, SOURCE_TREE_DOMAIN_V1)?;
    material.extend_from_slice(&count.to_be_bytes());
    for (path, size, digest) in entries {
        append_frame(&mut material, path.as_bytes())?;
        material.extend_from_slice(&size.to_be_bytes());
        material.extend_from_slice(&digest);
    }
    if material.len() != expected_length {
        return Err(());
    }
    Ok(material)
}

fn descriptor_material(bytes: &[u8]) -> Result<Vec<u8>, ()> {
    let capacity =
        frame_length(u64::try_from(ARTIFACT_DESCRIPTOR_DOMAIN_V1.len()).map_err(|_| ())?)
            .ok_or(())?
            .checked_add(frame_length(u64::try_from(bytes.len()).map_err(|_| ())?).ok_or(())?)
            .ok_or(())?;
    let mut material = Vec::new();
    material
        .try_reserve_exact(usize::try_from(capacity).map_err(|_| ())?)
        .map_err(|_| ())?;
    append_frame(&mut material, ARTIFACT_DESCRIPTOR_DOMAIN_V1)?;
    append_frame(&mut material, bytes)?;
    Ok(material)
}

fn domain_digest(domain: &[u8], material: &[u8]) -> Result<MusubiContentDigestV1, ()> {
    let length = u64::try_from(material.len()).map_err(|_| ())?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&length.to_be_bytes());
    hasher.update(material);
    Ok(MusubiContentDigestV1::new(*hasher.finalize().as_bytes()))
}

const fn frame_length(length: u64) -> Option<u64> {
    length.checked_add(8)
}

fn append_frame(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ()> {
    let length = u64::try_from(bytes.len()).map_err(|_| ())?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn update_frame(hasher: &mut blake3::Hasher, bytes: &[u8]) -> Result<(), ()> {
    let length = u64::try_from(bytes.len()).map_err(|_| ())?;
    hasher.update(&length.to_be_bytes());
    hasher.update(bytes);
    Ok(())
}

// TODO: Add a bounded streaming/seekable materialization entry point before provider readback or
// Musubi cache extraction uses this verifier. The current complete-`&[u8]` API is appropriate for
// seed ingress, whose bounded request body is already resident, but does not qualify the 64 MiB
// fetch-memory gate and must not be treated as production provider-readback/cache integration.

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiDependencyReqV1,
            MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiReleaseIdV1,
            MusubiReleaseMetadataV1, MusubiVerificationLockDigestV1,
        },
        nexus::DataSpaceId,
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
    };

    use super::*;
    use crate::{CarWriter, FileEntry, compute_por_root};

    #[derive(Clone, Copy)]
    enum FixtureFault {
        None,
        DescriptorSourceBinding,
        SemanticDependencyBinding,
        SemanticLockBinding,
    }

    struct BundleFixture {
        plan: CarBuildPlan,
        car: Vec<u8>,
        commitment: MusubiArchiveCommitmentV1,
        descriptor: MusubiArtifactDescriptorV1,
        semantic_release: MusubiSemanticReleaseManifestV1,
        verification_lock: MusubiVerificationLockV1,
    }

    fn bundle_fixture(fault: FixtureFault) -> BundleFixture {
        bundle_fixture_with_sources(
            fault,
            vec![(vec!["Musubi.toml".to_owned()], vec![b'm'; 4 * 1024])],
        )
    }

    fn bundle_fixture_with_sources(
        fault: FixtureFault,
        sources: Vec<(Vec<String>, Vec<u8>)>,
    ) -> BundleFixture {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "provider-fixture".parse().expect("fixture package name"),
        );
        let release = MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("fixture version"));
        let verification_lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let actual_lock_digest = verification_lock.digest();
        let semantic_lock_digest = if matches!(fault, FixtureFault::SemanticLockBinding) {
            MusubiVerificationLockDigestV1::new([0x72; 32])
        } else {
            actual_lock_digest
        };
        let semantic_release = MusubiSemanticReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0x70; 32]).expect("fixture ABI"),
            dependencies: if matches!(fault, FixtureFault::SemanticDependencyBinding) {
                vec![MusubiDependencyReqV1 {
                    alias: "dependency".parse().expect("fixture dependency alias"),
                    package: MusubiPackageIdV1::new(
                        DataSpaceId::new(8),
                        MusubiPackageScopeV1::DataspaceRoot,
                        "dependency".parse().expect("fixture dependency name"),
                    ),
                    requirement: "^1.0.0".parse().expect("fixture dependency requirement"),
                }]
            } else {
                Vec::new()
            },
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0x71; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            verification_lock_digest: semantic_lock_digest,
        };
        semantic_release
            .validate()
            .expect("fixture semantic release");
        verification_lock
            .validate()
            .expect("fixture verification lock");

        let mut source_entries = Vec::new();
        source_entries
            .try_reserve_exact(sources.len())
            .expect("bounded source transcript");
        let mut source_bytes = 0_u64;
        for (path, bytes) in &sources {
            let size = u64::try_from(bytes.len()).expect("fixture source length");
            source_bytes = source_bytes.checked_add(size).expect("source byte bound");
            source_entries.push((path.join("/"), size, *blake3::hash(bytes).as_bytes()));
        }
        let source_material_length = source_entries.iter().fold(
            8 + SOURCE_TREE_DOMAIN_V1.len() + 4,
            |total, (path, _, _)| total + 8 + path.len() + 8 + 32,
        );
        let source_material = source_material(source_entries, source_material_length)
            .expect("fixture source transcript");
        let source_tree_digest = domain_digest(SOURCE_TREE_DOMAIN_V1, &source_material)
            .expect("fixture source-tree digest");
        let descriptor_source_digest = if matches!(fault, FixtureFault::DescriptorSourceBinding) {
            MusubiContentDigestV1::new([0x73; 32])
        } else {
            source_tree_digest
        };
        let descriptor = MusubiArtifactDescriptorV1::new(
            semantic_release.semantic_digest(),
            descriptor_source_digest,
            actual_lock_digest,
            source_bytes,
            u32::try_from(sources.len()).expect("fixture source count"),
        )
        .expect("fixture descriptor");
        let semantic_release_bytes = semantic_release.encode();
        let descriptor_bytes = descriptor.encode();
        let verification_lock_bytes = verification_lock.encode();
        let descriptor_material =
            descriptor_material(&descriptor_bytes).expect("fixture descriptor transcript");
        let descriptor_digest = domain_digest(
            ARTIFACT_DESCRIPTOR_DOMAIN_V1,
            descriptor_material.as_slice(),
        )
        .expect("fixture descriptor digest");
        let mut bundle_material = Vec::new();
        for bytes in [
            BUNDLE_DOMAIN_V1,
            semantic_release_bytes.as_slice(),
            descriptor_material.as_slice(),
            source_material.as_slice(),
            verification_lock_bytes.as_slice(),
        ] {
            append_frame(&mut bundle_material, bytes).expect("fixture bundle frame");
        }
        let bundle_digest =
            domain_digest(BUNDLE_DOMAIN_V1, &bundle_material).expect("fixture bundle digest");

        let mut entries = sources
            .into_iter()
            .map(|(path, data)| FileEntry { path, data })
            .collect::<Vec<_>>();
        entries.extend([
            FileEntry {
                path: MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: semantic_release_bytes,
            },
            FileEntry {
                path: MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: descriptor_bytes,
            },
            FileEntry {
                path: MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                data: verification_lock_bytes,
            },
        ]);
        let (plan, payload) =
            CarBuildPlan::from_files(entries).expect("canonical fixture bundle plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("fixture CAR writer")
            .write_to(&mut car)
            .expect("canonical fixture CAR");
        let chunker = crate::chunker_registry::default_descriptor();
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::try_from(stats.root_cids[0].clone())
                .expect("fixture root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: chunker.id.0,
                namespace: chunker.namespace.to_owned(),
                name: chunker.name.to_owned(),
                semver: chunker.semver.to_owned(),
                multihash_code: chunker.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(
                &plan.chunks,
            )),
            por_root: MusubiContentDigestV1::new(
                compute_por_root(&payload, &plan).expect("fixture PoR"),
            ),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
            car_size: stats.car_size,
            bundle_digest,
            source_tree_digest,
            descriptor_digest,
            file_count: u32::try_from(sources_len(&plan)).expect("fixture source count"),
            chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count"),
        };
        commitment.validate().expect("fixture commitment");
        BundleFixture {
            plan,
            car,
            commitment,
            descriptor,
            semantic_release,
            verification_lock,
        }
    }

    fn sources_len(plan: &CarBuildPlan) -> usize {
        plan.files
            .iter()
            .filter(|file| !file.path.join("/").starts_with(".musubi/"))
            .count()
    }

    fn verify_error(
        fixture: &BundleFixture,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> MusubiBundleVerificationErrorV1 {
        MusubiBundleVerifierV1::verify(&fixture.plan, &fixture.car, commitment)
            .expect_err("substitution must fail")
    }

    #[test]
    fn verifies_complete_bundle_and_returns_typed_evidence() {
        let fixture = bundle_fixture(FixtureFault::None);
        let evidence =
            MusubiBundleVerifierV1::verify(&fixture.plan, &fixture.car, &fixture.commitment)
                .expect("valid complete bundle");
        assert_eq!(evidence.descriptor(), &fixture.descriptor);
        assert_eq!(evidence.semantic_release(), &fixture.semantic_release);
        assert_eq!(evidence.verification_lock(), &fixture.verification_lock);
        assert_eq!(evidence.source_file_count(), 1);
        assert_eq!(evidence.source_bytes(), 4 * 1024);
        assert_eq!(evidence.car_stats().car_size, fixture.commitment.car_size);
    }

    #[test]
    fn reports_closed_surfaces_for_commitment_substitutions() {
        let fixture = bundle_fixture(FixtureFault::None);

        let mut substituted_car = fixture.car.clone();
        let last = substituted_car
            .last_mut()
            .expect("canonical fixture CAR is nonempty");
        *last ^= 0x01;
        assert_eq!(
            MusubiBundleVerifierV1::verify(&fixture.plan, &substituted_car, &fixture.commitment,)
                .expect_err("CAR substitution must fail")
                .surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );

        let mut commitment = fixture.commitment.clone();
        commitment.por_root = MusubiContentDigestV1::new([0xa1; 32]);
        assert_eq!(
            verify_error(&fixture, &commitment).surface(),
            MusubiBundleIntegritySurfaceV1::ArchiveCommitment
        );

        let mut commitment = fixture.commitment.clone();
        commitment.source_tree_digest = MusubiContentDigestV1::new([0xa2; 32]);
        assert_eq!(
            verify_error(&fixture, &commitment).surface(),
            MusubiBundleIntegritySurfaceV1::SourceTree
        );

        let mut commitment = fixture.commitment.clone();
        commitment.descriptor_digest = MusubiContentDigestV1::new([0xa3; 32]);
        assert_eq!(
            verify_error(&fixture, &commitment).surface(),
            MusubiBundleIntegritySurfaceV1::Descriptor
        );

        let mut commitment = fixture.commitment.clone();
        commitment.bundle_digest = MusubiContentDigestV1::new([0xa4; 32]);
        assert_eq!(
            verify_error(&fixture, &commitment).surface(),
            MusubiBundleIntegritySurfaceV1::Bundle
        );
    }

    #[test]
    fn rejects_nested_descriptor_and_lock_substitutions_after_outer_rebinding() {
        let descriptor_fault = bundle_fixture(FixtureFault::DescriptorSourceBinding);
        assert_eq!(
            verify_error(&descriptor_fault, &descriptor_fault.commitment).surface(),
            MusubiBundleIntegritySurfaceV1::Descriptor
        );

        let lock_fault = bundle_fixture(FixtureFault::SemanticLockBinding);
        assert_eq!(
            verify_error(&lock_fault, &lock_fault.commitment).surface(),
            MusubiBundleIntegritySurfaceV1::VerificationLock
        );

        let dependency_fault = bundle_fixture(FixtureFault::SemanticDependencyBinding);
        assert_eq!(
            verify_error(&dependency_fault, &dependency_fault.commitment).surface(),
            MusubiBundleIntegritySurfaceV1::VerificationLock
        );
    }

    #[test]
    fn source_transcript_uses_joined_portable_path_order() {
        let fixture = bundle_fixture_with_sources(
            FixtureFault::None,
            vec![
                (vec!["a".to_owned(), "z".to_owned()], b"slash-z".to_vec()),
                (vec!["a-".to_owned()], b"dash".to_vec()),
            ],
        );
        let evidence =
            MusubiBundleVerifierV1::verify(&fixture.plan, &fixture.car, &fixture.commitment)
                .expect("joined-path transcript order");
        assert_eq!(evidence.source_file_count(), 2);
    }

    #[test]
    fn verification_error_is_payload_free() {
        let fixture = bundle_fixture(FixtureFault::None);
        let mut commitment = fixture.commitment.clone();
        commitment.bundle_digest = MusubiContentDigestV1::new([0xfe; 32]);
        let error = verify_error(&fixture, &commitment);
        assert_eq!(error.surface(), MusubiBundleIntegritySurfaceV1::Bundle);
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains("Musubi.toml"));
        assert!(!display.contains("Musubi.toml"));
        assert!(!debug.contains("provider-fixture"));
        assert!(!display.contains("provider-fixture"));
    }
}
