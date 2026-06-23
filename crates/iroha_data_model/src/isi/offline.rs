use super::*;
use crate::{
    asset::AssetDefinitionId,
    offline::{
        KagemushaRecursiveSpendBundleV1, KagemushaRecursiveSpendLineageWitnessV1,
        OfflineDeviceAttestationPolicy, OfflineDeviceAttestationRegistration,
        OfflineNoteAuditBundle, OfflineNoteIssue, OfflineNoteRedeem,
    },
    proof::ProofAttachment,
};

isi! {
    /// Issue a production Offline bearer note.
    pub struct IssueOfflineNote {
        /// Compact note issuance record.
        pub issue: OfflineNoteIssue,
    }
}

isi! {
    /// Redeem a production Offline bearer note claim.
    ///
    /// This instruction consumes a claim that is already known to ledger state. For a
    /// peer-to-peer bearer output that has not been audited yet, submit the ordered
    /// `AuditOfflineNote` lineage before this instruction in the same transaction.
    pub struct RedeemOfflineNote {
        /// Compact recursive proof and consumed nullifiers.
        pub redemption: OfflineNoteRedeem,
    }
}

isi! {
    /// Submit an Offline audit bundle.
    ///
    /// Audits are optional for offline transfer finality, but they are the lineage that makes
    /// peer-to-peer bearer outputs recognizable to the ledger before later defunding.
    pub struct AuditOfflineNote {
        /// Compact audit payload.
        pub audit: OfflineNoteAuditBundle,
    }
}

/// Compatibility alias for the first-release Offline note issue instruction.
pub type IssueOfflineNoteV2 = IssueOfflineNote;
/// Compatibility alias for the first-release Offline note redeem instruction.
pub type RedeemOfflineNoteV2 = RedeemOfflineNote;
/// Compatibility alias for the first-release Offline note audit instruction.
pub type AuditOfflineNoteV2 = AuditOfflineNote;

isi! {
    /// Settle a Kagemusha offline-offline shielded transfer.
    ///
    /// This is the default private offline-offline settlement surface. It uses the same
    /// transparent shielded ledger accumulator as ZK assets: input nullifiers are consumed,
    /// output commitments are appended, and the proof must be verified against the asset's
    /// configured transparent verifier.
    pub struct KagemushaTransfer {
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Spent nullifiers.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub inputs: Vec<[u8; 32]>,
        /// Output note commitments.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub outputs: Vec<[u8; 32]>,
        /// Proof attachment for the private transfer.
        pub proof: ProofAttachment,
        /// Optional recent Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

isi! {
    /// Redeem recursive Kagemusha offline cash into an online public balance.
    ///
    /// This instruction is submitted by the final offline holder. It verifies the
    /// constant-size recursive spend bundle, then verifies the final redeem proof
    /// against the bundle's current spendable note descriptor. Exact redeem
    /// mints the full note amount; partial redeem also appends the proof-bound
    /// private change commitment.
    pub struct RedeemKagemushaRecursive {
        /// Final holder's recursive Kagemusha spend bundle.
        pub bundle: KagemushaRecursiveSpendBundleV1,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Public amount credited online.
        pub public_amount: u128,
        /// Final unshield/redeem proof bound to the current note descriptor.
        pub redeem_proof: ProofAttachment,
        /// Optional record-backed lineage witness used for production chain admission.
        pub lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
        /// Optional private change note commitment for partial redemption.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub change_output: Option<[u8; 32]>,
    }
}

isi! {
    /// Register a platform-attested Offline note key for trustless issuance.
    pub struct RegisterOfflineDeviceAttestation {
        /// Platform attestation registration material.
        pub registration: OfflineDeviceAttestationRegistration,
    }
}

isi! {
    /// Replace the governed Offline device-attestation verifier policy.
    pub struct SetOfflineDeviceAttestationPolicy {
        /// Verifier policy to store on-chain.
        pub policy: OfflineDeviceAttestationPolicy,
    }
}

impl crate::seal::Instruction for IssueOfflineNote {}
impl crate::seal::Instruction for RedeemOfflineNote {}
impl crate::seal::Instruction for AuditOfflineNote {}
impl crate::seal::Instruction for KagemushaTransfer {}
impl crate::seal::Instruction for RedeemKagemushaRecursive {}
impl crate::seal::Instruction for RegisterOfflineDeviceAttestation {}
impl crate::seal::Instruction for SetOfflineDeviceAttestationPolicy {}

impl IssueOfflineNote {
    /// Construct an Offline note issuance instruction.
    #[must_use]
    pub fn new(issue: OfflineNoteIssue) -> Self {
        Self { issue }
    }
}

impl RedeemOfflineNote {
    /// Construct an Offline note redemption instruction.
    #[must_use]
    pub fn new(redemption: OfflineNoteRedeem) -> Self {
        Self { redemption }
    }
}

impl AuditOfflineNote {
    /// Construct an Offline optional audit instruction.
    #[must_use]
    pub fn new(audit: OfflineNoteAuditBundle) -> Self {
        Self { audit }
    }
}

impl KagemushaTransfer {
    /// Construct a Kagemusha shielded offline-offline transfer instruction.
    #[must_use]
    pub fn new(
        asset: AssetDefinitionId,
        inputs: Vec<[u8; 32]>,
        outputs: Vec<[u8; 32]>,
        proof: ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            asset,
            inputs,
            outputs,
            proof,
            root_hint,
        }
    }
}

impl RedeemKagemushaRecursive {
    /// Construct a recursive Kagemusha redemption instruction.
    #[must_use]
    pub fn new(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
    ) -> Self {
        Self::new_with_lineage_witness(bundle, recipient, public_amount, redeem_proof, None)
    }

    /// Construct a recursive Kagemusha redemption instruction with lineage witness material.
    #[must_use]
    pub fn new_with_lineage_witness(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
        lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
    ) -> Self {
        Self::new_with_lineage_witness_and_change(
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            lineage_witness,
            None,
        )
    }

    /// Construct a recursive Kagemusha redemption instruction with optional private change.
    #[must_use]
    pub fn new_with_change(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
        change_output: Option<[u8; 32]>,
    ) -> Self {
        Self::new_with_lineage_witness_and_change(
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            None,
            change_output,
        )
    }

    /// Construct a recursive Kagemusha redemption instruction with lineage material and change.
    #[must_use]
    pub fn new_with_lineage_witness_and_change(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
        lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
        change_output: Option<[u8; 32]>,
    ) -> Self {
        Self {
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            lineage_witness,
            change_output,
        }
    }
}

impl RegisterOfflineDeviceAttestation {
    /// Construct an Offline device attestation registration instruction.
    #[must_use]
    pub fn new(registration: OfflineDeviceAttestationRegistration) -> Self {
        Self { registration }
    }
}

impl SetOfflineDeviceAttestationPolicy {
    /// Construct an Offline device attestation policy instruction.
    #[must_use]
    pub fn new(policy: OfflineDeviceAttestationPolicy) -> Self {
        Self { policy }
    }
}

fn offline_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_one_offline_field {
    ($ty:ident { $field:ident: $field_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = offline_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $field = super::decode_aos_slice_field::<$field_ty>(
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

impl_decode_one_offline_field!(IssueOfflineNote {
    issue: OfflineNoteIssue
});
impl_decode_one_offline_field!(RedeemOfflineNote {
    redemption: OfflineNoteRedeem
});
impl_decode_one_offline_field!(AuditOfflineNote {
    audit: OfflineNoteAuditBundle
});

impl<'a> norito::core::DecodeFromSlice<'a> for RedeemKagemushaRecursive {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let bundle = super::decode_aos_canonical_field::<KagemushaRecursiveSpendBundleV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let recipient = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let public_amount = super::decode_aos_canonical_field::<u128>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let redeem_proof = super::decode_aos_canonical_field::<ProofAttachment>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let lineage_witness = super::decode_aos_canonical_field::<
            Option<KagemushaRecursiveSpendLineageWitnessV1>,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let change_output = super::decode_aos_canonical_field::<Option<[u8; 32]>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                bundle,
                recipient,
                public_amount,
                redeem_proof,
                lineage_witness,
                change_output,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for KagemushaTransfer {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let asset = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let inputs = super::decode_aos_canonical_field::<Vec<[u8; 32]>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let outputs = super::decode_aos_canonical_field::<Vec<[u8; 32]>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof = super::decode_aos_canonical_field::<ProofAttachment>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let root_hint = super::decode_aos_canonical_field::<Option<[u8; 32]>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                asset,
                inputs,
                outputs,
                proof,
                root_hint,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterOfflineDeviceAttestation {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let registration = super::decode_aos_canonical_field::<OfflineDeviceAttestationRegistration>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { registration }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetOfflineDeviceAttestationPolicy {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = offline_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let policy = super::decode_aos_canonical_field::<OfflineDeviceAttestationPolicy>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { policy }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_primitives::numeric::Numeric;
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        offline::{
            OfflineAndroidAppAttestationPolicy, OfflineDeviceAttestationTrustedRoot,
            OfflineIosAppAttestationPolicy, OfflineNoteAuditOutputClaim, OfflineNoteIssuedClaim,
            OfflineNoteKeyCertificate, OfflineNoteRecursiveProof,
        },
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    };

    fn account() -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![0xC1; 32], Algorithm::Ed25519)
            .expect("derive checked offline-note fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_id(account_id: &AccountId) -> AssetId {
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        AssetId::of(asset_definition_id, account_id.clone())
    }

    fn key_certificate(account_id: AccountId) -> OfflineNoteKeyCertificate {
        OfflineNoteKeyCertificate {
            version: crate::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id,
            public_key: vec![0x01, 0x02, 0x03],
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: Signature::from_bytes(&[0xAB; 64]),
        }
    }

    fn proof() -> OfflineNoteRecursiveProof {
        OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-recursive"),
            public_inputs_hash: Hash::new(b"offline-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        }
    }

    fn issue() -> OfflineNoteIssue {
        let account_id = account();
        OfflineNoteIssue {
            note_commitment: Hash::new(b"note-commitment"),
            key_certificate: key_certificate(account_id.clone()),
            asset: asset_id(&account_id),
            amount: Numeric::new(10, 0),
        }
    }

    fn redemption() -> OfflineNoteRedeem {
        let account_id = account();
        OfflineNoteRedeem {
            source_note_commitment: Hash::new(b"note-commitment"),
            input_nullifiers: vec![Hash::new(b"input-nullifier")],
            sender_key_certificate: key_certificate(account_id.clone()),
            recipient: account_id.clone(),
            asset: asset_id(&account_id),
            amount: Numeric::new(10, 0),
            recursive_proof: proof(),
        }
    }

    fn audit() -> OfflineNoteAuditBundle {
        let issue = issue();
        OfflineNoteAuditBundle {
            token_id: Hash::new(b"token"),
            sender_key_certificate: issue.key_certificate.clone(),
            input_nullifiers: vec![Hash::new(b"audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaim::from_issue(&issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"output-note"),
                key_certificate: issue.key_certificate,
                asset: issue.asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof(),
        }
    }

    fn attestation_registration() -> OfflineDeviceAttestationRegistration {
        let account_id = account();
        let certificate = key_certificate(account_id.clone());
        let attestation_report = b"offline-attestation-report".to_vec();
        let evidence = b"offline-attestation-evidence".to_vec();
        OfflineDeviceAttestationRegistration {
            version: 1,
            platform: certificate.platform,
            key_id: certificate.key_id,
            device_id: certificate.device_id,
            account_id,
            asset_definition_id: Some(asset_id(&account()).definition().clone()),
            ios_team_id: Some("TEAMID1234".to_owned()),
            ios_bundle_id: Some("jp.co.soramitsu.iroha.offline".to_owned()),
            ios_environment: Some("production".to_owned()),
            android_package_name: None,
            android_signing_certificate_sha256: None,
            public_key: certificate.public_key,
            assertion_scheme: certificate.assertion_scheme,
            assertion_key_algorithm: certificate.assertion_key_algorithm,
            assertion_public_key: certificate.assertion_public_key,
            assertion_usage_count_limit: certificate.assertion_usage_count_limit,
            one_use: certificate.one_use,
            challenge_hash: Hash::new(b"offline-attestation-challenge"),
            attestation_report_hash: Hash::new(&attestation_report),
            attestation_report,
            evidence_hash: Hash::new(&evidence),
            evidence,
            recent_block_height: 1,
            recent_block_hash: Hash::new(b"offline-attestation-block"),
            expires_at_ms: 10_000,
        }
    }

    fn attestation_policy() -> OfflineDeviceAttestationPolicy {
        OfflineDeviceAttestationPolicy {
            version: 1,
            trusted_roots: vec![OfflineDeviceAttestationTrustedRoot {
                platform: "android-keymint".to_owned(),
                der: vec![0x30, 0x03, 0x02, 0x01, 0x01],
                not_before_ms: Some(1),
                not_after_ms: Some(10_000),
            }],
            revoked_certificate_sha256: vec![vec![0xA5; 32]],
            ios_apps: vec![OfflineIosAppAttestationPolicy {
                team_id: "TEAMID1234".to_owned(),
                bundle_id: "jp.co.soramitsu.iroha.offline".to_owned(),
                environment: "production".to_owned(),
            }],
            android_apps: vec![OfflineAndroidAppAttestationPolicy {
                package_name: "jp.co.soramitsu.iroha.offline".to_owned(),
                signing_certificate_sha256: vec![vec![0xC3; 32]],
            }],
            require_ios_app_policy: true,
            require_android_app_policy: true,
        }
    }

    fn kagemusha_transfer() -> KagemushaTransfer {
        KagemushaTransfer::new(
            issue().asset.definition().clone(),
            vec![[0x11; 32]],
            vec![[0x22; 32], [0x33; 32]],
            ProofAttachment::new_ref(
                "halo2/ipa".into(),
                ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
                VerifyingKeyId::new("halo2/ipa", "offline-kagemusha-transfer"),
            ),
            Some([0x44; 32]),
        )
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

    fn kotlin_nested_audit_instruction_frame() -> Vec<u8> {
        let audit = audit();
        let (inner_payload, inner_flags) = norito::codec::encode_with_header_flags(&audit);
        assert!(
            inner_flags & norito::core::header_flags::COMPACT_LEN != 0,
            "Kotlin encodeAudit uses COMPACT_LEN"
        );
        let inner_framed = norito::core::frame_bare_with_header_flags::<OfflineNoteAuditBundle>(
            &inner_payload,
            inner_flags,
        )
        .expect("frame inner audit bundle");
        assert_eq!(&inner_framed[..4], b"NRT0");

        let mut wrapper_payload = Vec::new();
        norito::core::write_len_with_flags(&mut wrapper_payload, inner_framed.len() as u64, 0)
            .expect("write fixed wrapper field length");
        wrapper_payload.extend_from_slice(&inner_framed);

        let framed =
            norito::core::frame_bare_with_header_flags::<AuditOfflineNote>(&wrapper_payload, 0)
                .expect("frame outer audit instruction");
        assert_eq!(framed[norito::core::Header::SIZE - 1], 0);
        framed
    }

    fn kotlin_bare_audit_instruction_frame() -> Vec<u8> {
        let audit = audit();
        let (bare_payload, flags) = {
            let _guard =
                norito::core::DecodeFlagsGuard::enter(norito::core::header_flags::COMPACT_LEN);
            norito::codec::encode_with_header_flags(&audit)
        };
        assert_eq!(
            flags,
            norito::core::header_flags::COMPACT_LEN,
            "fixed Kotlin instruction wrappers encode the model payload with wrapper flags"
        );

        let mut wrapper_payload = Vec::new();
        norito::core::write_len_with_flags(&mut wrapper_payload, bare_payload.len() as u64, flags)
            .expect("write wrapper field length");
        wrapper_payload.extend_from_slice(&bare_payload);

        let framed =
            norito::core::frame_bare_with_header_flags::<AuditOfflineNote>(&wrapper_payload, flags)
                .expect("frame outer audit instruction");
        assert_eq!(framed[norito::core::Header::SIZE - 1], flags);
        framed
    }

    #[test]
    fn offline_note_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(IssueOfflineNote::new(issue()));
        assert_slice_roundtrip(RedeemOfflineNote::new(redemption()));
        assert_slice_roundtrip(AuditOfflineNote::new(audit()));
        assert_slice_roundtrip(kagemusha_transfer());
        assert_slice_roundtrip(RegisterOfflineDeviceAttestation::new(
            attestation_registration(),
        ));
        assert_slice_roundtrip(SetOfflineDeviceAttestationPolicy::new(attestation_policy()));
    }

    #[test]
    fn offline_note_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<IssueOfflineNote>()
            .register_slice::<RedeemOfflineNote>()
            .register_slice::<AuditOfflineNote>()
            .register_slice::<KagemushaTransfer>()
            .register_slice::<RegisterOfflineDeviceAttestation>()
            .register_slice::<SetOfflineDeviceAttestationPolicy>();

        assert_registry_decodes(&registry, IssueOfflineNote::new(issue()));
        assert_registry_decodes(&registry, RedeemOfflineNote::new(redemption()));
        assert_registry_decodes(&registry, AuditOfflineNote::new(audit()));
        assert_registry_decodes(&registry, kagemusha_transfer());
        assert_registry_decodes(
            &registry,
            RegisterOfflineDeviceAttestation::new(attestation_registration()),
        );
        assert_registry_decodes(
            &registry,
            SetOfflineDeviceAttestationPolicy::new(attestation_policy()),
        );
    }

    #[test]
    fn kotlin_nested_audit_instruction_reproduces_length_mismatch() {
        let registry = crate::isi::InstructionRegistry::new().register_slice::<AuditOfflineNote>();
        let framed = kotlin_nested_audit_instruction_frame();
        let err = crate::isi::InstructionRegistry::decode(
            &registry,
            std::any::type_name::<AuditOfflineNote>(),
            &framed,
        )
        .expect("registered")
        .expect_err("Kotlin nested audit wrapper should not decode as a bare Rust bundle");

        // Legacy Kotlin emitted a second Norito header inside the wrapper field.
        assert!(matches!(err, norito::Error::LengthMismatch), "{err:?}");
    }

    #[test]
    fn kotlin_bare_audit_instruction_decodes_as_rust_instruction() {
        let registry = crate::isi::InstructionRegistry::new().register_slice::<AuditOfflineNote>();
        let expected = AuditOfflineNote::new(audit());
        let framed = kotlin_bare_audit_instruction_frame();
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            std::any::type_name::<AuditOfflineNote>(),
            &framed,
        )
        .expect("registered")
        .expect("bare Kotlin wrapper decodes");

        assert_eq!(
            crate::isi::Instruction::dyn_encode(&*decoded),
            expected.encode()
        );
    }
}
