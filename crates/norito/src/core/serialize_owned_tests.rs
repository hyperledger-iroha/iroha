use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static EXACT_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy)]
struct ExactLen(u8);

impl NoritoSerialize for ExactLen {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&[self.0])?;
        Ok(())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        EXACT_CALLS.fetch_add(1, Ordering::Relaxed);
        Some(1)
    }
}

#[test]
fn serialize_owned_counts_real_payload_instead_of_trusting_exact_hint() {
    EXACT_CALLS.store(0, Ordering::Relaxed);
    reset_decode_state();
    let value = Box::new(ExactLen(0xAB));
    let mut buf = Vec::new();
    serialize_to_buffer(&value, &mut buf).expect("serialize owned payload");
    let (payload, used) = parse_owned_payload(&buf).expect("parse owned payload");
    assert_eq!(used, buf.len());
    assert_eq!(payload, &[0xAB]);
    assert_eq!(EXACT_CALLS.load(Ordering::Relaxed), 0);
    reset_decode_state();
}

#[derive(Clone, Copy)]
struct BadExactLen;

impl NoritoSerialize for BadExactLen {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&[0xAA, 0xBB])?;
        Ok(())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(1)
    }
}

#[test]
fn serialize_owned_ignores_an_incorrect_exact_length_hint() {
    reset_decode_state();
    let value = Box::new(BadExactLen);
    let mut buf = Vec::new();
    serialize_to_buffer(&value, &mut buf).expect("counted owned payload");
    let (payload, used) = parse_owned_payload(&buf).expect("parse owned payload");
    assert_eq!(used, buf.len());
    assert_eq!(payload, &[0xAA, 0xBB]);
    assert!(crate::to_bytes(&value).is_ok());
    reset_decode_state();
}

#[derive(Clone, Copy)]
struct UnknownLen(u8);

impl NoritoSerialize for UnknownLen {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&[self.0, self.0.wrapping_add(1)])?;
        Ok(())
    }
}

#[test]
fn serialize_owned_streams_when_exact_length_is_unavailable() {
    reset_decode_state();
    let value = Box::new(UnknownLen(0xAB));
    let mut buf = Vec::new();
    serialize_to_buffer(&value, &mut buf).expect("serialize unknown-length owned payload");
    let (payload, used) = parse_owned_payload(&buf).expect("parse owned payload");
    assert_eq!(used, buf.len());
    assert_eq!(payload, &[0xAB, 0xAC]);
    reset_decode_state();
}

#[test]
fn serialize_owned_rejects_a_changed_second_pass() {
    use std::cell::Cell;

    struct Growing(Cell<usize>);

    impl NoritoSerialize for Growing {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let pass = self.0.get();
            self.0.set(pass + 1);
            writer.write_all(&[0x11])?;
            if pass != 0 {
                for _ in 0..256 {
                    writer.write_all(&[0x22; 4 * 1024])?;
                }
            }
            Ok(())
        }
    }

    reset_decode_state();
    let value = Box::new(Growing(Cell::new(0)));
    let mut buf = Vec::with_capacity(32);
    let initial_capacity = buf.capacity();
    assert!(matches!(
        serialize_to_buffer(&value, &mut buf),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(value.0.get(), 2);
    let (payload, used) = parse_owned_payload(&buf).expect("bounded partial payload");
    assert_eq!(used, buf.len());
    assert_eq!(payload, &[0x11]);
    assert_eq!(buf.capacity(), initial_capacity);
    reset_decode_state();
}
