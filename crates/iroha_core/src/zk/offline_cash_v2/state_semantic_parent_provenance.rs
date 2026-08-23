//! Move-only semantic-parent provenance for the Offline Cash V2 STATE fold.
//!
//! One semantic STATE parent is represented by its exact Eq and Ep public
//! instances. The two public statements must agree word-for-word except for
//! parity word 3, the parity-specific STATE protocol words 16..24, and the
//! parity-local predecessor-lineage words 93..237. This source-only contract
//! owns those statements through the eventual fold input, but does not verify
//! a proof, implement BGH19, load an artifact, add a wire type, or authorize a
//! production path.

use core::{convert::Infallible, fmt};

use super::{
    OfflineCashHalo2CircuitRoleV2, OfflineCashHalo2ParityV2,
    guard_bundle_provenance::offline_cash_halo2_protocol_source_identity_v2,
    state_lineage::{
        OFFLINE_CASH_STATE_DIGEST_WORDS_V2, OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2,
        OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2, OfflineCashStatePublicInstancesV2,
    },
    state_recursive_fold::{CanonicalStateAccumulatorV2, StateRecursiveFoldParityV2},
};

const STATE_PARITY_WORD_V2: usize = 3;
const STATE_PROTOCOL_WORD_END_V2: usize =
    OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2 + OFFLINE_CASH_STATE_DIGEST_WORDS_V2;

/// The move-only semantic-parent provenance contract is implemented.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROVENANCE_CONTRACT_IMPLEMENTED_V2: bool = true;
/// No ordinary STATE proof verifier is available through this source contract.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROOF_VERIFIER_AVAILABLE_V2: bool = false;
/// This ownership-only tranche adds no wire representation.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_PARENT_WIRE_AVAILABLE_V2: bool = false;
/// This ownership-only tranche authenticates no artifact.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_PARENT_ARTIFACTS_AUTHENTICATED_V2: bool = false;
/// This ownership-only tranche cannot authorize production.
pub(super) const OFFLINE_CASH_STATE_SEMANTIC_PARENT_PRODUCTION_AVAILABLE_V2: bool = false;

const _: () = assert!(STATE_PARITY_WORD_V2 == 3);
const _: () = assert!(OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2 == 16);
const _: () = assert!(STATE_PROTOCOL_WORD_END_V2 == 24);
const _: () = assert!(OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2 == 93);
const _: () = assert!(OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROVENANCE_CONTRACT_IMPLEMENTED_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROOF_VERIFIER_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_WIRE_AVAILABLE_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_ARTIFACTS_AUTHENTICATED_V2);
const _: () = assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_PRODUCTION_AVAILABLE_V2);

/// Failure while binding one paired-parity STATE parent to a fold position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OfflineCashStateSemanticParentProvenanceErrorV2 {
    /// The first owned instance was not the Eq statement.
    EqInstanceParityMismatch,
    /// The second owned instance was not the Ep statement.
    EpInstanceParityMismatch,
    /// Eq protocol words did not name the exact source-only STATE identity.
    EqStateProtocolSourceIdentityMismatch,
    /// Ep protocol words did not name the exact source-only STATE identity.
    EpStateProtocolSourceIdentityMismatch,
    /// A word which must be common across parities differed.
    CommonStatementWordMismatch { word: usize },
    /// The Eq public tail was not one canonical live Eq lineage.
    InvalidEqParentLineage,
    /// The Ep public tail was not one canonical live Ep lineage.
    InvalidEpParentLineage,
    /// A reserved all-zero bootstrap tail reached the live parent boundary.
    UnauthenticatedBootstrap,
    /// A proof-derived current accumulator used the other Pasta parity.
    CurrentAccumulatorParityMismatch,
    /// No ordinary STATE proof verifier is installed.
    VerificationUnavailable,
}

impl fmt::Display for OfflineCashStateSemanticParentProvenanceErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EqInstanceParityMismatch => {
                formatter.write_str("offline-cash V2 semantic parent Eq instance has wrong parity")
            }
            Self::EpInstanceParityMismatch => {
                formatter.write_str("offline-cash V2 semantic parent Ep instance has wrong parity")
            }
            Self::EqStateProtocolSourceIdentityMismatch => formatter
                .write_str("offline-cash V2 semantic parent Eq STATE source identity differs"),
            Self::EpStateProtocolSourceIdentityMismatch => formatter
                .write_str("offline-cash V2 semantic parent Ep STATE source identity differs"),
            Self::CommonStatementWordMismatch { word } => write!(
                formatter,
                "offline-cash V2 semantic parent common statement word {word} differs"
            ),
            Self::InvalidEqParentLineage => {
                formatter.write_str("offline-cash V2 semantic parent Eq lineage is non-canonical")
            }
            Self::InvalidEpParentLineage => {
                formatter.write_str("offline-cash V2 semantic parent Ep lineage is non-canonical")
            }
            Self::UnauthenticatedBootstrap => formatter
                .write_str("offline-cash V2 semantic parent bootstrap lineage is unauthenticated"),
            Self::CurrentAccumulatorParityMismatch => formatter
                .write_str("offline-cash V2 semantic parent current accumulator has wrong parity"),
            Self::VerificationUnavailable => {
                formatter.write_str("offline-cash V2 semantic parent verification is unavailable")
            }
        }
    }
}

impl std::error::Error for OfflineCashStateSemanticParentProvenanceErrorV2 {}

/// Exact paired-parity public statements for one unverified semantic STATE parent.
///
/// This owner is deliberately neither `Clone` nor `Copy`. Its instances can be
/// observed only by shared borrow and move intact into a verified handoff.
pub(super) struct UnverifiedOfflineCashStateSemanticParentPairV2 {
    eq_instances: OfflineCashStatePublicInstancesV2,
    ep_instances: OfflineCashStatePublicInstancesV2,
}

impl UnverifiedOfflineCashStateSemanticParentPairV2 {
    /// Join exact Eq-then-Ep STATE public instances for one semantic parent.
    pub(super) fn from_eq_then_ep(
        eq_instances: OfflineCashStatePublicInstancesV2,
        ep_instances: OfflineCashStatePublicInstancesV2,
    ) -> Result<Self, OfflineCashStateSemanticParentProvenanceErrorV2> {
        if eq_instances.parity() != OfflineCashHalo2ParityV2::Eq {
            return Err(OfflineCashStateSemanticParentProvenanceErrorV2::EqInstanceParityMismatch);
        }
        if ep_instances.parity() != OfflineCashHalo2ParityV2::Ep {
            return Err(OfflineCashStateSemanticParentProvenanceErrorV2::EpInstanceParityMismatch);
        }

        let expected_eq_protocol = offline_cash_halo2_protocol_source_identity_v2(
            OfflineCashHalo2ParityV2::Eq,
            OfflineCashHalo2CircuitRoleV2::State,
        )
        .digest();
        if read_state_digest_v2(
            eq_instances.words(),
            OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2,
        ) != expected_eq_protocol
        {
            return Err(
                OfflineCashStateSemanticParentProvenanceErrorV2::EqStateProtocolSourceIdentityMismatch,
            );
        }
        let expected_ep_protocol = offline_cash_halo2_protocol_source_identity_v2(
            OfflineCashHalo2ParityV2::Ep,
            OfflineCashHalo2CircuitRoleV2::State,
        )
        .digest();
        if read_state_digest_v2(
            ep_instances.words(),
            OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2,
        ) != expected_ep_protocol
        {
            return Err(
                OfflineCashStateSemanticParentProvenanceErrorV2::EpStateProtocolSourceIdentityMismatch,
            );
        }

        compare_common_state_words_v2(eq_instances.words(), ep_instances.words())?;

        let eq_lineage = eq_instances
            .eq_parent_lineage()
            .map_err(|_| OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEqParentLineage)?;
        let ep_lineage = ep_instances
            .ep_parent_lineage()
            .map_err(|_| OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEpParentLineage)?;
        if eq_lineage.is_bootstrap() || ep_lineage.is_bootstrap() {
            return Err(OfflineCashStateSemanticParentProvenanceErrorV2::UnauthenticatedBootstrap);
        }

        Ok(Self {
            eq_instances,
            ep_instances,
        })
    }

    /// Borrow the exact owned Eq public instances.
    pub(super) const fn eq_instances(&self) -> &OfflineCashStatePublicInstancesV2 {
        &self.eq_instances
    }

    /// Borrow the exact owned Ep public instances.
    pub(super) const fn ep_instances(&self) -> &OfflineCashStatePublicInstancesV2 {
        &self.ep_instances
    }
}

fn compare_common_state_words_v2(
    eq_words: &[u32],
    ep_words: &[u32],
) -> Result<(), OfflineCashStateSemanticParentProvenanceErrorV2> {
    for range in [
        0..STATE_PARITY_WORD_V2,
        STATE_PARITY_WORD_V2 + 1..OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2,
        STATE_PROTOCOL_WORD_END_V2..OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2,
    ] {
        for word in range {
            if eq_words[word] != ep_words[word] {
                return Err(
                    OfflineCashStateSemanticParentProvenanceErrorV2::CommonStatementWordMismatch {
                        word,
                    },
                );
            }
        }
    }
    Ok(())
}

fn read_state_digest_v2(words: &[u32], start: usize) -> [u8; 32] {
    let mut digest = [0_u8; 32];
    for (chunk, word) in digest
        .chunks_exact_mut(4)
        .zip(&words[start..start + OFFLINE_CASH_STATE_DIGEST_WORDS_V2])
    {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    digest
}

/// Uninhabited authority for ordinary STATE proof verification and accumulation.
pub(super) enum OfflineCashStateSemanticParentProofVerifierAuthorityV2 {}

/// Move-only handoff which can exist only after both ordinary STATE proofs verify.
pub(super) struct VerifiedOfflineCashStateSemanticParentHandoffV2 {
    provenance: UnverifiedOfflineCashStateSemanticParentPairV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

/// Sole production-shaped constructor; impossible while verifier authority is uninhabited.
pub(super) fn verify_offline_cash_state_semantic_parent_for_fold_v2(
    _provenance: UnverifiedOfflineCashStateSemanticParentPairV2,
    _eq_current: CanonicalStateAccumulatorV2,
    _ep_current: CanonicalStateAccumulatorV2,
    authority: OfflineCashStateSemanticParentProofVerifierAuthorityV2,
) -> Result<
    VerifiedOfflineCashStateSemanticParentHandoffV2,
    OfflineCashStateSemanticParentProvenanceErrorV2,
> {
    match authority {}
}

impl VerifiedOfflineCashStateSemanticParentHandoffV2 {
    /// Test-only stand-in for proof verification; priors are derived from owned tails.
    #[cfg(test)]
    pub(super) fn from_test_verified_parts_v2(
        provenance: UnverifiedOfflineCashStateSemanticParentPairV2,
        eq_current: CanonicalStateAccumulatorV2,
        ep_current: CanonicalStateAccumulatorV2,
    ) -> Result<Self, OfflineCashStateSemanticParentProvenanceErrorV2> {
        if eq_current.parity() != StateRecursiveFoldParityV2::Eq
            || ep_current.parity() != StateRecursiveFoldParityV2::Ep
        {
            return Err(
                OfflineCashStateSemanticParentProvenanceErrorV2::CurrentAccumulatorParityMismatch,
            );
        }
        let eq_prior = CanonicalStateAccumulatorV2::decode(
            StateRecursiveFoldParityV2::Eq,
            &provenance
                .eq_instances()
                .eq_parent_lineage()
                .map_err(|_| {
                    OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEqParentLineage
                })?
                .encode(),
        )
        .map_err(|_| OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEqParentLineage)?;
        let ep_prior = CanonicalStateAccumulatorV2::decode(
            StateRecursiveFoldParityV2::Ep,
            &provenance
                .ep_instances()
                .ep_parent_lineage()
                .map_err(|_| {
                    OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEpParentLineage
                })?
                .encode(),
        )
        .map_err(|_| OfflineCashStateSemanticParentProvenanceErrorV2::InvalidEpParentLineage)?;
        Ok(Self {
            provenance,
            eq_current,
            eq_prior,
            ep_current,
            ep_prior,
        })
    }

    /// Consume the verified handoff into opaque, still-unpositioned fold parts.
    pub(super) fn into_fold_accumulator_parts_v2(
        self,
    ) -> OfflineCashStateSemanticParentAccumulatorPartsV2 {
        OfflineCashStateSemanticParentAccumulatorPartsV2 {
            provenance_seal: OfflineCashStateSemanticParentProvenanceSealV2(self.provenance),
            eq_current: self.eq_current,
            eq_prior: self.eq_prior,
            ep_current: self.ep_current,
            ep_prior: self.ep_prior,
        }
    }
}

/// Opaque ownership seal retained beside every position-bound accumulator view.
pub(super) struct OfflineCashStateSemanticParentProvenanceSealV2(
    UnverifiedOfflineCashStateSemanticParentPairV2,
);

impl OfflineCashStateSemanticParentProvenanceSealV2 {
    /// Borrow the paired statements without separating them from the seal.
    pub(super) const fn provenance(&self) -> &UnverifiedOfflineCashStateSemanticParentPairV2 {
        &self.0
    }
}

/// Opaque accumulator parts emitted only by the verified semantic-parent handoff.
///
/// The only exits consume the entire value into one exact parent position.
pub(super) struct OfflineCashStateSemanticParentAccumulatorPartsV2 {
    provenance_seal: OfflineCashStateSemanticParentProvenanceSealV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

impl OfflineCashStateSemanticParentAccumulatorPartsV2 {
    /// Consume this parent into the immutable P0 position.
    pub(super) fn bind_parent_0_v2(self) -> ProvenanceBoundStateParent0InputsV2 {
        ProvenanceBoundStateParent0InputsV2 {
            provenance_seal: self.provenance_seal,
            eq_current: self.eq_current,
            eq_prior: self.eq_prior,
            ep_current: self.ep_current,
            ep_prior: self.ep_prior,
        }
    }

    /// Consume this parent into the immutable P1 position.
    pub(super) fn bind_parent_1_v2(self) -> ProvenanceBoundStateParent1InputsV2 {
        ProvenanceBoundStateParent1InputsV2 {
            provenance_seal: self.provenance_seal,
            eq_current: self.eq_current,
            eq_prior: self.eq_prior,
            ep_current: self.ep_current,
            ep_prior: self.ep_prior,
        }
    }
}

/// Move-only P0 STATE inputs retaining the exact paired-statement provenance seal.
pub(super) struct ProvenanceBoundStateParent0InputsV2 {
    provenance_seal: OfflineCashStateSemanticParentProvenanceSealV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

impl ProvenanceBoundStateParent0InputsV2 {
    pub(super) const fn provenance_seal(&self) -> &OfflineCashStateSemanticParentProvenanceSealV2 {
        &self.provenance_seal
    }

    pub(super) const fn eq_current(&self) -> &CanonicalStateAccumulatorV2 {
        &self.eq_current
    }

    pub(super) const fn eq_prior(&self) -> &CanonicalStateAccumulatorV2 {
        &self.eq_prior
    }

    pub(super) const fn ep_current(&self) -> &CanonicalStateAccumulatorV2 {
        &self.ep_current
    }

    pub(super) const fn ep_prior(&self) -> &CanonicalStateAccumulatorV2 {
        &self.ep_prior
    }
}

/// Move-only P1 STATE inputs retaining the exact paired-statement provenance seal.
pub(super) struct ProvenanceBoundStateParent1InputsV2 {
    provenance_seal: OfflineCashStateSemanticParentProvenanceSealV2,
    eq_current: CanonicalStateAccumulatorV2,
    eq_prior: CanonicalStateAccumulatorV2,
    ep_current: CanonicalStateAccumulatorV2,
    ep_prior: CanonicalStateAccumulatorV2,
}

impl ProvenanceBoundStateParent1InputsV2 {
    pub(super) const fn provenance_seal(&self) -> &OfflineCashStateSemanticParentProvenanceSealV2 {
        &self.provenance_seal
    }

    pub(super) const fn eq_current(&self) -> &CanonicalStateAccumulatorV2 {
        &self.eq_current
    }

    pub(super) const fn eq_prior(&self) -> &CanonicalStateAccumulatorV2 {
        &self.eq_prior
    }

    pub(super) const fn ep_current(&self) -> &CanonicalStateAccumulatorV2 {
        &self.ep_current
    }

    pub(super) const fn ep_prior(&self) -> &CanonicalStateAccumulatorV2 {
        &self.ep_prior
    }
}

/// Fail closed before unverified paired statements can cross the proof boundary.
pub(super) fn fail_closed_offline_cash_state_semantic_parent_boundary_v2(
    _provenance: UnverifiedOfflineCashStateSemanticParentPairV2,
) -> Result<Infallible, OfflineCashStateSemanticParentProvenanceErrorV2> {
    Err(OfflineCashStateSemanticParentProvenanceErrorV2::VerificationUnavailable)
}

#[cfg(test)]
#[path = "state_semantic_parent_provenance_tests.rs"]
mod tests;
