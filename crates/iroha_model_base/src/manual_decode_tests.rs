//! Fallible manual model decoders return context errors without altering their shipping payloads.

use crate::{chain::ChainId, name::Name, state_path::StatePath};
use norito::{
    DeserializePayload, SerializePayload,
    core::{DecodeFlagsGuard, PayloadCtxGuard},
};

fn assert_context_error_and_wire_retry<T>(expected: &T)
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + std::fmt::Debug + PartialEq,
{
    let _layout = DecodeFlagsGuard::enter(0);
    let mut bytes = Vec::new();
    norito::core::serialize_to_buffer(expected, &mut bytes).unwrap();
    let archived = norito::core::archived_from_slice::<u8>(&bytes).unwrap();
    let missing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        T::try_deserialize(archived.archived().cast())
    }));
    assert!(
        matches!(missing, Ok(Err(norito::Error::MissingPayloadContext))),
        "the context-error fallback must propagate its fallible String decoder, not panic"
    );
    let decoded = {
        let _payload = PayloadCtxGuard::enter(archived.bytes());
        T::try_deserialize(archived.archived().cast()).unwrap()
    };
    assert_eq!(&decoded, expected);
    let mut repeated = Vec::new();
    norito::core::serialize_to_buffer(&decoded, &mut repeated).unwrap();
    assert_eq!(repeated, bytes);
    let truncated = norito::core::archived_from_slice::<u8>(&bytes[..bytes.len() - 1]).unwrap();
    let _payload = PayloadCtxGuard::enter(truncated.bytes());
    assert!(T::try_deserialize(truncated.archived().cast()).is_err());
}

#[test]
fn manual_decode_chain_id_returns_missing_context_without_changing_wire() {
    assert_context_error_and_wire_retry(&ChainId::from("manual-decode-root"));
}

#[test]
fn manual_decode_name_returns_missing_context_without_changing_wire() {
    assert_context_error_and_wire_retry(&"original_name".parse::<Name>().unwrap());
}

#[test]
fn manual_decode_state_path_returns_missing_context_without_changing_wire() {
    assert_context_error_and_wire_retry(&"sc/original/balance".parse::<StatePath>().unwrap());
}
