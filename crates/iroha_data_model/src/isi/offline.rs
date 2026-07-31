use super::*;
use crate::{
    asset::NewAssetDefinition,
    governance::types::ParliamentEnactmentCertificate,
    nexus::DataSpaceId,
    offline::{
        KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendReleaseActivationV4,
        KagemushaRecursiveSpendTopUpRequestV4, OfflineDeviceAttestationPolicy,
        OfflineDeviceAttestationRegistration, OfflineNoteAuditBundle, OfflineNoteIssue,
        OfflineNoteRedeem,
    },
};
use iroha_crypto::blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};

/// Domain separator for governed offline-asset bootstrap manifests.
pub const OFFLINE_ASSET_BOOTSTRAP_MANIFEST_FINGERPRINT_DOMAIN_V1: &[u8] =
    b"iroha:offline:asset-bootstrap-manifest:v1";
/// Domain separator for the ordered pre-enactment instruction commitment.
pub const OFFLINE_ASSET_BOOTSTRAP_PREFIX_FINGERPRINT_DOMAIN_V1: &[u8] =
    b"iroha:offline:asset-bootstrap-prefix:v1";

/// Commit to the exact ordered instructions preceding an offline bootstrap
/// enactment in one transaction.
#[must_use]
pub fn offline_asset_bootstrap_prefix_fingerprint(
    instructions: &[crate::isi::InstructionBox],
) -> [u8; 32] {
    let encoded = Encode::encode(&instructions.to_vec());
    let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length");
    hasher.update(OFFLINE_ASSET_BOOTSTRAP_PREFIX_FINGERPRINT_DOMAIN_V1);
    hasher.update(&encoded);
    let mut out = [0_u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("finalize Blake2bVar");
    out
}

#[cfg(test)]
mod bootstrap_prefix_tests {
    use super::*;
    use crate::{Level, isi::Log};

    #[test]
    fn offline_bootstrap_prefix_fingerprint_binds_order_and_exact_bytes() {
        let first: crate::isi::InstructionBox = Log::new(Level::INFO, "first".to_owned()).into();
        let second: crate::isi::InstructionBox = Log::new(Level::WARN, "second".to_owned()).into();
        let ordered = offline_asset_bootstrap_prefix_fingerprint(&[first.clone(), second.clone()]);
        assert_ne!(
            ordered,
            offline_asset_bootstrap_prefix_fingerprint(&[second, first.clone()]),
            "instruction reordering must change the certificate-bound prefix"
        );
        assert_ne!(
            ordered,
            offline_asset_bootstrap_prefix_fingerprint(&[first]),
            "instruction omission must change the certificate-bound prefix"
        );
        assert_ne!(
            ordered,
            offline_asset_bootstrap_prefix_fingerprint(&[]),
            "an empty prefix cannot substitute a non-empty plan"
        );
    }
}

/// Complete phase-two manifest authorized by a Parliament enactment certificate.
///
/// The manifest deliberately carries the exact asset, ZK policy, issuer, escrow, and
/// authenticated ABI-21/V4 release. The corresponding instruction has no caller-supplied
/// permission list, so a valid certificate cannot be reused to expand privileges.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct OfflineAssetBootstrapManifestV1 {
    /// Chain on which this exact bootstrap may be enacted.
    pub chain_id: crate::ChainId,
    /// Sole directly signed transaction authority permitted to enact this manifest.
    ///
    /// This is also the asset-definition owner. Binding it into the fingerprint
    /// prevents a valid Parliament certificate from being replayed under a
    /// different transaction signer or through a trigger/contract authority.
    pub enactment_authority: crate::account::AccountId,
    /// Proposal-time JIT Parliament selection height bound into the authorization.
    pub parliament_selection_height: u64,
    /// Commitment to the exact recomputed JIT Parliament bodies.
    ///
    /// This prevents a certificate created for one eligible-citizen snapshot from
    /// being replayed after the same-height roster state has been substituted.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub parliament_roster_root: [u8; 32],
    /// Commitment to every ordered instruction preceding this enactment in the
    /// same transaction.
    ///
    /// The enactment instruction must be the final instruction. Together with
    /// [`Self::fingerprint`], this binds the Parliament certificate to the complete
    /// phase-two transaction without a self-referential transaction hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub preceding_instructions_hash: [u8; 32],
    /// Exact dataspace on whose route the asset must be registered.
    pub dataspace_id: DataSpaceId,
    /// Complete domainless fixed-scale asset definition.
    pub asset_definition: NewAssetDefinition,
    /// Account recorded as owner of the registered asset definition.
    pub asset_definition_owner: crate::account::AccountId,
    /// Exact hybrid ZK registration for the same asset definition.
    pub zk_asset: crate::isi::zk::RegisterZkAsset,
    /// Exact issuer receiving the three fixed offline lifecycle permissions.
    pub issuer: crate::account::AccountId,
    /// Exact positive initial balances minted atomically by the enactment.
    ///
    /// Keeping the allocations inside the fingerprinted manifest lets the
    /// enactment remain the final transaction instruction while still making
    /// the issuer and FI reserves usable immediately after commit.
    pub initial_allocations:
        std::collections::BTreeMap<crate::account::AccountId, iroha_primitives::numeric::Quantity>,
    /// Deterministic chain-and-asset-derived offline escrow account.
    pub escrow: crate::account::AccountId,
    /// Complete authenticated asset-specific ABI-21/V4 release activation.
    pub release: ActivateKagemushaRecursiveReleaseV4,
}

impl OfflineAssetBootstrapManifestV1 {
    /// Compute the deterministic certificate preimage fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let encoded = Encode::encode(self);
        let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length");
        hasher.update(OFFLINE_ASSET_BOOTSTRAP_MANIFEST_FINGERPRINT_DOMAIN_V1);
        hasher.update(&encoded);
        let mut out = [0_u8; 32];
        hasher
            .finalize_variable(&mut out)
            .expect("finalize Blake2bVar");
        out
    }
}

isi! {
    /// Issue a legacy BOI offline bearer note into ledger-recognized escrow.
    pub struct IssueOfflineNote {
        /// Compact note issuance record.
        pub issue: OfflineNoteIssue,
    }
}

isi! {
    /// Redeem a ledger-recognized legacy BOI offline bearer note.
    pub struct RedeemOfflineNote {
        /// Compact proof and consumed nullifiers.
        pub redemption: OfflineNoteRedeem,
    }
}

isi! {
    /// Anchor an optional legacy BOI offline-note audit lineage.
    pub struct AuditOfflineNote {
        /// Compact audit payload.
        pub audit: OfflineNoteAuditBundle,
    }
}

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

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    /// Enact one exact post-genesis offline asset bootstrap authorized by Parliament.
    pub struct EnactOfflineAssetBootstrapV1 {
        /// Exact immutable bootstrap manifest.
        pub manifest: OfflineAssetBootstrapManifestV1,
        /// Threshold Parliament signatures over the manifest fingerprint and enactment window.
        pub certificate: ParliamentEnactmentCertificate,
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[getset(get = "pub")]
    /// Atomically publish one device-attestation policy and activate one signed ABI-21 release.
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

impl PartialOrd for EnactOfflineAssetBootstrapV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EnactOfflineAssetBootstrapV1 {
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
impl crate::seal::Instruction for EnactOfflineAssetBootstrapV1 {}
impl crate::seal::Instruction for RegisterOfflineDeviceAttestation {}
impl crate::seal::Instruction for SetOfflineDeviceAttestationPolicy {}
impl crate::seal::Instruction for IssueOfflineNote {}
impl crate::seal::Instruction for RedeemOfflineNote {}
impl crate::seal::Instruction for AuditOfflineNote {}

impl IssueOfflineNote {
    /// Construct a legacy note issuance instruction.
    #[must_use]
    pub fn new(issue: OfflineNoteIssue) -> Self {
        Self { issue }
    }
}

impl RedeemOfflineNote {
    /// Construct a legacy note redemption instruction.
    #[must_use]
    pub fn new(redemption: OfflineNoteRedeem) -> Self {
        Self { redemption }
    }
}

impl AuditOfflineNote {
    /// Construct an optional legacy note audit instruction.
    #[must_use]
    pub fn new(audit: OfflineNoteAuditBundle) -> Self {
        Self { audit }
    }
}

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
        activation: KagemushaRecursiveSpendReleaseActivationV4,
        device_attestation_policy: OfflineDeviceAttestationPolicy,
    ) -> Self {
        Self {
            activation,
            device_attestation_policy,
        }
    }
}

impl EnactOfflineAssetBootstrapV1 {
    /// Stable public wire identifier for the governed phase-two lifecycle.
    pub const WIRE_ID: &'static str = "iroha.offline.asset_bootstrap.enact.v1";

    /// Construct an exact governed offline-asset bootstrap instruction.
    #[must_use]
    pub fn new(
        manifest: OfflineAssetBootstrapManifestV1,
        certificate: ParliamentEnactmentCertificate,
    ) -> Self {
        Self {
            manifest,
            certificate,
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

macro_rules! impl_decode_one_legacy_offline_field {
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

impl_decode_one_legacy_offline_field!(IssueOfflineNote {
    issue: OfflineNoteIssue
});
impl_decode_one_legacy_offline_field!(RedeemOfflineNote {
    redemption: OfflineNoteRedeem
});
impl_decode_one_legacy_offline_field!(AuditOfflineNote {
    audit: OfflineNoteAuditBundle
});

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

impl<'a> norito::core::DecodeFromSlice<'a> for EnactOfflineAssetBootstrapV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<OfflineAssetBootstrapManifestV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let certificate = super::decode_aos_canonical_field::<ParliamentEnactmentCertificate>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                manifest,
                certificate,
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
