use iroha_crypto::Hash;

use super::*;
use crate::kaigi::{
    KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiRelayHealthStatus,
    KaigiRelayManifest, KaigiRelayRegistration, NewKaigi,
};

isi! {
    /// Create a new Kaigi session anchored to a domain.
    pub struct CreateKaigi {
        /// Template describing the call to create.
        pub call: NewKaigi,
        /// Commitment describing the host (privacy mode only).
        pub commitment: Option<KaigiParticipantCommitment>,
        /// Nullifier preventing proof replay (privacy mode only).
        pub nullifier: Option<KaigiParticipantNullifier>,
        /// Merkle root the host used when generating the proof (privacy mode only).
        pub roster_root: Option<Hash>,
        /// Proof bytes attesting ownership of the commitment (privacy mode only).
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Add a participant to an active Kaigi.
    pub struct JoinKaigi {
        /// Identifier of the call to join.
        pub call_id: KaigiId,
        /// Account joining the call.
        pub participant: AccountId,
    /// Commitment describing the participant (privacy mode only).
    pub commitment: Option<KaigiParticipantCommitment>,
    /// Nullifier preventing duplicate joins (privacy mode only).
    pub nullifier: Option<KaigiParticipantNullifier>,
    /// Merkle root the participant used when generating their proof (privacy mode only).
    pub roster_root: Option<Hash>,
    /// Proof bytes attesting ownership of the commitment (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Remove a participant from an active Kaigi.
    pub struct LeaveKaigi {
        /// Identifier of the call to leave.
        pub call_id: KaigiId,
        /// Account leaving the call.
        pub participant: AccountId,
    /// Commitment describing the participant (privacy mode only).
    pub commitment: Option<KaigiParticipantCommitment>,
    /// Nullifier preventing duplicate leaves (privacy mode only).
    pub nullifier: Option<KaigiParticipantNullifier>,
    /// Merkle root the participant used when generating their proof (privacy mode only).
    pub roster_root: Option<Hash>,
    /// Proof bytes attesting ownership of the commitment (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Conclude an active Kaigi.
    pub struct EndKaigi {
        /// Identifier of the call to end.
        pub call_id: KaigiId,
        /// Optional timestamp in milliseconds when the call ended.
        pub ended_at_ms: Option<u64>,
        /// Commitment describing the host (privacy mode only).
        pub commitment: Option<KaigiParticipantCommitment>,
        /// Nullifier preventing proof replay (privacy mode only).
        pub nullifier: Option<KaigiParticipantNullifier>,
        /// Merkle root the host used when generating the proof (privacy mode only).
        pub roster_root: Option<Hash>,
        /// Proof bytes attesting ownership of the commitment (privacy mode only).
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub proof: Option<Vec<u8>>,
    }
}

isi! {
    /// Record usage metrics for a Kaigi segment.
    pub struct RecordKaigiUsage {
    /// Identifier of the call to update.
    pub call_id: KaigiId,
    /// Duration in milliseconds for this usage segment.
    pub duration_ms: u64,
    /// Gas billed for this segment (as computed off-ledger).
    pub billed_gas: u64,
    /// Commitment to the usage tuple (privacy mode only).
    pub usage_commitment: Option<Hash>,
    /// Optional proof tying the commitment to encrypted logs (privacy mode only).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Option<Vec<u8>>,
}
}

isi! {
    /// Update the relay manifest advertised for a Kaigi session.
    pub struct SetKaigiRelayManifest {
        /// Identifier of the call to update.
        pub call_id: KaigiId,
        /// Optional relay manifest describing the desired relay hops.
        pub relay_manifest: Option<KaigiRelayManifest>,
    }
}

isi! {
    /// Register or update a Kaigi relay within its home domain.
    pub struct RegisterKaigiRelay {
        /// Registration payload describing the relay capabilities.
        pub relay: KaigiRelayRegistration,
    }
}

isi! {
    /// Report the observed health for a relay participating in a Kaigi session.
    pub struct ReportKaigiRelayHealth {
        /// Identifier of the call where the relay was observed.
        pub call_id: KaigiId,
        /// Relay account whose health is being reported.
        pub relay_id: AccountId,
        /// Health status observed by the reporter.
        pub status: KaigiRelayHealthStatus,
        /// Timestamp (milliseconds since epoch) for when the observation occurred.
        pub reported_at_ms: u64,
        /// Optional free-form notes capturing failure context.
        pub notes: Option<String>,
    }
}

// Seal implementations
impl crate::seal::Instruction for CreateKaigi {}
impl crate::seal::Instruction for JoinKaigi {}
impl crate::seal::Instruction for LeaveKaigi {}
impl crate::seal::Instruction for EndKaigi {}
impl crate::seal::Instruction for RecordKaigiUsage {}
impl crate::seal::Instruction for SetKaigiRelayManifest {}
impl crate::seal::Instruction for RegisterKaigiRelay {}
impl crate::seal::Instruction for ReportKaigiRelayHealth {}

fn kaigi_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_kaigi_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = kaigi_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_kaigi_decode_from_slice!(CreateKaigi {
    call: NewKaigi,
    commitment: Option<KaigiParticipantCommitment>,
    nullifier: Option<KaigiParticipantNullifier>,
    roster_root: Option<Hash>,
    proof: Option<Vec<u8>>,
});

impl_kaigi_decode_from_slice!(JoinKaigi {
    call_id: KaigiId,
    participant: AccountId,
    commitment: Option<KaigiParticipantCommitment>,
    nullifier: Option<KaigiParticipantNullifier>,
    roster_root: Option<Hash>,
    proof: Option<Vec<u8>>,
});

impl_kaigi_decode_from_slice!(LeaveKaigi {
    call_id: KaigiId,
    participant: AccountId,
    commitment: Option<KaigiParticipantCommitment>,
    nullifier: Option<KaigiParticipantNullifier>,
    roster_root: Option<Hash>,
    proof: Option<Vec<u8>>,
});

impl_kaigi_decode_from_slice!(EndKaigi {
    call_id: KaigiId,
    ended_at_ms: Option<u64>,
    commitment: Option<KaigiParticipantCommitment>,
    nullifier: Option<KaigiParticipantNullifier>,
    roster_root: Option<Hash>,
    proof: Option<Vec<u8>>,
});

impl_kaigi_decode_from_slice!(RecordKaigiUsage {
    call_id: KaigiId,
    duration_ms: u64,
    billed_gas: u64,
    usage_commitment: Option<Hash>,
    proof: Option<Vec<u8>>,
});

impl_kaigi_decode_from_slice!(SetKaigiRelayManifest {
    call_id: KaigiId,
    relay_manifest: Option<KaigiRelayManifest>,
});

impl_kaigi_decode_from_slice!(RegisterKaigiRelay {
    relay: KaigiRelayRegistration,
});

impl_kaigi_decode_from_slice!(ReportKaigiRelayHealth {
    call_id: KaigiId,
    relay_id: AccountId,
    status: KaigiRelayHealthStatus,
    reported_at_ms: u64,
    notes: Option<String>,
});

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        domain::DomainId,
        kaigi::{KaigiPrivacyMode, KaigiRelayHop, KaigiRoomPolicy},
        name::Name,
    };

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn call_id() -> KaigiId {
        KaigiId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            Name::from_str("standup").expect("call name"),
        )
    }

    fn participant_commitment(byte: u8) -> KaigiParticipantCommitment {
        KaigiParticipantCommitment {
            commitment: Hash::new([byte; 32]),
            alias_tag: Some(format!("participant-{byte}")),
        }
    }

    fn participant_nullifier(byte: u8) -> KaigiParticipantNullifier {
        KaigiParticipantNullifier {
            digest: Hash::new([byte; 32]),
            issued_at_ms: 1_700_000_000 + u64::from(byte),
        }
    }

    fn relay_manifest() -> KaigiRelayManifest {
        KaigiRelayManifest {
            hops: vec![KaigiRelayHop {
                relay_id: account(3),
                hpke_public_key: vec![0xA1, 0xA2, 0xA3],
                weight: 2,
            }],
            expiry_ms: 1_700_100_000,
        }
    }

    fn relay_registration() -> KaigiRelayRegistration {
        KaigiRelayRegistration {
            relay_id: account(4),
            hpke_public_key: vec![0xB1, 0xB2, 0xB3],
            bandwidth_class: 7,
        }
    }

    fn new_kaigi() -> NewKaigi {
        let mut call = NewKaigi::with_defaults(call_id(), account(1));
        call.title = Some("Daily standup".to_owned());
        call.description = Some("Engineering sync".to_owned());
        call.max_participants = Some(16);
        call.gas_rate_per_minute = 5;
        call.scheduled_start_ms = Some(1_700_010_000);
        call.billing_account = Some(account(2));
        call.privacy_mode = KaigiPrivacyMode::ZkRosterV1;
        call.room_policy = KaigiRoomPolicy::Authenticated;
        call.relay_manifest = Some(relay_manifest());
        call
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn kaigi_decode_from_slice_roundtrips() {
        let call_id = call_id();
        let commitment = participant_commitment(0x11);
        let nullifier = participant_nullifier(0x22);
        let roster_root = Hash::new("kaigi-roster-root");

        assert_slice_roundtrip(CreateKaigi {
            call: new_kaigi(),
            commitment: Some(commitment.clone()),
            nullifier: Some(nullifier.clone()),
            roster_root: Some(roster_root),
            proof: Some(vec![0x01, 0x02, 0x03]),
        });
        assert_slice_roundtrip(JoinKaigi {
            call_id: call_id.clone(),
            participant: account(5),
            commitment: Some(commitment.clone()),
            nullifier: Some(nullifier.clone()),
            roster_root: Some(roster_root),
            proof: Some(vec![0x04, 0x05]),
        });
        assert_slice_roundtrip(LeaveKaigi {
            call_id: call_id.clone(),
            participant: account(5),
            commitment: Some(commitment.clone()),
            nullifier: Some(nullifier.clone()),
            roster_root: Some(roster_root),
            proof: Some(vec![0x06, 0x07]),
        });
        assert_slice_roundtrip(EndKaigi {
            call_id: call_id.clone(),
            ended_at_ms: Some(1_700_020_000),
            commitment: Some(commitment),
            nullifier: Some(nullifier),
            roster_root: Some(roster_root),
            proof: Some(vec![0x08, 0x09]),
        });
        assert_slice_roundtrip(RecordKaigiUsage {
            call_id: call_id.clone(),
            duration_ms: 60_000,
            billed_gas: 15,
            usage_commitment: Some(Hash::new("kaigi-usage")),
            proof: Some(vec![0x10, 0x11]),
        });
        assert_slice_roundtrip(SetKaigiRelayManifest {
            call_id: call_id.clone(),
            relay_manifest: Some(relay_manifest()),
        });
        assert_slice_roundtrip(RegisterKaigiRelay {
            relay: relay_registration(),
        });
        assert_slice_roundtrip(ReportKaigiRelayHealth {
            call_id,
            relay_id: account(4),
            status: KaigiRelayHealthStatus::Degraded,
            reported_at_ms: 1_700_030_000,
            notes: Some("packet loss".to_owned()),
        });
    }

    #[test]
    fn kaigi_default_registry_decodes_type_names() {
        let registry = crate::isi::registry::default();
        let call_id = call_id();

        assert_registry_decodes(
            &registry,
            CreateKaigi {
                call: new_kaigi(),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            },
        );
        assert_registry_decodes(
            &registry,
            JoinKaigi {
                call_id: call_id.clone(),
                participant: account(5),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            },
        );
        assert_registry_decodes(
            &registry,
            LeaveKaigi {
                call_id: call_id.clone(),
                participant: account(5),
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            },
        );
        assert_registry_decodes(
            &registry,
            EndKaigi {
                call_id: call_id.clone(),
                ended_at_ms: None,
                commitment: None,
                nullifier: None,
                roster_root: None,
                proof: None,
            },
        );
        assert_registry_decodes(
            &registry,
            RecordKaigiUsage {
                call_id: call_id.clone(),
                duration_ms: 60_000,
                billed_gas: 15,
                usage_commitment: None,
                proof: None,
            },
        );
        assert_registry_decodes(
            &registry,
            SetKaigiRelayManifest {
                call_id: call_id.clone(),
                relay_manifest: Some(relay_manifest()),
            },
        );
        assert_registry_decodes(
            &registry,
            RegisterKaigiRelay {
                relay: relay_registration(),
            },
        );
        assert_registry_decodes(
            &registry,
            ReportKaigiRelayHealth {
                call_id,
                relay_id: account(4),
                status: KaigiRelayHealthStatus::Healthy,
                reported_at_ms: 1_700_030_000,
                notes: None,
            },
        );
    }
}
