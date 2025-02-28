//! Iroha executor with fees support.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;

use dlmalloc::GlobalDlmalloc;
use fees_executor_data_model::parameters::*;
use iroha_executor::prelude::*;
use iroha_executor_data_model::parameter::Parameter as _;

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[derive(Visit, Execute, Entrypoints, Debug, Clone)]
#[visit(custom(
    visit_set_parameter,
    visit_unregister_domain,
    visit_unregister_asset_definition,
    visit_unregister_account,
))]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

/// Finds currently used fees options
fn finds_fee_options(host: &Iroha) -> FeesOptions {
    let parameters = host.query_single(FindParameters).dbg_unwrap();

    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .expect("INTERNAL BUG: Failed to deserialize json as `FeesOptions`");

    fees_options
}

/// Errors as a result of fees options validation
#[derive(Debug)]
enum FeesOptionsValidationError {
    /// Invalid Asset
    Asset,
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
        .map_err(|_| FeesOptionsValidationError::Asset)
}

fn visit_set_parameter(executor: &mut Executor, isi: &SetParameter) {
    if let Parameter::Custom(param) = isi.parameter() {
        // Fees options are read-only
        if param.id().eq(&FeesOptions::id()) {
            deny!(executor, "Fees options cannot be changed");
        }
    }

    execute!(executor, isi);
}

fn visit_unregister_domain(executor: &mut Executor, isi: &Unregister<Domain>) {
    let fees_options = finds_fee_options(&executor.host());

    if isi.object().eq(fees_options.asset.account().domain())
        || isi.object().eq(fees_options.asset.definition().domain())
    {
        deny!(
            executor,
            "Domain associated with technical account cannot be unregistered"
        );
    }

    execute!(executor, isi);
}

fn visit_unregister_asset_definition(executor: &mut Executor, isi: &Unregister<AssetDefinition>) {
    let fees_options = finds_fee_options(&executor.host());

    if isi.object().eq(fees_options.asset.definition()) {
        deny!(
            executor,
            "Asset definition associated with technical account cannot be unregistered"
        );
    }

    execute!(executor, isi);
}

fn visit_unregister_account(executor: &mut Executor, isi: &Unregister<Account>) {
    let fees_options = finds_fee_options(&executor.host());

    if isi.object().eq(fees_options.asset.account()) {
        deny!(
            executor,
            "Account associated with technical account cannot be unregistered"
        );
    }

    execute!(executor, isi);
}

#[iroha_executor::migrate]
fn migrate(host: Iroha, context: Context) {
    let options = FeesOptions::default();

    validate_fees_options(&host, &options).dbg_unwrap();

    DataModelBuilder::with_default_permissions()
        .add_parameter(options)
        .build_and_set(&host);
}
