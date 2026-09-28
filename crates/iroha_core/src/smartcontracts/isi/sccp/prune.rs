//! Bounded post-execution pruning (`specs/sccp.md` §4.10). Owner: ws30.
//!
//! Attestation signatures are pruned by block time after `attestation_retention_ms`, rotation
//! subjects are kept until the outgoing roster expires plus one day, and light-client
//! checkpoints follow §4.13.1. At most [`MAX_PRUNE_DELETIONS_PER_BLOCK`] records are deleted
//! per block; `sccp_prune_cursor` resumes the scan.

use super::not_wired_block;
use crate::{block::BlockValidationError, state::StateBlock};

/// Per-block work bound of the pruning step (§4.10).
pub const MAX_PRUNE_DELETIONS_PER_BLOCK: usize = 1_024;

/// Prune expired SCCP records at block time `now_ms`, deleting at most `budget` records, and
/// return the number deleted.
///
/// # Errors
///
/// Fails closed until ws30 implements pruning.
pub fn prune(
    state_block: &mut StateBlock<'_>,
    now_ms: u64,
    budget: usize,
) -> Result<usize, BlockValidationError> {
    let _ = (state_block, now_ms, budget);
    // TODO(ws30): signatures by retention, rotation subjects, then `light_clients::prune`.
    Err(not_wired_block("pruning", "ws30"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn pruning_fails_closed_until_implemented() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let error = prune(&mut block, 0, MAX_PRUNE_DELETIONS_PER_BLOCK).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws30)"), "{error}");
    }
}
