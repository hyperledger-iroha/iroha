//! Authored controls only; this detached candidate has not been compiled/run.
use super::*;
use std::cell::Cell;
fn limits() -> norito::core::EncodeWorkLimits {
    norito::core::EncodeWorkLimits {
        max_frame_bytes: 8192,
        max_byte_visits: 1_000_000,
        max_operations: 100_000,
        max_depth: 64,
    }
}
struct Payload<'a> {
    passes: &'a Cell<usize>,
    pool: &'a AllocationBudget,
    fail_second: bool,
}
impl norito::core::SerializePayload for Payload<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        let pass = self.passes.get() + 1;
        self.passes.set(pass);
        if pass == 1 {
            assert_eq!(self.pool.reserved_bytes(), 0);
        } else {
            assert!(
                self.pool.reserved_bytes() > 0,
                "backing charged before output serialization"
            );
        }
        writer.write_all(&[5, 2, 7])?;
        if pass == 2 && self.fail_second {
            return Err(norito::Error::NonCanonicalEncoding);
        }
        Ok(())
    }
}
#[test]
fn actual_codec_measured_hook_charges_exact_complete_backing() {
    let pool = AllocationBudget::new(8192);
    let passes = Cell::new(0);
    let payload = Payload {
        passes: &passes,
        pool: &pool,
        fail_second: false,
    };
    let owner =
        RetainedActionFrameBytesV1::try_encode_unvalidated_payload(&payload, &pool, limits())
            .unwrap();
    assert_eq!(passes.get(), 2);
    assert_eq!(owner.backing_bytes(), owner.as_bytes().len());
    assert_eq!(pool.reserved_bytes(), owner.as_bytes().len());
    assert!(owner.belongs_to(&pool));
    // This raw fixture emits no length prefix, so the codec finalizes its header
    // flags to zero even though compact lengths are enabled for serialization.
    let body = norito::canonical_frame_payload::<crate::trigger::action::Action>(
        owner.as_bytes(),
        0,
    )
    .unwrap();
    assert_eq!(body, &[5, 2, 7]); // Framing only; deliberately not a semantic Action.
    assert!(matches!(
        norito::canonical_frame_payload::<crate::trigger::action::Action>(
            owner.as_bytes(),
            norito::core::default_encode_flags(),
        ),
        Err(norito::Error::NonCanonicalEncoding)
    ));
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn allocation_refusal_occurs_before_second_pass_or_any_output() {
    let pool = AllocationBudget::new(0);
    let passes = Cell::new(0);
    let payload = Payload {
        passes: &passes,
        pool: &pool,
        fail_second: false,
    };
    let error =
        RetainedActionFrameBytesV1::try_encode_unvalidated_payload(&payload, &pool, limits())
            .unwrap_err();
    assert!(matches!(
        error,
        ActionFrameProductionFailureV1::Allocation(_)
    ));
    assert_eq!(passes.get(), 1);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(pool.peak_reserved_bytes(), 0);
}
#[test]
fn codec_failure_destroys_partial_output_before_returning_original_error() {
    let pool = AllocationBudget::new(8192);
    let passes = Cell::new(0);
    let payload = Payload {
        passes: &passes,
        pool: &pool,
        fail_second: true,
    };
    let error =
        RetainedActionFrameBytesV1::try_encode_unvalidated_payload(&payload, &pool, limits())
            .unwrap_err();
    assert!(matches!(
        error,
        ActionFrameProductionFailureV1::Codec(norito::core::BoundedEncodeWorkError::Codec(
            norito::Error::NonCanonicalEncoding
        ))
    ));
    assert_eq!(passes.get(), 2);
    assert_eq!(pool.reserved_bytes(), 0);
    assert!(pool.peak_reserved_bytes() > 0);
}
#[test]
fn work_refusal_never_allocates_a_backing() {
    let pool = AllocationBudget::new(8192);
    let passes = Cell::new(0);
    let payload = Payload {
        passes: &passes,
        pool: &pool,
        fail_second: false,
    };
    let error = RetainedActionFrameBytesV1::try_encode_unvalidated_payload(
        &payload,
        &pool,
        norito::core::EncodeWorkLimits {
            max_byte_visits: 0,
            ..limits()
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ActionFrameProductionFailureV1::Codec(norito::core::BoundedEncodeWorkError::Refused(
            norito::core::EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert_eq!(passes.get(), 0);
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(pool.peak_reserved_bytes(), 0);
}

struct RefusesSecond<'a>(&'a Cell<usize>);
impl norito::core::SerializePayload for RefusesSecond<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        self.0.set(self.0.get() + 1);
        if self.0.get() == 2 {
            let _ignored = norito::core::refuse_encode_source();
            return Ok(());
        }
        writer.write_all(&[1])?;
        Ok(())
    }
}
#[test]
fn swallowed_second_pass_source_refusal_drops_partial_owner_charge() {
    let pool = AllocationBudget::new(8192);
    let calls = Cell::new(0);
    let error = RetainedActionFrameBytesV1::try_encode_unvalidated_payload(
        &RefusesSecond(&calls),
        &pool,
        limits(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ActionFrameProductionFailureV1::Codec(norito::core::BoundedEncodeWorkError::Refused(
            norito::core::EncodeWorkRefusal::Source
        ))
    ));
    assert_eq!(calls.get(), 2);
    assert!(pool.peak_reserved_bytes() > 0);
    assert_eq!(pool.reserved_bytes(), 0);
}
