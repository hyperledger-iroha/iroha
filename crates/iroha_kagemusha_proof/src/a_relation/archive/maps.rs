//! Terminal Archive mode rule and both authenticated pending removals.

use iroha_pasta::{Ep, Fp};
use iroha_plonk::frontend::{Error, Region};
use iroha_plonk_gadgets::{Bit, GlueChip};
use iroha_plonk_recursion::verifier::VerifierChip;

use super::{
    results::ArchiveResultClaims,
    retained::{DESCRIPTOR_SLOT, DESCRIPTOR_SPEC},
};
use crate::{
    a_relation::context::{ContextInputs, ContextObjectCells, ContextPlan},
    operation_relation::map_effects::{ArchiveMapWitness, MapEffectsChip, MapState, MapTransition},
};

/// Terminal evidence mode verdict after constraining both pending-root effects.
/// It does not establish execution of the other owners or final Omega decision.
#[must_use = "close every recursive obligation and durably commit Omega before deletion"]
#[derive(Clone, Debug)]
pub struct ArchiveEffectsCells {
    valid: Bit<Fp>,
}
impl ArchiveEffectsCells {
    /// Complete three-result iff verdict used by these exact map effects.
    pub const fn valid(&self) -> &Bit<Fp> {
        &self.valid
    }
}

/// Apply the fixed complete result/mode rule and authenticate both removals.
/// All descriptor and path constraints execute even when evidence is false;
/// only the adjusted pending root selects preservation on the no-op branch.
/// # Errors
/// Wrong owner/descriptor schema, spliced retained descriptor, missing results
/// or modes, invalid paths, discretionary no-op or changed unrelated state.
pub fn constrain_effects(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    plan: &ContextPlan,
    stage: u32,
    input: &ContextInputs<'_>,
    claims: &ArchiveResultClaims,
    witness: &ArchiveMapWitness,
) -> Result<ArchiveEffectsCells, Error> {
    let valid = claims.terminal_modes(chip, region, plan, stage, input)?;
    let spec = *plan
        .object_specs()
        .get(DESCRIPTOR_SLOT)
        .ok_or(Error::Synthesis)?;
    if spec != DESCRIPTOR_SPEC {
        return Err(Error::Synthesis);
    }
    let actual = ContextObjectCells::from_internal_words(chip, region, spec, &witness.descriptor)?;
    let expected = input.objects.get(DESCRIPTOR_SLOT).ok_or(Error::Synthesis)?;
    for (a, b) in actual
        .commitment_words()
        .iter()
        .zip(expected.commitment_words())
    {
        GlueChip::assert_equal(region, a, &b)?;
    }
    let predecessor = input.predecessor.ok_or(Error::Synthesis)?;
    let transition = MapTransition {
        statement: input.own_statement,
        predecessor: MapState {
            state: predecessor.state,
            lineage: predecessor.public,
        },
        successor: MapState {
            state: input.successor.state,
            lineage: input.successor.public,
        },
    };
    let lanes = chip.operation_lanes()?;
    MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash).archive(
        region,
        &transition,
        witness,
        &valid,
    )?;
    Ok(ArchiveEffectsCells { valid })
}
