//! Fixed genesis start for the installed ordinary-validator history source.

use super::{HistoryAnchor, HistoryState, PREFIX_PROGRAM_ID, prefix_context, prefix_context_cells};
use crate::finality::continuity::{
    SourceCheckpoint, SourceEndpoints, SourceMergeCircuit, SourcePairConfig, leaf_frame_native,
};
use ff::Field;
use iroha_pasta::Fp;
use iroha_plonk::{
    cs::ConstraintSystem,
    frontend::{Circuit, Error, Layouter, SimpleFloorPlanner, Value},
};
use iroha_plonk_recursion::verifier::VerifierChip;

/// Exact genesis start compiled into an independently installed source key.
/// This contains no witness-selected anchor or execution-result authority.
#[derive(Clone, Copy, Debug)]
pub struct GenesisSourceCircuit {
    anchor: HistoryAnchor,
    history_key: Fp,
    known: bool,
}
impl GenesisSourceCircuit {
    /// Select fixed policy already authenticated by the installation owner.
    /// The constructor itself does not verify or authenticate a genesis block.
    pub const fn new(anchor: HistoryAnchor, history_key: Fp) -> Self {
        Self {
            anchor,
            history_key,
            known: true,
        }
    }
    /// Witnessless original source layout, independent of the future wrapper key.
    pub const fn for_source(anchor: HistoryAnchor) -> Self {
        Self {
            anchor,
            history_key: Fp::ZERO,
            known: false,
        }
    }
    /// Independently selected complete anchor.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }
    /// Exact fixed start: no predecessor, followed by the initial height-two state.
    pub fn endpoints(&self) -> [Fp; 6] {
        let digest = self.anchor.digest();
        [
            Fp::from(PREFIX_PROGRAM_ID),
            prefix_context(digest, self.history_key),
            Fp::ZERO,
            Fp::ONE,
            Fp::ZERO,
            HistoryState::genesis(&self.anchor).digest(digest),
        ]
    }
    /// Complete source frame with mandatory deciding filler claims.
    /// # Errors
    /// Invalid pinned filler encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
}
impl Circuit<Fp> for GenesisSourceCircuit {
    type Config = SourcePairConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..*self
        }
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
            || "installed signed genesis start",
            |mut region| {
                let values = self.endpoints();
                let anchor = chip
                    .uint()
                    .glue()
                    .constant(&mut region, self.anchor.digest())?;
                let key = chip.uint().glue().witness(
                    &mut region,
                    if self.known {
                        Value::known(self.history_key)
                    } else {
                        Value::unknown()
                    },
                )?;
                let context = prefix_context_cells(&mut chip, &mut region, &anchor, &key)?;
                let before = chip.uint().glue().constant(&mut region, values[4])?;
                let after = chip.uint().glue().constant(&mut region, values[5])?;
                let endpoints = SourceEndpoints::leaf(
                    &mut chip,
                    &mut region,
                    Fp::from(PREFIX_PROGRAM_ID),
                    &context,
                    0,
                    1,
                    &before,
                    &after,
                )?;
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
