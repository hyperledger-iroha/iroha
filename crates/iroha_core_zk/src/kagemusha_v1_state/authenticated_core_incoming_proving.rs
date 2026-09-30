//! Borrowed proving admission from the actual exclusive, descriptor-held incoming attempt.
//!
//! A decoded preview, public credit or proof never constructs this token. It borrows the
//! consumed native owner before completion, retains its exact original journal and exposes
//! only the original inputs required by the genuine paired State circuit. Proving does not
//! apply a fold, perform hardware CAS or publish a new usable owner.

use super::*;
use iroha_data_model::kagemusha::{KagemushaMintAuthorizationV1, KagemushaMintCreditV1};

/// Non-cloneable proving selection borrowing one actual native prepared incoming fold.
/// No serialization, public constructor or caller-selected State/credit overload exists.
// Keep this public token non-Clone and non-Copy; each new selection must
// pass proving_selection's explicit native original recheck.
#[allow(missing_copy_implementations)]
pub struct KagemushaAuthenticatedIncomingProvingSelectionV1<'a> {
    fold: &'a KagemushaAuthenticatedIncomingFoldV1,
}

impl KagemushaAuthenticatedIncomingFoldV1 {
    /// Borrow this original before completion; the owner cannot mutate while it is borrowed.
    pub fn proving_selection(
        &self,
    ) -> Result<KagemushaAuthenticatedIncomingProvingSelectionV1<'_>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        Ok(KagemushaAuthenticatedIncomingProvingSelectionV1 { fold: self })
    }
}

impl KagemushaAuthenticatedIncomingProvingSelectionV1<'_> {
    /// Recheck original WAL bytes, complete predecessor selection, native history and descriptors.
    /// Once completion begins this surface closes; retry cannot become a new proving attempt.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.fold.recheck_originals()
    }

    /// Exact production release retained by the genuine native recursive and Guard owners.
    pub fn authenticated_release(
        &self,
    ) -> Result<
        Arc<iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1>,
        KagemushaStateErrorV1,
    > {
        self.recheck()?;
        let release = self
            .fold
            .owner
            .machine
            .guard_verifier
            .authenticated_release()
            .map_err(|e| KagemushaStateErrorV1::GuardRejected(e.to_string()))?;
        self.recheck()?;
        Ok(release)
    }

    /// Original native history operation identity, not a replacement proving nonce.
    pub fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.fold.history_operation_id())
    }

    /// Original preview; consumers must retain this borrow under independently qualified custody.
    pub fn transition(&self) -> Result<&TransitionPreviewV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.fold.transition())
    }

    /// Actual selected native predecessor and its checked exact successor.
    pub fn private_state_link(
        &self,
    ) -> Result<(&KagemushaStateV1, &KagemushaStateV1), KagemushaStateErrorV1> {
        self.recheck()?;
        Ok((
            &self.fold.intent.previous.state,
            &self.fold.transition().successor,
        ))
    }

    /// Checked-preview mint opening, present only for the original mint branch.
    pub fn mint_fold_opening(
        &self,
    ) -> Result<Option<KagemushaMintFoldOpeningCapabilityV1<'_>>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(match &self.fold.preview {
            Preview::Mint(preview) => Some(preview.mint_fold_opening()),
            Preview::Receive(_) => None,
        })
    }

    /// Exact authenticated mint authorization and finalized credit; never padding inputs.
    pub fn mint_originals(
        &self,
    ) -> Result<
        Option<(&KagemushaMintAuthorizationV1, &KagemushaMintCreditV1)>,
        KagemushaStateErrorV1,
    > {
        self.recheck()?;
        Ok(match &self.fold.preview {
            Preview::Mint(preview) => {
                let opening = preview.mint_fold_opening().opening();
                Some((opening.authorization(), opening.credit()))
            }
            Preview::Receive(_) => None,
        })
    }

    /// Exact staged peer request/payment and native authenticated plaintext/replay opening.
    pub fn peer_originals(
        &self,
    ) -> Result<
        Option<(
            &KagemushaPaymentRequestV1,
            &KagemushaPaymentV1,
            &PeerCreditFoldInputV1,
        )>,
        KagemushaStateErrorV1,
    > {
        self.recheck()?;
        Ok(match &self.fold.preview {
            Preview::Mint(_) => None,
            Preview::Receive(preview) => {
                let staged = self
                    .fold
                    .owner
                    .machine
                    .pending_credits
                    .get(&self.fold.intent.credit_id)
                    .ok_or(KagemushaStateErrorV1::CreditNotStaged(
                        self.fold.intent.credit_id,
                    ))?;
                Some((&staged.request, &staged.payment, &preview.credit))
            }
        })
    }

    /// Exact original sparse replay insertion from the checked native preview.
    pub fn replay_insert(&self) -> Result<&ConsumedCreditInsertWitnessV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(match &self.fold.preview {
            Preview::Mint(preview) => &preview.replay_insert_witness,
            Preview::Receive(preview) => &preview.credit.replay_insert_witness,
        })
    }

    /// Independently reverify generated State bytes against exact selected transition semantics.
    /// This pure verification grants no history CAS, Guard, hardware selection or wallet.
    pub fn authenticate_pair(
        &self,
        proof: &KagemushaPairedProofV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let machine = &self.fold.owner.machine;
        let preview = self.fold.transition();
        validate_paired_proof(proof, preview.transport_semantic_digest)?;
        let inputs = transition_state_public_inputs(
            machine.proof_release.artifacts,
            &machine.state,
            preview,
            proof,
        )?;
        verify_kagemusha_state_proof_v1(
            &machine.recursive_verifier,
            machine.proof_release.artifacts,
            &inputs,
            proof,
        )
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
        self.recheck()
    }
}
