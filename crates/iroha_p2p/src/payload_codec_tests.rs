//! Exact payload-prefix and inherited resource-budget regressions for P2P envelopes.

use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
};

/// A nested codec owner deliberately lacking a typed frame identity.
#[derive(Clone, Debug, norito::Encode, norito::Decode)]
pub(crate) struct PayloadOnly(pub(crate) u32);

fn payload<T: SerializePayload>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .serialize(&mut ncore::Encoder::new(&mut bytes))
        .unwrap();
    bytes
}

pub(crate) fn prefix<T>(value: &T)
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + for<'de> DecodeFromSlice<'de>,
{
    let previous = ncore::get_decode_flags();
    for flags in (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
    {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let bytes = payload(value);
        let (decoded, used) = T::decode_from_slice(&bytes).expect("complete payload");
        assert_eq!(used, bytes.len());
        assert_eq!(payload(&decoded), bytes);
        let mut extended = bytes.clone();
        extended.extend_from_slice(&[0xa5, 0x5a]);
        let (decoded, used) = T::decode_from_slice(&extended).expect("payload prefix");
        assert_eq!(used, bytes.len(), "caller suffix must remain unread");
        assert_eq!(&extended[used..], &[0xa5, 0x5a]);
        assert_eq!(payload(&decoded), bytes);
        assert!(matches!(
            ncore::decode_field_canonical::<T>(&extended),
            Err(ncore::Error::LengthMismatch)
        ));
        for end in 0..bytes.len() {
            assert!(
                T::decode_from_slice(&bytes[..end]).is_err(),
                "truncation at {end}"
            );
            assert_eq!(ncore::get_decode_flags(), flags);
        }
        assert!(T::decode_from_slice(&bytes).is_ok());
        assert_eq!(ncore::get_decode_flags(), flags);
    }
    assert_eq!(ncore::get_decode_flags(), previous);
}

pub(crate) fn allocation_limit<T>(value: &T)
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + for<'de> DecodeFromSlice<'de>,
{
    for flags in [0, ncore::header_flags::COMPACT_LEN] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let bytes = payload(value);
        let outer = ncore::DecodeLimits::new(1024, 4096, 4096, 65536, 32);
        ncore::with_decode_limits_scope(outer, || {
            let deny = ncore::DecodeLimits::new(1024, 4096, 4096, 0, 32);
            ncore::with_decode_limits_scope(deny, || {
                assert!(matches!(
                    T::decode_from_slice(&bytes),
                    Err(ncore::Error::TotalAllocationExceeded { .. })
                ));
            });
            assert_eq!(ncore::get_decode_flags(), flags);
            let (decoded, used) = T::decode_from_slice(&bytes).unwrap();
            assert_eq!(used, bytes.len());
            assert_eq!(payload(&decoded), bytes);
        });
        assert!(T::decode_from_slice(&bytes).is_ok());
        assert_eq!(ncore::get_decode_flags(), flags);
    }
}
