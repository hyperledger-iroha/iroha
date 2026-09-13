//! Semantic classification survives the exact peer Data/Ping/Pong wrapper.

use super::Message;
use crate::network::{
    admission_class_tests::AdmissionFixture,
    message::{ClassifyTopic, TransportAdmissionClass as A},
};
use norito::core as ncore;

#[test]
fn peer_envelope_preserves_admission_and_rejects_discriminator_substitution() {
    for fixture in AdmissionFixture::all() {
        let expected = fixture.admission_class();
        let message = Message::Data(fixture);
        assert_eq!(message.admission_class(), expected);
        for requested in [
            0,
            ncore::header_flags::COMPACT_LEN,
            ncore::header_flags::PACKED_STRUCT | ncore::header_flags::COMPACT_LEN,
            ncore::header_flags::PACKED_STRUCT
                | ncore::header_flags::COMPACT_LEN
                | ncore::header_flags::FIELD_BITSET,
        ] {
            let (bare, flags) = {
                let _encode_guard = ncore::DecodeFlagsGuard::enter(requested);
                norito::codec::encode_with_header_flags(&message)
            };
            let _decode_guard = ncore::DecodeFlagsGuard::enter(flags);
            let (decoded, consumed) =
                ncore::decode_field_canonical::<Message<AdmissionFixture>>(&bare)
                    .expect("decode canonical peer Data under its advertised flags");
            assert_eq!(consumed, bare.len());
            assert_eq!(decoded.admission_class(), expected);
            assert_eq!(decoded.topic(), message.topic());
            assert_eq!(
                Message::<AdmissionFixture>::inbound_admission_class(&bare, flags).unwrap(),
                expected
            );
            let mut trailing = bare.clone();
            trailing.push(0);
            assert!(
                Message::<AdmissionFixture>::inbound_admission_class(&trailing, flags).is_err()
            );
            for tag in [1_u32, 2, 99] {
                let mut substituted = bare.clone();
                substituted[..4].copy_from_slice(&tag.to_le_bytes());
                assert!(
                    Message::<AdmissionFixture>::inbound_admission_class(&substituted, flags)
                        .is_err(),
                    "a unit or unknown tag cannot hide an application payload"
                );
            }
            assert!(
                Message::<AdmissionFixture>::inbound_admission_class(
                    &bare[..bare.len() - 1],
                    flags
                )
                .is_err()
            );
        }
    }
    for message in [Message::<AdmissionFixture>::Ping, Message::Pong] {
        let (bare, flags) = norito::codec::encode_with_header_flags(&message);
        let _decode_guard = ncore::DecodeFlagsGuard::enter(flags);
        let (decoded, consumed) = ncore::decode_field_canonical::<Message<AdmissionFixture>>(&bare)
            .expect("decode exact Ping/Pong under advertised flags");
        assert_eq!(consumed, bare.len());
        assert_eq!(decoded.admission_class(), A::Low);
        assert_eq!(decoded.topic(), message.topic());
        assert_eq!(message.admission_class(), A::Low);
        assert_eq!(
            Message::<AdmissionFixture>::inbound_admission_class(&bare, flags).unwrap(),
            A::Low
        );
    }
}
