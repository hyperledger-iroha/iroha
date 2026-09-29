//! Exact native proposal source identity and mutation rejection before execution.

use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

#[test]
fn native_header_source_binds_complete_original_wire_and_header_context() {
    let mut chain =
        CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1_000)).unwrap();
    chain.commit_at(2_000, Vec::new());
    let committed = chain.committed(2);
    let header = committed.header().unwrap();
    let proposal = committed.block().canonical_resultless_proposal();
    let payload = proposal.encode_wire().unwrap();
    let source =
        ValidBlock::native_header_source(&proposal, chain.state(), header, &payload).unwrap();
    assert_eq!(source.header(), proposal.header());
    assert!(std::ptr::eq(source.state(), chain.state().as_ref()));
    assert_eq!(source.generation(), chain.state().state_view_generation());
    assert_eq!(
        source.consensus_hash,
        Hash::prehashed(committed.core_hash().0)
    );
    assert_eq!(source.expected_context.instance, header.instance.0);
    assert_eq!(
        source.expected_context.epoch_context_id,
        header.epoch.context.0
    );
    assert_eq!(
        source.expected_context.parent_consensus_hash,
        header.parent_hash.0
    );
    assert_eq!(
        source.expected_context.parent_result,
        header.parent_result.0
    );
    source.validate_body(&proposal).unwrap();
    assert!(
        source.validate_body(committed.block()).is_err(),
        "output-bearing copy is not the original proposal"
    );
    for offset in [0, payload.len() / 2, payload.len() - 1] {
        let mut changed = payload.clone();
        changed[offset] ^= 1;
        assert!(
            ValidBlock::native_header_source(&proposal, chain.state(), header, &changed).is_err()
        );
    }
    let mut trailing = payload.clone();
    trailing.push(0);
    assert!(ValidBlock::native_header_source(&proposal, chain.state(), header, &trailing).is_err());
    for changed in 0..5 {
        let mut header = header.clone();
        match changed {
            0 => header.height += 1,
            1 => header.origin_view += 1,
            2 => header.payload_len += 1,
            3 => header.payload_hash.0[0] ^= 1,
            4 => {
                header.control_witness =
                    iroha_sumeragi::types::ControlWitness::try_from_slice(&[0xff]).unwrap()
            }
            _ => unreachable!(),
        }
        assert!(
            ValidBlock::native_header_source(&proposal, chain.state(), &header, &payload).is_err()
        );
    }
}
