//! One-shot provenance for a gateway action in an exact directly signed Network entry.

use crate::state::StateTransaction;
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId, executor::ValidationFail,
    sorafs::stream_token_gateway::native::StreamTokenGatewayExecutionV1,
    transaction::TransactionEntrypoint,
};

fn rejected() -> ValidationFail {
    ValidationFail::NotPermitted(
        "native stream-token gateway requires an exact directly signed instruction".to_owned(),
    )
}

/// Consume the gateway-only ordinal and derive provenance from the actual execution context.
///
/// The executor creates this marker only after matching the complete native instruction at
/// its original signed position. Consumption precedes all checks, so a rejected action cannot
/// lend its marker to a later or nested call. This function does not grant gateway permissions
/// or authenticate finalized readback.
pub(crate) fn execution(
    tx: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
) -> Result<StreamTokenGatewayExecutionV1, ValidationFail> {
    let instruction_index = tx
        .current_direct_stream_token_gateway_instruction_index
        .take()
        .ok_or_else(rejected)?;
    let outer: HashOf<TransactionEntrypoint> =
        tx.current_network_entrypoint_hash.ok_or_else(rejected)?;
    let inner = tx.tx_call_hash.ok_or_else(rejected)?;
    if outer != HashOf::from_untyped_unchecked(inner) || tx.current_tx_hash.is_none() {
        return Err(rejected());
    }
    let height = tx._curr_block.height().get();
    let parent = u64::try_from(tx.block_hashes().len()).map_err(|_| rejected())?;
    let entry_index = tx
        .current_entrypoint_index
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(rejected)?;
    let recorded_at_unix_ms = tx.block_unix_timestamp_ms();
    if height != parent.checked_add(1).ok_or_else(rejected)?
        || recorded_at_unix_ms == 0
        || recorded_at_unix_ms == u64::MAX
    {
        return Err(rejected());
    }
    Ok(StreamTokenGatewayExecutionV1 {
        height,
        transaction_hash: *outer.as_ref(),
        entry_index,
        instruction_index,
        recorded_at_unix_ms,
        authority: authority.clone(),
    })
}
