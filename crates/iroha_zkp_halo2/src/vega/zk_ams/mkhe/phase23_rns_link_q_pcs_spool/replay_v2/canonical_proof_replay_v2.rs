//! One-shot, purpose-bound replay capabilities for the canonical qPCS proof.
use core::array;
use crate::vega::zk_ams::mkhe::phase23_rns_link::q_pcs::v2_soundness::{
    CanonicalProofSectionV2, CanonicalProofTreeKindV2, ProverCanonicalProofPlanV2,
};
use super::prover_v2::ProverPrerequisiteErrorV2;
use super::*;
const CANONICAL_REPLAY_PURPOSE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.qpcs.canonical-proof-replay-purpose\0";
const CANONICAL_TREE_PURPOSE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.qpcs.canonical-proof-replay-purpose.tree-purpose\0";
const CANONICAL_TREE_COUNT_V2: usize = 20;
const CANONICAL_FRI_ROOTS_V2: usize = 18;
#[derive(Clone, Copy)]
pub(super) struct CanonicalProofReplayBindingV2 {
    pub(super) parameter_digest: [u8; 32],
    pub(super) context: PublicSpoolContextV2,
    pub(super) initial_root: [u8; 32],
    pub(super) quotient_root: [u8; 32],
    pub(super) fri_roots: [[u8; 32]; CANONICAL_FRI_ROOTS_V2],
    pub(super) terminal_digest: [u8; 32],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CanonicalTreeReplayShapeV2 {
    pub(super) length: u32,
    pub(super) columns: u16,
    pub(super) values_per_block: u16,
}
/// A replay can only advance by its internally selected next logical column.
pub(super) trait CanonicalTreeReplayV2 {
    type Owner;
    fn shape_v2(&self) -> Result<CanonicalTreeReplayShapeV2, ProverPrerequisiteErrorV2>;
    fn read_next_column_v2(
        &mut self,
    ) -> Result<AuthenticatedReplayChunkV2, ProverPrerequisiteErrorV2>;
    fn complete_v2(self) -> Result<Self::Owner, ProverPrerequisiteErrorV2>;
}
pub(super) struct CanonicalTreeReplayPurposeV2 {
    master_binding: [u8; 32],
    section: CanonicalProofSectionV2,
    expected_root: [u8; 32],
    purpose_digest: [u8; 32],
}
pub(super) struct CanonicalTreePurposeBoundV2 {
    ordinal: u8,
    purpose_digest: [u8; 32],
}
impl CanonicalTreePurposeBoundV2 {
    pub(super) fn complete_v2(self, ordinal: u8) -> Result<(), ProverPrerequisiteErrorV2> {
        if self.ordinal != ordinal || self.purpose_digest == [0; 32] {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        Ok(())
    }
}
pub(super) struct CanonicalProofReplayPurposesV2 {
    plan: Option<ProverCanonicalProofPlanV2>,
    master_binding: [u8; 32],
    purposes: [Option<CanonicalTreeReplayPurposeV2>; CANONICAL_TREE_COUNT_V2],
    next_ordinal: u8,
}
pub(super) struct CanonicalProofReplayCompleteV2 {
    binding_digest: [u8; 32],
}
impl CanonicalProofReplayCompleteV2 {
    pub(super) const fn binding_digest_v2(&self) -> [u8; 32] {
        self.binding_digest
    }
}
fn expected_root_v2(
    binding: &CanonicalProofReplayBindingV2,
    section: CanonicalProofSectionV2,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    match section.kind_v2() {
        CanonicalProofTreeKindV2::Initial => Ok(binding.initial_root),
        CanonicalProofTreeKindV2::OpeningQuotient => Ok(binding.quotient_root),
        CanonicalProofTreeKindV2::Fri => binding
            .fri_roots
            .get(usize::from(section.merkle_layer_v2()))
            .copied()
            .ok_or(ProverPrerequisiteErrorV2::InvalidCanonicalProof),
    }
}
fn master_binding_digest_v2(
    binding: &CanonicalProofReplayBindingV2,
    transcript: [u8; 32],
    batch_schedule_digest: [u8; 32],
    fold_schedule_digest: [u8; 32],
    query_digest: [u8; 32],
    section_shape_digest: [u8; 32],
    exact_wire_bytes: usize,
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    binding.context.validate_v2()?;
    if binding.parameter_digest == [0; 32]
        || binding.initial_root == [0; 32]
        || binding.quotient_root == [0; 32]
        || binding.fri_roots.iter().any(|root| *root == [0; 32])
        || binding.terminal_digest == [0; 32]
        || transcript == [0; 32]
        || batch_schedule_digest == [0; 32]
        || fold_schedule_digest == [0; 32]
        || query_digest == [0; 32]
        || section_shape_digest == [0; 32]
    {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_REPLAY_PURPOSE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&binding.parameter_digest);
    hash.update(&binding.context.sealed_source_transcript_digest);
    hash.update(&binding.context.source_algebra_binding_digest);
    hash.update(&binding.initial_root);
    hash.update(&binding.quotient_root);
    for root in binding.fri_roots {
        hash.update(&root);
    }
    hash.update(&transcript);
    hash.update(&batch_schedule_digest);
    hash.update(&fold_schedule_digest);
    hash.update(&binding.terminal_digest);
    hash.update(&query_digest);
    hash.update(&section_shape_digest);
    hash.update(
        &u64::try_from(exact_wire_bytes)
            .map_err(|_| ProverPrerequisiteErrorV2::ArithmeticOverflow)?
            .to_be_bytes(),
    );
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    Ok(digest)
}
fn tree_purpose_digest_v2(
    master_binding: [u8; 32],
    section: CanonicalProofSectionV2,
    expected_root: [u8; 32],
) -> Result<[u8; 32], ProverPrerequisiteErrorV2> {
    if master_binding == [0; 32] || expected_root == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_TREE_PURPOSE_DOMAIN_V2);
    hash.update(&[Q_PCS_SPOOL_VERSION_V2]);
    hash.update(&master_binding);
    hash.update(&[
        section.ordinal_v2(),
        section.kind_v2() as u8,
        section.layer_v2(),
    ]);
    hash.update(&section.length_v2().to_be_bytes());
    hash.update(&expected_root);
    hash.update(&section.opened_v2().to_be_bytes());
    hash.update(&section.authentication_v2().to_be_bytes());
    let digest = hash.finalize();
    if digest == [0; 32] {
        return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
    }
    Ok(digest)
}
impl CanonicalTreeReplayPurposeV2 {
    pub(super) fn bind_v2(
        self,
        master_binding: [u8; 32],
        section: CanonicalProofSectionV2,
        expected_root: [u8; 32],
    ) -> Result<CanonicalTreePurposeBoundV2, ProverPrerequisiteErrorV2> {
        let expected_digest = tree_purpose_digest_v2(master_binding, section, expected_root)?;
        if self.master_binding != master_binding
            || self.section != section
            || self.expected_root != expected_root
            || self.purpose_digest != expected_digest
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        Ok(CanonicalTreePurposeBoundV2 {
            ordinal: section.ordinal_v2(),
            purpose_digest: self.purpose_digest,
        })
    }
}
impl CanonicalProofReplayPurposesV2 {
    pub(super) fn bind_v2(
        plan: ProverCanonicalProofPlanV2,
        binding: CanonicalProofReplayBindingV2,
    ) -> Result<Self, ProverPrerequisiteErrorV2> {
        let (transcript, batch_schedule, fold_schedule) = plan.transcript_context_v2();
        let master_binding = master_binding_digest_v2(
            &binding,
            transcript,
            batch_schedule,
            fold_schedule,
            plan.query_digest_v2(),
            plan.section_shape_digest_v2(),
            plan.exact_wire_bytes_v2(),
        )?;
        let mut purposes: [Option<CanonicalTreeReplayPurposeV2>; CANONICAL_TREE_COUNT_V2] =
            array::from_fn(|_| None);
        for (ordinal, destination) in purposes.iter_mut().enumerate() {
            let section = plan.section_v2(ordinal)?;
            let expected_root = expected_root_v2(&binding, section)?;
            *destination = Some(CanonicalTreeReplayPurposeV2 {
                master_binding,
                section,
                expected_root,
                purpose_digest: tree_purpose_digest_v2(master_binding, section, expected_root)?,
            });
        }
        Ok(Self {
            plan: Some(plan),
            master_binding,
            purposes,
            next_ordinal: 0,
        })
    }
    pub(super) fn plan_v2(&self) -> Result<&ProverCanonicalProofPlanV2, ProverPrerequisiteErrorV2> {
        self.plan
            .as_ref()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)
    }
    pub(super) const fn master_binding_v2(&self) -> [u8; 32] {
        self.master_binding
    }
    pub(super) fn take_next_purpose_v2(
        &mut self,
        ordinal: u8,
    ) -> Result<CanonicalTreeReplayPurposeV2, ProverPrerequisiteErrorV2> {
        if ordinal != self.next_ordinal || usize::from(ordinal) >= CANONICAL_TREE_COUNT_V2 {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let purpose = self.purposes[usize::from(ordinal)]
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        self.next_ordinal += 1;
        Ok(purpose)
    }
    pub(super) fn complete_v2(
        mut self,
    ) -> Result<CanonicalProofReplayCompleteV2, ProverPrerequisiteErrorV2> {
        if usize::from(self.next_ordinal) != CANONICAL_TREE_COUNT_V2
            || self.purposes.iter().any(Option::is_some)
        {
            return Err(ProverPrerequisiteErrorV2::InvalidCanonicalProof);
        }
        let _plan = self
            .plan
            .take()
            .ok_or(ProverPrerequisiteErrorV2::Poisoned)?;
        Ok(CanonicalProofReplayCompleteV2 {
            binding_digest: self.master_binding,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const SHAPE_DIGEST_KAT_V2: [u8; 32] = [
        0x03, 0xb8, 0x27, 0x20, 0x89, 0x43, 0xc7, 0x25, 0xf2, 0x02, 0x34, 0x24, 0x09, 0x0c, 0x5a,
        0x1a, 0x9a, 0x1a, 0xd1, 0x75, 0x20, 0x76, 0x43, 0x87, 0xd2, 0x90, 0xf9, 0xc4, 0x91, 0xa1,
        0xe1, 0x5d,
    ];
    const MASTER_KAT_V2: [u8; 32] = [
        0x8a, 0xbf, 0x77, 0x82, 0x04, 0x62, 0x6c, 0x07, 0xc3, 0xd0, 0xd0, 0x2b, 0xb2, 0xfe, 0x5a,
        0xd7, 0x80, 0xd4, 0xc1, 0xd2, 0xcf, 0xa8, 0xe0, 0xc3, 0xfc, 0x52, 0x95, 0x1f, 0x93, 0x90,
        0xa8, 0xd1,
    ];
    const C0_PERMIT_KAT_V2: [u8; 32] = [
        0xca, 0xe6, 0xa4, 0x4c, 0x4a, 0x67, 0x98, 0x82, 0x78, 0x0f, 0xed, 0x05, 0x0f, 0x6c, 0x8f,
        0x0f, 0xe2, 0x44, 0x55, 0x07, 0x23, 0x45, 0x6c, 0x1c, 0x39, 0xd9, 0x57, 0xeb, 0xc2, 0x08,
        0xbf, 0x6e,
    ];
    fn synthetic_binding_v2() -> CanonicalProofReplayBindingV2 {
        CanonicalProofReplayBindingV2 {
            parameter_digest: [0x11; 32],
            context: PublicSpoolContextV2 {
                sealed_source_transcript_digest: [0x12; 32],
                source_algebra_binding_digest: [0x13; 32],
            },
            initial_root: [0x14; 32],
            quotient_root: [0x15; 32],
            fri_roots: array::from_fn(|index| [0x20 + index as u8; 32]),
            terminal_digest: [0x43; 32],
        }
    }
    #[test]
    fn transparent_master_and_c0_permit_frames_are_frozen() {
        let binding = synthetic_binding_v2();
        let master = master_binding_digest_v2(
            &binding,
            [0x40; 32],
            [0x41; 32],
            [0x42; 32],
            [0x44; 32],
            SHAPE_DIGEST_KAT_V2,
            27_196_704,
        )
        .unwrap();
        assert_eq!(master, MASTER_KAT_V2);
        let section = CanonicalProofSectionV2::test_only_v2(0, 1, 0xff, 524_288, 320, 3_096);
        assert_eq!(
            tree_purpose_digest_v2(master, section, binding.initial_root).unwrap(),
            C0_PERMIT_KAT_V2
        );
    }
    #[test]
    fn wrong_root_master_or_shape_cannot_rebind_a_one_shot_permit() {
        let section = CanonicalProofSectionV2::test_only_v2(0, 1, 0xff, 524_288, 320, 3_096);
        let permit = || CanonicalTreeReplayPurposeV2 {
            master_binding: MASTER_KAT_V2,
            section,
            expected_root: [0x14; 32],
            purpose_digest: C0_PERMIT_KAT_V2,
        };
        let mut slot = Some(permit());
        let purpose = slot.take().unwrap();
        assert!(purpose.bind_v2(MASTER_KAT_V2, section, [0x99; 32]).is_err());
        assert!(slot.take().is_none());
        assert!(permit().bind_v2([0x98; 32], section, [0x14; 32]).is_err());
        let wrong_shape = CanonicalProofSectionV2::test_only_v2(0, 1, 0xff, 524_288, 318, 3_096);
        assert!(
            permit()
                .bind_v2(MASTER_KAT_V2, wrong_shape, [0x14; 32])
                .is_err()
        );
    }
}
