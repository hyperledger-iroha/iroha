//! Attestation subjects and statements (`specs/sccp.md` §3.6, §4.6). Owner: ws30.
//!
//! The statement of height `h` is its stored subject plus `block_hashes[h]`; its digest is the
//! §3.6 EIP-712 attestation digest under the live `NetworkId`.

use crate::state::StateReadOnly;
use iroha_data_model::sccp::attestation::SccpAttestationStatementV1;

/// Return the canonical statement of `height`, or `None` when no subject exists there.
#[must_use]
pub fn statement(view: &impl StateReadOnly, height: u64) -> Option<SccpAttestationStatementV1> {
    let _ = (view, height);
    // TODO(ws30): subject + block hash (§4.6).
    None
}

/// Return the §3.6 attestation digest of `height`'s statement under the live `NetworkId`.
#[must_use]
pub fn statement_digest(view: &impl StateReadOnly, height: u64) -> Option<[u8; 32]> {
    let _ = (view, height);
    // TODO(ws30): `iroha_sccp::v1::eip712` attestation digest (§3.6).
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::blank_state;

    #[test]
    fn no_statement_exists_without_a_subject() {
        let state = blank_state();
        let view = state.view();
        assert_eq!(statement(&view, 1), None);
        assert_eq!(statement_digest(&view, 1), None);
    }
}
