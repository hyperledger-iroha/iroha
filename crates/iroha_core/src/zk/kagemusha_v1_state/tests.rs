//! Focused aggregate-state tests for the three-message KAGEMUSHA V1 protocol.

use super::*;
use std::collections::BTreeMap;

#[test]
fn pending_prefix_has_no_protocol_count_ceiling() {
    let pending = (0_u32..4_096)
        .map(|index| {
            let mut id = [0_u8; 32];
            id[..4].copy_from_slice(&index.to_be_bytes());
            (CreditIdV1(id), 1_u128)
        })
        .collect::<BTreeMap<_, _>>();
    let selected =
        required_pending_credit_prefix(0, 4_096, pending).expect("all credits remain spendable");
    assert_eq!(selected.len(), 4_096);
}

#[test]
fn pending_prefix_checks_asset_arithmetic_overflow() {
    let pending = [(CreditIdV1([1; 32]), 1_u128)];
    assert_eq!(
        required_pending_credit_prefix(u128::MAX, u128::MAX, pending),
        Ok(Vec::new())
    );
}
