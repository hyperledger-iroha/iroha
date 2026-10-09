//! Fixed pairs of complete BLS transitions, preserving every original cursor.

use super::*;

/// Exact installed cursor pair, selected solely by its fixed proof ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlsBatchPlan {
    ordinal: u32,
    leaves: [BlsLeafPlan; 2],
}
impl BlsBatchPlan {
    /// Installed proof leaves; the semantic program still has 1,084 steps.
    pub const LENGTH: u32 = BlsLeafPlan::LENGTH / 2;
    /// Select one complete adjacent pair from the immutable program schedule.
    pub fn at(ordinal: u32) -> Option<Self> {
        if ordinal >= Self::LENGTH {
            return None;
        }
        let cursor = 2 * ordinal;
        Some(Self {
            ordinal,
            leaves: [BlsLeafPlan::at(cursor)?, BlsLeafPlan::at(cursor + 1)?],
        })
    }
    /// Installed proof ordinal, distinct from the semantic instruction cursor.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
    fn needs_sha(self) -> bool {
        self.leaves.iter().any(|plan| plan.needs_sha())
    }
}
impl Default for BlsBatchPlan {
    fn default() -> Self {
        Self::at(0).expect("fixed initial BLS pair")
    }
}

/// Two complete original BLS operations with their intermediate state constrained.
/// This circuit supplies interval evidence and never standalone finality authority.
#[derive(Clone, Debug)]
pub struct BlsBatchCircuit {
    plan: BlsBatchPlan,
    leaves: [BlsLeafCircuit; 2],
}
impl BlsBatchCircuit {
    /// Build the exact unknown source without a signature or acceptance verdict.
    /// # Errors
    /// Inconsistent internal phase/register shapes.
    pub fn for_source(plan: BlsBatchPlan) -> Result<Self, Error> {
        Ok(Self {
            plan,
            leaves: [
                BlsLeafCircuit::for_source(plan.leaves[0])?,
                BlsLeafCircuit::for_source(plan.leaves[1])?,
            ],
        })
    }
    fn new(plan: BlsBatchPlan, leaves: [BlsLeafCircuit; 2]) -> Result<Self, Error> {
        if leaves[0].plan != plan.leaves[0] || leaves[1].plan != plan.leaves[1] {
            return Err(Error::Synthesis);
        }
        let left = leaves[0].endpoints();
        let right = leaves[1].endpoints();
        if left[..2] != right[..2] || left[3] != right[2] || left[5] != right[4] {
            return Err(Error::Synthesis);
        }
        Ok(Self { plan, leaves })
    }
    /// Exact immutable cursor pair used for source key qualification.
    pub const fn plan(&self) -> BlsBatchPlan {
        self.plan
    }
    /// Original semantic interval, retaining full initial and final live states.
    pub fn endpoints(&self) -> [Fp; 6] {
        let first = self.leaves[0].endpoints();
        let last = self.leaves[1].endpoints();
        [first[0], first[1], first[2], last[3], first[4], last[5]]
    }
    /// Exact 69-word source frame with both deciding empty carry obligations.
    /// # Errors
    /// Invalid endpoint geometry or pinned filler encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
}

/// Fixed cursor-pair source layout, including SHA lanes only where required.
#[derive(Clone, Debug)]
pub struct BlsBatchConfig {
    verifier: VerifierConfig<Ep>,
    sha: Option<Sha256Config>,
    public: Column<Instance>,
    plan: BlsBatchPlan,
}
impl Circuit<Fp> for BlsBatchCircuit {
    type Config = BlsBatchConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = BlsBatchPlan;
    fn params(&self) -> Self::Params {
        self.plan
    }
    fn without_witnesses(&self) -> Self {
        Self {
            plan: self.plan,
            leaves: self.leaves.each_ref().map(Circuit::without_witnesses),
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        Self::configure_with_params(meta, BlsBatchPlan::default())
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, plan: Self::Params) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed three-bank source profile");
        let sha = plan.needs_sha().then(|| {
            let advice = core::array::from_fn(|_| meta.advice_column());
            let constants = meta.fixed_column();
            Sha256Config::configure(meta, advice, constants)
        });
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        BlsBatchConfig {
            verifier,
            sha,
            public,
            plan,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        if self.plan != config.plan
            || self.leaves[0].plan != self.plan.leaves[0]
            || self.leaves[1].plan != self.plan.leaves[1]
        {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let mut sha = config.sha.as_ref().map(Sha256Chip::new);
        if let Some(sha) = sha.as_mut() {
            sha.load_table(&mut layouter)?;
        }
        let frame = layouter.assign_region(
            || "complete fixed BLS cursor pair",
            |mut region| {
                let first =
                    self.leaves[0].assign_transition(&mut chip, sha.as_mut(), &mut region)?;
                let last =
                    self.leaves[1].assign_transition(&mut chip, sha.as_mut(), &mut region)?;
                let left = first.words();
                let right = last.words();
                for (a, b) in [(0, 0), (1, 1), (3, 2), (5, 4)] {
                    GlueChip::assert_equal(&mut region, &left[a], &right[b])?;
                }
                let endpoints = SourceEndpoints::from_words(
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
                )?;
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (row, word) in frame.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}

/// Prepare every original BLS operation and group the complete trace into pairs.
/// Native intermediate values confer no verification or installation authority.
/// # Errors
/// Original encoding/arithmetic errors or a discontinuous internal program trace.
pub fn prepare_bls_batches(
    message: [u8; 165],
    public_key: [u8; 48],
    signature: [u8; 96],
) -> Result<Vec<BlsBatchCircuit>, BlsWitnessError> {
    let mut original = prepare_bls_witness(message, public_key, signature)?.into_iter();
    let mut batches = Vec::with_capacity(BlsBatchPlan::LENGTH as usize);
    for position in 0..BlsBatchPlan::LENGTH {
        let plan = BlsBatchPlan::at(position).ok_or(BlsWitnessError::Program)?;
        let leaves = [
            original.next().ok_or(BlsWitnessError::Program)?,
            original.next().ok_or(BlsWitnessError::Program)?,
        ];
        batches.push(BlsBatchCircuit::new(plan, leaves).map_err(|_| BlsWitnessError::Program)?);
    }
    if original.next().is_some() {
        return Err(BlsWitnessError::Program);
    }
    Ok(batches)
}

#[cfg(test)]
mod tests;
