//! Bilateral settlement leg extraction shared by routing target collectors.

use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{
        Instruction,
        settlement::{DvpIsi, PvpIsi, SettlementInstructionBox},
    },
    nexus::DataSpaceId,
};
use std::collections::BTreeSet;

/// Return the two asset definitions carried by a direct or boxed DVP/PVP instruction.
pub(super) fn asset_definitions(
    instruction: &dyn Instruction,
) -> Option<(&AssetDefinitionId, &AssetDefinitionId)> {
    let any = instruction.as_any();
    if let Some(dvp) = any.downcast_ref::<DvpIsi>() {
        return Some((
            dvp.delivery_leg().asset_definition_id(),
            dvp.payment_leg().asset_definition_id(),
        ));
    }
    if let Some(pvp) = any.downcast_ref::<PvpIsi>() {
        return Some((
            pvp.primary_leg().asset_definition_id(),
            pvp.counter_leg().asset_definition_id(),
        ));
    }
    match any.downcast_ref::<SettlementInstructionBox>()? {
        SettlementInstructionBox::Dvp(dvp) => Some((
            dvp.delivery_leg().asset_definition_id(),
            dvp.payment_leg().asset_definition_id(),
        )),
        SettlementInstructionBox::Pvp(pvp) => Some((
            pvp.primary_leg().asset_definition_id(),
            pvp.counter_leg().asset_definition_id(),
        )),
        SettlementInstructionBox::SetFxCorridorPolicy(_)
        | SettlementInstructionBox::FundFxCorridorEscrow(_)
        | SettlementInstructionBox::RefundFxCorridorEscrow(_)
        | SettlementInstructionBox::SettleFxCorridor(_) => None,
    }
}

/// Resolve each bilateral leg independently without collapsing distinct dataspaces.
pub(super) fn concrete_dataspaces<E>(
    instruction: &dyn Instruction,
    mut resolve: impl FnMut(&AssetDefinitionId) -> Result<Option<DataSpaceId>, E>,
) -> Result<Option<BTreeSet<DataSpaceId>>, E> {
    let Some((first, second)) = asset_definitions(instruction) else {
        return Ok(None);
    };
    Ok(Some(
        [resolve(first)?, resolve(second)?]
            .into_iter()
            .flatten()
            .collect(),
    ))
}
