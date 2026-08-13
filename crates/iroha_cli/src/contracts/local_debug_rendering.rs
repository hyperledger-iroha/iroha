//! Rendering helpers for local contract-debug responses.
use std::collections::BTreeMap;
use eyre::{Result, WrapErr as _};
use iroha::data_model::{isi::InstructionBox, prelude::StatePath};
use super::{LocalContractDebugEntrypoint, LocalContractDebugParam};
/// Build the stable JSON-facing entrypoint description for a local execution.
pub(super) fn build_local_debug_entrypoint(
    descriptor: &ivm::EmbeddedEntrypointDescriptor,
    entrypoint_pc: u64,
) -> LocalContractDebugEntrypoint {
    LocalContractDebugEntrypoint {
        name: descriptor.name.clone(),
        kind: format!("{:?}", descriptor.kind),
        pc: entrypoint_pc,
        return_type: descriptor.return_type.clone(),
        params: descriptor
            .params
            .iter()
            .map(|param| LocalContractDebugParam {
                name: param.name.clone(),
                type_name: param.type_name.clone(),
            })
            .collect(),
    }
}
/// Serialize queued instructions without changing their canonical JSON shape.
pub(super) fn render_queued_instructions(queued: &[InstructionBox]) -> Result<norito::json::Value> {
    let values = queued
        .iter()
        .map(norito::json::to_value)
        .collect::<Result<Vec<_>, _>>()
        .wrap_err("failed to serialize queued instructions")?;
    Ok(norito::json::Value::Array(values))
}
/// Render the durable-state overlay using canonical state paths and hex values.
pub(super) fn render_durable_state_overlay(
    overlay: &BTreeMap<StatePath, Option<Vec<u8>>>,
) -> Result<norito::json::Value> {
    let mut object = norito::json::Map::new();
    for (path, value) in overlay {
        object.insert(
            path.as_ref().to_owned(),
            value.as_ref().map_or(norito::json::Value::Null, |bytes| {
                norito::json::Value::from(format!("0x{}", hex::encode(bytes)))
            }),
        );
    }
    Ok(norito::json::Value::Object(object))
}
