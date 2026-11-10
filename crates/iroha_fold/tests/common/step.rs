//! The toy IVC step circuit shared by the IVC tests (output `IVC.md`): a fold
//! frame (`circuit::ivc::fold_frame`) around a toy business. Q steps double their
//! state and add one; A steps add the square of the Q bridge they read.
//!
//! Each side keeps two digests: the accumulator of the other side's instances,
//! then its own claim accumulator (A: Pallas claims, Q: Vesta claims). The claim
//! accumulator starts on the side's first instance from one fixed true claim
//! and then passes through unchanged ([`claim_q`], [`claim_a`]).

#![allow(dead_code)]

use std::sync::OnceLock;

use ff::Field;
use iroha_fold::{
    circuit::{
        FoldVerifierChips, FoldVerifierConfig,
        ivc::{Frame, assert_equal_unless, fold_frame, frame_outputs},
    },
    claims::{ClaimAccumulator, claim_for},
    ivc::{ChainSide, FrameWitness, embed, ivc_hash},
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, PastaField};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance as InstanceColumn},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner},
    pcs::ipa::PinnedParams,
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
/// Claim-accumulator domains of the two sides.
pub const CLAIM_Q: [u8; 32] = *b"toy ivc Q side claim domain.....";
pub const CLAIM_A: [u8; 32] = *b"toy ivc A side claim domain.....";
/// Each side's digest list: the kept accumulator, then the claim accumulator.
pub const DIGESTS: usize = 2;
/// Position of the claim accumulator's digest in the list.
pub const CLAIM_SLOT: usize = 1;

/// A claim accumulator started from one fixed true claim over the curve's
/// `2^16` IPA generators, and its witness `s`.
fn start_claims<C: PastaCurve>(domain: &[u8; 32]) -> (ClaimAccumulator<C>, Vec<C::ScalarExt>) {
    let params = PinnedParams::<C>::derive(16).expect("params");
    let u = core::array::from_fn(|i| C::ScalarExt::from(u64::try_from(i).expect("small") + 2));
    let claim = claim_for::<C>(params.params().g(), u).expect("claim");
    ClaimAccumulator::from_claim(domain, &claim)
}

/// Q's claim accumulator (Vesta claims) and its witness, the same at every step.
pub fn claim_q() -> &'static (ClaimAccumulator<Eq>, Vec<Fp>) {
    static CLAIM: OnceLock<(ClaimAccumulator<Eq>, Vec<Fp>)> = OnceLock::new();
    CLAIM.get_or_init(|| start_claims::<Eq>(&CLAIM_Q))
}

/// A's claim accumulator (Pallas claims) and its witness, the same at every step.
pub fn claim_a() -> &'static (ClaimAccumulator<Ep>, Vec<Fq>) {
    static CLAIM: OnceLock<(ClaimAccumulator<Ep>, Vec<Fq>)> = OnceLock::new();
    CLAIM.get_or_init(|| start_claims::<Ep>(&CLAIM_A))
}

/// The digest of a side's claim accumulator, over the side's field (`F` is
/// `Fq` for Q and `Fp` for A).
pub fn claim_digest<F: PastaField>(side: Side) -> F {
    match side {
        Side::Q => embed(&claim_q().0.digest(&CLAIM_Q)),
        Side::A => embed(&claim_a().0.digest(&CLAIM_A)),
    }
}

/// The digest list a side outputs: the frame's, with the claim accumulator
/// started on the side's first instance (natively).
pub fn business_digests<F: PastaField>(side: Side, first: bool, mut digests: Vec<F>) -> Vec<F> {
    if first {
        digests[CLAIM_SLOT] = claim_digest(side);
    }
    digests
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Q,
    A,
}

impl Side {
    pub fn chain(self) -> ChainSide {
        match self {
            Self::Q => ChainSide {
                ivc_domain: IVC_Q,
                fold_domain: DOMAIN_A,
                first_step: 1,
                slot: 0,
                digests: DIGESTS,
                choices: 1,
            },
            Self::A => ChainSide {
                ivc_domain: IVC_A,
                fold_domain: DOMAIN_Q,
                first_step: 2,
                slot: 0,
                digests: DIGESTS,
                choices: 1,
            },
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
        Self {
            side: self.side,
            shape: self.shape.clone(),
            witness: None,
        }
    }

    fn configure(meta: &mut ConstraintSystem<C::Base>) -> Config<C> {
        let fold = FoldVerifierConfig::configure(meta);
        let instance = meta.instance_column(3);
        meta.enable_equality(instance);
        Config { fold, instance }
    }

    fn synthesize(
        &self,
        config: Config<C>,
        mut layouter: impl Layouter<C::Base>,
    ) -> Result<(), Error> {
        let mut chips = FoldVerifierChips::new(&config.fold);
        chips.load_tables(&mut layouter)?;
        let outputs =
            layouter.assign_region(|| "step", |mut region| self.layout(&mut chips, &mut region))?;
        for (row, word) in outputs.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.instance, row)?;
        }
        Ok(())
    }
}

impl<C: PastaCurve> Step<C> {
    fn layout(
        &self,
        chips: &mut FoldVerifierChips<C>,
        region: &mut Region<'_, C::Base>,
    ) -> Result<Vec<Word<C::Base>>, Error> {
        let chain = self.side.chain();
        let frame = fold_frame(
            &mut chips.lanes(),
            region,
            &chain,
            &self.shape,
            C::Base::from(BASE),
            start_passthrough(),
            1,
            self.witness.as_ref(),
        )?;
        // The side's first instance starts from the base state.
        let later = chips.glue.not(region, &frame.first)?;
        assert_equal_unless(
            &mut chips.lanes(),
            region,
            &frame.state[0],
            &frame.base,
            Some(&later),
        )?;
        let digests = claim_business(chips, region, self.side, &frame)?;
        // Toy business: Q doubles and adds one; A adds the square of Q's bridge.
        let next = match self.side {
            Side::Q => chips.glue.linear(
                region,
                &[(C::Base::from(2), &frame.state[0])],
                <C::Base as Field>::ONE,
            )?,
            Side::A => {
                let square = chips.glue.mul(region, &frame.bridge[0], &frame.bridge[0])?;
                chips.glue.add(region, &frame.state[0], &square)?
            }
        };
        frame_outputs(
            &mut chips.lanes(),
            region,
            &chain,
            &frame,
            &[next],
            &digests,
            &[frame.step.clone()],
        )
    }
}

/// The digest list a step outputs: the frame's, with the side's claim
/// accumulator started (a constant digest) on its first instance.
///
/// # Errors
/// Layout errors.
pub fn claim_business<C: PastaCurve>(
    chips: &mut FoldVerifierChips<C>,
    region: &mut Region<'_, C::Base>,
    side: Side,
    frame: &Frame<C>,
) -> Result<Vec<Word<C::Base>>, Error> {
    let started = chips.glue.constant(region, claim_digest(side))?;
    let mut digests = frame.digests.clone();
    digests[CLAIM_SLOT] =
        chips
            .glue
            .select(region, &frame.first, &started, &frame.digests[CLAIM_SLOT])?;
    Ok(digests)
}

/// The toy business, natively.
pub fn business<F: PastaField>(side: Side, state: F, bridge: F) -> F {
    match side {
        Side::Q => state.double() + F::ONE,
        Side::A => state + bridge.square(),
    }
}

/// The passthrough of the chain's first instance (Q's first step): A's base hash
/// `H_A(0, base, [base], [0, 0])`, a circuit constant.
#[must_use]
pub fn start_passthrough<F: PastaField>() -> F {
    embed(&ivc_hash::<Fp>(
        &IVC_A,
        0,
        Fp::from(BASE),
        &[Fp::from(BASE)],
        &[Fp::ZERO; DIGESTS],
    ))
}
