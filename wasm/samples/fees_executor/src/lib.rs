//! Iroha executor with fees support.
//! 
//! Namings:
//! Treasury - asset that is used to pay fees.
//! 
//! Only trasury account specified in the fees configuration can grant/revoke `CanModifyFeesOptions` permission.
//! Accounts with `CanModifyFeesOptions` can change default and individuals's fee amounts.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use dlmalloc::GlobalDlmalloc;
use fees_executor_data_model::{isi::*, parameters::*, permissions::*};
use iroha_data_model::parameter::CustomParameter;
use iroha_executor::{permission::ExecutorPermission as _, prelude::*};
use iroha_executor_data_model::parameter::Parameter as _;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[derive(Visit, Execute, Entrypoints, Debug, Clone)]
#[visit(custom(
    visit_set_parameter,
    visit_set_account_key_value,
    visit_unregister_domain,
    visit_unregister_asset_definition,
    visit_register_account,
    visit_unregister_account,
    visit_custom_instruction,
    visit_grant_role_permission,
    visit_revoke_role_permission,
    visit_grant_account_permission,
    visit_revoke_account_permission,
))]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

/// Finds currently used default fees options
fn find_default_fees_options(host: &Iroha) -> FeesOptions {
    let parameters = host.query_single(FindParameters).dbg_expect("Failed to get parameters");

    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .expect("INTERNAL BUG: Fees executor must set FeesOptions during migration")
        .try_into()
        .expect("INTERNAL BUG: Failed to deserialize json as `FeesOptions`");

    fees_options
}

/// Errors as a result of fees options validation
#[derive(Debug, thiserror::Error)]
enum FeesOptionsValidationError {
    #[error("Could not find asset specified in the fee options {0}")]
    AssetNotFound(AssetId),
}

/// Validates fees options against the current network
fn validate_fees_options(
    host: &Iroha,
    options: &FeesOptions,
) -> Result<(), FeesOptionsValidationError> {
    host.query(FindAssets)
        .filter_with(|asset| asset.id.eq(options.asset.clone()))
        .execute_single()
        .map(|_| ())
        .map_err(|_| FeesOptionsValidationError::AssetNotFound(options.asset.clone()))
}

fn visit_set_parameter(executor: &mut Executor, isi: &SetParameter) {
    if let Parameter::Custom(param) = isi.parameter() {
        if param.id().eq(&FeesOptions::id()) {
            deny!(executor, "Default fees options cannot be changed directly; use `SetDefaultFeesAmountsOptions` ISI");
        }
    }

    iroha_executor::default::visit_set_parameter(executor, isi)
}

fn visit_set_account_key_value(executor: &mut Executor, isi: &SetKeyValue<Account>) {
    if isi.key().eq(FeesAmountsOptions::id().name()) {
        deny!(executor, "Account fees options cannot be changed directly; use `SetAccountFeesAmountsOptions` ISI");
    }

    iroha_executor::default::visit_set_account_key_value(executor, isi)
}

fn visit_unregister_domain(executor: &mut Executor, isi: &Unregister<Domain>) {
    let fees_options = find_default_fees_options(&executor.host());

    if isi.object().eq(fees_options.asset.account().domain())
        || isi.object().eq(fees_options.asset.definition().domain())
    {
        deny!(
            executor,
            "Domain associated with treasury account cannot be unregistered"
        );
    }

    iroha_executor::default::visit_unregister_domain(executor, isi)
}

fn visit_unregister_asset_definition(executor: &mut Executor, isi: &Unregister<AssetDefinition>) {
    let fees_options = find_default_fees_options(&executor.host());

    if isi.object().eq(fees_options.asset.definition()) {
        deny!(
            executor,
            "Asset definition associated with treasury asset cannot be unregistered"
        );
    }

    iroha_executor::default::visit_unregister_asset_definition(executor, isi)
}

fn visit_unregister_account(executor: &mut Executor, isi: &Unregister<Account>) {
    let fees_options = find_default_fees_options(&executor.host());

    if isi.object().eq(fees_options.asset.account()) {
        deny!(
            executor,
            "Account associated with treasury asset cannot be unregistered"
        );
    }

    iroha_executor::default::visit_unregister_account(executor, isi)
}

fn visit_custom_instruction(executor: &mut Executor, isi: &CustomInstruction) {
    let Ok(isi) = FeesInstructionBox::try_from(isi.payload()) else {
        deny!(executor, "Failed to parse custom instruction");
    };
    visit_fees_instruction(isi, executor)
}

fn visit_fees_instruction(isi: FeesInstructionBox, executor: &mut Executor) {
    match isi {
        FeesInstructionBox::SetDefaultFeesAmountsOptions(isi) => {
            visit_set_default_fees_amounts_options(executor, isi)
        }
        FeesInstructionBox::SetAccountFeesAmountsOptions(isi) => {
            visit_set_account_fees_amounts_options(executor, isi)
        }
    }
}

fn visit_set_default_fees_amounts_options(
    executor: &mut Executor,
    isi: SetDefaultFeesAmountsOptions,
) {
    if executor.context().curr_block.is_genesis()
        || CanModifyFeesOptions.is_owned_by(&executor.context().authority, executor.host())
    {
        let mut fees_options = find_default_fees_options(&executor.host());
        fees_options.amounts = isi.0;
        let update_options = &SetParameter::new(Parameter::Custom(fees_options.into()));
        execute!(executor, update_options);
    }

    deny!(
        executor,
        "Account doesn't have permission to modify default fee amounts"
    );
}

/// Set fees options for the account.
/// No permission checks are performed.
fn set_account_fees_amounts(
    host: &Iroha,
    account: AccountId,
    options: FeesAmountsOptions,
) -> Result<(), ValidationFail> {
    let update_options = &SetKeyValue::account(
        account,
        FeesAmountsOptions::id().name().clone(),
        Into::<CustomParameter>::into(options).payload().clone(),
    );
    host.submit(update_options)
}

fn visit_set_account_fees_amounts_options(
    executor: &mut Executor,
    isi: SetAccountFeesAmountsOptions,
) {
    if executor.context().curr_block.is_genesis()
        || CanModifyFeesOptions.is_owned_by(&executor.context().authority, executor.host())
    {
        set_account_fees_amounts(executor.host(), isi.account, isi.options).dbg_expect("Failed to set account fees options")
    }

    deny!(
        executor,
        "Account doesn't have permission to modify account fee amounts"
    );
}

fn visit_register_account(executor: &mut Executor, isi: &Register<Account>) {
    iroha_executor::default::visit_register_account(executor, isi);
    if !executor.verdict().is_ok() {
        return;
    }

    let fees_options = find_default_fees_options(&executor.host());
    set_account_fees_amounts(executor.host(), isi.object().id().clone(), fees_options.amounts).dbg_expect("Failed to set account fees options")
}

fn visit_grant_role_permission(executor: &mut Executor, isi: &Grant<Permission, Role>) {
    let role_id = isi.destination().clone();

    if let Ok(permission) = CanModifyFeesOptions::try_from(isi.object()) {
        if can_update_fees_permissions(executor, executor.context().authority.clone()) {
            let isi = &Grant::role_permission(permission, role_id);
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Account doesn't have permission to grant `CanModifyFeesOptions` permission"
        );
    }

    iroha_executor::default::visit_grant_role_permission(executor, isi)
}

fn visit_revoke_role_permission(executor: &mut Executor, isi: &Revoke<Permission, Role>) {
    let role_id = isi.destination().clone();

    if let Ok(permission) = CanModifyFeesOptions::try_from(isi.object()) {
        if can_update_fees_permissions(executor, executor.context().authority.clone()) {
            let isi = &Revoke::role_permission(permission, role_id);
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Account doesn't have permission to revoke `CanModifyFeesOptions` permission"
        );
    }

    iroha_executor::default::visit_revoke_role_permission(executor, isi)
}

fn visit_grant_account_permission(executor: &mut Executor, isi: &Grant<Permission, Account>) {
    let account_id = isi.destination().clone();

    if let Ok(permission) = CanModifyFeesOptions::try_from(isi.object()) {
        if can_update_fees_permissions(executor, executor.context().authority.clone()) {
            let isi = &Grant::account_permission(permission, account_id);
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Account doesn't have permission to grant `CanModifyFeesOptions` permission"
        );
    }

    iroha_executor::default::visit_grant_account_permission(executor, isi)
}

fn visit_revoke_account_permission(executor: &mut Executor, isi: &Revoke<Permission, Account>) {
    let account_id = isi.destination().clone();

    if let Ok(permission) = CanModifyFeesOptions::try_from(isi.object()) {
        if can_update_fees_permissions(executor, executor.context().authority.clone()) {
            let isi = &Revoke::account_permission(permission, account_id);
            execute!(executor, isi);
        }
        deny!(
            executor,
            "Account doesn't have permission to revoke `CanModifyFeesOptions` permission"
        );
    }

    iroha_executor::default::visit_revoke_account_permission(executor, isi)
}

/// Check if account can control who has permissions to change accounts' fees
fn can_update_fees_permissions(executor: &mut Executor, account: AccountId) -> bool {
    let fees_options = find_default_fees_options(&executor.host());
    executor.context().curr_block.is_genesis()
        || account.eq(fees_options.asset.account())
}

/// Update all accounts' metadata to include default fee options.
/// Executed once during the executor migration.
fn apply_config_for_all(host: &Iroha, options: FeesAmountsOptions) {
    let accounts = host
        .query(FindAccounts)
        .execute()
        .dbg_expect("Failed to execute `FindAllAccounts`")
        .map(|account| {
            account
                .dbg_expect("Failed to get account from cursor")
                .id()
                .clone()
        });

    for account in accounts {
        set_account_fees_amounts(host, account, options.clone()).dbg_expect("Failed to set account fees options");
    }
}

#[iroha_executor::migrate]
fn migrate(host: Iroha, context: Context) {
    let options = FeesOptions::default();

    validate_fees_options(&host, &options).dbg_unwrap();

    DataModelBuilder::with_default_permissions()
        .add_parameter(options.clone())
        .add_permission::<CanModifyFeesOptions>()
        .add_instruction::<SetDefaultFeesAmountsOptions>()
        .add_instruction::<SetAccountFeesAmountsOptions>()
        .build_and_set(&host);

    host.submit(&Grant::account_permission(
        CanModifyFeesOptions,
        options.asset.account().clone(),
    ))
    .dbg_expect("Fail to grant `CanModifyFeesOptions` to the treasury");

    apply_config_for_all(&host, options.amounts.clone());
}
