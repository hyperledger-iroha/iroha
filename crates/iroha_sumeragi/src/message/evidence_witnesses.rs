//! Allocation-free traversal of every native evidence result-witness attachment.
use super::{ByteAdmissionError, Evidence, Qc, ResultWitness, TimeoutCert};
use iroha_allocation::AllocationBudget;

// Generate both borrows from the same exhaustive graph so admission and inspection cannot
// disagree about an attachment. Four is the exact maximum (two per conflicting proposal).
macro_rules! witnesses {
    ($($name:ident, $borrow:ident, [$($mut:tt)?]);* $(;)?) => { $(
        fn $name(&$($mut)? self) -> impl Iterator<Item = &$($mut)? ResultWitness> {
            fn qc(value: &$($mut)? Qc) -> Option<&$($mut)? ResultWitness> {
                value.attestation_witness.$borrow()
            }
            fn tc(value: &$($mut)? TimeoutCert) -> Option<&$($mut)? ResultWitness> {
                value.high_pqc.$borrow().and_then(qc)
            }
            let mut found = [None, None, None, None];
            match self {
                Self::ProposalEquivocation(first, second) => {
                    found[0] = first.parent_qc.$borrow().and_then(qc);
                    found[1] = first.justify.$borrow().and_then(tc);
                    found[2] = second.parent_qc.$borrow().and_then(qc);
                    found[3] = second.justify.$borrow().and_then(tc);
                }
                Self::VoteEquivocation(first, second) => {
                    found[0] = first.attestation.$borrow().map(|a| &$($mut)? a.witness);
                    found[1] = second.attestation.$borrow().map(|a| &$($mut)? a.witness);
                }
                Self::TimeoutEquivocation(first, second) => {
                    found[0] = first.high_pqc.$borrow().and_then(qc);
                    found[1] = second.high_pqc.$borrow().and_then(qc);
                }
                Self::InvalidProposal { proposal, .. } => {
                    found[0] = proposal.parent_qc.$borrow().and_then(qc);
                    found[1] = proposal.justify.$borrow().and_then(tc);
                }
                Self::ConflictingCertificates(first, second) => {
                    found[0] = qc(first);
                    found[1] = qc(second);
                }
            }
            found.into_iter().flatten()
        }
    )* };
}

impl Evidence {
    witnesses! {
        result_witnesses, as_ref, [];
        result_witnesses_mut, as_mut, [mut];
    }

    /// Whether every present witness backing and shared control belongs to this exact pool.
    /// This allocation-free inspection neither validates signatures nor requires attachments
    /// where the protocol demands them, and does not fund the rest of the decoded graph.
    #[must_use]
    pub fn result_witnesses_admitted_to(&self, budget: &AllocationBudget) -> bool {
        self.result_witnesses()
            .all(|witness| witness.admitted_to(budget))
    }

    /// Fund each present witness backing and shared control under the original finite pool.
    /// Canonical bytes are unchanged. Successful and partial admissions stay attached on
    /// refusal; retrying this same proof retains those exact owners. This grants no authority.
    ///
    /// # Errors
    /// Rejects foreign custody, invalid byte-domain geometry or local allocation refusal.
    pub fn admit_result_witnesses(
        &mut self,
        budget: &AllocationBudget,
    ) -> Result<(), ByteAdmissionError> {
        self.result_witnesses_mut()
            .try_for_each(|witness| witness.admit(budget))
    }
}

#[cfg(test)]
mod tests;
