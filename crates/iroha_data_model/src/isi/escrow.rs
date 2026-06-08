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
    /// Open a generic ledger-managed asset lock.
    pub struct OpenAssetLock {
        /// Caller-selected lock identifier.
        pub escrow_id: crate::escrow::EscrowId,
        /// Asset definition to lock.
        pub asset_definition: crate::asset::AssetDefinitionId,
        /// Account that receives drawdowns from the lock.
        pub destination: crate::account::AccountId,
        /// Amount to lock.
        pub amount: iroha_primitives::numeric::Numeric,
        /// Optional account required to draw down this lock.
        #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
        pub release_authority: Option<crate::account::AccountId>,
        /// Optional Unix timestamp (milliseconds) after which the lock may expire.
        #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
        pub expires_at_ms: Option<u64>,
        /// Evidence hashes attached when opening the lock.
        #[cfg_attr(feature = "json", norito(default))]
        pub evidence_hashes: Vec<iroha_crypto::Hash>,
    }
}

impl OpenAssetLock {
    /// Construct a generic asset lock without initial evidence.
    #[must_use]
    pub fn new(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        destination: crate::account::AccountId,
        amount: iroha_primitives::numeric::Numeric,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            destination,
            amount,
            release_authority: None,
            expires_at_ms: None,
            evidence_hashes: Vec::new(),
        }
    }

    /// Construct a generic asset lock with optional authority, expiry, and evidence.
    #[must_use]
    pub fn with_options(
        escrow_id: crate::escrow::EscrowId,
        asset_definition: crate::asset::AssetDefinitionId,
        destination: crate::account::AccountId,
        amount: iroha_primitives::numeric::Numeric,
        release_authority: Option<crate::account::AccountId>,
        expires_at_ms: Option<u64>,
        evidence_hashes: Vec<iroha_crypto::Hash>,
    ) -> Self {
        Self {
            escrow_id,
            asset_definition,
            destination,
            amount,
            release_authority,
            expires_at_ms,
            evidence_hashes,
        }
    }
}

isi! {
    /// Draw down funds from an active generic asset lock.
    pub struct DrawdownAssetLock {
        /// Lock to draw down.
        pub escrow_id: crate::escrow::EscrowId,
        /// Amount to release to the lock destination.
        pub amount: iroha_primitives::numeric::Numeric,
    }
}

impl DrawdownAssetLock {
    /// Construct a generic asset lock drawdown instruction.
    #[must_use]
    pub const fn new(
        escrow_id: crate::escrow::EscrowId,
        amount: iroha_primitives::numeric::Numeric,
    ) -> Self {
        Self { escrow_id, amount }
    }
}

isi! {
    /// Cancel an active generic asset lock and refund remaining custody.
    pub struct CancelAssetLock {
        /// Lock to cancel.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl CancelAssetLock {
    /// Construct a generic asset lock cancellation instruction.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
    }
}

isi! {
    /// Expire a generic asset lock whose deadline has passed.
    pub struct ExpireAssetLock {
        /// Lock to expire.
        pub escrow_id: crate::escrow::EscrowId,
    }
}

impl ExpireAssetLock {
    /// Construct a generic asset lock expiry instruction.
    #[must_use]
    pub const fn new(escrow_id: crate::escrow::EscrowId) -> Self {
        Self { escrow_id }
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
impl crate::seal::Instruction for OpenAssetLock {}
impl crate::seal::Instruction for DrawdownAssetLock {}
impl crate::seal::Instruction for CancelAssetLock {}
impl crate::seal::Instruction for ExpireAssetLock {}
impl crate::seal::Instruction for OpenAnonymousAssetEscrow {}
impl crate::seal::Instruction for AcceptAnonymousAssetEscrow {}
impl crate::seal::Instruction for MarkAnonymousEscrowPaymentSent {}
impl crate::seal::Instruction for ReleaseAnonymousAssetEscrow {}
impl crate::seal::Instruction for CancelAnonymousAssetEscrow {}
impl crate::seal::Instruction for OpenAnonymousEscrowDispute {}
impl crate::seal::Instruction for ResolveAnonymousEscrowDispute {}

fn escrow_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_escrow_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = escrow_decode_flags();
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

impl_escrow_decode_from_slice!(OpenAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
    asset_definition: crate::asset::AssetDefinitionId,
    amount: iroha_primitives::numeric::Numeric,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(AcceptAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(MarkEscrowPaymentSent {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(ReleaseAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(CancelAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(OpenEscrowDispute {
    escrow_id: crate::escrow::EscrowId,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(ResolveEscrowDispute {
    escrow_id: crate::escrow::EscrowId,
    buyer_amount: iroha_primitives::numeric::Numeric,
    seller_amount: iroha_primitives::numeric::Numeric,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(OpenAssetLock {
    escrow_id: crate::escrow::EscrowId,
    asset_definition: crate::asset::AssetDefinitionId,
    destination: crate::account::AccountId,
    amount: iroha_primitives::numeric::Numeric,
    release_authority: Option<crate::account::AccountId>,
    expires_at_ms: Option<u64>,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(DrawdownAssetLock {
    escrow_id: crate::escrow::EscrowId,
    amount: iroha_primitives::numeric::Numeric,
});

impl_escrow_decode_from_slice!(CancelAssetLock {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(ExpireAssetLock {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(OpenAnonymousAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
    asset_definition: crate::asset::AssetDefinitionId,
    funding_nullifiers: Vec<[u8; 32]>,
    escrow_commitment: [u8; 32],
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(AcceptAnonymousAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(MarkAnonymousEscrowPaymentSent {
    escrow_id: crate::escrow::EscrowId,
});

impl_escrow_decode_from_slice!(ReleaseAnonymousAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
    escrow_nullifiers: Vec<[u8; 32]>,
    buyer_output_commitments: Vec<[u8; 32]>,
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
});

impl_escrow_decode_from_slice!(CancelAnonymousAssetEscrow {
    escrow_id: crate::escrow::EscrowId,
    escrow_nullifiers: Vec<[u8; 32]>,
    seller_output_commitments: Vec<[u8; 32]>,
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
});

impl_escrow_decode_from_slice!(OpenAnonymousEscrowDispute {
    escrow_id: crate::escrow::EscrowId,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

impl_escrow_decode_from_slice!(ResolveAnonymousEscrowDispute {
    escrow_id: crate::escrow::EscrowId,
    escrow_nullifiers: Vec<[u8; 32]>,
    buyer_output_commitments: Vec<[u8; 32]>,
    seller_output_commitments: Vec<[u8; 32]>,
    proof: crate::proof::ProofAttachment,
    root_hint: Option<[u8; 32]>,
    evidence_hashes: Vec<iroha_crypto::Hash>,
});

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        domain::DomainId,
        name::Name,
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
    };

    fn proof_attachment() -> ProofAttachment {
        let backend: iroha_schema::Ident = "halo2/ipa/poly-open".into();
        ProofAttachment::new_ref(
            backend.clone(),
            ProofBox::new(backend.clone(), vec![1, 2, 3]),
            VerifyingKeyId::new(backend, "escrow_vk"),
        )
    }

    fn escrow_id() -> crate::escrow::EscrowId {
        crate::escrow::EscrowId::new(Hash::new("escrow-slice"))
    }

    fn asset_definition_id() -> crate::asset::AssetDefinitionId {
        crate::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse::<Name>().unwrap(),
        )
    }

    fn account(seed: u8) -> crate::account::AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        crate::account::AccountId::new(key_pair.public_key().clone())
    }

    fn evidence_hashes() -> Vec<Hash> {
        vec![Hash::new("escrow-slice-evidence")]
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
    #[allow(clippy::too_many_lines)]
    fn escrow_instruction_constructors_fill_expected_fields() {
        let escrow_id = crate::escrow::EscrowId::new(Hash::new("escrow-ctor"));
        let asset_definition = crate::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse::<Name>().unwrap(),
        );
        let evidence = vec![Hash::new("evidence")];
        let destination = account(0xA1);
        let release_authority = account(0xA2);

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
        assert_eq!(
            OpenAssetLock::new(
                escrow_id,
                asset_definition.clone(),
                destination.clone(),
                Numeric::from(20_u64),
            ),
            OpenAssetLock {
                escrow_id,
                asset_definition: asset_definition.clone(),
                destination: destination.clone(),
                amount: Numeric::from(20_u64),
                release_authority: None,
                expires_at_ms: None,
                evidence_hashes: Vec::new(),
            }
        );
        let lock_with_options = OpenAssetLock::with_options(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::from(20_u64),
            Some(release_authority.clone()),
            Some(12_345),
            evidence.clone(),
        );
        assert_eq!(lock_with_options.release_authority, Some(release_authority));
        assert_eq!(lock_with_options.expires_at_ms, Some(12_345));
        assert_eq!(lock_with_options.evidence_hashes, evidence.clone());
        assert_eq!(
            DrawdownAssetLock::new(escrow_id, Numeric::from(5_u64)).amount,
            Numeric::from(5_u64)
        );
        assert_eq!(CancelAssetLock::new(escrow_id).escrow_id, escrow_id);
        assert_eq!(ExpireAssetLock::new(escrow_id).escrow_id, escrow_id);

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

    #[test]
    #[allow(clippy::too_many_lines)]
    fn escrow_decode_from_slice_roundtrips() {
        let escrow_id = escrow_id();
        let asset_definition = asset_definition_id();
        let proof = proof_attachment();
        let evidence = evidence_hashes();
        let destination = account(0xB1);
        let release_authority = account(0xB2);

        assert_slice_roundtrip(OpenAssetEscrow::with_evidence_hashes(
            escrow_id,
            asset_definition.clone(),
            Numeric::from(10_u64),
            evidence.clone(),
        ));
        assert_slice_roundtrip(AcceptAssetEscrow::new(escrow_id));
        assert_slice_roundtrip(MarkEscrowPaymentSent::new(escrow_id));
        assert_slice_roundtrip(ReleaseAssetEscrow::new(escrow_id));
        assert_slice_roundtrip(CancelAssetEscrow::new(escrow_id));
        assert_slice_roundtrip(OpenEscrowDispute::with_evidence_hashes(
            escrow_id,
            evidence.clone(),
        ));
        assert_slice_roundtrip(ResolveEscrowDispute::with_evidence_hashes(
            escrow_id,
            Numeric::from(7_u64),
            Numeric::from(3_u64),
            evidence.clone(),
        ));
        assert_slice_roundtrip(OpenAssetLock::with_options(
            escrow_id,
            asset_definition.clone(),
            destination.clone(),
            Numeric::from(20_u64),
            Some(release_authority.clone()),
            Some(12_345),
            evidence.clone(),
        ));
        assert_slice_roundtrip(DrawdownAssetLock::new(escrow_id, Numeric::from(5_u64)));
        assert_slice_roundtrip(CancelAssetLock::new(escrow_id));
        assert_slice_roundtrip(ExpireAssetLock::new(escrow_id));
        assert_slice_roundtrip(OpenAnonymousAssetEscrow::with_evidence_hashes(
            escrow_id,
            asset_definition,
            vec![[0x11; 32]],
            [0x22; 32],
            proof.clone(),
            Some([0x33; 32]),
            evidence.clone(),
        ));
        assert_slice_roundtrip(AcceptAnonymousAssetEscrow::new(escrow_id));
        assert_slice_roundtrip(MarkAnonymousEscrowPaymentSent::new(escrow_id));
        assert_slice_roundtrip(ReleaseAnonymousAssetEscrow::new(
            escrow_id,
            vec![[0x44; 32]],
            vec![[0x55; 32]],
            proof.clone(),
            None,
        ));
        assert_slice_roundtrip(CancelAnonymousAssetEscrow::new(
            escrow_id,
            vec![[0x44; 32]],
            vec![[0x66; 32]],
            proof.clone(),
            None,
        ));
        assert_slice_roundtrip(OpenAnonymousEscrowDispute::with_evidence_hashes(
            escrow_id,
            evidence.clone(),
        ));
        assert_slice_roundtrip(ResolveAnonymousEscrowDispute::with_evidence_hashes(
            escrow_id,
            vec![[0x44; 32]],
            vec![[0x55; 32]],
            vec![[0x66; 32]],
            proof,
            Some([0x77; 32]),
            evidence,
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn escrow_default_registry_decodes_type_names() {
        let registry = crate::isi::registry::default();
        let escrow_id = escrow_id();
        let asset_definition = asset_definition_id();
        let proof = proof_attachment();
        let evidence = evidence_hashes();
        let destination = account(0xC1);
        let release_authority = account(0xC2);

        assert_registry_decodes(
            &registry,
            OpenAssetEscrow::with_evidence_hashes(
                escrow_id,
                asset_definition.clone(),
                Numeric::from(10_u64),
                evidence.clone(),
            ),
        );
        assert_registry_decodes(&registry, AcceptAssetEscrow::new(escrow_id));
        assert_registry_decodes(&registry, MarkEscrowPaymentSent::new(escrow_id));
        assert_registry_decodes(&registry, ReleaseAssetEscrow::new(escrow_id));
        assert_registry_decodes(&registry, CancelAssetEscrow::new(escrow_id));
        assert_registry_decodes(
            &registry,
            OpenEscrowDispute::with_evidence_hashes(escrow_id, evidence.clone()),
        );
        assert_registry_decodes(
            &registry,
            ResolveEscrowDispute::with_evidence_hashes(
                escrow_id,
                Numeric::from(7_u64),
                Numeric::from(3_u64),
                evidence.clone(),
            ),
        );
        assert_registry_decodes(
            &registry,
            OpenAssetLock::with_options(
                escrow_id,
                asset_definition.clone(),
                destination,
                Numeric::from(20_u64),
                Some(release_authority),
                Some(12_345),
                evidence.clone(),
            ),
        );
        assert_registry_decodes(
            &registry,
            DrawdownAssetLock::new(escrow_id, Numeric::from(5_u64)),
        );
        assert_registry_decodes(&registry, CancelAssetLock::new(escrow_id));
        assert_registry_decodes(&registry, ExpireAssetLock::new(escrow_id));
        assert_registry_decodes(
            &registry,
            OpenAnonymousAssetEscrow::with_evidence_hashes(
                escrow_id,
                asset_definition,
                vec![[0x11; 32]],
                [0x22; 32],
                proof.clone(),
                Some([0x33; 32]),
                evidence.clone(),
            ),
        );
        assert_registry_decodes(&registry, AcceptAnonymousAssetEscrow::new(escrow_id));
        assert_registry_decodes(&registry, MarkAnonymousEscrowPaymentSent::new(escrow_id));
        assert_registry_decodes(
            &registry,
            ReleaseAnonymousAssetEscrow::new(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x55; 32]],
                proof.clone(),
                None,
            ),
        );
        assert_registry_decodes(
            &registry,
            CancelAnonymousAssetEscrow::new(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x66; 32]],
                proof.clone(),
                None,
            ),
        );
        assert_registry_decodes(
            &registry,
            OpenAnonymousEscrowDispute::with_evidence_hashes(escrow_id, evidence.clone()),
        );
        assert_registry_decodes(
            &registry,
            ResolveAnonymousEscrowDispute::with_evidence_hashes(
                escrow_id,
                vec![[0x44; 32]],
                vec![[0x55; 32]],
                vec![[0x66; 32]],
                proof,
                Some([0x77; 32]),
                evidence,
            ),
        );
    }
}
