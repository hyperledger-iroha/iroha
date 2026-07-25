//! Shared finalized-chain context checks for authoritative SoraFS query pages.

use iroha_data_model::{
    query::error::QueryExecutionFail, sorafs::finality::SorafsFinalizedPageContextV1,
};

use crate::state::StateReadOnly;

/// Resolve chain identity and block time for an already-derived finalized cursor.
///
/// The block-hash vector alone cannot supply authoritative time and can outlive
/// an unavailable Kura header. Every finalized event page therefore proves that
/// the loaded terminal block is the exact height/hash named by its domain cursor.
pub(crate) fn resolve_sorafs_finalized_page_context(
    state_ro: &impl StateReadOnly,
    expected_height: u64,
    expected_block_hash: [u8; 32],
    domain: &str,
) -> Result<SorafsFinalizedPageContextV1, QueryExecutionFail> {
    let latest_block = state_ro.latest_block().ok_or_else(|| {
        QueryExecutionFail::Conversion(format!(
            "finalized SoraFS {domain} query requires the terminal block header"
        ))
    })?;
    let header = latest_block.header();
    let header_height = header.height().get();
    let header_hash = *latest_block.hash().as_ref();
    if header_height != expected_height || header_hash != expected_block_hash {
        return Err(QueryExecutionFail::Conversion(format!(
            "finalized SoraFS {domain} cursor does not match the terminal block header"
        )));
    }
    if state_ro.chain_id().as_str().is_empty() {
        return Err(QueryExecutionFail::Conversion(format!(
            "finalized SoraFS {domain} query resolved an empty chain identifier"
        )));
    }
    let finalized_at_unix_ms = u64::try_from(header.creation_time().as_millis()).map_err(|_| {
        QueryExecutionFail::Conversion(format!(
            "finalized SoraFS {domain} block time does not fit into u64 milliseconds"
        ))
    })?;

    Ok(SorafsFinalizedPageContextV1 {
        chain_id: state_ro.chain_id().clone(),
        finalized_at_unix_ms,
    })
}
