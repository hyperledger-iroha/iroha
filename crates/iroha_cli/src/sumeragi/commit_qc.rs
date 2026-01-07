#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;

use crate::RunContext;

use super::commands::CommitQcGetArgs;

pub(crate) fn get<C: RunContext>(context: &mut C, args: CommitQcGetArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_commit_qc_json(&args.hash)?;
    context.print_data(&value)
}
