use super::*;
use crate::offline::{
    KagemushaRecursiveSpendRedeemRequestV2, KagemushaRecursiveSpendTopUpRequestV2,
    OfflineDeviceAttestationPolicy, OfflineDeviceAttestationRegistration,
};

isi! {
    /// Charge an online balance and create the first scale-bound Kagemusha state.
    pub struct TopUpKagemushaRecursiveV2 {
        /// Canonical top-up request, including payer and device authorization.
        pub request: KagemushaRecursiveSpendTopUpRequestV2,
    }
}

isi! {
    /// Redeem a branch-safe, scale-bound Kagemusha state.
    pub struct RedeemKagemushaRecursiveV2 {
        /// Canonical redemption request with Reserved-lineage and unshield evidence.
        pub request: KagemushaRecursiveSpendRedeemRequestV2,
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

impl crate::seal::Instruction for TopUpKagemushaRecursiveV2 {}
impl crate::seal::Instruction for RedeemKagemushaRecursiveV2 {}
impl crate::seal::Instruction for RegisterOfflineDeviceAttestation {}
impl crate::seal::Instruction for SetOfflineDeviceAttestationPolicy {}

impl TopUpKagemushaRecursiveV2 {
    /// Construct a scale-bound Kagemusha top-up instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendTopUpRequestV2) -> Self {
        Self { request }
    }
}

impl RedeemKagemushaRecursiveV2 {
    /// Construct a branch-safe Kagemusha redemption instruction.
    #[must_use]
    pub fn new(request: KagemushaRecursiveSpendRedeemRequestV2) -> Self {
        Self { request }
    }
}

impl RegisterOfflineDeviceAttestation {
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

impl_decode_one_canonical_offline_field!(TopUpKagemushaRecursiveV2 {
    request: KagemushaRecursiveSpendTopUpRequestV2
});
impl_decode_one_canonical_offline_field!(RedeemKagemushaRecursiveV2 {
    request: KagemushaRecursiveSpendRedeemRequestV2
});
impl_decode_one_canonical_offline_field!(RegisterOfflineDeviceAttestation {
    registration: OfflineDeviceAttestationRegistration
});
impl_decode_one_canonical_offline_field!(SetOfflineDeviceAttestationPolicy {
    policy: OfflineDeviceAttestationPolicy
});
