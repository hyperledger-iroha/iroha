//! The toy IVC step circuit shared by the IVC tests (output `IVC.md`): a fold
//! frame (`circuit::ivc::fold_frame`) around a toy business. Q steps double their
//! state and add one; A steps add the square of the Q bridge they read.

#![allow(dead_code)]

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{assert_equal_unless, fold_frame, frame_outputs},
    },
    ivc::{ChainSide, FrameWitness},
    relation::FoldShape,
};
use iroha_pasta::{PastaCurve, PastaField};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner},
};
use iroha_plonk_gadgets::Word;

pub const K: u32 = 16;
/// Fold-transcript domains of the two relations.
pub const DOMAIN_Q: [u8; 32] = *b"toy ivc Q relation fold domain!!";
pub const DOMAIN_A: [u8; 32] = *b"toy ivc A relation fold domain!!";
/// IVC-hash domains of the two sides.
pub const IVC_Q: [u8; 32] = *b"toy ivc Q side hash domain......";
pub const IVC_A: [u8; 32] = *b"toy ivc A side hash domain......";
/// The segment base (both sides' first state).
pub const BASE: u64 = 0x5eed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Q,
    A,
}

impl Side {
    pub fn chain(self) -> ChainSide {
        match self {
            Self::Q => ChainSide { ivc_domain: IVC_Q, fold_domain: DOMAIN_A, first_step: 1 },
            Self::A => ChainSide { ivc_domain: IVC_A, fold_domain: DOMAIN_Q, first_step: 2 },
        }
    }
}

pub struct Step<C: PastaCurve> {
    pub side: Side,
    pub shape: FoldShape,
    pub witness: Option<FrameWitness<C>>,
}

#[derive(Clone, Debug)]
pub struct Config<C: PastaCurve> {
    fold: FoldVerifierConfig<C>,
    instance: Column<InstanceColumn>,
}

impl<C: PastaCurve> Circuit<C::Base> for Step<C> {
    type Config = Config<C>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { side: self.side, shape: self.shape.clone(), witness: None }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(&self, config: Config<C>, mut layouter: impl Layouter<C::Base>) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let outputs = layouter.assign_region(|| "step", |mut region| self.layout(&mut chips, &mut region))?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}

impl<C: PastaCurve> Step<C> {
    fn layout(&self, chips: &mut FoldVerifierChips<C>, region: &mut Region<'_, C::Base>) -> Result<Vec<Word<C::Base>>, Error> {
        let chain = self.side.chain();
        let frame = fold_frame(chips, region, &chain, &self.shape, C::Base::from(BASE), 1, self.witness.as_ref())?;
        // The side's first instance starts from the base state.
        let later = chips.glue.not(region, &frame.first)?;
        assert_equal_unless(chips, region, &frame.state[0], &frame.base, Some(&later))?;
        // Toy business: Q doubles and adds one; A adds the square of Q's bridge.
        let next = match self.side {
            Side::Q => chips.glue.linear(region, &[(C::Base::from(2), &frame.state[0])], <C::Base as Field>::ONE)?,
            Side::A => {
                let square = chips.glue.mul(region, &frame.bridge[0], &frame.bridge[0])?;
                chips.glue.add(region, &frame.state[0], &square)?
            }
        };
        frame_outputs(chips, region, &chain, &frame, &[next], &[frame.step.clone()])
    }
}

/// The toy business, natively.
pub fn business<F: PastaField>(side: Side, state: F, bridge: F) -> F {
    match side {
        Side::Q => state.double() + F::ONE,
        Side::A => state + bridge.square(),
    }
}

