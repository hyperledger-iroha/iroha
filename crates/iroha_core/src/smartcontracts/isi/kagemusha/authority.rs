//! Canonical semantic projection of the installed Kagemusha verifier runtime.
//!
//! The projection records authenticated release identities and lifecycle
//! decisions, never local artifact paths, trait-object addresses, or loaded key
//! allocations. An authenticated local runtime must match the independently
//! finalized release authority before it can replace the fail-closed verifier.
//! State publication also accepts that built-in reject-all runtime while local
//! artifacts are unavailable; it never grants monetary authority. Its commit
//! check reads the borrowed runtime directly.

use std::any::Any;

use iroha_data_model::kagemusha::{
    KagemushaGovernedVerifierRegistryV1, KagemushaGovernedVerifierReleaseV1,
};
use norito::{Decode, Encode, NoritoSchema};

use super::{
    AuthenticatedKagemushaV1RuntimeVerifier, KagemushaVerifierReleaseStatusV1,
    RejectAllKagemushaV1RuntimeVerifier,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:kagemusha-verifier-release:v1")]
struct KagemushaVerifierReleaseAuthorityV1 {
    release_id: [u8; 32],
    status: u8,
    profile_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
    receipt_digest: [u8; 32],
    attestation_digest: [u8; 32],
    authority_policy_digest: [u8; 32],
    hardware_policy_digest: [u8; 32],
    native_profile_digest: [u8; 32],
    provider_policy_root: [u8; 32],
    suite_id: [u8; 32],
    vk_set_digest: [u8; 32],
}

/// Canonical, path-free runtime policy that can be compared with governed State.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:kagemusha-verifier-authority:v1")]
pub(crate) struct KagemushaVerifierAuthorityV1 {
    /// Zero is the fail-closed runtime; one is the authenticated release registry.
    mode: u8,
    active_release_id: Option<[u8; 32]>,
    releases: Vec<KagemushaVerifierReleaseAuthorityV1>,
}

fn status_tag(status: KagemushaVerifierReleaseStatusV1) -> u8 {
    match status {
        KagemushaVerifierReleaseStatusV1::Active => 1,
        KagemushaVerifierReleaseStatusV1::Standby => 2,
        KagemushaVerifierReleaseStatusV1::VerificationOnly => 3,
    }
}

fn release_matches_governed(
    local: &KagemushaVerifierReleaseAuthorityV1,
    governed: &KagemushaGovernedVerifierReleaseV1,
) -> bool {
    local.release_id == governed.release_id
        && local.status == governed.status
        && local.profile_digest == governed.profile_digest
        && local.artifact_manifest_digest == governed.artifact_manifest_digest
        && local.receipt_digest == governed.receipt_digest
        && local.attestation_digest == governed.attestation_digest
        && local.authority_policy_digest == governed.authority_policy_digest
        && local.hardware_policy_digest == governed.hardware_policy_digest
        && local.native_profile_digest == governed.native_profile_digest
        && local.provider_policy_root == governed.provider_policy_root
        && local.suite_id == governed.suite_id
        && local.vk_set_digest == governed.vk_set_digest
}

impl AuthenticatedKagemushaV1RuntimeVerifier {
    fn semantic_authority(&self) -> Result<KagemushaVerifierAuthorityV1, String> {
        self.lifecycle.validate()?;
        if self.releases.is_empty() || self.releases.len() != self.lifecycle.statuses.len() {
            return Err("Kagemusha verifier release and lifecycle sets differ".to_owned());
        }
        let mut releases = Vec::new();
        releases
            .try_reserve_exact(self.releases.len())
            .map_err(|_| "Kagemusha verifier authority allocation unavailable".to_owned())?;
        for (release_id, runtime) in &self.releases {
            let status = self
                .lifecycle
                .status(*release_id)
                .ok_or_else(|| "Kagemusha verifier release has no lifecycle status".to_owned())?;
            let artifacts = runtime.artifacts.recursion_artifacts();
            if artifacts.release_id != *release_id {
                return Err("Kagemusha verifier artifact release identity differs".to_owned());
            }
            releases.push(KagemushaVerifierReleaseAuthorityV1 {
                release_id: *release_id,
                status: status_tag(status),
                profile_digest: artifacts.profile_digest,
                artifact_manifest_digest: artifacts.artifact_manifest_digest,
                receipt_digest: runtime.release_receipt_digest,
                attestation_digest: runtime.release_attestation_digest,
                authority_policy_digest: runtime.release_authority_policy_digest,
                hardware_policy_digest: runtime.release_hardware_policy_digest,
                native_profile_digest: runtime.artifacts.native_profile_digest(),
                provider_policy_root: runtime.artifacts.provider_policy_root(),
                suite_id: runtime.artifacts.suite_id(),
                vk_set_digest: runtime.artifacts.vk_set_digest(),
            });
        }
        Ok(KagemushaVerifierAuthorityV1 {
            mode: 1,
            active_release_id: self.lifecycle.active_release_id,
            releases,
        })
    }
}

/// Project only the two built-in runtime implementations; unknown local code
/// cannot claim consensus authority by reporting plausible release identifiers.
pub(crate) fn runtime_verifier_authority(
    verifier: &dyn Any,
) -> Result<KagemushaVerifierAuthorityV1, String> {
    if verifier.is::<RejectAllKagemushaV1RuntimeVerifier>() {
        return Ok(KagemushaVerifierAuthorityV1 {
            mode: 0,
            active_release_id: None,
            releases: Vec::new(),
        });
    }
    verifier
        .downcast_ref::<AuthenticatedKagemushaV1RuntimeVerifier>()
        .ok_or_else(|| "unrecognized Kagemusha verifier runtime cannot enter State".to_owned())?
        .semantic_authority()
}

/// Check local authenticated artifacts against independently finalized release authority.
pub(crate) fn runtime_matches_governed_registry(
    verifier: &dyn Any,
    registry: &KagemushaGovernedVerifierRegistryV1,
) -> Result<(), String> {
    registry.validate().map_err(str::to_owned)?;
    // A locally unavailable verifier never grants monetary authority. Keeping
    // RejectAll valid across certified activation permits every peer to replay
    // the same State transition before its local artifacts are loaded.
    if verifier.is::<RejectAllKagemushaV1RuntimeVerifier>() {
        return Ok(());
    }
    if registry.active_release_id.is_none() {
        return if verifier.is::<AuthenticatedKagemushaV1RuntimeVerifier>() {
            Err("local KAGEMUSHA verifier has no active finalized release authority".to_owned())
        } else {
            Err("unrecognized Kagemusha verifier runtime cannot enter State".to_owned())
        };
    }
    let runtime = verifier
        .downcast_ref::<AuthenticatedKagemushaV1RuntimeVerifier>()
        .ok_or_else(|| {
            "local KAGEMUSHA verifier lifecycle differs from finalized authority".to_owned()
        })?;
    runtime.lifecycle.validate()?;
    if runtime.releases.is_empty()
        || runtime.releases.len() != runtime.lifecycle.statuses.len()
        || runtime.lifecycle.active_release_id != registry.active_release_id
        || runtime.releases.len() != registry.releases.len()
    {
        return Err(
            "local KAGEMUSHA verifier lifecycle differs from finalized authority".to_owned(),
        );
    }
    for ((release_id, local), governed) in runtime.releases.iter().zip(&registry.releases) {
        let status = runtime
            .lifecycle
            .status(*release_id)
            .ok_or_else(|| "Kagemusha verifier release has no lifecycle status".to_owned())?;
        let artifacts = local.artifacts.recursion_artifacts();
        let local_identity = KagemushaVerifierReleaseAuthorityV1 {
            release_id: *release_id,
            status: status_tag(status),
            profile_digest: artifacts.profile_digest,
            artifact_manifest_digest: artifacts.artifact_manifest_digest,
            receipt_digest: local.release_receipt_digest,
            attestation_digest: local.release_attestation_digest,
            authority_policy_digest: local.release_authority_policy_digest,
            hardware_policy_digest: local.release_hardware_policy_digest,
            native_profile_digest: local.artifacts.native_profile_digest(),
            provider_policy_root: local.artifacts.provider_policy_root(),
            suite_id: local.artifacts.suite_id(),
            vk_set_digest: local.artifacts.vk_set_digest(),
        };
        if artifacts.release_id != *release_id
            || !release_matches_governed(&local_identity, governed)
        {
            return Err(
                "local KAGEMUSHA verifier release differs from finalized authority".to_owned(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
// This test-only allocator observes the exact commit check without changing production code.
#[allow(unsafe_code)]
mod tests {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
        collections::BTreeMap,
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaReleaseAuthorityPolicyV1,
    };
    use norito::{decode_canonical, encode_canonical};

    use super::*;
    use crate::smartcontracts::isi::kagemusha::KagemushaVerifierReleaseLifecycleV1;

    thread_local! {
        static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
        static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    }

    struct CountingAllocator;

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn record_allocation() {
        let _ = TRACK_ALLOCATIONS.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
            }
        });
    }

    // SAFETY: all operations delegate to System with the original allocation layout.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_allocation();
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record_allocation();
            }
            pointer
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let result = unsafe { System.realloc(pointer, layout, size) };
            if !result.is_null() {
                record_allocation();
            }
            result
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }
    }

    fn allocations_during(f: impl FnOnce()) -> usize {
        ALLOCATION_COUNT.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|tracking| tracking.set(true));
        struct StopTracking;
        impl Drop for StopTracking {
            fn drop(&mut self) {
                TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
            }
        }
        let stop = StopTracking;
        f();
        drop(stop);
        ALLOCATION_COUNT.with(Cell::get)
    }

    #[test]
    fn status_tags_are_distinct_and_stable() {
        assert_eq!(status_tag(KagemushaVerifierReleaseStatusV1::Active), 1);
        assert_eq!(status_tag(KagemushaVerifierReleaseStatusV1::Standby), 2);
        assert_eq!(
            status_tag(KagemushaVerifierReleaseStatusV1::VerificationOnly),
            3
        );
    }

    #[test]
    fn reject_all_is_canonical_and_unknown_code_is_rejected() {
        let policy = runtime_verifier_authority(&RejectAllKagemushaV1RuntimeVerifier).unwrap();
        assert_eq!(policy.mode, 0);
        assert!(policy.releases.is_empty());
        let bytes = encode_canonical(&policy).unwrap();
        assert_eq!(
            decode_canonical::<KagemushaVerifierAuthorityV1>(&bytes).unwrap(),
            policy
        );
        assert!(runtime_verifier_authority(&0_u8).is_err());
    }

    #[test]
    fn local_verifier_requires_exact_finalized_registry() {
        let reject_all = RejectAllKagemushaV1RuntimeVerifier;
        let mut registry = KagemushaGovernedVerifierRegistryV1::default();
        runtime_matches_governed_registry(&reject_all, &registry).unwrap();
        assert!(runtime_matches_governed_registry(&0_u8, &registry).is_err());
        registry.version = 7;
        assert!(runtime_matches_governed_registry(&reject_all, &registry).is_err());
    }

    #[test]
    fn reject_all_commit_authority_check_uses_no_heap() {
        let reject_all = RejectAllKagemushaV1RuntimeVerifier;
        let mut registry = KagemushaGovernedVerifierRegistryV1::default();
        assert_eq!(
            allocations_during(
                || runtime_matches_governed_registry(&reject_all, &registry).unwrap()
            ),
            0
        );

        let signer = KeyPair::try_from_seed(vec![7; 32], Algorithm::Ed25519).unwrap();
        registry.authority_policy = Some(KagemushaReleaseAuthorityPolicyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            authority_set_id: [1; 32],
            threshold: 1,
            authorized_signers: vec![signer.public_key().clone()],
        });
        assert_eq!(
            allocations_during(
                || runtime_matches_governed_registry(&reject_all, &registry).unwrap()
            ),
            0
        );
    }

    #[test]
    fn empty_authenticated_registry_cannot_masquerade_as_reject_all() {
        let verifier = AuthenticatedKagemushaV1RuntimeVerifier {
            releases: BTreeMap::new(),
            lifecycle: KagemushaVerifierReleaseLifecycleV1::default(),
        };
        assert!(runtime_verifier_authority(&verifier).is_err());
        assert!(
            runtime_matches_governed_registry(
                &verifier,
                &KagemushaGovernedVerifierRegistryV1::default()
            )
            .is_err()
        );
    }

    #[test]
    fn direct_release_comparison_covers_multi_release_order_and_every_field() {
        let local = KagemushaVerifierReleaseAuthorityV1 {
            release_id: [1; 32],
            status: 1,
            profile_digest: [2; 32],
            artifact_manifest_digest: [3; 32],
            receipt_digest: [4; 32],
            attestation_digest: [5; 32],
            authority_policy_digest: [6; 32],
            hardware_policy_digest: [7; 32],
            native_profile_digest: [8; 32],
            provider_policy_root: [9; 32],
            suite_id: [10; 32],
            vk_set_digest: [11; 32],
        };
        let governed = KagemushaGovernedVerifierReleaseV1 {
            release_id: local.release_id,
            status: local.status,
            profile_digest: local.profile_digest,
            artifact_manifest_digest: local.artifact_manifest_digest,
            receipt_digest: local.receipt_digest,
            attestation_digest: local.attestation_digest,
            authority_policy_digest: local.authority_policy_digest,
            hardware_policy_digest: local.hardware_policy_digest,
            native_profile_digest: local.native_profile_digest,
            provider_policy_root: local.provider_policy_root,
            suite_id: local.suite_id,
            vk_set_digest: local.vk_set_digest,
        };
        let mut second_local = local.clone();
        second_local.release_id = [12; 32];
        second_local.status = 2;
        let mut second_governed = governed.clone();
        second_governed.release_id = second_local.release_id;
        second_governed.status = second_local.status;
        let mut historical_local = local.clone();
        historical_local.release_id = [13; 32];
        historical_local.status = 3;
        let mut historical_governed = governed.clone();
        historical_governed.release_id = historical_local.release_id;
        historical_governed.status = historical_local.status;
        let locals = [local.clone(), second_local, historical_local];
        let governed_rows = [governed.clone(), second_governed, historical_governed];
        assert!(
            locals
                .iter()
                .zip(governed_rows.iter())
                .all(|(local, governed)| release_matches_governed(local, governed))
        );
        assert!(!release_matches_governed(&locals[0], &governed_rows[1]));
        assert!(!release_matches_governed(&locals[1], &governed_rows[0]));

        let mut changed = governed.clone();
        changed.status = 2;
        assert!(!release_matches_governed(&local, &changed));
        macro_rules! changed_digest {
            ($field:ident) => {{
                let mut changed = governed.clone();
                changed.$field = [42; 32];
                assert!(!release_matches_governed(&local, &changed));
            }};
        }
        changed_digest!(release_id);
        changed_digest!(profile_digest);
        changed_digest!(artifact_manifest_digest);
        changed_digest!(receipt_digest);
        changed_digest!(attestation_digest);
        changed_digest!(authority_policy_digest);
        changed_digest!(hardware_policy_digest);
        changed_digest!(native_profile_digest);
        changed_digest!(provider_policy_root);
        changed_digest!(suite_id);
        changed_digest!(vk_set_digest);
    }

    #[test]
    fn every_release_authority_field_changes_the_canonical_preimage() {
        let row = KagemushaVerifierReleaseAuthorityV1 {
            release_id: [1; 32],
            status: 1,
            profile_digest: [2; 32],
            artifact_manifest_digest: [3; 32],
            receipt_digest: [4; 32],
            attestation_digest: [5; 32],
            authority_policy_digest: [6; 32],
            hardware_policy_digest: [7; 32],
            native_profile_digest: [8; 32],
            provider_policy_root: [9; 32],
            suite_id: [10; 32],
            vk_set_digest: [11; 32],
        };
        let baseline = KagemushaVerifierAuthorityV1 {
            mode: 1,
            active_release_id: Some(row.release_id),
            releases: vec![row.clone()],
        };
        let expected = encode_canonical(&baseline).unwrap();
        let mut variants = Vec::new();
        let mut changed = baseline.clone();
        changed.mode = 0;
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.active_release_id = None;
        variants.push(changed);
        let mut changed = baseline.clone();
        changed.releases[0].status = 2;
        variants.push(changed);
        macro_rules! changed_digest {
            ($field:ident) => {{
                let mut changed = baseline.clone();
                changed.releases[0].$field = [42; 32];
                variants.push(changed);
            }};
        }
        changed_digest!(release_id);
        changed_digest!(profile_digest);
        changed_digest!(artifact_manifest_digest);
        changed_digest!(receipt_digest);
        changed_digest!(attestation_digest);
        changed_digest!(authority_policy_digest);
        changed_digest!(hardware_policy_digest);
        changed_digest!(native_profile_digest);
        changed_digest!(provider_policy_root);
        changed_digest!(suite_id);
        changed_digest!(vk_set_digest);
        assert!(
            variants
                .iter()
                .all(|variant| encode_canonical(variant).unwrap() != expected)
        );
    }
}
