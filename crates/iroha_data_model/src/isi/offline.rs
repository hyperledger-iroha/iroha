use super::*;
use crate::{
    asset::AssetDefinitionId,
    offline::{OfflineNoteAuditBundle, OfflineNoteIssue, OfflineNoteRedeem},
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

impl crate::seal::Instruction for IssueOfflineNote {}
impl crate::seal::Instruction for RedeemOfflineNote {}
impl crate::seal::Instruction for AuditOfflineNote {}
impl crate::seal::Instruction for KagemushaTransfer {}

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
            OfflineNoteAuditOutputClaim, OfflineNoteIssuedClaim, OfflineNoteKeyCertificate,
            OfflineNoteRecursiveProof,
        },
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
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

    #[test]
    fn offline_note_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(IssueOfflineNote::new(issue()));
        assert_slice_roundtrip(RedeemOfflineNote::new(redemption()));
        assert_slice_roundtrip(AuditOfflineNote::new(audit()));
        assert_slice_roundtrip(kagemusha_transfer());
    }

    #[test]
    fn offline_note_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<IssueOfflineNote>()
            .register_slice::<RedeemOfflineNote>()
            .register_slice::<AuditOfflineNote>()
            .register_slice::<KagemushaTransfer>();

        assert_registry_decodes(&registry, IssueOfflineNote::new(issue()));
        assert_registry_decodes(&registry, RedeemOfflineNote::new(redemption()));
        assert_registry_decodes(&registry, AuditOfflineNote::new(audit()));
        assert_registry_decodes(&registry, kagemusha_transfer());
    }
}
