//! KAGEMUSHA-Fold: the folded lineage program (output `IVC.md` and
//! `RECEIVE-STEPS.md` of the folding lane).
//!
//! A segment of a lineage is one two-curve IVC chain (`iroha_fold::ivc`): Q
//! steps are circuits over `Fq` that fold the preceding A instance (committed
//! on Vesta), A steps are circuits over `Fp` that fold the preceding Q
//! instance (committed on Pallas). An operation is a fixed run of half-steps.
//! A Receive runs today's Receive relation, with its Q leaves folded instead of
//! proved and its A stages chained by the IVC state instead of W proofs:
//!
//! | half-step | side | kind | folds | work |
//! |---|---|---|---|---|
//! | 1 | Q | [`QKind::Frame`] | the previous operation's last A | frame only |
//! | 2 | A | [`AKind::Receive1`] | `Frame` | stage A1: consumed and credit effects, transition |
//! | 3 | Q | [`QKind::Sigma`] | `Receive1` | today's Q0: the `σ_send` soft verifier |
//! | 4 | A | [`AKind::Receive3`] | `Sigma` | stage A3: proofs, objects; binds Q0's columns |
//! | 5 | Q | [`QKind::Signature`] | `Receive3` | today's Q1: the merged signature leaf |
//! | 6 | A | [`AKind::Receive4`] | `Signature` | stage A4: signatures, authorization; binds Q1's columns |
//! | 7 | Q | [`QKind::Frame`] | `Receive4` | frame only |
//! | 8 | A | [`AKind::Receive5`] | `Frame` | stage A5: nonmembership, effects, proof digest |
//!
//! Today's stage A2 only verifies Q0; the folded A3 binds Q0's columns where
//! its owners read them, so A2 has no folded counterpart.
//!
//! Every relation kind is fixed per circuit, and so are the kinds it may fold
//! (`ChainSide::choices`): the frame-only Q step follows A4 inside a Receive and
//! the last A of the previous operation, so its witness chooses that slot. A
//! side keeps one accumulator per kind of the other side, then its own claim
//! accumulator ([`A_DIGESTS`], [`Q_DIGESTS`]). All relations of a side fold
//! under one domain ([`FOLD_A`], [`FOLD_Q`]): kinds are kept apart by their
//! slots, because each slot's accumulator is decided against its own relation,
//! so an instance folded into another kind's slot leaves that slot undecidable.
//!
//! The A state ([`A_STATE`] words) is the lineage public fields (as `D_A`
//! hashes them), the digest of the current operation's step context (zero
//! between operations) and the program counter (the number of the operation's
//! A sub-steps done, zero between operations). The Q state is empty: Q steps
//! export their leaves' public columns, which the following A step reads from
//! the instance it folds.
//!
//! The incoming Omega's deferred IPA claims enter the claim accumulators of
//! the curves they live on: A5 folds its two Pallas claims (the transported
//! `acc_P` and the proof opening) into A's, and the Q Sigma step folds the
//! `σ_send` opening and the transported `acc_V` into Q's. A1 hands `acc_V`,
//! its mode and its correction to the Q side in its export column
//! ([`A_EXPORT`]), as today's A frame hands them to Omega.

pub mod sigma;
pub mod signature;

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldLanes, FoldVerifierChips, FoldVerifierConfig,
        claims::{
            ClaimCells, assign_claim_accumulator, assign_claim_proof, claim_accumulator_digest, verify_claim_fold,
            verify_claim_fold_or_start,
        },
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    claims::{Claim, ClaimAccumulator, ClaimProof, fold_claim, fold_or_start_claim, placeholder_claim_accumulator},
    ivc::{ChainSide, FrameWitness, embed, ivc_hash},
    relation::FoldShape,
};
use iroha_pasta::{Eq, Fp, Fq, PastaCurve};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{GlueChip, Word};
use iroha_plonk_recursion::{FoldInput, accumulation_circuit::FoldInputCells};

use crate::a_relation::LINEAGE_FIELDS;

/// IVC-hash domain of the A side.
pub const IVC_A: [u8; 32] = *b"kagemusha-fold lineage A side v1";
/// IVC-hash domain of the Q side.
pub const IVC_Q: [u8; 32] = *b"kagemusha-fold lineage Q side v1";
/// Fold domain of every A relation (their instances' and accumulators'
/// transcripts, and the deciders of `U_A`).
pub const FOLD_A: [u8; 32] = *b"kagemusha-fold Receive A rel v1.";
/// Fold domain of every Q relation.
pub const FOLD_Q: [u8; 32] = *b"kagemusha-fold Receive Q rel v1.";
/// Claim-accumulator domain of the A side (Pallas claims).
pub const CLAIM_A: [u8; 32] = *b"kagemusha-fold A claim acc v1...";
/// Claim-accumulator domain of the Q side (Vesta claims).
pub const CLAIM_Q: [u8; 32] = *b"kagemusha-fold Q claim acc v1...";
/// The segment base word every IVC hash carries.
pub const BASE: u64 = u64::from_le_bytes(*b"kgwfseg1");

/// Words of the A state: the lineage public fields, the step-context digest,
/// the program counter.
pub const A_STATE: usize = LINEAGE_FIELDS + 2;
/// Position of the step-context digest in the A state.
pub const A_CONTEXT: usize = LINEAGE_FIELDS;
/// Position of the program counter in the A state.
pub const A_PC: usize = LINEAGE_FIELDS + 1;
/// Words of the Q state.
pub const Q_STATE: usize = 0;
/// Length of an A instance's frame column (`[passthrough, h_out]`).
pub const A_FRAME: usize = 2;
/// Length of a Q instance's frame column (`[passthrough, h_out]`); a Q leaf's
/// public columns follow as further instance columns.
pub const Q_FRAME: usize = 2;
/// Length of an A instance's export column, its second instance column:
/// today's A-frame incoming Vesta words (the transported `acc_V`'s coordinate
/// limbs and challenges, its one-hot mode, the correction's coordinate limbs).
/// A1 fills it; every other A kind exports zeros.
pub const A_EXPORT: usize = 27;

/// A relations of the folded lineage, in the order of Q's digest list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AKind {
    /// Receive stage A1 (consumed and credit effects, transition).
    Receive1,
    /// Receive stage A3 (proofs, objects).
    Receive3,
    /// Receive stage A4 (signatures, authorization).
    Receive4,
    /// Receive stage A5 (nonmembership, effects, proof digest).
    Receive5,
}

/// Q relations of the folded lineage, in the order of A's digest list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QKind {
    /// Frame only: opens an operation, and precedes a Receive's last A
    /// sub-step.
    Frame,
    /// The `σ_send` soft verifier (today's Q0).
    Sigma,
    /// The merged signature leaf (today's Q1).
    Signature,
}

/// A's digest list: one accumulator per Q kind, then its claim accumulator.
pub const A_DIGESTS: usize = 3 + 1;
/// Q's digest list: one accumulator per A kind, then its claim accumulator.
pub const Q_DIGESTS: usize = 4 + 1;
/// Position of A's claim-accumulator digest in its list.
pub const A_CLAIM_SLOT: usize = 3;
/// Position of Q's claim-accumulator digest in its list.
pub const Q_CLAIM_SLOT: usize = 4;

impl AKind {
    /// Every A kind, in digest order.
    pub const ALL: [Self; 4] = [Self::Receive1, Self::Receive3, Self::Receive4, Self::Receive5];

    /// The Q kind a step of this kind folds.
    #[must_use]
    pub const fn folds(self) -> QKind {
        match self {
            Self::Receive1 | Self::Receive5 => QKind::Frame,
            Self::Receive3 => QKind::Sigma,
            Self::Receive4 => QKind::Signature,
        }
    }

    /// The frame of a step of this kind.
    #[must_use]
    pub const fn chain(self) -> ChainSide {
        let slot = self.folds() as usize;
        ChainSide { ivc_domain: IVC_A, fold_domain: FOLD_Q, first_step: 2, slot, digests: A_DIGESTS, choices: 1 << slot }
    }
}

impl QKind {
    /// Every Q kind, in digest order.
    pub const ALL: [Self; 3] = [Self::Frame, Self::Sigma, Self::Signature];

    /// The A kinds a step of this kind may fold, as a slot bit set: the frame
    /// step follows A4 inside a Receive, or the previous operation's last A
    /// (A5 of a Receive).
    #[must_use]
    pub const fn choices(self) -> u64 {
        match self {
            Self::Frame => (1 << AKind::Receive4 as usize) | (1 << AKind::Receive5 as usize),
            Self::Sigma => 1 << AKind::Receive1 as usize,
            Self::Signature => 1 << AKind::Receive3 as usize,
        }
    }

    /// Whether a step of this kind may fold an instance of `kind`.
    #[must_use]
    pub const fn folds(self, kind: AKind) -> bool {
        (self.choices() >> kind as usize) & 1 == 1
    }

    /// The frame of a step of this kind folding an instance of `kind` (the
    /// circuit lays out the same frame for every kind it may fold).
    #[must_use]
    pub const fn chain(self, kind: AKind) -> ChainSide {
        ChainSide {
            ivc_domain: IVC_Q,
            fold_domain: FOLD_A,
            first_step: 1,
            slot: kind as usize,
            digests: Q_DIGESTS,
            choices: self.choices(),
        }
    }
}

/// The passthrough of a segment's first instance (a Q step without
/// predecessor): A's base hash `H_A(0, base, a_base, [0, ..])`, a circuit
/// constant of the Q steps that can open a segment. `a_base` is the A state
/// the segment starts from.
#[must_use]
pub fn start_passthrough(a_base: &[Fp]) -> Fq {
    embed(&ivc_hash::<Fp>(&IVC_A, 0, Fp::from(BASE), a_base, &[Fp::ZERO; A_DIGESTS]))
}

/// The claim a selected incoming opening defers: its generator and its sixteen
/// challenges (zero-padded to k16).
#[must_use]
pub fn claim_of<C: PastaCurve>(input: &FoldInput<C>) -> Claim<C> {
    Claim { g: *input.g(), u: *input.challenges() }
}

/// The claim folds of a step, natively: the claim accumulator its side keeps
/// before the step (`None` while absent) and each fold's messages, in order
/// (zero for the fold that starts the accumulator).
#[derive(Clone, Debug)]
pub struct ClaimFolds<C: PastaCurve> {
    /// The claim accumulator before the step.
    pub kept: Option<ClaimAccumulator<C>>,
    /// One message set per claim.
    pub proofs: Vec<ClaimProof<C::ScalarExt>>,
}

/// Folds `claims`, in order, into the claim accumulator a side keeps, starting
/// it from the first claim while absent. Returns the new accumulator, its
/// witness and the step's [`ClaimFolds`].
///
/// # Errors
/// [`iroha_fold::Error::Shape`] without claims; as [`fold_claim`].
pub fn fold_claims<C: PastaCurve>(
    domain: &[u8; 32],
    kept: Option<(&ClaimAccumulator<C>, &[C::ScalarExt])>,
    claims: &[Claim<C>],
) -> Result<(ClaimAccumulator<C>, Vec<C::ScalarExt>, ClaimFolds<C>), iroha_fold::Error> {
    let (first, rest) = claims.split_first().ok_or(iroha_fold::Error::Shape("claim folds"))?;
    let before = kept.map(|(acc, _)| *acc);
    let (mut acc, mut s, proof) = fold_or_start_claim(domain, kept, first)?;
    let mut proofs = vec![proof];
    for claim in rest {
        let (next, next_s, proof) = fold_claim(domain, &acc, &s, claim)?;
        (acc, s) = (next, next_s);
        proofs.push(proof);
    }
    Ok((acc, s, ClaimFolds { kept: before, proofs }))
}

/// The claim cells of a selected claim on the fold lanes: a point and sixteen
/// scalars copy-bound to the selection's generator coordinates and challenge
/// limbs.
pub(crate) fn claim_cells<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    selected: &FoldInputCells<C>,
    value: Option<&FoldInput<C>>,
) -> Result<ClaimCells<C>, Error> {
    let point = value.map_or_else(Value::unknown, |v| Value::known(C::from(*v.g())));
    let g = lanes.assign_point(region, point)?;
    GlueChip::assert_equal(region, g.x(), selected.g().x())?;
    GlueChip::assert_equal(region, g.y(), selected.g().y())?;
    let mut u = Vec::with_capacity(selected.challenges().len());
    for (i, cell) in selected.challenges().iter().enumerate() {
        let scalar = lanes.assign_scalar(region, value.map_or_else(Value::unknown, |v| Value::known(v.challenges()[i])))?;
        GlueChip::assert_equal(region, scalar.s6.lo().word(), cell.lo().word())?;
        GlueChip::assert_equal(region, scalar.s6.hi().word(), cell.hi().word())?;
        u.push(scalar);
    }
    Ok(ClaimCells { g, u })
}

/// Folds `claims` into the claim accumulator whose digest `digests[slot]`
/// holds: opens it unless absent (digest zero), folds the first claim into it
/// or starts it from that claim, folds the rest, and returns the digest list
/// with the new accumulator's digest. Nothing is accepted here: the claim
/// decider accepts the accumulator or not.
pub(crate) fn fold_claims_cells<C: PastaCurve>(
    lanes: &mut FoldLanes<'_, C>,
    region: &mut Region<'_, C::Base>,
    (domain, slot): (&[u8; 32], usize),
    digests: &[Word<C::Base>],
    claims: &[ClaimCells<C>],
    witness: Option<&ClaimFolds<C>>,
) -> Result<Vec<Word<C::Base>>, Error> {
    if claims.is_empty() || slot >= digests.len() || witness.is_some_and(|w| w.proofs.len() != claims.len()) {
        return Err(Error::Synthesis);
    }
    let placeholder = placeholder_claim_accumulator::<C>();
    let acc = assign_claim_accumulator(lanes, region, witness.map(|w| w.kept.as_ref().unwrap_or(&placeholder)))?;
    let stored = digests[slot].clone();
    let absent = lanes.glue.is_zero(region, &stored)?;
    let opened = claim_accumulator_digest(lanes, region, domain, &acc)?;
    assert_equal_unless(lanes, region, &opened, &stored, Some(&absent))?;
    let mut folded: Option<_> = None;
    for (i, claim) in claims.iter().enumerate() {
        let proof = assign_claim_proof(lanes, region, witness.map(|w| &w.proofs[i]))?;
        folded = Some(match &folded {
            None => verify_claim_fold_or_start(lanes, region, domain, &acc, claim, &proof, &absent)?,
            Some(acc) => verify_claim_fold(lanes, region, domain, acc, claim, &proof)?,
        });
    }
    let folded = folded.ok_or(Error::Synthesis)?;
    let mut out = digests.to_vec();
    out[slot] = claim_accumulator_digest(lanes, region, domain, &folded)?;
    Ok(out)
}

/// The Q step without business ([`QKind::Frame`]): it folds the preceding A
/// instance (A4 or the previous operation's last A, as its witness chooses)
/// and passes its digests through.
#[derive(Clone, Debug)]
pub struct QFrameStep {
    /// The shape of the A relations it folds.
    pub shape: FoldShape,
    /// The passthrough of a segment's first instance ([`start_passthrough`]).
    pub start: Fq,
    /// The frame witness (`None` at key generation).
    pub witness: Option<FrameWitness<Eq>>,
}

/// Columns of a [`QFrameStep`].
#[derive(Clone, Debug)]
pub struct QFrameConfig {
    fold: FoldVerifierConfig<Eq>,
    frame: Column<Instance>,
}

impl Circuit<Fq> for QFrameStep {
    type Config = QFrameConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { witness: None, ..self.clone() }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> QFrameConfig {
        let fold = FoldVerifierConfig::configure(meta);
        let frame = meta.instance_column(Q_FRAME);
        meta.enable_equality(frame);
        QFrameConfig { fold, frame }
    }

    fn synthesize(&self, config: QFrameConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let chain = QKind::Frame.chain(AKind::Receive5);
        let outputs = layouter.assign_region(
            || "folded lineage Q frame step",
            |mut region| {
                let mut lanes = chips.lanes();
                let frame = fold_frame(
                    &mut lanes,
                    &mut region,
                    &chain,
                    &self.shape,
                    Fq::from(BASE),
                    self.start,
                    Q_STATE,
                    self.witness.as_ref(),
                )?;
                frame_outputs(&mut lanes, &mut region, &chain, &frame, &[], &frame.digests, &[])
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_folds_the_kind_before_it_in_its_own_slot() {
        for kind in AKind::ALL {
            let chain = kind.chain();
            assert!(chain.is_valid() && !chain.is_selected() && chain.slot < A_CLAIM_SLOT);
            assert_eq!(QKind::ALL[chain.slot], kind.folds());
        }
        for q in QKind::ALL {
            for kind in AKind::ALL.into_iter().filter(|kind| q.folds(*kind)) {
                let chain = q.chain(kind);
                assert!(chain.is_valid() && chain.slot < Q_CLAIM_SLOT);
                assert_eq!(AKind::ALL[chain.slot], kind);
                assert_eq!(chain.is_selected(), q == QKind::Frame);
            }
        }
        // The Receive's half-steps alternate Frame, A1, Sigma, A3, Signature, A4,
        // Frame, A5, and the next operation opens with a Frame folding A5.
        let schedule = [
            (QKind::Frame, AKind::Receive1),
            (QKind::Sigma, AKind::Receive3),
            (QKind::Signature, AKind::Receive4),
            (QKind::Frame, AKind::Receive5),
        ];
        let mut previous = AKind::Receive5;
        for (q, a) in schedule {
            assert!(q.folds(previous), "{q:?} folds {previous:?}");
            assert_eq!(a.folds(), q);
            previous = a;
        }
        assert!(QKind::Frame.folds(previous));
        let mut domains = vec![IVC_A, IVC_Q, CLAIM_A, CLAIM_Q, FOLD_A, FOLD_Q];
        let count = domains.len();
        domains.sort_unstable();
        domains.dedup();
        assert_eq!(domains.len(), count, "every domain is distinct");
    }
}
