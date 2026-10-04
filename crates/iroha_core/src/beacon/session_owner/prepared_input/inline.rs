//! Allocation-free scalar leaves and the same generated walk for inline records.
//!
//! Ordinary owning field decoding may allocate alignment backing even for Copy
//! values. These adapters use the canonical slice primitives and shared record
//! walk directly; they never call an archived owning decoder.

use super::*;
use common::*;
use iroha_data_model::{consensus::GlobalThresholdBeaconDkgSessionV1, id::NetworkId};
use norito::core::DecodeFromSlice;

pub(super) trait InlineValue: Copy {
    fn decode_payload(bytes: &[u8]) -> DecodeResult<Self>;
}
macro_rules! scalar {
    ($ty:ty) => {
        impl InlineValue for $ty {
            fn decode_payload(bytes: &[u8]) -> DecodeResult<Self> {
                let (value, used) = <$ty as DecodeFromSlice>::decode_from_slice(bytes)?;
                complete(used, bytes)?;
                Ok(value)
            }
        }
    };
}
scalar!(u16);
scalar!(u64);
scalar!(NetworkId);
impl<const N: usize> InlineValue for [u8; N] {
    fn decode_payload(bytes: &[u8]) -> DecodeResult<Self> {
        let (value, used) = <Self as DecodeFromSlice>::decode_from_slice(bytes)?;
        complete(used, bytes)?;
        Ok(value)
    }
}

struct Destination<T>(T);
impl<T> FieldDestination for Destination<T> {
    type Error = DestinationError;
}
macro_rules! field {
    ($record:ty, $index:literal, $name:ident, $ty:ty) => {
        impl DecodeField<$index, $ty> for Destination<$record> {
            type Value = ();
            fn decode_field(&mut self, value: CanonicalField<'_, $ty>) -> DecodeResult<()> {
                self.0.$name = value.with_payload(<$ty as InlineValue>::decode_payload)?;
                Ok(())
            }
        }
    };
}
macro_rules! record {
    ($record:ty, $initial:expr) => {
        impl InlineValue for $record {
            fn decode_payload(bytes: &[u8]) -> DecodeResult<Self> {
                let mut destination = Destination($initial);
                let (_, used) = <$record>::decode_fields(bytes, &mut destination)?;
                complete(used, bytes)?;
                Ok(destination.0)
            }
        }
    };
}
field!(
    GlobalThresholdBeaconDkgConstantProofV1,
    0,
    commitment,
    [u8; 96]
);
field!(
    GlobalThresholdBeaconDkgConstantProofV1,
    1,
    response,
    [u8; 32]
);
record!(
    GlobalThresholdBeaconDkgConstantProofV1,
    GlobalThresholdBeaconDkgConstantProofV1 {
        commitment: [0; 96],
        response: [0; 32],
    }
);
field!(GlobalThresholdBeaconPublicShareV1, 0, index, u16);
field!(
    GlobalThresholdBeaconPublicShareV1,
    1,
    participant_seat_binding,
    [u8; 32]
);
field!(
    GlobalThresholdBeaconPublicShareV1,
    2,
    public_key_share,
    [u8; 96]
);
record!(
    GlobalThresholdBeaconPublicShareV1,
    GlobalThresholdBeaconPublicShareV1 {
        index: 0,
        participant_seat_binding: [0; 32],
        public_key_share: [0; 96],
    }
);
field!(GlobalThresholdBeaconDkgSessionV1, 0, version, u16);
field!(GlobalThresholdBeaconDkgSessionV1, 1, network_id, NetworkId);
field!(GlobalThresholdBeaconDkgSessionV1, 2, session_id, [u8; 32]);
field!(GlobalThresholdBeaconDkgSessionV1, 3, attempt_id, [u8; 32]);
field!(
    GlobalThresholdBeaconDkgSessionV1,
    4,
    authority_generation,
    u64
);
field!(GlobalThresholdBeaconDkgSessionV1, 5, roster_hash, [u8; 32]);
field!(GlobalThresholdBeaconDkgSessionV1, 6, committee_size, u16);
field!(GlobalThresholdBeaconDkgSessionV1, 7, threshold, u16);
field!(GlobalThresholdBeaconDkgSessionV1, 8, start_height, u64);
field!(
    GlobalThresholdBeaconDkgSessionV1,
    9,
    commitments_end_height,
    u64
);
field!(
    GlobalThresholdBeaconDkgSessionV1,
    10,
    deliveries_end_height,
    u64
);
field!(
    GlobalThresholdBeaconDkgSessionV1,
    11,
    acceptances_end_height,
    u64
);
// The initial identity is only initialized destination storage. The shared
// generated walk must replace every field before this record can be returned.
record!(
    GlobalThresholdBeaconDkgSessionV1,
    GlobalThresholdBeaconDkgSessionV1 {
        version: 0,
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0; 32])
        )),
        session_id: [0; 32],
        attempt_id: [0; 32],
        authority_generation: 0,
        roster_hash: [0; 32],
        committee_size: 0,
        threshold: 0,
        start_height: 0,
        commitments_end_height: 0,
        deliveries_end_height: 0,
        acceptances_end_height: 0,
    }
);
