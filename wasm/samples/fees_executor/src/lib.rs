//! Iroha executor with fees support.

#![no_std]

#[cfg(not(test))]
extern crate panic_halt;

extern crate alloc;
use alloc::format;
use iroha_executor::{
    data_model::{isi::CustomInstruction, query::builder::SingleQueryError, parameter::{CustomParameter, Parameter}},
    prelude::*, permission::ExecutorPermission as _,
};
use fees_executor_data_model::{parameters::*};
use iroha_schema::IntoSchema;
use serde::{Deserialize, Serialize};
use iroha_executor_data_model::parameter::Parameter as ExecutorParameter;

use dlmalloc::GlobalDlmalloc;
use iroha_executor::{data_model::block::BlockHeader, prelude::*};

#[global_allocator]
static ALLOC: GlobalDlmalloc = GlobalDlmalloc;

#[derive(Debug, Clone, Visit, Execute, Entrypoints)]
#[visit(custom(
    visit_set_parameter,
))]
struct Executor {
    host: Iroha,
    context: Context,
    verdict: Result,
}

fn find_fee_options(executor: &mut Executor) -> FeesOptions {
    let parameters = executor.host().query_single(FindParameters).dbg_unwrap();

    let fees_options: FeesOptions = parameters
        .custom()
        .get(&FeesOptions::id())
        .unwrap()
        .try_into()
        .expect("INTERNAL BUG: Failed to deserialize json as `FeesOptions`");

    fees_options
}

fn visit_set_parameter(executor: &mut Executor, isi: &SetParameter) {
    if let Parameter::Custom(param) = isi.parameter() {
        if param.id() == &FeesOptions::id() {
            let payload = param.payload();
            match payload.try_into_any::<FeesOptions>() {
                Ok(options) => {
                    // Account should exist
                    executor
                    .host()
                    .query(FindAccounts::new())
                    .filter_with(|account| account.id.eq(options.receiver))
                    .execute_single()
                    .dbg_unwrap();

                    // Asset should exist
                    executor
                    .host()
                    .query(FindAssetsDefinitions::new())
                    .filter_with(|asset| asset.id.eq(options.asset))
                    .execute_single()
                    .dbg_unwrap();
                },
                Err(_) => {
                    deny!(executor, "Invalid fees options");
                }
            }
            iroha_executor::log::info!(&format!("Updating fee parameters: {}", payload));
        }
    }

    execute!(executor, isi);
}


#[iroha_executor::migrate]
fn migrate(host: Iroha, context: Context) {
    let options = FeesOptions::default();
    
    DataModelBuilder::with_default_permissions()
    .add_parameter(options)
    .build_and_set(&host);
}
