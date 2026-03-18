//! API for *Runtime Executors*.
#![allow(unsafe_code)]
#![allow(unexpected_cfgs)]
#![allow(clippy::result_large_err)]

use std::collections::{BTreeMap, BTreeSet};

use data_model::{ValidationFail, executor::Result, parameter::CustomParameterId};
#[cfg(not(test))]
use data_model::{prelude::*, query::AnyQueryBox, smart_contract::payloads};
use iroha_executor_data_model::{
    parameter::Parameter, permission::Permission as ExecutorPermission,
};
pub use iroha_executor_derive::{entrypoint, migrate};
use iroha_schema::{Ident, MetaMap};
pub use iroha_smart_contract as smart_contract;
pub use iroha_smart_contract_utils::{DebugExpectExt, DebugUnwrapExt, dbg, dbg_panic};
pub use smart_contract::{Iroha, data_model};

pub mod default;
pub mod permission;
pub mod runtime;

#[cfg(feature = "bridge")]
pub mod bridge;

pub mod log {
    //! IVM runtime logging utilities
    pub use iroha_smart_contract_utils::{debug, error, event, info, trace, warn};
}

#[doc(hidden)]
pub mod utils {
    //! Crate with utilities

    #[cfg(not(test))]
    use iroha_smart_contract_codec::decode_with_length_prefix_from_raw;
    pub use iroha_smart_contract_codec::encode_with_length_prefix;
    pub use iroha_smart_contract_utils::register_getrandom_err_callback;

    #[cfg(not(test))]
    use super::*;

    /// Get context for `validate_transaction()` entrypoint.
    ///
    /// # Safety
    ///
    /// It's safe to call this function as long as it's safe to construct, from the given
    /// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
    #[doc(hidden)]
    #[cfg(not(test))]
    pub unsafe fn __decode_execute_transaction_context(
        context: *const u8,
    ) -> payloads::Validate<SignedTransaction> {
        unsafe { decode_with_length_prefix_from_raw(context) }
    }

    /// Get context for `validate_instruction()` entrypoint.
    ///
    /// # Safety
    ///
    /// It's safe to call this function as long as it's safe to construct, from the given
    /// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
    #[doc(hidden)]
    #[cfg(not(test))]
    pub unsafe fn __decode_execute_instruction_context(
        context: *const u8,
    ) -> payloads::Validate<InstructionBox> {
        unsafe { decode_with_length_prefix_from_raw(context) }
    }

    /// Get context for `validate_query()` entrypoint.
    ///
    /// # Safety
    ///
    /// It's safe to call this function as long as it's safe to construct, from the given
    /// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
    #[doc(hidden)]
    #[cfg(not(test))]
    pub unsafe fn __decode_validate_query_context(
        context: *const u8,
    ) -> payloads::Validate<AnyQueryBox> {
        unsafe { decode_with_length_prefix_from_raw(context) }
    }

    /// Get context for `migrate()` entrypoint.
    ///
    /// # Safety
    ///
    /// It's safe to call this function as long as it's safe to construct, from the given
    /// pointer, byte array of prefix length and `Box<[u8]>` containing the encoded object
    #[doc(hidden)]
    #[cfg(not(test))]
    pub unsafe fn __decode_migrate_context(context: *const u8) -> payloads::ExecutorContext {
        unsafe { decode_with_length_prefix_from_raw(context) }
    }
}

/// Set new [`ExecutorDataModel`].
///
/// # Errors
///
/// - If execution on Iroha side failed
///
/// # Traps
///
/// Host side will generate a trap if this function was not called from a
/// executor's `migrate()` entrypoint.
#[cfg(not(test))]
pub fn set_data_model(data_model: &ExecutorDataModel) {
    // Safety: - ownership of the returned result is transferred into `_decode_from_raw`
    unsafe { iroha_smart_contract_utils::encode_and_execute(data_model, host::set_data_model) }
}

#[cfg(not(test))]
mod host {
    unsafe extern "C" {
        /// Set new [`ExecutorDataModel`].
        pub(super) fn set_data_model(ptr: *const u8, len: usize);
    }
}

/// Execute instruction if verdict is [`Ok`], deny if execution failed and return.
///
/// Convention is that you have no checks left if you decided to execute instruction.
#[macro_export]
macro_rules! execute {
    ($executor:ident, $isi:ident) => {{
        #[cfg(debug_assertions)]
        if !$executor.verdict().is_ok() {
            unreachable!("Executor already denied");
        }

        if let Err(err) = $executor.host().submit($isi) {
            $executor.deny(err);
        }

        return;
    }};
}

/// Shortcut for setting verdict to [`Err`] and return.
///
/// Supports [`format!`](std::fmt::format) syntax as well as any expression returning [`String`](std::string::String).
#[macro_export]
macro_rules! deny {
    ($executor:ident, $l:literal $(,)?) => {{
        #[cfg(debug_assertions)]
        if $executor.verdict().is_err() {
            unreachable!("Executor already denied");
        }
        $executor.deny($crate::data_model::ValidationFail::NotPermitted(
            ::std::fmt::format(::core::format_args!($l)),
        ));
        return;
    }};
    ($executor:ident, $e:expr $(,)?) => {{
        #[cfg(debug_assertions)]
        if $executor.verdict().is_err() {
            unreachable!("Executor already denied");
        }
        $executor.deny($e);
        return;
    }};
}

/// A convenience to build [`ExecutorDataModel`] from within the executor
#[derive(Debug, Clone)]
pub struct DataModelBuilder {
    parameters: BTreeMap<CustomParameterId, data_model::parameter::CustomParameter>,
    instructions: BTreeSet<Ident>,
    permissions: BTreeSet<Ident>,
    schema: MetaMap,
}

impl DataModelBuilder {
    /// Constructor
    // we don't need to confuse with `with_default_permissions`
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            parameters: <_>::default(),
            instructions: <_>::default(),
            permissions: <_>::default(),
            schema: <_>::default(),
        }
    }

    /// Creates a data model with default permissions preset (defined in [`default::permission`])
    #[must_use]
    pub fn with_default_permissions() -> Self {
        let mut builder = Self::new();

        macro_rules! add_to_schema {
            ($token_ty:ty) => {
                builder = builder.add_permission::<$token_ty>();
            };
        }

        permission::map_default_permissions!(add_to_schema);

        builder
    }

    /// Define a permission in the data model
    #[must_use]
    pub fn add_parameter<T: Parameter + Into<data_model::parameter::CustomParameter>>(
        mut self,
        param: T,
    ) -> Self {
        T::update_schema_map(&mut self.schema);
        self.parameters.insert(
            <T as iroha_executor_data_model::parameter::Parameter>::id(),
            param.into(),
        );
        self
    }

    /// Define a type of custom instruction in the data model.
    /// Corresponds to payload of `InstructionBox::Custom`.
    #[must_use]
    pub fn add_instruction<T: iroha_schema::IntoSchema>(mut self) -> Self {
        T::update_schema_map(&mut self.schema);
        self.instructions.insert(T::type_name());
        self
    }

    /// Define a permission in the data model
    #[must_use]
    pub fn add_permission<T: ExecutorPermission>(mut self) -> Self {
        T::update_schema_map(&mut self.schema);
        self.permissions.insert(T::name());
        self
    }

    /// Remove a permission from the data model
    #[must_use]
    pub fn remove_permission<T: ExecutorPermission>(mut self) -> Self {
        T::remove_from_schema(&mut self.schema);
        self.permissions.remove(&T::name());
        self
    }

    /// Set the data model of the executor via [`set_data_model`]
    #[cfg(not(test))]
    pub fn build_and_set(self, host: &Iroha) {
        let all_accounts = host.query(FindAccounts::new()).execute().unwrap();
        let all_roles = host.query(FindRoles::new()).execute().unwrap();

        for role in all_roles.into_iter().map(|role| role.unwrap()) {
            for permission in role.permissions() {
                if !self.permissions.contains(permission.name()) {
                    host.submit(&Revoke::role_permission(
                        permission.clone(),
                        role.id().clone(),
                    ))
                    .unwrap();
                }
            }
        }

        for account in all_accounts.into_iter().map(|account| account.unwrap()) {
            let account_permissions = host
                .query(FindPermissionsByAccountId::new(account.id().clone()))
                .execute()
                .unwrap();

            for permission in account_permissions.map(|permission| permission.unwrap()) {
                if !self.permissions.contains(permission.name()) {
                    host.submit(&Revoke::account_permission(
                        permission,
                        account.id().clone(),
                    ))
                    .unwrap();
                }
            }
        }

        set_data_model(&ExecutorDataModel::new(
            self.parameters,
            self.instructions,
            self.permissions,
            norito::json::to_value(&self.schema)
                .expect("INTERNAL BUG: Failed to serialize Executor data model entity")
                .into(),
        ));
    }
}

/// Executor of Iroha operations
pub trait Execute {
    /// Handle to the host environment.
    fn host(&self) -> &Iroha;

    /// Context of the execution.
    ///
    /// Represents the current state of the world
    fn context(&self) -> &prelude::Context;

    /// Mutable context for e.g. switching to another authority after validation before execution.
    /// Note that mutations are persistent to the instance unless reset
    fn context_mut(&mut self) -> &mut prelude::Context;

    /// Executor verdict.
    fn verdict(&self) -> &Result;

    /// Set executor verdict to deny
    fn deny(&mut self, reason: ValidationFail);
}

pub mod prelude {
    //! Contains useful re-exports

    pub use std::vec::Vec;

    pub use iroha_executor_derive::{Entrypoints, Execute, Visit};

    pub use crate::{
        DataModelBuilder, DebugExpectExt, DebugUnwrapExt, Execute, Iroha,
        data_model::{
            executor::Result, prelude::*, smart_contract::payloads::ExecutorContext as Context,
            visit::Visit,
        },
        dbg, dbg_panic, deny, execute, runtime,
    };
}

#[cfg(test)]
mod tests {
    use core::{
        mem::ManuallyDrop,
        sync::atomic::{AtomicBool, Ordering},
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        slice,
    };

    #[cfg(feature = "fast_dsl")]
    use data_model::query::QueryItemKind;
    use data_model::{
        permission::Permission,
        prelude::Json,
        query::{
            QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryRequest,
            QueryResponse, SingularQueryOutputBox,
        },
    };

    use super::*;

    static CALLED: AtomicBool = AtomicBool::new(false);

    #[cfg(not(feature = "fast_dsl"))]
    macro_rules! iter_query_empty_batch {
        ($query_box:expr; $($ty:ty => $variant:ident),+ $(,)?) => {{
            'found: {
                $(
                    if data_model::query::iter_query_inner::<$ty>($query_box).is_some() {
                        break 'found QueryOutputBatchBox::$variant(Vec::new());
                    }
                )+
                QueryOutputBatchBox::Permission(Vec::new())
            }
        }};
    }

    #[cfg(not(feature = "fast_dsl"))]
    fn empty_iterable_batch_non_fast(
        query: &data_model::query::QueryWithParams,
    ) -> QueryOutputBatchBox {
        let query_box = query
            .query_box()
            .expect("non-fast_dsl query must provide query box");
        iter_query_empty_batch!(
            query_box;
            data_model::domain::Domain => Domain,
            data_model::account::Account => Account,
            data_model::asset::value::Asset => Asset,
            data_model::asset::definition::AssetDefinition => AssetDefinition,
            data_model::repo::RepoAgreement => RepoAgreement,
            data_model::nft::Nft => Nft,
            data_model::role::Role => Role,
            data_model::role::RoleId => RoleId,
            data_model::peer::PeerId => Peer,
            data_model::trigger::TriggerId => TriggerId,
            data_model::trigger::Trigger => Trigger,
            data_model::query::CommittedTransaction => CommittedTransaction,
            data_model::block::SignedBlock => Block,
            data_model::block::BlockHeader => BlockHeader,
            data_model::proof::ProofRecord => ProofRecord,
            data_model::permission::Permission => Permission,
            data_model::offline::OfflineAllowanceRecord => OfflineAllowanceRecord,
            data_model::offline::OfflineTransferRecord => OfflineToOnlineTransfer,
            data_model::offline::OfflineCounterSummary => OfflineCounterSummary,
            data_model::offline::OfflineVerdictRevocation => OfflineVerdictRevocation,
        )
    }

    #[cfg(feature = "fast_dsl")]
    fn empty_iterable_batch_fast(
        query: &data_model::query::QueryWithParams,
    ) -> QueryOutputBatchBox {
        match query.item {
            QueryItemKind::Domain => QueryOutputBatchBox::Domain(Vec::new()),
            QueryItemKind::Account => QueryOutputBatchBox::Account(Vec::new()),
            QueryItemKind::Asset => QueryOutputBatchBox::Asset(Vec::new()),
            QueryItemKind::AssetDefinition => QueryOutputBatchBox::AssetDefinition(Vec::new()),
            QueryItemKind::RepoAgreement => QueryOutputBatchBox::RepoAgreement(Vec::new()),
            QueryItemKind::Nft => QueryOutputBatchBox::Nft(Vec::new()),
            QueryItemKind::Role => QueryOutputBatchBox::Role(Vec::new()),
            QueryItemKind::RoleId => QueryOutputBatchBox::RoleId(Vec::new()),
            QueryItemKind::PeerId => QueryOutputBatchBox::Peer(Vec::new()),
            QueryItemKind::TriggerId => QueryOutputBatchBox::TriggerId(Vec::new()),
            QueryItemKind::Trigger => QueryOutputBatchBox::Trigger(Vec::new()),
            QueryItemKind::CommittedTransaction => {
                QueryOutputBatchBox::CommittedTransaction(Vec::new())
            }
            QueryItemKind::SignedBlock => QueryOutputBatchBox::Block(Vec::new()),
            QueryItemKind::BlockHeader => QueryOutputBatchBox::BlockHeader(Vec::new()),
            QueryItemKind::ProofRecord => QueryOutputBatchBox::ProofRecord(Vec::new()),
            QueryItemKind::Permission => QueryOutputBatchBox::Permission(Vec::new()),
            QueryItemKind::OfflineAllowanceRecord => {
                QueryOutputBatchBox::OfflineAllowanceRecord(Vec::new())
            }
            QueryItemKind::OfflineToOnlineTransfer => {
                QueryOutputBatchBox::OfflineToOnlineTransfer(Vec::new())
            }
            QueryItemKind::OfflineCounterSummary => {
                QueryOutputBatchBox::OfflineCounterSummary(Vec::new())
            }
            QueryItemKind::OfflineVerdictRevocation => {
                QueryOutputBatchBox::OfflineVerdictRevocation(Vec::new())
            }
        }
    }

    fn empty_iterable_batch(
        query: &data_model::query::QueryWithParams,
    ) -> QueryOutputBatchBoxTuple {
        #[cfg(not(feature = "fast_dsl"))]
        let batch = empty_iterable_batch_non_fast(query);

        #[cfg(feature = "fast_dsl")]
        let batch = empty_iterable_batch_fast(query);

        QueryOutputBatchBoxTuple::new(vec![batch])
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn execute_instruction(ptr: *const u8, len: usize) -> *const u8 {
        let _ = unsafe { slice::from_raw_parts(ptr, len) };
        let body =
            norito::to_bytes(&Result::<(), ValidationFail>::Ok(())).expect("encode instruction ok");
        unsafe { encode_with_len_prefix(&body) }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn execute_query(ptr: *const u8, len: usize) -> *const u8 {
        let bytes = unsafe { slice::from_raw_parts(ptr, len) };
        let query_request = norito::decode_from_bytes::<QueryRequest>(bytes).ok();

        let response: Result<QueryResponse, ValidationFail> = Ok(match query_request {
            Some(QueryRequest::Singular(_)) => QueryResponse::Singular(
                SingularQueryOutputBox::Parameters(data_model::parameter::Parameters::default()),
            ),
            Some(QueryRequest::Start(query)) => {
                QueryResponse::Iterable(QueryOutput::new(empty_iterable_batch(&query), 0, None))
            }
            Some(QueryRequest::Continue(_)) | None => QueryResponse::Iterable(QueryOutput::new(
                QueryOutputBatchBoxTuple::new(vec![QueryOutputBatchBox::Permission(Vec::new())]),
                0,
                None,
            )),
        });
        let body = norito::to_bytes(&response).expect("encode query ok");
        unsafe { encode_with_len_prefix(&body) }
    }

    unsafe fn encode_with_len_prefix(body: &[u8]) -> *const u8 {
        let len_size = core::mem::size_of::<usize>();
        let mut out = Vec::with_capacity(len_size + body.len());
        out.extend_from_slice(&(len_size + body.len()).to_le_bytes());
        out.extend_from_slice(body);
        ManuallyDrop::new(out.into_boxed_slice()).as_ptr()
    }

    #[cfg(test)]
    pub fn with_mock_permissions<R>(permissions: Vec<Permission>, f: impl FnOnce() -> R) -> R {
        struct RestoreGuard {
            previous: Option<Vec<Permission>>,
        }

        impl Drop for RestoreGuard {
            fn drop(&mut self) {
                if let Some(previous) = self.previous.take() {
                    let _ = crate::permission::test_override::replace_permissions(previous);
                }
            }
        }

        let previous = crate::permission::test_override::replace_permissions(permissions);
        let _restore = RestoreGuard {
            previous: Some(previous),
        };

        f()
    }

    unsafe extern "C" fn dummy(_: *const u8, _: usize) {
        CALLED.store(true, Ordering::Relaxed);
    }

    #[test]
    fn encode_executor_data_model() {
        let data_model = data_model::ExecutorDataModel::new(
            BTreeMap::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            Json::new(()),
        );

        unsafe { iroha_smart_contract_utils::encode_and_execute(&data_model, dummy) };
        assert!(CALLED.load(Ordering::Relaxed));
    }
}
