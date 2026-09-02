//! Content-addressed artifact resolution for the authenticated Kagemusha V1 release.
//!
//! The threshold-authenticated release manifest is the only role registry. Artifact files are
//! deliberately unframed: IPA parameters use Halo2's canonical `ParamsIPA::write` bytes and keys
//! use `SerdeFormat::Processed`. This avoids a second, potentially divergent role header and keeps
//! the manifest's `(role, sha256, byte_len)` tuple authoritative.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
};

use halo2_proofs::{
    halo2curves::{
        CurveAffine,
        pasta::{EpAffine, EqAffine},
    },
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use iroha_data_model::isi::KagemushaRedemptionRequestV1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ARTIFACT_SET_MAX_BYTES_V1, KAGEMUSHA_HALO2_K_V1,
    KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1, KAGEMUSHA_PARAMS_BYTES_V1,
    KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1,
    KagemushaArtifactBindingV1, KagemushaArtifactRoleV1, KagemushaAuthenticatedReleaseV1,
};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{
    DigestV1, KagemushaPastaParityV1, KagemushaRecursionArtifactsV1,
    KagemushaRecursionErrorV1, KagemushaRecursiveVerifierV1,
    VerifiedKagemushaRedemptionProofV1, verify_kagemusha_redemption_request_v1,
};

/// Logical circuit family selected by one authenticated artifact role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCircuitFamilyV1 {
    /// The one six-operation aggregate-state recursion circuit.
    State,
    /// Recipient hardware authorization checked before reserve mutation.
    MintAuthorization,
    /// Finalized reserve-mint receipt and consensus-finality helper.
    MintCredit,
    /// Provider-neutral hardware credential helper.
    PlatformCredential,
    /// Normalized composition of every monetary hardware guard.
    GuardBundle,
    /// Terminal wrapper binding a prepared transition to hardware commit.
    CommitWrapper,
}

/// Serialization kind selected by one authenticated artifact role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaArtifactKindV1 {
    /// Canonical transparent IPA parameters produced by `ParamsIPA::write`.
    Parameters,
    /// Halo2 proving key in `SerdeFormat::Processed`.
    ProvingKey,
    /// Halo2 verifying key in `SerdeFormat::Processed`.
    VerifyingKey,
}

/// Complete static interpretation of one role in the sole V1 artifact ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaArtifactDescriptorV1 {
    /// Exact authenticated role.
    pub role: KagemushaArtifactRoleV1,
    /// Non-interchangeable Pasta parity.
    pub parity: KagemushaPastaParityV1,
    /// Circuit family, or `None` for shared IPA parameters.
    pub family: Option<KagemushaCircuitFamilyV1>,
    /// Exact raw serialization kind.
    pub kind: KagemushaArtifactKindV1,
    /// Required exact byte length for parameters, or inclusive maximum for a key.
    pub byte_limit: u64,
}

impl KagemushaArtifactDescriptorV1 {
    /// Return the sole V1 interpretation of an authenticated artifact role.
    #[must_use]
    pub const fn for_role(role: KagemushaArtifactRoleV1) -> Self {
        use KagemushaArtifactKindV1::{Parameters, ProvingKey, VerifyingKey};
        use KagemushaArtifactRoleV1 as Role;
        use KagemushaCircuitFamilyV1::{
            CommitWrapper, GuardBundle, MintAuthorization, MintCredit, PlatformCredential, State,
        };
        use KagemushaPastaParityV1::{Ep, Eq};

        let parity = match role {
            Role::ParamsEq
            | Role::StatePkEq
            | Role::StateVkEq
            | Role::MintAuthorizationPkEq
            | Role::MintAuthorizationVkEq
            | Role::MintCreditPkEq
            | Role::MintCreditVkEq
            | Role::PlatformCredentialPkEq
            | Role::PlatformCredentialVkEq
            | Role::GuardBundlePkEq
            | Role::GuardBundleVkEq
            | Role::CommitWrapperPkEq
            | Role::CommitWrapperVkEq => Eq,
            Role::ParamsEp
            | Role::StatePkEp
            | Role::StateVkEp
            | Role::MintAuthorizationPkEp
            | Role::MintAuthorizationVkEp
            | Role::MintCreditPkEp
            | Role::MintCreditVkEp
            | Role::PlatformCredentialPkEp
            | Role::PlatformCredentialVkEp
            | Role::GuardBundlePkEp
            | Role::GuardBundleVkEp
            | Role::CommitWrapperPkEp
            | Role::CommitWrapperVkEp => Ep,
        };
        let family = match role {
            Role::ParamsEq | Role::ParamsEp => None,
            Role::StatePkEq | Role::StateVkEq | Role::StatePkEp | Role::StateVkEp => Some(State),
            Role::MintAuthorizationPkEq
            | Role::MintAuthorizationVkEq
            | Role::MintAuthorizationPkEp
            | Role::MintAuthorizationVkEp => Some(MintAuthorization),
            Role::MintCreditPkEq
            | Role::MintCreditVkEq
            | Role::MintCreditPkEp
            | Role::MintCreditVkEp => Some(MintCredit),
            Role::PlatformCredentialPkEq
            | Role::PlatformCredentialVkEq
            | Role::PlatformCredentialPkEp
            | Role::PlatformCredentialVkEp => Some(PlatformCredential),
            Role::GuardBundlePkEq
            | Role::GuardBundleVkEq
            | Role::GuardBundlePkEp
            | Role::GuardBundleVkEp => Some(GuardBundle),
            Role::CommitWrapperPkEq
            | Role::CommitWrapperVkEq
            | Role::CommitWrapperPkEp
            | Role::CommitWrapperVkEp => Some(CommitWrapper),
        };
        let (kind, byte_limit) = match role {
            Role::ParamsEq | Role::ParamsEp => (Parameters, KAGEMUSHA_PARAMS_BYTES_V1),
            Role::StatePkEq | Role::StatePkEp => {
                (ProvingKey, KAGEMUSHA_STATE_PROVING_KEY_MAX_BYTES_V1)
            }
            Role::MintAuthorizationPkEq
            | Role::MintAuthorizationPkEp
            | Role::MintCreditPkEq
            | Role::MintCreditPkEp
            | Role::PlatformCredentialPkEq
            | Role::PlatformCredentialPkEp
            | Role::GuardBundlePkEq
            | Role::GuardBundlePkEp
            | Role::CommitWrapperPkEq
            | Role::CommitWrapperPkEp => (ProvingKey, KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1),
            Role::StateVkEq
            | Role::StateVkEp
            | Role::MintAuthorizationVkEq
            | Role::MintAuthorizationVkEp
            | Role::MintCreditVkEq
            | Role::MintCreditVkEp
            | Role::PlatformCredentialVkEq
            | Role::PlatformCredentialVkEp
            | Role::GuardBundleVkEq
            | Role::GuardBundleVkEp
            | Role::CommitWrapperVkEq
            | Role::CommitWrapperVkEp => (VerifyingKey, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1),
        };
        Self {
            role,
            parity,
            family,
            kind,
            byte_limit,
        }
    }

    fn validate_binding(
        self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<(), KagemushaArtifactErrorV1> {
        let valid_len = match self.kind {
            KagemushaArtifactKindV1::Parameters => binding.byte_len == self.byte_limit,
            KagemushaArtifactKindV1::ProvingKey | KagemushaArtifactKindV1::VerifyingKey => {
                binding.byte_len != 0 && binding.byte_len <= self.byte_limit
            }
        };
        if binding.role != self.role || binding.sha256 == [0; 32] || !valid_len {
            return Err(KagemushaArtifactErrorV1::InvalidBinding(self.role));
        }
        Ok(())
    }
}

/// Failure while resolving or interpreting threshold-authenticated artifact bytes.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KagemushaArtifactErrorV1 {
    /// One manifest binding does not match the fixed role schema.
    #[error("invalid Kagemusha V1 artifact binding for {0:?}")]
    InvalidBinding(KagemushaArtifactRoleV1),
    /// The manifest inventory exceeds the fixed complete-package bound.
    #[error("Kagemusha V1 artifact inventory exceeds its fixed byte bound")]
    InventoryTooLarge,
    /// Release-derived verifier metadata or the canonical empty-effect digest is invalid.
    #[error("invalid Kagemusha V1 authenticated verifier release: {0}")]
    InvalidRelease(String),
    /// A content-addressed artifact could not be read.
    #[error("failed to read Kagemusha V1 artifact {role:?}: {reason}")]
    Read {
        /// Role being resolved.
        role: KagemushaArtifactRoleV1,
        /// Non-sensitive I/O failure description.
        reason: String,
    },
    /// A resolver returned a different length than the authenticated binding.
    #[error("Kagemusha V1 artifact {role:?} has length {actual}, expected {expected}")]
    LengthMismatch {
        /// Role being resolved.
        role: KagemushaArtifactRoleV1,
        /// Authenticated length.
        expected: u64,
        /// Resolved length.
        actual: u64,
    },
    /// A resolver returned bytes with a different SHA-256 content address.
    #[error("Kagemusha V1 artifact {0:?} does not match its content address")]
    DigestMismatch(KagemushaArtifactRoleV1),
    /// The transparent IPA parameter file is not the unique deterministic fixed-k encoding.
    #[error("Kagemusha V1 {0:?} IPA parameters are not the canonical k=16 parameters")]
    NonCanonicalParameters(KagemushaPastaParityV1),
    /// A requested in-memory content address was not installed.
    #[error("Kagemusha V1 artifact {0:?} is not installed")]
    Missing(KagemushaArtifactRoleV1),
}

/// Read exact bytes selected by an authenticated `(role, SHA-256, byte_len)` binding.
///
/// Implementations are untrusted storage adapters. Every caller rechecks both length and digest;
/// successful resolution alone never grants proof authority.
pub trait KagemushaArtifactByteResolverV1: Send + Sync {
    /// Resolve one artifact's exact bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the content address is absent or cannot be read.
    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1>;
}

/// Filesystem resolver whose only filename is the lowercase SHA-256 content address.
#[derive(Clone, Debug)]
pub struct KagemushaDirectoryArtifactResolverV1 {
    root: PathBuf,
}

impl KagemushaDirectoryArtifactResolverV1 {
    /// Open an existing artifact directory and freeze its canonical absolute path.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when `root` does not name an accessible directory.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = fs::canonicalize(root)?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Kagemusha artifact root is not a directory",
            ));
        }
        Ok(Self { root })
    }

    /// Return the exact content-addressed path used for a digest.
    #[must_use]
    pub fn path_for_digest(&self, digest: DigestV1) -> PathBuf {
        self.root.join(lower_hex(digest))
    }
}

impl KagemushaArtifactByteResolverV1 for KagemushaDirectoryArtifactResolverV1 {
    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        KagemushaArtifactDescriptorV1::for_role(binding.role).validate_binding(binding)?;
        let path = self.path_for_digest(binding.sha256);
        let metadata = fs::metadata(&path).map_err(|error| KagemushaArtifactErrorV1::Read {
            role: binding.role,
            reason: error.to_string(),
        })?;
        if !metadata.is_file() {
            return Err(KagemushaArtifactErrorV1::Read {
                role: binding.role,
                reason: "content address is not a regular file".to_owned(),
            });
        }
        if metadata.len() != binding.byte_len {
            return Err(KagemushaArtifactErrorV1::LengthMismatch {
                role: binding.role,
                expected: binding.byte_len,
                actual: metadata.len(),
            });
        }
        let bytes = fs::read(path).map_err(|error| KagemushaArtifactErrorV1::Read {
            role: binding.role,
            reason: error.to_string(),
        })?;
        validate_resolved_bytes(binding, &bytes)?;
        Ok(Arc::from(bytes))
    }
}

/// In-memory content-addressed resolver for embedded artifacts and deterministic generation.
#[derive(Clone, Debug, Default)]
pub struct KagemushaMemoryArtifactResolverV1 {
    by_digest: BTreeMap<DigestV1, Arc<[u8]>>,
}

impl KagemushaMemoryArtifactResolverV1 {
    /// Install bytes under their computed SHA-256 address.
    pub fn insert(&mut self, bytes: impl Into<Arc<[u8]>>) -> DigestV1 {
        let bytes = bytes.into();
        let digest = Sha256::digest(bytes.as_ref()).into();
        self.by_digest.insert(digest, bytes);
        digest
    }

    /// Return the number of distinct installed content addresses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_digest.len()
    }

    /// Return true when no content addresses are installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_digest.is_empty()
    }
}

impl KagemushaArtifactByteResolverV1 for KagemushaMemoryArtifactResolverV1 {
    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        let bytes = self
            .by_digest
            .get(&binding.sha256)
            .cloned()
            .ok_or(KagemushaArtifactErrorV1::Missing(binding.role))?;
        validate_resolved_bytes(binding, bytes.as_ref())?;
        Ok(bytes)
    }
}

/// Release-authenticated inventory paired with an untrusted byte resolver.
///
/// The set intentionally resolves one artifact at a time rather than retaining the complete
/// package: the release permits a large proving-key package while fixing a much smaller runtime
/// resident-memory ceiling.
#[derive(Clone, Debug)]
pub struct KagemushaAuthenticatedArtifactSetV1<R> {
    recursion: KagemushaRecursionArtifactsV1,
    suite_id: DigestV1,
    vk_set_digest: DigestV1,
    bindings: [KagemushaArtifactBindingV1; KagemushaArtifactRoleV1::ALL.len()],
    resolver: R,
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaAuthenticatedArtifactSetV1<R> {
    /// Bind an untrusted resolver to one already threshold-authenticated release.
    ///
    /// This validates the one release-wide proof suite, all 26 role/length bindings, and the
    /// complete package size before any bytes are read. It does not authenticate storage until
    /// [`Self::resolve`] is called.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-uniform release suite/verifier set, any malformed artifact
    /// binding, or an oversized complete inventory.
    pub fn new(
        release: &KagemushaAuthenticatedReleaseV1,
        canonical_empty_effect_digest: DigestV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactErrorV1> {
        let recursion = KagemushaRecursionArtifactsV1::from_authenticated_release(
            release,
            canonical_empty_effect_digest,
        );
        recursion
            .validate()
            .map_err(|error| KagemushaArtifactErrorV1::InvalidRelease(error.to_string()))?;
        let enabled_profiles = release.enabled_profiles();
        let suite_id = enabled_profiles
            .first()
            .ok_or_else(|| {
                KagemushaArtifactErrorV1::InvalidRelease(
                    "authenticated release has no enabled hardware profile".to_owned(),
                )
            })?
            .suite_id;
        let vk_set_digest = release.vk_set_digest();
        if enabled_profiles
            .iter()
            .any(|profile| profile.suite_id != suite_id || profile.vk_digest != vk_set_digest)
        {
            return Err(KagemushaArtifactErrorV1::InvalidRelease(
                "authenticated release profiles do not share one proof suite and verifier set"
                    .to_owned(),
            ));
        }
        let bindings =
            std::array::from_fn(|index| release.artifact(KagemushaArtifactRoleV1::ALL[index]));
        let mut total = 0_u64;
        let mut digests = BTreeSet::new();
        for binding in bindings {
            KagemushaArtifactDescriptorV1::for_role(binding.role).validate_binding(binding)?;
            if !digests.insert(binding.sha256) {
                return Err(KagemushaArtifactErrorV1::InvalidBinding(binding.role));
            }
            total = total
                .checked_add(binding.byte_len)
                .ok_or(KagemushaArtifactErrorV1::InventoryTooLarge)?;
        }
        if total > KAGEMUSHA_ARTIFACT_SET_MAX_BYTES_V1 {
            return Err(KagemushaArtifactErrorV1::InventoryTooLarge);
        }
        Ok(Self {
            recursion,
            suite_id,
            vk_set_digest,
            bindings,
            resolver,
        })
    }

    /// Return the proof-release identities consumed by the public verification seam.
    #[must_use]
    pub const fn recursion_artifacts(&self) -> KagemushaRecursionArtifactsV1 {
        self.recursion
    }

    /// Return the sole proof-suite identity admitted by the authenticated release.
    #[must_use]
    pub const fn suite_id(&self) -> DigestV1 {
        self.suite_id
    }

    /// Return the exact verifier-set digest from the threshold-authenticated release.
    #[must_use]
    pub const fn vk_set_digest(&self) -> DigestV1 {
        self.vk_set_digest
    }

    /// Return one exact authenticated binding.
    #[must_use]
    pub fn binding(&self, role: KagemushaArtifactRoleV1) -> KagemushaArtifactBindingV1 {
        self.bindings[role_index(role)]
    }

    /// Resolve and reauthenticate one role's exact bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, truncated, extended, or digest-substituted bytes.
    pub fn resolve(
        &self,
        role: KagemushaArtifactRoleV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        let binding = self.binding(role);
        let bytes = self.resolver.resolve_bytes(binding)?;
        validate_resolved_bytes(binding, bytes.as_ref())?;
        Ok(bytes)
    }

    /// Load and authenticate the deterministic Eq/Vesta `k = 16` IPA parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when storage bytes differ from Halo2's sole transparent derivation.
    pub fn load_eq_params(&self) -> Result<ParamsIPA<EqAffine>, KagemushaArtifactErrorV1> {
        let bytes = self.resolve(KagemushaArtifactRoleV1::ParamsEq)?;
        load_canonical_params::<EqAffine>(KagemushaPastaParityV1::Eq, bytes.as_ref())
    }

    /// Load and authenticate the deterministic Ep/Pallas `k = 16` IPA parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when storage bytes differ from Halo2's sole transparent derivation.
    pub fn load_ep_params(&self) -> Result<ParamsIPA<EpAffine>, KagemushaArtifactErrorV1> {
        let bytes = self.resolve(KagemushaArtifactRoleV1::ParamsEp)?;
        load_canonical_params::<EpAffine>(KagemushaPastaParityV1::Ep, bytes.as_ref())
    }

    /// Verify and seal one redemption using only this authenticated release's artifact identity.
    ///
    /// The verifier implementation must itself have been loaded from this set's resolved keys and
    /// rejects any protocol identity mismatch through the request passed to its hooks. No caller
    /// can substitute a separately assembled [`KagemushaRecursionArtifactsV1`] value here.
    ///
    /// # Errors
    ///
    /// Returns any signed-request, release, proof, accumulator, or governed-backend failure.
    pub fn verify_redemption_request<V>(
        &self,
        verifier: &V,
        request: KagemushaRedemptionRequestV1,
    ) -> Result<VerifiedKagemushaRedemptionProofV1, KagemushaRecursionErrorV1>
    where
        V: KagemushaRecursiveVerifierV1,
    {
        verify_kagemusha_redemption_request_v1(verifier, self.recursion, request)
    }
}

fn role_index(role: KagemushaArtifactRoleV1) -> usize {
    KagemushaArtifactRoleV1::ALL
        .iter()
        .position(|candidate| *candidate == role)
        .expect("Kagemusha V1 role belongs to its closed inventory")
}

fn validate_resolved_bytes(
    binding: KagemushaArtifactBindingV1,
    bytes: &[u8],
) -> Result<(), KagemushaArtifactErrorV1> {
    KagemushaArtifactDescriptorV1::for_role(binding.role).validate_binding(binding)?;
    let actual =
        u64::try_from(bytes.len()).map_err(|_| KagemushaArtifactErrorV1::LengthMismatch {
            role: binding.role,
            expected: binding.byte_len,
            actual: u64::MAX,
        })?;
    if actual != binding.byte_len {
        return Err(KagemushaArtifactErrorV1::LengthMismatch {
            role: binding.role,
            expected: binding.byte_len,
            actual,
        });
    }
    if <[u8; 32]>::from(Sha256::digest(bytes)) != binding.sha256 {
        return Err(KagemushaArtifactErrorV1::DigestMismatch(binding.role));
    }
    Ok(())
}

fn load_canonical_params<C>(
    parity: KagemushaPastaParityV1,
    bytes: &[u8],
) -> Result<ParamsIPA<C>, KagemushaArtifactErrorV1>
where
    C: CurveAffine,
{
    if bytes.len() != usize::try_from(KAGEMUSHA_PARAMS_BYTES_V1).expect("parameter size fits") {
        return Err(KagemushaArtifactErrorV1::NonCanonicalParameters(parity));
    }
    let params = ParamsIPA::<C>::new(KAGEMUSHA_HALO2_K_V1);
    let mut canonical = Vec::with_capacity(bytes.len());
    params
        .write(&mut canonical)
        .map_err(|_| KagemushaArtifactErrorV1::NonCanonicalParameters(parity))?;
    if canonical.as_slice() != bytes {
        return Err(KagemushaArtifactErrorV1::NonCanonicalParameters(parity));
    }
    Ok(params)
}

fn lower_hex(bytes: DigestV1) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

const _: () = {
    assert!(KAGEMUSHA_HALO2_K_V1 == 16);
    assert!(KAGEMUSHA_PARAMS_BYTES_V1 == 4_194_372);
    assert!(KagemushaArtifactRoleV1::ALL.len() == 26);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_has_one_closed_descriptor() {
        for role in KagemushaArtifactRoleV1::ALL {
            let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
            assert_eq!(descriptor.role, role);
            assert_eq!(
                descriptor.family.is_none(),
                matches!(
                    role,
                    KagemushaArtifactRoleV1::ParamsEq | KagemushaArtifactRoleV1::ParamsEp
                )
            );
        }
    }

    #[test]
    fn memory_resolver_rechecks_length_and_digest() {
        let bytes: Arc<[u8]> = Arc::from([0x41_u8; 17]);
        let mut resolver = KagemushaMemoryArtifactResolverV1::default();
        let digest = resolver.insert(Arc::clone(&bytes));
        let binding = KagemushaArtifactBindingV1 {
            role: KagemushaArtifactRoleV1::StateVkEq,
            sha256: digest,
            byte_len: 17,
        };
        assert_eq!(
            resolver
                .resolve_bytes(binding)
                .expect("exact content address"),
            bytes
        );
        assert!(matches!(
            resolver.resolve_bytes(KagemushaArtifactBindingV1 {
                byte_len: 16,
                ..binding
            }),
            Err(KagemushaArtifactErrorV1::LengthMismatch { .. })
        ));
        assert!(matches!(
            resolver.resolve_bytes(KagemushaArtifactBindingV1 {
                sha256: [0x11; 32],
                ..binding
            }),
            Err(KagemushaArtifactErrorV1::Missing(_))
        ));
    }

    #[test]
    fn content_address_filename_is_exact_lowercase_sha256() {
        assert_eq!(
            lower_hex([0xab; 32]),
            "abababababababababababababababababababababababababababababababab"
        );
    }
}
