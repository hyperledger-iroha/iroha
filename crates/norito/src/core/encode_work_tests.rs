//! Authored controls only; this detached candidate has not been compiled/run.
use super::*;
use std::cell::Cell;

#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito.test.bounded.StaticU32")]
struct StaticU32(u32);
impl SerializePayload for StaticU32 {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

fn schema_cost() -> usize {
    <StaticU32 as crate::NoritoSchema>::static_frame_name()
        .unwrap()
        .len()
        + 128
}

fn limits() -> EncodeWorkLimits {
    EncodeWorkLimits {
        max_frame_bytes: 8192,
        max_byte_visits: 1_000_000,
        max_operations: 100_000,
        max_depth: 64,
    }
}
fn encode<P: SerializePayload + ?Sized>(
    source: &P,
    limits: EncodeWorkLimits,
) -> (Result<(), BoundedEncodeWorkError>, Vec<u8>, usize) {
    let mut bytes = Vec::new();
    let mut prepared = 0;
    let result = write_canonical_payload_bounded::<StaticU32, _, _, _>(
        source,
        &mut bytes,
        limits,
        |_, length| {
            prepared = length;
            Ok(())
        },
    );
    (result, bytes, prepared)
}
#[test]
fn nominal_schema_alignment_and_bytes_match_existing_canonical_writer() {
    let value = 53_u32;
    let (result, bytes, prepared) = encode(&value, limits());
    result.unwrap();
    assert_eq!(bytes, crate::encode_canonical(&StaticU32(value)).unwrap());
    assert_eq!(prepared, bytes.len());
}
#[test]
fn byte_refusal_precedes_measured_prepare_and_output() {
    let (result, bytes, prepared) = encode(
        &53_u32,
        EncodeWorkLimits {
            max_byte_visits: 0,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
#[test]
fn root_frame_ceiling_refuses_before_prepare() {
    let (result, bytes, prepared) = encode(
        &53_u32,
        EncodeWorkLimits {
            max_frame_bytes: 1,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::FrameBytes
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct Swallows;
impl SerializePayload for Swallows {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let _ignored = writer.write_all(&[1; 32]);
        let _ignored = writer.write_all(&[]);
        Ok(())
    }
}
#[test]
fn swallowed_refusal_stays_first_and_prevents_preparation() {
    let (result, bytes, prepared) = encode(
        &Swallows,
        EncodeWorkLimits {
            max_byte_visits: schema_cost() + 1,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct Nested;
impl SerializePayload for Nested {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let _ignored = write_canonical_payload_bounded::<StaticU32, _, _, _>(
            &1_u32,
            writer,
            limits(),
            |_, _| Ok(()),
        );
        Ok(())
    }
}
#[test]
fn nested_bounded_entry_cannot_reset_outer_allowance() {
    let (result, bytes, prepared) = encode(&Nested, limits());
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::NestedScope
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct Changing<'a>(&'a Cell<u8>, bool);
impl SerializePayload for Changing<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let next = self.0.get() + 1;
        self.0.set(next);
        writer.write_all(&[next])?;
        if self.1 && next > 1 {
            writer.write_all(&[0])?;
        }
        Ok(())
    }
}
#[test]
fn same_length_second_pass_change_preserves_original_checksum_error() {
    let calls = Cell::new(0);
    let (result, _, _) = encode(&Changing(&calls, false), limits());
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Codec(Error::ChecksumMismatch))
    ));
    assert_eq!(calls.get(), 2);
}
#[test]
fn second_pass_overrun_preserves_original_length_error() {
    let calls = Cell::new(0);
    let (result, _, _) = encode(&Changing(&calls, true), limits());
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Codec(Error::LengthMismatch))
    ));
}
struct NestedMeasure;
impl SerializePayload for NestedMeasure {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        for _ in 0..100 {
            write_len_prefixed(writer, &7_u32)?;
        }
        Ok(())
    }
}
#[test]
fn first_pass_nested_measurements_share_finite_byte_allowance() {
    let (result, bytes, prepared) = encode(
        &NestedMeasure,
        EncodeWorkLimits {
            max_byte_visits: schema_cost() + 64,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct EmptyWrites<'a> {
    attempts: &'a Cell<usize>,
    first_write_refused: &'a Cell<bool>,
}
impl SerializePayload for EmptyWrites<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        for _ in 0..100 {
            let attempt = self.attempts.get();
            self.attempts.set(attempt + 1);
            let result = writer.write_all(&[]);
            if attempt == 0 {
                self.first_write_refused.set(result.is_err());
            }
        }
        Ok(())
    }
}
#[test]
fn zero_length_writes_consume_operations_and_cannot_evade_limit() {
    let attempts = Cell::new(0);
    let first_write_refused = Cell::new(false);
    let (result, bytes, prepared) = encode(
        &EmptyWrites {
            attempts: &attempts,
            first_write_refused: &first_write_refused,
        },
        EncodeWorkLimits {
            max_operations: 3,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::Operations
        ))
    ));
    assert_eq!(attempts.get(), 100);
    assert!(first_write_refused.get());
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct Recursive;
impl SerializePayload for Recursive {
    fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
        encoded_payload_len(self)?;
        Ok(())
    }
}
#[test]
fn codec_driven_recursive_measurement_refuses_at_finite_depth() {
    let (result, bytes, prepared) = encode(
        &Recursive,
        EncodeWorkLimits {
            max_depth: 4,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Depth))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
struct Panics;
impl SerializePayload for Panics {
    fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
        panic!("test unwind");
    }
}
#[test]
fn unwind_restores_opt_in_scope_without_poisoning_next_independent_call() {
    assert!(std::panic::catch_unwind(|| encode(&Panics, limits())).is_err());
    encode(&1_u32, limits()).0.unwrap();
}
#[test]
fn measured_hook_cannot_reset_resource_failure_before_second_pass() {
    let mut bytes = Vec::new();
    let result = write_canonical_payload_bounded::<StaticU32, _, _, _>(
        &1_u32,
        &mut bytes,
        limits(),
        |_, _| {
            let mut discarded = std::io::sink();
            let _ignored = write_canonical_payload_bounded::<StaticU32, _, _, _>(
                &2_u32,
                &mut discarded,
                limits(),
                |_, _| Ok(()),
            );
            Ok(())
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::NestedScope
        ))
    ));
    assert!(bytes.is_empty());
}

#[test]
fn byte_sequence_fixed_scratch_obeys_scope_before_measured_prepare() {
    let value = vec![3_u8; 300];
    let (result, bytes, prepared) = encode(
        &value,
        EncodeWorkLimits {
            max_byte_visits: schema_cost() + 16,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}

struct PayloadProbe<'a>(&'a Cell<usize>);
impl SerializePayload for PayloadProbe<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.set(self.0.get() + 1);
        writer.write_all(&[7])?;
        Ok(())
    }
}
#[test]
fn cold_schema_charge_precedes_payload_and_storage_preparation() {
    let calls = Cell::new(0);
    let (result, bytes, prepared) = encode(
        &PayloadProbe(&calls),
        EncodeWorkLimits {
            max_byte_visits: schema_cost() - 1,
            ..limits()
        },
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
    assert_eq!(calls.get(), 0);
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
#[test]
fn dynamic_identity_refuses_before_payload_even_with_a_warm_memo() {
    // Primitive u32 in this retained schema source has no static_frame_name.
    let _warm = crate::schema::identity::frame_hash::<u32>();
    let calls = Cell::new(0);
    let mut bytes = Vec::new();
    let result = write_canonical_payload_bounded::<u32, _, _, _>(
        &PayloadProbe(&calls),
        &mut bytes,
        limits(),
        |_, _| panic!("must not prepare"),
    );
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source))
    ));
    assert_eq!(calls.get(), 0);
    assert!(bytes.is_empty());
}
struct IgnoresSourceFailure;
impl SerializePayload for IgnoresSourceFailure {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        let _ignored = refuse_encode_source();
        let _ignored = charge_encode_prewrite(usize::MAX);
        let _ignored = writer.write_all(&[9]);
        Ok(())
    }
}
#[test]
fn first_source_refusal_survives_swallowed_errors_and_later_budget_failure() {
    let (result, bytes, prepared) = encode(&IgnoresSourceFailure, limits());
    assert!(matches!(
        result,
        Err(BoundedEncodeWorkError::Refused(EncodeWorkRefusal::Source))
    ));
    assert!(bytes.is_empty());
    assert_eq!(prepared, 0);
}
#[test]
fn prewrite_hook_does_not_install_a_scope_or_change_legacy_errors() {
    assert!(!charge_encode_prewrite(usize::MAX).unwrap());
    assert!(matches!(
        refuse_encode_source(),
        Error::NonCanonicalEncoding
    ));
    encode(&1_u32, limits()).0.unwrap();
}

#[test]
fn even_empty_byte_scratch_prepays_its_full_initialization() {
    let guard = encode_work::Guard::enter(EncodeWorkLimits {
        max_byte_visits: BYTE_ELEMENT_RUN - 1,
        ..limits()
    })
    .unwrap();
    assert!(ByteElementFramer::new(default_encode_flags()).is_err());
    assert!(matches!(
        guard.finish(Ok(())),
        Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::ByteVisits
        ))
    ));
}
