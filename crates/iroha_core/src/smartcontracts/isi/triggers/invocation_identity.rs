//! Fresh store-owned trigger action binding, without execution or live authority.
//!
//! Canonical frames of persistent model values are streamed into a semantic
//! preimage. No LoadedAction/Core-schema, cache representation or ephemeral
//! registration generation is encoded. Callback dispatch remains unchanged.
//! TODO: connect this helper only within the actual exclusive invocation owner,
//! after bounded resource reservation and immediately before matching/execution.

use std::io::{self, Write};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{block::execution_output::TriggerUseV1, prelude::*};
use mv::storage::StorageReadOnly;

use super::{ExecutableRef, SetReadOnly};
use crate::smartcontracts::isi::triggers::{
    specialized::LoadedAction, trigger_registered_block_height_metadata_key,
};

const ACTION_DOMAIN: &[u8] = b"iroha:trigger-use:action:v1\0";

/// Reload a pipeline action from its exact typed store and bind persistent content.
///
/// This does not establish enabled/repeat/filter eligibility, execution admission,
/// current State publication or authority to execute. The eventual caller must
/// retain the same exclusive State owner and execute the exact freshly loaded
/// action, with no intervening mutation/reload from another observation.
///
/// # Errors
/// Rejects missing/wrong-kind actions, invalid registration, forbidden pipeline
/// retry state, missing/substituted IVM storage and canonical encoding failures.
pub(crate) fn pipeline_trigger_use_v1(
    store: &impl SetReadOnly,
    id: &TriggerId,
    applying_height: u64,
) -> Result<TriggerUseV1, String> {
    if store.ids().get(id) != Some(&TriggeringEventType::Pipeline) {
        return Err("pipeline trigger is absent from its exact type registry".into());
    }
    let action = store
        .pipeline_triggers()
        .get(id)
        .ok_or_else(|| "pipeline trigger action is absent".to_owned())?;
    if action.retry_policy.is_some() || action.retry_state.is_some() {
        return Err("pipeline trigger carries Time retry policy or state".into());
    }
    bind_action(store, id, applying_height, 0, action)
}

/// Reload a Time action from its exact typed store and bind persistent content.
///
/// As with [`pipeline_trigger_use_v1`], this returns a description, not a fresh
/// State/eligibility token. The actual scheduler must still verify the event,
/// action policy and resource budget under its sole transaction owner.
///
/// # Errors
/// Rejects missing/wrong-kind actions, invalid registration, missing/substituted
/// IVM storage and canonical encoding failures.
pub(crate) fn time_trigger_use_v1(
    store: &impl SetReadOnly,
    id: &TriggerId,
    applying_height: u64,
) -> Result<TriggerUseV1, String> {
    if store.ids().get(id) != Some(&TriggeringEventType::Time) {
        return Err("Time trigger is absent from its exact type registry".into());
    }
    let action = store
        .time_triggers()
        .get(id)
        .ok_or_else(|| "Time trigger action is absent".to_owned())?;
    bind_action(store, id, applying_height, 1, action)
}

fn bind_action<F: norito::NoritoSerialize>(
    store: &impl SetReadOnly,
    id: &TriggerId,
    applying_height: u64,
    kind: u8,
    action: &LoadedAction<F>,
) -> Result<TriggerUseV1, String> {
    let registered_at_height = action
        .metadata
        .get(trigger_registered_block_height_metadata_key())
        .ok_or_else(|| "trigger registration height is missing".to_owned())?
        .try_into_any_norito::<u64>()
        .map_err(|_| "trigger registration height is malformed".to_owned())?;
    if applying_height == 0 || registered_at_height >= applying_height {
        return Err("trigger registration is not strictly before applying height".into());
    }
    let action_hash = Hash::new_from_writer(|writer| {
        writer.write_all(ACTION_DOMAIN)?;
        writer.write_all(&[kind])?;
        frame(&action.authority, writer)?;
        frame(&action.filter, writer)?;
        frame(&action.repeats, writer)?;
        frame(&action.metadata, writer)?;
        frame(&action.retry_policy, writer)?;
        // Persistent primitive values only: TimeTriggerRetryState has a Core
        // schema identity and must not silently become part of this protocol.
        let retry = action
            .retry_state
            .map(|state| (state.retries_used, state.next_retry_at_ms));
        frame(&retry, writer)?;
        match &action.executable {
            ExecutableRef::Instructions(instructions) => {
                writer.write_all(&[0])?;
                frame(instructions, writer)?;
            }
            ExecutableRef::ContractCall(invocation) => {
                writer.write_all(&[1])?;
                frame(invocation, writer)?;
            }
            ExecutableRef::Ivm(blob_hash) => {
                let (artifact, retained_code_hash) = store
                    .get_original_contract_with_code_hash(blob_hash)
                    .ok_or_else(|| {
                        io::Error::other(
                            "trigger IVM artifact is absent from its authoritative store",
                        )
                    })?;
                // Both computations stream borrowed bytes. Do not use
                // get_original_action, which clones bytecode and drops retry state.
                if HashOf::new(artifact) != *blob_hash {
                    return Err(io::Error::other(
                        "trigger IVM lookup identity differs from retained artifact",
                    ));
                }
                let actual_code_hash = ivm::contract_code_hash(artifact.as_ref());
                if actual_code_hash != retained_code_hash {
                    return Err(io::Error::other(
                        "trigger IVM deployable identity differs from retained artifact",
                    ));
                }
                let len = u64::try_from(artifact.as_ref().len())
                    .map_err(|_| io::Error::other("trigger IVM artifact length exceeds u64"))?;
                writer.write_all(&[2])?;
                frame(blob_hash, writer)?;
                frame(&actual_code_hash, writer)?;
                frame(&len, writer)?;
            }
            ExecutableRef::Batch(items) => {
                writer.write_all(&[3])?;
                frame(items, writer)?;
            }
        }
        Ok(())
    })
    .map_err(|error| format!("trigger persistent action binding failed: {error}"))?;
    Ok(TriggerUseV1 {
        trigger_id: id.clone(),
        registered_at_height,
        action_hash,
    })
}

fn frame<T: norito::NoritoSerialize>(value: &T, writer: &mut dyn Write) -> io::Result<()> {
    norito::core::write_canonical_to_writer(value, writer)
        .map_err(|error| io::Error::other(error.to_string()))
}

#[cfg(test)]
#[path = "invocation_identity_tests.rs"]
mod tests;
