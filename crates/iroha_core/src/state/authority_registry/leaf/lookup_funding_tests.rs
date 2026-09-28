//! Funded lookup refusal preserves semantic priority, old roots and proof checks.

use super::*;
use iroha_data_model::consensus::{ConsensusKeyId, ConsensusKeyRole};
use mv::allocation::{AllocationBudget, AllocationRefusal};

#[test]
fn lookup_admission_is_local_and_verification_requires_no_resident_owner() {
    let table = "world.consensus_keys_by_pk";
    let limits = LeafLimits {
        max_tables: 1,
        max_rows: 4,
        max_payload_bytes: 4096,
        max_ordered_table_bytes: 4096,
        max_streamed_value_bytes: 8192,
    };
    let budget = AllocationBudget::new(0);
    let mut leaves = CanonicalTableLeafSet::new(&[table], limits, &budget).unwrap();
    let empty_root = leaves.root();
    let key = "borrowed".to_owned();
    let value = vec![ConsensusKeyId::new(ConsensusKeyRole::Validator, "entry")];
    assert!(matches!(
        leaves.insert(table, &key, &value),
        Err(LeafError::Admission(AllocationRefusal::ExceedsLimit {
            limit_bytes: 0,
            ..
        }))
    ));
    assert_eq!(leaves.root(), empty_root);
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(64 * 1024);
    leaves.insert(table, &key, &value).unwrap();
    let root = leaves.root();
    let proof = leaves.prove_lookup(table, &key).unwrap();
    let reserved = budget.reserved_bytes();
    budget.set_limit_bytes(0);
    assert!(
        matches!(leaves.insert(table, &key, &value), Err(LeafError::DuplicateRow(id)) if id == table)
    );
    assert_eq!(leaves.root(), root);
    let digest =
        CanonicalTableLeafSet::verify_lookup(&[table], limits, &root, table, &key, &proof).unwrap();
    assert!(digest.is_some());
    assert_eq!(
        budget.reserved_bytes(),
        reserved,
        "verification has no resident allocation owner"
    );
    drop(leaves);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        CanonicalTableLeafSet::verify_lookup(&[table], limits, &root, table, &key, &proof)
            .unwrap()
            .is_some()
    );
}
