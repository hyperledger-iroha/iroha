//! Closed history continuation with a witnessed, context-bound predecessor key.
//!
//! The source key depends on the future wrapper descriptor only. Its complete
//! key digest is carried unchanged through each prefix and independently pinned
//! by the receipt owner. The shared wrapper admits exactly genesis and append.

use super::{HistoryAnchor, PREFIX_PROGRAM_ID, PROGRAM_ID, prefix_context, prefix_context_cells};
use crate::finality::continuity::{
    SourceCheckpoint, SourceEndpoints, SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig,
    SourceVerifier,
    pair::{assign_evidence, frame_native},
    single::{verify_key_cells, verify_key_native_cancellable},
};
use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::ConstraintSystem,
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::ipa::PinnedParams,
    transcript::TranscriptRepr,
};
use iroha_plonk_gadgets::{GlueChip, Word};
use iroha_plonk_recursion::{
    AccumulatorT, FOLD_WITNESS_BYTES, FoldConfig,
    accumulation_circuit::{FoldInputCells, FoldPlan, FoldSource},
    create_fold,
    verifier::{VerificationMode, VerifierChip, VerifierPlan},
};

/// Fixed body source and predecessor wrapper layout, with no future key constant.
#[derive(Clone, Debug)]
pub struct HistoryAppendPlan {
    anchor: HistoryAnchor,
    previous: VerifierPlan<Ep>,
    body: SourceVerifier,
    fold: FoldPlan<Ep>,
}
impl HistoryAppendPlan {
    /// Select the exact future wrapper descriptor and qualified ordinary block body.
    /// The finite catalog owner must qualify this source and the shared wrapper.
    /// # Errors
    /// Non-k16 parameters or wrong complete wrapper public layout/profile.
    pub fn new(
        anchor: HistoryAnchor,
        descriptor: DescriptorBinding,
        body: SourceVerifier,
        pallas: PinnedParams<Ep>,
    ) -> Result<Self, Error> {
        if pallas.k() != 16
            || descriptor.descriptor().instance_lengths != [1, 2, 16]
            || descriptor.descriptor().instance_types.as_deref()
                != Some(&crate::omega::OmegaPlan::instance_types())
        {
            return Err(Error::Synthesis);
        }
        let fold = FoldPlan::with_sources(&pallas, vec![FoldSource::Fixed(16); 4])
            .map_err(|_| Error::Synthesis)?;
        let previous = VerifierPlan::new(descriptor, pallas).map_err(|_| Error::Synthesis)?;
        Ok(Self {
            anchor,
            previous,
            body,
            fold,
        })
    }
    /// Installed policy, fixed into the original source key.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }
    /// Exact predecessor wrapper descriptor; its actual key remains a witness.
    pub fn wrapper_binding(&self) -> &DescriptorBinding {
        self.previous.binding()
    }
}

pub(super) fn join_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    anchor: &Word<Fp>,
    key: &Word<Fp>,
    children: [&SourceEndpoints; 2],
) -> Result<SourceEndpoints, Error> {
    let context = prefix_context_cells(chip, region, anchor, key)?;
    let [prefix, body] = children.map(SourceEndpoints::words);
    for (words, program) in [(&prefix, PREFIX_PROGRAM_ID), (&body, PROGRAM_ID)] {
        for (at, value) in [(0, Fp::from(program)), (2, Fp::ZERO), (3, Fp::ONE)] {
            GlueChip::assert_constant(region, &words[at], value)?;
        }
    }
    GlueChip::assert_equal(region, &prefix[1], &context)?;
    GlueChip::assert_constant(region, &prefix[4], Fp::ZERO)?;
    GlueChip::assert_equal(region, &body[1], anchor)?;
    GlueChip::assert_equal(region, &prefix[5], &body[4])?;
    SourceEndpoints::leaf(
        chip,
        region,
        Fp::from(PREFIX_PROGRAM_ID),
        &context,
        0,
        1,
        &prefix[4],
        &body[5],
    )
}

/// Append one exact verified block to a genesis-rooted prefix under a finite key catalog.
#[derive(Clone, Debug)]
pub struct HistoryAppendCircuit {
    plan: HistoryAppendPlan,
    previous_key: Option<VerifyingKey<Ep>>,
    children: [SourceNodeEvidence; 2],
    fold: [u8; FOLD_WITNESS_BYTES],
    pallas: AccumulatorT<Ep>,
    endpoints: [Fp; 6],
    public: [Fp; 69],
    known: bool,
}
impl HistoryAppendCircuit {
    /// Original source layout with no live prefix key, proof, or accepted host fact.
    /// # Errors
    /// Invalid canonical filler encodings.
    pub fn for_source(plan: HistoryAppendPlan) -> Result<Self, Error> {
        let body = plan.body.blank_evidence()?;
        let mut previous = body.clone();
        previous.proof = vec![0; plan.previous.proof_length()];
        Ok(Self {
            pallas: previous.pallas.clone(),
            plan,
            previous_key: None,
            children: [previous, body],
            fold: [0; FOLD_WITNESS_BYTES],
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
            known: false,
        })
    }
    /// Verify both originals and retain every Pallas/Vesta obligation.
    /// # Errors
    /// Wrong prefix key/context, partial history, wrong body, or invalid proof/fold.
    pub fn prepare(
        plan: HistoryAppendPlan,
        previous_key: VerifyingKey<Ep>,
        children: [SourceNodeEvidence; 2],
        vesta: &PinnedParams<Eq>,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<Self, Error> {
        let cancellation = config.cancellation.as_ref();
        iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(|_| Error::Cancelled)?;
        let key_digest = previous_key
            .kagemusha_digest(plan.previous.binding())
            .map_err(|_| Error::Synthesis)?;
        let anchor = plan.anchor.digest();
        let context = prefix_context(anchor, key_digest);
        let [prefix, body] = children.each_ref().map(|c| c.endpoints);
        if prefix[..5]
            != [
                Fp::from(PREFIX_PROGRAM_ID),
                context,
                Fp::ZERO,
                Fp::ONE,
                Fp::ZERO,
            ]
            || body[..4] != [Fp::from(PROGRAM_ID), anchor, Fp::ZERO, Fp::ONE]
            || prefix[5] != body[4]
        {
            return Err(Error::Synthesis);
        }
        let first = verify_key_native_cancellable(
            &plan.previous,
            &previous_key,
            &children[0],
            vesta,
            config.kernel_budget,
            cancellation,
        )?;
        let second = plan.body.verify_native_cancellable(
            &children[1],
            vesta,
            config.kernel_budget,
            cancellation,
        )?;
        let (fold, pallas) = create_fold(
            plan.previous.params(),
            &[
                children[0].pallas.as_input(),
                first,
                children[1].pallas.as_input(),
                second,
            ],
            salt.to_repr(),
            config,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Synthesis
            }
        })?;
        pallas
            .decide_cancellable(plan.previous.params(), config.kernel_budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Synthesis
                }
            })?;
        let endpoints = [
            Fp::from(PREFIX_PROGRAM_ID),
            context,
            Fp::ZERO,
            Fp::ONE,
            Fp::ZERO,
            body[5],
        ];
        let public = frame_native(
            endpoints,
            &pallas,
            [&children[0].vesta, &children[1].vesta],
            vesta,
            config.kernel_budget,
        )?;
        Ok(Self {
            plan,
            previous_key: Some(previous_key),
            children,
            fold: fold.to_bytes(),
            pallas,
            endpoints,
            public,
            known: true,
        })
    }
    /// Exact installed anchor.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.plan.anchor
    }
    /// Original complete public69 frame.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Original complete singleton prefix opening.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Entire folded Pallas obligation.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.children.each_ref().map(|c| c.vesta.clone())
    }
    fn value<T: Copy>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
        } else {
            Value::unknown()
        }
    }
}
impl Circuit<Fp> for HistoryAppendCircuit {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        let mut blank = self.clone();
        blank.known = false;
        blank
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        SourceMergeCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "closed history prefix append",
            |mut region| {
                let assigned = assign_evidence(
                    &mut chip,
                    &mut region,
                    &self.children,
                    &self.fold,
                    self.known,
                )?;
                let children = assigned.children();
                let descriptor = self.plan.previous.binding().descriptor();
                let (first, digest) = verify_key_cells(
                    &mut chip,
                    &mut region,
                    &self.plan.previous,
                    children[0],
                    |chip, region| {
                        let repr =
                            self.previous_key
                                .as_ref()
                                .map_or(Ok(Value::unknown()), |key| {
                                    let TranscriptRepr::Base(repr) = *key.transcript_repr() else {
                                        return Err(Error::Synthesis);
                                    };
                                    Ok(self.value(repr))
                                })?;
                        let fixed_count = usize::try_from(descriptor.num_fixed_columns)
                            .map_err(|_| Error::BoundsFailure)?;
                        let fixed = (0..fixed_count)
                            .map(|i| {
                                self.previous_key.as_ref().map_or(Value::unknown(), |key| {
                                    self.value(Ep::from(key.fixed_commitments()[i]))
                                })
                            })
                            .collect::<Vec<_>>();
                        let permutation = (0..descriptor.permutation.len())
                            .map(|i| {
                                self.previous_key.as_ref().map_or(Value::unknown(), |key| {
                                    self.value(Ep::from(key.permutation_commitments()[i]))
                                })
                            })
                            .collect::<Vec<_>>();
                        chip.witness_key(region, repr, &fixed, &permutation)
                    },
                )?;
                let second = self
                    .plan
                    .body
                    .verify_cells(&mut chip, &mut region, children[1])?;
                let anchor = chip
                    .uint()
                    .glue()
                    .constant(&mut region, self.plan.anchor.digest())?;
                let endpoints = join_cells(
                    &mut chip,
                    &mut region,
                    &anchor,
                    &digest,
                    [&assigned.endpoints[0], &assigned.endpoints[1]],
                )?;
                let inputs = vec![
                    first.pallas().clone(),
                    first.opening().clone(),
                    second.pallas().clone(),
                    second.opening().clone(),
                ];
                let folded = chip.verify_fold(
                    &mut region,
                    &self.plan.fold,
                    &inputs,
                    assigned.fold.messages(),
                    assigned.fold.length(),
                    VerificationMode::Hard,
                )?;
                let pallas = FoldInputCells::from_claim(&mut chip, &mut region, &folded.claim)?;
                SourceCheckpoint::from_folded(
                    &mut chip,
                    &mut region,
                    endpoints,
                    pallas,
                    [first.vesta().clone(), second.vesta().clone()],
                )?
                .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
