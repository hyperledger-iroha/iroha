//! Trusted Rust-host entrypoint for an experimental testnet mint observation lane.
//!
//! A signed `TestnetExperiment` release, exact private mint reservation, and independently
//! authenticated first finality context must originate with the native host. This module
//! exposes no C/JNI installer, private opening transport, or caller-selected finality anchor.
//! The retained observation owner still produces no production monetary or hardware authority.

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use iroha_core::zk::{
    kagemusha_v1_recursion::{
        KagemushaRecursiveVerifierProfileV1, KagemushaTestnetStateObservationScopeV1,
        KagemushaVerifiedFinalityChainV1,
    },
    kagemusha_v1_state::MintInboxReservationV1,
};
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId, block::consensus_v2::HeightContextId,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
    kagemusha::KagemushaReleaseAuthorityPolicyV1,
};

use crate::{
    kagemusha_testnet_finality_chain_v1::pin_kagemusha_testnet_authenticated_finality_chain_v1,
    kagemusha_testnet_observation_v1::{
        KagemushaTestnetDurableObservationModeV1,
        load_and_install_kagemusha_testnet_durable_state_observation_owner_v1,
        reserve_kagemusha_testnet_mint_before_submission_v1,
    },
};

/// Independently provisioned native configuration and signed-release inputs.
///
/// The manifest, receipt, attestation, and artifact directory are authenticated by the
/// installer. The authority policy, scope, storage path, network, and first finality context
/// must be supplied by trusted native configuration, never copied from an app request or Torii
/// operation response. No member of this type crosses the C/JNI observer boundary.
pub struct KagemushaTestnetNativeMintInstallV1<'a> {
    /// Threshold-signed release manifest archive.
    pub manifest_archive: &'a [u8],
    /// Internal validation receipt for the experimental release.
    pub validation_receipt_archive: &'a [u8],
    /// Threshold release attestation archive.
    pub release_attestation_archive: &'a [u8],
    /// Native operator-pinned release authority policy.
    pub trusted_authority_policy: &'a KagemushaReleaseAuthorityPolicyV1,
    /// Native operator-pinned network, asset, reserve, and release scope.
    pub scope: KagemushaTestnetStateObservationScopeV1,
    /// Exact release-authenticated native verifier layout.
    pub profile: KagemushaRecursiveVerifierProfileV1,
    /// Content-addressed proof artifact directory.
    pub artifact_root: &'a Path,
    /// Private durable mint and observation journal path.
    pub journal_path: &'a Path,
    /// Create or recover the private journal.
    pub mode: KagemushaTestnetDurableObservationModeV1,
    /// Independently authenticated historical anchors required for recovery.
    pub independent_anchors: &'a BTreeMap<[u8; 32], KagemushaVerifiedFinalityChainV1>,
    /// Native-pinned network, independently of the signed release and status response.
    pub trusted_network_id: NetworkId,
    /// Native-pinned first context for verification of every later finality bundle.
    pub trusted_first_context_id: HeightContextId,
}

/// Proof that this process persisted one exact private reservation before submission.
///
/// Only [`KagemushaTestnetNativeMintRuntimeV1::reserve_before_submission`] creates this
/// nonserializable token. The durable owner, not this process-local token, remains authoritative
/// after a crash; the native host must recover the journal and repeat the exact reservation.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct KagemushaTestnetNativeMintReservationV1 {
    operation_id: [u8; 32],
    reservation_digest: [u8; 32],
}

impl KagemushaTestnetNativeMintReservationV1 {
    /// Return the original top-up operation ID for the native submit path.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 32] {
        self.operation_id
    }
}

/// One process-local handle to the signed-release durable testnet observation owner.
///
/// This is a Rust-host integration prerequisite, not a mobile SDK or production mint backend.
/// It retains the independent first finality context privately and accepts only a signed chain
/// for a previously persisted reservation. The owner itself is installed once process-wide.
pub struct KagemushaTestnetNativeMintRuntimeV1 {
    trusted_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
    reservations: Mutex<BTreeMap<[u8; 32], [u8; 32]>>,
}

impl KagemushaTestnetNativeMintRuntimeV1 {
    /// Authenticate and install one durable experimental observer using trusted native pins.
    ///
    /// # Errors
    /// Rejects an invalid native network/context pin, unauthenticated release, changed journal,
    /// failed replay, or an already installed owner.
    pub fn install(inputs: KagemushaTestnetNativeMintInstallV1<'_>) -> Result<Self, String> {
        require_trusted_pins(
            inputs.scope,
            inputs.trusted_network_id,
            inputs.trusted_first_context_id,
        )?;
        for verified in inputs.independent_anchors.values() {
            require_matching_verified_chain_root(
                inputs.trusted_network_id,
                inputs.trusted_first_context_id,
                verified.anchor().network_id,
                verified.first_context_id(),
            )?;
        }
        load_and_install_kagemusha_testnet_durable_state_observation_owner_v1(
            inputs.manifest_archive,
            inputs.validation_receipt_archive,
            inputs.release_attestation_archive,
            inputs.trusted_authority_policy,
            inputs.scope,
            inputs.profile,
            inputs.artifact_root,
            inputs.journal_path,
            inputs.mode,
            inputs.independent_anchors,
        )?;
        Ok(Self {
            trusted_network_id: inputs.trusted_network_id,
            trusted_first_context_id: inputs.trusted_first_context_id,
            reservations: Mutex::new(BTreeMap::new()),
        })
    }

    /// Persist the exact native-owned private mint reservation before the top-up is submitted.
    ///
    /// An exact retry is allowed, including after native journal recovery. The credit opening
    /// remains inside the Rust host and durable owner; no app-provided serialized reservation is
    /// accepted at this boundary.
    ///
    /// # Errors
    /// Rejects a malformed or conflicting reservation, unavailable durable owner, or failed
    /// journal persistence. No token is issued until the durable owner accepts the reservation.
    pub fn reserve_before_submission(
        &self,
        reservation: &MintInboxReservationV1,
    ) -> Result<KagemushaTestnetNativeMintReservationV1, String> {
        let operation_id = reservation.operation_id();
        let reservation_digest = reservation
            .digest()
            .map_err(|error| format!("invalid private testnet mint reservation: {error}"))?;
        let mut reservations = self
            .reservations
            .lock()
            .map_err(|_| "testnet mint reservation index is poisoned".to_owned())?;
        if reservations
            .get(&operation_id)
            .is_some_and(|previous| previous != &reservation_digest)
        {
            return Err("testnet mint operation changed its private reservation".to_owned());
        }
        reserve_kagemusha_testnet_mint_before_submission_v1(reservation)?;
        reservations.insert(operation_id, reservation_digest);
        Ok(KagemushaTestnetNativeMintReservationV1 {
            operation_id,
            reservation_digest,
        })
    }

    /// Verify a signed finality chain from the retained first context and pin its last context.
    ///
    /// The chain may be obtained from an untrusted network response; its validator signatures,
    /// contiguous heights, network, and first context are checked before the durable owner pins
    /// the result. This method accepts neither raw anchor coordinates nor a status-derived trust
    /// root. The private reservation token must belong to this runtime.
    ///
    /// # Errors
    /// Rejects an absent or changed reservation, invalid signed chain, missing durable owner,
    /// or a replacement finality pin. Returns whether a new pin was written and the exact
    /// anchor from that same verified chain; an exact retry returns `false` with that anchor.
    pub fn pin_finality_chain(
        &self,
        reservation: &KagemushaTestnetNativeMintReservationV1,
        chain_json: &[u8],
    ) -> Result<(bool, KagemushaFinalityTrustAnchorV1), String> {
        let reservations = self
            .reservations
            .lock()
            .map_err(|_| "testnet mint reservation index is poisoned".to_owned())?;
        if reservations.get(&reservation.operation_id) != Some(&reservation.reservation_digest) {
            return Err(
                "testnet mint finality requires this runtime's persisted reservation".to_owned(),
            );
        }
        drop(reservations);
        pin_kagemusha_testnet_authenticated_finality_chain_v1(
            reservation.operation_id,
            self.trusted_network_id,
            self.trusted_first_context_id,
            chain_json,
        )
    }
}

fn require_trusted_pins(
    scope: KagemushaTestnetStateObservationScopeV1,
    network_id: NetworkId,
    first_context_id: HeightContextId,
) -> Result<(), String> {
    if scope.network_id() != *network_id.as_bytes() {
        return Err("testnet mint native network differs from signed-release scope".to_owned());
    }
    // Hash::prehashed marks the final bit, so an all-zero input becomes this
    // canonical placeholder rather than an all-zero HashOf value.
    if first_context_id.0.as_ref() == Hash::prehashed([0; 32]).as_ref() {
        return Err("testnet mint first finality context is unpinned".to_owned());
    }
    Ok(())
}

fn require_matching_verified_chain_root(
    trusted_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
    verified_network_id: NetworkId,
    verified_first_context_id: HeightContextId,
) -> Result<(), String> {
    if verified_network_id != trusted_network_id
        || verified_first_context_id != trusted_first_context_id
    {
        return Err("recovered testnet finality chain differs from native trust root".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "kagemusha_testnet_native_mint_runtime_v1_tests.rs"]
mod tests;
