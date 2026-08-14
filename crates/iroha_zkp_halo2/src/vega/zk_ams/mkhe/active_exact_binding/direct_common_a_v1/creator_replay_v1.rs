//! Consuming H0/H1 creator replay typestate for the common `a` authority.

use super::super::{PersistentDirectRelationUseSelectorV1, VerifiedPersistentWitnessBindingSetV1};
use super::{
    DirectCommonAStatementStreamV1, VerifiedDirectCommonAStatementV1,
    derive_verified_direct_common_a_statement_v1, new_rkg_round_one_selector_v1,
};
use crate::vega::zk_ams::mkhe::{
    ZkAmsMkheErrorV1, active::ZkAmsMkheGovernedActiveRosterV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectCeremonyContextV1,
};

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectCommonACreatorH0ReadyV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    authority: VerifiedDirectCommonAStatementV1,
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectCommonACreatorH0ReplayV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    expected_statement_digest: [u8; 32],
    authority: VerifiedDirectCommonAStatementV1,
    stream: DirectCommonAStatementStreamV1,
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectCommonACreatorH1ReadyV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    authority: VerifiedDirectCommonAStatementV1,
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct DirectCommonACreatorH1ReplayV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    expected_statement_digest: [u8; 32],
    authority: VerifiedDirectCommonAStatementV1,
    stream: DirectCommonAStatementStreamV1,
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) struct CompletedDirectCommonACreatorAuthorityV1
{
    context: ZkAmsMkheDirectCeremonyContextV1,
    authority: VerifiedDirectCommonAStatementV1,
}

impl CompletedDirectCommonACreatorAuthorityV1 {
    /// Write the bound statement digest into its canonical destination without
    /// exposing a reusable raw-digest accessor outside the common-`a` child.
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn write_statement_digest_v1(
        &self,
        context: ZkAmsMkheDirectCeremonyContextV1,
        destination: &mut [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if context != self.context {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        destination.copy_from_slice(&self.authority.statement_digest_for(context)?);
        Ok(())
    }
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn prepare_direct_common_a_creator_h0_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
) -> Result<DirectCommonACreatorH0ReadyV1, ZkAmsMkheErrorV1> {
    Ok(DirectCommonACreatorH0ReadyV1 {
        context,
        authority: derive_verified_direct_common_a_statement_v1(roster, bindings, context)?,
    })
}

impl DirectCommonACreatorH0ReadyV1 {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn begin_h0_v1(
        self,
    ) -> Result<DirectCommonACreatorH0ReplayV1, ZkAmsMkheErrorV1> {
        let expected_statement_digest = self.authority.statement_digest_for(self.context)?;
        Ok(DirectCommonACreatorH0ReplayV1 {
            context: self.context,
            expected_statement_digest,
            authority: self.authority,
            stream: DirectCommonAStatementStreamV1::begin(self.context)?,
        })
    }
}

impl DirectCommonACreatorH0ReplayV1 {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn derive_next_limb_into(
        &mut self,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.stream.derive_next_limb_into(output)
    }

    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn finish_h0_v1(
        self,
    ) -> Result<DirectCommonACreatorH1ReadyV1, ZkAmsMkheErrorV1> {
        let observed = self.stream.finish()?.statement_digest_for(self.context)?;
        if observed != self.expected_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(DirectCommonACreatorH1ReadyV1 {
            context: self.context,
            authority: self.authority,
        })
    }
}

impl DirectCommonACreatorH1ReadyV1 {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn begin_h1_v1(
        self,
    ) -> Result<DirectCommonACreatorH1ReplayV1, ZkAmsMkheErrorV1> {
        let expected_statement_digest = self.authority.statement_digest_for(self.context)?;
        Ok(DirectCommonACreatorH1ReplayV1 {
            context: self.context,
            expected_statement_digest,
            authority: self.authority,
            stream: DirectCommonAStatementStreamV1::begin(self.context)?,
        })
    }
}

impl DirectCommonACreatorH1ReplayV1 {
    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn derive_next_limb_into(
        &mut self,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.stream.derive_next_limb_into(output)
    }

    pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn finish_h1_v1(
        self,
    ) -> Result<CompletedDirectCommonACreatorAuthorityV1, ZkAmsMkheErrorV1> {
        let observed = self.stream.finish()?.statement_digest_for(self.context)?;
        if observed != self.expected_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(CompletedDirectCommonACreatorAuthorityV1 {
            context: self.context,
            authority: self.authority,
        })
    }
}

pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn consume_completed_creator_authority_v1(
    completed: CompletedDirectCommonACreatorAuthorityV1,
    prior_round_digest: [u8; 32],
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
) -> Result<PersistentDirectRelationUseSelectorV1, ZkAmsMkheErrorV1> {
    new_rkg_round_one_selector_v1(
        completed.context,
        prior_round_digest,
        contribution_statement_digest,
        proof_commitment_transcript_digest,
        completed.authority,
    )
}

#[cfg(test)]
#[path = "creator_replay_v1_tests.rs"]
mod tests;
