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
}
