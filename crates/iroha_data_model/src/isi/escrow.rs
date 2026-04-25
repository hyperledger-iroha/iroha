use super::*;

isi! {
    /// Open a ledger-managed numeric asset escrow.
    pub struct OpenAssetEscrow {
        /// Caller-selected escrow identifier.
        pub escrow_id: crate::escrow::EscrowId,
        /// Asset definition to lock.
        pub asset_definition: crate::asset::AssetDefinitionId,
        /// Amount to lock.
        pub amount: iroha_primitives::numeric::Numeric,
        /// Evidence hashes attached when opening the escrow.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl OpenAssetEscrow {
    /// Construct an escrow-opening instruction without initial evidence.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        amount: iroha_primitives::numeric::Numeric,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            amount,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct an escrow-opening instruction with evidence hashes attached.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        amount: iroha_primitives::numeric::Numeric,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            amount,
            evidence_hashes,
        }
    }
}

isi! {
    /// Accept an open asset escrow as its buyer.
    pub struct AcceptAssetEscrow {
        /// Escrow to accept.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl AcceptAssetEscrow {
    /// Construct an escrow-acceptance instruction.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Mark an accepted asset escrow as paid off-chain.
    pub struct MarkEscrowPaymentSent {
        /// Escrow whose payment has been sent.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl MarkEscrowPaymentSent {
    /// Construct an instruction that marks off-chain payment as sent.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Release a paid escrow to its accepted buyer.
    pub struct ReleaseAssetEscrow {
        /// Escrow to release.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl ReleaseAssetEscrow {
    /// Construct an instruction that releases escrowed funds to the buyer.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Cancel an open or accepted escrow before payment is marked.
    pub struct CancelAssetEscrow {
        /// Escrow to cancel.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl CancelAssetEscrow {
    /// Construct an instruction that refunds an escrow before payment is marked.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Open a dispute for court moderation.
    pub struct OpenEscrowDispute {
        /// Escrow to dispute.
        pub escrow_id: crate::escrow::EscrowId,
        /// Evidence hashes attached by the disputing party.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl OpenEscrowDispute {
    /// Construct an instruction that opens an escrow dispute without new evidence.
    #[must_use]
    pub fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self {
            escrow_id,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct an instruction that opens an escrow dispute with evidence hashes.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            evidence_hashes,
        }
    }
}

isi! {
    /// Resolve a disputed escrow by splitting funds between buyer and seller.
    pub struct ResolveEscrowDispute {
        /// Escrow to resolve.
        pub escrow_id: crate::escrow::EscrowId,
        /// Amount released to the accepted buyer.
        pub buyer_amount: iroha_primitives::numeric::Numeric,
        /// Amount refunded to the seller.
        pub seller_amount: iroha_primitives::numeric::Numeric,
        /// Evidence or judgement hashes attached by the resolver.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl ResolveEscrowDispute {
    /// Construct a dispute-resolution instruction without additional evidence.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        buyer_amount: iroha_primitives::numeric::Numeric,
        seller_amount: iroha_primitives::numeric::Numeric,
    ) -> Self {
        Self {
            escrow_id,
            buyer_amount,
            seller_amount,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct a dispute-resolution instruction with evidence or judgement hashes.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        buyer_amount: iroha_primitives::numeric::Numeric,
        seller_amount: iroha_primitives::numeric::Numeric,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            buyer_amount,
            seller_amount,
            evidence_hashes,
        }
    }
}

isi! {
    /// Open a ledger-managed anonymous asset escrow using shielded inputs.
    pub struct OpenAnonymousAssetEscrow {
        /// Caller-selected escrow identifier.
        pub escrow_id: crate::escrow::EscrowId,
        /// Shielded asset definition to lock.
        pub asset_definition: crate::asset::AssetDefinitionId,
        /// Nullifiers consumed by the funding proof.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub funding_nullifiers: Vec<[u8; 32]>,
        /// Escrow note commitment created by the funding proof.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub escrow_commitment: [u8; 32],
        /// Proof attachment for the shielded funding transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent shielded Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
        /// Evidence hashes attached when opening the escrow.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl OpenAnonymousAssetEscrow {
    /// Construct an anonymous escrow-opening instruction without initial evidence.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        funding_nullifiers: Vec<[u8; 32]>,
        escrow_commitment: [u8; 32],
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            funding_nullifiers,
            escrow_commitment,
            proof,
            root_hint,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct an anonymous escrow-opening instruction with evidence hashes attached.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        funding_nullifiers: Vec<[u8; 32]>,
        escrow_commitment: [u8; 32],
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            funding_nullifiers,
            escrow_commitment,
            proof,
            root_hint,
            evidence_hashes,
        }
    }
}

isi! {
    /// Accept an open anonymous asset escrow as its buyer.
    pub struct AcceptAnonymousAssetEscrow {
        /// Escrow to accept.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl AcceptAnonymousAssetEscrow {
    /// Construct an anonymous escrow-acceptance instruction.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Mark an accepted anonymous asset escrow as paid off-chain.
    pub struct MarkAnonymousEscrowPaymentSent {
        /// Escrow whose payment has been sent.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl MarkAnonymousEscrowPaymentSent {
    /// Construct an instruction that marks anonymous escrow payment as sent.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Release a paid anonymous escrow to its accepted buyer using shielded outputs.
    pub struct ReleaseAnonymousAssetEscrow {
        /// Escrow to release.
        pub escrow_id: crate::escrow::EscrowId,
        /// Nullifiers spending the escrow note.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub escrow_nullifiers: Vec<[u8; 32]>,
        /// Output commitments labelled for the accepted buyer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub buyer_output_commitments: Vec<[u8; 32]>,
        /// Proof attachment for the shielded release transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent shielded Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl ReleaseAnonymousAssetEscrow {
    /// Construct an instruction that releases an anonymous escrow.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        escrow_nullifiers: Vec<[u8; 32]>,
        buyer_output_commitments: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            escrow_id,
            escrow_nullifiers,
            buyer_output_commitments,
            proof,
            root_hint,
        }
    }
}

isi! {
    /// Cancel an open or accepted anonymous escrow before payment is marked.
    pub struct CancelAnonymousAssetEscrow {
        /// Escrow to cancel.
        pub escrow_id: crate::escrow::EscrowId,
        /// Nullifiers spending the escrow note.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub escrow_nullifiers: Vec<[u8; 32]>,
        /// Output commitments labelled for the seller.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub seller_output_commitments: Vec<[u8; 32]>,
        /// Proof attachment for the shielded cancellation transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent shielded Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
    }
}

impl CancelAnonymousAssetEscrow {
    /// Construct an instruction that cancels an anonymous escrow.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        escrow_nullifiers: Vec<[u8; 32]>,
        seller_output_commitments: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            escrow_id,
            escrow_nullifiers,
            seller_output_commitments,
            proof,
            root_hint,
        }
    }
}

isi! {
    /// Open an anonymous escrow dispute for court moderation.
    pub struct OpenAnonymousEscrowDispute {
        /// Escrow to dispute.
        pub escrow_id: crate::escrow::EscrowId,
        /// Evidence hashes attached by the disputing party.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl OpenAnonymousEscrowDispute {
    /// Construct an instruction that opens an anonymous escrow dispute without new evidence.
    #[must_use]
    pub fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self {
            escrow_id,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct an instruction that opens an anonymous escrow dispute with evidence hashes.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            evidence_hashes,
        }
    }
}

isi! {
    /// Resolve a disputed anonymous escrow by spending the escrow note to labelled outputs.
    pub struct ResolveAnonymousEscrowDispute {
        /// Escrow to resolve.
        pub escrow_id: crate::escrow::EscrowId,
        /// Nullifiers spending the escrow note.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub escrow_nullifiers: Vec<[u8; 32]>,
        /// Output commitments labelled for the accepted buyer.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub buyer_output_commitments: Vec<[u8; 32]>,
        /// Output commitments labelled for the seller.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::vec"))]
        pub seller_output_commitments: Vec<[u8; 32]>,
        /// Proof attachment for the shielded dispute-resolution transfer.
        pub proof: crate::proof::ProofAttachment,
        /// Optional recent shielded Merkle root used during proof construction.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes::option"))]
        pub root_hint: Option<[u8; 32]>,
        /// Evidence or judgement hashes attached by the resolver.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl ResolveAnonymousEscrowDispute {
    /// Construct an anonymous dispute-resolution instruction without additional evidence.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        escrow_nullifiers: Vec<[u8; 32]>,
        buyer_output_commitments: Vec<[u8; 32]>,
        seller_output_commitments: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
    ) -> Self {
        Self {
            escrow_id,
            escrow_nullifiers,
            buyer_output_commitments,
            seller_output_commitments,
            proof,
            root_hint,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct an anonymous dispute-resolution instruction with evidence or judgement hashes.
    #[must_use]
    pub fn with_evidence_hashes(
        escrow_id: crate::escrow::EscrowId,
        escrow_nullifiers: Vec<[u8; 32]>,
        buyer_output_commitments: Vec<[u8; 32]>,
        seller_output_commitments: Vec<[u8; 32]>,
        proof: crate::proof::ProofAttachment,
        root_hint: Option<[u8; 32]>,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            escrow_nullifiers,
            buyer_output_commitments,
            seller_output_commitments,
            proof,
            root_hint,
            evidence_hashes,
        }
    }
}

impl crate::seal::Instruction for OpenAssetEscrow {}
impl crate::seal::Instruction for AcceptAssetEscrow {}
impl crate::seal::Instruction for MarkEscrowPaymentSent {}
impl crate::seal::Instruction for ReleaseAssetEscrow {}
impl crate::seal::Instruction for CancelAssetEscrow {}
impl crate::seal::Instruction for OpenEscrowDispute {}
impl crate::seal::Instruction for ResolveEscrowDispute {}
impl crate::seal::Instruction for OpenAnonymousAssetEscrow {}
impl crate::seal::Instruction for AcceptAnonymousAssetEscrow {}
impl crate::seal::Instruction for MarkAnonymousEscrowPaymentSent {}
impl crate::seal::Instruction for ReleaseAnonymousAssetEscrow {}
impl crate::seal::Instruction for CancelAnonymousAssetEscrow {}
impl crate::seal::Instruction for OpenAnonymousEscrowDispute {}
impl crate::seal::Instruction for ResolveAnonymousEscrowDispute {}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{
        domain::DomainId,
        name::Name,
        proof::{ProofAttachment, ProofBox, VerifyingKeyBox},
    };

    fn proof_attachment() -> ProofAttachment {
        let backend: iroha_schema::Ident = "halo2/ipa/poly-open".into();
        ProofAttachment::new_inline(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![1, 2, 3]),
            VerifyingKeyBox::new(backend, Vec::new()),
        )
    }

    #[test]
    fn escrow_instruction_constructors_fill_expected_fields() {
        let escrow_id = crate::escrow::EscrowId::new(Hash::new("escrow-ctor"));
        let asset_definition = crate::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse::<Name>().unwrap(),
        );
        let evidence = vec![Hash::new("evidence")];

        assert_eq!(
            OpenAssetEscrow::new(escrow_id, asset_definition.clone(), Numeric::from(10_u64)),
            OpenAssetEscrow {
                escrow_id,
                asset_definition: asset_definition.clone(),
                amount: Numeric::from(10_u64),
                evidence_hashes: Vec::new(),
            }
        );
        assert_eq!(
            OpenAssetEscrow::with_evidence_hashes(
                escrow_id,
                asset_definition.clone(),
                Numeric::from(10_u64),
                evidence.clone(),
            )
            .evidence_hashes,
            evidence
        );
        assert_eq!(AcceptAssetEscrow::new(escrow_id).escrow_id, escrow_id);
        assert_eq!(MarkEscrowPaymentSent::new(escrow_id).escrow_id, escrow_id);
        assert_eq!(ReleaseAssetEscrow::new(escrow_id).escrow_id, escrow_id);
        assert_eq!(CancelAssetEscrow::new(escrow_id).escrow_id, escrow_id);
        assert_eq!(
            OpenEscrowDispute::new(escrow_id).evidence_hashes,
            Vec::new()
        );
        assert_eq!(
            OpenEscrowDispute::with_evidence_hashes(escrow_id, evidence.clone()).evidence_hashes,
            evidence
        );
        assert_eq!(
            ResolveEscrowDispute::new(escrow_id, Numeric::from(7_u64), Numeric::from(3_u64))
                .seller_amount,
            Numeric::from(3_u64)
        );
        assert_eq!(
            ResolveEscrowDispute::with_evidence_hashes(
                escrow_id,
                Numeric::from(7_u64),
                Numeric::from(3_u64),
                evidence.clone(),
            )
            .evidence_hashes,
            evidence
        );

        let proof = proof_attachment();
        assert_eq!(
            OpenAnonymousAssetEscrow::new(
                escrow_id,
                asset_definition.clone(),
                vec![[0x11; 32]],
                [0x22; 32],
                proof.clone(),
                Some([0x33; 32]),
            )
            .evidence_hashes,
            Vec::new()
        );
        assert_eq!(
            OpenAnonymousAssetEscrow::with_evidence_hashes(
                escrow_id,
                asset_definition,
                vec![[0x11; 32]],
                [0x22; 32],
                proof.clone(),
                Some([0x33; 32]),
                evidence.clone(),
            )
            .evidence_hashes,
            evidence
        );
        assert_eq!(
            AcceptAnonymousAssetEscrow::new(escrow_id).escrow_id,
            escrow_id
        );
        assert_eq!(
            MarkAnonymousEscrowPaymentSent::new(escrow_id).escrow_id,
            escrow_id
        );
        assert_eq!(
            ReleaseAnonymousAssetEscrow::new(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x55; 32]],
                proof.clone(),
                None,
            )
            .buyer_output_commitments,
            vec![[0x55; 32]]
        );
        assert_eq!(
            CancelAnonymousAssetEscrow::new(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x66; 32]],
                proof.clone(),
                None,
            )
            .seller_output_commitments,
            vec![[0x66; 32]]
        );
        assert_eq!(
            OpenAnonymousEscrowDispute::new(escrow_id).evidence_hashes,
            Vec::new()
        );
        assert_eq!(
            ResolveAnonymousEscrowDispute::new(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x55; 32]],
                vec![[0x66; 32]],
                proof,
                None,
            )
            .seller_output_commitments,
            vec![[0x66; 32]]
        );
    }
}
