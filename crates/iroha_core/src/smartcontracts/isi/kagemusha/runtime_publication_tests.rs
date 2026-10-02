//! Native local-cache validation and strict genuine-artifact publication test inputs.

use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1, KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
    KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1, KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
};
use std::path::PathBuf;

/// Test custody of exact authenticated originals, never fabricated recursive verifier keys.
pub(crate) struct OriginalBundle {
    pub(crate) manifest: KagemushaReleaseManifestV1,
    pub(crate) receipt: KagemushaInternalValidationReceiptV1,
    pub(crate) policy: KagemushaReleaseAuthorityPolicyV1,
    pub(crate) attestation: KagemushaReleaseAttestationV1,
    release: Arc<KagemushaAuthenticatedReleaseV1>,
    profile: KagemushaRecursiveVerifierProfileV1,
    artifacts: PathBuf,
}
impl OriginalBundle {
    /// Authenticate a complete native release bundle selected only by an ignored native test.
    pub(crate) fn read(root: &Path) -> Self {
        fn bounded(root: &Path, name: &str, limit: usize) -> Vec<u8> {
            let path = root.join(name);
            use std::io::Read as _;
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .unwrap_or_else(|error| {
                    panic!("required genuine artifact {}: {error}", path.display())
                })
                .take(u64::try_from(limit).unwrap() + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            assert!(
                !bytes.is_empty() && bytes.len() <= limit,
                "bounded genuine input {name}"
            );
            bytes
        }
        let manifest = KagemushaReleaseManifestV1::decode_canonical_exact(&bounded(
            root,
            "manifest.norito",
            KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
        ))
        .unwrap();
        let receipt = KagemushaInternalValidationReceiptV1::decode_canonical_exact(&bounded(
            root,
            "validation-receipt.norito",
            KAGEMUSHA_INTERNAL_VALIDATION_RECEIPT_MAX_BYTES_V1,
        ))
        .unwrap();
        let policy = KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(&bounded(
            root,
            "authority-policy.norito",
            KAGEMUSHA_RELEASE_AUTHORITY_POLICY_MAX_BYTES_V1,
        ))
        .unwrap();
        let attestation = KagemushaReleaseAttestationV1::decode_canonical_exact(&bounded(
            root,
            "attestation.norito",
            KAGEMUSHA_RELEASE_ATTESTATION_MAX_BYTES_V1,
        ))
        .unwrap();
        let profile: KagemushaRecursiveVerifierProfileFileV1 = norito::json::from_slice(&bounded(
            root,
            "recursive-profile.json",
            KAGEMUSHA_RECURSIVE_PROFILE_MAX_BYTES_V1,
        ))
        .unwrap();
        let release = Arc::new(
            manifest
                .authenticate(&receipt, &policy, &attestation)
                .unwrap(),
        );
        assert_eq!(release.purpose(), KagemushaReleasePurposeV1::Production);
        Self {
            manifest,
            receipt,
            policy,
            attestation,
            release,
            profile: profile.try_into_profile().unwrap(),
            artifacts: root.join("artifacts"),
        }
    }
    /// Use the actual original-key loader, including complete profile/protocol authentication.
    pub(crate) fn load(&self) -> AuthenticatedKagemushaV1RuntimeVerifier {
        AuthenticatedKagemushaV1RuntimeVerifier::from_authenticated_release(
            self.release.clone(),
            &self.artifacts,
            self.profile.clone(),
        )
        .expect("actual complete native artifact and recursive key authentication")
    }
    /// Add a complete second release through the real multi-release loader.
    pub(crate) fn install_into(&self, runtime: &mut AuthenticatedKagemushaV1RuntimeVerifier) {
        runtime
            .install_authenticated_release(
                self.release.clone(),
                &self.artifacts,
                self.profile.clone(),
            )
            .expect("actual second authenticated release and keys")
    }
    /// Exact signed release identity.
    pub(crate) fn release_id(&self) -> [u8; 32] {
        self.release.release_id()
    }
}

/// Exercise the same gated transaction entry used immediately before recursive verification.
pub(crate) fn check_transaction(
    tx: &mut crate::state::StateTransaction<'_, '_>,
    release: [u8; 32],
    eligible: bool,
    available: bool,
) {
    for operation in [
        execution_availability::Operation::TopUp,
        execution_availability::Operation::Redemption,
    ] {
        let result = super::isi::require_execution_runtime(tx, release, operation);
        if eligible && available {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
        if eligible && !available {
            assert_eq!(
                tx.execution_deferral().unwrap().reason(),
                ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable
            );
        } else {
            assert!(tx.execution_deferral().is_none());
        }
    }
}

fn test_network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"local runtime publication",
    )))
}

#[test]
fn local_publication_cache_refuses_unknown_or_incomplete_implementations() {
    assert_eq!(
        crate::test_allocations::allocations_during(|| {
            validate_runtime_cache_for_publication(
                &RejectAllKagemushaV1RuntimeVerifier,
                test_network(),
            )
            .unwrap()
        }),
        0
    );
    assert!(validate_runtime_cache_for_publication(&0_u8, test_network()).is_err());
    let empty = AuthenticatedKagemushaV1RuntimeVerifier {
        releases: BTreeMap::new(),
        lifecycle: KagemushaVerifierReleaseLifecycleV1::default(),
    };
    assert!(validate_runtime_cache_for_publication(&empty, test_network()).is_err());
}

#[test]
fn malformed_local_cache_never_publishes_original_state_effects() {
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let before = state.world.kagemusha_verifier_registry.view().get().clone();
    let mut block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::MIN,
        None,
        None,
        0,
        0,
    ));
    // Deliberately malformed local input reaches the real State publication boundary.
    block.kagemusha_v1_runtime_verifier = Arc::new(AuthenticatedKagemushaV1RuntimeVerifier {
        releases: BTreeMap::new(),
        lifecycle: KagemushaVerifierReleaseLifecycleV1::default(),
    });
    assert!(matches!(
        block.commit_empty_block_for_testing(),
        Err(crate::state::storage_transactions::TransactionsBlockError::KagemushaVerifierAuthority)
    ));
    assert!(state.latest_block_hash_fast().is_none());
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &before
    );
}
