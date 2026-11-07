//! The folded Receive's Q Sigma step (half-step 3, [`QKind::Sigma`]): today's
//! Q0 incoming slot (the `σ_send` soft verifier, the one-hot mode and the
//! selected claim) without its local fold, the selected claim folded into Q's
//! Vesta claim accumulator instead, and the frame folding the A1 instance, all
//! on one PIPA-R verifier's lanes. Its public columns are the frame column,
//! then Q0's five columns (the selected claim where today's folded part is),
//! which the A3 step reads from the instance it folds.

use iroha_fold::{
    circuit::{
        FoldLanes,
        claims::{
            ClaimCells, assign_claim_accumulator, assign_claim_proof, claim_accumulator_digest,
            verify_claim_fold_or_start,
        },
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    claims::{Claim, ClaimAccumulator, ClaimFoldOutput, ClaimProof, fold_or_start_claim, placeholder_claim_accumulator},
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Eq, Fp, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, Word,
    bytes::tape::{BytesChip, BytesConfig},
};
use iroha_plonk_recursion::{
    FoldInput,
    accumulation_circuit::FoldInputCells,
    verifier::{VerifierChip, VerifierConfig},
};

use super::{AKind, BASE, CLAIM_Q, Q_CLAIM_SLOT, Q_FRAME, Q_STATE, QKind};
use crate::q_sigma::QSigmaCircuit;

/// The claim a selected incoming opening defers: its generator and its sixteen
/// challenges (zero-padded to k16).
#[must_use]
pub fn selected_claim(selected: &FoldInput<Eq>) -> Claim<Eq> {
    Claim { g: *selected.g(), u: *selected.challenges() }
}

/// The Q Sigma step's claim business natively: folds the selected claim into
/// the claim accumulator Q keeps, or starts it while absent.
///
/// # Errors
/// As [`fold_or_start_claim`].
pub fn fold_selected(
    kept: Option<(&ClaimAccumulator<Eq>, &[Fp])>,
    selected: &FoldInput<Eq>,
) -> Result<ClaimFoldOutput<Eq>, iroha_fold::Error> {
    fold_or_start_claim(&CLAIM_Q, kept, &selected_claim(selected))
}

/// The witness of a Q Sigma step besides its leaf.
#[derive(Clone, Debug)]
pub struct QSigmaStepWitness {
    /// The frame: the preceding A1 instance and the accumulator it folds into.
    pub frame: FrameWitness<Eq>,
    /// The selected claim (the leaf's output, natively).
    pub selected: FoldInput<Eq>,
    /// The claim accumulator Q keeps before this step (`None` while absent).
    pub claims: Option<ClaimAccumulator<Eq>>,
    /// The claim fold's messages (zero when starting).
    pub claim_proof: ClaimProof<Fp>,
}

/// The Q Sigma step.
#[derive(Clone, Debug)]
pub struct QSigmaStep {
    /// Today's Q0 leaf over the incoming-only plan (its local fold is not laid
    /// out).
    pub leaf: QSigmaCircuit,
    /// The shape of the A relations it folds.
    pub shape: FoldShape,
    /// The passthrough of a segment's first instance
    /// ([`super::start_passthrough`]).
    pub start: Fq,
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

/// The claim cells of the leaf's selected claim on the fold lanes: a point and
/// sixteen scalars copy-bound to the leaf's generator coordinates and challenge
/// limbs.
fn claim_cells(
    lanes: &mut FoldLanes<'_, Eq>,
    region: &mut Region<'_, Fq>,
    selected: &FoldInputCells<Eq>,
    value: Option<&FoldInput<Eq>>,
) -> Result<ClaimCells<Eq>, Error> {
    let point = value.map_or_else(Value::unknown, |v| Value::known(Eq::from(*v.g())));
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

impl QSigmaStep {
    fn layout(
        &self,
        chip: &mut VerifierChip<Eq>,
        bytes: &mut BytesChip<Fq>,
        region: &mut Region<'_, Fq>,
    ) -> Result<(Vec<Word<Fq>>, [Vec<Word<Fq>>; 5]), Error> {
        // The leaf first: its byte rows precede every curve-lane row.
        let incoming = self.leaf.fold_incoming(chip, bytes, region)?;
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
        let chain = QKind::Sigma.chain(AKind::Receive1);
        let witness = self.witness.as_ref();
        let frame = fold_frame(
            &mut lanes,
            region,
            &chain,
            &self.shape,
            Fq::from(BASE),
            self.start,
            Q_STATE,
            witness.map(|w| &w.frame),
        )?;
        // The selected claim folds into Q's claim accumulator (or starts it).
        let claim = claim_cells(&mut lanes, region, &incoming.selected, witness.map(|w| &w.selected))?;
        let placeholder = placeholder_claim_accumulator::<Eq>();
        let acc_value = witness.map(|w| w.claims.as_ref().unwrap_or(&placeholder));
        let acc = assign_claim_accumulator(&mut lanes, region, acc_value)?;
        let proof = assign_claim_proof(&mut lanes, region, witness.map(|w| &w.claim_proof))?;
        let stored = frame.digests[Q_CLAIM_SLOT].clone();
        let absent = lanes.glue.is_zero(region, &stored)?;
        let opened = claim_accumulator_digest(&mut lanes, region, &CLAIM_Q, &acc)?;
        assert_equal_unless(&mut lanes, region, &opened, &stored, Some(&absent))?;
        let folded = verify_claim_fold_or_start(&mut lanes, region, &CLAIM_Q, &acc, &claim, &proof, &absent)?;
        let mut digests = frame.digests.clone();
        digests[Q_CLAIM_SLOT] = claim_accumulator_digest(&mut lanes, region, &CLAIM_Q, &folded)?;
        let outputs = frame_outputs(&mut lanes, region, &chain, &frame, &[], &digests, &[])?;
        Ok((outputs, incoming.columns))
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
