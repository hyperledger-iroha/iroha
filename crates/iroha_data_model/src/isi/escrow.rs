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

impl crate::seal::Instruction for OpenAssetEscrow {}
impl crate::seal::Instruction for AcceptAssetEscrow {}
impl crate::seal::Instruction for MarkEscrowPaymentSent {}
impl crate::seal::Instruction for ReleaseAssetEscrow {}
impl crate::seal::Instruction for CancelAssetEscrow {}
impl crate::seal::Instruction for OpenEscrowDispute {}
impl crate::seal::Instruction for ResolveEscrowDispute {}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::{domain::DomainId, name::Name};

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
                asset_definition,
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
    }
}
