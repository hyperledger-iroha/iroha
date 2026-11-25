//! First-enrollment/first-Load candidate leaves. Both operations precede
//! credential renewal, so their prior-credential root is the fixed padding root.
//! The ordinary administrative state and monetary constraints are reused.
use super::*;
use crate::operation_relation::map_effects::{MapState, MapTransition};
use ff::Field;
use iroha_plonk::frontend::Region;

pub(crate) fn initial_prior_root() -> Fp {
    let mut root = hash_with_domain(u64::from_le_bytes(*b"kgwpcrd1"), &[Fp::ZERO]);
    for _ in 0..16 {
        root = hash_with_domain(u64::from_le_bytes(*b"kgwpcrn1"), &[root, root]);
    }
    root
}

#[derive(Clone)]
pub(crate) struct InitialAdmin<const LOAD: bool> {
    predecessor: StateWitness,
    successor: StateWitness,
    statement: [Fp; 26],
    known: bool,
}

impl InitialAdmin<false> {
    pub(crate) fn bootstrap(w: &BootstrapWitness) -> Self {
        let initial = StateWitness::from(w);
        Self {
            predecessor: initial,
            successor: initial,
            statement: w.statement,
            known: true,
        }
    }
}

impl InitialAdmin<true> {
    pub(crate) fn load(w: &LoadWitness) -> Self {
        Self {
            predecessor: w.predecessor,
            successor: w.successor,
            statement: w.statement,
            known: true,
        }
    }
}

impl<const LOAD: bool> InitialAdmin<LOAD> {
    pub(crate) fn public(&self) -> [Vec<Fp>; 1] {
        [vec![hash_with_domain(STATEMENT_DOMAIN, &self.statement)]]
    }

    fn open(
        &self,
        region: &mut Region<'_, Fp>,
        uint: &mut UintChip<'_, Fp>,
        sponge: &mut SpongeChip<Fp>,
        witness: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), Error> {
        let values = |words: &[Fp]| {
            words
                .iter()
                .map(|v| {
                    if self.known {
                        Value::known(*v)
                    } else {
                        Value::unknown()
                    }
                })
                .collect::<Vec<_>>()
        };
        let core = uint
            .glue()
            .witnesses(region, &values(&witness.core))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let rest = uint
            .glue()
            .witnesses(region, &values(&witness.rest))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let lineage = uint
            .glue()
            .witnesses(region, &values(&witness.lineage))?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let prior = uint.glue().constant(region, initial_prior_root())?;
        let state =
            StateCells::constrain_commitment(uint, sponge, region, &core, &rest, Some(prior))?;
        Ok((
            state,
            LineagePublicCells::constrain(uint, region, &lineage)?,
        ))
    }
}

impl<const LOAD: bool> Circuit<Fp> for InitialAdmin<LOAD> {
    type Config = AdminConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        configure_head(meta, CORE_FIELDS + 2)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let states = if LOAD { 2 } else { 1 };
        let rows = (states
            * (domain_permutations(CORE_FIELDS + 2, true)
                + domain_permutations(REST_FIELDS, true))
            + domain_permutations(26, true))
            * ROWS_PER_PERMUTATION;
        let mut glue = GlueChip::starting_at(config.glue, rows);
        let mut range = RunningSumChip::new(config.range);
        let mut sponge = SpongeChip::new(config.sponge);
        range.load_table(&mut layouter)?;
        let digest = layouter.assign_region(
            || "candidate initial administrative step",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let (after, successor) =
                    self.open(&mut region, &mut uint, &mut sponge, &self.successor)?;
                let values = self.statement.map(|x| {
                    if self.known {
                        Value::known(x)
                    } else {
                        Value::unknown()
                    }
                });
                let words = uint
                    .glue()
                    .witnesses(&mut region, &values)?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let variant = if LOAD {
                    Variant::Load
                } else {
                    Variant::Bootstrap
                };
                let statement = StatementCells::constrain(
                    &mut uint,
                    &mut sponge,
                    &mut region,
                    variant,
                    &words,
                )?;
                if LOAD {
                    let (before, predecessor) =
                        self.open(&mut region, &mut uint, &mut sponge, &self.predecessor)?;
                    administrative::monetary(
                        &mut uint,
                        &mut sponge,
                        &mut region,
                        &MapTransition {
                            statement: &statement,
                            predecessor: MapState {
                                state: &before,
                                lineage: &predecessor,
                            },
                            successor: MapState {
                                state: &after,
                                lineage: &successor,
                            },
                        },
                    )?;
                } else {
                    administrative::bootstrap(
                        &mut uint,
                        &mut region,
                        &statement,
                        &after,
                        &successor,
                    )?;
                }
                if sponge.lane().rows_used() != rows {
                    return Err(Error::Synthesis);
                }
                Ok(statement.digest().clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
    }
}

pub(crate) fn open_initial_state(
    chip: &mut iroha_plonk_recursion::verifier::VerifierChip<iroha_pasta::Ep>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    core: &[iroha_plonk_gadgets::Word<Fp>; 29],
    rest: &[iroha_plonk_gadgets::Word<Fp>; 6],
) -> Result<crate::operation_relation::state::StateCells, iroha_plonk::frontend::Error> {
    use iroha_plonk_gadgets::UintChip;
    let lanes = chip.operation_lanes()?;
    let mut uint = UintChip::new(lanes.glue, lanes.range);
    let prior = uint.glue().constant(region, initial_prior_root())?;
    crate::operation_relation::state::StateCells::constrain_commitment(
        &mut uint,
        lanes.hash,
        region,
        core,
        rest,
        Some(prior),
    )
}
