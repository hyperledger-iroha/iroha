//! Restoration accepts the current writer's authenticated SCCP fields without granting authority.
use super::*;
use crate::state::World;
use iroha_data_model::{bridge::SccpNetworkV1, sccp::reconciliation::SccpReconciliationStateV1};
use norito::codec::Encode as _;

#[test]
fn current_world_writer_indexes_and_reconciliations_restore_exact_changes() {
    let world = World::new();
    let network = SccpNetworkV1::EthereumMainnet;
    let key = (network, 1);
    let proof_key = (network, 1, [7; 32]);
    let due_key = (20, network, 1);
    let reconciliation = SccpReconciliationStateV1 {
        opened_at_ms: 10,
        closes_at_ms: 20,
        forged_supply: 3,
        rho: None,
        settled_face: 0,
    };
    let names: &[&[u8]] = &[
        b"sccp_destination_pending_generations",
        b"sccp_reconciliations",
        b"sccp_reconciliation_proven",
        b"sccp_pending_refund_amount",
        b"sccp_reconciliation_due",
    ];
    let keys = [
        7_u64.encode(),
        key.encode(),
        proof_key.encode(),
        key.encode(),
        due_key.encode(),
    ];
    let values = [
        ().encode(),
        reconciliation.encode(),
        3_u128.encode(),
        4_u128.encode(),
        ().encode(),
    ];
    let inserted = {
        let mut block = world.block();
        block.sccp_destination_pending_generations.insert(7, ());
        block
            .sccp_reconciliations
            .try_insert_admitted(key, reconciliation)
            .unwrap();
        block
            .sccp_reconciliation_proven
            .try_insert_admitted(proof_key, 3)
            .unwrap();
        block
            .sccp_pending_refund_amount
            .try_insert_admitted(key, 4)
            .unwrap();
        block
            .sccp_reconciliation_due
            .try_insert_admitted(due_key, ())
            .unwrap();
        let mut bytes = Vec::new();
        block.write_sccp_execution_delta(&mut bytes).unwrap();
        block.commit();
        bytes
    };
    let mut seen = 0;
    visit::<super::super::Error>(&inserted, |entry| {
        assert_eq!(entry.name, names[seen]);
        assert_eq!(entry.key, Some(keys[seen].as_slice()));
        assert_eq!(entry.value, Some(values[seen].as_slice()));
        seen += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, names.len());
    assert_eq!(
        field_range(&inserted, b"sccp_forgery_offences").unwrap(),
        None
    );
    let ranges: Vec<_> = names
        .iter()
        .map(|name| field_range(&inserted, name).unwrap().unwrap())
        .collect();
    for (index, name) in names.iter().enumerate() {
        let (start, end) = ranges[index];
        let selected = &inserted[start..end];
        assert_eq!(
            select(selected, name, Some(&|bytes| bytes == keys[index])).unwrap(),
            Some(SelectedValue::Updated(values[index].as_slice())),
            "sparse deltas may omit unchanged preceding and following fields",
        );
    }
    // A recognized field cannot make an unknown, duplicate or reversed tail acceptable.
    let first = &inserted[ranges[0].0..ranges[0].1];
    let last = &inserted[ranges[4].0..ranges[4].1];
    let mut unknown = Vec::new();
    let name = b"sccp_unknown_field";
    unknown.extend_from_slice(&(name.len() as u64).to_le_bytes());
    unknown.extend_from_slice(name);
    unknown.extend_from_slice(&0_u64.to_le_bytes());
    for invalid in [
        [last, first].concat(),
        [first, first].concat(),
        [first, unknown.as_slice()].concat(),
    ] {
        assert!(visit::<super::super::Error>(&invalid, |_| Ok(())).is_err());
        assert!(field_range(&invalid, names[0]).is_err());
    }
    let mut block = world.block();
    block.sccp_destination_pending_generations.remove(7);
    block.sccp_reconciliations.try_remove_admitted(key).unwrap();
    block
        .sccp_reconciliation_proven
        .try_remove_admitted(proof_key)
        .unwrap();
    block
        .sccp_pending_refund_amount
        .try_remove_admitted(key)
        .unwrap();
    block
        .sccp_reconciliation_due
        .try_remove_admitted(due_key)
        .unwrap();
    let mut deleted = Vec::new();
    block.write_sccp_execution_delta(&mut deleted).unwrap();
    let mut seen = 0;
    visit::<super::super::Error>(&deleted, |entry| {
        assert_eq!(entry.name, names[seen]);
        assert_eq!(entry.key, Some(keys[seen].as_slice()));
        assert_eq!(entry.value, None, "deletion is not an absent field");
        seen += 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, names.len());
}
