//! Immutable runtime consent of each trigger incarnation.
//!
//! Registration captures the resolved execution closure. Its domain-separated digest is
//! committed in reserved metadata, so the conservative native bundle does not duplicate its
//! entire definition graph in every trigger. Execution recomputes that closure before any
//! effects and checks restrictions independently. There is no fallback for absent captures.

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::error::InstructionExecutionError, runtime::lifecycle::RuntimeRejectionV1,
    transaction::Executable,
};
use iroha_model_base::{metadata::Metadata, name::Name};
use std::sync::OnceLock;

use super::{TRIGGER_RUNTIME_EXPECTATION_METADATA_KEY, set::ExecutableRef};
use crate::{runtime::expectation, state::WorldTransaction};

pub(super) fn metadata_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| {
        TRIGGER_RUNTIME_EXPECTATION_METADATA_KEY
            .parse()
            .expect("runtime expectation metadata key is valid")
    })
}

/// Capture only an executable closure that can run at the registration height.
pub(super) fn capture_trigger_runtime(
    executable: &Executable,
    world: &WorldTransaction<'_, '_>,
    height: u64,
) -> Result<Hash, InstructionExecutionError> {
    let captured = expectation::capture_executable(executable, world)?;
    expectation::check(&captured, world, height)?;
    expectation::digest_owned(&captured, world)
}

/// Refuse changed or restricted semantics before a callback starts doing work.
pub(crate) fn check_trigger_runtime(
    metadata: &Metadata,
    executable: &ExecutableRef,
    world: &WorldTransaction<'_, '_>,
    height: u64,
) -> Result<(), InstructionExecutionError> {
    let value = metadata
        .get(metadata_key())
        .ok_or(RuntimeRejectionV1::ExpectationMismatch { wire_id: None })?;
    let expected = norito::core::classify_decode_attempt(|| value.try_into_any_norito::<Hash>())
        .map_err(|error| {
            world.attempt_error_to_instruction_error(
                crate::execution_attempt::canonical_decode_attempt_error(error, |_| {
                    RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.into()
                }),
            )
        })?;
    let actual = expectation::capture_loaded_executable(executable, world)?;
    if expected != expectation::digest_owned(&actual, world)? {
        return Err(RuntimeRejectionV1::ExpectationMismatch { wire_id: None }.into());
    }
    expectation::check(&actual, world, height)
}
