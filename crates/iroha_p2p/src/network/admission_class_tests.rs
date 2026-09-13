//! Raw and typed semantic classification across the signed relay envelope.

use super::{
    RelayMessage, RelayTarget,
    message::{ClassifyTopic, Topic, TransportAdmissionClass as A},
};
use iroha_crypto::{Algorithm, KeyPair};
use norito::{Decode, Encode, core as ncore};

/// Canonical test payload exercising classes independently of shared topics.
#[derive(Clone, Debug, Encode, Decode, norito::NoritoSchema)]
#[norito(decode_from_slice)]
#[norito_schema(name = "iroha_p2p::network::admission_class_tests::AdmissionFixture")]
pub enum AdmissionFixture {
    Safety(u8),
    Lane(u8),
    Payload(u8),
    Availability(u8),
    RecoveryControl(u8),
    RecoveryData(u8),
    Control(u8),
    BlockSync(u8),
    Low(u8),
}
impl AdmissionFixture {
    pub(crate) fn all() -> [Self; A::COUNT] {
        [
            Self::Safety(1),
            Self::Lane(2),
            Self::Payload(3),
            Self::Availability(4),
            Self::RecoveryControl(5),
            Self::RecoveryData(6),
            Self::Control(7),
            Self::BlockSync(8),
            Self::Low(9),
        ]
    }
}
impl ClassifyTopic for AdmissionFixture {
    fn topic(&self) -> Topic {
        match self {
            Self::Safety(_) => Topic::ConsensusSafety,
            Self::Lane(_) | Self::RecoveryControl(_) => Topic::Consensus,
            Self::Payload(_) => Topic::ConsensusPayload,
            Self::Availability(_) | Self::RecoveryData(_) => Topic::ConsensusChunk,
            Self::Control(_) => Topic::Control,
            Self::BlockSync(_) => Topic::BlockSync,
            Self::Low(_) => Topic::Health,
        }
    }
    fn inbound_topic(payload: &[u8], flags: u8) -> Result<Option<Topic>, ncore::Error> {
        Ok(Some(match Self::inbound_admission_class(payload, flags)? {
            A::Safety => Topic::ConsensusSafety,
            A::Lane | A::RecoveryControl => Topic::Consensus,
            A::Payload => Topic::ConsensusPayload,
            A::Availability | A::RecoveryData => Topic::ConsensusChunk,
            A::Control => Topic::Control,
            A::BlockSync => Topic::BlockSync,
            A::Low => Topic::Health,
        }))
    }
    fn admission_class(&self) -> A {
        match self {
            Self::Safety(_) => A::Safety,
            Self::Lane(_) => A::Lane,
            Self::Payload(_) => A::Payload,
            Self::Availability(_) => A::Availability,
            Self::RecoveryControl(_) => A::RecoveryControl,
            Self::RecoveryData(_) => A::RecoveryData,
            Self::Control(_) => A::Control,
            Self::BlockSync(_) => A::BlockSync,
            Self::Low(_) => A::Low,
        }
    }
    fn inbound_admission_class(payload: &[u8], flags: u8) -> Result<A, ncore::Error> {
        ncore::validate_header_flags(flags)?;
        let tag = u32::from_le_bytes(
            payload
                .get(..4)
                .ok_or(ncore::Error::LengthMismatch)?
                .try_into()
                .map_err(|_| ncore::Error::LengthMismatch)?,
        );
        let remaining = &payload[4..];
        // Derive-generated tuple variants omit the length of a fixed scalar
        // when PACKED_STRUCT is declared. Unlike a packed struct, this enum
        // field has neither an offset table nor a FIELD_BITSET byte.
        if flags & ncore::header_flags::PACKED_STRUCT != 0 {
            if remaining.len() != 1 {
                return Err(ncore::Error::LengthMismatch);
            }
        } else {
            let (len, prefix) = ncore::read_len_from_slice_with_flags(remaining, flags)?;
            if len != 1 || prefix.checked_add(len) != Some(remaining.len()) {
                return Err(ncore::Error::LengthMismatch);
            }
        }
        A::ALL
            .get(tag as usize)
            .copied()
            .ok_or_else(|| ncore::Error::Message("unknown admission fixture tag".to_owned()))
    }
}

#[test]
fn raw_admission_fixture_honors_declared_fixed_scalar_layout() {
    for fixture in AdmissionFixture::all() {
        for requested in [
            0,
            ncore::header_flags::COMPACT_LEN,
            ncore::header_flags::PACKED_STRUCT | ncore::header_flags::COMPACT_LEN,
            ncore::header_flags::PACKED_STRUCT
                | ncore::header_flags::COMPACT_LEN
                | ncore::header_flags::FIELD_BITSET,
        ] {
            let (bare, flags) = {
                let _guard = ncore::DecodeFlagsGuard::enter(requested);
                norito::codec::encode_with_header_flags(&fixture)
            };
            let field_len = if flags & ncore::header_flags::PACKED_STRUCT != 0 {
                1
            } else {
                ncore::len_prefix_len_with_flags(1, flags) + 1
            };
            assert_eq!(bare.len(), 4 + field_len);
            assert_eq!(
                AdmissionFixture::inbound_admission_class(&bare, flags).unwrap(),
                fixture.admission_class()
            );
            let mut trailing = bare.clone();
            trailing.push(0);
            assert!(AdmissionFixture::inbound_admission_class(&trailing, flags).is_err());
            assert!(
                AdmissionFixture::inbound_admission_class(&bare[..bare.len() - 1], flags).is_err()
            );
            let mut unknown = bare;
            unknown[..4].copy_from_slice(&99_u32.to_le_bytes());
            assert!(AdmissionFixture::inbound_admission_class(&unknown, flags).is_err());
        }
    }
}

#[test]
fn admission_class_indices_and_ordinary_topics_are_total() {
    for (index, class) in A::ALL.into_iter().enumerate() {
        assert_eq!(class.index(), index);
    }
    let topics = [
        (Topic::ConsensusSafety, A::Safety),
        (Topic::Consensus, A::Lane),
        (Topic::ConsensusChunk, A::Availability),
        (Topic::ConsensusPayload, A::Payload),
        (Topic::BlockSync, A::BlockSync),
        (Topic::Control, A::Control),
        (Topic::TxGossip, A::Low),
        (Topic::TxGossipRestricted, A::Low),
        (Topic::PeerGossip, A::Low),
        (Topic::TrustGossip, A::Low),
        (Topic::Health, A::Low),
        (Topic::Connect, A::Low),
        (Topic::Other, A::Low),
    ];
    struct Ordinary(Topic);
    impl ClassifyTopic for Ordinary {
        fn topic(&self) -> Topic {
            self.0
        }
    }
    for (topic, expected) in topics {
        assert_eq!(A::ordinary_for_topic(topic), expected);
        assert_eq!(Ordinary(topic).admission_class(), expected);
        assert!(!matches!(expected, A::RecoveryControl | A::RecoveryData));
    }
    assert!(
        Ordinary::inbound_admission_class(&[], 0).is_err(),
        "an absent raw classifier cannot fall back to the decoded topic"
    );
}

#[test]
fn ordinary_raw_admission_uses_only_the_raw_topic_owner() {
    struct Ordinary;
    impl ClassifyTopic for Ordinary {
        fn inbound_topic(payload: &[u8], _flags: u8) -> Result<Option<Topic>, ncore::Error> {
            match payload {
                [0] => Ok(Some(Topic::ConsensusChunk)),
                [1] => Ok(Some(Topic::Consensus)),
                [2] => Ok(None),
                _ => Err(ncore::Error::LengthMismatch),
            }
        }
    }
    assert_eq!(
        Ordinary::inbound_admission_class(&[0], 0).unwrap(),
        A::Availability
    );
    assert_eq!(Ordinary::inbound_admission_class(&[1], 0).unwrap(), A::Lane);
    assert!(Ordinary::inbound_admission_class(&[2], 0).is_err());
    assert!(Ordinary::inbound_admission_class(&[3], 0).is_err());
}

#[test]
fn relay_envelope_preserves_admission_classes_and_rejects_malformed_fields() {
    let key = KeyPair::try_from_seed(vec![61; 32], Algorithm::BlsNormal).unwrap();
    for fixture in AdmissionFixture::all() {
        let expected = fixture.admission_class();
        for target in [
            RelayTarget::Broadcast,
            RelayTarget::Direct(key.public_key().clone().into()),
        ] {
            let relay = RelayMessage::new_signed(&key, target, 1, fixture.clone());
            relay
                .verify_origin_signature()
                .expect("real signed relay fixture");
            assert_eq!(relay.admission_class(), expected);
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
                    norito::codec::encode_with_header_flags(&relay)
                };
                let decode_guard = ncore::DecodeFlagsGuard::enter(flags);
                let (decoded, consumed) =
                    ncore::decode_field_canonical::<RelayMessage<AdmissionFixture>>(&bare)
                        .expect("decode canonical relay under its advertised flags");
                assert_eq!(consumed, bare.len());
                assert_eq!(decoded.admission_class(), expected);
                assert_eq!(decoded.topic(), relay.topic());
                assert_eq!(
                    super::relay_message_wire_payload_len(
                        matches!(&relay.target, RelayTarget::Direct(_)),
                        ncore::encoded_payload_len(&relay.payload).unwrap(),
                        flags,
                    ),
                    Some(bare.len()),
                    "exact relay size witness follows its declared layout"
                );
                // Origin signatures bind canonical payload semantics. Restore
                // the signing context after decoding the selected frame layout.
                drop(decode_guard);
                decoded
                    .verify_origin_signature()
                    .expect("decoded relay retains its real origin signature");
                assert_eq!(
                    RelayMessage::<AdmissionFixture>::inbound_admission_class(&bare, flags)
                        .unwrap_or_else(|error| {
                            panic!(
                                "relay raw classification failed for flags {flags:#04x}: {error}"
                            )
                        }),
                    expected
                );
                if flags & ncore::header_flags::FIELD_BITSET != 0 {
                    assert_eq!(bare[0], 0b0001_0011);
                    let mut bad_bitset = bare.clone();
                    bad_bitset[0] |= 1 << 3;
                    assert!(
                        RelayMessage::<AdmissionFixture>::inbound_admission_class(
                            &bad_bitset,
                            flags
                        )
                        .is_err(),
                        "a signature-size bit is not part of the declared relay type"
                    );
                    let mut sizes = [0; 3];
                    let mut data_offset = 1;
                    for size in &mut sizes {
                        let (len, used) =
                            ncore::read_len_from_slice_with_flags(&bare[data_offset..], flags)
                                .unwrap();
                        *size = len;
                        data_offset += used;
                    }
                    let signature_offset = data_offset + sizes[0] + sizes[1] + 1;
                    let (signature_len, prefix) =
                        ncore::inspect_seq_len_slice(&bare[signature_offset..]).unwrap();
                    assert_eq!(signature_len, relay.origin_signature.len());
                    assert_eq!(prefix, 8);
                    let mut bad_signature_size = bare.clone();
                    bad_signature_size[signature_offset..signature_offset + prefix]
                        .copy_from_slice(&u64::MAX.to_le_bytes());
                    assert!(
                        RelayMessage::<AdmissionFixture>::inbound_admission_class(
                            &bad_signature_size,
                            flags
                        )
                        .is_err(),
                        "an unbounded self-delimiting signature cannot hide the payload"
                    );
                }
                let mut trailing = bare.clone();
                trailing.push(0);
                assert!(
                    RelayMessage::<AdmissionFixture>::inbound_admission_class(&trailing, flags)
                        .is_err()
                );
                assert!(
                    RelayMessage::<AdmissionFixture>::inbound_admission_class(
                        &bare[..bare.len() - 1],
                        flags
                    )
                    .is_err()
                );
                let nested = super::relay_message_payload_field(&bare, flags).unwrap();
                let offset = bare.len() - nested.len();
                let mut unknown = bare.clone();
                unknown[offset..offset + 4].copy_from_slice(&99_u32.to_le_bytes());
                assert!(
                    RelayMessage::<AdmissionFixture>::inbound_admission_class(&unknown, flags)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn semantic_subscribers_separate_recovery_from_shared_topics_and_reject_overlap() {
    use super::SubscriberFilter;
    use super::message::SubscriberRoute;
    for expected in A::ALL {
        let filter = SubscriberFilter::semantic_class(expected);
        for fixture in AdmissionFixture::all() {
            assert_eq!(
                filter.matches(
                    fixture.topic(),
                    SubscriberRoute::General,
                    fixture.admission_class()
                ),
                expected == fixture.admission_class()
            );
            assert!(!filter.matches(
                fixture.topic(),
                SubscriberRoute::ToriiProxy,
                fixture.admission_class()
            ));
        }
        for other in A::ALL {
            if expected != other {
                assert!(!filter.overlaps_reliable(&SubscriberFilter::semantic_class(other)));
            }
        }
    }
    let broad_chunk = SubscriberFilter::topics([Topic::ConsensusChunk]);
    assert!(broad_chunk.overlaps_reliable(&SubscriberFilter::semantic_class(A::Availability)));
    assert!(broad_chunk.overlaps_reliable(&SubscriberFilter::semantic_class(A::RecoveryData)));
    assert!(!broad_chunk.overlaps_reliable(&SubscriberFilter::semantic_class(A::Safety)));
    assert!(
        SubscriberFilter::topics([Topic::Consensus])
            .overlaps_reliable(&SubscriberFilter::semantic_class(A::RecoveryControl))
    );
}

#[test]
fn admission_schedule_preserves_low_priority_without_starving_low_service() {
    for class in A::ALL {
        let ranks = A::SCHEDULE
            .into_iter()
            .filter(|rank| *rank == class)
            .count();
        assert_eq!(ranks, if class.is_low() { 1 } else { 2 });
        assert_eq!(class.is_low(), A::LOW.contains(&class));
    }
    assert_eq!(A::ordinary_for_topic(Topic::BlockSync), A::BlockSync);
    assert_eq!(
        Topic::BlockSync.scheduling_priority(),
        super::message::Priority::Low
    );
    assert!(!A::Availability.is_low());
    assert_ne!(A::Availability, A::RecoveryData);
}

#[test]
fn actor_semantic_sources_do_not_collapse_availability_or_recovery_into_body() {
    use super::ActorProgressClass as C;
    let expected = [
        Some(C::Safety),
        Some(C::Lane),
        Some(C::Bulk),
        Some(C::Availability),
        Some(C::RecoveryControl),
        Some(C::RecoveryData),
        None,
        Some(C::Bulk),
        None,
    ];
    for (fixture, expected) in AdmissionFixture::all().into_iter().zip(expected) {
        assert_eq!(C::for_payload(&fixture), expected);
    }
}

#[test]
fn actor_waiters_preserve_lane_producers_and_the_original_total() {
    use super::{
        ActorProgressByteLimits, ActorProgressClass as C, NetworkActorProgressBudget,
        RELIABLE_PROGRESS_WAITERS_PER_SOURCE, actor_waiter_limits,
    };
    let limits = actor_waiter_limits().unwrap();
    assert_eq!(
        limits[C::Lane.index()],
        RELIABLE_PROGRESS_WAITERS_PER_SOURCE
    );
    assert_eq!(
        limits.into_iter().sum::<usize>(),
        3 * RELIABLE_PROGRESS_WAITERS_PER_SOURCE
    );
    assert!(
        C::ALL
            .into_iter()
            .filter(|class| *class != C::Lane)
            .all(|class| limits[class.index()] == 26)
    );
    let total = 4 * 3 * RELIABLE_PROGRESS_WAITERS_PER_SOURCE;
    assert!(
        NetworkActorProgressBudget::new_classed(ActorProgressByteLimits::uniform(1024), 4, total)
            .is_some()
    );
    assert!(
        NetworkActorProgressBudget::new_classed(
            ActorProgressByteLimits::uniform(1024),
            4,
            total + 1
        )
        .is_none()
    );
}

#[test]
fn actor_semantic_reserves_transfer_exact_bytes_and_counts_without_expansion() {
    use super::{ActorProgressByteLimits, semantic_actor_geometry};
    use iroha_crypto::encryption::ChaCha20Poly1305;
    let old = ActorProgressByteLimits {
        safety: 100,
        lane: 200,
        bulk: 300,
        availability: 0,
        recovery_control: 0,
        recovery_data: 0,
    };
    let maxima = [100; A::COUNT];
    let targets = 8;
    let high = 8192;
    let count = 128;
    let new =
        semantic_actor_geometry::<ChaCha20Poly1305>(old, maxima, targets, high, count).unwrap();
    assert_eq!(
        new.ordinary_high_bytes + new.progress.checked_per_target_total().unwrap() * targets,
        high + old.checked_per_target_total().unwrap() * targets
    );
    assert_eq!(new.ordinary_high_count + 6 * targets, count + 3 * targets);
    assert!(semantic_actor_geometry::<ChaCha20Poly1305>(old, maxima, targets, 300, count).is_err());
    assert!(
        semantic_actor_geometry::<ChaCha20Poly1305>(old, maxima, targets, high, 3 * targets)
            .is_err()
    );
    assert!(
        semantic_actor_geometry::<ChaCha20Poly1305>(old, maxima, usize::MAX, high, count).is_err()
    );
}

#[tokio::test]
async fn semantic_actor_and_partial_record_owners_bypass_blocked_payload_then_service_it() {
    let key = KeyPair::try_from_seed(vec![69; 32], Algorithm::BlsNormal).unwrap();
    super::assert_payload_availability_progress_for_test(
        &key,
        AdmissionFixture::Payload(3),
        AdmissionFixture::Availability(4),
    )
    .await;
}

#[test]
fn admission_class_codes_bind_geometry_and_record_order() {
    let expected = [
        (A::Safety, 0_u8),
        (A::Lane, 1),
        (A::Payload, 2),
        (A::Availability, 3),
        (A::RecoveryControl, 4),
        (A::RecoveryData, 5),
        (A::Control, 6),
        (A::BlockSync, 7),
        (A::Low, 8),
    ];
    assert_eq!(A::ALL, expected.map(|(class, _)| class));
    for (class, code) in expected {
        assert_eq!(class.wire_code(), code);
        assert_eq!(class.index(), usize::from(code));
    }
}
