//! Confidential-tree default-state invariants.
use iroha_core::state::ZkAssetState;
#[test]
fn default_confidential_tree_metadata_is_canonical() {
    ZkAssetState::default()
        .validate_tree_metadata()
        .expect("default confidential tree root must match its empty frontier");
}
