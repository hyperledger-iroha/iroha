//! State-block committed-fragment counter regression tests.
use super::*;
use crate::kura::Kura;
use iroha_data_model::block::BlockHeader;
use nonzero_ext::nonzero;
use std::sync::Arc;
#[test]
fn state_block_fragment_counter_updates_on_apply() {
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new(World::new(), Arc::clone(&kura), query);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut state_block = state.block(header);
    let maintenance_fragments = state_block.committed_fragment_count();
    {
        let tx = state_block.transaction();
        tx.apply();
    }
    assert_eq!(
        state_block.committed_fragment_count(),
        maintenance_fragments + 1,
        "applying one transaction adds one fragment to block-start maintenance"
    );
}
