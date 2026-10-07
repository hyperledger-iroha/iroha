//! Fixed pairs of complete context-hash transitions, with no skipped padding.

use super::*;

/// Installed leaf count; semantic cursors still cover all 2,561 original steps.
pub const BATCH_LENGTH: u32 = CRC_LEAVES / 2 + BLAKE_LEAVES.div_ceil(2);

/// Three fixed source shapes covering the complete original program exactly once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContextHashBatchPlan {
    /// Two consecutive 32-byte CRC transitions, including constrained padding.
    #[default]
    CrcPair,
    /// Two consecutive 128-byte Blake transitions, including constrained padding.
    BlakePair,
    /// The final single Blake transition of the odd-length phase.
    BlakeTail,
}
impl ContextHashBatchPlan {
    /// Select the immutable source class from an installed batch ordinal.
    pub const fn at(position: u32) -> Option<Self> {
        if position >= BATCH_LENGTH {
            None
        } else if position < CRC_LEAVES / 2 {
            Some(Self::CrcPair)
        } else if position + 1 < BATCH_LENGTH {
            Some(Self::BlakePair)
        } else if position + 1 == BATCH_LENGTH {
            Some(Self::BlakeTail)
        } else {
            None
        }
    }
    const fn phase(self) -> ContextHashPhase {
        match self {
            Self::CrcPair => ContextHashPhase::Crc,
            Self::BlakePair | Self::BlakeTail => ContextHashPhase::Blake,
        }
    }
    const fn count(self) -> usize {
        match self {
            Self::CrcPair | Self::BlakePair => 2,
            Self::BlakeTail => 1,
        }
    }
}

/// A compiled span of one or two original transitions over all live registers.
/// This is an arithmetic source, never standalone finality or monetary authority.
#[derive(Clone, Debug)]
pub struct ContextHashBatchCircuit {
    plan: ContextHashBatchPlan,
    leaves: Vec<ContextHashLeafCircuit>,
}
impl ContextHashBatchCircuit {
    /// Reconstruct the exact unknown source without proof or acceptance claims.
    pub fn for_source(plan: ContextHashBatchPlan) -> Self {
        let start = match plan {
            ContextHashBatchPlan::CrcPair => 0,
            ContextHashBatchPlan::BlakePair => CRC_LEAVES,
            ContextHashBatchPlan::BlakeTail => PROGRAM_LENGTH - 1,
        };
        Self {
            plan,
            leaves: (0..plan.count())
                .map(|i| {
                    let mut leaf = ContextHashLeafCircuit::for_source(plan.phase());
                    leaf.cursor = start + u32::try_from(i).expect("fixed pair index");
                    leaf
                })
                .collect(),
        }
    }
    fn new(plan: ContextHashBatchPlan, leaves: Vec<ContextHashLeafCircuit>) -> Result<Self, Error> {
        if leaves.len() != plan.count() || leaves.iter().any(|leaf| leaf.phase != plan.phase()) {
            return Err(Error::Synthesis);
        }
        for pair in leaves.windows(2) {
            let left = pair[0].endpoints();
            let right = pair[1].endpoints();
            if left[..2] != right[..2] || left[3] != right[2] || left[5] != right[4] {
                return Err(Error::Synthesis);
            }
        }
        Ok(Self { plan, leaves })
    }
    /// Exact original context, still unauthenticated until complete source closure.
    pub fn input(&self) -> &ContextHashInput {
        self.leaves[0].input()
    }
    /// Full outer interval endpoints; intermediate states remain circuit constrained.
    pub fn endpoints(&self) -> [Fp; 6] {
        let first = self.leaves[0].endpoints();
        let last = self.leaves[self.leaves.len() - 1].endpoints();
        [first[0], first[1], first[2], last[3], first[4], last[5]]
    }
    /// Exact 69-word arithmetic-source frame with deciding empty obligations.
    /// # Errors
    /// Invalid endpoint geometry or pinned filler encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
}

/// Fixed source configuration; witnesses cannot select a phase or a span length.
#[derive(Clone, Debug)]
pub struct ContextHashBatchConfig {
    inner: ContextHashConfig,
    plan: ContextHashBatchPlan,
}
impl Circuit<Fp> for ContextHashBatchCircuit {
    type Config = ContextHashBatchConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ContextHashBatchPlan;
    fn params(&self) -> Self::Params {
        self.plan
    }
    fn without_witnesses(&self) -> Self {
        Self {
            plan: self.plan,
            leaves: self.leaves.iter().map(Circuit::without_witnesses).collect(),
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        Self::configure_with_params(meta, ContextHashBatchPlan::CrcPair)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, plan: Self::Params) -> Self::Config {
        ContextHashBatchConfig {
            inner: ContextHashLeafCircuit::configure(meta),
            plan,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        if self.plan != config.plan
            || self.leaves.len() != self.plan.count()
            || self
                .leaves
                .iter()
                .any(|leaf| leaf.phase != self.plan.phase())
        {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::new(config.inner.verifier);
        chip.load_tables(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.inner.blake);
        let frame = layouter.assign_region(
            || "complete adjacent context transitions",
            |mut region| {
                let mut joined: Option<SourceEndpoints> = None;
                for leaf in &self.leaves {
                    let next = leaf.assign_transition(&mut chip, &mut blake, &mut region)?;
                    joined = Some(if let Some(previous) = joined {
                        let left = previous.words();
                        let right = next.words();
                        for (a, b) in [(0, 0), (1, 1), (3, 2), (5, 4)] {
                            GlueChip::assert_equal(&mut region, &left[a], &right[b])?;
                        }
                        SourceEndpoints::from_words(
                            &mut chip,
                            &mut region,
                            &[
                                left[0].clone(),
                                left[1].clone(),
                                left[2].clone(),
                                right[3].clone(),
                                left[4].clone(),
                                right[5].clone(),
                            ],
                        )?
                    } else {
                        next
                    });
                }
                SourceCheckpoint::leaf(&mut chip, &mut region, joined.ok_or(Error::Synthesis)?)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (row, word) in frame.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.inner.public, row)?;
        }
        Ok(())
    }
}

/// Prepare every original instruction and group only fixed adjacent pairs.
/// No active or padding instruction is omitted according to payload length.
/// # Errors
/// Invalid original frame/hash, source interval or phase continuity.
pub fn prepare_context_batches(
    frame: Vec<u8>,
    start: u32,
    len: u32,
    id: [u8; 32],
) -> Result<Vec<ContextHashBatchCircuit>, Error> {
    let mut leaves = prepare_context_hash(frame, start, len, id)?.into_iter();
    let mut batches = Vec::with_capacity(BATCH_LENGTH as usize);
    for position in 0..BATCH_LENGTH {
        let plan = ContextHashBatchPlan::at(position).ok_or(Error::Synthesis)?;
        let group = leaves.by_ref().take(plan.count()).collect();
        batches.push(ContextHashBatchCircuit::new(plan, group)?);
    }
    if leaves.next().is_some() {
        return Err(Error::Synthesis);
    }
    Ok(batches)
}

#[cfg(test)]
mod tests;
