//! Bounded prepare/inspect/dispatch/recovery for one exact native parameter write.

use crate::{RunContext, read_cli_text_file_bounded};
use eyre::{Result, eyre};
use iroha::data_model::{asset::AssetDefinitionId, parameter::Parameter};
use iroha_primitives::numeric::Quantity;
use iroha_wallet::operations::{AccountService, BoundedTransactionOptions, ParameterUpdateRequest};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// The independently retained original request used for every journal action.
#[derive(clap::Args, Debug)]
pub(crate) struct RequestArgs {
    /// Exact original parameter JSON file.
    #[arg(long, value_name = "PATH")]
    pub parameter: PathBuf,
    #[command(flatten)]
    pub bounds: Bounds,
}

/// One selected private journal and explicit fee/deadline bounds.
#[derive(clap::Args, Debug)]
pub(crate) struct Bounds {
    /// Fresh journal for prepare; the identical retained journal for later actions.
    #[arg(long, value_name = "DIRECTORY")]
    pub journal: PathBuf,
    /// Native JSON array of {asset_definition_id,max_amount}, sorted by exact asset ID.
    #[arg(long, value_name = "PATH")]
    pub fee_maximums: PathBuf,
    /// Deadline for this action's I/O; recovery preserves the original signed lifetime.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=900_000))]
    pub timeout_ms: u64,
}

#[derive(norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Maximum {
    asset_definition_id: AssetDefinitionId,
    max_amount: Quantity,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    Prepare,
    Inspect,
    Submit,
    Resume,
}

pub(crate) fn request<C: RunContext>(
    context: &C,
    parameter: Parameter,
    bounds: &Bounds,
) -> Result<ParameterUpdateRequest> {
    let maxima: Vec<Maximum> = read_json(&bounds.fee_maximums, "original parameter fee maxima")?;
    if maxima.len() > 16
        || maxima
            .windows(2)
            .any(|pair| pair[0].asset_definition_id >= pair[1].asset_definition_id)
        || maxima.iter().any(|item| item.max_amount.is_zero())
    {
        return Err(eyre!(
            "parameter fee maxima must be bounded, positive and strictly ordered"
        ));
    }
    let max_total_fees: BTreeMap<_, _> = maxima
        .into_iter()
        .map(|item| (item.asset_definition_id, item.max_amount))
        .collect();
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(bounds.timeout_ms))
        .ok_or_else(|| eyre!("parameter action deadline overflow"))?;
    Ok(ParameterUpdateRequest {
        parameter,
        options: BoundedTransactionOptions {
            fee_payment: context.transaction_fee_payment()?,
            max_total_fees,
            deadline,
        },
    })
}

pub(crate) fn read_json<T: norito::json::JsonDeserialize>(path: &Path, label: &str) -> Result<T> {
    crate::parse_json(&read_cli_text_file_bounded(path, label)?)
        .map_err(|error| eyre!("invalid {label}: {error}"))
}

pub(crate) fn run<C: RunContext>(context: &mut C, action: Action, args: RequestArgs) -> Result<()> {
    let parameter = read_json(&args.parameter, "original native parameter")?;
    run_parameter(context, action, parameter, &args.bounds)
}

pub(crate) fn run_parameter<C: RunContext>(
    context: &mut C,
    action: Action,
    parameter: Parameter,
    bounds: &Bounds,
) -> Result<()> {
    if context.input_instructions() || context.output_instructions() {
        return Err(eyre!(
            "prepared parameter actions require their private journal, not instruction piping"
        ));
    }
    let request = request(context, parameter, bounds)?;
    let account =
        AccountService::new(context.config().clone())?.with_deadline(request.options.deadline)?;
    let report = match action {
        Action::Prepare => account.prepare_parameter_update(&request, &bounds.journal)?,
        Action::Inspect => account.inspect_parameter_update(&bounds.journal, &request)?,
        Action::Submit => account.submit_parameter_update(&bounds.journal, &request)?,
        Action::Resume => account.resume_parameter_update(&bounds.journal, &request)?,
    };
    context.print_data(&report.data)
}
