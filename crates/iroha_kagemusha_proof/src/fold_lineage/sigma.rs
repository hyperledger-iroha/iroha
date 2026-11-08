//! The folded Receive's Q Sigma step (half-step 3, [`QKind::Sigma`]): today's
//! Q0 incoming slot (the `σ_send` soft verifier, the one-hot mode and the
//! selected claim) without its local fold, and the frame folding the A1
//! instance, all on one PIPA-R verifier's lanes. Its public columns are the
//! frame column, then Q0's five columns, which the A3 step reads from the
//! instance it folds.
//!
//! Two Vesta claims fold into Q's claim accumulator here: the selected
//! `σ_send` opening, then the incoming Omega's transported `acc_V`, which A1
//! exports with its mode and correction ([`super::A_EXPORT`], today's A-frame
//! words). The step selects `acc_V` with today's selector, as today's Omega
//! does from the A frame.

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldLanes,
        ivc::{fold_frame, foreign_bounded_word, frame_outputs},
    },
    claims::ClaimAccumulator,
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Eq, Fp, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner},
};
use iroha_plonk_gadgets::{
    Word,
    bytes::tape::{BytesChip, BytesConfig},
};
use iroha_plonk_recursion::{
    FoldInput,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    obligation::ModeCells,
    verifier::{VerifierChip, VerifierConfig},
};

use super::{
    A_EXPORT, AKind, BASE, CLAIM_Q, ClaimFolds, Q_CLAIM_SLOT, Q_FRAME, Q_STATE, QKind, claim_cells, claim_of, fold_claims,
    fold_claims_cells, segment_start,
};
use crate::q_sigma::QSigmaCircuit;

/// The Q Sigma step's claim business natively: folds the selected `σ_send`
/// claim, then the selected incoming `acc_V` claim, into the claim accumulator
/// Q keeps (starting it while absent).
///
/// # Errors
/// As [`fold_claims`].
pub fn fold_selected(
    kept: Option<(&ClaimAccumulator<Eq>, &[Fp])>,
    selected: &FoldInput<Eq>,
    incoming: &FoldInput<Eq>,
) -> Result<(ClaimAccumulator<Eq>, Vec<Fp>, ClaimFolds<Eq>), iroha_fold::Error> {
    fold_claims(&CLAIM_Q, kept, &[claim_of(selected), claim_of(incoming)])
}

/// The witness of a Q Sigma step besides its leaf.
#[derive(Clone, Debug)]
pub struct QSigmaStepWitness {
    /// The frame: the preceding A1 instance and the accumulator it folds into.
    pub frame: FrameWitness<Eq>,
    /// The selected `σ_send` claim (the leaf's output, natively).
    pub selected: FoldInput<Eq>,
    /// The selected incoming `acc_V` claim (from A1's export, natively).
    pub incoming: FoldInput<Eq>,
    /// The claim accumulator Q keeps before this step and both folds' messages.
    pub claims: ClaimFolds<Eq>,
}

/// The Q Sigma step.
#[derive(Clone, Debug)]
pub struct QSigmaStep {
    /// Today's Q0 leaf over the incoming-only plan (its local fold is not laid
    /// out).
    pub leaf: QSigmaCircuit,
    /// The shape of the A relations it folds.
    pub shape: FoldShape,
    /// The step witness (`None` at key generation; the leaf then has none either).
    pub witness: Option<QSigmaStepWitness>,
}

/// Columns of a [`QSigmaStep`]: the frame column first, then today's Q0
/// verifier with its byte tape and its five public columns.
#[derive(Clone, Debug)]
pub struct QSigmaStepConfig {
    frame: Column<Instance>,
    verifier: VerifierConfig<Eq>,
    bytes: BytesConfig,
    output: [Column<Instance>; 5],
}

/// A native `Fq` coordinate from its two exported limbs (each below `2^128`).
fn coordinate(chip: &mut VerifierChip<Eq>, region: &mut Region<'_, Fq>, lo: &Word<Fq>, hi: &Word<Fq>) -> Result<Word<Fq>, Error> {
    chip.uint().glue().linear(region, &[(Fq::ONE, lo), (Fq::from(2).pow_vartime([128]), hi)], Fq::ZERO)
}

/// The incoming `acc_V` claim, mode and correction of A1's export column (the
/// folded A1 instance's second public column), selected with today's
/// selector: Accept keeps the claim, Trivial takes the pinned trivial claim,
/// Corrected replaces its generator by a different correction.
fn incoming_vesta(
    chip: &mut VerifierChip<Eq>,
    region: &mut Region<'_, Fq>,
    column: &[iroha_fold::circuit::ScalarCell<Eq>],
    small: &[Word<Fq>],
) -> Result<FoldInputCells<Eq>, Error> {
    let [x_lo, x_hi, y_lo, y_hi, accept, trivial, corrected, cx_lo, cx_hi, cy_lo, cy_hi] = small else {
        return Err(Error::Synthesis);
    };
    let x = coordinate(chip, region, x_lo, x_hi)?;
    let y = coordinate(chip, region, y_lo, y_hi)?;
    let g = chip.constrain_generator(region, &x, &y)?;
    let challenges = column
        .get(4..20)
        .ok_or(Error::Synthesis)?
        .iter()
        .map(|value| ScalarCells::from_limbs(&mut chip.uint(), region, value.s6.lo(), value.s6.hi()))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Synthesis)?;
    let original = FoldInputCells::from_normalized(chip, region, 16, g, challenges)?;
    let mode = ModeCells::constrain(chip.uint().glue(), region, &[accept.clone(), trivial.clone(), corrected.clone()])?;
    let cx = coordinate(chip, region, cx_lo, cx_hi)?;
    let cy = coordinate(chip, region, cy_lo, cy_hi)?;
    let correction = chip.constrain_generator(region, &cx, &cy)?;
    FoldInputCells::select_incoming(chip, region, &original, &correction, &mode)
}

impl QSigmaStep {
    fn layout(
        &self,
        chip: &mut VerifierChip<Eq>,
        bytes: &mut BytesChip<Fq>,
        region: &mut Region<'_, Fq>,
    ) -> Result<(Vec<Word<Fq>>, [Vec<Word<Fq>>; 5]), Error> {
        // The leaf first: its byte rows precede every curve-lane row.
        let slot = self.leaf.fold_incoming(chip, bytes, region)?;
        let chain = QKind::Sigma.chain(AKind::Receive1);
        let witness = self.witness.as_ref();
        let (frame, small) = {
            let refs = chip.fold_lanes()?;
            let mut lanes = FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
            let frame = fold_frame(
                &mut lanes,
                region,
                &chain,
                &self.shape,
                Fq::from(BASE),
                segment_start(),
                Q_STATE,
                witness.map(|w| &w.frame),
            )?;
            // A1's export: coordinate limbs, mode bits and correction limbs are
            // small words; the challenges stay foreign scalars.
            let column = frame.public.get(1).filter(|c| c.len() == A_EXPORT).ok_or(Error::Synthesis)?;
            let mut small = Vec::with_capacity(11);
            for value in column[..4].iter().chain(&column[20..]) {
                small.push(foreign_bounded_word(&mut lanes, region, value)?);
            }
            (frame, small)
        };
        let incoming = incoming_vesta(chip, region, &frame.public[1], &small)?;
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
        let claims = [
            claim_cells(&mut lanes, region, &slot.selected, witness.map(|w| &w.selected))?,
            claim_cells(&mut lanes, region, &incoming, witness.map(|w| &w.incoming))?,
        ];
        let digests =
            fold_claims_cells(&mut lanes, region, (&CLAIM_Q, Q_CLAIM_SLOT), &frame.digests, &claims, witness.map(|w| &w.claims))?;
        let outputs = frame_outputs(&mut lanes, region, &chain, &frame, &[], &digests, &[])?;
        Ok((outputs, slot.columns))
    }
}

impl Circuit<Fq> for QSigmaStep {
    type Config = QSigmaStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ([usize; 5], usize);

    fn params(&self) -> Self::Params {
        (self.leaf.plan.instance_lengths(), self.leaf.folded_byte_rows().unwrap_or(0))
    }

    fn without_witnesses(&self) -> Self {
        Self { leaf: self.leaf.without_witnesses(), witness: None, ..self.clone() }
    }

    fn configure(meta: &mut ConstraintSystem<Fq>) -> QSigmaStepConfig {
        Self::configure_with_params(meta, ([0; 5], 0))
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, (lengths, byte_rows): Self::Params) -> QSigmaStepConfig {
        let frame = meta.instance_column(Q_FRAME);
        meta.enable_equality(frame);
        let (verifier, bytes) = VerifierConfig::configure_with_byte_tape(meta, byte_rows);
        let output = lengths.map(|length| {
            let column = meta.instance_column(length);
            meta.enable_equality(column);
            column
        });
        QSigmaStepConfig { frame, verifier, bytes, output }
    }

    fn synthesize(&self, config: QSigmaStepConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let (outputs, columns) =
            layouter.assign_region(|| "folded Receive Q sigma step", |mut region| self.layout(&mut chip, &mut bytes, &mut region))?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        for (column, values) in config.output.into_iter().zip(columns) {
            for (row, value) in values.into_iter().enumerate() {
                layouter.constrain_instance(value.cell(), column, row)?;
            }
        }
        Ok(())
    }
}
