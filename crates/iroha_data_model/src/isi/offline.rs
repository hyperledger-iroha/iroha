use super::*;
use crate::offline::{
    KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendReleaseActivationV4,
    KagemushaRecursiveSpendTopUpRequestV4, OfflineDeviceAttestationPolicy,
    OfflineDeviceAttestationRegistration,
};

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Charge an online balance and create the first scale-bound Kagemusha state.
    pub struct TopUpKagemushaRecursiveV4 {
        /// Canonical top-up request, including payer and device authorization.
        pub request: KagemushaRecursiveSpendTopUpRequestV4,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Redeem a branch-safe, scale-bound Kagemusha state.
    pub struct RedeemKagemushaRecursiveV4 {
        /// Canonical redemption request with recursive-lineage and unshield evidence.
        pub request: KagemushaRecursiveSpendRedeemRequestV4,
    }
}

impl PartialOrd for TopUpKagemushaRecursiveV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TopUpKagemushaRecursiveV4 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl PartialOrd for RedeemKagemushaRecursiveV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RedeemKagemushaRecursiveV4 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Atomically publish one device-attestation policy and activate one signed ABI-20 release.
    pub struct ActivateKagemushaRecursiveReleaseV4 {
        /// Complete authenticated release activation payload.
        pub activation: KagemushaRecursiveSpendReleaseActivationV4,
        /// Exact governed device-attestation policy installed with the release.
        pub device_attestation_policy: OfflineDeviceAttestationPolicy,
    }
}

impl PartialOrd for ActivateKagemushaRecursiveReleaseV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ActivateKagemushaRecursiveReleaseV4 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

isi! {
    /// Register a platform-attested Kagemusha device authority.
    pub struct RegisterOfflineDeviceAttestation {
        /// Platform attestation registration material.
        pub registration: OfflineDeviceAttestationRegistration,
    }
}

isi! {
    /// Replace the governed Kagemusha device-attestation verifier policy.
    pub struct SetOfflineDeviceAttestationPolicy {
        /// Verifier policy to store on-chain.
        pub policy: OfflineDeviceAttestationPolicy,
    }
}

impl crate::seal::Instruction for TopUpKagemushaRecursiveV4 {}
impl crate::seal::Instruction for RedeemKagemushaRecursiveV4 {}
impl crate::seal::Instruction for ActivateKagemushaRecursiveReleaseV4 {}
impl crate::seal::Instruction for RegisterOfflineDeviceAttestation {}
impl crate::seal::Instruction for SetOfflineDeviceAttestationPolicy {}

impl TopUpKagemushaRecursiveV4 {
    /// Construct a scale-bound ABI-20 Kagemusha top-up instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendTopUpRequestV4) -> Self {
        Self { request }
    }
}

impl RedeemKagemushaRecursiveV4 {
    /// Construct a branch-safe ABI-20 Kagemusha redemption instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendRedeemRequestV4) -> Self {
        Self { request }
    }
}

impl ActivateKagemushaRecursiveReleaseV4 {
    /// Construct an atomic device-policy and ABI-20 release activation instruction.
    #[must_use]
    pub fn new(
        activation: KagemushaRecursiveSpendReleaseActivationV4,
        device_attestation_policy: OfflineDeviceAttestationPolicy,
    ) -> Self {
        Self {
            activation,
            device_attestation_policy,
        }
    }
}

impl RegisterOfflineDeviceAttestation {
    /// Stable wire identifier used to frame device-attestation registrations.
    pub const WIRE_ID: &'static str = "iroha.offline.device_attestation.register";

    /// Construct a Kagemusha device-attestation registration instruction.
    #[must_use]
    pub fn new(registration: OfflineDeviceAttestationRegistration) -> Self {
        Self { registration }
    }
}

impl SetOfflineDeviceAttestationPolicy {
    /// Construct a Kagemusha device-attestation policy instruction.
    #[must_use]
    pub fn new(policy: OfflineDeviceAttestationPolicy) -> Self {
        Self { policy }
    }
}

fn offline_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_one_canonical_offline_field {
    ($ty:ident { $field:ident: $field_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = offline_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $field = super::decode_aos_canonical_field::<$field_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $field }, offset))
            }
        }
    };
}

impl<'a> norito::core::DecodeFromSlice<'a> for ActivateKagemushaRecursiveReleaseV4 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let activation = super::decode_aos_canonical_field::<
            KagemushaRecursiveSpendReleaseActivationV4,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let device_attestation_policy = super::decode_aos_canonical_field::<
            OfflineDeviceAttestationPolicy,
        >(
            super::read_aos_field(bytes, &mut offset, flags)?, flags
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                activation,
                device_attestation_policy,
            },
            offset,
        ))
    }
}

impl_decode_one_canonical_offline_field!(TopUpKagemushaRecursiveV4 {
    request: KagemushaRecursiveSpendTopUpRequestV4
});
impl_decode_one_canonical_offline_field!(RedeemKagemushaRecursiveV4 {
    request: KagemushaRecursiveSpendRedeemRequestV4
});
impl_decode_one_canonical_offline_field!(RegisterOfflineDeviceAttestation {
    registration: OfflineDeviceAttestationRegistration
});
impl_decode_one_canonical_offline_field!(SetOfflineDeviceAttestationPolicy {
    policy: OfflineDeviceAttestationPolicy
});

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::core::NoritoDeserialize as _;

    use super::*;
    use crate::offline::KagemushaDevicePublicKeyV2;

    fn registration_fixture() -> OfflineDeviceAttestationRegistration {
        let account_key = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519)
            .expect("derive checked offline attestation fixture keypair");
        let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(&[
            0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
            0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
            0x45, 0xd8, 0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e,
            0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e,
            0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
        ])
        .expect("canonical uncompressed P-256 generator point");
        let attestation_report = b"offline-attestation-roundtrip-report".to_vec();
        let attestation_report_hash = Hash::new(&attestation_report);
        let evidence = b"offline-attestation-roundtrip-evidence".to_vec();

        OfflineDeviceAttestationRegistration {
            version: 1,
            platform: "android-keymint".to_owned(),
            key_id: "offline-attestation-roundtrip-key".to_owned(),
            device_id: "offline-attestation-roundtrip-device".to_owned(),
            account_id: AccountId::new(account_key.public_key().clone()),
            asset_definition_id: None,
            ios_team_id: None,
            ios_bundle_id: None,
            ios_environment: None,
            android_package_name: Some("org.hyperledger.iroha.roundtrip".to_owned()),
            android_signing_certificate_sha256: Some(vec![0x51; 32]),
            public_key,
            assertion_scheme: "android-keymint".to_owned(),
            assertion_key_algorithm: "ecdsa-p256-sha256".to_owned(),
            assertion_public_key: vec![0x52; 65],
            assertion_usage_count_limit: Some(1),
            one_use: true,
            challenge_hash: Hash::new(b"offline-attestation-roundtrip-challenge"),
            attestation_report_hash,
            attestation_report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 42,
            recent_block_hash: Hash::new(b"offline-attestation-roundtrip-block"),
            expires_at_ms: 2_000_000_000_000,
        }
    }

    #[test]
    fn device_attestation_instruction_uses_stable_wire_id_and_roundtrips() {
        let instruction = RegisterOfflineDeviceAttestation::new(registration_fixture());
        let boxed = InstructionBox::from(instruction.clone());

        assert_eq!(
            crate::isi::instruction_wire_id(&boxed),
            Some(RegisterOfflineDeviceAttestation::WIRE_ID)
        );

        let bytes = norito::core::to_bytes(&boxed).expect("serialize instruction box");
        let archived = norito::core::from_bytes::<InstructionBox>(&bytes)
            .expect("decode instruction box archive");
        let decoded = InstructionBox::try_deserialize(archived)
            .expect("deserialize device attestation instruction");
        assert_eq!(
            decoded
                .as_any()
                .downcast_ref::<RegisterOfflineDeviceAttestation>(),
            Some(&instruction)
        );
    }
}
