//! Authenticated, source-backed Offline Cash V1 Halo2 artifact boundary.

use std::{
    collections::BTreeSet,
    fmt,
    fs::File,
    io::{self, Read, Seek as _, SeekFrom, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

use halo2_proofs::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    halo2curves::{
        CurveAffine,
        ff::{FromUniformBytes, PrimeField as _},
        pasta::{EpAffine, EqAffine},
    },
    plonk::{Circuit, ProvingKey, VerifyingKey},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::offline::{
    OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1, OFFLINE_CASH_HALO2_K_V1, OfflineCashArtifactBindingV1,
    OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1,
};
use sha2::{Digest as _, Sha256};

use super::halo2_primitives::{parse_params_exact_for_k, parse_processed_verifier_key_v1};
use super::protocol::{
    OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
    offline_cash_artifact_length_bounds_v1, offline_cash_artifact_protocol_v1,
    offline_cash_halo2_profile_digest_v1, offline_cash_halo2_protocol_identity_v1,
};

const ARTIFACT_COUNT: usize = OfflineCashArtifactRoleV1::ALL.len();
const AUTHENTICATION_BUFFER_BYTES: usize = 64 * 1024;

/// Admission failure for a pinned Offline Cash V1 artifact file set.
///
/// This error deliberately does not expose paths or operating-system error
/// strings. Runtime callers can fail closed without accidentally forwarding
/// local custody details across an FFI boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashArtifactFileSetErrorV1 {
    /// The authenticated release does not select the compiled Offline Cash V1 profile.
    ReleaseMismatch,
    /// The file inventory is incomplete, duplicated, or not in canonical role order.
    InvalidInventory,
    /// A supplied handle does not identify a regular file.
    NotRegularFile,
    /// A supplied file's exact byte length differs from its authenticated binding.
    LengthMismatch,
    /// A file could not be inspected, rewound, locked, or read.
    FileAccess,
    /// Exact file bytes differ from the threshold-authenticated SHA-256 binding.
    DigestMismatch,
}

impl fmt::Display for OfflineCashArtifactFileSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ReleaseMismatch => "offline-cash artifact release mismatch",
            Self::InvalidInventory => "invalid offline-cash artifact file inventory",
            Self::NotRegularFile => "offline-cash artifact handle is not a regular file",
            Self::LengthMismatch => "offline-cash artifact file length mismatch",
            Self::FileAccess => "offline-cash artifact file access failure",
            Self::DigestMismatch => "offline-cash artifact file digest mismatch",
        })
    }
}

impl std::error::Error for OfflineCashArtifactFileSetErrorV1 {}

/// Pinned, complete file source for one threshold-authenticated Offline Cash V1 release.
///
/// Construction consumes exactly 34 already-open handles in canonical
/// [`OfflineCashArtifactRoleV1::ALL`] order. Core verifies regular-file type,
/// exact length, and every byte's SHA-256 before returning. Each later parser
/// access rewinds and reauthenticates the selected handle, so an external file
/// mutation cannot reuse the successful installation pass.
///
/// The type grants artifact custody only. It does not enable Offline Cash
/// credit authority or weaken the separate production/testnet activation gate.
pub struct OfflineCashAuthenticatedArtifactFileSetV1 {
    authenticated_release: Arc<OfflineCashAuthenticatedReleaseV1>,
    files: Vec<Arc<Mutex<File>>>,
}

impl fmt::Debug for OfflineCashAuthenticatedArtifactFileSetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashAuthenticatedArtifactFileSetV1")
            .field("release_id", &self.authenticated_release.release_id())
            .field(
                "manifest_digest",
                &self.authenticated_release.manifest_digest(),
            )
            .field("artifact_count", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl OfflineCashAuthenticatedArtifactFileSetV1 {
    /// Consume, pin, and authenticate the complete canonical file inventory.
    ///
    /// # Errors
    ///
    /// Returns an error before publishing the source if the authenticated
    /// release differs from the compiled profile, any role is missing or out
    /// of order, a handle is not a regular file, or any length/digest/access
    /// check fails.
    pub fn new(
        authenticated_release: OfflineCashAuthenticatedReleaseV1,
        files: Vec<(OfflineCashArtifactRoleV1, File)>,
    ) -> Result<Self, OfflineCashArtifactFileSetErrorV1> {
        let manifest =
            OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(&authenticated_release)
                .map_err(|_| OfflineCashArtifactFileSetErrorV1::ReleaseMismatch)?;
        if files.len() != ARTIFACT_COUNT {
            return Err(OfflineCashArtifactFileSetErrorV1::InvalidInventory);
        }

        let mut pinned = Vec::with_capacity(ARTIFACT_COUNT);
        for ((observed_role, mut file), expected_role) in files
            .into_iter()
            .zip(OfflineCashArtifactRoleV1::ALL.iter().copied())
        {
            if observed_role != expected_role {
                return Err(OfflineCashArtifactFileSetErrorV1::InvalidInventory);
            }
            let metadata = file
                .metadata()
                .map_err(|_| OfflineCashArtifactFileSetErrorV1::FileAccess)?;
            if !metadata.file_type().is_file() {
                return Err(OfflineCashArtifactFileSetErrorV1::NotRegularFile);
            }
            if metadata.len() != manifest.artifact(expected_role).byte_len {
                return Err(OfflineCashArtifactFileSetErrorV1::LengthMismatch);
            }
            file.seek(SeekFrom::Start(0))
                .map_err(|_| OfflineCashArtifactFileSetErrorV1::FileAccess)?;
            pinned.push(Arc::new(Mutex::new(file)));
        }

        let source = Self {
            authenticated_release: Arc::new(authenticated_release),
            files: pinned,
        };
        for role in OfflineCashArtifactRoleV1::ALL {
            let expected = manifest.artifact(role);
            authenticate_source_artifact(&source, expected).map_err(|error| match error {
                OfflineCashHalo2ArtifactErrorV1::LengthMismatch => {
                    OfflineCashArtifactFileSetErrorV1::LengthMismatch
                }
                OfflineCashHalo2ArtifactErrorV1::DigestMismatch => {
                    OfflineCashArtifactFileSetErrorV1::DigestMismatch
                }
                _ => OfflineCashArtifactFileSetErrorV1::FileAccess,
            })?;
        }
        Ok(source)
    }

    /// Threshold-authenticated release identifier served by this file set.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.authenticated_release.release_id()
    }

    /// Digest of the complete threshold-authenticated release manifest.
    #[must_use]
    pub fn manifest_digest(&self) -> [u8; 32] {
        self.authenticated_release.manifest_digest()
    }

    /// Exact number of pinned governed artifact roles.
    #[must_use]
    pub fn artifact_count(&self) -> usize {
        self.files.len()
    }

    /// Core-private release object retained with the pinned handles.
    pub(super) fn authenticated_release_arc(&self) -> Arc<OfflineCashAuthenticatedReleaseV1> {
        Arc::clone(&self.authenticated_release)
    }

    /// Core-private release reference retained with the pinned handles.
    pub(super) fn authenticated_release(&self) -> &OfflineCashAuthenticatedReleaseV1 {
        &self.authenticated_release
    }
}

/// Fail-closed artifact/profile admission error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineCashHalo2ArtifactErrorV1 {
    /// Authenticated release selected a different compiled profile.
    ProfileMismatch,
    /// Authenticated release selected a different STATE protocol.
    ProtocolMismatch,
    /// Authenticated artifact metadata was malformed or internally ambiguous.
    InvalidManifest,
    /// Artifact source failed before or after its parser callback.
    SourceFailure,
    /// Artifact source omitted, repeated, or swallowed its callback result.
    SourceContractViolation,
    /// Exact artifact byte length differed from the authenticated binding.
    LengthMismatch,
    /// Exact artifact bytes differed from the authenticated SHA-256 identity.
    DigestMismatch,
    /// Authenticated transparent Pasta parameters were not canonical for the compiled profile.
    InvalidParameterArtifact,
    /// Authenticated processed verifier-key bytes did not encode the compiled STATE circuit.
    InvalidVerifierKeyArtifact,
    /// Authenticated processed proving-key bytes did not encode the selected compiled circuit.
    InvalidProvingKeyArtifact,
}

impl fmt::Display for OfflineCashHalo2ArtifactErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ProfileMismatch => "offline-cash Halo2 profile mismatch",
            Self::ProtocolMismatch => "offline-cash Halo2 protocol mismatch",
            Self::InvalidManifest => "invalid offline-cash Halo2 artifact manifest",
            Self::SourceFailure => "offline-cash Halo2 artifact source failure",
            Self::SourceContractViolation => {
                "offline-cash Halo2 artifact source callback contract violation"
            }
            Self::LengthMismatch => "offline-cash Halo2 artifact length mismatch",
            Self::DigestMismatch => "offline-cash Halo2 artifact digest mismatch",
            Self::InvalidParameterArtifact => "invalid offline-cash transparent parameter artifact",
            Self::InvalidVerifierKeyArtifact => {
                "invalid offline-cash processed verifier-key artifact"
            }
            Self::InvalidProvingKeyArtifact => {
                "invalid offline-cash processed proving-key artifact"
            }
        })
    }
}

impl std::error::Error for OfflineCashHalo2ArtifactErrorV1 {}

/// Immutable exact artifact plan derived only from an authenticated release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OfflineCashHalo2ArtifactManifestV1 {
    release_id: [u8; 32],
    release_manifest_digest: [u8; 32],
    profile_digest: [u8; 32],
    eq_state_protocol_digest: [u8; 32],
    ep_state_protocol_digest: [u8; 32],
    artifacts: [OfflineCashArtifactBindingV1; ARTIFACT_COUNT],
}

impl OfflineCashHalo2ArtifactManifestV1 {
    /// Derive a strict finite manifest from an already authenticated release.
    pub(crate) fn from_authenticated_release(
        release: &OfflineCashAuthenticatedReleaseV1,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        let profile_digest = offline_cash_halo2_profile_digest_v1();
        if release.profile_digest() != profile_digest {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProfileMismatch);
        }
        let eq_state_protocol_digest = offline_cash_halo2_protocol_identity_v1(
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::State,
        )
        .digest();
        let ep_state_protocol_digest = offline_cash_halo2_protocol_identity_v1(
            OfflineCashHalo2ParityV1::Ep,
            OfflineCashHalo2CircuitRoleV1::State,
        )
        .digest();
        if release.eq_protocol_digest() != eq_state_protocol_digest
            || release.ep_protocol_digest() != ep_state_protocol_digest
            || eq_state_protocol_digest == ep_state_protocol_digest
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }

        let artifacts =
            std::array::from_fn(|index| release.artifact(OfflineCashArtifactRoleV1::ALL[index]));
        let mut digests = BTreeSet::new();
        let mut total = 0_u64;
        for (binding, expected_role) in artifacts
            .iter()
            .zip(OfflineCashArtifactRoleV1::ALL.iter().copied())
        {
            let (minimum, maximum) = offline_cash_artifact_length_bounds_v1(expected_role);
            if binding.role != expected_role
                || binding.sha256 == [0; 32]
                || binding.byte_len < minimum
                || binding.byte_len > maximum
                || !digests.insert(binding.sha256)
            {
                return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
            }
            total = total
                .checked_add(binding.byte_len)
                .ok_or(OfflineCashHalo2ArtifactErrorV1::InvalidManifest)?;
        }
        if total > OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1 {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        Ok(Self {
            release_id: release.release_id(),
            release_manifest_digest: release.manifest_digest(),
            profile_digest,
            eq_state_protocol_digest,
            ep_state_protocol_digest,
            artifacts,
        })
    }

    /// Exact authenticated binding for one finite artifact role.
    pub(crate) fn artifact(&self, role: OfflineCashArtifactRoleV1) -> OfflineCashArtifactBindingV1 {
        let index = OfflineCashArtifactRoleV1::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .expect("offline-cash artifact role belongs to the finite inventory");
        self.artifacts[index]
    }

    /// Exact STATE protocol selected for one Pasta parity.
    pub(crate) const fn state_protocol_digest(&self, parity: OfflineCashHalo2ParityV1) -> [u8; 32] {
        match parity {
            OfflineCashHalo2ParityV1::Eq => self.eq_state_protocol_digest,
            OfflineCashHalo2ParityV1::Ep => self.ep_state_protocol_digest,
        }
    }

    /// Exact compiled protocol for any finite role and parity.
    pub(crate) fn protocol_digest(
        &self,
        parity: OfflineCashHalo2ParityV1,
        role: OfflineCashHalo2CircuitRoleV1,
    ) -> [u8; 32] {
        if role == OfflineCashHalo2CircuitRoleV1::State {
            self.state_protocol_digest(parity)
        } else {
            offline_cash_halo2_protocol_identity_v1(parity, role).digest()
        }
    }

    fn matches_release(&self, release: &OfflineCashAuthenticatedReleaseV1) -> bool {
        release.release_id() == self.release_id
            && release.manifest_digest() == self.release_manifest_digest
            && release.profile_digest() == self.profile_digest
            && release.eq_protocol_digest() == self.eq_state_protocol_digest
            && release.ep_protocol_digest() == self.ep_state_protocol_digest
            && OfflineCashArtifactRoleV1::ALL
                .iter()
                .copied()
                .all(|role| release.artifact(role) == self.artifact(role))
    }
}

/// Private sealing namespace for trusted Core artifact providers.
pub(super) mod sealed {
    /// Prevent downstream crates from supplying governed artifact bytes.
    pub trait Sealed {}
}

/// Core-owned source of immutable artifacts for one authenticated release.
///
/// The source must invoke `consume` exactly once and propagate its result.  It
/// performs no network or environment lookup through this interface.
pub(crate) trait OfflineCashHalo2ArtifactSourceV1: sealed::Sealed + Send + Sync {
    /// Release whose exact artifact bindings this source serves.
    fn authenticated_release(&self) -> &OfflineCashAuthenticatedReleaseV1;

    /// Lend one exact role's bytes to the Core-owned authenticator.
    fn with_artifact(
        &self,
        role: OfflineCashArtifactRoleV1,
        consume: &mut dyn FnMut(&mut dyn Read) -> Result<(), String>,
    ) -> Result<(), String>;
}

impl sealed::Sealed for OfflineCashAuthenticatedArtifactFileSetV1 {}

impl OfflineCashHalo2ArtifactSourceV1 for OfflineCashAuthenticatedArtifactFileSetV1 {
    fn authenticated_release(&self) -> &OfflineCashAuthenticatedReleaseV1 {
        &self.authenticated_release
    }

    fn with_artifact(
        &self,
        role: OfflineCashArtifactRoleV1,
        consume: &mut dyn FnMut(&mut dyn Read) -> Result<(), String>,
    ) -> Result<(), String> {
        let index = OfflineCashArtifactRoleV1::ALL
            .iter()
            .position(|candidate| *candidate == role)
            .ok_or_else(|| {
                "offline-cash artifact role is outside the finite inventory".to_owned()
            })?;
        let file = self
            .files
            .get(index)
            .ok_or_else(|| "offline-cash artifact file is missing".to_owned())?;
        let mut file = file
            .lock()
            .map_err(|_| "offline-cash artifact file lock is poisoned".to_owned())?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| "offline-cash artifact file rewind failed".to_owned())?;
        consume(&mut *file)
    }
}

fn authenticate_reader(
    reader: &mut dyn Read,
    expected: OfflineCashArtifactBindingV1,
) -> Result<(), OfflineCashHalo2ArtifactErrorV1> {
    let mut hasher = Sha256::new();
    let mut remaining = expected.byte_len;
    let mut buffer = [0_u8; AUTHENTICATION_BUFFER_BYTES];
    while remaining != 0 {
        let requested = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded artifact chunk fits usize");
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?;
        if read == 0 {
            return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
        }
        hasher.update(&buffer[..read]);
        remaining -= u64::try_from(read).expect("read length fits u64");
    }
    let mut excess = [0_u8; 1];
    if reader
        .read(&mut excess)
        .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?
        != 0
    {
        return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
    }
    if <[u8; 32]>::from(hasher.finalize()) != expected.sha256 {
        return Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch);
    }
    Ok(())
}

fn authenticate_source_artifact(
    source: &dyn OfflineCashHalo2ArtifactSourceV1,
    expected: OfflineCashArtifactBindingV1,
) -> Result<(), OfflineCashHalo2ArtifactErrorV1> {
    let mut callback_count = 0_u8;
    let mut outcome = None;
    let source_result = source.with_artifact(expected.role, &mut |reader| {
        callback_count = callback_count.saturating_add(1);
        if callback_count != 1 {
            return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation.to_string());
        }
        let result = authenticate_reader(reader, expected);
        let callback_result = result.map_err(|error| error.to_string());
        outcome = Some(result);
        callback_result
    });
    if callback_count == 0 {
        return if source_result.is_err() {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceFailure)
        } else {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation)
        };
    }
    if callback_count != 1 {
        return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation);
    }
    match outcome {
        Some(Err(error)) => Err(error),
        Some(Ok(())) => source_result.map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure),
        None => Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation),
    }
}

fn read_authenticated_source_artifact(
    source: &dyn OfflineCashHalo2ArtifactSourceV1,
    expected: OfflineCashArtifactBindingV1,
) -> Result<Vec<u8>, OfflineCashHalo2ArtifactErrorV1> {
    let expected_len = usize::try_from(expected.byte_len)
        .map_err(|_| OfflineCashHalo2ArtifactErrorV1::LengthMismatch)?;
    let mut callback_count = 0_u8;
    let mut outcome = None;
    let source_result = source.with_artifact(expected.role, &mut |reader| {
        callback_count = callback_count.saturating_add(1);
        if callback_count != 1 {
            return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation.to_string());
        }
        let result = (|| {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(expected_len)
                .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?;
            bytes.resize(expected_len, 0);
            let mut offset = 0_usize;
            while offset != expected_len {
                let read = reader
                    .read(&mut bytes[offset..])
                    .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?;
                if read == 0 {
                    return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
                }
                offset = offset
                    .checked_add(read)
                    .ok_or(OfflineCashHalo2ArtifactErrorV1::LengthMismatch)?;
            }
            let mut excess = [0_u8; 1];
            if reader
                .read(&mut excess)
                .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?
                != 0
            {
                return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
            }
            if <[u8; 32]>::from(Sha256::digest(&bytes)) != expected.sha256 {
                return Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch);
            }
            Ok(bytes)
        })();
        let callback_result = result.as_ref().map(|_| ()).map_err(ToString::to_string);
        outcome = Some(result);
        callback_result
    });
    if callback_count == 0 {
        return if source_result.is_err() {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceFailure)
        } else {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation)
        };
    }
    if callback_count != 1 {
        return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation);
    }
    match outcome {
        Some(Err(error)) => Err(error),
        Some(Ok(bytes)) => source_result
            .map(|()| bytes)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure),
        None => Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation),
    }
}

fn consume_source_artifact_once<T>(
    source: &dyn OfflineCashHalo2ArtifactSourceV1,
    role: OfflineCashArtifactRoleV1,
    mut consume: impl FnMut(&mut dyn Read) -> Result<T, OfflineCashHalo2ArtifactErrorV1>,
) -> Result<T, OfflineCashHalo2ArtifactErrorV1> {
    let mut callback_count = 0_u8;
    let mut outcome = None;
    let source_result = source.with_artifact(role, &mut |reader| {
        callback_count = callback_count.saturating_add(1);
        if callback_count != 1 {
            return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation.to_string());
        }
        let result = consume(reader);
        let callback_result = result.as_ref().map(|_| ()).map_err(ToString::to_string);
        outcome = Some(result);
        callback_result
    });
    if callback_count == 0 {
        return if source_result.is_err() {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceFailure)
        } else {
            Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation)
        };
    }
    if callback_count != 1 {
        return Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation);
    }
    match outcome {
        Some(Err(error)) => Err(error),
        Some(Ok(value)) => source_result
            .map(|()| value)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure),
        None => Err(OfflineCashHalo2ArtifactErrorV1::SourceContractViolation),
    }
}

#[derive(Debug)]
struct RequiredProvingKeyBytesV1 {
    offset: u64,
    bytes: Vec<u8>,
}

/// Exact processed-PK framing expected by the vendor reader.
///
/// The upstream reader uses infallible length-prefixed polynomial allocation.
/// This plan pins every allocation-driving word before those bytes reach it.
#[derive(Debug)]
struct ProcessedProvingKeyShapeV1 {
    exact_byte_len: u64,
    required: Vec<RequiredProvingKeyBytesV1>,
}

impl ProcessedProvingKeyShapeV1 {
    fn for_verifier<C: CurveAffine>(
        verifier_bytes: &[u8],
        verifier: &VerifyingKey<C>,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1>
    where
        C::ScalarExt: SerdePrimeField,
    {
        if verifier.get_domain().k() != OFFLINE_CASH_HALO2_K_V1 {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact);
        }
        let scalar_bytes = C::ScalarExt::default().to_repr().as_ref().len();
        Self::from_geometry(
            verifier_bytes,
            OFFLINE_CASH_HALO2_K_V1,
            scalar_bytes,
            verifier.fixed_commitments().len(),
            verifier.permutation().commitments().len(),
        )
    }

    fn from_geometry(
        verifier_bytes: &[u8],
        k: u32,
        scalar_bytes: usize,
        fixed_polynomials: usize,
        permutation_polynomials: usize,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        if k != OFFLINE_CASH_HALO2_K_V1 || scalar_bytes != 32 || verifier_bytes.is_empty() {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact);
        }
        let polynomial_len = 1_u64
            .checked_shl(k)
            .ok_or(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;
        let polynomial_len_u32 = u32::try_from(polynomial_len)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;
        let polynomial_payload_bytes = polynomial_len
            .checked_mul(
                u64::try_from(scalar_bytes)
                    .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?,
            )
            .ok_or(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;

        let mut required = vec![RequiredProvingKeyBytesV1 {
            offset: 0,
            bytes: verifier_bytes.to_vec(),
        }];
        let mut offset = u64::try_from(verifier_bytes.len())
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;

        let append_polynomial = |required: &mut Vec<RequiredProvingKeyBytesV1>,
                                 offset: &mut u64| {
            required.push(RequiredProvingKeyBytesV1 {
                offset: *offset,
                bytes: polynomial_len_u32.to_be_bytes().to_vec(),
            });
            *offset = offset
                .checked_add(4)
                .and_then(|value| value.checked_add(polynomial_payload_bytes))
                .ok_or(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;
            Ok::<(), OfflineCashHalo2ArtifactErrorV1>(())
        };
        for _ in 0..3 {
            append_polynomial(&mut required, &mut offset)?;
        }

        let append_polynomial_vector = |required: &mut Vec<RequiredProvingKeyBytesV1>,
                                        offset: &mut u64,
                                        count: usize|
         -> Result<(), OfflineCashHalo2ArtifactErrorV1> {
            let count = u32::try_from(count)
                .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;
            required.push(RequiredProvingKeyBytesV1 {
                offset: *offset,
                bytes: count.to_be_bytes().to_vec(),
            });
            *offset = offset
                .checked_add(4)
                .ok_or(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)?;
            for _ in 0..count {
                append_polynomial(required, offset)?;
            }
            Ok(())
        };
        for _ in 0..2 {
            append_polynomial_vector(&mut required, &mut offset, fixed_polynomials)?;
        }
        for _ in 0..2 {
            append_polynomial_vector(&mut required, &mut offset, permutation_polynomials)?;
        }

        Ok(Self {
            exact_byte_len: offset,
            required,
        })
    }
}

/// Bounded authenticated reader that also pins every allocation-driving PK byte.
struct AuthenticatedProvingKeyReaderV1<'a> {
    inner: &'a mut dyn Read,
    expected: OfflineCashArtifactBindingV1,
    shape: &'a ProcessedProvingKeyShapeV1,
    hasher: Sha256,
    remaining: u64,
    offset: u64,
    next_required: usize,
    shape_mismatch: bool,
}

impl<'a> AuthenticatedProvingKeyReaderV1<'a> {
    fn new(
        inner: &'a mut dyn Read,
        expected: OfflineCashArtifactBindingV1,
        shape: &'a ProcessedProvingKeyShapeV1,
    ) -> Self {
        Self {
            inner,
            expected,
            shape,
            hasher: Sha256::new(),
            remaining: expected.byte_len,
            offset: 0,
            next_required: 0,
            shape_mismatch: false,
        }
    }

    fn validate_required_bytes(&mut self, start: u64, bytes: &[u8]) -> bool {
        let end = start.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let mut index = self.next_required;
        while let Some(required) = self.shape.required.get(index) {
            let required_end = required
                .offset
                .saturating_add(u64::try_from(required.bytes.len()).unwrap_or(u64::MAX));
            if required_end <= start {
                index += 1;
                continue;
            }
            if required.offset >= end {
                break;
            }
            let overlap_start = start.max(required.offset);
            let overlap_end = end.min(required_end);
            let observed_start = usize::try_from(overlap_start - start).ok();
            let observed_end = usize::try_from(overlap_end - start).ok();
            let required_start = usize::try_from(overlap_start - required.offset).ok();
            let required_end_index = usize::try_from(overlap_end - required.offset).ok();
            if observed_start
                .zip(observed_end)
                .zip(required_start.zip(required_end_index))
                .is_none_or(
                    |((observed_start, observed_end), (required_start, required_end))| {
                        bytes.get(observed_start..observed_end)
                            != required.bytes.get(required_start..required_end)
                    },
                )
            {
                self.shape_mismatch = true;
                return false;
            }
            if required_end <= end {
                index += 1;
            } else {
                break;
            }
        }
        self.next_required = index;
        true
    }

    fn finish_authentication(mut self) -> Result<bool, OfflineCashHalo2ArtifactErrorV1> {
        let parser_consumed_exact = self.remaining == 0
            && self.offset == self.shape.exact_byte_len
            && self.next_required == self.shape.required.len()
            && !self.shape_mismatch;
        let mut buffer = [0_u8; AUTHENTICATION_BUFFER_BYTES];
        while self.remaining != 0 {
            let requested = usize::try_from(self.remaining.min(buffer.len() as u64))
                .expect("bounded artifact chunk fits usize");
            let read = self
                .inner
                .read(&mut buffer[..requested])
                .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?;
            if read == 0 {
                return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
            }
            self.hasher.update(&buffer[..read]);
            self.remaining -= u64::try_from(read).expect("read length fits u64");
            self.offset = self
                .offset
                .checked_add(u64::try_from(read).expect("read length fits u64"))
                .ok_or(OfflineCashHalo2ArtifactErrorV1::LengthMismatch)?;
        }
        let mut excess = [0_u8; 1];
        if self
            .inner
            .read(&mut excess)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?
            != 0
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
        }
        if <[u8; 32]>::from(self.hasher.finalize()) != self.expected.sha256 {
            return Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch);
        }
        Ok(parser_consumed_exact)
    }
}

impl Read for AuthenticatedProvingKeyReaderV1<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 || buffer.is_empty() {
            return Ok(0);
        }
        let requested = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("bounded proving-key read fits usize");
        let read = self.inner.read(&mut buffer[..requested])?;
        if read == 0 {
            return Ok(0);
        }
        let start = self.offset;
        self.hasher.update(&buffer[..read]);
        self.remaining -= u64::try_from(read).expect("read length fits u64");
        self.offset = self
            .offset
            .checked_add(u64::try_from(read).expect("read length fits u64"))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PK offset overflow"))?;
        if !self.validate_required_bytes(start, &buffer[..read]) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "processed proving-key framing differs from authenticated plan",
            ));
        }
        Ok(read)
    }
}

struct AuthenticatedArtifactComparisonWriterV1<'a> {
    inner: &'a mut dyn Read,
    expected: OfflineCashArtifactBindingV1,
    hasher: Sha256,
    remaining: u64,
    exact_match: bool,
}

impl<'a> AuthenticatedArtifactComparisonWriterV1<'a> {
    fn new(inner: &'a mut dyn Read, expected: OfflineCashArtifactBindingV1) -> Self {
        Self {
            inner,
            expected,
            hasher: Sha256::new(),
            remaining: expected.byte_len,
            exact_match: true,
        }
    }

    fn finish_authentication(mut self) -> Result<bool, OfflineCashHalo2ArtifactErrorV1> {
        let serialization_consumed_exact = self.remaining == 0 && self.exact_match;
        let mut buffer = [0_u8; AUTHENTICATION_BUFFER_BYTES];
        while self.remaining != 0 {
            let requested = usize::try_from(self.remaining.min(buffer.len() as u64))
                .expect("bounded artifact chunk fits usize");
            let read = self
                .inner
                .read(&mut buffer[..requested])
                .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?;
            if read == 0 {
                return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
            }
            self.hasher.update(&buffer[..read]);
            self.remaining -= u64::try_from(read).expect("read length fits u64");
        }
        let mut excess = [0_u8; 1];
        if self
            .inner
            .read(&mut excess)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::SourceFailure)?
            != 0
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::LengthMismatch);
        }
        if <[u8; 32]>::from(self.hasher.finalize()) != self.expected.sha256 {
            return Err(OfflineCashHalo2ArtifactErrorV1::DigestMismatch);
        }
        Ok(serialization_consumed_exact)
    }
}

impl Write for AuthenticatedArtifactComparisonWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > self.remaining {
            self.exact_match = false;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "serialization exceeds authenticated artifact length",
            ));
        }
        let mut offset = 0_usize;
        let mut buffer = [0_u8; AUTHENTICATION_BUFFER_BYTES];
        while offset != bytes.len() {
            let chunk_len = (bytes.len() - offset).min(buffer.len());
            let mut read_offset = 0_usize;
            while read_offset != chunk_len {
                let read = self.inner.read(&mut buffer[read_offset..chunk_len])?;
                if read == 0 {
                    self.exact_match = false;
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "authenticated artifact ended during canonical comparison",
                    ));
                }
                read_offset += read;
            }
            self.hasher.update(&buffer[..chunk_len]);
            self.remaining -= u64::try_from(chunk_len).expect("chunk length fits u64");
            if buffer[..chunk_len] != bytes[offset..offset + chunk_len] {
                self.exact_match = false;
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "canonical serialization differs from authenticated artifact",
                ));
            }
            offset += chunk_len;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Compile-time identity of one concrete governed proving circuit.
pub(super) trait OfflineCashProvingCircuitV1<C>: Circuit<C::ScalarExt>
where
    C: CurveAffine,
{
    /// Pasta parity fixed by the circuit's scalar field.
    const PARITY: OfflineCashHalo2ParityV1;
    /// Governed relation role implemented by the circuit.
    const ROLE: OfflineCashHalo2CircuitRoleV1;
}

macro_rules! impl_offline_cash_proving_circuit_v1 {
    ($curve:ty, $circuit:ty, $parity:expr, $role:expr) => {
        impl OfflineCashProvingCircuitV1<$curve> for $circuit {
            const PARITY: OfflineCashHalo2ParityV1 = $parity;
            const ROLE: OfflineCashHalo2CircuitRoleV1 = $role;
        }
    };
}

impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::state_recursion::OfflineCashEqStateCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::State
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::state_recursion::OfflineCashEpStateCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::State
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::helper_circuit::OfflineCashEqGuardUseBindingCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::helper_circuit::OfflineCashEpGuardUseBindingCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardUse
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::helper_circuit::OfflineCashEqPlatformBindBindingCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::helper_circuit::OfflineCashEpPlatformBindBindingCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::PlatformBind
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::helper_circuit::OfflineCashEqAndroidKeyCertBindingCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::helper_circuit::OfflineCashEpAndroidKeyCertBindingCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::guard_bundle_recursion::OfflineCashEqGuardBundleCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardBundle
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::guard_bundle_recursion::OfflineCashEpGuardBundleCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardBundle
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::P256PackedAffineEqChildCircuitV3,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::P256V3
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::P256PackedAffineEpChildCircuitV3,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::P256V3
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::state_circuit::OfflineCashEqStateLeafCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::StateLeaf
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::state_circuit::OfflineCashEpStateLeafCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::StateLeaf
);
impl_offline_cash_proving_circuit_v1!(
    EqAffine,
    super::helper_circuit::OfflineCashEqGuardBundleLeafBindingCircuitV1,
    OfflineCashHalo2ParityV1::Eq,
    OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
);
impl_offline_cash_proving_circuit_v1!(
    EpAffine,
    super::helper_circuit::OfflineCashEpGuardBundleLeafBindingCircuitV1,
    OfflineCashHalo2ParityV1::Ep,
    OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf
);

const fn proving_artifact_roles_v1(
    parity: OfflineCashHalo2ParityV1,
    role: OfflineCashHalo2CircuitRoleV1,
) -> (
    OfflineCashArtifactRoleV1,
    OfflineCashArtifactRoleV1,
    OfflineCashArtifactRoleV1,
) {
    use OfflineCashArtifactRoleV1 as Artifact;
    use OfflineCashHalo2CircuitRoleV1 as CircuitRole;
    use OfflineCashHalo2ParityV1 as Parity;
    match (parity, role) {
        (Parity::Eq, CircuitRole::State) => {
            (Artifact::ParamsEq, Artifact::StatePkEq, Artifact::StateVkEq)
        }
        (Parity::Ep, CircuitRole::State) => {
            (Artifact::ParamsEp, Artifact::StatePkEp, Artifact::StateVkEp)
        }
        (Parity::Eq, CircuitRole::GuardUse) => (
            Artifact::ParamsEq,
            Artifact::GuardUsePkEq,
            Artifact::GuardUseVkEq,
        ),
        (Parity::Ep, CircuitRole::GuardUse) => (
            Artifact::ParamsEp,
            Artifact::GuardUsePkEp,
            Artifact::GuardUseVkEp,
        ),
        (Parity::Eq, CircuitRole::PlatformBind) => (
            Artifact::ParamsEq,
            Artifact::PlatformBindPkEq,
            Artifact::PlatformBindVkEq,
        ),
        (Parity::Ep, CircuitRole::PlatformBind) => (
            Artifact::ParamsEp,
            Artifact::PlatformBindPkEp,
            Artifact::PlatformBindVkEp,
        ),
        (Parity::Eq, CircuitRole::AndroidKeyCert) => (
            Artifact::ParamsEq,
            Artifact::AndroidKeyCertPkEq,
            Artifact::AndroidKeyCertVkEq,
        ),
        (Parity::Ep, CircuitRole::AndroidKeyCert) => (
            Artifact::ParamsEp,
            Artifact::AndroidKeyCertPkEp,
            Artifact::AndroidKeyCertVkEp,
        ),
        (Parity::Eq, CircuitRole::GuardBundle) => (
            Artifact::ParamsEq,
            Artifact::GuardBundlePkEq,
            Artifact::GuardBundleVkEq,
        ),
        (Parity::Ep, CircuitRole::GuardBundle) => (
            Artifact::ParamsEp,
            Artifact::GuardBundlePkEp,
            Artifact::GuardBundleVkEp,
        ),
        (Parity::Eq, CircuitRole::P256V3) => (
            Artifact::ParamsEq,
            Artifact::P256V3PkEq,
            Artifact::P256V3VkEq,
        ),
        (Parity::Ep, CircuitRole::P256V3) => (
            Artifact::ParamsEp,
            Artifact::P256V3PkEp,
            Artifact::P256V3VkEp,
        ),
        (Parity::Eq, CircuitRole::StateLeaf) => (
            Artifact::ParamsEq,
            Artifact::StateLeafPkEq,
            Artifact::StateLeafVkEq,
        ),
        (Parity::Ep, CircuitRole::StateLeaf) => (
            Artifact::ParamsEp,
            Artifact::StateLeafPkEp,
            Artifact::StateLeafVkEp,
        ),
        (Parity::Eq, CircuitRole::GuardBundleLeaf) => (
            Artifact::ParamsEq,
            Artifact::GuardBundleLeafPkEq,
            Artifact::GuardBundleLeafVkEq,
        ),
        (Parity::Ep, CircuitRole::GuardBundleLeaf) => (
            Artifact::ParamsEp,
            Artifact::GuardBundleLeafPkEp,
            Artifact::GuardBundleLeafVkEp,
        ),
    }
}

/// One parsed, authenticated, concrete-circuit proving capability.
pub(super) struct OfflineCashAuthenticatedProvingMaterialV1<C>
where
    C: CurveAffine,
{
    /// Canonical transparent parameters regenerated after exact-byte validation.
    pub(super) params: ParamsIPA<C>,
    /// Processed proving key parsed without a release-sized byte copy.
    pub(super) proving_key: ProvingKey<C>,
    /// Threshold-authenticated processed proving-key identity.
    pub(super) proving_key_binding: OfflineCashArtifactBindingV1,
    /// Compiled role/parity protocol identity.
    pub(super) protocol_digest: [u8; 32],
}

fn parse_authenticated_proving_key_v1<C, ConcreteCircuit>(
    source: &dyn OfflineCashHalo2ArtifactSourceV1,
    expected: OfflineCashArtifactBindingV1,
    shape: &ProcessedProvingKeyShapeV1,
) -> Result<ProvingKey<C>, OfflineCashHalo2ArtifactErrorV1>
where
    C: SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: Circuit<C::ScalarExt>,
    ConcreteCircuit::Params: Default,
{
    if expected.byte_len != shape.exact_byte_len {
        return Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact);
    }
    consume_source_artifact_once(source, expected.role, |reader| {
        let mut authenticated = AuthenticatedProvingKeyReaderV1::new(reader, expected, shape);
        let parsed = catch_unwind(AssertUnwindSafe(|| {
            #[cfg(feature = "circuit-params")]
            {
                ProvingKey::<C>::read::<_, ConcreteCircuit>(
                    &mut authenticated,
                    SerdeFormat::Processed,
                    ConcreteCircuit::Params::default(),
                )
            }
            #[cfg(not(feature = "circuit-params"))]
            {
                ProvingKey::<C>::read::<_, ConcreteCircuit>(
                    &mut authenticated,
                    SerdeFormat::Processed,
                )
            }
        }));
        let consumed_exact = authenticated.finish_authentication()?;
        match parsed {
            Ok(Ok(proving_key)) if consumed_exact => Ok(proving_key),
            Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {
                Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)
            }
        }
    })
}

fn compare_authenticated_proving_key_v1<C>(
    source: &dyn OfflineCashHalo2ArtifactSourceV1,
    expected: OfflineCashArtifactBindingV1,
    proving_key: &ProvingKey<C>,
) -> Result<(), OfflineCashHalo2ArtifactErrorV1>
where
    C: SerdeCurveAffine,
    C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
{
    consume_source_artifact_once(source, expected.role, |reader| {
        let mut comparison = AuthenticatedArtifactComparisonWriterV1::new(reader, expected);
        let serialized = catch_unwind(AssertUnwindSafe(|| {
            proving_key.write_streaming(&mut comparison, SerdeFormat::Processed)
        }));
        let exact_match = comparison.finish_authentication()?;
        match serialized {
            Ok(Ok(())) if exact_match => Ok(()),
            Ok(Ok(())) | Ok(Err(_)) | Err(_) => {
                Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)
            }
        }
    })
}

/// Hash-authenticated verifier artifacts without a proof-acceptance capability.
///
/// Raw bytes remain private to this module. The STATE backend may borrow exact
/// authenticated payloads through the role-safe loader below and must still
/// parse them against the concrete compiled circuit before it can verify.
pub(crate) struct OfflineCashAuthenticatedVerifierArtifactsV1 {
    source: Arc<dyn OfflineCashHalo2ArtifactSourceV1>,
    manifest: OfflineCashHalo2ArtifactManifestV1,
}

impl fmt::Debug for OfflineCashAuthenticatedVerifierArtifactsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashAuthenticatedVerifierArtifactsV1")
            .field("release_id", &self.manifest.release_id)
            .field(
                "release_manifest_digest",
                &self.manifest.release_manifest_digest,
            )
            .finish_non_exhaustive()
    }
}

impl OfflineCashAuthenticatedVerifierArtifactsV1 {
    /// Authenticate the exact Eq/Ep parameter and STATE verifier-key bytes.
    pub(crate) fn load(
        source: Arc<dyn OfflineCashHalo2ArtifactSourceV1>,
    ) -> Result<Self, OfflineCashHalo2ArtifactErrorV1> {
        let manifest = OfflineCashHalo2ArtifactManifestV1::from_authenticated_release(
            source.authenticated_release(),
        )?;
        for role in [
            OfflineCashArtifactRoleV1::ParamsEq,
            OfflineCashArtifactRoleV1::StateVkEq,
            OfflineCashArtifactRoleV1::ParamsEp,
            OfflineCashArtifactRoleV1::StateVkEp,
        ] {
            authenticate_source_artifact(source.as_ref(), manifest.artifact(role))?;
        }
        Ok(Self { source, manifest })
    }

    /// Parse one role-safe proving capability directly from its authenticated
    /// source without first allocating a second proving-key-sized byte vector.
    pub(super) fn load_proving_material<C, ConcreteCircuit>(
        &self,
    ) -> Result<OfflineCashAuthenticatedProvingMaterialV1<C>, OfflineCashHalo2ArtifactErrorV1>
    where
        C: SerdeCurveAffine,
        C::ScalarExt: SerdePrimeField + FromUniformBytes<64>,
        ConcreteCircuit: OfflineCashProvingCircuitV1<C>,
        ConcreteCircuit::Params: Default,
    {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let parity = ConcreteCircuit::PARITY;
        let circuit_role = ConcreteCircuit::ROLE;
        let (params_role, proving_role, verifier_role) =
            proving_artifact_roles_v1(parity, circuit_role);
        if offline_cash_artifact_protocol_v1(proving_role) != Some((parity, circuit_role))
            || offline_cash_artifact_protocol_v1(verifier_role) != Some((parity, circuit_role))
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let protocol_digest = self.manifest.protocol_digest(parity, circuit_role);
        if protocol_digest != offline_cash_halo2_protocol_identity_v1(parity, circuit_role).digest()
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }

        let params_binding = self.manifest.artifact(params_role);
        let params_bytes =
            read_authenticated_source_artifact(self.source.as_ref(), params_binding)?;
        let params = parse_params_exact_for_k::<C>(&params_bytes, OFFLINE_CASH_HALO2_K_V1)
            .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidParameterArtifact)?;
        drop(params_bytes);

        let verifier_binding = self.manifest.artifact(verifier_role);
        let verifier_bytes =
            read_authenticated_source_artifact(self.source.as_ref(), verifier_binding)?;
        let verifier = parse_processed_verifier_key_v1::<C, ConcreteCircuit>(
            &verifier_bytes,
            OFFLINE_CASH_HALO2_K_V1,
        )
        .map_err(|_| OfflineCashHalo2ArtifactErrorV1::InvalidVerifierKeyArtifact)?;
        let shape = ProcessedProvingKeyShapeV1::for_verifier(&verifier_bytes, &verifier)?;

        let proving_key_binding = self.manifest.artifact(proving_role);
        let proving_key = parse_authenticated_proving_key_v1::<C, ConcreteCircuit>(
            self.source.as_ref(),
            proving_key_binding,
            &shape,
        )?;
        if proving_key.get_vk().to_bytes(SerdeFormat::Processed) != verifier_bytes {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact);
        }
        drop(verifier_bytes);
        drop(verifier);
        compare_authenticated_proving_key_v1(
            self.source.as_ref(),
            proving_key_binding,
            &proving_key,
        )?;

        Ok(OfflineCashAuthenticatedProvingMaterialV1 {
            params,
            proving_key,
            proving_key_binding,
            protocol_digest,
        })
    }

    pub(super) fn authenticate_state_verifier(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(), OfflineCashHalo2ArtifactErrorV1> {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let (params_role, verifier_role) = match parity {
            OfflineCashHalo2ParityV1::Eq => (
                OfflineCashArtifactRoleV1::ParamsEq,
                OfflineCashArtifactRoleV1::StateVkEq,
            ),
            OfflineCashHalo2ParityV1::Ep => (
                OfflineCashArtifactRoleV1::ParamsEp,
                OfflineCashArtifactRoleV1::StateVkEp,
            ),
        };
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier || verifying_key.role != verifier_role {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest != self.manifest.state_protocol_digest(parity) {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        authenticate_source_artifact(self.source.as_ref(), self.manifest.artifact(params_role))?;
        authenticate_source_artifact(self.source.as_ref(), expected_verifier)
    }

    /// Load the exact authenticated parameter and STATE verifier-key payloads.
    ///
    /// The returned bytes have already been length- and SHA-256-authenticated
    /// against the immutable threshold-authenticated release. They remain
    /// private to the first-party backend and grant no acceptance authority on
    /// their own.
    pub(super) fn load_state_verifier_bytes(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>), OfflineCashHalo2ArtifactErrorV1> {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let (params_role, verifier_role) = match parity {
            OfflineCashHalo2ParityV1::Eq => (
                OfflineCashArtifactRoleV1::ParamsEq,
                OfflineCashArtifactRoleV1::StateVkEq,
            ),
            OfflineCashHalo2ParityV1::Ep => (
                OfflineCashArtifactRoleV1::ParamsEp,
                OfflineCashArtifactRoleV1::StateVkEp,
            ),
        };
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier || verifying_key.role != verifier_role {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest != self.manifest.state_protocol_digest(parity) {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        let parameters = read_authenticated_source_artifact(
            self.source.as_ref(),
            self.manifest.artifact(params_role),
        )?;
        let verifier = read_authenticated_source_artifact(self.source.as_ref(), expected_verifier)?;
        Ok((parameters, verifier))
    }

    /// Authenticate one FloorPlanner helper-leaf verifier key under its exact
    /// role/parity protocol. The recursive GuardBundle wrapper has a distinct
    /// artifact identity and loader.
    ///
    /// This boundary grants no proof-verification capability. It only proves
    /// that the immutable bytes match the already threshold-authenticated
    /// release manifest and the compiled helper profile.
    pub(super) fn authenticate_helper_verifier(
        &self,
        parity: OfflineCashHalo2ParityV1,
        role: OfflineCashHalo2CircuitRoleV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(), OfflineCashHalo2ArtifactErrorV1> {
        if matches!(
            role,
            OfflineCashHalo2CircuitRoleV1::State
                | OfflineCashHalo2CircuitRoleV1::StateLeaf
                | OfflineCashHalo2CircuitRoleV1::GuardBundle
                | OfflineCashHalo2CircuitRoleV1::P256V3
        ) || !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let params_role = match parity {
            OfflineCashHalo2ParityV1::Eq => OfflineCashArtifactRoleV1::ParamsEq,
            OfflineCashHalo2ParityV1::Ep => OfflineCashArtifactRoleV1::ParamsEp,
        };
        let verifier_role = match (parity, role) {
            (OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2CircuitRoleV1::GuardUse) => {
                OfflineCashArtifactRoleV1::GuardUseVkEq
            }
            (OfflineCashHalo2ParityV1::Ep, OfflineCashHalo2CircuitRoleV1::GuardUse) => {
                OfflineCashArtifactRoleV1::GuardUseVkEp
            }
            (OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2CircuitRoleV1::PlatformBind) => {
                OfflineCashArtifactRoleV1::PlatformBindVkEq
            }
            (OfflineCashHalo2ParityV1::Ep, OfflineCashHalo2CircuitRoleV1::PlatformBind) => {
                OfflineCashArtifactRoleV1::PlatformBindVkEp
            }
            (OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2CircuitRoleV1::AndroidKeyCert) => {
                OfflineCashArtifactRoleV1::AndroidKeyCertVkEq
            }
            (OfflineCashHalo2ParityV1::Ep, OfflineCashHalo2CircuitRoleV1::AndroidKeyCert) => {
                OfflineCashArtifactRoleV1::AndroidKeyCertVkEp
            }
            (OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf) => {
                OfflineCashArtifactRoleV1::GuardBundleLeafVkEq
            }
            (OfflineCashHalo2ParityV1::Ep, OfflineCashHalo2CircuitRoleV1::GuardBundleLeaf) => {
                OfflineCashArtifactRoleV1::GuardBundleLeafVkEp
            }
            (_, OfflineCashHalo2CircuitRoleV1::State) => {
                return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
            }
            (_, OfflineCashHalo2CircuitRoleV1::P256V3) => {
                return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
            }
            (_, OfflineCashHalo2CircuitRoleV1::StateLeaf) => {
                return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
            }
            (_, OfflineCashHalo2CircuitRoleV1::GuardBundle) => {
                return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
            }
        };
        if verifying_key.role != verifier_role
            || offline_cash_artifact_protocol_v1(verifying_key.role) != Some((parity, role))
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest != self.manifest.protocol_digest(parity, role) {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        authenticate_source_artifact(self.source.as_ref(), self.manifest.artifact(params_role))?;
        authenticate_source_artifact(self.source.as_ref(), expected_verifier)
    }

    /// Load one exact authenticated helper verifier after role/parity mapping.
    ///
    /// The result grants no authority by itself; the helper proof boundary must
    /// still parse the key against the concrete circuit and cryptographically
    /// verify the complete public instance, circuit-bound carried lineage, and
    /// reciprocal audit equations.
    pub(super) fn load_helper_verifier_bytes(
        &self,
        parity: OfflineCashHalo2ParityV1,
        role: OfflineCashHalo2CircuitRoleV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>), OfflineCashHalo2ArtifactErrorV1> {
        self.authenticate_helper_verifier(parity, role, verifying_key, protocol_digest)?;
        let params_role = match parity {
            OfflineCashHalo2ParityV1::Eq => OfflineCashArtifactRoleV1::ParamsEq,
            OfflineCashHalo2ParityV1::Ep => OfflineCashArtifactRoleV1::ParamsEp,
        };
        let parameters = read_authenticated_source_artifact(
            self.source.as_ref(),
            self.manifest.artifact(params_role),
        )?;
        let verifier = read_authenticated_source_artifact(self.source.as_ref(), verifying_key)?;
        Ok((parameters, verifier))
    }

    /// Authenticate and load the recursive GuardBundle wrapper verifier. It is
    /// deliberately distinct from the GuardBundle SHA leaf key.
    pub(super) fn load_guard_bundle_verifier_bytes(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>), OfflineCashHalo2ArtifactErrorV1> {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let (params_role, verifier_role) = match parity {
            OfflineCashHalo2ParityV1::Eq => (
                OfflineCashArtifactRoleV1::ParamsEq,
                OfflineCashArtifactRoleV1::GuardBundleVkEq,
            ),
            OfflineCashHalo2ParityV1::Ep => (
                OfflineCashArtifactRoleV1::ParamsEp,
                OfflineCashArtifactRoleV1::GuardBundleVkEp,
            ),
        };
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier
            || offline_cash_artifact_protocol_v1(verifying_key.role)
                != Some((parity, OfflineCashHalo2CircuitRoleV1::GuardBundle))
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest
            != self
                .manifest
                .protocol_digest(parity, OfflineCashHalo2CircuitRoleV1::GuardBundle)
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        let parameters = read_authenticated_source_artifact(
            self.source.as_ref(),
            self.manifest.artifact(params_role),
        )?;
        let verifier = read_authenticated_source_artifact(self.source.as_ref(), expected_verifier)?;
        Ok((parameters, verifier))
    }

    /// Authenticate and load the one shared P-256 V3 child verifier for a parity.
    ///
    /// The exact typed role mapping prevents a helper VK, the reciprocal
    /// parity's VK, or an unauthenticated key from becoming a P-256 statement
    /// verifier. Parsing against the concrete V3 circuit remains mandatory at
    /// the recursive-wrapper boundary.
    pub(super) fn load_p256_v3_verifier_bytes(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>), OfflineCashHalo2ArtifactErrorV1> {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let (params_role, verifier_role) = match parity {
            OfflineCashHalo2ParityV1::Eq => (
                OfflineCashArtifactRoleV1::ParamsEq,
                OfflineCashArtifactRoleV1::P256V3VkEq,
            ),
            OfflineCashHalo2ParityV1::Ep => (
                OfflineCashArtifactRoleV1::ParamsEp,
                OfflineCashArtifactRoleV1::P256V3VkEp,
            ),
        };
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier
            || offline_cash_artifact_protocol_v1(verifying_key.role)
                != Some((parity, OfflineCashHalo2CircuitRoleV1::P256V3))
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest
            != self
                .manifest
                .protocol_digest(parity, OfflineCashHalo2CircuitRoleV1::P256V3)
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        let parameters = read_authenticated_source_artifact(
            self.source.as_ref(),
            self.manifest.artifact(params_role),
        )?;
        let verifier = read_authenticated_source_artifact(self.source.as_ref(), expected_verifier)?;
        Ok((parameters, verifier))
    }

    /// Authenticate and load the private STATE-relation leaf verifier.
    pub(super) fn load_state_leaf_verifier_bytes(
        &self,
        parity: OfflineCashHalo2ParityV1,
        verifying_key: OfflineCashArtifactBindingV1,
        protocol_digest: [u8; 32],
    ) -> Result<(Vec<u8>, Vec<u8>), OfflineCashHalo2ArtifactErrorV1> {
        if !self
            .manifest
            .matches_release(self.source.authenticated_release())
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        let (params_role, verifier_role) = match parity {
            OfflineCashHalo2ParityV1::Eq => (
                OfflineCashArtifactRoleV1::ParamsEq,
                OfflineCashArtifactRoleV1::StateLeafVkEq,
            ),
            OfflineCashHalo2ParityV1::Ep => (
                OfflineCashArtifactRoleV1::ParamsEp,
                OfflineCashArtifactRoleV1::StateLeafVkEp,
            ),
        };
        let expected_verifier = self.manifest.artifact(verifier_role);
        if verifying_key != expected_verifier
            || offline_cash_artifact_protocol_v1(verifying_key.role)
                != Some((parity, OfflineCashHalo2CircuitRoleV1::StateLeaf))
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::InvalidManifest);
        }
        if protocol_digest
            != self
                .manifest
                .protocol_digest(parity, OfflineCashHalo2CircuitRoleV1::StateLeaf)
        {
            return Err(OfflineCashHalo2ArtifactErrorV1::ProtocolMismatch);
        }
        let parameters = read_authenticated_source_artifact(
            self.source.as_ref(),
            self.manifest.artifact(params_role),
        )?;
        let verifier = read_authenticated_source_artifact(self.source.as_ref(), expected_verifier)?;
        Ok((parameters, verifier))
    }

    /// Confirm that a separately supplied authenticated release is the exact
    /// release that owns this source and immutable artifact manifest.
    pub(super) fn matches_authenticated_release(
        &self,
        release: &OfflineCashAuthenticatedReleaseV1,
    ) -> bool {
        self.manifest.matches_release(release)
            && self
                .manifest
                .matches_release(self.source.authenticated_release())
    }

    /// Exact immutable manifest derived at artifact authentication.
    pub(crate) const fn manifest(&self) -> &OfflineCashHalo2ArtifactManifestV1 {
        &self.manifest
    }
}

#[cfg(test)]
mod proving_key_shape_tests {
    use super::*;

    #[test]
    fn processed_pk_shape_pins_every_allocation_header_and_exact_length() {
        let verifier_bytes = vec![0xa5; 19];
        let shape = ProcessedProvingKeyShapeV1::from_geometry(
            &verifier_bytes,
            OFFLINE_CASH_HALO2_K_V1,
            32,
            2,
            3,
        )
        .expect("governed processed-PK geometry");

        let polynomial_payload = (1_u64 << OFFLINE_CASH_HALO2_K_V1) * 32;
        let polynomial_frame = 4 + polynomial_payload;
        let polynomial_count = 3 + 2 * 2 + 2 * 3;
        let vector_count_headers = 4_u64;
        assert_eq!(
            shape.exact_byte_len,
            verifier_bytes.len() as u64
                + polynomial_count * polynomial_frame
                + vector_count_headers * 4
        );
        assert_eq!(shape.required.len(), 1 + polynomial_count as usize + 4);
        assert_eq!(shape.required[0].offset, 0);
        assert_eq!(shape.required[0].bytes, verifier_bytes);
        assert!(shape.required.windows(2).all(|pair| {
            pair[0].offset + u64::try_from(pair[0].bytes.len()).expect("small required frame")
                <= pair[1].offset
        }));
        assert!(shape.required.iter().skip(1).all(|required| {
            required.bytes == (1_u32 << OFFLINE_CASH_HALO2_K_V1).to_be_bytes()
                || required.bytes == 2_u32.to_be_bytes()
                || required.bytes == 3_u32.to_be_bytes()
        }));

        assert!(matches!(
            ProcessedProvingKeyShapeV1::from_geometry(
                &verifier_bytes,
                OFFLINE_CASH_HALO2_K_V1 - 1,
                32,
                2,
                3,
            ),
            Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)
        ));
        assert!(matches!(
            ProcessedProvingKeyShapeV1::from_geometry(
                &verifier_bytes,
                OFFLINE_CASH_HALO2_K_V1,
                31,
                2,
                3,
            ),
            Err(OfflineCashHalo2ArtifactErrorV1::InvalidProvingKeyArtifact)
        ));
    }

    #[test]
    fn processed_pk_required_bytes_are_checked_across_fragmented_reads() {
        let shape = ProcessedProvingKeyShapeV1 {
            exact_byte_len: 12,
            required: vec![
                RequiredProvingKeyBytesV1 {
                    offset: 0,
                    bytes: vec![1, 2, 3],
                },
                RequiredProvingKeyBytesV1 {
                    offset: 5,
                    bytes: vec![4, 5, 6, 7],
                },
            ],
        };
        let expected = OfflineCashArtifactBindingV1 {
            role: OfflineCashArtifactRoleV1::StatePkEq,
            sha256: [0x5a; 32],
            byte_len: shape.exact_byte_len,
        };
        let mut source = io::empty();
        let mut reader = AuthenticatedProvingKeyReaderV1::new(&mut source, expected, &shape);
        assert!(reader.validate_required_bytes(0, &[1, 2]));
        assert_eq!(reader.next_required, 0);
        assert!(reader.validate_required_bytes(2, &[3, 0, 0, 4]));
        assert_eq!(reader.next_required, 1);
        assert!(reader.validate_required_bytes(6, &[5, 6, 7]));
        assert_eq!(reader.next_required, shape.required.len());

        let mut source = io::empty();
        let mut reader = AuthenticatedProvingKeyReaderV1::new(&mut source, expected, &shape);
        assert!(!reader.validate_required_bytes(0, &[1, 2, 0]));
        assert!(reader.shape_mismatch);
    }
}
