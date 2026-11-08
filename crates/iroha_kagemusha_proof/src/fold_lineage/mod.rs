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
//! Every segment starts from one constant A state ([`segment_base`]), so the
//! start passthrough every Q step holds is one constant for every wallet
//! ([`segment_start`]). The segment's first two half-steps rebase it on the
//! wallet's last published Omega (DESIGN.md §8):
//!
//! | half-step | side | kind | folds | work |
//! |---|---|---|---|---|
//! | 1 | Q | [`QKind::Frame`] | nothing (the segment's first instance) | frame only |
//! | 2 | A | [`AKind::Rebase`] | `Frame` | hard-verifies the predecessor Omega, starts at its lineage |
//!
//! Then the operations follow; a Receive's first Frame folds the Rebase or the
//! previous operation's last A.
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
//! ([`A_EXPORT`]), as today's A frame hands them to Omega. The predecessor
//! Omega's claims go the same way: the Rebase folds its opening and `acc_P`
//! into A's, and exports `acc_V` in mode Accept, which the Frame step folding
//! the Rebase folds into Q's. Every Frame step folds the export claim of the A
//! instance it folds; the A kinds that hand nothing over export the trivial
//! claim in mode Trivial ([`trivial_export`]).

pub mod sigma;
pub mod signature;

use ff::{Field, PrimeField};
use iroha_fold::{
    circuit::{
        FoldLanes, FoldVerifierChips, FoldVerifierConfig, ScalarCell,
        claims::{
            ClaimCells, assign_claim_accumulator, assign_claim_proof, claim_accumulator_digest, verify_claim_fold,
            verify_claim_fold_or_start,
        },
        ivc::{assert_equal_unless, fold_frame, foreign_bounded_word, frame_outputs},
    },
    claims::{Claim, ClaimAccumulator, ClaimProof, fold_claim, fold_or_start_claim, placeholder_claim_accumulator},
    ivc::{ChainSide, FrameWitness, embed, ivc_hash},
    relation::FoldShape,
};
use iroha_pasta::{Eq, EqAffine, Fp, Fq, PastaCurve};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    transcript::{decode_point, generator_coordinates},
};
use iroha_plonk_gadgets::{GlueChip, Word, statement::foreign_limbs};
use iroha_plonk_recursion::{
    FoldInput, VESTA_TRIVIAL_GENERATOR, accumulation_circuit::FoldInputCells, obligation::ModeCells,
};

use crate::{a_relation::LINEAGE_FIELDS, q_sigma::native::IncomingMode};

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
/// today's A-frame incoming Vesta words (a Vesta claim's coordinate limbs and
/// challenges, its one-hot mode, the correction's coordinate limbs). A1
/// exports the incoming `acc_V` and the Rebase the predecessor's; every other
/// A kind exports [`trivial_export`].
pub const A_EXPORT: usize = 27;

/// The program counter of [`segment_base`]: no operation's sub-step accepts it,
/// so only a Rebase can follow a segment's first instance.
pub const REBASE_PC: u64 = u64::from_le_bytes(*b"kgwfrbs1");

/// The step bound of the deciders of `U_A` (the A relations' accumulators,
/// `iroha_fold::decider_v2::DeciderV2Plan::with_step_runs`): key polynomials
/// of at most this many runs are evaluated by the verifier instead of opened.
/// The A batch's commitment side is the wraps' tightest lane and its field
/// side has room; the Q batch keeps the default bound.
pub const A_STEP_RUNS: usize = 128;

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
    /// A segment's first A step: hard-verifies the wallet's last published
    /// Omega and starts the segment at its lineage.
    Rebase,
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
pub const Q_DIGESTS: usize = 5 + 1;
/// Position of A's claim-accumulator digest in its list.
pub const A_CLAIM_SLOT: usize = 3;
/// Position of Q's claim-accumulator digest in its list.
pub const Q_CLAIM_SLOT: usize = 5;

impl AKind {
    /// Every A kind, in digest order.
    pub const ALL: [Self; 5] = [Self::Receive1, Self::Receive3, Self::Receive4, Self::Receive5, Self::Rebase];

    /// The Q kind a step of this kind folds.
    #[must_use]
    pub const fn folds(self) -> QKind {
        match self {
            Self::Receive1 | Self::Receive5 | Self::Rebase => QKind::Frame,
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
    /// step follows A4 inside a Receive, the previous operation's last A (A5 of
    /// a Receive) or the segment's Rebase.
    #[must_use]
    pub const fn choices(self) -> u64 {
        match self {
            Self::Frame => {
                (1 << AKind::Receive4 as usize) | (1 << AKind::Receive5 as usize) | (1 << AKind::Rebase as usize)
            }
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

/// The A state every segment starts from: no lineage, no context and
/// [`REBASE_PC`]. Every wallet's segments share it, so the start passthrough
/// is one constant ([`segment_start`]); the Rebase replaces it by the lineage
/// the predecessor Omega proves.
#[must_use]
pub fn segment_base() -> Vec<Fp> {
    let mut state = vec![Fp::ZERO; A_STATE];
    state[A_PC] = Fp::from(REBASE_PC);
    state
}

/// The passthrough of every segment's first instance, a constant of every Q
/// step: A's base hash over [`segment_base`].
#[must_use]
pub fn segment_start() -> Fq {
    start_passthrough(&segment_base())
}

/// The pinned trivial Vesta claim: every challenge one, the generator they
/// fold.
///
/// # Panics
/// Never: the trivial generator is a fixed canonical point.
#[must_use]
pub fn trivial_vesta() -> FoldInput<Eq> {
    let point = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).expect("the trivial generator decodes");
    FoldInput::from_normalized(point, 16, [Fp::ONE; 16]).expect("the trivial claim is canonical")
}

/// An export column natively ([`A_EXPORT`]): the claim's coordinate limbs and
/// challenges, the one-hot mode (accept, trivial, corrected), the correction's
/// coordinate limbs.
#[must_use]
pub fn export_words(claim: &FoldInput<Eq>, mode: IncomingMode, correction: &EqAffine) -> Vec<Fp> {
    let mut words = Vec::with_capacity(A_EXPORT);
    let (x, y) = generator_coordinates::<Eq>(claim.g());
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    words.extend(claim.challenges());
    let bits = match mode {
        IncomingMode::Accept => [1, 0, 0],
        IncomingMode::Trivial => [0, 1, 0],
        IncomingMode::Corrected => [0, 0, 1],
    };
    words.extend(bits.map(Fp::from));
    let (x, y) = generator_coordinates::<Eq>(correction);
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    words
}

/// The export of an A kind that hands no claim to the Q side: the trivial claim
/// in mode Trivial, with its own generator in the correction slot.
#[must_use]
pub fn trivial_export() -> Vec<Fp> {
    let trivial = trivial_vesta();
    export_words(&trivial, IncomingMode::Trivial, trivial.g())
}

/// The claim an A instance's export column hands to the Q side, selected with
/// today's incoming selector on the fold lanes (as
/// `FoldInputCells::select_incoming`): Accept keeps the exported claim,
/// Trivial takes the pinned trivial claim, Corrected replaces its generator by
/// the correction, which must differ from it. `value` is the selection,
/// natively.
pub(crate) fn export_claim_cells(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    column: &[ScalarCell<Eq>],
    value: Option<&FoldInput<Eq>>,
) -> Result<ClaimCells<Eq>, Error> {
    if column.len() != A_EXPORT {
        return Err(Error::Synthesis);
    }
    let mut small = Vec::with_capacity(11);
    for cell in column[..4].iter().chain(&column[20..]) {
        small.push(foreign_bounded_word(lanes, region, cell)?);
    }
    let [x_lo, x_hi, y_lo, y_hi, accept, trivial, corrected, cx_lo, cx_hi, cy_lo, cy_hi] = &small[..] else {
        return Err(Error::Synthesis);
    };
    let mode = ModeCells::constrain(&mut *lanes.glue, region, &[accept.clone(), trivial.clone(), corrected.clone()])?;
    let shift = Fq::from(2).pow_vartime([128]);
    let mut coordinate =
        |lo: &Word<Fq>, hi: &Word<Fq>| lanes.glue.linear(region, &[(Fq::ONE, lo), (shift, hi)], Fq::ZERO);
    let (x, y) = (coordinate(x_lo, x_hi)?, coordinate(y_lo, y_hi)?);
    let (cx, cy) = (coordinate(cx_lo, cx_hi)?, coordinate(cy_lo, cy_hi)?);
    let same_x = lanes.glue.is_equal(region, &x, &cx)?;
    let same_y = lanes.glue.is_equal(region, &y, &cy)?;
    let same = lanes.glue.and(region, &same_x, &same_y)?;
    let forbidden = lanes.glue.and(region, mode.corrected(), &same)?;
    lanes.glue.enforce_constant(region, forbidden.word(), Fq::ZERO)?;
    let pinned = trivial_vesta();
    let (tx, ty) = generator_coordinates::<Eq>(pinned.g());
    let mut point = Vec::with_capacity(2);
    for (original, correction, constant) in [(&x, &cx, tx), (&y, &cy, ty)] {
        let selected = lanes.glue.select(region, mode.corrected(), correction, original)?;
        let constant = lanes.glue.constant(region, constant)?;
        point.push(lanes.glue.select(region, mode.trivial(), &constant, &selected)?);
    }
    let g = lanes.assign_point(region, value.map_or_else(Value::unknown, |v| Value::known(Eq::from(*v.g()))))?;
    GlueChip::assert_equal(region, g.x(), &point[0])?;
    GlueChip::assert_equal(region, g.y(), &point[1])?;
    let one = lanes.glue.constant(region, Fq::ONE)?;
    let zero = lanes.glue.constant(region, Fq::ZERO)?;
    let mut u = Vec::with_capacity(16);
    for (i, cell) in column[4..20].iter().enumerate() {
        let lo = lanes.glue.select(region, mode.trivial(), &one, cell.s6.lo().word())?;
        let hi = lanes.glue.select(region, mode.trivial(), &zero, cell.s6.hi().word())?;
        let scalar = lanes.assign_scalar(region, value.map_or_else(Value::unknown, |v| Value::known(v.challenges()[i])))?;
        GlueChip::assert_equal(region, scalar.s6.lo().word(), &lo)?;
        GlueChip::assert_equal(region, scalar.s6.hi().word(), &hi)?;
        u.push(scalar);
    }
    Ok(ClaimCells { g, u })
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
/// instance (A4, the previous operation's last A or the segment's Rebase, as
/// its witness chooses) and folds the claim that instance exports into Q's
/// claim accumulator ([`export_claim_cells`]): the predecessor's `acc_V` after
/// a Rebase, the trivial claim otherwise. A segment's first instance folds no
/// A instance; its placeholder carries [`trivial_export`].
#[derive(Clone, Debug)]
pub struct QFrameStep {
    /// The shape of the A relations it folds.
    pub shape: FoldShape,
    /// The step witness (`None` at key generation).
    pub witness: Option<QFrameStepWitness>,
}

/// The witness of a [`QFrameStep`].
#[derive(Clone, Debug)]
pub struct QFrameStepWitness {
    /// The frame: the preceding A instance and the accumulator it folds into.
    pub frame: FrameWitness<Eq>,
    /// The claim that instance exports, selected natively.
    pub selected: FoldInput<Eq>,
    /// The claim accumulator Q keeps before this step and the fold's messages.
    pub claims: ClaimFolds<Eq>,
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
        let witness = self.witness.as_ref();
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
                    segment_start(),
                    Q_STATE,
                    witness.map(|w| &w.frame),
                )?;
                let column = frame.public.get(1).ok_or(Error::Synthesis)?;
                let claim = export_claim_cells(&mut lanes, &mut region, column, witness.map(|w| &w.selected))?;
                let digests = fold_claims_cells(
                    &mut lanes,
                    &mut region,
                    (&CLAIM_Q, Q_CLAIM_SLOT),
                    &frame.digests,
                    &[claim],
                    witness.map(|w| &w.claims),
                )?;
                frame_outputs(&mut lanes, &mut region, &chain, &frame, &[], &digests, &[])
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
        // A segment opens with a Frame and the Rebase, which a Frame folds.
        assert_eq!(AKind::Rebase.folds(), QKind::Frame);
        assert!(QKind::Frame.folds(AKind::Rebase));
        // The segment base is no operation's state, and the start passthrough
        // is the base hash over it.
        let base = segment_base();
        assert_eq!(base.len(), A_STATE);
        assert_eq!(base[A_PC], Fp::from(REBASE_PC));
        assert!(base[..A_PC].iter().all(|v| *v == Fp::ZERO));
        assert_eq!(segment_start(), start_passthrough(&base));
        // The trivial export is the pinned claim in mode Trivial.
        let export = trivial_export();
        assert_eq!(export.len(), A_EXPORT);
        assert_eq!(export[20..23], [Fp::ZERO, Fp::ONE, Fp::ZERO]);
        assert_eq!(export[4..20], [Fp::ONE; 16]);
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
