//! Public-boundary tests for native Ethereum SCCP light-client primitives.
use iroha_sccp::{
    CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA, CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA, EthereumFork,
    FINALIZED_ROOT_GINDEX_ELECTRA, FINALIZED_ROOT_GINDEX_PRE_ELECTRA,
    NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA, NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA,
    generalized_indices,
};
#[test]
fn public_fork_gindices_match_the_consensus_spec() {
    let pre_electra = generalized_indices(EthereumFork::Deneb);
    assert_eq!(
        pre_electra.finalized_root,
        FINALIZED_ROOT_GINDEX_PRE_ELECTRA
    );
    assert_eq!(
        pre_electra.current_sync_committee,
        CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA
    );
    assert_eq!(
        pre_electra.next_sync_committee,
        NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA
    );
    let electra = generalized_indices(EthereumFork::Electra);
    assert_eq!(electra.finalized_root, FINALIZED_ROOT_GINDEX_ELECTRA);
    assert_eq!(
        electra.current_sync_committee,
        CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA
    );
    assert_eq!(
        electra.next_sync_committee,
        NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA
    );
}
