//! KAGEMUSHA-Fold A steps ([`crate::fold_lineage`]): circuits over `Fp` that
//! fold the preceding Q instance (committed on Pallas) on their verifier's own
//! lanes and run one A sub-step of an operation with today's task owners.
//!
//! An A step lays out the IVC frame (`iroha_fold::circuit::ivc::fold_frame`
//! on `VerifierChip::fold_lanes`), the operation's step context exactly as
//! today's split stages assign it, the sub-step's tasks through the operation's
//! stage plan, and the next A state. Folding replaces today's hard predecessor
//! verifier (the predecessor is the A state the frame authenticates), the hard
//! Q verifiers (a Q leaf's columns arrive in the Q instance the frame folds)
//! and the W continuations (sub-steps share the context digest through the A
//! state).

pub mod rebase;
pub mod receive;

use core::fmt;

use ff::PrimeField;
use iroha_pasta::{Ep, Eq, Fp, Fq};
use iroha_plonk::{
    VerifyingKey,
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Error as SynthesisError, Region, Value},
    transcript::TranscriptRepr,
};
use iroha_plonk_gadgets::{
    GlueChip, UintChip, Word,
    bytes::{
        element::LeElement,
        tape::{BytesChip, BytesConfig, SegmentSpec},
        variable::ActiveBytes,
    },
    imt::{LeafCells, OpeningCells, PathCells},
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldInput,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    verifier::{VerifierChip, VerifierConfig, VerifierKeyCells},
};

use super::{
    LINEAGE_FIELDS, LineagePublicCells, ProofMessageCells, VestaClaimCells, native::receive::INTERNAL_RANGE_BUSES,
};
use crate::{
    admin_sigma::StateWitness,
    fold_lineage::{A_EXPORT, A_FRAME},
    operation_relation::{map_effects::InsertCells, state::StateCells},
    tree::IndexedInsert,
};

/// A refused native preparation of a fold step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// An installed plan, key or schedule differs.
    Artifact,
    /// A source of the wrong shape, or one whose values are not canonical.
    Input,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KAGEMUSHA-Fold A step: {self:?}")
    }
}

impl std::error::Error for Error {}

/// Columns of an A step: today's internal-stage verifier (tagged four-bus
/// range layout), the byte tape lanes, the frame column and the export column
/// ([`A_EXPORT`]).
#[derive(Clone, Debug)]
pub struct AStepConfig {
    pub(crate) verifier: VerifierConfig<Ep>,
    pub(crate) bytes: BytesConfig,
    pub(crate) frame: Column<Instance>,
    pub(crate) export: Column<Instance>,
}

impl AStepConfig {
    /// Configures every A step the same way, so that all A relations share one
    /// fold shape.
    ///
    /// # Panics
    /// Never for the fixed bus count.
    pub fn configure(meta: &mut ConstraintSystem<Fp>) -> Self {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, INTERNAL_RANGE_BUSES)
            .expect("the internal tagged range layout is valid");
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let frame = meta.instance_column(A_FRAME);
        meta.enable_equality(frame);
        let export = meta.instance_column(A_EXPORT);
        meta.enable_equality(export);
        Self { verifier, bytes, frame, export }
    }
}

/// Witness assignment helpers shared by the A steps (key generation lays out
/// the same cells with unknown values).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Assign {
    pub(crate) known: bool,
}

impl Assign {
    pub(crate) fn value<T: Copy>(self, value: T) -> Value<T> {
        if self.known { Value::known(value) } else { Value::unknown() }
    }

    pub(crate) fn words<const N: usize>(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: &[Fp; N],
    ) -> Result<[Word<Fp>; N], SynthesisError> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| SynthesisError::Synthesis)
    }

    /// A foreign `Fq` scalar as range-checked canonical limbs.
    pub(crate) fn scalar(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Fq,
    ) -> Result<ScalarCells<Ep>, SynthesisError> {
        let [low, high] = foreign_limbs(&value);
        let low = chip.uint().assign::<128>(region, self.value(low))?;
        let high = chip.uint().assign::<127>(region, self.value(high))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &low, &high)
    }

    /// A full-k16 Pallas claim.
    pub(crate) fn pallas(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: &FoldInput<Ep>,
    ) -> Result<FoldInputCells<Ep>, SynthesisError> {
        let point = chip.witness_generator(region, self.value(Ep::from(*value.g())))?;
        let scalars = value
            .challenges()
            .iter()
            .map(|v| self.scalar(chip, region, *v))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| SynthesisError::Synthesis)?;
        FoldInputCells::from_normalized(chip, region, value.source_k(), point, scalars)
    }

    /// A full-k16 Vesta claim with canonical foreign coordinates.
    pub(crate) fn vesta(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: &AccumulatorT<Eq>,
    ) -> Result<VestaClaimCells, SynthesisError> {
        let (x, y) = iroha_plonk::transcript::generator_coordinates::<Eq>(value.g());
        let coordinates = [self.scalar(chip, region, x)?, self.scalar(chip, region, y)?];
        let challenges = self.words(chip, region, value.challenges())?;
        VestaClaimCells::constrain(chip, region, 16, coordinates, challenges)
    }

    /// A state opening and its lineage public fields.
    pub(crate) fn state(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        state: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), SynthesisError> {
        let core = self.words(chip, region, &state.core)?;
        let rest = self.words(chip, region, &state.rest)?;
        let cells = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = self.words(chip, region, &state.lineage)?;
        Ok((cells, LineagePublicCells::constrain(&mut chip.uint(), region, &public)?))
    }

    /// An original tape of at most `capacity` bytes, with its active length.
    pub(crate) fn active(
        self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        raw: &[u8],
        capacity: usize,
        segments: &[SegmentSpec],
    ) -> Result<ActiveBytes<Fp>, SynthesisError> {
        let values = if self.known { Value::known(raw.to_vec()) } else { Value::unknown() };
        ActiveBytes::assign(&mut chip.uint(), bytes, region, capacity, &values, segments)
    }

    /// The scheme's Omega key as verifier key cells (witnessed, as today's
    /// stages do; the owner verifying an Omega binds its digest to the
    /// lineage's).
    pub(crate) fn omega_key(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        key: &VerifyingKey<Ep>,
    ) -> Result<VerifierKeyCells<Ep>, SynthesisError> {
        let TranscriptRepr::Base(representation) = *key.transcript_repr() else {
            return Err(SynthesisError::Synthesis);
        };
        let points = |points: &[iroha_pasta::EpAffine]| points.iter().map(|p| self.value(Ep::from(*p))).collect::<Vec<_>>();
        chip.witness_key(
            region,
            self.value(representation),
            &points(key.fixed_commitments()),
            &points(key.permutation_commitments()),
        )
    }

    /// A proof's bytes as checked 256-bit messages and their length (as
    /// today's stages assign a predecessor proof).
    pub(crate) fn proof(
        self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        raw: &[u8],
    ) -> Result<ProofMessageCells, SynthesisError> {
        if !raw.len().is_multiple_of(32) {
            return Err(SynthesisError::Synthesis);
        }
        let length = u128::try_from(raw.len()).map_err(|_| SynthesisError::BoundsFailure)?;
        let length = chip.uint().assign::<32>(region, self.value(length))?;
        let messages = raw
            .as_chunks::<32>()
            .0
            .iter()
            .map(|chunk| LeElement::assign(&mut chip.uint(), region, self.value(*chunk)))
            .collect::<Result<Vec<_>, _>>()?;
        ProofMessageCells::from_messages(messages, length)
    }

    /// An authenticated insertion route (low leaf and slot paths).
    pub(crate) fn insertion(
        self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        witness: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, SynthesisError> {
        let leaf = witness.leaf;
        let values = uint
            .glue()
            .witnesses(region, &[leaf.key, leaf.value, leaf.next_key].map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| SynthesisError::Synthesis)?;
        let mut paths = Vec::with_capacity(2);
        for (index, siblings) in [(witness.leaf_slot, witness.leaf_siblings), (witness.slot, witness.slot_siblings)] {
            let index = uint.glue().witness(region, self.value(Fp::from(u64::from(index))))?;
            let siblings = uint
                .glue()
                .witnesses(region, &siblings.map(|v| self.value(v)))?
                .try_into()
                .map_err(|_| SynthesisError::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| SynthesisError::Synthesis)?;
        Ok(InsertCells { low: OpeningCells { leaf: LeafCells::from_words(values), path: low }, slot })
    }
}

/// Constrains a Pallas claim's cells to a fixed claim (its generator and
/// challenges are circuit constants).
pub(crate) fn pin_pallas(
    region: &mut Region<'_, Fp>,
    cells: &FoldInputCells<Ep>,
    value: &FoldInput<Ep>,
) -> Result<(), SynthesisError> {
    let (x, y) = iroha_plonk::transcript::generator_coordinates::<Ep>(value.g());
    GlueChip::assert_constant(region, cells.g().x(), x)?;
    GlueChip::assert_constant(region, cells.g().y(), y)?;
    for (cell, challenge) in cells.challenges().iter().zip(value.challenges()) {
        let [lo, hi] = foreign_limbs(challenge);
        GlueChip::assert_constant(region, cell.lo().word(), Fp::from_u128(lo))?;
        GlueChip::assert_constant(region, cell.hi().word(), Fp::from_u128(hi))?;
    }
    Ok(())
}

/// Constrains a Vesta claim's context words to a fixed claim.
pub(crate) fn pin_vesta(
    region: &mut Region<'_, Fp>,
    cells: &VestaClaimCells,
    value: &AccumulatorT<Eq>,
) -> Result<(), SynthesisError> {
    let expected = vesta_words(&value.as_input());
    let words = cells.words();
    if words.len() != expected.len() {
        return Err(SynthesisError::Synthesis);
    }
    for (word, value) in words.iter().zip(expected) {
        GlueChip::assert_constant(region, word, value)?;
    }
    Ok(())
}

/// The context words of a full-k16 Pallas claim, natively: `k`, the generator
/// coordinates, then each challenge's two limbs.
pub(crate) fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) -> Result<(), Error> {
    if claim.source_k() != 16 {
        return Err(Error::Input);
    }
    let (x, y) = iroha_plonk::transcript::generator_coordinates::<Ep>(claim.g());
    words.extend([Fp::from(16), x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    Ok(())
}

/// The context words of a Vesta claim, natively: both coordinates' limbs, then
/// the challenges.
pub(crate) fn vesta_words(claim: &FoldInput<Eq>) -> Vec<Fp> {
    let (x, y) = iroha_plonk::transcript::generator_coordinates::<Eq>(claim.g());
    let mut words = Vec::with_capacity(20);
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    words.extend(claim.challenges());
    words
}

/// The A state's cells as the frame authenticated them.
#[derive(Clone, Debug)]
pub(crate) struct AStateCells {
    pub(crate) lineage: [Word<Fp>; LINEAGE_FIELDS],
    pub(crate) context: Word<Fp>,
    pub(crate) pc: Word<Fp>,
}

impl AStateCells {
    pub(crate) fn from_frame(state: &[Word<Fp>]) -> Result<Self, SynthesisError> {
        use crate::fold_lineage::{A_CONTEXT, A_PC, A_STATE};
        if state.len() != A_STATE {
            return Err(SynthesisError::Synthesis);
        }
        Ok(Self {
            lineage: state[..LINEAGE_FIELDS].to_vec().try_into().map_err(|_| SynthesisError::Synthesis)?,
            context: state[A_CONTEXT].clone(),
            pc: state[A_PC].clone(),
        })
    }
}
