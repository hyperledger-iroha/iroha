//! The folded Receive's Q Signature step (half-step 5, [`QKind::Signature`]):
//! today's Q1 merged signature leaf (the own receipt hard, the Send receipt
//! and Request soft) beside the frame folding the A3 instance. The leaf has no
//! Vesta curve lane, so the frame runs on its own fold-verifier columns. Its
//! public columns are the frame column, then Q1's column, which the A4 step
//! reads from the instance it folds. Signatures defer no IPA claim, so the
//! claim accumulator passes through.

use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{fold_frame, frame_outputs},
    },
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Eq, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
};

use super::{AKind, BASE, Q_FRAME, Q_STATE, QKind, segment_start};
use crate::q_signature::{QSignatureCircuit, QSignatureConfig};

/// The Q Signature step.
#[derive(Clone, Debug)]
pub struct QSignatureStep {
    /// Today's Q1 leaf.
    pub leaf: QSignatureCircuit,
    /// The shape of the A relations it folds.
    pub shape: FoldShape,
    /// The frame witness (`None` at key generation).
    pub frame: Option<FrameWitness<Eq>>,
}

/// Columns of a [`QSignatureStep`]: the frame column, the fold verifier and
/// the leaf (whose public column follows the frame column).
#[derive(Clone, Debug)]
pub struct QSignatureStepConfig {
    frame: Column<Instance>,
    fold: FoldVerifierConfig<Eq>,
    leaf: QSignatureConfig,
}

impl Circuit<Fq> for QSignatureStep {
    type Config = QSignatureStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = <QSignatureCircuit as Circuit<Fq>>::Params;

    fn params(&self) -> Self::Params {
        self.leaf.params()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            leaf: self.leaf.without_witnesses(),
            frame: None,
            ..self.clone()
        }
    }

    fn configure(_meta: &mut ConstraintSystem<Fq>) -> QSignatureStepConfig {
        unreachable!("configured with the leaf's parameters")
    }

    fn configure_with_params(
        meta: &mut ConstraintSystem<Fq>,
        params: Self::Params,
    ) -> QSignatureStepConfig {
        let frame = meta.instance_column(Q_FRAME);
        meta.enable_equality(frame);
        let fold = FoldVerifierConfig::configure(meta);
        let leaf = QSignatureCircuit::configure_with_params(meta, params);
        QSignatureStepConfig { frame, fold, leaf }
    }

    fn synthesize(
        &self,
        config: QSignatureStepConfig,
        mut layouter: impl Layouter<Fq>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let chain = QKind::Signature.chain(AKind::Receive3);
        let outputs = layouter.assign_region(
            || "folded Receive Q signature frame",
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
                    self.frame.as_ref(),
                )?;
                frame_outputs(
                    &mut lanes,
                    &mut region,
                    &chain,
                    &frame,
                    &[],
                    &frame.digests,
                    &[],
                )
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        self.leaf
            .synthesize(config.leaf, layouter.namespace(|| "merged signature leaf"))
    }
}
