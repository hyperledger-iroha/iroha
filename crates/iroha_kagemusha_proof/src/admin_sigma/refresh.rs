//! One fixed tag7 leaf for all five refresh kinds; signed originals belong to A.

use ff::Field;
use iroha_plonk_gadgets::{Bit, Word};

use super::*;
use crate::{
    operation_relation::{state::rest_index as rest, statement::RefreshClassStatementCells},
    witness::core_index as core,
};

/// Refresh state openings and the update projections authenticated separately by A.
#[derive(Clone, Copy, Debug)]
pub struct RefreshWitness {
    /// Original committed state and its adjusted lineage.
    pub predecessor: StateWitness,
    /// Successor state and unchanged adjusted lineage values.
    pub successor: StateWitness,
    /// Exact tag7 statement; effect kind is constrained to 1 through 5.
    pub statement: [Fp; 26],
    /// Signed update time, or predecessor floor for a scheme policy.
    pub issued: Fp,
    /// Scheme-policy controls before permission intersection; zero for other kinds.
    pub policy_controls: Fp,
}

/// Shared `RefreshPolicy` sigma class with a witness-independent circuit/key shape.
/// A binds every update projection to the signed original and proves the history
/// insertion or fixed64 quota rebuild. This leaf cannot authenticate an update.
#[derive(Clone, Copy, Debug)]
pub struct RefreshCircuit {
    witness: RefreshWitness,
    known: bool,
}
impl RefreshCircuit {
    /// Carry the proposed state and exact statement to the shared relation.
    #[must_use]
    pub const fn new(witness: &RefreshWitness) -> Self {
        Self {
            witness: *witness,
            known: true,
        }
    }
    /// One bounded statement digest for canonical sigma selector14.
    #[must_use]
    pub fn instances(&self) -> [Vec<Fp>; 1] {
        [vec![hash_with_domain(
            STATEMENT_DOMAIN,
            &self.witness.statement,
        )]]
    }
    /// Exact homogeneous PIPA-R public-input type.
    #[must_use]
    pub const fn instance_types() -> [InstanceType; 1] {
        [InstanceType::Bounded]
    }
}
impl Circuit<Fp> for RefreshCircuit {
    type Config = AdminConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..*self
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> AdminConfig {
        BootstrapCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: AdminConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::starting_at(config.glue, load::BASE_HASH_ROWS);
        let mut range = RunningSumChip::new(config.range);
        let mut sponge = SpongeChip::new(config.sponge);
        range.load_table(&mut layouter)?;
        let digest = layouter.assign_region(
            || "shared five-kind RefreshPolicy sigma",
            |mut region| {
                let w = &self.witness;
                let projections = [w.issued, w.policy_controls];
                let values = w
                    .predecessor
                    .core
                    .iter()
                    .chain(&w.predecessor.rest)
                    .chain(&w.predecessor.lineage)
                    .chain(&w.successor.core)
                    .chain(&w.successor.rest)
                    .chain(&w.successor.lineage)
                    .chain(&w.statement)
                    .chain(&projections)
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
                let n = load::STATE_WORDS;
                let (before, previous) =
                    load::state(&mut uint, &mut sponge, &mut region, &words[..n])?;
                let (after, lineage) =
                    load::state(&mut uint, &mut sponge, &mut region, &words[n..2 * n])?;
                let statement = RefreshClassStatementCells::constrain(
                    &mut uint,
                    &mut sponge,
                    &mut region,
                    &::core::array::from_fn(|i| words[2 * n + i].clone()),
                )?;
                statement.bind_states(
                    &mut uint,
                    &mut region,
                    (&before, &previous),
                    &after,
                    &lineage,
                )?;
                effects(
                    &mut uint,
                    &mut region,
                    &statement,
                    &before,
                    &after,
                    [&words[2 * n + 26], &words[2 * n + 27]],
                )?;
                for i in [14, 15, 16] {
                    GlueChip::assert_equal(
                        &mut region,
                        &previous.fields()[i],
                        &lineage.fields()[i],
                    )?;
                }
                if sponge.lane().rows_used() != load::BASE_HASH_ROWS {
                    return Err(Error::Synthesis);
                }
                Ok(statement.digest().clone())
            },
        )?;
        layouter.constrain_instance(digest.cell(), config.public, 0)
    }
}

fn equal_if(
    uint: &mut UintChip<'_, Fp>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    selected: &Word<Fp>,
    a: &Word<Fp>,
    b: &Word<Fp>,
) -> Result<(), Error> {
    let difference = uint.glue().sub(region, a, b)?;
    let mismatch = uint.glue().mul(region, selected, &difference)?;
    GlueChip::assert_constant(region, &mismatch, Fp::ZERO)
}

fn require_if(
    uint: &mut UintChip<'_, Fp>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    selected: &Bit<Fp>,
    valid: &Bit<Fp>,
) -> Result<(), Error> {
    let invalid = uint.glue().not(region, valid)?;
    let failure = uint.glue().mul(region, selected.word(), invalid.word())?;
    GlueChip::assert_constant(region, &failure, Fp::ZERO)
}

fn effects(
    uint: &mut UintChip<'_, Fp>,
    region: &mut iroha_plonk::frontend::Region<'_, Fp>,
    statement: &RefreshClassStatementCells,
    before: &StateCells,
    after: &StateCells,
    projections: [&Word<Fp>; 2],
) -> Result<(), Error> {
    let kinds = statement.kinds();
    let b = before.core();
    let a = after.core();
    let br = before.rest();
    let ar = after.rest();
    let [issued, policy_controls] = projections;
    let zero = uint.glue().constant(region, Fp::ZERO)?;
    let one = uint.glue().constant(region, Fp::ONE)?;
    for (kind, target) in [
        &a[core::CREDENTIAL],
        &ar[rest::SCHEME_POLICY],
        &ar[rest::BLACKLIST],
        &ar[rest::QUOTA_SHARE],
        &ar[rest::TIME_ANCHOR],
    ]
    .iter()
    .enumerate()
    {
        equal_if(
            uint,
            region,
            kinds[kind].word(),
            target,
            &statement.fields()[18],
        )?;
    }
    for (kind, old, new) in [
        (1, &b[core::POLICY_EPOCH], &a[core::POLICY_EPOCH]),
        (2, &b[core::BLACKLIST_VERSION], &a[core::BLACKLIST_VERSION]),
        (3, &br[rest::QUOTA_SHARE_ID], &ar[rest::QUOTA_SHARE_ID]),
    ] {
        let old = uint.range_check::<64>(region, old)?;
        let new = uint.range_check::<64>(region, new)?;
        let increasing = uint.lt(region, &old, &new)?;
        require_if(uint, region, &kinds[kind], &increasing)?;
    }
    let unchanged_anchor =
        uint.glue()
            .is_equal(region, &br[rest::TIME_ANCHOR], &ar[rest::TIME_ANCHOR])?;
    let changed_anchor = uint.glue().not(region, &unchanged_anchor)?;
    require_if(uint, region, &kinds[4], &changed_anchor)?;
    equal_if(uint, region, kinds[1].word(), issued, &b[core::TIME_FLOOR])?;
    equal_if(
        uint,
        region,
        kinds[2].word(),
        issued,
        &a[core::BLACKLIST_ISSUED_AT],
    )?;
    let issued = uint.range_check::<64>(region, issued)?;
    let floor = uint.range_check::<64>(region, &b[core::TIME_FLOOR])?;
    let advances = uint.lt(region, &floor, &issued)?;
    let accepted = uint
        .glue()
        .select(region, &advances, issued.word(), floor.word())?;
    GlueChip::assert_equal(region, &accepted, &a[core::TIME_FLOOR])?;
    GlueChip::assert_equal(region, &accepted, &statement.fields()[19])?;
    let expires = uint.range_check::<64>(region, &a[core::QUOTA_SHARE_EXPIRY])?;
    let valid_expiry = uint.lt(region, &issued, &expires)?;
    require_if(uint, region, &kinds[3], &valid_expiry)?;
    let policy = crate::operation_relation::refresh::mask(uint, region, policy_controls)?;
    let permitted = crate::operation_relation::refresh::mask(uint, region, &br[rest::PERMITTED])?;
    let mut enabled = Vec::with_capacity(3);
    for i in 0..3 {
        enabled.push(
            uint.glue()
                .mul(region, policy[i].word(), permitted[i].word())?,
        );
    }
    let intersection = uint.glue().linear(
        region,
        &[
            (Fp::ONE, &enabled[0]),
            (Fp::from(2), &enabled[1]),
            (Fp::from(4), &enabled[2]),
        ],
        Fp::ZERO,
    )?;
    equal_if(
        uint,
        region,
        kinds[1].word(),
        &intersection,
        &a[core::ENABLED_CONTROLS],
    )?;
    let other = uint.glue().not(region, &kinds[1])?;
    equal_if(uint, region, other.word(), policy_controls, &zero)?;
    for (index, old) in b.iter().enumerate() {
        if matches!(index, core::SEQUENCE | core::STATE_NONCE | core::TIME_FLOOR) {
            continue;
        }
        let kind = match index {
            core::CREDENTIAL | core::LEASE_EXPIRY => Some(0),
            core::POLICY_EPOCH | core::ENABLED_CONTROLS => Some(1),
            core::BLACKLIST_VERSION | core::BLACKLIST_ROOT | core::BLACKLIST_ISSUED_AT => Some(2),
            core::QUOTA_WINDOWS_ROOT | core::QUOTA_SHARE_EXPIRY | core::QUOTA_USAGE_ROOT => Some(3),
            _ => None,
        };
        let preserve = if let Some(kind) = kind {
            uint.glue().not(region, &kinds[kind])?.word().clone()
        } else {
            one.clone()
        };
        equal_if(uint, region, &preserve, old, &a[index])?;
    }
    for (index, old) in br.iter().enumerate() {
        let kind = match index {
            rest::SCHEME_POLICY | rest::FEE_SCHEDULE => Some(1),
            rest::BLACKLIST | rest::BLACKLIST_HISTORY => Some(2),
            rest::QUOTA_SHARE | rest::QUOTA_SHARE_ID => Some(3),
            rest::TIME_ANCHOR => Some(4),
            _ => None,
        };
        let preserve = if let Some(kind) = kind {
            uint.glue().not(region, &kinds[kind])?.word().clone()
        } else {
            one.clone()
        };
        equal_if(uint, region, &preserve, old, &ar[index])?;
    }
    Ok(())
}
