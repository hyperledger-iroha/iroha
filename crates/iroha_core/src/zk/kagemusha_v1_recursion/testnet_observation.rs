//! Non-authorizing testnet observations of actual paired KAGEMUSHA State proofs.
//!
//! This boundary is for a wallet to inspect experimental State lineage on a specifically
//! configured network, asset reserve, and authenticated release. It does not issue a monetary
//! admission token,
//! attest an app or device, authorize a terminal transition, or qualify hardware custody.

use iroha_data_model::kagemusha::{KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaPairedProofV1};

use super::{
    DigestV1, KagemushaAuthenticatedRecursiveVerifierV1, KagemushaOperationV1,
    KagemushaRecursionErrorV1, KagemushaStateRelationPublicInputsV1,
    kagemusha_candidate_envelope_digest_v1, verify_kagemusha_state_proof_v1,
};
use crate::zk::kagemusha_v1_state::KagemushaStateV1;

/// Trusted, exact testnet network, asset, reserve, and release supplied by the operator.
///
/// The authenticated release manifest does not carry a network ID. An application must obtain
/// the network and asset/reserve pins from its independently trusted testnet configuration.
/// Self-declared identifiers copied from the submitted proof provide no pinning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaTestnetStateObservationScopeV1 {
    network_id: DigestV1,
    asset_identity_digest: DigestV1,
    asset_incarnation: DigestV1,
    asset_scale: u32,
    liability_pool_id: DigestV1,
    release_id: DigestV1,
    release_attestation_digest: DigestV1,
}

impl KagemushaTestnetStateObservationScopeV1 {
    /// Pin the exact testnet liability pool and authenticated release before proof inspection.
    ///
    /// # Errors
    ///
    /// Rejects absent, aliased, or out-of-range pins. The operator must derive the asset
    /// identity and liability pool from its trusted asset registration, not from a submitted
    /// proof. The native verifier checks their exact equality to each proved State.
    pub fn new(
        network_id: DigestV1,
        asset_identity_digest: DigestV1,
        asset_incarnation: DigestV1,
        asset_scale: u32,
        liability_pool_id: DigestV1,
        release_id: DigestV1,
        release_attestation_digest: DigestV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if network_id == [0; 32]
            || asset_identity_digest == [0; 32]
            || asset_incarnation == [0; 32]
            || asset_scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || liability_pool_id == [0; 32]
            || release_id == [0; 32]
            || release_attestation_digest == [0; 32]
            || network_id == release_id
            || network_id == release_attestation_digest
            || release_id == release_attestation_digest
            || asset_identity_digest == liability_pool_id
        {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
        }
        Ok(Self {
            network_id,
            asset_identity_digest,
            asset_incarnation,
            asset_scale,
            liability_pool_id,
            release_id,
            release_attestation_digest,
        })
    }

    /// Return the independently pinned raw network identifier.
    #[must_use]
    pub const fn network_id(&self) -> DigestV1 {
        self.network_id
    }

    /// Return the operator-pinned normalized asset identity.
    #[must_use]
    pub const fn asset_identity_digest(&self) -> DigestV1 {
        self.asset_identity_digest
    }

    /// Return the operator-pinned exact asset incarnation.
    #[must_use]
    pub const fn asset_incarnation(&self) -> DigestV1 {
        self.asset_incarnation
    }

    /// Return the operator-pinned decimal asset scale.
    #[must_use]
    pub const fn asset_scale(&self) -> u32 {
        self.asset_scale
    }

    /// Return the operator-pinned asset reserve-liability pool.
    #[must_use]
    pub const fn liability_pool_id(&self) -> DigestV1 {
        self.liability_pool_id
    }

    /// Return the authenticated proof-release identifier.
    #[must_use]
    pub const fn release_id(&self) -> DigestV1 {
        self.release_id
    }

    /// Return the authenticated release-attestation digest.
    #[must_use]
    pub const fn release_attestation_digest(&self) -> DigestV1 {
        self.release_attestation_digest
    }

    fn check_release_bindings(
        self,
        authenticated_release_id: DigestV1,
        authenticated_release_attestation_digest: DigestV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if authenticated_release_id != self.release_id
            || authenticated_release_attestation_digest != self.release_attestation_digest
        {
            return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
        }
        Ok(())
    }

    fn check_state_bindings(
        self,
        successor: &KagemushaStateV1,
        predecessor: Option<&KagemushaStateV1>,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        for state in core::iter::once(successor).chain(predecessor) {
            if state.release_id != self.release_id {
                return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
            }
            let asset_identity = state
                .lane
                .normalized_asset_id()
                .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?;
            if state.lane.normalized_network_id() != self.network_id
                || asset_identity != self.asset_identity_digest
                || state.asset_incarnation.as_bytes() != &self.asset_incarnation
                || state.lane.scale != self.asset_scale
                || state.liability_pool_id != self.liability_pool_id
            {
                return Err(KagemushaRecursionErrorV1::StateStatement(
                    "KAGEMUSHA State proof is outside the operator-pinned testnet asset and reserve"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }
}

/// Proof inspection result with no monetary or hardware authority.
///
/// Private fields and no Norito encoding prevent this diagnostic value from being confused with
/// a verifiable payment or redemption capability. Its existence means only that the native
/// authenticated-release verifier accepted the paired State proof at the observation boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaTestnetStateProofObservationV1 {
    scope: KagemushaTestnetStateObservationScopeV1,
    operation: KagemushaOperationV1,
    candidate_envelope_digest: DigestV1,
    successor_state_commitment: DigestV1,
}

impl KagemushaTestnetStateProofObservationV1 {
    /// Return the complete operator-pinned network, asset reserve, and release scope.
    #[must_use]
    pub const fn scope(&self) -> KagemushaTestnetStateObservationScopeV1 {
        self.scope
    }

    /// Return the verified transition kind.
    #[must_use]
    pub const fn operation(&self) -> KagemushaOperationV1 {
        self.operation
    }

    /// Return the verified public candidate envelope name.
    #[must_use]
    pub const fn candidate_envelope_digest(&self) -> DigestV1 {
        self.candidate_envelope_digest
    }

    /// Return the verified successor State commitment.
    #[must_use]
    pub const fn successor_state_commitment(&self) -> DigestV1 {
        self.successor_state_commitment
    }

    /// Testnet State observation never qualifies app or device hardware custody.
    #[must_use]
    pub const fn hardware_qualified(&self) -> bool {
        false
    }
}

/// Process-local trial of one testnet lane's proven aggregate-state lineage.
///
/// Only the authenticated native verifier can advance this trial. It retains the entire prior
/// public state, so the next proof must consume exactly that state rather than a caller-supplied
/// digest with a matching name. A separate trial must be started for a receiving phone's lane.
/// This is deliberately not a durable wallet, settlement ledger, hardware counter, or monetary
/// admission capability: recreating the object does not prevent a fork across processes.
/// TODO: connect actual testnet issuance/finality, conflict adjudication, and redemption runtime
/// to this proof-lineage diagnostic; process-local observation alone cannot move value.
pub struct KagemushaTestnetLineageTrialV1 {
    scope: KagemushaTestnetStateObservationScopeV1,
    head: Option<KagemushaStateV1>,
    observed_transitions: u64,
}

/// Native owner of the authenticated testnet verifier and one process-local lineage trial.
///
/// Construction requires an already release-authorized concrete verifier; a caller cannot
/// install a callback that claims proof success. The operator's network/release pins are checked
/// against that verifier before the owner can be used. State asset and reserve pins are checked
/// for every observation. No method grants a hardware qualification
/// or returns a spend/redemption capability.
pub struct KagemushaTestnetProofObservationOwnerV1 {
    verifier: KagemushaAuthenticatedRecursiveVerifierV1,
    trial: KagemushaTestnetLineageTrialV1,
}

impl KagemushaTestnetProofObservationOwnerV1 {
    /// Return the operator-pinned scope retained alongside this concrete verifier.
    #[must_use]
    pub const fn scope(&self) -> KagemushaTestnetStateObservationScopeV1 {
        self.trial.scope()
    }

    /// Canonically name the submitted public candidate before proof verification.
    ///
    /// This is only a deterministic projection for preparing an output buffer; it
    /// does not validate the submitted proof or authorize the candidate.
    ///
    /// # Errors
    ///
    /// Rejects a malformed public candidate projection.
    pub fn candidate_envelope_digest(
        public_inputs: &KagemushaStateRelationPublicInputsV1,
    ) -> Result<DigestV1, KagemushaRecursionErrorV1> {
        kagemusha_candidate_envelope_digest_v1(public_inputs)
            .map_err(KagemushaRecursionErrorV1::StateStatement)
    }

    /// Retain the exact authenticated native verifier and independently configured testnet pins.
    ///
    /// # Errors
    ///
    /// Rejects a verifier without an authenticated monetary release, or mismatched release pins.
    pub fn new(
        verifier: KagemushaAuthenticatedRecursiveVerifierV1,
        scope: KagemushaTestnetStateObservationScopeV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        let release_identity = verifier
            .monetary_release()
            .map(|release| (release.release_id(), release.attestation_digest()));
        require_owner_release_pins(scope, release_identity)?;
        Ok(Self {
            verifier,
            trial: KagemushaTestnetLineageTrialV1::new(scope),
        })
    }

    /// Verify and append one paired State proof using only this owner's retained verifier.
    ///
    /// # Errors
    ///
    /// Rejects any proof, scope, or lineage mismatch without advancing the trial.
    pub fn observe_and_advance(
        &mut self,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<KagemushaTestnetStateProofObservationV1, KagemushaRecursionErrorV1> {
        self.trial
            .observe_and_advance(&self.verifier, public_inputs, proof)
    }
}

fn require_owner_release_pins(
    scope: KagemushaTestnetStateObservationScopeV1,
    authenticated_release_identity: Result<(DigestV1, DigestV1), String>,
) -> Result<(), KagemushaRecursionErrorV1> {
    let (release_id, attestation_digest) =
        authenticated_release_identity.map_err(KagemushaRecursionErrorV1::StateProofRejected)?;
    scope.check_release_bindings(release_id, attestation_digest)
}

impl KagemushaTestnetLineageTrialV1 {
    /// Start a new testnet trial that may accept only a verified zero bootstrap first.
    #[must_use]
    pub const fn new(scope: KagemushaTestnetStateObservationScopeV1) -> Self {
        Self {
            scope,
            head: None,
            observed_transitions: 0,
        }
    }

    /// Return the exact network, asset reserve, and authenticated release pinned to this trial.
    #[must_use]
    pub const fn scope(&self) -> KagemushaTestnetStateObservationScopeV1 {
        self.scope
    }

    /// Return the last proven State commitment, if any.
    #[must_use]
    pub fn head_commitment(&self) -> Option<DigestV1> {
        self.head.as_ref().map(|state| state.state_commitment)
    }

    /// Return the number of verified transitions consumed in this process.
    #[must_use]
    pub const fn observed_transitions(&self) -> u64 {
        self.observed_transitions
    }

    /// Inspect and append one real paired proof to this process-local experimental lineage.
    ///
    /// Mint, send, receive and redemption experiments use the same V1 State proof relation and
    /// authenticated release. This API cannot fund an account, release an outbox item, redeem a
    /// liability, or grant a production hardware claim. Those require their separate finality,
    /// terminal and qualified device gates.
    ///
    /// # Errors
    ///
    /// Rejects a missing or forked predecessor, a changed lane, a different network, asset,
    /// reserve or release, counter overflow, or failure of the native paired-proof verifier.
    pub fn observe_and_advance(
        &mut self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<KagemushaTestnetStateProofObservationV1, KagemushaRecursionErrorV1> {
        self.check_next(public_inputs)?;
        let observation =
            observe_kagemusha_testnet_state_proof_v1(verifier, self.scope, public_inputs, proof)?;
        self.record_verified(public_inputs, observation)?;
        Ok(observation)
    }

    fn check_next(
        &self,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if self.observed_transitions == u64::MAX {
            return Err(KagemushaRecursionErrorV1::StateStatement(
                "KAGEMUSHA testnet trial transition count overflow".to_owned(),
            ));
        }
        match (&self.head, &public_inputs.predecessor) {
            (None, None) if public_inputs.operation == KagemushaOperationV1::Bootstrap => Ok(()),
            (Some(head), Some(predecessor))
                if public_inputs.operation != KagemushaOperationV1::Bootstrap
                    && head == predecessor
                    && head.lane == public_inputs.successor.lane =>
            {
                Ok(())
            }
            _ => Err(KagemushaRecursionErrorV1::StateStatement(
                "KAGEMUSHA testnet trial does not consume its exact prior state".to_owned(),
            )),
        }
    }

    fn record_verified(
        &mut self,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        observation: KagemushaTestnetStateProofObservationV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        self.check_next(public_inputs)?;
        self.scope
            .check_state_bindings(&public_inputs.successor, public_inputs.predecessor.as_ref())?;
        if observation.scope != self.scope
            || observation.operation != public_inputs.operation
            || observation.successor_state_commitment != public_inputs.successor.state_commitment
            || observation.candidate_envelope_digest
                != kagemusha_candidate_envelope_digest_v1(public_inputs)
                    .map_err(KagemushaRecursionErrorV1::StateStatement)?
        {
            return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
        }
        self.head = Some(public_inputs.successor.clone());
        self.observed_transitions += 1;
        Ok(())
    }
}

/// Observe an actual paired State proof under an operator-pinned testnet asset and signed release.
///
/// This uses the native authenticated verifier to check and decide both Pasta proofs and their
/// histories. The State relation checks its finalized mint, value conservation and replay
/// transitions; this function does not convert that check into permission to spend, receive,
/// redeem, or settle. The production Guard and terminal monetary gates remain separate.
///
/// # Errors
///
/// Rejects a missing authenticated release, wrong network, asset, reserve, or release pin, malformed public
/// candidate projection, substituted proof artifact, or any native paired-proof failure.
pub fn observe_kagemusha_testnet_state_proof_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    scope: KagemushaTestnetStateObservationScopeV1,
    public_inputs: &KagemushaStateRelationPublicInputsV1,
    proof: &KagemushaPairedProofV1,
) -> Result<KagemushaTestnetStateProofObservationV1, KagemushaRecursionErrorV1> {
    // TODO: Exercise this public entrypoint with a rejected paired proof once a test fixture
    // supplies a fully loaded verifier and threshold-authenticated release. The current
    // CountingResolver fixture fails during verifier loading, before observation is callable.
    let release = verifier
        .monetary_release()
        .map_err(KagemushaRecursionErrorV1::StateProofRejected)?;
    scope.check_release_bindings(release.release_id(), release.attestation_digest())?;
    scope.check_state_bindings(&public_inputs.successor, public_inputs.predecessor.as_ref())?;
    let artifacts = verifier.state_checkpoint_material().artifacts;
    verify_kagemusha_state_proof_v1(verifier, artifacts, public_inputs, proof)?;
    let candidate_envelope_digest = kagemusha_candidate_envelope_digest_v1(public_inputs)
        .map_err(KagemushaRecursionErrorV1::StateStatement)?;
    Ok(KagemushaTestnetStateProofObservationV1 {
        scope,
        operation: public_inputs.operation,
        candidate_envelope_digest,
        successor_state_commitment: public_inputs.successor.state_commitment,
    })
}

#[cfg(test)]
mod tests {
    use super::super::KagemushaPreparedIntentCommitmentsV1;
    use super::*;

    const NETWORK: DigestV1 = [1; 32];
    const RELEASE: DigestV1 = [2; 32];
    const ATTESTATION: DigestV1 = [3; 32];
    const ASSET: DigestV1 = [4; 32];
    const INCARNATION: DigestV1 = [5; 32];
    const POOL: DigestV1 = [6; 32];
    const SCALE: u32 = 2;

    fn scope() -> KagemushaTestnetStateObservationScopeV1 {
        KagemushaTestnetStateObservationScopeV1::new(
            NETWORK,
            ASSET,
            INCARNATION,
            SCALE,
            POOL,
            RELEASE,
            ATTESTATION,
        )
        .expect("distinct operator pins")
    }

    #[test]
    fn scope_rejects_missing_and_aliased_pins() {
        assert_eq!(scope().network_id(), NETWORK);
        assert_eq!(scope().asset_identity_digest(), ASSET);
        assert_eq!(scope().asset_incarnation(), INCARNATION);
        assert_eq!(scope().asset_scale(), SCALE);
        assert_eq!(scope().liability_pool_id(), POOL);
        assert_eq!(scope().release_id(), RELEASE);
        assert_eq!(scope().release_attestation_digest(), ATTESTATION);
        for pins in [
            ([0; 32], RELEASE, ATTESTATION),
            (NETWORK, [0; 32], ATTESTATION),
            (NETWORK, RELEASE, [0; 32]),
            (NETWORK, NETWORK, ATTESTATION),
            (NETWORK, RELEASE, NETWORK),
            (NETWORK, RELEASE, RELEASE),
        ] {
            assert!(matches!(
                KagemushaTestnetStateObservationScopeV1::new(
                    pins.0,
                    ASSET,
                    INCARNATION,
                    SCALE,
                    POOL,
                    pins.1,
                    pins.2,
                ),
                Err(KagemushaRecursionErrorV1::InvalidArtifacts)
            ));
        }
        for (asset, incarnation, scale, pool) in [
            ([0; 32], INCARNATION, SCALE, POOL),
            (ASSET, [0; 32], SCALE, POOL),
            (ASSET, INCARNATION, KAGEMUSHA_ASSET_SCALE_MAX_V1 + 1, POOL),
            (ASSET, INCARNATION, SCALE, [0; 32]),
            (ASSET, INCARNATION, SCALE, ASSET),
        ] {
            assert!(matches!(
                KagemushaTestnetStateObservationScopeV1::new(
                    NETWORK,
                    asset,
                    incarnation,
                    scale,
                    pool,
                    RELEASE,
                    ATTESTATION,
                ),
                Err(KagemushaRecursionErrorV1::InvalidArtifacts)
            ));
        }
    }

    #[test]
    fn scope_checks_signed_release_and_exact_state_asset_pool() {
        let (trial, public) = trial_fixture();
        let scope = trial.scope();
        let state = &public.successor;
        assert_eq!(
            scope.check_release_bindings(scope.release_id(), scope.release_attestation_digest()),
            Ok(()),
        );
        assert_eq!(scope.check_state_bindings(state, Some(state)), Ok(()));
        let mut changed = state.clone();
        changed.release_id = [0xA1; 32];
        assert_eq!(
            scope.check_state_bindings(state, Some(&changed)),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution),
        );
        assert_eq!(
            scope.check_release_bindings([0xA2; 32], scope.release_attestation_digest()),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution),
        );
        assert_eq!(
            scope.check_release_bindings(scope.release_id(), [0xA3; 32]),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution),
        );
        let mut changed = state.clone();
        changed.liability_pool_id = [0xA4; 32];
        assert!(matches!(
            scope.check_state_bindings(&changed, None),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        // The predecessor is checked as well as the successor.
        changed = state.clone();
        changed.liability_pool_id = [0xA5; 32];
        assert!(matches!(
            scope.check_state_bindings(state, Some(&changed)),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        let mut changed = state.clone();
        changed.lane.scale = changed.lane.scale.saturating_add(1);
        assert!(scope.check_state_bindings(&changed, None).is_err());
        for (network, asset, incarnation, scale, pool) in [
            (
                [0xA9; 32],
                scope.asset_identity_digest(),
                scope.asset_incarnation(),
                scope.asset_scale(),
                scope.liability_pool_id(),
            ),
            (
                scope.network_id(),
                [0xA6; 32],
                scope.asset_incarnation(),
                scope.asset_scale(),
                scope.liability_pool_id(),
            ),
            (
                scope.network_id(),
                scope.asset_identity_digest(),
                [0xA7; 32],
                scope.asset_scale(),
                scope.liability_pool_id(),
            ),
            (
                scope.network_id(),
                scope.asset_identity_digest(),
                scope.asset_incarnation(),
                scope.asset_scale() + 1,
                scope.liability_pool_id(),
            ),
            (
                scope.network_id(),
                scope.asset_identity_digest(),
                scope.asset_incarnation(),
                scope.asset_scale(),
                [0xA8; 32],
            ),
        ] {
            let wrong_scope = KagemushaTestnetStateObservationScopeV1::new(
                network,
                asset,
                incarnation,
                scale,
                pool,
                scope.release_id(),
                scope.release_attestation_digest(),
            )
            .expect("distinct but wrong operator asset pin");
            assert!(wrong_scope.check_state_bindings(state, None).is_err());
        }
        let mut changed = state.clone();
        changed.lane.device_lane_id[0] ^= 1;
        assert_eq!(scope.check_state_bindings(&changed, None), Ok(()));
        // Lane continuity belongs to the lineage trial, while scope fixes the asset reserve.
    }

    #[test]
    fn observation_is_explicitly_unqualified() {
        let observation = KagemushaTestnetStateProofObservationV1 {
            scope: scope(),
            operation: KagemushaOperationV1::MintFold,
            candidate_envelope_digest: [4; 32],
            successor_state_commitment: [5; 32],
        };
        assert_eq!(observation.scope(), scope());
        assert_eq!(observation.operation(), KagemushaOperationV1::MintFold);
        assert_eq!(observation.candidate_envelope_digest(), [4; 32]);
        assert_eq!(observation.successor_state_commitment(), [5; 32]);
        assert!(!observation.hardware_qualified());
    }

    #[test]
    fn native_owner_requires_an_authenticated_release_matching_operator_pins() {
        let scope = scope();
        assert_eq!(
            require_owner_release_pins(scope, Ok((RELEASE, ATTESTATION))),
            Ok(())
        );
        assert!(matches!(
            require_owner_release_pins(scope, Err("release not authorized".to_owned())),
            Err(KagemushaRecursionErrorV1::StateProofRejected(_))
        ));
        for identity in [([4; 32], ATTESTATION), (RELEASE, [4; 32])] {
            assert_eq!(
                require_owner_release_pins(scope, Ok(identity)),
                Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
            );
        }
    }

    #[test]
    fn owner_candidate_projection_is_deterministic_but_not_authority() {
        let (_, public) = trial_fixture();
        let expected = kagemusha_candidate_envelope_digest_v1(&public).expect("fixture candidate");
        assert_eq!(
            KagemushaTestnetProofObservationOwnerV1::candidate_envelope_digest(&public),
            Ok(expected)
        );
    }

    fn trial_fixture() -> (
        KagemushaTestnetLineageTrialV1,
        KagemushaStateRelationPublicInputsV1,
    ) {
        let (public, _) = super::super::tests::state_verification_fixture();
        let scope = KagemushaTestnetStateObservationScopeV1::new(
            public.successor.lane.normalized_network_id(),
            public
                .successor
                .lane
                .normalized_asset_id()
                .expect("fixture asset identity"),
            *public.successor.asset_incarnation.as_bytes(),
            public.successor.lane.scale,
            public.successor.liability_pool_id,
            public.successor.release_id,
            [0xD1; 32],
        )
        .expect("fixture pins");
        (KagemushaTestnetLineageTrialV1::new(scope), public)
    }

    fn observed(
        public: &KagemushaStateRelationPublicInputsV1,
        scope: KagemushaTestnetStateObservationScopeV1,
    ) -> KagemushaTestnetStateProofObservationV1 {
        // A private helper tests only the post-verification continuity guard. The public method
        // obtains this observation exclusively from the authenticated native paired verifier.
        KagemushaTestnetStateProofObservationV1 {
            scope,
            operation: public.operation,
            candidate_envelope_digest: kagemusha_candidate_envelope_digest_v1(public)
                .expect("canonical fixture candidate"),
            successor_state_commitment: public.successor.state_commitment,
        }
    }

    fn next_trial_public(
        previous: &KagemushaStateRelationPublicInputsV1,
        operation: KagemushaOperationV1,
        tag: u8,
    ) -> KagemushaStateRelationPublicInputsV1 {
        let mut next = previous.clone();
        next.operation = operation;
        next.predecessor = Some(previous.successor.clone());
        next.successor.state_commitment = [tag; 32];
        next.successor.logical_sequence += 1;
        next.successor.secure_index += 1;
        next.journal_revision_before = previous.journal_revision_after;
        next.journal_revision_after += 1;
        next.prepared_intent = matches!(
            operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        )
        .then_some(KagemushaPreparedIntentCommitmentsV1 {
            preparation_id: [tag; 32],
            sealed_transition_inputs_digest: [tag.wrapping_add(1); 32],
            sealed_recovery_seeds_digest: [tag.wrapping_add(2); 32],
        });
        next
    }

    #[test]
    fn trial_tracks_bootstrap_mint_send_receive_and_redeem_without_hardware_authority() {
        let (mut trial, mut public) = trial_fixture();
        assert_eq!(trial.head_commitment(), None);
        assert_eq!(trial.observed_transitions(), 0);
        assert_eq!(trial.scope().release_id(), public.successor.release_id);
        for (operation, tag) in [
            (KagemushaOperationV1::Bootstrap, 0),
            (KagemushaOperationV1::MintFold, 0xE1),
            (KagemushaOperationV1::SendSplit, 0xE2),
            (KagemushaOperationV1::ReceiveFold, 0xE3),
            (KagemushaOperationV1::RedeemSplit, 0xE4),
        ] {
            if operation != KagemushaOperationV1::Bootstrap {
                public = next_trial_public(&public, operation, tag);
            }
            let observation = observed(&public, trial.scope());
            assert!(!observation.hardware_qualified());
            trial
                .record_verified(&public, observation)
                .expect("verified lineage fixture");
            assert_eq!(
                trial.head_commitment(),
                Some(public.successor.state_commitment)
            );
        }
        assert_eq!(trial.observed_transitions(), 5);
    }

    #[test]
    fn trial_rejects_fork_replay_changed_lane_and_substituted_observation() {
        let (mut trial, public) = trial_fixture();
        let trial_scope = trial.scope();
        let bootstrap = observed(&public, trial_scope);
        trial
            .record_verified(&public, bootstrap)
            .expect("first bootstrap");
        assert!(matches!(
            trial.record_verified(&public, bootstrap),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        let mut next = next_trial_public(&public, KagemushaOperationV1::MintFold, 0xE1);
        let mut wrong_predecessor = next.clone();
        wrong_predecessor.predecessor.as_mut().unwrap().balance = 1;
        assert!(matches!(
            trial.record_verified(
                &wrong_predecessor,
                observed(&wrong_predecessor, trial_scope)
            ),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        let mut wrong_lane = next.clone();
        wrong_lane.successor.lane.device_lane_id = [0xE5; 32];
        assert!(matches!(
            trial.record_verified(&wrong_lane, observed(&wrong_lane, trial_scope)),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        let before = trial.head_commitment();
        let wrong_scope = KagemushaTestnetStateObservationScopeV1::new(
            trial_scope.network_id(),
            trial_scope.asset_identity_digest(),
            trial_scope.asset_incarnation(),
            trial_scope.asset_scale(),
            trial_scope.liability_pool_id(),
            [0xE6; 32],
            trial_scope.release_attestation_digest(),
        )
        .expect("other release");
        assert_eq!(
            trial.record_verified(&next, observed(&next, wrong_scope)),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
        );
        next.successor.release_id = [0xE6; 32];
        assert_eq!(
            trial.record_verified(&next, observed(&next, trial_scope)),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
        );
        assert_eq!(trial.head_commitment(), before);
        assert_eq!(trial.observed_transitions(), 1);
    }

    #[test]
    fn trial_requires_bootstrap_and_rejects_counter_overflow() {
        let (mut trial, public) = trial_fixture();
        let trial_scope = trial.scope();
        let next = next_trial_public(&public, KagemushaOperationV1::MintFold, 0xE1);
        assert!(matches!(
            trial.record_verified(&next, observed(&next, trial_scope)),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        trial.observed_transitions = u64::MAX;
        assert!(matches!(
            trial.record_verified(&public, observed(&public, trial_scope)),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        assert_eq!(trial.head_commitment(), None);
    }
}
