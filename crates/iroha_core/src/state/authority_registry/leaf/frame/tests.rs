//! Canonical encoding remains identical while exact bytes retain original credit.

use super::*;
use mv::allocation::AllocationRefusal;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn funded_and_existing_frame_owners_receive_the_same_canonical_bytes() {
    for value in ["", "key", "日本語🔑"] {
        let value = value.to_owned();
        let expected = norito::codec::encode_adaptive(&value);
        let budget = AllocationBudget::new(expected.len());
        let funded = bare_payload_with_bound(&value, expected.len(), |length| {
            let frame = FundedFrame::new(length, &budget)?;
            budget.set_limit_bytes(0);
            Ok(frame)
        })
        .unwrap()
        .into_buffer();
        let existing = bare_payload_with_bound(&value, expected.len(), vector).unwrap();
        assert_eq!(funded.as_slice(), expected);
        assert_eq!(funded.as_slice(), existing);
        assert_eq!(funded.capacity(), expected.len());
        assert_eq!(budget.reserved_bytes(), expected.len());
        drop(funded);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

struct Stateful<'a> {
    calls: Cell<usize>,
    mode: u8,
    budget: &'a AllocationBudget,
}
impl norito::SerializePayload for Stateful<'_> {
    fn serialize(&self, encoder: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        let call = self.calls.get();
        self.calls.set(call + 1);
        if call > 0 {
            assert_eq!(
                self.budget.reserved_bytes(),
                2,
                "output was admitted before second pass"
            );
            match self.mode {
                0 => return Ok(()),
                1 => {
                    encoder.write_all(b"cd")?;
                    return Ok(());
                }
                2 => {
                    let _ = encoder.write_all(b"abc");
                    return Ok(());
                }
                _ => panic!("serializer unwind after allocation"),
            }
        }
        encoder.write_all(b"ab")?;
        Ok(())
    }
}

#[test]
fn changed_or_unwinding_second_pass_never_retains_the_funded_frame() {
    for mode in 0..4 {
        let budget = AllocationBudget::new(2);
        let value = Stateful {
            calls: Cell::new(0),
            mode,
            budget: &budget,
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            bare_payload_with_bound(&value, 8, |length| FundedFrame::new(length, &budget))
        }));
        if mode < 3 {
            assert!(matches!(result, Ok(Err(LeafError::Encoding(_)))));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(value.calls.get(), 2);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn first_pass_bound_and_original_pool_refusal_precede_second_encoding() {
    let budget = AllocationBudget::new(0);
    let value = Stateful {
        calls: Cell::new(0),
        mode: 3,
        budget: &budget,
    };
    assert!(matches!(
        bare_payload_with_bound(&value, 1, |length| FundedFrame::new(length, &budget)),
        Err(LeafError::PayloadLimit)
    ));
    assert_eq!(value.calls.get(), 1);
    value.calls.set(0);
    assert!(matches!(
        bare_payload_with_bound(&value, 8, |length| FundedFrame::new(length, &budget)),
        Err(LeafError::Admission(AllocationRefusal::ExceedsLimit { .. }))
    ));
    assert_eq!(value.calls.get(), 1);
    assert_eq!(budget.reserved_bytes(), 0);
}
