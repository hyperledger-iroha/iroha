use super::*;

isi! {
    /// Claim a promotional reward for an active Twitter follow binding.
    pub struct ClaimTwitterFollowReward {
        /// Binding hash (keyed) proven by the soracles feed.
        pub binding_hash: crate::oracle::KeyedHash,
    }
}

isi! {
    /// Send a reward to a Twitter handle; funds are escrowed until the binding appears.
    pub struct SendToTwitter {
        /// Binding hash (keyed) for the target handle.
        pub binding_hash: crate::oracle::KeyedHash,
        /// Amount to escrow or deliver immediately.
        pub amount: iroha_primitives::numeric::Numeric,
    }
}

isi! {
    /// Cancel an existing escrow created by [`SendToTwitter`].
    pub struct CancelTwitterEscrow {
        /// Binding hash (keyed) for the escrow.
        pub binding_hash: crate::oracle::KeyedHash,
    }
}

impl crate::seal::Instruction for ClaimTwitterFollowReward {}
impl crate::seal::Instruction for SendToTwitter {}
impl crate::seal::Instruction for CancelTwitterEscrow {}

fn social_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for ClaimTwitterFollowReward {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = social_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let binding_hash = super::decode_aos_canonical_field::<crate::oracle::KeyedHash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { binding_hash }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SendToTwitter {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = social_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let binding_hash = super::decode_aos_canonical_field::<crate::oracle::KeyedHash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let amount = super::decode_aos_canonical_field::<iroha_primitives::numeric::Numeric>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                binding_hash,
                amount,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for CancelTwitterEscrow {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = social_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let binding_hash = super::decode_aos_canonical_field::<crate::oracle::KeyedHash>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { binding_hash }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::Numeric;
    use norito::core::DecodeFromSlice;

    use super::*;

    fn binding_hash() -> crate::oracle::KeyedHash {
        crate::oracle::KeyedHash::new("pepper-social-v1", b"pepper", b"twitter_user_123")
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
    fn social_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(ClaimTwitterFollowReward {
            binding_hash: binding_hash(),
        });
        assert_slice_roundtrip(SendToTwitter {
            binding_hash: binding_hash(),
            amount: Numeric::from(12_u64),
        });
        assert_slice_roundtrip(CancelTwitterEscrow {
            binding_hash: binding_hash(),
        });
    }

    #[test]
    fn social_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<ClaimTwitterFollowReward>()
            .register_slice::<SendToTwitter>()
            .register_slice::<CancelTwitterEscrow>();

        assert_registry_decodes(
            &registry,
            ClaimTwitterFollowReward {
                binding_hash: binding_hash(),
            },
        );
        assert_registry_decodes(
            &registry,
            SendToTwitter {
                binding_hash: binding_hash(),
                amount: Numeric::from(12_u64),
            },
        );
        assert_registry_decodes(
            &registry,
            CancelTwitterEscrow {
                binding_hash: binding_hash(),
            },
        );
    }
}
