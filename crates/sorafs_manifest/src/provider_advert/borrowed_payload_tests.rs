//! Borrowed field encoding without an independent typed-frame capability.

use super::borrowed_norito;
use crate::canonical_test_support::{PayloadOnly, assert_same_payload};

#[test]
fn borrowed_fields_accept_payload_only_values_and_preserve_layouts() {
    let value = PayloadOnly(42);
    assert_same_payload(&borrowed_norito::Value(&value), &value);
    for values in [vec![], vec![PayloadOnly(0), PayloadOnly(u64::MAX)]] {
        assert_same_payload(&borrowed_norito::Vec(&values), &values);
    }
}
