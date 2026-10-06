//! Load's private state transition, with voucher and map authentication in A.

use iroha_plonk_gadgets::Word;

use super::*;
use crate::operation_relation::map_effects::{MapState, MapTransition};

/// Canonical G1 state opening and associated public lineage prefix.
#[derive(Clone, Copy, Debug)]
pub struct StateWitness {
    /// Exact 33-field state core.
    pub core: [Fp; CORE_FIELDS],
    /// Exact eight-field rest.
    pub rest: [Fp; REST_FIELDS],
    /// Exact 18-field lineage prefix, authenticated by A's proof ownership.
    pub lineage: [Fp; 18],
}
impl From<&BootstrapWitness> for StateWitness {
    fn from(source: &BootstrapWitness) -> Self {
        Self {
            core: source.core,
            rest: source.rest,
            lineage: source.lineage,
        }
    }
}

/// Load's original/successor openings and exact public statement preimage.
#[derive(Clone, Copy, Debug)]
pub struct LoadWitness {
    /// State opened under the hard predecessor lineage proof.
    pub predecessor: StateWitness,
    /// Newly committed state.
    pub successor: StateWitness,
    /// Exact 26-field Load statement.
    pub statement: [Fp; 26],
}

/// Load arithmetic, continuity and unchanged fields on the fixed k12 sigma class.
///
/// A authenticates the finalized voucher and its issuer, binds its exact amount,
/// ordinal and online charge, and proves the recovery-map insertion. The sigma
/// opens both heads and binds the carried recovery root; it does not replace A.
#[derive(Clone, Copy, Debug)]
pub struct LoadCircuit {
    witness: LoadWitness,
    known: bool,
}
impl LoadCircuit {
    /// Carry the complete witness to the circuit; this does not accept a Load.
    #[must_use]
    pub const fn new(witness: &LoadWitness) -> Self {
        Self {
            witness: *witness,
            known: true,
        }
    }

    /// One bounded public statement digest, computed from the original fields.
    #[must_use]
    pub fn instances(&self) -> [Vec<Fp>; 1] {
        [vec![hash_with_domain(
            STATEMENT_DOMAIN,
            &self.witness.statement,
        )]]
    }

    /// Exact instance membership for native PIPA-R key generation.
    #[must_use]
    pub const fn instance_types() -> [InstanceType; 1] {
        [InstanceType::Bounded]
    }
}
const LOAD_HASH_ROWS: usize = (2
    * (domain_permutations(CORE_FIELDS + 1, true) + domain_permutations(REST_FIELDS, true))
    + domain_permutations(26, true))
    * ROWS_PER_PERMUTATION;
const STATE_WORDS: usize = CORE_FIELDS + REST_FIELDS + 18;

fn state(
    uint: &mut UintChip<'_, Fp>,
    sponge: &mut SpongeChip<Fp>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    words: &[Word<Fp>],
) -> Result<(StateCells, LineagePublicCells), Error> {
    if words.len() != STATE_WORDS {
        return Err(Error::Synthesis);
    }
    let state = StateCells::constrain(
        uint,
        sponge,
        region,
        &core::array::from_fn(|i| words[i].clone()),
        &core::array::from_fn(|i| words[CORE_FIELDS + i].clone()),
    )?;
    let lineage = LineagePublicCells::constrain(
        uint,
        region,
        &core::array::from_fn(|i| words[CORE_FIELDS + REST_FIELDS + i].clone()),
    )?;
    Ok((state, lineage))
}
impl Circuit<Fp> for LoadCircuit {
    type Config = AdminConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..*self
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        BootstrapCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::starting_at(config.glue, LOAD_HASH_ROWS);
        let mut range = RunningSumChip::new(config.range);
        let mut sponge = SpongeChip::new(config.sponge);
        range.load_table(&mut layouter)?;
        let digest = layouter.assign_region(
            || "Load administrative sigma",
            |mut region| {
                let before = &self.witness.predecessor;
                let after = &self.witness.successor;
                let values = before
                    .core
                    .iter()
                    .chain(&before.rest)
                    .chain(&before.lineage)
                    .chain(&after.core)
                    .chain(&after.rest)
                    .chain(&after.lineage)
                    .chain(&self.witness.statement)
                    .map(|v| {
                        if self.known {
                            Value::known(*v)
                        } else {
                            Value::unknown()
                        }
                    })
                    .collect::<Vec<_>>();
                let words = glue.witnesses(&mut region, &values)?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let (before, previous) =
                    state(&mut uint, &mut sponge, &mut region, &words[..STATE_WORDS])?;
                let (after, successor) = state(
                    &mut uint,
                    &mut sponge,
                    &mut region,
                    &words[STATE_WORDS..2 * STATE_WORDS],
                )?;
                let statement = StatementCells::constrain(
                    &mut uint,
                    &mut sponge,
                    &mut region,
                    Variant::Load,
                    &core::array::from_fn(|i| words[2 * STATE_WORDS + i].clone()),
                )?;
                administrative::monetary(
                    &mut uint,
                    &mut sponge,
                    &mut region,
                    &MapTransition {
                        statement: &statement,
                        predecessor: MapState {
                            state: &before,
                            lineage: &previous,
                        },
                        successor: MapState {
                            state: &after,
                            lineage: &successor,
                        },
                    },
                )?;
                if sponge.lane().rows_used() != LOAD_HASH_ROWS {
                    return Err(Error::Synthesis);
                }
                Ok(statement.digest().clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
    }
}
