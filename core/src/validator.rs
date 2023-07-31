//! Structures and impls related to *runtime* `Validator`s processing.

use derive_more::DebugCustom;
#[cfg(test)]
use iroha_data_model::transaction::Executable;
use iroha_data_model::{
    account::AccountId,
    isi::InstructionBox,
    query::QueryBox,
    transaction::VersionedSignedTransaction,
    validator::{self as data_model_validator, MigrationResult},
    ValidationFail,
};
use iroha_logger::trace;

use super::wsv::WorldStateView;
use crate::smartcontracts::wasm::{
    self,
    error::{Error, ExportFnCallError},
};

impl From<Error> for ValidationFail {
    fn from(err: Error) -> Self {
        match err {
            Error::ExportFnCall(call_error) => {
                use ExportFnCallError::*;

                match call_error {
                    ExecutionLimitsExceeded(_) => Self::TooComplex,
                    HostExecution(error) | Other(error) => Self::InternalError(error.to_string()),
                }
            }
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Validator that verifies the operation is valid.
///
/// Can be upgraded with [`Upgrade`](iroha_data_model::isi::Upgrade) instruction.
#[derive(DebugCustom, Clone)]
#[debug(
    fmt = "Validator {{ loaded_validator: {0:?}, engine: <Engine is truncated> }}",
    "&self.loaded_validator"
)]
pub struct Validator {
    /// Pre-loaded validator.
    /// Can be set with [`update()`](Validator::update).
    loaded_validator: LoadedValidator,
}

impl Validator {
    /// Create new [`Validator`] from raw validator.
    ///
    /// # Errors
    ///
    /// Fails if failed to load wasm blob.
    pub fn new(
        raw_validator: data_model_validator::Validator,
        engine: &wasmtime::Engine,
    ) -> Result<Self, wasm::error::Error> {
        Ok(Self {
            loaded_validator: LoadedValidator::load(engine, raw_validator)?,
        })
    }

    /// Validate [`VersionedSignedTransaction`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Validator denied the operation.
    pub fn validate_transaction(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        transaction: VersionedSignedTransaction,
    ) -> Result<(), ValidationFail> {
        let runtime = wasm::RuntimeBuilder::<wasm::state::validator::ValidateTransaction>::new()
            .with_engine(wsv.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_configuration(wsv.config.wasm_runtime_config)
            .build()?;

        trace!("Running transaction validation");
        runtime.execute_validator_validate_transaction(
            wsv,
            authority,
            &self.loaded_validator.module,
            transaction,
        )?
    }

    /// Validate [`InstructionBox`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Validator denied the operation.
    pub fn validate_instruction(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        let runtime = wasm::RuntimeBuilder::<wasm::state::validator::ValidateInstruction>::new()
            .with_engine(wsv.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_configuration(wsv.config.wasm_runtime_config)
            .build()?;

        trace!("Running instruction validation");
        runtime.execute_validator_validate_instruction(
            wsv,
            authority,
            &self.loaded_validator.module,
            instruction,
        )?
    }

    /// Validate [`QueryBox`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Validator denied the operation.
    pub fn validate_query(
        &self,
        wsv: &WorldStateView,
        authority: &AccountId,
        query: QueryBox,
    ) -> Result<(), ValidationFail> {
        let runtime = wasm::RuntimeBuilder::<wasm::state::validator::ValidateQuery>::new()
            .with_engine(wsv.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_configuration(wsv.config.wasm_runtime_config)
            .build()?;

        trace!("Running query validation");
        runtime.execute_validator_validate_query(
            wsv,
            authority,
            &self.loaded_validator.module,
            query,
        )?
    }

    /// Run migration.
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute entrypoint of the WASM blob.
    pub fn migrate(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
    ) -> Result<MigrationResult, wasm::error::Error> {
        let runtime = wasm::RuntimeBuilder::<wasm::state::validator::Migrate>::new()
            .with_engine(wsv.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_configuration(wsv.config.wasm_runtime_config)
            .build()?;

        runtime
            .execute_validator_migration(wsv, authority, &self.loaded_validator.module)
            .map_err(Into::into)
    }
}

/// Mock of validator for unit tests of `iroha_core`.
///
/// We can't use real validator because WASM for it is produced in runtime from outside world.
#[cfg(test)]
#[derive(Default, Debug, Copy, Clone)]
pub struct MockValidator;

#[cfg(test)]
#[allow(
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::trivially_copy_pass_by_ref,
    clippy::needless_pass_by_value
)]
impl MockValidator {
    /// Mock for creating new validator from raw validator.
    ///
    /// # Errors
    ///
    /// Never fails with [`Err`].
    ///
    /// # Panics
    ///
    /// Will immediately panic, because you shouldn't call it in tests.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        _raw_validator: data_model_validator::Validator,
        _engine: &wasmtime::Engine,
    ) -> Result<Self, wasm::error::Error> {
        panic!("You probably don't need this method in tests")
    }

    /// Mock for transaction validation.
    /// Will just execute instructions if there are some.
    ///
    /// Without this step invalid transactions won't be marked as rejected in
    /// [`ChainedBlock::validate`].
    ///
    /// # Errors
    ///
    /// Fails if instruction execution fails
    pub fn validate_transaction(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        transaction: VersionedSignedTransaction,
    ) -> Result<(), ValidationFail> {
        let (_authority, Executable::Instructions(instructions)) = transaction.into() else {
            return Ok(());
        };
        for isi in instructions {
            Self::execute_instruction(wsv, authority, isi)?;
        }
        Ok(())
    }

    /// Mock for instruction validation. Will just execute instruction.
    ///
    /// # Errors
    ///
    /// Fails if instruction execution fails
    pub fn validate_instruction(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        Self::execute_instruction(wsv, authority, instruction)
    }

    /// Mock for query validation.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn validate_query(
        &self,
        _wsv: &WorldStateView,
        _authority: &AccountId,
        _query: QueryBox,
    ) -> Result<(), ValidationFail> {
        Ok(())
    }

    /// Mock for validator migration.
    ///
    /// # Errors
    ///
    /// Never fails.
    pub fn migrate(
        &self,
        _wsv: &mut WorldStateView,
        _authority: &AccountId,
    ) -> Result<MigrationResult, wasm::error::Error> {
        Ok(Ok(()))
    }

    fn execute_instruction(
        wsv: &mut WorldStateView,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        use super::smartcontracts::Execute as _;

        instruction.execute(authority, wsv).map_err(Into::into)
    }
}

/// [`Validator`] with [`Module`](wasmtime::Module) for execution.
///
/// Creating [`Module`] is expensive, so we do it once on [`upgrade()`](Validator::upgrade)
/// step and reuse it on [`validate()`](Validator::validate) step.
#[derive(DebugCustom, Clone)]
#[debug(fmt = "LoadedValidator {{ module: <Module is truncated> }}")]
struct LoadedValidator {
    #[cfg_attr(test, allow(dead_code))]
    module: wasmtime::Module,
}

impl LoadedValidator {
    pub fn load(
        engine: &wasmtime::Engine,
        raw_validator: data_model_validator::Validator,
    ) -> Result<Self, wasm::error::Error> {
        Ok(Self {
            module: wasm::load_module(engine, raw_validator.wasm)?,
        })
    }
}
