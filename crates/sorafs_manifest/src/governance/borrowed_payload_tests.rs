//! Borrowed field encoding without an independent typed-frame capability.

use super::borrowed_norito;
use crate::canonical_test_support::{PayloadOnly, assert_same_payload};

#[test]
fn borrowed_fields_accept_payload_only_values_and_preserve_layouts() {
    let value = PayloadOnly(42);
    assert_same_payload(&borrowed_norito::Value(&value), &value);
    for bytes in [vec![], vec![0, 1, 255]] {
        assert_same_payload(&borrowed_norito::Vec(&bytes), &bytes);
        assert_same_payload(&borrowed_norito::Option(Some(&bytes)), &Some(bytes.clone()));
    }
    assert_same_payload(&borrowed_norito::Option(None), &None::<Vec<u8>>);
}
