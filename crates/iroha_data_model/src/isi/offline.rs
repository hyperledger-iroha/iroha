use super::*;
use crate::offline::{OfflineNoteAuditBundleV2, OfflineNoteIssueV2, OfflineNoteRedeemV2};

isi! {
    /// Issue a production Offline V2 bearer note.
    pub struct IssueOfflineNoteV2 {
        /// Compact note issuance record.
        pub issue: OfflineNoteIssueV2,
    }
}

isi! {
    /// Redeem a production Offline V2 bearer note token.
    pub struct RedeemOfflineNoteV2 {
        /// Compact recursive proof and consumed nullifiers.
        pub redemption: OfflineNoteRedeemV2,
    }
}

isi! {
    /// Submit an optional Offline V2 audit bundle without requiring online settlement for finality.
    pub struct AuditOfflineNoteV2 {
        /// Compact audit payload.
        pub audit: OfflineNoteAuditBundleV2,
    }
}

impl crate::seal::Instruction for IssueOfflineNoteV2 {}
impl crate::seal::Instruction for RedeemOfflineNoteV2 {}
impl crate::seal::Instruction for AuditOfflineNoteV2 {}

impl IssueOfflineNoteV2 {
    /// Construct an Offline V2 note issuance instruction.
    #[must_use]
    pub fn new(issue: OfflineNoteIssueV2) -> Self {
        Self { issue }
    }
}

impl RedeemOfflineNoteV2 {
    /// Construct an Offline V2 note redemption instruction.
    #[must_use]
    pub fn new(redemption: OfflineNoteRedeemV2) -> Self {
        Self { redemption }
    }
}

impl AuditOfflineNoteV2 {
    /// Construct an Offline V2 optional audit instruction.
    #[must_use]
    pub fn new(audit: OfflineNoteAuditBundleV2) -> Self {
        Self { audit }
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

impl_decode_one_offline_field!(IssueOfflineNoteV2 {
    issue: OfflineNoteIssueV2
});
impl_decode_one_offline_field!(RedeemOfflineNoteV2 {
    redemption: OfflineNoteRedeemV2
});
impl_decode_one_offline_field!(AuditOfflineNoteV2 {
    audit: OfflineNoteAuditBundleV2
});

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
            OfflineNoteAuditOutputClaimV2, OfflineNoteIssuedClaimV2, OfflineNoteKeyCertificateV2,
            OfflineNoteRecursiveProofV2,
        },
        proof::{ProofBox, VerifyingKeyId},
    };

    fn account() -> AccountId {
        let key_pair = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_id(account_id: &AccountId) -> AssetId {
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        AssetId::of(asset_definition_id, account_id.clone())
    }

    fn key_certificate(account_id: AccountId) -> OfflineNoteKeyCertificateV2 {
        OfflineNoteKeyCertificateV2 {
            version: 2,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id,
            public_key: vec![0x01, 0x02, 0x03],
            assertion_scheme: "apple-appattest-counter-v1".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: Signature::from_bytes(&[0xAB; 64]),
        }
    }

    fn proof() -> OfflineNoteRecursiveProofV2 {
        OfflineNoteRecursiveProofV2 {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
            public_inputs_hash: Hash::new(b"offline-v2-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        }
    }

    fn issue() -> OfflineNoteIssueV2 {
        let account_id = account();
        OfflineNoteIssueV2 {
            note_commitment: Hash::new(b"note-commitment"),
            key_certificate: key_certificate(account_id.clone()),
            asset: asset_id(&account_id),
            amount: Numeric::new(10, 0),
        }
    }

    fn redemption() -> OfflineNoteRedeemV2 {
        let account_id = account();
        OfflineNoteRedeemV2 {
            source_note_commitment: Hash::new(b"note-commitment"),
            input_nullifiers: vec![Hash::new(b"input-nullifier")],
            sender_key_certificate: key_certificate(account_id.clone()),
            recipient: account_id.clone(),
            asset: asset_id(&account_id),
            amount: Numeric::new(10, 0),
            recursive_proof: proof(),
        }
    }

    fn audit() -> OfflineNoteAuditBundleV2 {
        let issue = issue();
        OfflineNoteAuditBundleV2 {
            token_id: Hash::new(b"token"),
            sender_key_certificate: issue.key_certificate.clone(),
            input_nullifiers: vec![Hash::new(b"audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaimV2::from_issue(&issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaimV2 {
                note_commitment: Hash::new(b"output-note"),
                key_certificate: issue.key_certificate,
                asset: issue.asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof(),
        }
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
    fn offline_note_v2_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(IssueOfflineNoteV2::new(issue()));
        assert_slice_roundtrip(RedeemOfflineNoteV2::new(redemption()));
        assert_slice_roundtrip(AuditOfflineNoteV2::new(audit()));
    }

    #[test]
    fn offline_note_v2_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<IssueOfflineNoteV2>()
            .register_slice::<RedeemOfflineNoteV2>()
            .register_slice::<AuditOfflineNoteV2>();

        assert_registry_decodes(&registry, IssueOfflineNoteV2::new(issue()));
        assert_registry_decodes(&registry, RedeemOfflineNoteV2::new(redemption()));
        assert_registry_decodes(&registry, AuditOfflineNoteV2::new(audit()));
    }
}
