//! Structures and impls related to *runtime* `Validator`s processing.

use derive_more::DebugCustom;
use iroha_data_model::{account::AccountId, validator as data_model_validator, ValidationFail};
#[cfg(test)]
use iroha_data_model::{
    isi::InstructionBox, transaction::Executable, validator::NeedsValidationBox,
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
                    HostExecution(_) | Other(_) => Self::InternalError(call_error.to_string()),
                }
            }
            _ => Self::InternalError(err.to_string()),
        }
    }
}

/// Result type for [`Validator`] operations.
pub type Result<T, E = ValidationFail> = core::result::Result<T, E>;

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
    ) -> Result<Self> {
        Ok(Self {
            loaded_validator: LoadedValidator::load(engine, raw_validator)?,
        })
    }

    /// Validate operation.
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute WASM blob;
    /// - Validator denied the operation
    pub fn validate(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        operation: impl Into<data_model_validator::NeedsValidationBox>,
    ) -> Result<()> {
        let operation = operation.into();

        let runtime = wasm::RuntimeBuilder::<wasm::state::Validator>::new()
            .with_engine(wsv.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_configuration(wsv.config.wasm_runtime_config)
            .build()?;

        trace!("Running validator");
        runtime.execute_validator_module(
            wsv,
            authority,
            &self.loaded_validator.module,
            &operation,
        )?
    }
}

/// Mock of validator for unit tests of `iroha_core`.
///
/// We can't use real validator because WASM for it is produced in runtime from outside world.
#[cfg(test)]
#[derive(Default, Debug, Copy, Clone)]
pub struct MockValidator;

#[cfg(test)]
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
    ) -> Result<Self> {
        panic!("You probably don't need this method in tests")
    }

    /// Mock for operation validation.
    /// Will just execute instructions if there are some.
    ///
    /// Without this step invalid transactions won't be marked as rejected in
    /// [`ChainedBlock::validate`].
    /// Real [`Validator`] assumes that internal WASM performs this.
    ///
    /// # Errors
    ///
    /// Never fails.
    #[allow(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        clippy::trivially_copy_pass_by_ref,
        clippy::needless_pass_by_value
    )]
    pub fn validate(
        &self,
        wsv: &mut WorldStateView,
        authority: &AccountId,
        operation: impl Into<data_model_validator::NeedsValidationBox>,
    ) -> Result<()> {
        match operation.into() {
            NeedsValidationBox::Instruction(isi) => Self::execute_instruction(wsv, authority, isi),
            NeedsValidationBox::Transaction(tx) => {
                let (_authority, Executable::Instructions(instructions)) = tx.into() else {
                    return Ok(());
                };
                for isi in instructions {
                    Self::execute_instruction(wsv, authority, isi)?;
                }
                Ok(())
            }
            NeedsValidationBox::Query(_) => Ok(()),
        }
    }

    fn execute_instruction(
        wsv: &mut WorldStateView,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<()> {
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
    ) -> Result<Self> {
        Ok(Self {
            module: wasm::load_module(engine, raw_validator.wasm)?,
        })
    }
}
