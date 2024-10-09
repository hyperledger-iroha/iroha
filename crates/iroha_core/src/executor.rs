//! Structures and impls related to *runtime* `Executor`s processing.

use std::sync::Arc;

use derive_more::DebugCustom;
use iroha_data_model::{
    account::AccountId,
    executor as data_model_executor,
    isi::InstructionBox,
    query::{AnyQueryBox, QueryRequest},
    transaction::{Executable, SignedTransaction},
    ValidationFail,
};
use iroha_logger::trace;
use serde::{
    de::{DeserializeSeed, VariantAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{
    smartcontracts::{wasm, Execute as _},
    state::{deserialize::WasmSeed, StateReadOnly, StateTransaction},
    WorldReadOnly as _,
};

impl From<wasm::error::Error> for ValidationFail {
    fn from(err: wasm::error::Error) -> Self {
        match err {
            wasm::error::Error::ExportFnCall(call_error) => {
                use wasm::error::ExportFnCallError::*;

                match call_error {
                    ExecutionLimitsExceeded(_) => Self::TooComplex,
                    HostExecution(error) | Other(error) => {
                        Self::InternalError(format!("{error:#}"))
                    }
                }
            }
            _ => Self::InternalError(format!("{err:#}")),
        }
    }
}

/// Executor that verifies that operation is valid and executes it.
///
/// Executing is done in order to verify dependent instructions in transaction.
/// Can be upgraded with [`Upgrade`](iroha_data_model::isi::Upgrade) instruction.
#[derive(Debug, Default, Clone, Serialize)]
pub enum Executor {
    /// Initial executor that allows all operations and performs no permission checking.
    #[default]
    Initial,
    /// User-provided executor with arbitrary logic.
    UserProvided(LoadedExecutor),
}

impl<'de> DeserializeSeed<'de> for WasmSeed<'_, Executor> {
    type Value = Executor;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExecutorVisitor<'l> {
            loader: &'l WasmSeed<'l, Executor>,
        }

        #[derive(Deserialize)]
        #[serde(variant_identifier)]
        enum Field {
            Initial,
            UserProvided,
        }

        impl<'de> Visitor<'de> for ExecutorVisitor<'_> {
            type Value = Executor;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an enum variant")
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::EnumAccess<'de>,
            {
                match data.variant()? {
                    ("Initial", variant) => {
                        variant.unit_variant()?;
                        Ok(Executor::Initial)
                    }
                    ("UserProvided", variant) => {
                        let loaded =
                            variant.newtype_variant_seed(self.loader.cast::<LoadedExecutor>())?;
                        Ok(Executor::UserProvided(loaded))
                    }
                    (other, _) => Err(serde::de::Error::unknown_variant(
                        other,
                        &["Initial", "UserProvided"],
                    )),
                }
            }
        }

        deserializer.deserialize_enum(
            "Executor",
            &["Initial", "UserProvided"],
            ExecutorVisitor { loader: &self },
        )
    }
}

impl Executor {
    /// Execute [`SignedTransaction`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Executor denied the operation.
    pub fn execute_transaction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        transaction: SignedTransaction,
    ) -> Result<(), ValidationFail> {
        trace!("Running transaction execution");

        match self {
            Self::Initial => {
                let (_authority, Executable::Instructions(instructions)) = transaction.into()
                else {
                    return Err(ValidationFail::NotPermitted(
                        "Genesis transaction must not be a smart contract".to_owned(),
                    ));
                };

                for isi in instructions {
                    isi.execute(authority, state_transaction)?
                }
                Ok(())
            }
            Self::UserProvided(loaded_executor) => {
                let runtime =
                    wasm::RuntimeBuilder::<wasm::state::executor::ExecuteTransaction>::new()
                        .with_engine(state_transaction.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
                        .with_config(state_transaction.world.parameters().executor)
                        .build()?;

                runtime.execute_executor_execute_transaction(
                    state_transaction,
                    authority,
                    &loaded_executor.module,
                    transaction,
                )?
            }
        }
    }

    /// Execute [`Instruction`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Executor denied the operation.
    pub fn execute_instruction(
        &self,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        instruction: InstructionBox,
    ) -> Result<(), ValidationFail> {
        trace!("Running instruction execution");

        match self {
            Self::Initial => instruction
                .execute(authority, state_transaction)
                .map_err(Into::into),
            Self::UserProvided(loaded_executor) => {
                let runtime =
                    wasm::RuntimeBuilder::<wasm::state::executor::ExecuteInstruction>::new()
                        .with_engine(state_transaction.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
                        .with_config(state_transaction.world.parameters().executor)
                        .build()?;

                runtime.execute_executor_execute_instruction(
                    state_transaction,
                    authority,
                    &loaded_executor.module,
                    instruction,
                )?
            }
        }
    }

    /// Validate [`QueryBox`].
    ///
    /// # Errors
    ///
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute the entrypoint of the WASM blob;
    /// - Executor denied the operation.
    pub fn validate_query<S: StateReadOnly>(
        &self,
        state_ro: &S,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), ValidationFail> {
        trace!("Running query validation");

        let query = match query {
            QueryRequest::Singular(singular) => AnyQueryBox::Singular(singular.clone()),
            QueryRequest::Start(iterable) => AnyQueryBox::Iterable(iterable.clone()),
            QueryRequest::Continue(_) => {
                // The iterable query was already validated when it started
                return Ok(());
            }
        };

        match self {
            Self::Initial => Ok(()),
            Self::UserProvided(loaded_executor) => {
                let runtime =
                    wasm::RuntimeBuilder::<wasm::state::executor::ValidateQuery<S>>::new()
                        .with_engine(state_ro.engine().clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
                        .with_config(state_ro.world().parameters().executor)
                        .build()?;

                runtime.execute_executor_validate_query(
                    state_ro,
                    authority,
                    &loaded_executor.module,
                    query,
                )?
            }
        }
    }

    /// Migrate executor to a new user-provided one.
    ///
    /// Execute `migrate()` entrypoint of the `raw_executor` and set `self` to
    /// [`UserProvided`](Executor::UserProvided) with `raw_executor`.
    ///
    /// # Errors
    ///
    /// - Failed to load `raw_executor`;
    /// - Failed to prepare runtime for WASM execution;
    /// - Failed to execute entrypoint of the WASM blob.
    pub fn migrate(
        &mut self,
        raw_executor: data_model_executor::Executor,
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<(), wasm::error::Error> {
        trace!("Running executor migration");

        let loaded_executor = LoadedExecutor::load(state_transaction.engine, raw_executor)?;

        let runtime = wasm::RuntimeBuilder::<wasm::state::executor::Migrate>::new()
            .with_engine(state_transaction.engine.clone()) // Cloning engine is cheap, see [`wasmtime::Engine`] docs
            .with_config(state_transaction.world().parameters().executor)
            .build()?;

        runtime.execute_executor_migration(
            state_transaction,
            authority,
            &loaded_executor.module,
        )?;

        *self = Self::UserProvided(loaded_executor);
        Ok(())
    }
}

/// [`Executor`] with [`Module`](wasmtime::Module) for execution.
///
/// Creating a [`wasmtime::Module`] is expensive, so we do it once on [`migrate()`](Executor::migrate)
/// step and reuse it later on validating steps.
#[derive(DebugCustom, Clone, Serialize)]
#[debug(fmt = "LoadedExecutor {{ module: <Module is truncated> }}")]
pub struct LoadedExecutor {
    #[serde(skip)]
    module: wasmtime::Module,
    /// Arc is needed so cloning of executor will be fast.
    /// See [`crate::tx::TransactionExecutor::validate_with_runtime_executor`].
    raw_executor: Arc<data_model_executor::Executor>,
}

impl LoadedExecutor {
    fn load(
        engine: &wasmtime::Engine,
        raw_executor: data_model_executor::Executor,
    ) -> Result<Self, wasm::error::Error> {
        Ok(Self {
            module: wasm::load_module(engine, &raw_executor.wasm)?,
            raw_executor: Arc::new(raw_executor),
        })
    }
}

impl<'de> DeserializeSeed<'de> for WasmSeed<'_, LoadedExecutor> {
    type Value = LoadedExecutor;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // a copy of `LoadedExecutor` without the `module` field
        #[derive(Deserialize)]
        struct LoadedExecutor {
            raw_executor: data_model_executor::Executor,
        }

        let executor = LoadedExecutor::deserialize(deserializer)?;

        self::LoadedExecutor::load(self.engine, executor.raw_executor)
            .map_err(serde::de::Error::custom)
    }
}
