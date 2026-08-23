use super::*;
use crate::offline::{
    KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendReleaseActivationV4,
    KagemushaRecursiveSpendTopUpRequestV4, KagemushaV4IssuanceEnableWitnessV1,
    KagemushaV4PromotionBindingV1, KagemushaV4ReleaseCancellationV1,
    KagemushaV4ReleaseDeactivationV1, KagemushaV4ReleaseLifecycleValidationError,
    KagemushaV4TairaCanaryPermitV1, KagemushaV4TairaCanaryReservationV1,
    OfflineDeviceAttestationPolicy, OfflineDeviceAttestationRegistration,
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
    /// Atomically publish one device-attestation policy and activate one signed ABI-21 release.
    pub struct ActivateKagemushaRecursiveReleaseV4 {
        /// Complete authenticated release activation payload.
        pub activation: KagemushaRecursiveSpendReleaseActivationV4,
        /// Exact governed device-attestation policy installed with the release.
        pub device_attestation_policy: OfflineDeviceAttestationPolicy,
        /// Controller-signed promotion identity committed for post-activation verification.
        pub promotion_binding: KagemushaV4PromotionBindingV1,
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
iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Enable public Kagemusha V4 issuance after signed post-canary liveness closure.
    pub struct EnableKagemushaRecursiveIssuanceV4 {
        /// Complete bounded staged-to-enabled evidence package.
        pub witness: KagemushaV4IssuanceEnableWitnessV1,
    }
}
impl PartialOrd for EnableKagemushaRecursiveIssuanceV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for EnableKagemushaRecursiveIssuanceV4 {
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
    /// Permanently cancel a staged Kagemusha V4 release before issuance is enabled.
    pub struct CancelKagemushaRecursiveReleaseV4 {
        /// Exact predecessor-bound governance cancellation.
        pub cancellation: KagemushaV4ReleaseCancellationV1,
    }
}
impl PartialOrd for CancelKagemushaRecursiveReleaseV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CancelKagemushaRecursiveReleaseV4 {
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
    /// Permanently stop new issuance from an enabled Kagemusha V4 release.
    pub struct DeactivateKagemushaRecursiveIssuanceV4 {
        /// Exact predecessor-bound governance deactivation.
        pub deactivation: KagemushaV4ReleaseDeactivationV1,
    }
}
impl PartialOrd for DeactivateKagemushaRecursiveIssuanceV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DeactivateKagemushaRecursiveIssuanceV4 {
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
    /// Consume one controller-signed, activation-bound Taira canary permit.
    pub struct RecordKagemushaTairaCanaryV4 {
        /// Exact controller permit authenticated again during consensus execution.
        pub permit: KagemushaV4TairaCanaryPermitV1,
    }
}
impl PartialOrd for RecordKagemushaTairaCanaryV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RecordKagemushaTairaCanaryV4 {
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
    /// Publish one controller-signed exact canary transaction authorization.
    pub struct AuthorizeKagemushaTairaCanaryV4 {
        /// Signed exact-hash projection that withholds the canary transaction wire.
        pub reservation: KagemushaV4TairaCanaryReservationV1,
    }
}
impl PartialOrd for AuthorizeKagemushaTairaCanaryV4 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AuthorizeKagemushaTairaCanaryV4 {
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
impl crate::seal::Instruction for EnableKagemushaRecursiveIssuanceV4 {}
impl crate::seal::Instruction for CancelKagemushaRecursiveReleaseV4 {}
impl crate::seal::Instruction for DeactivateKagemushaRecursiveIssuanceV4 {}
impl crate::seal::Instruction for RecordKagemushaTairaCanaryV4 {}
impl crate::seal::Instruction for AuthorizeKagemushaTairaCanaryV4 {}
impl crate::seal::Instruction for RegisterOfflineDeviceAttestation {}
impl crate::seal::Instruction for SetOfflineDeviceAttestationPolicy {}
impl TopUpKagemushaRecursiveV4 {
    /// Construct a scale-bound ABI-21 Kagemusha top-up instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendTopUpRequestV4) -> Self {
        Self { request }
    }
}
impl RedeemKagemushaRecursiveV4 {
    /// Construct a branch-safe ABI-21 Kagemusha redemption instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendRedeemRequestV4) -> Self {
        Self { request }
    }
}
impl ActivateKagemushaRecursiveReleaseV4 {
    /// Construct an atomic device-policy and ABI-21 release activation instruction.
    #[must_use]
    pub fn new(
        promotion_binding: KagemushaV4PromotionBindingV1,
        activation: KagemushaRecursiveSpendReleaseActivationV4,
        device_attestation_policy: OfflineDeviceAttestationPolicy,
    ) -> Self {
        Self {
            activation,
            device_attestation_policy,
            promotion_binding,
        }
    }

    /// Return the promotion-run identity committed by this activation.
    #[must_use]
    pub const fn promotion_id(&self) -> &[u8; 32] {
        &self.promotion_binding.promotion_id
    }

    /// Validate the promotion-run identity before consensus mutation.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed-width identity is all zeroes.
    pub fn validate_promotion_id(&self) -> Result<(), &'static str> {
        if self.promotion_binding.promotion_id == [0; 32] {
            return Err("Kagemusha V4 promotion id must be nonzero");
        }
        Ok(())
    }
}
impl EnableKagemushaRecursiveIssuanceV4 {
    /// Stable wire identifier for staged-to-enabled issuance transition.
    pub const WIRE_ID: &'static str = "iroha.offline.kagemusha.recursive_release.enable.v1";

    /// Construct a staged-to-enabled issuance transition.
    #[must_use]
    pub fn new(witness: KagemushaV4IssuanceEnableWitnessV1) -> Self {
        Self { witness }
    }

    /// Validate the bounded witness before consensus execution.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for malformed,
    /// oversized, or cross-spliced lifecycle evidence.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.witness.validate()
    }
}
impl CancelKagemushaRecursiveReleaseV4 {
    /// Stable wire identifier for staged release cancellation.
    pub const WIRE_ID: &'static str = "iroha.offline.kagemusha.recursive_release.cancel.v1";

    /// Construct a predecessor-bound staged release cancellation.
    #[must_use]
    pub fn new(cancellation: KagemushaV4ReleaseCancellationV1) -> Self {
        Self { cancellation }
    }

    /// Validate the cancellation before consensus execution.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for malformed or
    /// oversized transition data.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.cancellation.validate()
    }
}
impl DeactivateKagemushaRecursiveIssuanceV4 {
    /// Stable wire identifier for enabled issuance deactivation.
    pub const WIRE_ID: &'static str = "iroha.offline.kagemusha.recursive_release.deactivate.v1";

    /// Construct a predecessor-bound issuance deactivation.
    #[must_use]
    pub fn new(deactivation: KagemushaV4ReleaseDeactivationV1) -> Self {
        Self { deactivation }
    }

    /// Validate the deactivation before consensus execution.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaV4ReleaseLifecycleValidationError`] for malformed or
    /// oversized transition data.
    pub fn validate(&self) -> Result<(), KagemushaV4ReleaseLifecycleValidationError> {
        self.deactivation.validate()
    }
}
impl RecordKagemushaTairaCanaryV4 {
    /// Stable wire identifier for the consensus canary record instruction.
    pub const WIRE_ID: &'static str = "iroha.offline.kagemusha.taira_canary.record.v1";

    /// Construct an exact controller-permitted Taira canary record.
    #[must_use]
    pub fn new(permit: KagemushaV4TairaCanaryPermitV1) -> Self {
        Self { permit }
    }
}
impl AuthorizeKagemushaTairaCanaryV4 {
    /// Stable wire identifier for exact canary authorization publication.
    pub const WIRE_ID: &'static str = "iroha.offline.kagemusha.taira_canary.authorize.v1";

    /// Construct a publication for one controller-signed exact canary transaction.
    #[must_use]
    pub fn new(reservation: KagemushaV4TairaCanaryReservationV1) -> Self {
        Self { reservation }
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
const KAGEMUSHA_V4_ISSUANCE_ENABLE_ISI_DECODE_STACK_BYTES: usize = 32 * 1024 * 1024;
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
        let promotion_binding = super::decode_aos_canonical_field::<KagemushaV4PromotionBindingV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                activation,
                device_attestation_policy,
                promotion_binding,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for EnableKagemushaRecursiveIssuanceV4 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        std::thread::scope(|scope| {
            let decoder = std::thread::Builder::new()
                .name("kagemusha-enable-isi-decode".to_owned())
                .stack_size(KAGEMUSHA_V4_ISSUANCE_ENABLE_ISI_DECODE_STACK_BYTES)
                .spawn_scoped(scope, move || {
                    let _guard = norito::core::DecodeFlagsGuard::enter(flags);
                    if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                        return super::decode_packed_instruction_payload::<Self>(bytes);
                    }
                    let mut offset = 0usize;
                    let witness = super::decode_aos_canonical_field::<
                        KagemushaV4IssuanceEnableWitnessV1,
                    >(
                        super::read_aos_field(bytes, &mut offset, flags)?, flags
                    )?;
                    if offset != bytes.len() {
                        return Err(norito::core::Error::LengthMismatch);
                    }
                    norito::core::note_payload_access(bytes, offset);
                    Ok((Self { witness }, offset))
                })
                .map_err(|_| norito::core::Error::LengthMismatch)?;
            decoder
                .join()
                .map_err(|_| norito::core::Error::LengthMismatch)?
        })
    }
}
impl_decode_one_canonical_offline_field!(TopUpKagemushaRecursiveV4 {
    request: KagemushaRecursiveSpendTopUpRequestV4
});
impl_decode_one_canonical_offline_field!(RedeemKagemushaRecursiveV4 {
    request: KagemushaRecursiveSpendRedeemRequestV4
});
impl_decode_one_canonical_offline_field!(CancelKagemushaRecursiveReleaseV4 {
    cancellation: KagemushaV4ReleaseCancellationV1
});
impl_decode_one_canonical_offline_field!(DeactivateKagemushaRecursiveIssuanceV4 {
    deactivation: KagemushaV4ReleaseDeactivationV1
});
impl_decode_one_canonical_offline_field!(RecordKagemushaTairaCanaryV4 {
    permit: KagemushaV4TairaCanaryPermitV1
});
impl_decode_one_canonical_offline_field!(AuthorizeKagemushaTairaCanaryV4 {
    reservation: KagemushaV4TairaCanaryReservationV1
});
impl_decode_one_canonical_offline_field!(RegisterOfflineDeviceAttestation {
    registration: OfflineDeviceAttestationRegistration
});
impl_decode_one_canonical_offline_field!(SetOfflineDeviceAttestationPolicy {
    policy: OfflineDeviceAttestationPolicy
});
#[cfg(test)]
mod tests {
    use super::*;
    use crate::offline::{
        KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1, KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1,
        KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1, KagemushaDevicePublicKeyV2,
        KagemushaExactBytesDigestV1, KagemushaV4ReleaseLifecycleReasonV1,
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::core::NoritoDeserialize as _;
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
    fn exact_digest(seed: u8) -> KagemushaExactBytesDigestV1 {
        KagemushaExactBytesDigestV1 {
            byte_len: u64::from(seed) + 1,
            sha256: [seed.max(1); 32],
        }
    }
    fn cancellation_fixture() -> KagemushaV4ReleaseCancellationV1 {
        KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [0x31; 32],
            manifest_sha256: [0x32; 32],
            expected_predecessor_lifecycle: exact_digest(0x33),
            transition_id: [0x34; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: Some(exact_digest(0x35)),
        }
    }
    fn deactivation_fixture() -> KagemushaV4ReleaseDeactivationV1 {
        KagemushaV4ReleaseDeactivationV1 {
            schema: KAGEMUSHA_V4_RELEASE_DEACTIVATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [0x31; 32],
            manifest_sha256: [0x32; 32],
            expected_predecessor_lifecycle: exact_digest(0x36),
            transition_id: [0x37; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::EmergencyDeactivation,
            evidence: None,
        }
    }
    fn assert_instruction_layout_roundtrip<T>(value: T, flags: u8)
    where
        T: Clone
            + core::fmt::Debug
            + PartialEq
            + Encode
            + for<'a> norito::core::DecodeFromSlice<'a>,
    {
        let (payload, encoded_flags) = {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            norito::codec::encode_with_header_flags(&value)
        };
        assert_eq!(encoded_flags & flags, flags);
        let (decoded, used) = {
            let _guard = norito::core::DecodeFlagsGuard::enter(encoded_flags);
            T::decode_from_slice(&payload).expect("decode instruction payload")
        };
        assert_eq!(used, payload.len());
        assert_eq!(decoded, value);

        let mut trailing = payload;
        trailing.push(0xA5);
        let outcome = {
            let _guard = norito::core::DecodeFlagsGuard::enter(encoded_flags);
            T::decode_from_slice(&trailing)
        };
        assert!(
            outcome.is_err(),
            "instruction decoder accepted trailing bytes"
        );
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
    #[test]
    fn release_lifecycle_terminal_isis_validate_box_and_decode_both_layouts() {
        let cancellation = CancelKagemushaRecursiveReleaseV4::new(cancellation_fixture());
        let deactivation = DeactivateKagemushaRecursiveIssuanceV4::new(deactivation_fixture());
        cancellation.validate().expect("valid cancellation ISI");
        deactivation.validate().expect("valid deactivation ISI");

        for flags in [
            norito::core::default_encode_flags(),
            norito::core::header_flags::PACKED_STRUCT
                | norito::core::header_flags::COMPACT_LEN
                | norito::core::header_flags::FIELD_BITSET,
        ] {
            assert_instruction_layout_roundtrip(cancellation.clone(), flags);
            assert_instruction_layout_roundtrip(deactivation.clone(), flags);
        }

        for (boxed, wire_id) in [
            (
                InstructionBox::from(cancellation),
                CancelKagemushaRecursiveReleaseV4::WIRE_ID,
            ),
            (
                InstructionBox::from(deactivation),
                DeactivateKagemushaRecursiveIssuanceV4::WIRE_ID,
            ),
        ] {
            assert_eq!(crate::isi::instruction_wire_id(&boxed), Some(wire_id));
        }
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn release_lifecycle_enable_isi_boxes_and_rejects_trailing_bytes_in_both_layouts() {
        let instruction = EnableKagemushaRecursiveIssuanceV4::new(
            crate::offline::lifecycle_enable_witness_wire_fixture(),
        );
        instruction.validate().expect("valid enable ISI");
        for flags in [
            norito::core::default_encode_flags(),
            norito::core::header_flags::PACKED_STRUCT
                | norito::core::header_flags::COMPACT_LEN
                | norito::core::header_flags::FIELD_BITSET,
        ] {
            assert_instruction_layout_roundtrip(instruction.clone(), flags);
        }
        let boxed = InstructionBox::from(instruction);
        assert_eq!(
            crate::isi::instruction_wire_id(&boxed),
            Some(EnableKagemushaRecursiveIssuanceV4::WIRE_ID)
        );
    }
}
