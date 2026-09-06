//! Content-addressed artifact resolution for the authenticated Kagemusha V1 release.
//!
//! The threshold-authenticated release manifest is the only role registry. Artifact files are
//! deliberately unframed: IPA parameters use Halo2's canonical `ParamsIPA::write` bytes and keys
//! use `SerdeFormat::Processed`. This avoids a second, potentially divergent role header and keeps
//! the manifest's `(role, sha256, byte_len)` tuple authoritative.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Read as _},
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
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
    DigestV1, KagemushaPastaParityV1, KagemushaRecursionArtifactsV1, KagemushaRecursionErrorV1,
    KagemushaRecursiveVerifierV1, VerifiedKagemushaRedemptionProofV1,
    verify_kagemusha_redemption_request_v1,
};

const ARTIFACT_STREAM_BUFFER_BYTES_V1: usize = 64 * 1024;

/// One canonical transparent parameter derivation and its exact wire encoding.
///
/// Parameter roles are release-authenticated on every load. Caching only avoids repeating the
/// deterministic hash-to-curve derivation and serialization used for the final canonical-byte
/// comparison; callers still receive an owned parameter set.
struct KagemushaCanonicalParamsCacheV1<C: CurveAffine> {
    params: ParamsIPA<C>,
    bytes: Box<[u8]>,
}

fn build_canonical_params_cache_v1<C>() -> io::Result<KagemushaCanonicalParamsCacheV1<C>>
where
    C: CurveAffine,
{
    let params = ParamsIPA::<C>::new(KAGEMUSHA_HALO2_K_V1);
    if params.k() != KAGEMUSHA_HALO2_K_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "canonical IPA parameter degree changed",
        ));
    }
    let expected_len =
        usize::try_from(KAGEMUSHA_PARAMS_BYTES_V1).expect("parameter size fits usize");
    let mut bytes = Vec::with_capacity(expected_len);
    params.write(&mut bytes)?;
    if bytes.len() != expected_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "canonical IPA parameter encoding length changed",
        ));
    }
    Ok(KagemushaCanonicalParamsCacheV1 {
        params,
        bytes: bytes.into_boxed_slice(),
    })
}

fn canonical_eq_params_cache_v1()
-> Result<&'static KagemushaCanonicalParamsCacheV1<EqAffine>, KagemushaArtifactErrorV1> {
    static CACHE: OnceLock<Result<KagemushaCanonicalParamsCacheV1<EqAffine>, ()>> = OnceLock::new();
    CACHE
        .get_or_init(|| build_canonical_params_cache_v1().map_err(|_| ()))
        .as_ref()
        .map_err(|()| KagemushaArtifactErrorV1::NonCanonicalParameters(KagemushaPastaParityV1::Eq))
}

fn canonical_ep_params_cache_v1()
-> Result<&'static KagemushaCanonicalParamsCacheV1<EpAffine>, KagemushaArtifactErrorV1> {
    static CACHE: OnceLock<Result<KagemushaCanonicalParamsCacheV1<EpAffine>, ()>> = OnceLock::new();
    CACHE
        .get_or_init(|| build_canonical_params_cache_v1().map_err(|_| ()))
        .as_ref()
        .map_err(|()| KagemushaArtifactErrorV1::NonCanonicalParameters(KagemushaPastaParityV1::Ep))
}

/// Logical circuit family selected by one authenticated artifact role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCircuitFamilyV1 {
    /// Private recursive aggregate-state carrier.
    InnerState,
    /// Compact public transport decider for the aggregate state.
    State,
    /// Compact outer recipient hardware authorization checked before reserve mutation.
    MintAuthorization,
    /// Compact outer finalized reserve-mint receipt and consensus-finality helper.
    MintCredit,
    /// Provider-neutral hardware credential helper.
    PlatformCredential,
    /// Normalized composition of every monetary hardware guard.
    GuardBundle,
    /// Release-reserved terminal candidate/commit binding family.
    TerminalAuthorization,
    /// Compact post-commit proof wrapping the complete terminal authorization.
    CommitWrapper,
    /// Inner recipient hardware authorization checked by the compact mint wrapper.
    InnerMintAuthorization,
    /// Inner finalized reserve-mint receipt and consensus-finality relation.
    InnerMintCredit,
    /// One-block `k = 12` mint-certificate SHA-256 compression relation.
    MintHashShard,
    /// Ordered recursive `k = 16` mint-hash completeness relation.
    MintHashClaim,
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
            CommitWrapper, GuardBundle, InnerMintAuthorization, InnerMintCredit, InnerState,
            MintAuthorization, MintCredit, MintHashClaim, MintHashShard, PlatformCredential, State,
            TerminalAuthorization,
        };
        use KagemushaPastaParityV1::{Ep, Eq};

        let parity = match role {
            Role::ParamsEq
            | Role::InnerStatePkEq
            | Role::InnerStateVkEq
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
            | Role::TerminalAuthorizationPkEq
            | Role::TerminalAuthorizationVkEq
            | Role::CommitWrapperPkEq
            | Role::CommitWrapperVkEq
            | Role::InnerMintAuthorizationPkEq
            | Role::InnerMintAuthorizationVkEq
            | Role::InnerMintCreditPkEq
            | Role::InnerMintCreditVkEq
            | Role::MintHashShardPkEq
            | Role::MintHashShardVkEq
            | Role::MintHashClaimPkEq
            | Role::MintHashClaimVkEq => Eq,
            Role::ParamsEp
            | Role::InnerStatePkEp
            | Role::InnerStateVkEp
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
            | Role::TerminalAuthorizationPkEp
            | Role::TerminalAuthorizationVkEp
            | Role::CommitWrapperPkEp
            | Role::CommitWrapperVkEp
            | Role::InnerMintAuthorizationPkEp
            | Role::InnerMintAuthorizationVkEp
            | Role::InnerMintCreditPkEp
            | Role::InnerMintCreditVkEp
            | Role::MintHashShardPkEp
            | Role::MintHashShardVkEp
            | Role::MintHashClaimPkEp
            | Role::MintHashClaimVkEp => Ep,
        };
        let family = match role {
            Role::ParamsEq | Role::ParamsEp => None,
            Role::InnerStatePkEq
            | Role::InnerStateVkEq
            | Role::InnerStatePkEp
            | Role::InnerStateVkEp => Some(InnerState),
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
            Role::TerminalAuthorizationPkEq
            | Role::TerminalAuthorizationVkEq
            | Role::TerminalAuthorizationPkEp
            | Role::TerminalAuthorizationVkEp => Some(TerminalAuthorization),
            Role::CommitWrapperPkEq
            | Role::CommitWrapperVkEq
            | Role::CommitWrapperPkEp
            | Role::CommitWrapperVkEp => Some(CommitWrapper),
            Role::InnerMintAuthorizationPkEq
            | Role::InnerMintAuthorizationVkEq
            | Role::InnerMintAuthorizationPkEp
            | Role::InnerMintAuthorizationVkEp => Some(InnerMintAuthorization),
            Role::InnerMintCreditPkEq
            | Role::InnerMintCreditVkEq
            | Role::InnerMintCreditPkEp
            | Role::InnerMintCreditVkEp => Some(InnerMintCredit),
            Role::MintHashShardPkEq
            | Role::MintHashShardVkEq
            | Role::MintHashShardPkEp
            | Role::MintHashShardVkEp => Some(MintHashShard),
            Role::MintHashClaimPkEq
            | Role::MintHashClaimVkEq
            | Role::MintHashClaimPkEp
            | Role::MintHashClaimVkEp => Some(MintHashClaim),
        };
        let (kind, byte_limit) = match role {
            Role::ParamsEq | Role::ParamsEp => (Parameters, KAGEMUSHA_PARAMS_BYTES_V1),
            Role::InnerStatePkEq | Role::InnerStatePkEp | Role::StatePkEq | Role::StatePkEp => {
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
            | Role::TerminalAuthorizationPkEq
            | Role::TerminalAuthorizationPkEp
            | Role::CommitWrapperPkEq
            | Role::CommitWrapperPkEp
            | Role::InnerMintAuthorizationPkEq
            | Role::InnerMintAuthorizationPkEp
            | Role::InnerMintCreditPkEq
            | Role::InnerMintCreditPkEp
            | Role::MintHashShardPkEq
            | Role::MintHashShardPkEp
            | Role::MintHashClaimPkEq
            | Role::MintHashClaimPkEp => (ProvingKey, KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1),
            Role::InnerStateVkEq
            | Role::InnerStateVkEp
            | Role::StateVkEq
            | Role::StateVkEp
            | Role::MintAuthorizationVkEq
            | Role::MintAuthorizationVkEp
            | Role::MintCreditVkEq
            | Role::MintCreditVkEp
            | Role::PlatformCredentialVkEq
            | Role::PlatformCredentialVkEp
            | Role::GuardBundleVkEq
            | Role::GuardBundleVkEp
            | Role::TerminalAuthorizationVkEq
            | Role::TerminalAuthorizationVkEp
            | Role::CommitWrapperVkEq
            | Role::CommitWrapperVkEp
            | Role::InnerMintAuthorizationVkEq
            | Role::InnerMintAuthorizationVkEp
            | Role::InnerMintCreditVkEq
            | Role::InnerMintCreditVkEp
            | Role::MintHashShardVkEq
            | Role::MintHashShardVkEp
            | Role::MintHashClaimVkEq
            | Role::MintHashClaimVkEp => (VerifyingKey, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1),
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
    /// A decoder returned before consuming the complete authenticated artifact.
    #[error("Kagemusha V1 artifact {role:?} decoder left {remaining} authenticated bytes unread")]
    IncompleteRead {
        /// Role being decoded.
        role: KagemushaArtifactRoleV1,
        /// Authenticated bytes the decoder did not consume.
        remaining: u64,
    },
    /// The same opened stream contains bytes beyond its authenticated length.
    #[error("Kagemusha V1 artifact {0:?} has bytes beyond its authenticated length")]
    TrailingBytes(KagemushaArtifactRoleV1),
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

    /// Open one untrusted artifact stream without granting proof authority.
    ///
    /// The default preserves byte-based adapters. File-backed adapters may override this to
    /// avoid retaining the complete encoded key. Callers must authenticate the entire stream;
    /// [`KagemushaAuthenticatedArtifactSetV1::read_verified`] performs that check before returning
    /// a decoded value. This method alone does not authenticate the returned reader.
    ///
    /// # Errors
    ///
    /// Returns an error when the content address is absent or cannot be opened.
    fn open_reader(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
        Ok(Box::new(io::Cursor::new(self.resolve_bytes(binding)?)))
    }
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
    fn open_reader(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
        KagemushaArtifactDescriptorV1::for_role(binding.role).validate_binding(binding)?;
        let file = fs::File::open(self.path_for_digest(binding.sha256)).map_err(|error| {
            KagemushaArtifactErrorV1::Read {
                role: binding.role,
                reason: error.to_string(),
            }
        })?;
        // Metadata and all subsequent reads concern this one handle. Reopening the path after
        // validation would permit a content-address replacement between validation and parsing.
        let metadata = file
            .metadata()
            .map_err(|error| KagemushaArtifactErrorV1::Read {
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
        Ok(Box::new(io::BufReader::with_capacity(
            ARTIFACT_STREAM_BUFFER_BYTES_V1,
            file,
        )))
    }

    fn resolve_bytes(
        &self,
        binding: KagemushaArtifactBindingV1,
    ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
        let reader = self.open_reader(binding)?;
        decode_verified_artifact_v1(binding, reader, |reader| {
            let mut bytes = Vec::new();
            reader
                .read_to_end(&mut bytes)
                .map_err(|error| KagemushaArtifactErrorV1::Read {
                    role: binding.role,
                    reason: error.to_string(),
                })?;
            Ok(Arc::from(bytes))
        })
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
    /// This validates the one release-wide proof suite, all 50 role/length bindings, and the
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

    /// Decode one exact artifact and authenticate its stream before returning the value.
    ///
    /// The decoder sees at most the authenticated byte length and must consume it exactly.
    /// Successful parsing alone is insufficient: the same stream is checked for trailing bytes
    /// and its incremental SHA-256 must match the release binding. No complete encoded-file
    /// allocation is needed when the resolver implements streaming reads.
    ///
    /// The decoder must use shape-bounded parsing and must not publish or use a partially decoded
    /// object outside the closure. A decoded object becomes usable only after this method returns
    /// `Ok`. Storage adapters and their readers remain untrusted.
    ///
    /// # Errors
    ///
    /// Returns the decoder's error, or an artifact error for invalid bindings, read failures,
    /// incomplete consumption, truncation, trailing bytes, or a content-address mismatch.
    pub fn read_verified<T, E>(
        &self,
        role: KagemushaArtifactRoleV1,
        decode: impl FnOnce(&mut dyn io::Read) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<KagemushaArtifactErrorV1>,
    {
        let binding = self.binding(role);
        KagemushaArtifactDescriptorV1::for_role(role)
            .validate_binding(binding)
            .map_err(E::from)?;
        let reader = self.resolver.open_reader(binding).map_err(E::from)?;
        decode_verified_artifact_v1(binding, reader, decode)
    }

    /// Construct a storage-only fixture without claiming threshold release authentication.
    #[cfg(test)]
    pub(super) fn for_stream_tests(resolver: R, binding: KagemushaArtifactBindingV1) -> Self {
        tests::stream_test_set(resolver, binding)
    }

    /// Load and authenticate the deterministic Eq/Vesta `k = 16` IPA parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when storage bytes differ from Halo2's sole transparent derivation.
    pub fn load_eq_params(&self) -> Result<ParamsIPA<EqAffine>, KagemushaArtifactErrorV1> {
        let bytes = self.resolve(KagemushaArtifactRoleV1::ParamsEq)?;
        load_canonical_params(
            KagemushaPastaParityV1::Eq,
            bytes.as_ref(),
            canonical_eq_params_cache_v1()?,
        )
    }

    /// Load and authenticate the deterministic Ep/Pallas `k = 16` IPA parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when storage bytes differ from Halo2's sole transparent derivation.
    pub fn load_ep_params(&self) -> Result<ParamsIPA<EpAffine>, KagemushaArtifactErrorV1> {
        let bytes = self.resolve(KagemushaArtifactRoleV1::ParamsEp)?;
        load_canonical_params(
            KagemushaPastaParityV1::Ep,
            bytes.as_ref(),
            canonical_ep_params_cache_v1()?,
        )
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

/// Bounds and hashes decoder reads, retaining any fatal I/O failure until finalization.
struct KagemushaArtifactStreamV1 {
    binding: KagemushaArtifactBindingV1,
    reader: Box<dyn io::Read + Send>,
    remaining: u64,
    hash: Sha256,
    failure: Option<KagemushaArtifactErrorV1>,
}

impl io::Read for KagemushaArtifactStreamV1 {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // Empty reads do not probe EOF or poison an otherwise valid stream.
        if buffer.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let limit = buffer
            .len()
            .min(usize::try_from(self.remaining).unwrap_or(usize::MAX));
        let count = match self.reader.read(&mut buffer[..limit]) {
            Ok(count) if count <= limit => count,
            Ok(_) => {
                let error = io::Error::new(
                    io::ErrorKind::InvalidData,
                    "artifact reader reported more bytes than its supplied buffer",
                );
                self.failure
                    .get_or_insert_with(|| KagemushaArtifactErrorV1::Read {
                        role: self.binding.role,
                        reason: error.to_string(),
                    });
                return Err(error);
            }
            Err(error) => {
                // Interrupted reads are retryable under the Read contract. Other failures may
                // not be hidden by a decoder that catches an error and subsequently returns Ok.
                if error.kind() != io::ErrorKind::Interrupted {
                    self.failure
                        .get_or_insert_with(|| KagemushaArtifactErrorV1::Read {
                            role: self.binding.role,
                            reason: error.to_string(),
                        });
                }
                return Err(error);
            }
        };
        if count == 0 {
            self.failure
                .get_or_insert(KagemushaArtifactErrorV1::LengthMismatch {
                    role: self.binding.role,
                    expected: self.binding.byte_len,
                    actual: self.binding.byte_len - self.remaining,
                });
            return Ok(0);
        }
        let count_u64 = u64::try_from(count).map_err(|_| {
            let error = io::Error::new(
                io::ErrorKind::InvalidData,
                "artifact read count exceeds u64",
            );
            self.failure
                .get_or_insert_with(|| KagemushaArtifactErrorV1::Read {
                    role: self.binding.role,
                    reason: error.to_string(),
                });
            error
        })?;
        self.hash.update(&buffer[..count]);
        self.remaining -= count_u64;
        Ok(count)
    }
}

impl KagemushaArtifactStreamV1 {
    fn finish(mut self) -> Result<(), KagemushaArtifactErrorV1> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if self.remaining != 0 {
            return Err(KagemushaArtifactErrorV1::IncompleteRead {
                role: self.binding.role,
                remaining: self.remaining,
            });
        }
        let mut extra = [0_u8; 1];
        loop {
            match self.reader.read(&mut extra) {
                Ok(0) => break,
                Ok(_) => return Err(KagemushaArtifactErrorV1::TrailingBytes(self.binding.role)),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    return Err(KagemushaArtifactErrorV1::Read {
                        role: self.binding.role,
                        reason: error.to_string(),
                    });
                }
            }
        }
        if <[u8; 32]>::from(self.hash.finalize()) != self.binding.sha256 {
            return Err(KagemushaArtifactErrorV1::DigestMismatch(self.binding.role));
        }
        Ok(())
    }
}

fn decode_verified_artifact_v1<T, E>(
    binding: KagemushaArtifactBindingV1,
    reader: Box<dyn io::Read + Send>,
    decode: impl FnOnce(&mut dyn io::Read) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<KagemushaArtifactErrorV1>,
{
    KagemushaArtifactDescriptorV1::for_role(binding.role)
        .validate_binding(binding)
        .map_err(E::from)?;
    let mut stream = KagemushaArtifactStreamV1 {
        binding,
        reader,
        remaining: binding.byte_len,
        hash: Sha256::new(),
        failure: None,
    };
    let decoded = decode(&mut stream)?;
    stream.finish().map_err(E::from)?;
    Ok(decoded)
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
    canonical: &KagemushaCanonicalParamsCacheV1<C>,
) -> Result<ParamsIPA<C>, KagemushaArtifactErrorV1>
where
    C: CurveAffine,
{
    if bytes.len() != usize::try_from(KAGEMUSHA_PARAMS_BYTES_V1).expect("parameter size fits") {
        return Err(KagemushaArtifactErrorV1::NonCanonicalParameters(parity));
    }
    if canonical.params.k() != KAGEMUSHA_HALO2_K_V1 || canonical.bytes.as_ref() != bytes {
        return Err(KagemushaArtifactErrorV1::NonCanonicalParameters(parity));
    }
    Ok(canonical.params.clone())
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
    assert!(KagemushaArtifactRoleV1::ALL.len() == 50);
};

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_binding(bytes: &[u8]) -> KagemushaArtifactBindingV1 {
        KagemushaArtifactBindingV1 {
            role: KagemushaArtifactRoleV1::StateVkEq,
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }

    fn read_stream_bytes(reader: &mut dyn io::Read) -> Result<Vec<u8>, KagemushaArtifactErrorV1> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|error| KagemushaArtifactErrorV1::Read {
                role: KagemushaArtifactRoleV1::StateVkEq,
                reason: error.to_string(),
            })?;
        Ok(bytes)
    }

    /// Storage-only shape fixture; unused monetary identities do not authenticate a release.
    /// Production construction still requires `new` and its sealed release.
    pub(super) fn stream_test_set<R: KagemushaArtifactByteResolverV1>(
        resolver: R,
        binding: KagemushaArtifactBindingV1,
    ) -> KagemushaAuthenticatedArtifactSetV1<R> {
        let role_binding = |role| KagemushaArtifactBindingV1 { role, ..binding };
        let recursion = KagemushaRecursionArtifactsV1 {
            release_id: [1; 32],
            profile_digest: [2; 32],
            eq_protocol_digest: [3; 32],
            ep_protocol_digest: [4; 32],
            terminal_authorization_eq_protocol_digest: [5; 32],
            terminal_authorization_ep_protocol_digest: [6; 32],
            commit_wrapper_eq_protocol_digest: [7; 32],
            commit_wrapper_ep_protocol_digest: [8; 32],
            mint_authorization_eq_protocol_digest: [9; 32],
            mint_authorization_ep_protocol_digest: [10; 32],
            mint_finality_eq_protocol_digest: [17; 32],
            mint_finality_ep_protocol_digest: [18; 32],
            guard_bundle_eq_protocol_digest: [11; 32],
            guard_bundle_ep_protocol_digest: [12; 32],
            mint_hash_shard_eq_protocol_digest: [19; 32],
            mint_hash_shard_ep_protocol_digest: [20; 32],
            mint_hash_claim_eq_protocol_digest: [21; 32],
            mint_hash_claim_ep_protocol_digest: [22; 32],
            guard_bundle_verifying_key_eq: role_binding(KagemushaArtifactRoleV1::GuardBundleVkEq),
            guard_bundle_verifying_key_ep: role_binding(KagemushaArtifactRoleV1::GuardBundleVkEp),
            terminal_authorization_verifying_key_eq: role_binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            ),
            terminal_authorization_verifying_key_ep: role_binding(
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEp,
            ),
            commit_wrapper_verifying_key_eq: role_binding(
                KagemushaArtifactRoleV1::CommitWrapperVkEq,
            ),
            commit_wrapper_verifying_key_ep: role_binding(
                KagemushaArtifactRoleV1::CommitWrapperVkEp,
            ),
            mint_finality: super::super::KagemushaMintFinalityArtifactsV1 {
                proving_key_eq: role_binding(KagemushaArtifactRoleV1::MintCreditPkEq),
                verifying_key_eq: role_binding(KagemushaArtifactRoleV1::MintCreditVkEq),
                proving_key_ep: role_binding(KagemushaArtifactRoleV1::MintCreditPkEp),
                verifying_key_ep: role_binding(KagemushaArtifactRoleV1::MintCreditVkEp),
            },
            artifact_manifest_digest: [13; 32],
            canonical_empty_effect_digest: [14; 32],
        };
        KagemushaAuthenticatedArtifactSetV1 {
            recursion,
            suite_id: [15; 32],
            vk_set_digest: [16; 32],
            bindings: std::array::from_fn(|index| {
                role_binding(KagemushaArtifactRoleV1::ALL[index])
            }),
            resolver,
        }
    }

    #[test]
    fn authenticated_set_read_verified_uses_stream_and_rechecks_untrusted_bytes() {
        struct StreamingOnlyResolver(Vec<u8>);
        impl KagemushaArtifactByteResolverV1 for StreamingOnlyResolver {
            fn resolve_bytes(
                &self,
                binding: KagemushaArtifactBindingV1,
            ) -> Result<Arc<[u8]>, KagemushaArtifactErrorV1> {
                Err(KagemushaArtifactErrorV1::Read {
                    role: binding.role,
                    reason: "whole-file byte resolution must not be used".to_owned(),
                })
            }

            fn open_reader(
                &self,
                _binding: KagemushaArtifactBindingV1,
            ) -> Result<Box<dyn io::Read + Send>, KagemushaArtifactErrorV1> {
                Ok(Box::new(io::Cursor::new(self.0.clone())))
            }
        }

        let binding = stream_binding(b"four");
        let valid = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(
            StreamingOnlyResolver(b"four".to_vec()),
            binding,
        );
        assert_eq!(
            valid
                .read_verified(binding.role, read_stream_bytes)
                .expect("authenticated streaming decode"),
            b"four"
        );
        let substituted = KagemushaAuthenticatedArtifactSetV1::for_stream_tests(
            StreamingOnlyResolver(b"fake".to_vec()),
            binding,
        );
        assert!(matches!(
            substituted.read_verified(binding.role, read_stream_bytes),
            Err(KagemushaArtifactErrorV1::DigestMismatch(_))
        ));
    }

    struct InterruptedChunkReader {
        bytes: io::Cursor<Vec<u8>>,
        first_error: Option<io::ErrorKind>,
        fail_at_eof: bool,
    }

    impl io::Read for InterruptedChunkReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            assert!(!buffer.is_empty(), "guard must not forward empty reads");
            if let Some(kind) = self.first_error.take() {
                return Err(io::Error::new(kind, "injected artifact read error"));
            }
            if self.fail_at_eof && self.bytes.position() == self.bytes.get_ref().len() as u64 {
                return Err(io::Error::other("injected final artifact probe failure"));
            }
            let count = buffer.len().min(2);
            self.bytes.read(&mut buffer[..count])
        }
    }

    #[test]
    fn object_safe_default_reader_preserves_byte_resolvers() {
        let bytes = b"authenticated artifact";
        let binding = stream_binding(bytes);
        let mut resolver = KagemushaMemoryArtifactResolverV1::default();
        resolver.insert(Arc::<[u8]>::from(bytes.as_slice()));
        let resolver: &dyn KagemushaArtifactByteResolverV1 = &resolver;
        let reader = resolver.open_reader(binding).expect("default reader");
        assert_eq!(
            decode_verified_artifact_v1(binding, reader, read_stream_bytes)
                .expect("complete authenticated stream"),
            bytes
        );
    }

    #[test]
    fn artifact_stream_handles_short_interrupted_and_empty_reads() {
        let bytes = b"authenticated artifact";
        let binding = stream_binding(bytes);
        let reader = InterruptedChunkReader {
            bytes: io::Cursor::new(bytes.to_vec()),
            first_error: Some(io::ErrorKind::Interrupted),
            fail_at_eof: false,
        };
        let decoded = decode_verified_artifact_v1(binding, Box::new(reader), |reader| {
            assert_eq!(reader.read(&mut []).expect("empty read"), 0);
            let bytes = read_stream_bytes(reader)?;
            assert_eq!(reader.read(&mut []).expect("empty read at bound"), 0);
            Ok::<_, KagemushaArtifactErrorV1>(bytes)
        })
        .expect("Interrupted is retryable and does not alter the digest");
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn artifact_stream_rejects_truncation_extra_bytes_and_incomplete_decoding() {
        let bytes = b"four";
        let binding = stream_binding(bytes);
        assert!(matches!(
            decode_verified_artifact_v1(
                binding,
                Box::new(io::Cursor::new(b"fou".to_vec())),
                read_stream_bytes,
            ),
            Err(KagemushaArtifactErrorV1::LengthMismatch {
                expected: 4,
                actual: 3,
                ..
            })
        ));
        assert!(matches!(
            decode_verified_artifact_v1(
                binding,
                Box::new(io::Cursor::new(b"fourmore".to_vec())),
                |reader| {
                    let decoded = read_stream_bytes(reader)?;
                    assert_eq!(
                        decoded, bytes,
                        "decoder cannot read the unauthenticated suffix"
                    );
                    Ok::<_, KagemushaArtifactErrorV1>(decoded)
                },
            ),
            Err(KagemushaArtifactErrorV1::TrailingBytes(_))
        ));
        assert!(matches!(
            decode_verified_artifact_v1(
                binding,
                Box::new(io::Cursor::new(bytes.to_vec())),
                |_reader| Ok::<_, KagemushaArtifactErrorV1>(()),
            ),
            Err(KagemushaArtifactErrorV1::IncompleteRead { remaining: 4, .. })
        ));
    }

    #[test]
    fn artifact_stream_drops_decoded_value_when_final_digest_fails() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct Decoded(Arc<AtomicBool>);
        impl Drop for Decoded {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let dropped = Arc::new(AtomicBool::new(false));
        let result = decode_verified_artifact_v1(
            stream_binding(b"four"),
            Box::new(io::Cursor::new(b"fake".to_vec())),
            |reader| {
                read_stream_bytes(reader)?;
                Ok::<_, KagemushaArtifactErrorV1>(Decoded(Arc::clone(&dropped)))
            },
        );
        assert!(matches!(
            result,
            Err(KagemushaArtifactErrorV1::DigestMismatch(_))
        ));
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn artifact_stream_cannot_hide_read_errors_or_failed_eof_probe() {
        let bytes = b"four";
        let binding = stream_binding(bytes);
        let reader = InterruptedChunkReader {
            bytes: io::Cursor::new(bytes.to_vec()),
            first_error: Some(io::ErrorKind::Other),
            fail_at_eof: false,
        };
        assert!(matches!(
            decode_verified_artifact_v1(binding, Box::new(reader), |reader| {
                assert!(reader.read(&mut [0; 1]).is_err());
                read_stream_bytes(reader)
            }),
            Err(KagemushaArtifactErrorV1::Read { .. })
        ));

        let reader = InterruptedChunkReader {
            bytes: io::Cursor::new(bytes.to_vec()),
            first_error: None,
            fail_at_eof: true,
        };
        assert!(matches!(
            decode_verified_artifact_v1(binding, Box::new(reader), read_stream_bytes),
            Err(KagemushaArtifactErrorV1::Read { .. })
        ));
    }

    #[test]
    fn artifact_stream_retries_interrupted_final_probe() {
        struct InterruptAtEof {
            bytes: io::Cursor<Vec<u8>>,
            interrupted: bool,
        }
        impl io::Read for InterruptAtEof {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if self.bytes.position() == self.bytes.get_ref().len() as u64 && !self.interrupted {
                    self.interrupted = true;
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "retry final probe",
                    ));
                }
                self.bytes.read(buffer)
            }
        }
        let bytes = b"four";
        let reader = InterruptAtEof {
            bytes: io::Cursor::new(bytes.to_vec()),
            interrupted: false,
        };
        assert_eq!(
            decode_verified_artifact_v1(stream_binding(bytes), Box::new(reader), read_stream_bytes)
                .expect("retry final probe on the same stream"),
            bytes
        );
    }

    #[test]
    fn artifact_stream_rejects_invalid_reader_counts_without_panicking() {
        struct InvalidCount;
        impl io::Read for InvalidCount {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                Ok(buffer.len() + 1)
            }
        }
        assert!(matches!(
            decode_verified_artifact_v1(
                stream_binding(b"four"),
                Box::new(InvalidCount),
                read_stream_bytes,
            ),
            Err(KagemushaArtifactErrorV1::Read { .. })
        ));
    }

    #[test]
    fn artifact_stream_preserves_decoder_errors() {
        #[derive(Debug, PartialEq, Eq)]
        enum DecodeError {
            Artifact(KagemushaArtifactErrorV1),
            InvalidKey,
        }
        impl From<KagemushaArtifactErrorV1> for DecodeError {
            fn from(error: KagemushaArtifactErrorV1) -> Self {
                Self::Artifact(error)
            }
        }
        assert_eq!(
            decode_verified_artifact_v1(
                stream_binding(b"four"),
                Box::new(io::Cursor::new(b"four".to_vec())),
                |_reader| Err::<(), _>(DecodeError::InvalidKey),
            ),
            Err(DecodeError::InvalidKey)
        );
    }

    #[test]
    fn directory_stream_rejects_length_changes_after_open() {
        let directory = tempfile::tempdir().expect("artifact directory");
        let resolver = KagemushaDirectoryArtifactResolverV1::new(directory.path())
            .expect("directory resolver");
        let bytes = b"four";
        let binding = stream_binding(bytes);
        let path = resolver.path_for_digest(binding.sha256);
        fs::write(&path, bytes).expect("write artifact");
        assert_eq!(
            resolver
                .resolve_bytes(binding)
                .expect("legacy byte API")
                .as_ref(),
            bytes
        );

        let reader = resolver.open_reader(binding).expect("open before append");
        fs::write(&path, b"fourmore").expect("append after opening");
        assert!(matches!(
            decode_verified_artifact_v1(binding, reader, read_stream_bytes),
            Err(KagemushaArtifactErrorV1::TrailingBytes(_))
        ));
        assert!(matches!(
            resolver.open_reader(binding),
            Err(KagemushaArtifactErrorV1::LengthMismatch { .. })
        ));

        fs::write(&path, bytes).expect("restore exact artifact");
        let reader = resolver
            .open_reader(binding)
            .expect("open before truncation");
        fs::write(&path, b"fou").expect("truncate opened artifact");
        assert!(matches!(
            decode_verified_artifact_v1(binding, reader, read_stream_bytes),
            Err(KagemushaArtifactErrorV1::LengthMismatch { actual: 3, .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn directory_stream_keeps_one_handle_when_content_address_is_replaced() {
        let directory = tempfile::tempdir().expect("artifact directory");
        let resolver = KagemushaDirectoryArtifactResolverV1::new(directory.path())
            .expect("directory resolver");
        let bytes = b"four";
        let binding = stream_binding(bytes);
        let path = resolver.path_for_digest(binding.sha256);
        fs::write(&path, bytes).expect("write original artifact");
        let reader = resolver.open_reader(binding).expect("open exact artifact");
        fs::rename(&path, directory.path().join("original-open-file"))
            .expect("replace the path without modifying the opened inode");
        fs::write(&path, b"fake").expect("write substituted path");
        assert_eq!(
            decode_verified_artifact_v1(binding, reader, read_stream_bytes)
                .expect("verify original opened bytes"),
            bytes
        );
        assert!(matches!(
            resolver.resolve_bytes(binding),
            Err(KagemushaArtifactErrorV1::DigestMismatch(_))
        ));
    }

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
        assert_eq!(
            KagemushaArtifactDescriptorV1::for_role(
                KagemushaArtifactRoleV1::TerminalAuthorizationVkEq,
            )
            .family,
            Some(KagemushaCircuitFamilyV1::TerminalAuthorization)
        );
        assert_eq!(
            KagemushaArtifactDescriptorV1::for_role(KagemushaArtifactRoleV1::CommitWrapperVkEq,)
                .family,
            Some(KagemushaCircuitFamilyV1::CommitWrapper)
        );
    }

    #[test]
    fn inner_mint_descriptors_bind_distinct_families_parities_and_key_bounds() {
        use KagemushaArtifactKindV1::{ProvingKey, VerifyingKey};
        use KagemushaArtifactRoleV1 as Role;
        use KagemushaCircuitFamilyV1 as Family;

        let families = [
            (
                Family::InnerMintAuthorization,
                Family::MintAuthorization,
                [
                    Role::InnerMintAuthorizationPkEq,
                    Role::InnerMintAuthorizationVkEq,
                    Role::InnerMintAuthorizationPkEp,
                    Role::InnerMintAuthorizationVkEp,
                ],
                [
                    Role::MintAuthorizationPkEq,
                    Role::MintAuthorizationVkEq,
                    Role::MintAuthorizationPkEp,
                    Role::MintAuthorizationVkEp,
                ],
            ),
            (
                Family::InnerMintCredit,
                Family::MintCredit,
                [
                    Role::InnerMintCreditPkEq,
                    Role::InnerMintCreditVkEq,
                    Role::InnerMintCreditPkEp,
                    Role::InnerMintCreditVkEp,
                ],
                [
                    Role::MintCreditPkEq,
                    Role::MintCreditVkEq,
                    Role::MintCreditPkEp,
                    Role::MintCreditVkEp,
                ],
            ),
        ];
        for (family_index, (inner_family, outer_family, inner_roles, outer_roles)) in
            families.into_iter().enumerate()
        {
            for (index, (role, outer_role)) in inner_roles.into_iter().zip(outer_roles).enumerate()
            {
                let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
                let outer_descriptor = KagemushaArtifactDescriptorV1::for_role(outer_role);
                let parity = if index < 2 {
                    KagemushaPastaParityV1::Eq
                } else {
                    KagemushaPastaParityV1::Ep
                };
                let (kind, byte_limit) = if index % 2 == 0 {
                    (ProvingKey, KAGEMUSHA_HELPER_PROVING_KEY_MAX_BYTES_V1)
                } else {
                    (VerifyingKey, KAGEMUSHA_VERIFYING_KEY_MAX_BYTES_V1)
                };
                assert_eq!(role_index(role), 34 + family_index * 4 + index);
                assert_eq!(descriptor.family, Some(inner_family));
                assert_eq!(outer_descriptor.family, Some(outer_family));
                assert_ne!(descriptor.family, outer_descriptor.family);
                assert_eq!(descriptor.parity, parity);
                assert_eq!(descriptor.kind, kind);
                assert_eq!(descriptor.byte_limit, byte_limit);
                let binding = KagemushaArtifactBindingV1 {
                    role,
                    sha256: [1; 32],
                    byte_len: byte_limit,
                };
                assert!(descriptor.validate_binding(binding).is_ok());
                for byte_len in [0, byte_limit + 1] {
                    assert_eq!(
                        descriptor.validate_binding(KagemushaArtifactBindingV1 {
                            byte_len,
                            ..binding
                        }),
                        Err(KagemushaArtifactErrorV1::InvalidBinding(role))
                    );
                }
                for wrong_role in [outer_role, inner_roles[(index + 1) % inner_roles.len()]] {
                    assert_eq!(
                        descriptor.validate_binding(KagemushaArtifactBindingV1 {
                            role: wrong_role,
                            ..binding
                        }),
                        Err(KagemushaArtifactErrorV1::InvalidBinding(role))
                    );
                }
            }
        }
    }

    #[test]
    fn mint_hash_descriptors_are_distinct_release_authorities() {
        use KagemushaArtifactKindV1::{ProvingKey, VerifyingKey};
        use KagemushaArtifactRoleV1 as Role;
        use KagemushaCircuitFamilyV1 as Family;

        for (family, roles) in [
            (
                Family::MintHashShard,
                [
                    Role::MintHashShardPkEq,
                    Role::MintHashShardVkEq,
                    Role::MintHashShardPkEp,
                    Role::MintHashShardVkEp,
                ],
            ),
            (
                Family::MintHashClaim,
                [
                    Role::MintHashClaimPkEq,
                    Role::MintHashClaimVkEq,
                    Role::MintHashClaimPkEp,
                    Role::MintHashClaimVkEp,
                ],
            ),
        ] {
            for (index, role) in roles.into_iter().enumerate() {
                let descriptor = KagemushaArtifactDescriptorV1::for_role(role);
                assert_eq!(descriptor.family, Some(family));
                assert_eq!(
                    descriptor.parity,
                    if index < 2 {
                        KagemushaPastaParityV1::Eq
                    } else {
                        KagemushaPastaParityV1::Ep
                    }
                );
                assert_eq!(
                    descriptor.kind,
                    if index % 2 == 0 {
                        ProvingKey
                    } else {
                        VerifyingKey
                    }
                );
            }
        }
        assert_ne!(
            KagemushaArtifactDescriptorV1::for_role(Role::MintHashShardVkEq).family,
            KagemushaArtifactDescriptorV1::for_role(Role::MintHashClaimVkEq).family
        );
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
