#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;

use crate::RunContext;

use super::commands::{ExecQcGetArgs, ExecRootGetArgs};

pub(crate) fn qc<C: RunContext>(context: &mut C, args: ExecQcGetArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_exec_qc_json(&args.hash)?;
    context.print_data(&value)
}

pub(crate) fn root<C: RunContext>(context: &mut C, args: ExecRootGetArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_exec_root_json(&args.hash)?;
    context.print_data(&value)
}
