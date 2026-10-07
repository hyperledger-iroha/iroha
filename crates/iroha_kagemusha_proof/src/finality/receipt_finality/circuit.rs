//! Hard source verification with exact terminal receipt byte parsing.

use super::*;
use crate::finality::{
    continuity::{
        SourceMergeCircuit, SourceNodeEvidence, SourcePairConfig, SourcePairPlan,
        pair::SourcePairWitness,
    },
    load_source::LoadReceiptProjection,
};
use iroha_pasta::Eq;
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_gadgets::bytes::tape::{BytesChip, BytesConfig};
use iroha_plonk_recursion::{AccumulatorT, FoldConfig};

/// Exact pair verifier lanes plus a source-linked receipt byte tape.
#[derive(Clone, Debug)]
pub struct ReceiptFinalityConfig {
    pair: SourcePairConfig,
    bytes: BytesConfig,
}

/// Ordinary Load receipt finality under one independently installed global root.
/// The source key fixes its anchor and both child keys; proof generation never
/// accepts a host authorization bit or separate issuer signature.
#[derive(Clone, Debug)]
pub struct ReceiptFinalityCircuit {
    anchor: HistoryAnchor,
    history_key: Fp,
    pair: SourcePairWitness,
    input: ReceiptFinalityInput,
    endpoints: [Fp; 6],
    public: [Fp; 69],
}
impl ReceiptFinalityCircuit {
    /// Witnessless original layout for the fixed anchor and exact two child keys.
    /// Child order is complete history first and complete receipt inclusion second.
    /// # Errors
    /// Invalid fixed parameter or filler encodings.
    pub fn for_source(anchor: HistoryAnchor, plan: SourcePairPlan) -> Result<Self, Error> {
        let history_key = plan.source(0).ok_or(Error::Synthesis)?.key_digest()?;
        Ok(Self {
            anchor,
            history_key,
            pair: SourcePairWitness::for_source(plan)?,
            input: ReceiptFinalityInput {
                terminal: HistoryState::genesis(&anchor),
                load: LoadSourceContext {
                    result_root: Fp::ZERO,
                    result_frame_len: 0,
                    receipt: LoadReceiptProjection::default(),
                    event_root: [0; 32],
                    event_count: 0,
                    event_index: 0,
                },
                receipt: [0; LoadReceiptCells::BYTES],
            },
            endpoints: [Fp::ZERO; 6],
            public: [Fp::ZERO; 69],
        })
    }
    /// Verify both original sources and prepare their complete carried claims.
    /// The exact receipt/history/event links are independently constrained.
    /// # Errors
    /// Wrong endpoints, invalid source proof/claim/key or bounded resource refusal.
    pub fn prepare(
        anchor: HistoryAnchor,
        plan: SourcePairPlan,
        input: &ReceiptFinalityInput,
        children: [SourceNodeEvidence; 2],
        vesta: &PinnedParams<Eq>,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<Self, Error> {
        let history_key = plan.source(0).ok_or(Error::Synthesis)?.key_digest()?;
        if children.each_ref().map(|child| child.endpoints)
            != input.source_endpoints(&anchor, history_key)
        {
            return Err(Error::Synthesis);
        }
        let context = input.digest(&anchor);
        let endpoints = [
            Fp::from(PROGRAM_ID),
            context,
            Fp::ZERO,
            Fp::ONE,
            Fp::ZERO,
            context,
        ];
        let pair = SourcePairWitness::prepare(plan, children, vesta, salt, config)?;
        let public = pair.frame(endpoints, vesta, config.kernel_budget)?;
        Ok(Self {
            anchor,
            history_key,
            pair,
            input: *input,
            endpoints,
            public,
        })
    }
    /// Complete independently selected root policy fixed in the original key.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }
    /// Native terminal identity prediction, independently computed by the circuit.
    pub fn digest(&self) -> Fp {
        self.input.digest(&self.anchor)
    }
    /// Complete source public frame and both retained Vesta obligations.
    pub const fn instances(&self) -> &[Fp; 69] {
        &self.public
    }
    /// Exact complete singleton interval opening.
    pub const fn endpoints(&self) -> &[Fp; 6] {
        &self.endpoints
    }
    /// Complete folded Pallas obligation committed in the source frame.
    pub const fn pallas(&self) -> &AccumulatorT<Ep> {
        &self.pair.pallas
    }
    pub(crate) fn carried_vesta(&self) -> [AccumulatorT<Eq>; 2] {
        self.pair.carried_vesta()
    }
}
impl Circuit<Fp> for ReceiptFinalityCircuit {
    type Config = ReceiptFinalityConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        let mut blank = self.clone();
        blank.pair.known = false;
        blank
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let pair = SourceMergeCircuit::configure(meta);
        let first = meta.advice_column();
        let second = meta.advice_column();
        let bytes = BytesConfig::configure(meta, first, second);
        ReceiptFinalityConfig { pair, bytes }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.pair.verifier);
        chip.load_tables(&mut layouter)?;
        let mut bytes = BytesChip::new(config.bytes);
        bytes.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "terminal ordinary receipt finality",
            |mut region| {
                let assigned = self.pair.assign(&mut chip, &mut region)?;
                let anchor = chip
                    .uint()
                    .glue()
                    .constant(&mut region, self.anchor.digest())?;
                let terminal = HistoryStateCells::assign(
                    &mut chip,
                    &mut region,
                    &self.pair.value(self.input.terminal),
                    &anchor,
                )?;
                let input = self.input.receipt.map(|b| self.pair.value(b));
                let run = bytes.run(
                    &mut region,
                    &input,
                    &LoadReceiptCells::primary_segments(),
                    &LoadReceiptCells::secondary_segments(),
                )?;
                let (mut uint, hash) = chip.uint_and_hasher()?;
                let receipt = LoadReceiptCells::from_run(&mut uint, hash, &mut region, &run)?;
                let [root] =
                    self.pair
                        .words(&mut chip, &mut region, [self.input.load.result_root])?;
                let length = chip.uint().assign::<32>(
                    &mut region,
                    self.pair
                        .value(u128::from(self.input.load.result_frame_len)),
                )?;
                let tape = ResultTape::new(&mut chip.uint(), &mut region, &root, &length)?;
                let event_root =
                    self.pair
                        .bytes(&mut chip, &mut region, self.input.load.event_root)?;
                let count = chip.uint().assign::<64>(
                    &mut region,
                    self.pair.value(u128::from(self.input.load.event_count)),
                )?;
                let index = chip.uint().assign::<32>(
                    &mut region,
                    self.pair.value(u128::from(self.input.load.event_index)),
                )?;
                let history_key = chip.uint().glue().constant(&mut region, self.history_key)?;
                let linked = ReceiptFinalityLinkCells::constrain(
                    &mut chip,
                    &mut region,
                    &anchor,
                    &history_key,
                    [&assigned.endpoints[0], &assigned.endpoints[1]],
                    &terminal,
                    ReceiptInclusionCells {
                        tape: &tape,
                        receipt: &receipt,
                        event_root: &event_root,
                        count: &count,
                        index: &index,
                    },
                )?;
                let zero = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(PROGRAM_ID),
                    linked.digest(),
                    0,
                    1,
                    &zero,
                    linked.digest(),
                )?;
                self.pair
                    .plan
                    .bind(
                        &mut chip,
                        &mut region,
                        assigned.children(),
                        &assigned.fold,
                        endpoints,
                    )?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.pair.public, i)?;
        }
        Ok(())
    }
}
