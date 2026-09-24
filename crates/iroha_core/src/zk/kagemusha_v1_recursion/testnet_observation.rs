//! Non-authorizing testnet observations of actual paired KAGEMUSHA State proofs.
//!
//! This boundary is for a wallet to inspect experimental State lineage on a specifically
//! configured network, asset reserve, and authenticated release. It does not issue a monetary
//! admission token,
//! attest an app or device, authorize a terminal transition, or qualify hardware custody.

use std::collections::BTreeMap;

use iroha_data_model::{
    isi::kagemusha_v1::{KagemushaFinalityTrustAnchorV1, KagemushaOperationStatusV1},
    kagemusha::{
        KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaMintCreditStatementV1, KagemushaPairedProofV1,
        kagemusha_asset_identity_digest_v1,
    },
};

use super::{
    DigestV1, KagemushaAuthenticatedRecursiveVerifierV1, KagemushaOperationV1,
    KagemushaRecursionErrorV1, KagemushaStateRelationPublicInputsV1,
    kagemusha_candidate_envelope_digest_v1, verify_kagemusha_state_proof_v1,
};
use crate::zk::kagemusha_v1_state::{
    KagemushaStateV1, MintInboxReservationV1, verify_applied_top_up_mint_stage_v1,
};

/// Trusted, exact testnet network, asset, reserve, and release supplied by the operator.
///
/// The operator's independent network pin must match the threshold-signed release manifest.
/// Asset/reserve pins also come from trusted testnet configuration; identifiers copied from a
/// submitted proof provide no pinning.
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
        authenticated_network_id: DigestV1,
        authenticated_release_id: DigestV1,
        authenticated_release_attestation_digest: DigestV1,
    ) -> Result<(), KagemushaRecursionErrorV1> {
        if authenticated_network_id != self.network_id
            || authenticated_release_id != self.release_id
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

/// Applied top-up and paired MintFold proof observed under one exact testnet release.
///
/// This process-local value is deliberately not encoded as a wallet spend capability and never
/// asserts hardware qualification. Its operation and credit IDs name only the verified trial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaTestnetFinalizedMintObservationV1 {
    observation: KagemushaTestnetStateProofObservationV1,
    operation_id: DigestV1,
    credit_id: DigestV1,
    mint_envelope_digest: DigestV1,
}

impl KagemushaTestnetFinalizedMintObservationV1 {
    /// Return the unqualified paired-State observation.
    #[must_use]
    pub const fn state_observation(self) -> KagemushaTestnetStateProofObservationV1 {
        self.observation
    }

    /// Return the unique finalized top-up operation ID.
    #[must_use]
    pub const fn operation_id(self) -> DigestV1 {
        self.operation_id
    }

    /// Return the exact consumed mint credit ID.
    #[must_use]
    pub const fn credit_id(self) -> DigestV1 {
        self.credit_id
    }

    /// Return the release-verified canonical mint envelope digest.
    #[must_use]
    pub const fn mint_envelope_digest(self) -> DigestV1 {
        self.mint_envelope_digest
    }
}

struct RetainedTestnetMintObservationV1 {
    reservation_digest: DigestV1,
    status: KagemushaOperationStatusV1,
    trust_anchor: KagemushaFinalityTrustAnchorV1,
    public_inputs: KagemushaStateRelationPublicInputsV1,
    proof: KagemushaPairedProofV1,
    result: KagemushaTestnetFinalizedMintObservationV1,
}

impl RetainedTestnetMintObservationV1 {
    fn exact_retry_matches(
        &self,
        reservation_digest: DigestV1,
        status: &KagemushaOperationStatusV1,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> bool {
        self.reservation_digest == reservation_digest
            && self.status == *status
            && self.trust_anchor == *trust_anchor
            && self.public_inputs == *public_inputs
            && self.proof == *proof
    }
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
    finalized_mints: BTreeMap<DigestV1, RetainedTestnetMintObservationV1>,
    mint_credit_owners: BTreeMap<DigestV1, DigestV1>,
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
        let release_identity = verifier.monetary_release().map(|release| {
            (
                *release.network_id().as_bytes(),
                release.release_id(),
                release.attestation_digest(),
            )
        });
        require_owner_release_pins(scope, release_identity)?;
        Ok(Self {
            verifier,
            trial: KagemushaTestnetLineageTrialV1::new(scope),
            finalized_mints: BTreeMap::new(),
            mint_credit_owners: BTreeMap::new(),
        })
    }

    /// Verify and append one non-mint paired State proof using only this owner's retained verifier.
    ///
    /// # Errors
    ///
    /// Rejects MintFold without its separate Applied-top-up linkage, or any proof, scope, or
    /// lineage mismatch, without advancing the trial.
    pub fn observe_and_advance(
        &mut self,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<KagemushaTestnetStateProofObservationV1, KagemushaRecursionErrorV1> {
        require_non_mint_observation(public_inputs.operation)?;
        self.trial
            .observe_and_advance(&self.verifier, public_inputs, proof)
    }

    /// Observe one actual Applied top-up and its paired MintFold State proof exactly once.
    ///
    /// The pre-debit reservation, independently pinned finality anchor, exact testnet release,
    /// both mint proofs, and the aggregate State proof must agree before the lineage advances.
    /// A byte-identical process-local retry returns the original observation without advancing;
    /// changed bytes or a second operation consuming the same credit fail closed. This remains
    /// an unqualified trial, not a durable wallet, peer payment, or hardware admission token.
    ///
    /// # Errors
    ///
    /// Rejects a non-Applied or unfinalized result, wrong scope or release, invalid proof,
    /// nonconserving State transition, changed-byte retry, or duplicate mint credit.
    pub fn observe_finalized_mint_and_advance(
        &mut self,
        reservation: &MintInboxReservationV1,
        status: &KagemushaOperationStatusV1,
        trust_anchor: &KagemushaFinalityTrustAnchorV1,
        public_inputs: &KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<KagemushaTestnetFinalizedMintObservationV1, KagemushaRecursionErrorV1> {
        let scope = self.trial.scope();
        if *trust_anchor.network_id.as_bytes() != scope.network_id() {
            return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
        }
        let reservation_digest = reservation
            .digest()
            .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
        let operation_id = reservation.operation_id();
        if let Some(previous) = self.finalized_mints.get(&operation_id) {
            status.validate_against(trust_anchor).map_err(|error| {
                KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string())
            })?;
            if previous.exact_retry_matches(
                reservation_digest,
                status,
                trust_anchor,
                public_inputs,
                proof,
            ) {
                return Ok(previous.result);
            }
            return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
                "testnet top-up retry changed the exact finalized mint transcript".to_owned(),
            ));
        }
        self.trial.check_next(public_inputs)?;
        require_unused_mint_credit(&self.mint_credit_owners, reservation.credit_id().0)?;
        let verified = verify_applied_top_up_mint_stage_v1(
            &self.verifier,
            self.verifier.state_checkpoint_material().artifacts,
            reservation,
            status,
            trust_anchor,
        )
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
        check_finalized_mint_public_binding(
            scope,
            &verified.credit().statement,
            reservation.credit_id().0,
            verified.mint_finality().semantic_digest(),
            verified.mint_finality().proof_binding_digest(),
            public_inputs,
        )?;
        let observation = self
            .trial
            .observe_and_advance(&self.verifier, public_inputs, proof)?;
        let result = KagemushaTestnetFinalizedMintObservationV1 {
            observation,
            operation_id,
            credit_id: reservation.credit_id().0,
            mint_envelope_digest: verified.envelope_digest(),
        };
        self.mint_credit_owners
            .insert(result.credit_id, operation_id);
        self.finalized_mints.insert(
            operation_id,
            RetainedTestnetMintObservationV1 {
                reservation_digest,
                status: status.clone(),
                trust_anchor: *trust_anchor,
                public_inputs: public_inputs.clone(),
                proof: proof.clone(),
                result,
            },
        );
        Ok(result)
    }
}

fn require_non_mint_observation(
    operation: KagemushaOperationV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    if operation == KagemushaOperationV1::MintFold {
        return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
            "testnet MintFold requires the exact Applied top-up and finality anchor".to_owned(),
        ));
    }
    Ok(())
}

fn require_unused_mint_credit(
    credit_owners: &BTreeMap<DigestV1, DigestV1>,
    credit_id: DigestV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    if credit_owners.contains_key(&credit_id) {
        return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
            "testnet mint credit was already consumed by another top-up".to_owned(),
        ));
    }
    Ok(())
}

fn check_finalized_mint_public_binding(
    scope: KagemushaTestnetStateObservationScopeV1,
    statement: &KagemushaMintCreditStatementV1,
    credit_id: DigestV1,
    semantic_digest: DigestV1,
    proof_binding_digest: DigestV1,
    public_inputs: &KagemushaStateRelationPublicInputsV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    scope.check_state_bindings(&public_inputs.successor, public_inputs.predecessor.as_ref())?;
    let predecessor = public_inputs.predecessor.as_ref().ok_or_else(|| {
        KagemushaRecursionErrorV1::MintFinalityBinding(
            "testnet MintFold has no exact predecessor".to_owned(),
        )
    })?;
    let lifecycle = &statement.lifecycle;
    let state = &public_inputs.successor;
    let statement_asset = kagemusha_asset_identity_digest_v1(&lifecycle.asset)
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    let statement_digest = statement
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::MintFinalityBinding(error.to_string()))?;
    if public_inputs.operation != KagemushaOperationV1::MintFold
        || lifecycle.operation_kind
            != iroha_data_model::kagemusha::KagemushaOperationKindV1::MintFold
        || credit_id == [0; 32]
        || lifecycle.credit_id != credit_id
        || semantic_digest == [0; 32]
        || proof_binding_digest == [0; 32]
        || statement.amount == 0
        || statement.amount != public_inputs.amount
        || predecessor.balance.checked_add(statement.amount) != Some(state.balance)
        || statement_digest != semantic_digest
        || public_inputs.mint_finality_semantic_digest != semantic_digest
        || public_inputs.mint_finality_proof_binding_digest != proof_binding_digest
        || *lifecycle.network_id.as_bytes() != scope.network_id()
        || statement_asset != scope.asset_identity_digest()
        || *lifecycle.asset_incarnation.as_bytes() != scope.asset_incarnation()
        || lifecycle.scale != scope.asset_scale()
        || lifecycle.liability_pool_id != scope.liability_pool_id()
        || lifecycle.release_id != scope.release_id()
        || lifecycle.network_id != state.lane.network_id
        || lifecycle.asset != state.lane.asset
        || lifecycle.asset_incarnation != state.asset_incarnation
        || lifecycle.scale != state.lane.scale
        || lifecycle.liability_pool_id != state.liability_pool_id
        || lifecycle.suite_id != state.suite_id
        || lifecycle.vk_digest != state.vk_digest
        || lifecycle.release_id != state.release_id
        || lifecycle.hardware_profile_id != state.hardware_profile_id
        || lifecycle.policy_epoch != state.policy_epoch
    {
        return Err(KagemushaRecursionErrorV1::MintFinalityBinding(
            "finalized testnet top-up and MintFold public inputs disagree".to_owned(),
        ));
    }
    Ok(())
}

fn require_owner_release_pins(
    scope: KagemushaTestnetStateObservationScopeV1,
    authenticated_release_identity: Result<(DigestV1, DigestV1, DigestV1), String>,
) -> Result<(), KagemushaRecursionErrorV1> {
    let (network_id, release_id, attestation_digest) =
        authenticated_release_identity.map_err(KagemushaRecursionErrorV1::StateProofRejected)?;
    scope.check_release_bindings(network_id, release_id, attestation_digest)
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
    scope.check_release_bindings(
        *release.network_id().as_bytes(),
        release.release_id(),
        release.attestation_digest(),
    )?;
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
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        consensus::v2::HeightContextId,
        isi::kagemusha_v1::{
            KAGEMUSHA_CHAIN_VERSION_V1, KagemushaOperationKindV1 as ChainOperationKindV1,
            KagemushaOperationStateV1,
        },
    };

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
            scope.check_release_bindings(
                scope.network_id(),
                scope.release_id(),
                scope.release_attestation_digest(),
            ),
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
            scope.check_release_bindings(
                scope.network_id(),
                [0xA2; 32],
                scope.release_attestation_digest(),
            ),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution),
        );
        assert_eq!(
            scope.check_release_bindings(scope.network_id(), scope.release_id(), [0xA3; 32]),
            Err(KagemushaRecursionErrorV1::ArtifactSubstitution),
        );
        assert_eq!(
            scope.check_release_bindings(
                [0xA4; 32],
                scope.release_id(),
                scope.release_attestation_digest()
            ),
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
            require_owner_release_pins(scope, Ok((NETWORK, RELEASE, ATTESTATION))),
            Ok(())
        );
        assert!(matches!(
            require_owner_release_pins(scope, Err("release not authorized".to_owned())),
            Err(KagemushaRecursionErrorV1::StateProofRejected(_))
        ));
        for identity in [
            ([4; 32], RELEASE, ATTESTATION),
            (NETWORK, [4; 32], ATTESTATION),
            (NETWORK, RELEASE, [4; 32]),
        ] {
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

    fn mint_binding_fixture() -> (
        KagemushaTestnetStateObservationScopeV1,
        KagemushaStateRelationPublicInputsV1,
        KagemushaMintCreditStatementV1,
        DigestV1,
        DigestV1,
    ) {
        // Structural policy inputs only: this helper does not construct a proof or finality seal.
        let (trial, bootstrap) = trial_fixture();
        let mut public = next_trial_public(&bootstrap, KagemushaOperationV1::MintFold, 0xD4);
        let mut statement = super::super::tests::compact_mint_credit_fixture().statement;
        let state = &public.successor;
        statement.lifecycle.network_id = state.lane.network_id;
        statement.lifecycle.asset = state.lane.asset.clone();
        statement.lifecycle.asset_incarnation = state.asset_incarnation;
        statement.lifecycle.scale = state.lane.scale;
        statement.lifecycle.liability_pool_id = state.liability_pool_id;
        statement.lifecycle.suite_id = state.suite_id;
        statement.lifecycle.vk_digest = state.vk_digest;
        statement.lifecycle.release_id = state.release_id;
        statement.lifecycle.hardware_profile_id = state.hardware_profile_id;
        statement.lifecycle.policy_epoch = state.policy_epoch;
        statement.amount = 7;
        statement.lifecycle.credit_id = statement.expected_credit_id().expect("mint credit ID");
        public.amount = statement.amount;
        public.successor.balance = bootstrap.successor.balance + statement.amount;
        let semantic = statement
            .canonical_digest()
            .expect("canonical mint statement");
        let binding = [0xC3; 32];
        public.mint_finality_semantic_digest = semantic;
        public.mint_finality_proof_binding_digest = binding;
        (trial.scope(), public, statement, semantic, binding)
    }

    #[test]
    fn finalized_mint_binding_requires_exact_scope_funding_and_conservation() {
        let (scope, public, statement, semantic, binding) = mint_binding_fixture();
        let credit_id = statement.lifecycle.credit_id;
        let check = |statement: &KagemushaMintCreditStatementV1,
                     public: &KagemushaStateRelationPublicInputsV1,
                     credit_id,
                     semantic,
                     binding| {
            check_finalized_mint_public_binding(
                scope, statement, credit_id, semantic, binding, public,
            )
        };
        assert_eq!(
            check(&statement, &public, credit_id, semantic, binding),
            Ok(())
        );
        assert!(check(&statement, &public, [0; 32], semantic, binding).is_err());
        assert!(check(&statement, &public, credit_id, [0; 32], binding).is_err());
        assert!(check(&statement, &public, credit_id, semantic, [0; 32]).is_err());
        let mut changed = public.clone();
        changed.amount += 1;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        changed = public.clone();
        changed.successor.balance += 1;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        changed = public.clone();
        changed.mint_finality_semantic_digest[0] ^= 1;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        changed = public.clone();
        changed.mint_finality_proof_binding_digest[0] ^= 1;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        changed = public.clone();
        changed.predecessor = None;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        changed = public.clone();
        changed.operation = KagemushaOperationV1::SendSplit;
        assert!(check(&statement, &changed, credit_id, semantic, binding).is_err());
        let mut changed_statement = statement.clone();
        changed_statement.lifecycle.release_id[0] ^= 1;
        changed_statement.lifecycle.credit_id = changed_statement
            .expected_credit_id()
            .expect("altered release credit ID");
        let changed_semantic = changed_statement
            .canonical_digest()
            .expect("canonical altered release statement");
        let mut changed_public = public.clone();
        changed_public.mint_finality_semantic_digest = changed_semantic;
        assert!(
            check(
                &changed_statement,
                &changed_public,
                changed_statement.lifecycle.credit_id,
                changed_semantic,
                binding
            )
            .is_err()
        );
        changed_statement = statement.clone();
        changed_statement.lifecycle.liability_pool_id[0] ^= 1;
        // The pool is derived from the network, asset and incarnation, so a different
        // pool cannot form a canonical statement to reach the later scope check.
        assert!(changed_statement.canonical_digest().is_err());
    }

    #[test]
    fn exact_mint_retry_retains_original_transcript_and_unqualified_result() {
        let (trial, public) = trial_fixture();
        let (_, proof) = super::super::tests::state_verification_fixture();
        let status = KagemushaOperationStatusV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [0xB1; 32],
            kind: ChainOperationKindV1::TopUp,
            state: KagemushaOperationStateV1::Pending,
            result: None,
            rejection: None,
        };
        let anchor = KagemushaFinalityTrustAnchorV1 {
            network_id: public.successor.lane.network_id,
            block_height: 1,
            height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"testnet mint retry context",
            ))),
        };
        let result = KagemushaTestnetFinalizedMintObservationV1 {
            observation: observed(&public, trial.scope()),
            operation_id: status.operation_id,
            credit_id: [0xB2; 32],
            mint_envelope_digest: [0xB3; 32],
        };
        assert_eq!(result.state_observation().operation(), public.operation);
        assert_eq!(result.operation_id(), status.operation_id);
        assert_eq!(result.credit_id(), [0xB2; 32]);
        assert_eq!(result.mint_envelope_digest(), [0xB3; 32]);
        assert!(!result.state_observation().hardware_qualified());
        let retained = RetainedTestnetMintObservationV1 {
            reservation_digest: [0xB4; 32],
            status: status.clone(),
            trust_anchor: anchor,
            public_inputs: public.clone(),
            proof: proof.clone(),
            result,
        };
        assert!(retained.exact_retry_matches([0xB4; 32], &status, &anchor, &public, &proof));
        assert!(!retained.exact_retry_matches([0xB5; 32], &status, &anchor, &public, &proof));
        let mut changed = status.clone();
        changed.operation_id[0] ^= 1;
        assert!(!retained.exact_retry_matches([0xB4; 32], &changed, &anchor, &public, &proof));
        let changed_anchor = KagemushaFinalityTrustAnchorV1 {
            block_height: 2,
            ..anchor
        };
        assert!(!retained.exact_retry_matches(
            [0xB4; 32],
            &status,
            &changed_anchor,
            &public,
            &proof
        ));
        let mut changed = public.clone();
        changed.amount += 1;
        assert!(!retained.exact_retry_matches([0xB4; 32], &status, &anchor, &changed, &proof));
        let mut changed = proof.clone();
        changed.semantic_digest[0] ^= 1;
        assert!(!retained.exact_retry_matches([0xB4; 32], &status, &anchor, &public, &changed));
    }

    #[test]
    fn testnet_mint_credit_identity_cannot_be_consumed_twice() {
        let mut credit_owners = BTreeMap::new();
        let credit = [0xF1; 32];
        assert_eq!(require_unused_mint_credit(&credit_owners, credit), Ok(()));
        credit_owners.insert(credit, [0xF2; 32]);
        assert!(matches!(
            require_unused_mint_credit(&credit_owners, credit),
            Err(KagemushaRecursionErrorV1::MintFinalityBinding(_))
        ));
        assert_eq!(
            require_unused_mint_credit(&credit_owners, [0xF3; 32]),
            Ok(())
        );
    }

    #[test]
    fn generic_testnet_owner_observation_cannot_advance_mint_without_finality() {
        assert!(matches!(
            require_non_mint_observation(KagemushaOperationV1::MintFold),
            Err(KagemushaRecursionErrorV1::MintFinalityBinding(_))
        ));
        for operation in [
            KagemushaOperationV1::Bootstrap,
            KagemushaOperationV1::SendSplit,
            KagemushaOperationV1::ReceiveFold,
            KagemushaOperationV1::RedeemSplit,
            KagemushaOperationV1::Rotate,
        ] {
            assert_eq!(require_non_mint_observation(operation), Ok(()));
        }
    }
}
