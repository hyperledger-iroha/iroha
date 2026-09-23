//! Non-authorizing testnet observations of actual paired KAGEMUSHA State proofs.
//!
//! This boundary is for a wallet to inspect experimental State lineage on a specifically
//! configured network and authenticated release. It does not issue a monetary admission token,
//! attest an app or device, authorize a terminal transition, or qualify hardware custody.

use iroha_data_model::kagemusha::KagemushaPairedProofV1;

use super::{
    DigestV1, KagemushaAuthenticatedRecursiveVerifierV1, KagemushaOperationV1,
    KagemushaRecursionErrorV1, KagemushaStateRelationPublicInputsV1,
    kagemusha_candidate_envelope_digest_v1, verify_kagemusha_state_proof_v1,
};
use crate::zk::kagemusha_v1_state::KagemushaStateV1;

/// Trusted, exact testnet scope supplied by the operator rather than the wallet proof.
///
/// The authenticated release manifest does not carry a network ID. An application must obtain
/// this network ID from its independently trusted testnet configuration. A self-declared network
/// or release copied from the submitted proof provides no pinning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaTestnetStateObservationScopeV1 {
    network_id: DigestV1,
    release_id: DigestV1,
    release_attestation_digest: DigestV1,
}

impl KagemushaTestnetStateObservationScopeV1 {
    /// Pin the network and complete authenticated release before inspecting a proof.
    ///
    /// # Errors
    ///
    /// Rejects zero or aliased identifiers, which cannot identify an operator-approved scope.
    pub fn new(
        network_id: DigestV1,
        release_id: DigestV1,
        release_attestation_digest: DigestV1,
    ) -> Result<Self, KagemushaRecursionErrorV1> {
        if network_id == [0; 32]
            || release_id == [0; 32]
            || release_attestation_digest == [0; 32]
            || network_id == release_id
            || network_id == release_attestation_digest
            || release_id == release_attestation_digest
        {
            return Err(KagemushaRecursionErrorV1::InvalidArtifacts);
        }
        Ok(Self {
            network_id,
            release_id,
            release_attestation_digest,
        })
    }

    /// Return the independently pinned raw network identifier.
    #[must_use]
    pub const fn network_id(&self) -> DigestV1 {
        self.network_id
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

    fn check_bindings(
        self,
        authenticated_release_id: DigestV1,
        authenticated_release_attestation_digest: DigestV1,
        successor_release_id: DigestV1,
        successor_network_id: DigestV1,
        predecessor: Option<(DigestV1, DigestV1)>,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if authenticated_release_id != self.release_id
            || authenticated_release_attestation_digest != self.release_attestation_digest
            || successor_release_id != self.release_id
            || predecessor.is_some_and(|(release_id, _)| release_id != self.release_id)
        {
            return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
        }
        if successor_network_id != self.network_id
            || predecessor.is_some_and(|(_, network_id)| network_id != self.network_id)
        {
            return Err(KagemushaRecursionErrorV1::StateStatement(
                "KAGEMUSHA State proof is outside the operator-pinned testnet".to_owned(),
            ));
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
    /// Return the operator-pinned network and release used for this observation.
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
/// against that verifier before the owner can be used. No method grants a hardware qualification
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
    scope.check_bindings(
        release_id,
        attestation_digest,
        scope.release_id,
        scope.network_id,
        None,
    )
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

    /// Return the exact network and authenticated release pinned to this trial.
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
    /// Rejects a missing or forked predecessor, a changed lane, a different network or release,
    /// counter overflow, or failure of the native paired-proof verifier.
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
        self.scope.check_bindings(
            self.scope.release_id,
            self.scope.release_attestation_digest,
            public_inputs.successor.release_id,
            public_inputs.successor.lane.normalized_network_id(),
            public_inputs
                .predecessor
                .as_ref()
                .map(|state| (state.release_id, state.lane.normalized_network_id())),
        )?;
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

/// Observe an actual paired State proof under an operator-pinned testnet and signed release.
///
/// This uses the native authenticated verifier to check and decide both Pasta proofs and their
/// histories. The State relation checks its finalized mint, value conservation and replay
/// transitions; this function does not convert that check into permission to spend, receive,
/// redeem, or settle. The production Guard and terminal monetary gates remain separate.
///
/// # Errors
///
/// Rejects a missing authenticated release, wrong network or release pin, malformed public
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
    scope.check_bindings(
        release.release_id(),
        release.attestation_digest(),
        public_inputs.successor.release_id,
        public_inputs.successor.lane.normalized_network_id(),
        public_inputs
            .predecessor
            .as_ref()
            .map(|state| (state.release_id, state.lane.normalized_network_id())),
    )?;
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
    use super::*;

    const NETWORK: DigestV1 = [1; 32];
    const RELEASE: DigestV1 = [2; 32];
    const ATTESTATION: DigestV1 = [3; 32];

    fn scope() -> KagemushaTestnetStateObservationScopeV1 {
        KagemushaTestnetStateObservationScopeV1::new(NETWORK, RELEASE, ATTESTATION)
            .expect("distinct operator pins")
    }

    #[test]
    fn scope_rejects_missing_and_aliased_pins() {
        assert_eq!(scope().network_id(), NETWORK);
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
                KagemushaTestnetStateObservationScopeV1::new(pins.0, pins.1, pins.2),
                Err(KagemushaRecursionErrorV1::InvalidArtifacts)
            ));
        }
    }

    #[test]
    fn scope_checks_signed_release_and_both_state_networks() {
        let scope = scope();
        assert_eq!(
            scope.check_bindings(
                RELEASE,
                ATTESTATION,
                RELEASE,
                NETWORK,
                Some((RELEASE, NETWORK))
            ),
            Ok(())
        );
        for (release_id, attestation, successor_release, predecessor) in [
            ([4; 32], ATTESTATION, RELEASE, None),
            (RELEASE, [4; 32], RELEASE, None),
            (RELEASE, ATTESTATION, [4; 32], None),
            (RELEASE, ATTESTATION, RELEASE, Some(([4; 32], NETWORK))),
        ] {
            assert_eq!(
                scope.check_bindings(
                    release_id,
                    attestation,
                    successor_release,
                    NETWORK,
                    predecessor,
                ),
                Err(KagemushaRecursionErrorV1::ArtifactSubstitution)
            );
        }
        assert!(matches!(
            scope.check_bindings(RELEASE, ATTESTATION, RELEASE, [4; 32], None),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
        assert!(matches!(
            scope.check_bindings(
                RELEASE,
                ATTESTATION,
                RELEASE,
                NETWORK,
                Some((RELEASE, [4; 32]))
            ),
            Err(KagemushaRecursionErrorV1::StateStatement(_))
        ));
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
