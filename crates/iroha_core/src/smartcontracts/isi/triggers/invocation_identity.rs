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
const LIFECYCLE_ACTION_DOMAIN: &[u8] = b"iroha:trigger-lifecycle:action:v1\0";
const EXECUTABLE_DOMAIN: &[u8] = b"iroha:trigger-lifecycle:executable:v1\0";

// Exact identities already declared by these fixed field types. Borrow them for
// bounded hashing instead of constructing their generic schema names. Tests
// compare both these declarations and complete frames against ordinary Norito.
const RETRY_POLICY_FRAME: &str =
    "core::option::Option<iroha_data_model::trigger::model::action::model::TimeTriggerRetryPolicy>";
const RETRY_STATE_FRAME: &str = "core::option::Option<(u32, u64)>";
const INSTRUCTIONS_FRAME: &str =
    "iroha_primitives::const_vec::ConstVec<iroha_data_model::isi::InstructionBox>";
const BATCH_FRAME: &str = "iroha_primitives::const_vec::ConstVec<iroha_data_model::transaction::executable::model::ExecutableBatchItem>";
const IVM_HASH_FRAME: &str =
    "iroha_crypto::hash::HashOf<iroha_data_model::transaction::executable::model::IvmBytecode>";

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
pub fn pipeline_trigger_use_v1(
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
pub fn time_trigger_use_v1(
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
    let action_hash = persistent_action_hash(store, action, kind)?;
    Ok(TriggerUseV1 {
        trigger_id: id.clone(),
        registered_at_height,
        action_hash,
    })
}

/// Existing use-time hash recipe without the scheduler's eligibility-height check.
/// Lifecycle snapshots include same-block registration and subsequent mutations.
pub(super) fn persistent_action_hash<F: norito::NoritoSerialize>(
    store: &impl SetReadOnly,
    action: &LoadedAction<F>,
    kind: u8,
) -> Result<Hash, String> {
    persistent_action_hash_inner(
        store,
        action,
        kind,
        &mut HashFailure::default(),
        &[],
        &mut None,
        None,
    )
    .map_err(|error| format!("trigger persistent action binding failed: {error}"))
}

/// Selected commitment of mutable Action fields and the immutable executable digest.
/// The predecessor uses its frozen byte-concatenation recipe above; a selected
/// lifecycle journal and its SDK consumer use this new service-bound recipe.
pub(super) fn lifecycle_action_hash<F: norito::NoritoSerialize>(
    store: &impl SetReadOnly,
    action: &LoadedAction<F>,
    kind: u8,
    verified: &[VerifiedIvm],
    executable: Option<Hash>,
) -> Result<(Hash, Option<VerifiedIvm>, Hash), norito::Error> {
    let mut failure = HashFailure::default();
    let mut fresh = None;
    let executable = executable.or(action.lifecycle_executable_digest);
    if !matches!(&action.executable, ExecutableRef::Ivm(_)) && executable.is_none() {
        // Construction/restore already attempted the ordinary canonical source
        // encoder. Its deterministic source refusal must not become a cold-cache
        // full-body fallback with consensus work depending on cache presence.
        return Err(norito::core::refuse_encode_source());
    }
    let operation = (|| {
        // Reuse only the same authoritative immutable IVM tuple. A source
        // disagreement remains unavailable even if its executable digest exists.
        let executable = if let (Some(_), ExecutableRef::Ivm(blob)) =
            (executable, &action.executable)
        {
            let (artifact, code) = store
                .get_original_contract_with_code_hash(blob)
                .ok_or_else(|| failure.source("trigger IVM artifact is absent"))?;
            let candidate = VerifiedIvm {
                blob: *blob,
                code,
                length: artifact.as_ref().len() as u64,
            };
            if store.verified_original_contract(blob) != Some((code, candidate.length)) {
                return Err(failure
                    .source("immutable trigger artifact differs from its authoritative identity"));
            }
            if norito::core::prepaid_validation_active() {
                let comparisons = verified.len().checked_ilog2().unwrap_or(0) as usize + 1;
                norito::core::charge_encode_prewrite(comparisons * Hash::LENGTH)
                    .map_err(|e| failure.codec(e))?;
            }
            match verified.binary_search_by_key(blob, |entry| entry.blob) {
                Ok(index) if verified[index] == candidate => executable,
                Ok(_) => {
                    return Err(failure
                        .source("cached trigger IVM identity differs from its authoritative row"));
                }
                Err(_) => None,
            }
        } else {
            executable
        };
        let executable = match executable {
            Some(hash) => hash,
            None => Hash::new_from_writer(|writer| {
                if norito::core::prepaid_validation_active() {
                    norito::core::charge_encode_prewrite(EXECUTABLE_DOMAIN.len() + 1)
                        .map_err(|e| failure.codec(e))?;
                }
                writer.write_all(EXECUTABLE_DOMAIN)?;
                write_executable(
                    store,
                    action,
                    writer,
                    &mut failure,
                    verified,
                    &mut fresh,
                    true,
                )
            })?,
        };
        persistent_action_hash_inner(
            store,
            action,
            kind,
            &mut failure,
            verified,
            &mut fresh,
            Some(executable),
        )
        .map(|hash| (hash, fresh, executable))
    })();
    operation.map_err(|_: io::Error| {
        if failure.source.is_some() {
            norito::core::refuse_encode_source()
        } else {
            failure.codec.unwrap_or(norito::Error::InvalidValue {
                context: "lifecycle action hash",
            })
        }
    })
}

/// One successful validation of an immutable contract-store row in this transaction.
/// This capability is never serialized or reused across State transactions. The
/// journal discards it when the authoritative row is removed or inserted anew.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VerifiedIvm {
    pub(super) blob: HashOf<IvmBytecode>,
    pub(super) code: Hash,
    pub(super) length: u64,
}

#[derive(Default)]
struct HashFailure {
    source: Option<&'static str>,
    codec: Option<norito::Error>,
}
impl HashFailure {
    fn source(&mut self, message: &'static str) -> io::Error {
        self.source = Some(message);
        hash_error(message)
    }
    fn codec(&mut self, error: norito::Error) -> io::Error {
        let diagnostic = if norito::core::prepaid_validation_active() {
            io::ErrorKind::InvalidData.into()
        } else {
            io::Error::other(error.to_string())
        };
        self.codec = Some(error);
        diagnostic
    }
}

fn hash_error(message: &'static str) -> io::Error {
    if norito::core::prepaid_validation_active() {
        io::ErrorKind::InvalidData.into()
    } else {
        io::Error::other(message)
    }
}

fn persistent_action_hash_inner<F: norito::NoritoSerialize>(
    store: &impl SetReadOnly,
    action: &LoadedAction<F>,
    kind: u8,
    failure: &mut HashFailure,
    verified: &[VerifiedIvm],
    fresh: &mut Option<VerifiedIvm>,
    executable: Option<Hash>,
) -> io::Result<Hash> {
    Hash::new_from_writer(|writer| {
        if norito::core::prepaid_validation_active() {
            // The domain, action kind and executable discriminant bypass Encoder.
            norito::core::charge_encode_prewrite(if executable.is_some() {
                LIFECYCLE_ACTION_DOMAIN.len() + 1
            } else {
                ACTION_DOMAIN.len() + 2
            })
            .map_err(|error| failure.codec(error))?;
        }
        writer.write_all(if executable.is_some() {
            LIFECYCLE_ACTION_DOMAIN
        } else {
            ACTION_DOMAIN
        })?;
        writer.write_all(&[kind])?;
        frame(&action.authority, writer, failure)?;
        frame(&action.filter, writer, failure)?;
        frame(&action.repeats, writer, failure)?;
        frame(&action.metadata, writer, failure)?;
        fixed_frame(&action.retry_policy, RETRY_POLICY_FRAME, writer, failure)?;
        // Persistent primitive values only: TimeTriggerRetryState has a Core
        // schema identity and must not silently become part of this protocol.
        let retry = action
            .retry_state
            .map(|state| (state.retries_used, state.next_retry_at_ms));
        fixed_frame(&retry, RETRY_STATE_FRAME, writer, failure)?;
        if let Some(executable) = executable {
            frame(&executable, writer, failure)?;
        } else {
            write_executable(store, action, writer, failure, verified, fresh, false)?;
        }
        Ok(())
    })
}

fn write_executable<F>(
    store: &impl SetReadOnly,
    action: &LoadedAction<F>,
    writer: &mut dyn Write,
    failure: &mut HashFailure,
    verified: &[VerifiedIvm],
    fresh: &mut Option<VerifiedIvm>,
    selected: bool,
) -> io::Result<()> {
    match &action.executable {
        ExecutableRef::Instructions(instructions) => {
            writer.write_all(&[0])?;
            fixed_frame(instructions, INSTRUCTIONS_FRAME, writer, failure)?;
        }
        ExecutableRef::ContractCall(invocation) => {
            writer.write_all(&[1])?;
            frame(invocation, writer, failure)?;
        }
        ExecutableRef::Ivm(blob_hash) => {
            let (artifact, retained_code_hash) = store
                .get_original_contract_with_code_hash(blob_hash)
                .ok_or_else(|| {
                    failure.source("trigger IVM artifact is absent from its authoritative store")
                })?;
            let len = u64::try_from(artifact.as_ref().len())
                .map_err(|_| failure.source("trigger IVM artifact length exceeds u64"))?;
            let candidate = VerifiedIvm {
                blob: *blob_hash,
                code: retained_code_hash,
                length: len,
            };
            // Cache keys describe private immutable store entries, not caller
            // hashes or allocation addresses. Registration/removal invalidates
            // their entry lifetime; a reference-count update changes no bytes.
            if norito::core::prepaid_validation_active() {
                let comparisons = verified.len().checked_ilog2().unwrap_or(0) as usize + 1;
                norito::core::charge_encode_prewrite(comparisons * Hash::LENGTH)
                    .map_err(|error| failure.codec(error))?;
            }
            if selected {
                if store.verified_original_contract(blob_hash) != Some((retained_code_hash, len)) {
                    return Err(failure.source(
                        "immutable trigger artifact differs from its authoritative identity",
                    ));
                }
                *fresh = Some(candidate);
            } else {
                match verified.binary_search_by_key(blob_hash, |entry| entry.blob) {
                    Ok(index) if verified[index] == candidate => {}
                    Ok(_) => {
                        return Err(failure.source(
                            "cached trigger IVM identity differs from its authoritative row",
                        ));
                    }
                    Err(_) => {
                        // Each distinct immutable row streams its original blob
                        // and deployable-code identity once per root, then all
                        // later Action snapshots reuse this verified pair.
                        let actual_blob_hash = if norito::core::prepaid_validation_active() {
                            HashOf::try_new(artifact).map_err(|error| failure.codec(error))?
                        } else {
                            HashOf::new(artifact)
                        };
                        if actual_blob_hash != *blob_hash {
                            return Err(failure.source(
                                "trigger IVM lookup identity differs from retained artifact",
                            ));
                        }
                        if norito::core::prepaid_validation_active() {
                            norito::core::charge_encode_prewrite(artifact.as_ref().len())
                                .map_err(|error| failure.codec(error))?;
                        }
                        if ivm::contract_code_hash(artifact.as_ref()) != retained_code_hash {
                            return Err(failure.source(
                                "trigger IVM deployable identity differs from retained artifact",
                            ));
                        }
                        *fresh = Some(candidate);
                    }
                }
            }
            writer.write_all(&[2])?;
            fixed_frame(blob_hash, IVM_HASH_FRAME, writer, failure)?;
            frame(&retained_code_hash, writer, failure)?;
            fixed_frame(&len, "u64", writer, failure)?;
        }
        ExecutableRef::Batch(items) => {
            writer.write_all(&[3])?;
            fixed_frame(items, BATCH_FRAME, writer, failure)?;
        }
    }
    Ok(())
}

/// Derived immutable binding, prepared at registration/restore and never serialized.
/// An unsupported source remains uncached; selected bounded capture still refuses it.
/// Cache preparation cannot reject a predecessor registration or change its hash.
pub(in crate::smartcontracts::isi::triggers) fn inline_executable_digest(
    executable: &ExecutableRef,
) -> Option<Hash> {
    assert!(
        !norito::core::prepaid_validation_active(),
        "inline trigger binding construction is forbidden inside prepaid capture"
    );
    if matches!(executable, ExecutableRef::Ivm(_)) {
        return None;
    }
    let mut failure = HashFailure::default();
    Hash::new_from_writer(|writer| {
        writer.write_all(EXECUTABLE_DOMAIN)?;
        match executable {
            ExecutableRef::Instructions(items) => {
                writer.write_all(&[0])?;
                fixed_frame(items, INSTRUCTIONS_FRAME, writer, &mut failure)?;
            }
            ExecutableRef::ContractCall(call) => {
                writer.write_all(&[1])?;
                frame(call, writer, &mut failure)?;
            }
            ExecutableRef::Batch(items) => {
                writer.write_all(&[3])?;
                fixed_frame(items, BATCH_FRAME, writer, &mut failure)?;
            }
            ExecutableRef::Ivm(_) => unreachable!(),
        }
        Ok(())
    })
    .ok()
}

fn fixed_frame<T: norito::NoritoSerialize>(
    value: &T,
    identity: &'static str,
    writer: &mut dyn Write,
    failure: &mut HashFailure,
) -> io::Result<()> {
    if norito::core::prepaid_validation_active() {
        norito::core::write_canonical_payload_with_identity::<T, T, _>(value, writer, identity)
            .map_err(|error| failure.codec(error))
    } else {
        frame(value, writer, failure)
    }
}

fn frame<T: norito::NoritoSerialize>(
    value: &T,
    writer: &mut dyn Write,
    failure: &mut HashFailure,
) -> io::Result<()> {
    norito::core::write_canonical_to_writer(value, writer).map_err(|error| failure.codec(error))
}

#[cfg(test)]
#[path = "invocation_identity_tests.rs"]
mod tests;
