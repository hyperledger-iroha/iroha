//! Transaction trait conformance and Merkle tests
use std::fmt::Debug;

use iroha_crypto::MerkleTree;
use iroha_data_model::transaction::signed::{
    ExecutionStep, TransactionEntrypoint, TransactionResult,
};

fn assert_traits<T: Debug + Clone + PartialEq + Eq + PartialOrd + Ord>() {}

#[test]
fn transaction_structs_impl_standard_traits() {
    assert_traits::<TransactionResult>();
    assert_traits::<ExecutionStep>();
}

#[test]
fn merkle_tree_builds_for_transaction_types() {
    let _: MerkleTree<TransactionEntrypoint> = MerkleTree::default();
    let _: MerkleTree<TransactionResult> = MerkleTree::default();
}
