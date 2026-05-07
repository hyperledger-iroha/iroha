//! ZK-related instructions.
//!
//! This module defines instructions for submitting proofs for verification
//! against on-chain verifying keys.

use iroha_crypto::Hash;

use super::*;
use crate::{
    asset::definition::ConfidentialPolicyMode, confidential::ConfidentialEncryptedPayload,
};

isi! {
    /// Verify a zero-knowledge proof against a verifying key.
    ///
    /// This instruction records verification result in WSV. Backends and
    /// cryptographic verification are provided by `iroha_core` under feature
    /// flags; this data model type acts as a transport envelope.
    pub struct VerifyProof {
        /// Proof attachment containing the proof and either a VK reference
        /// or an inline verifying key.
        pub attachment: crate::proof::ProofAttachment,
    }
}

impl crate::seal::Instruction for VerifyProof {}

impl VerifyProof {
    /// Construct a new `VerifyProof` instruction from a proof attachment.
    pub fn new(attachment: crate::proof::ProofAttachment) -> Self {
        Self { attachment }
    }
}

isi! {
    /// Prune proof registry entries according to the on-chain retention policy.
    ///
    /// When `backend` is `None`, all backends are considered; otherwise only
    /// the matching backend is pruned. Retention limits (cap/grace/batch) come
    /// from the `zk` configuration, keeping pruning deterministic.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct PruneProofs {
        /// Optional backend label to restrict pruning scope (e.g., `halo2/ipa`).
        pub backend: Option<String>,
    }
}

impl crate::seal::Instruction for PruneProofs {}

impl PruneProofs {
    /// Construct a new prune request.
    pub fn new(backend: Option<String>) -> Self {
        Self { backend }
    }
}

// --- ZK Assets ---

/// Shielded asset mode.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Decode,
    norito::codec::Encode,
    iroha_schema::IntoSchema,
)]
pub enum ZkAssetMode {
    /// Only shielded ledger (no public account balances).
    ZkNative,
    /// Hybrid: public balances plus shielded ledger; allows shield/unshield when policy permits.
    Hybrid,
}

#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ZkAssetMode {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ZkAssetMode::ZkNative => "ZkNative",
            ZkAssetMode::Hybrid => "Hybrid",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ZkAssetMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "ZkNative" => Ok(ZkAssetMode::ZkNative),
            "Hybrid" => Ok(ZkAssetMode::Hybrid),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}

isi! {
    /// Register a ZK-capable asset definition with policy and verifying keys.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterZkAsset {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Asset mode.
        pub mode: ZkAssetMode,
        /// Allow shielding from public to shielded.
        pub allow_shield: bool,
        /// Allow unshielding from shielded to public.
        pub allow_unshield: bool,
        /// Verifying key for shielded transfers.
        pub vk_transfer: Option<crate::proof::VerifyingKeyId>,
        /// Verifying key for unshield proofs.
        pub vk_unshield: Option<crate::proof::VerifyingKeyId>,
        /// Optional verifying key for shield proofs.
        pub vk_shield: Option<crate::proof::VerifyingKeyId>,
    }
}

impl crate::seal::Instruction for RegisterZkAsset {}
impl RegisterZkAsset {
    /// Construct a new `RegisterZkAsset` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        mode: ZkAssetMode,
        allow_shield: bool,
        allow_unshield: bool,
        vk_transfer: Option<crate::proof::VerifyingKeyId>,
        vk_unshield: Option<crate::proof::VerifyingKeyId>,
        vk_shield: Option<crate::proof::VerifyingKeyId>,
    ) -> Self {
        Self {
            asset,
            mode,
            allow_shield,
            allow_unshield,
            vk_transfer,
            vk_unshield,
            vk_shield,
        }
    }
}

isi! {
    /// Schedule a confidential policy transition for an asset definition.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ScheduleConfidentialPolicyTransition {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Policy mode that becomes active at `effective_height`.
        pub new_mode: ConfidentialPolicyMode,
        /// Block height at which the transition must be applied.
        pub effective_height: u64,
        /// Deterministic identifier for the transition (governance audit/replay).
        pub transition_id: Hash,
        /// Optional conversion window length (in blocks) prior to finalizing the transition.
        pub conversion_window: Option<u64>,
    }
}

impl crate::seal::Instruction for ScheduleConfidentialPolicyTransition {}
impl ScheduleConfidentialPolicyTransition {
    /// Construct a new `ScheduleConfidentialPolicyTransition` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        new_mode: ConfidentialPolicyMode,
        effective_height: u64,
        transition_id: Hash,
        conversion_window: Option<u64>,
    ) -> Self {
        Self {
            asset,
            new_mode,
            effective_height,
            transition_id,
            conversion_window,
        }
    }
}

isi! {
    /// Cancel a pending confidential policy transition for an asset definition.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct CancelConfidentialPolicyTransition {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Identifier of the transition to cancel.
        pub transition_id: Hash,
    }
}

impl crate::seal::Instruction for CancelConfidentialPolicyTransition {}
impl CancelConfidentialPolicyTransition {
    /// Construct a new `CancelConfidentialPolicyTransition` instruction.
    pub fn new(asset: AssetDefinitionId, transition_id: Hash) -> Self {
        Self {
            asset,
            transition_id,
        }
    }
}

isi! {
    /// Shield public funds into the asset's shielded ledger by appending a note commitment.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct Shield {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Account to debit.
        pub from: AccountId,
        /// Public amount to debit.
        pub amount: u128,
        /// Output note commitment (opaque 32 bytes under the asset's note scheme).
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Encrypted recipient payload (versioned envelope).
        pub enc_payload: ConfidentialEncryptedPayload,
    }
}

impl crate::seal::Instruction for Shield {}
impl Shield {
    /// Construct a new Shield instruction.
    pub fn new(
        asset: AssetDefinitionId,
        from: AccountId,
        amount: u128,
        note_commitment: [u8; 32],
        enc_payload: impl Into<ConfidentialEncryptedPayload>,
    ) -> Self {
        Self {
            asset,
            from,
            amount,
            note_commitment,
            enc_payload: enc_payload.into(),
        }
    }
}

isi! {
    /// Private-to-private transfer within a shielded ledger.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ZkTransfer {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Spent nullifiers.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub inputs: Vec<[u8; 32]>,
        /// Output note commitments.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub outputs: Vec<[u8; 32]>,
        /// Proof attachment for the transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for ZkTransfer {}
impl ZkTransfer {
    /// Construct a new `ZkTransfer` instruction.
    pub fn new(
        asset: AssetDefinitionId,
        inputs: Vec<[u8; 32]>,
        outputs: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
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

isi! {
    /// Unshield private funds into a public account balance.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct Unshield {
        /// Asset definition id.
        pub asset: AssetDefinitionId,
        /// Recipient account to credit.
        pub to: AccountId,
        /// Public amount to credit.
        pub public_amount: u128,
        /// Spent nullifiers.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub inputs: Vec<[u8; 32]>,
        /// Optional private change note commitments.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub outputs: Vec<[u8; 32]>,
        /// Proof attachment for the unshield.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl crate::seal::Instruction for Unshield {}
impl Unshield {
    /// Construct a new Unshield instruction.
    pub fn new(
        asset: AssetDefinitionId,
        to: AccountId,
        public_amount: u128,
        inputs: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self::new_with_outputs(
            asset,
            to,
            public_amount,
            inputs,
            Vec::new(),
            proof,
            root_hint,
        )
    }

    /// Construct a new Unshield instruction with explicit private change outputs.
    pub fn new_with_outputs(
        asset: AssetDefinitionId,
        to: AccountId,
        public_amount: u128,
        inputs: Vec<[u8; 32]>,
        outputs: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            asset,
            to,
            public_amount,
            inputs,
            outputs,
            proof,
            root_hint,
        }
    }
}

// --- ZK Voting ---

isi! {
    /// Create an anonymous election.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct CreateElection {
        /// Unique election id.
        pub election_id: String,
        /// Number of options (K).
        pub options: u32,
        /// Merkle root of eligible voters.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub eligible_root: [u8; 32],
        /// Start timestamp (ms since epoch).
        pub start_ts: u64,
        /// End timestamp (ms since epoch).
        pub end_ts: u64,
        /// Verifying key for ballot proofs.
        pub vk_ballot: crate::proof::VerifyingKeyId,
        /// Verifying key for tally proofs.
        pub vk_tally: crate::proof::VerifyingKeyId,
        /// Domain separation tag for ballot nullifiers.
        pub domain_tag: String,
    }
}

impl crate::seal::Instruction for CreateElection {}

isi! {
    /// Submit a private ballot for an election.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitBallot {
        /// Election id.
        pub election_id: String,
        /// Encrypted ballot payload (opaque bytes).
        pub ciphertext: Vec<u8>,
        /// ZK proof of eligibility and well-formed vote.
        pub ballot_proof: crate::proof::ProofAttachment,
        /// Unique ballot nullifier to prevent double-voting.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub nullifier: [u8; 32],
    }
}

impl crate::seal::Instruction for SubmitBallot {}

isi! {
    /// Finalize an election by verifying the tally proof and recording the result.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct FinalizeElection {
        /// Election id.
        pub election_id: String,
        /// Public tally per option.
        pub tally: Vec<u64>,
        /// ZK proof that `tally` is consistent with submitted ballots.
        pub tally_proof: crate::proof::ProofAttachment,
    }
}

impl crate::seal::Instruction for FinalizeElection {}

fn zk_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_zk_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = zk_decode_flags();
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

impl_zk_decode_from_slice!(VerifyProof {
    attachment: crate::proof::ProofAttachment,
});

impl_zk_decode_from_slice!(PruneProofs {
    backend: Option<String>,
});

impl_zk_decode_from_slice!(RegisterZkAsset {
    asset: AssetDefinitionId,
    mode: ZkAssetMode,
    allow_shield: bool,
    allow_unshield: bool,
    vk_transfer: Option<crate::proof::VerifyingKeyId>,
    vk_unshield: Option<crate::proof::VerifyingKeyId>,
    vk_shield: Option<crate::proof::VerifyingKeyId>,
});

impl_zk_decode_from_slice!(ScheduleConfidentialPolicyTransition {
    asset: AssetDefinitionId,
    new_mode: ConfidentialPolicyMode,
    effective_height: u64,
    transition_id: Hash,
    conversion_window: Option<u64>,
});

impl_zk_decode_from_slice!(CancelConfidentialPolicyTransition {
    asset: AssetDefinitionId,
    transition_id: Hash,
});

impl_zk_decode_from_slice!(Shield {
    asset: AssetDefinitionId,
    from: AccountId,
    amount: u128,
    note_commitment: [u8; 32],
    enc_payload: ConfidentialEncryptedPayload,
});

impl_zk_decode_from_slice!(ZkTransfer {
    asset: AssetDefinitionId,
    inputs: Vec<[u8; 32]>,
    outputs: Vec<[u8; 32]>,
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
});

impl_zk_decode_from_slice!(Unshield {
    asset: AssetDefinitionId,
    to: AccountId,
    public_amount: u128,
    inputs: Vec<[u8; 32]>,
    outputs: Vec<[u8; 32]>,
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
});

impl_zk_decode_from_slice!(CreateElection {
    election_id: String,
    options: u32,
    eligible_root: [u8; 32],
    start_ts: u64,
    end_ts: u64,
    vk_ballot: crate::proof::VerifyingKeyId,
    vk_tally: crate::proof::VerifyingKeyId,
    domain_tag: String,
});

impl_zk_decode_from_slice!(SubmitBallot {
    election_id: String,
    ciphertext: Vec<u8>,
    ballot_proof: crate::proof::ProofAttachment,
    nullifier: [u8; 32],
});

impl_zk_decode_from_slice!(FinalizeElection {
    election_id: String,
    tally: Vec<u64>,
    tally_proof: crate::proof::ProofAttachment,
});

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        domain::DomainId,
        name::Name,
        proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId},
    };

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            Name::from_str("xor").expect("asset name"),
        )
    }

    fn backend() -> iroha_schema::Ident {
        "halo2/ipa/poly-open".into()
    }

    fn verifying_key(name: &str) -> VerifyingKeyId {
        VerifyingKeyId::new(backend(), name)
    }

    fn proof_attachment() -> ProofAttachment {
        let backend = backend();
        ProofAttachment::new_inline(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![1, 2, 3, 4]),
            VerifyingKeyBox::new(backend, vec![5, 6, 7]),
        )
    }

    fn encrypted_payload() -> ConfidentialEncryptedPayload {
        ConfidentialEncryptedPayload::new([0xA1; 32], [0xB2; 24], vec![0xC3, 0xC4])
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
        wire_id: &str,
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
    #[allow(clippy::too_many_lines)]
    fn zk_decode_from_slice_roundtrips() {
        let asset = asset_definition_id();
        let proof = proof_attachment();

        assert_slice_roundtrip(VerifyProof::new(proof.clone()));
        assert_slice_roundtrip(PruneProofs::new(Some("halo2/ipa".to_owned())));
        assert_slice_roundtrip(RegisterZkAsset::new(
            asset.clone(),
            ZkAssetMode::Hybrid,
            true,
            true,
            Some(verifying_key("transfer")),
            Some(verifying_key("unshield")),
            Some(verifying_key("shield")),
        ));
        assert_slice_roundtrip(ScheduleConfidentialPolicyTransition::new(
            asset.clone(),
            ConfidentialPolicyMode::Convertible,
            42,
            Hash::new("policy-transition"),
            Some(7),
        ));
        assert_slice_roundtrip(CancelConfidentialPolicyTransition::new(
            asset.clone(),
            Hash::new("policy-transition"),
        ));
        assert_slice_roundtrip(Shield::new(
            asset.clone(),
            account(1),
            1_000,
            [0x11; 32],
            encrypted_payload(),
        ));
        assert_slice_roundtrip(ZkTransfer::new(
            asset.clone(),
            vec![[0x12; 32]],
            vec![[0x13; 32]],
            proof.clone(),
            Some([0x14; 32]),
        ));
        assert_slice_roundtrip(Unshield::new_with_outputs(
            asset,
            account(2),
            500,
            vec![[0x15; 32]],
            vec![[0x16; 32]],
            proof.clone(),
            Some([0x17; 32]),
        ));
        assert_slice_roundtrip(CreateElection {
            election_id: "election-1".to_owned(),
            options: 3,
            eligible_root: [0x21; 32],
            start_ts: 1_700_000_000,
            end_ts: 1_700_086_400,
            vk_ballot: verifying_key("ballot"),
            vk_tally: verifying_key("tally"),
            domain_tag: "iroha-election-v1".to_owned(),
        });
        assert_slice_roundtrip(SubmitBallot {
            election_id: "election-1".to_owned(),
            ciphertext: vec![0x31, 0x32],
            ballot_proof: proof.clone(),
            nullifier: [0x33; 32],
        });
        assert_slice_roundtrip(FinalizeElection {
            election_id: "election-1".to_owned(),
            tally: vec![1, 2, 3],
            tally_proof: proof,
        });
    }

    #[test]
    fn zk_default_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::registry::default();
        let asset = asset_definition_id();

        assert_registry_decodes(
            &registry,
            std::any::type_name::<VerifyProof>(),
            VerifyProof::new(proof_attachment()),
        );
        assert_registry_decodes(
            &registry,
            std::any::type_name::<PruneProofs>(),
            PruneProofs::new(None),
        );
        assert_registry_decodes(
            &registry,
            std::any::type_name::<RegisterZkAsset>(),
            RegisterZkAsset::new(
                asset.clone(),
                ZkAssetMode::ZkNative,
                false,
                false,
                Some(verifying_key("transfer")),
                None,
                None,
            ),
        );
        assert_registry_decodes(
            &registry,
            "zk::ScheduleConfidentialPolicyTransition",
            ScheduleConfidentialPolicyTransition::new(
                asset.clone(),
                ConfidentialPolicyMode::ShieldedOnly,
                99,
                Hash::new("policy-transition-stable"),
                None,
            ),
        );
        assert_registry_decodes(
            &registry,
            "zk::CancelConfidentialPolicyTransition",
            CancelConfidentialPolicyTransition::new(asset, Hash::new("policy-transition-stable")),
        );
    }
}
