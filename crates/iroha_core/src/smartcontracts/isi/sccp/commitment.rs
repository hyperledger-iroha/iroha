//! Block commitment and history accumulator (`specs/sccp.md` §3.4, §3.5, §4.5 step 1).
//! Owner: ws30.
//!
//! After all transactions of block `h`, the applied leaf outbox `sccp_block_leaves[(h, ·)]`
//! must hold exactly the indices `0..m`. When `m > 0`, the block root is computed over the
//! stored leaves, `history_leaf(h, root, m)` is appended to the accumulator and
//! `sccp_block_commitments[h]` is written.

use super::not_wired_block;
use crate::{block::BlockValidationError, state::StateBlock};
use iroha_data_model::sccp::attestation::SccpBlockCommitmentV1;

/// Commit the leaves of block `height`, returning its commitment when it holds any leaf.
///
/// # Errors
///
/// Fails closed until ws30 implements the commitment step.
pub fn commit_block(
    state_block: &mut StateBlock<'_>,
    height: u64,
) -> Result<Option<SccpBlockCommitmentV1>, BlockValidationError> {
    let _ = (state_block, height);
    // TODO(ws30): dense-index assertion, §3.4 root, §3.5 history append (§4.5 step 1).
    Err(not_wired_block("block commitment", "ws30"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn commitment_fails_closed_until_implemented() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let error = commit_block(&mut block, 2).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws30)"), "{error}");
    }
}
