//! Authenticated, source-backed Offline Cash V1 Halo2 artifact boundary.

use std::{collections::BTreeSet, fmt, io::Read, sync::Arc};

use iroha_data_model::offline::{
    OFFLINE_CASH_ARTIFACT_SET_MAX_BYTES_V1, OfflineCashArtifactBindingV1,
    OfflineCashArtifactRoleV1, OfflineCashAuthenticatedReleaseV1,
};
use sha2::{Digest as _, Sha256};

use super::protocol::{
    OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
    offline_cash_artifact_length_bounds_v1, offline_cash_artifact_protocol_v1,
    offline_cash_halo2_profile_digest_v1, offline_cash_halo2_protocol_identity_v1,
};

const ARTIFACT_COUNT: usize = OfflineCashArtifactRoleV1::ALL.len();
const AUTHENTICATION_BUFFER_BYTES: usize = 64 * 1024;

/// Fail-closed artifact/profile admission error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfflineCashHalo2ArtifactErrorV1 {
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
    /// First-party semantic key/proof verification is not implemented yet.
    VerificationUnavailable,
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
            Self::VerificationUnavailable => {
                "offline-cash first-party Halo2 verification is unavailable"
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

/// Hash-authenticated verifier artifacts without a proof-acceptance capability.
///
/// This owner intentionally does not expose raw bytes and is not evidence that
/// the keys are semantically valid Halo2 keys.  The verifier skeleton therefore
/// remains unavailable even after this boundary succeeds.
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

    /// Authenticate one helper verifier key under its exact role/parity protocol.
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
        if role == OfflineCashHalo2CircuitRoleV1::State
            || !self
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
            (OfflineCashHalo2ParityV1::Eq, OfflineCashHalo2CircuitRoleV1::GuardBundle) => {
                OfflineCashArtifactRoleV1::GuardBundleVkEq
            }
            (OfflineCashHalo2ParityV1::Ep, OfflineCashHalo2CircuitRoleV1::GuardBundle) => {
                OfflineCashArtifactRoleV1::GuardBundleVkEp
            }
            (_, OfflineCashHalo2CircuitRoleV1::State) => {
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

    /// Exact immutable manifest derived at artifact authentication.
    pub(crate) const fn manifest(&self) -> &OfflineCashHalo2ArtifactManifestV1 {
        &self.manifest
    }
}
