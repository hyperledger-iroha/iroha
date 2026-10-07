//! Exact full-stream pairs of result hashing operations, including constrained padding.

use super::*;

/// Installed leaf count; semantic program endpoints remain exactly zero through 515.
pub const RESULT_BATCH_LENGTH: u32 = RESULT_SCAN_LEAVES.div_ceil(2);

/// Three fixed source classes covering the complete original scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResultScanBatchPlan {
    /// Initialization followed by the first actual compression slot.
    #[default]
    StartAbsorb,
    /// Two adjacent compression slots, including unchanged completed padding.
    AbsorbPair,
    /// The final exact digest and complete consumption owner.
    Finish,
}
impl ResultScanBatchPlan {
    /// Select the fixed source class from the installed proof ordinal.
    pub const fn at(ordinal: u32) -> Option<Self> {
        if ordinal >= RESULT_BATCH_LENGTH {
            None
        } else if ordinal == 0 {
            Some(Self::StartAbsorb)
        } else if ordinal + 1 == RESULT_BATCH_LENGTH {
            Some(Self::Finish)
        } else {
            Some(Self::AbsorbPair)
        }
    }
    const fn operations(self) -> &'static [ResultScanPlan] {
        match self {
            Self::StartAbsorb => &[ResultScanPlan::Start, ResultScanPlan::Absorb],
            Self::AbsorbPair => &[ResultScanPlan::Absorb, ResultScanPlan::Absorb],
            Self::Finish => &[ResultScanPlan::Finish],
        }
    }
}

/// One or two complete original transitions with every intermediate register bound.
/// This source authenticates a tape interval; it supplies no consensus authority.
#[derive(Clone, Debug)]
pub struct ResultScanBatchCircuit {
    plan: ResultScanBatchPlan,
    leaves: Vec<ResultScanCircuit>,
}
impl ResultScanBatchCircuit {
    /// Construct the exact unknown source for original-table import.
    /// # Errors
    /// Failed bounded tape construction.
    pub fn for_source(plan: ResultScanBatchPlan) -> Result<Self, Error> {
        let start = match plan {
            ResultScanBatchPlan::StartAbsorb => 0,
            ResultScanBatchPlan::AbsorbPair => 2,
            ResultScanBatchPlan::Finish => RESULT_SCAN_LEAVES - 1,
        };
        let leaves = plan
            .operations()
            .iter()
            .enumerate()
            .map(|(index, op)| {
                let mut leaf = ResultScanCircuit::for_source(*op)?;
                leaf.cursor = start + u32::try_from(index).map_err(|_| Error::Synthesis)?;
                Ok(leaf)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self { plan, leaves })
    }
    fn new(plan: ResultScanBatchPlan, leaves: Vec<ResultScanCircuit>) -> Result<Self, Error> {
        if leaves.len() != plan.operations().len()
            || leaves
                .iter()
                .zip(plan.operations())
                .any(|(leaf, op)| leaf.plan != *op)
        {
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
    /// Untrusted original context; semantic composition must authenticate it.
    pub fn context(&self) -> &ResultScanContext {
        self.leaves[0].context()
    }
    /// Complete original first-to-last semantic interval and boundary state digests.
    pub fn endpoints(&self) -> [Fp; 6] {
        let first = self.leaves[0].endpoints();
        let last = self.leaves[self.leaves.len() - 1].endpoints();
        [first[0], first[1], first[2], last[3], first[4], last[5]]
    }
    /// Exact 69-word arithmetic source frame with both empty carry obligations.
    /// # Errors
    /// Invalid endpoint geometry or pinned filler encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
    pub(crate) fn proposed_digest(&self) -> Result<[u8; 32], Error> {
        if self.plan != ResultScanBatchPlan::Finish || self.leaves.len() != 1 {
            return Err(Error::Synthesis);
        }
        self.leaves[0].proposed_digest()
    }
}

/// Fixed complete-transition source configuration.
#[derive(Clone, Debug)]
pub struct ResultScanBatchConfig {
    inner: ResultScanConfig,
    plan: ResultScanBatchPlan,
}
impl Circuit<Fp> for ResultScanBatchCircuit {
    type Config = ResultScanBatchConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ResultScanBatchPlan;
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
        Self::configure_with_params(meta, ResultScanBatchPlan::default())
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, plan: Self::Params) -> Self::Config {
        Self::Config {
            inner: ResultScanCircuit::configure(meta),
            plan,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        if self.plan != config.plan
            || self.leaves.len() != self.plan.operations().len()
            || self
                .leaves
                .iter()
                .zip(self.plan.operations())
                .any(|(leaf, op)| leaf.plan != *op)
        {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::new(config.inner.verifier);
        chip.load_tables(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.inner.blake);
        let frame = layouter.assign_region(
            || "complete adjacent result scan transitions",
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
        for (index, word) in frame.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.inner.public, index)?;
        }
        Ok(())
    }
}

/// Prepare every original scan operation and group its complete fixed instruction stream.
/// A wrong expected digest is retained for the mandatory final equality to reject.
/// # Errors
/// Original size/tape errors or inconsistent internal operation continuity.
pub fn prepare_result_batches(
    frame: &[u8],
    expected: [u8; 32],
) -> Result<Vec<ResultScanBatchCircuit>, Error> {
    let mut original = prepare_result_scan(frame, expected)?.into_iter();
    let mut batches = Vec::with_capacity(RESULT_BATCH_LENGTH as usize);
    for ordinal in 0..RESULT_BATCH_LENGTH {
        let plan = ResultScanBatchPlan::at(ordinal).ok_or(Error::Synthesis)?;
        let leaves = (0..plan.operations().len())
            .map(|_| original.next().ok_or(Error::Synthesis))
            .collect::<Result<Vec<_>, Error>>()?;
        batches.push(ResultScanBatchCircuit::new(plan, leaves)?);
    }
    if original.next().is_some() {
        return Err(Error::Synthesis);
    }
    Ok(batches)
}

#[cfg(test)]
mod tests;
