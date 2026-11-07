//! The genuine Q step shared by the IVC tests: the Receive signature leaf
//! (`QSignatureCircuit`: own receipt hard, incoming receipt and Request soft, real
//! `SHA256withECDSA` P-256 signatures) beside a Q fold frame.

#![allow(dead_code)]

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_kagemusha_proof::q_signature::{
    QSignatureCircuit, QSignatureConfig, QSignaturePlan, SignatureKey, SignatureSlot, SignatureWitness,
};
use iroha_pasta::{Eq, Fp, Fq};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner},
};
use iroha_plonk_gadgets::{
    p256::{
        VerifyMode,
        native::{self, Affine, HALF_N, ORDER, verify_prehashed},
    },
    sha256::native::sha256_of_digest,
};

use super::step::{BASE, Side, claim_business, start_passthrough};

/// A genuine `SHA256withECDSA` signature over the 32-byte encoding of `digest`.
pub fn signature(digest: u64, secret: u64, nonce: u64) -> SignatureWitness {
    let digest = Fp::from(digest);
    let hash = sha256_of_digest(&digest);
    let message = core::array::from_fn(|i| u64::from_be_bytes(hash[(3 - i) * 8..(4 - i) * 8].try_into().unwrap()));
    let secret = [secret, 0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321, 0x0123_4567];
    let nonce = [nonce, 0x0bad_cafe_dead_beef, 0x1357_9bdf_2468_ace0, 0x0765_4321];
    let key = native::mul(&Affine::GENERATOR, &secret).unwrap();
    let r = ORDER.reduce_once(&native::mul(&Affine::GENERATOR, &nonce).unwrap().x);
    let mut s = ORDER.mul(&ORDER.inverse(&nonce), &ORDER.add(&ORDER.reduce_once(&message), &ORDER.mul(&r, &secret)));
    if native::words_cmp(&s, &HALF_N).is_gt() {
        s = ORDER.neg(&s);
    }
    assert!(verify_prehashed(&message, &r, &s, &key));
    SignatureWitness { digest, key: [key.x, key.y], signature: [r, s] }
}

/// Receive `i`: the wallet's own key signs its receipt and its Request; a payer
/// key signs the incoming receipt.
pub fn leaf(i: u64) -> QSignatureCircuit {
    let slot = |mode| SignatureSlot { mode, key: SignatureKey::Variable };
    let plan = QSignaturePlan::new(vec![slot(VerifyMode::Hard), slot(VerifyMode::Soft), slot(VerifyMode::Soft)]).unwrap();
    let own = 0x5eed_0001;
    let witnesses = vec![
        signature(0x1111_0000 + i, own, 0xa100 + i),
        signature(0x2222_0000 + i, 0x5eed_1000 + i, 0xa200 + i),
        signature(0x3333_0000 + i, own, 0xa300 + i),
    ];
    QSignatureCircuit::new(plan, witnesses).unwrap()
}

/// The signature leaf beside a Q fold frame; the frame column is instance column
/// 0, the leaf's public column instance column 1.
#[derive(Clone)]
pub struct QStep {
    pub leaf: QSignatureCircuit,
    pub shape: FoldShape,
    pub witness: Option<FrameWitness<Eq>>,
}

#[derive(Clone, Debug)]
pub struct QStepConfig {
    frame: Column<InstanceColumn>,
    fold: FoldVerifierConfig<Eq>,
    leaf: QSignatureConfig,
}

impl Circuit<Fq> for QStep {
    type Config = QStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = <QSignatureCircuit as Circuit<Fq>>::Params;

    fn params(&self) -> Self::Params {
        self.leaf.params()
    }

    fn without_witnesses(&self) -> Self {
        Self { leaf: self.leaf.without_witnesses(), shape: self.shape.clone(), witness: None }
    }

    fn configure(_meta: &mut ConstraintSystem<Fq>) -> QStepConfig {
        unreachable!("configured with the leaf's parameters")
    }

    fn configure_with_params(meta: &mut ConstraintSystem<Fq>, params: Self::Params) -> QStepConfig {
        let frame = meta.instance_column(3);
        meta.enable_equality(frame);
        let fold = FoldVerifierConfig::configure(meta);
        let leaf = QSignatureCircuit::configure_with_params(meta, params);
        QStepConfig { frame, fold, leaf }
    }

    fn synthesize(&self, config: QStepConfig, mut layouter: impl Layouter<Fq>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(
            || "fold frame",
            |mut region| {
                let chain = Side::Q.chain();
                let frame = fold_frame(
                    &mut chips.lanes(),
                    &mut region,
                    &chain,
                    &self.shape,
                    Fq::from(BASE),
                    start_passthrough(),
                    1,
                    self.witness.as_ref(),
                )?;
                let later = chips.glue.not(&mut region, &frame.first)?;
                assert_equal_unless(&mut chips.lanes(), &mut region, &frame.state[0], &frame.base, Some(&later))?;
                let digests = claim_business(&mut chips, &mut region, Side::Q, &frame)?;
                let next = chips.glue.linear(&mut region, &[(Fq::from(2), &frame.state[0])], Fq::ONE)?;
                frame_outputs(&mut chips.lanes(), &mut region, &chain, &frame, &[next], &digests, &[frame.step.clone()])
            },
        )?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        self.leaf.synthesize(config.leaf, layouter.namespace(|| "signature leaf"))
    }
}

