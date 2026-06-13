use super::*;

/// Register a domain endorsement committee.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct RegisterDomainCommittee {
    /// Committee configuration to register.
    pub committee: crate::nexus::DomainCommittee,
}

/// Set or replace the endorsement policy for a domain.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct SetDomainEndorsementPolicy {
    /// Domain requiring endorsements.
    pub domain: crate::domain::DomainId,
    /// Policy to apply.
    pub policy: crate::nexus::DomainEndorsementPolicy,
}

/// Submit an endorsement for a protected domain.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[getset(get = "pub")]
pub struct SubmitDomainEndorsement {
    /// Endorsement to validate and record.
    pub endorsement: crate::nexus::DomainEndorsement,
}

impl crate::seal::Instruction for RegisterDomainCommittee {}
impl crate::seal::Instruction for SetDomainEndorsementPolicy {}
impl crate::seal::Instruction for SubmitDomainEndorsement {}

fn endorsement_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterDomainCommittee {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = endorsement_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let committee = super::decode_aos_canonical_field::<crate::nexus::DomainCommittee>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { committee }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetDomainEndorsementPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = endorsement_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let domain = super::decode_aos_canonical_field::<crate::domain::DomainId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let policy = super::decode_aos_canonical_field::<crate::nexus::DomainEndorsementPolicy>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { domain, policy }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SubmitDomainEndorsement {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = endorsement_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let endorsement = super::decode_aos_canonical_field::<crate::nexus::DomainEndorsement>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { endorsement }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        domain::DomainId,
        metadata::Metadata,
        nexus::{
            DOMAIN_ENDORSEMENT_VERSION_V1, DomainCommittee, DomainEndorsement,
            DomainEndorsementPolicy, DomainEndorsementScope, DomainEndorsementSignature,
        },
    };

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
    }

    fn public_key(seed: u8) -> PublicKey {
        key_pair(seed).public_key().clone()
    }

    fn domain_id() -> DomainId {
        DomainId::try_new("wonderland", "universal").expect("domain id")
    }

    fn committee() -> DomainCommittee {
        DomainCommittee {
            committee_id: "retail".to_owned(),
            members: vec![public_key(0x91), public_key(0x92)],
            quorum: 2,
            metadata: Metadata::default(),
        }
    }

    fn policy() -> DomainEndorsementPolicy {
        DomainEndorsementPolicy {
            committee_id: "retail".to_owned(),
            max_endorsement_age: 100,
            required: true,
        }
    }

    fn endorsement() -> DomainEndorsement {
        let signer = key_pair(0x93);
        let mut endorsement = DomainEndorsement {
            version: DOMAIN_ENDORSEMENT_VERSION_V1,
            domain_id: domain_id(),
            committee_id: "retail".to_owned(),
            statement_hash: Hash::new(b"wonderland@universal"),
            issued_at_height: 10,
            expires_at_height: 110,
            scope: DomainEndorsementScope {
                dataspace: None,
                block_start: Some(10),
                block_end: Some(110),
            },
            signatures: Vec::new(),
            metadata: Metadata::default(),
        };
        let body_hash = endorsement.body_hash();
        endorsement.signatures.push(DomainEndorsementSignature {
            signer: signer.public_key().clone(),
            signature: iroha_crypto::Signature::try_new(signer.private_key(), body_hash.as_ref())
                .expect("checked domain endorsement ISI fixture signature"),
        });
        endorsement
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

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &'static str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn endorsement_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterDomainCommittee {
            committee: committee(),
        });
        assert_slice_roundtrip(SetDomainEndorsementPolicy {
            domain: domain_id(),
            policy: policy(),
        });
        assert_slice_roundtrip(SubmitDomainEndorsement {
            endorsement: endorsement(),
        });
    }

    #[test]
    fn endorsement_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterDomainCommittee>("nexus::RegisterDomainCommittee")
            .register_with_id_slice::<SetDomainEndorsementPolicy>(
                "nexus::SetDomainEndorsementPolicy",
            )
            .register_with_id_slice::<SubmitDomainEndorsement>("nexus::SubmitDomainEndorsement");

        assert_registry_decodes(
            &registry,
            "nexus::RegisterDomainCommittee",
            RegisterDomainCommittee {
                committee: committee(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::SetDomainEndorsementPolicy",
            SetDomainEndorsementPolicy {
                domain: domain_id(),
                policy: policy(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::SubmitDomainEndorsement",
            SubmitDomainEndorsement {
                endorsement: endorsement(),
            },
        );
    }
}
