//! Private operation selection for the one ordinary CommitVote BLS program.
use iroha_plonk_gadgets::bls12_381::{
    curve::{
        g1_program::G1_SUBGROUP_STEPS,
        programs::{G2_COFACTOR_STEPS, G2_SUBGROUP_STEPS},
    },
    pairing::{final_exponent::FINAL_EXPONENT_STEPS, miller_program::MILLER_STEPS},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    Start,
    G1(usize),
    StartSignature,
    G2(usize),
    HashFields,
    Swu0,
    Iso0,
    Swu1,
    Iso1,
    StartCofactor,
    Cofactor(usize),
    StartMiller,
    Miller(usize),
    StartFinal,
    Final(usize),
    Finish,
}
/// Exact fixed source program. The private step is derived only from its
/// canonical ordinal; a witness cannot select an arbitrary operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlsLeafPlan {
    cursor: u32,
    step: Step,
}
impl BlsLeafPlan {
    /// Number of real leaves, excluding any outer recursion-tree padding.
    pub const LENGTH: u32 = 1084;
    /// Ordinary Sumeragi CommitVote preimage length.
    pub const MESSAGE_BYTES: usize = 165;
    /// Fixed first-release program identity.
    pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwblsp1");
    /// Select a compiled leaf by its exact program cursor.
    /// Out-of-program indices have no plan or admissible source key.
    pub fn at(cursor: u32) -> Option<Self> {
        let segments: &[(usize, fn(usize) -> Step)] = &[
            (1, |_| Step::Start),
            (G1_SUBGROUP_STEPS.len(), Step::G1),
            (1, |_| Step::StartSignature),
            (G2_SUBGROUP_STEPS.len(), Step::G2),
            (1, |_| Step::HashFields),
            (1, |_| Step::Swu0),
            (1, |_| Step::Iso0),
            (1, |_| Step::Swu1),
            (1, |_| Step::Iso1),
            (1, |_| Step::StartCofactor),
            (G2_COFACTOR_STEPS.len(), Step::Cofactor),
            (1, |_| Step::StartMiller),
            (MILLER_STEPS.len(), Step::Miller),
            (1, |_| Step::StartFinal),
            (FINAL_EXPONENT_STEPS.len(), Step::Final),
            (1, |_| Step::Finish),
        ];
        let mut index = usize::try_from(cursor).ok()?;
        let mut selected = None;
        for &(length, select) in segments {
            if index < length {
                selected = Some(select(index));
                break;
            }
            index -= length;
        }
        let step = selected?;
        Some(Self { cursor, step })
    }
    /// Exact start cursor. Every leaf advances by one.
    pub const fn cursor(self) -> u32 {
        self.cursor
    }
    pub(super) const fn step(self) -> Step {
        self.step
    }
    pub(super) const fn needs_sha(self) -> bool {
        matches!(self.step, Step::HashFields)
    }
    pub(super) const fn before_tag(self) -> u64 {
        match self.step {
            Step::Start => 0,
            Step::G1(_) | Step::StartSignature => 1,
            Step::G2(_) | Step::HashFields => 2,
            Step::Swu0 => 3,
            Step::Iso0 => 4,
            Step::Swu1 => 5,
            Step::Iso1 => 6,
            Step::StartCofactor => 7,
            Step::Cofactor(_) | Step::StartMiller => 8,
            Step::Miller(_) | Step::StartFinal => 9,
            Step::Final(_) | Step::Finish => 10,
        }
    }
    pub(super) const fn after_tag(self) -> u64 {
        match self.step {
            Step::Start | Step::G1(_) => 1,
            Step::StartSignature | Step::G2(_) => 2,
            Step::HashFields => 3,
            Step::Swu0 => 4,
            Step::Iso0 => 5,
            Step::Swu1 => 6,
            Step::Iso1 => 7,
            Step::StartCofactor | Step::Cofactor(_) => 8,
            Step::StartMiller | Step::Miller(_) => 9,
            Step::StartFinal | Step::Final(_) => 10,
            Step::Finish => 11,
        }
    }
}

impl Default for BlsLeafPlan {
    fn default() -> Self {
        Self {
            cursor: 0,
            step: Step::Start,
        }
    }
}
