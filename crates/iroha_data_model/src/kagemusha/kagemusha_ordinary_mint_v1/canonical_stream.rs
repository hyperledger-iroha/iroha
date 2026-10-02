//! Encoder-owned ordinary struct field grammar for reusable bounded circuit streams.
//! This describes canonical bytes only; it never admits a proof, FI, clock or money operation.
use super::*;
use crate::kagemusha::kagemusha_v1::ordinary_struct_frame_prefix_v1;
use crate::kagemusha::{
    KagemushaCanonicalFramePrefixV1, KagemushaMintCreditV1, KagemushaOrdinaryIncomingReservationV1,
    KagemushaOrdinaryIncomingSelectionV1, KagemushaPairedProofV1, KagemushaRetailEnrollmentOwnerV1,
    KagemushaRetailEnrollmentRuntimeV1,
};

mod sealed {
    pub trait Sealed {}
}

/// Restricted ordinary struct inventory. Every entry uses the sole canonical encoder and
/// verifies its exact field concatenation before exposing a header or payload descriptor.
/// Variable field capacities are chosen by the authenticated protocol, never by this grammar.
pub trait KagemushaOrdinaryCanonicalFieldStreamV1:
    norito::NoritoSerialize + sealed::Sealed
{
    /// Return each field's bare canonical payload in declaration order. Raw byte-array fields
    /// use their raw bytes, exactly as the maintained derive encoder; nested fields keep their
    /// complete bare payload. This supplies no semantic or financial authority.
    /// # Errors
    /// Returns an error if any sole field encoder fails.
    fn canonical_field_payloads(&self) -> Result<Vec<Vec<u8>>, String>;
}

/// Data-only exact structural descriptor. Its field bytes must be assigned in full bounded
/// buffers and joined to genuine semantic cells by the enclosing circuit; they are not constants.
#[derive(Clone, Debug)]
pub struct KagemushaOrdinaryCanonicalFieldStreamGrammarV1 {
    framing: KagemushaCanonicalFramePrefixV1,
    fields: Vec<Vec<u8>>,
}
impl KagemushaOrdinaryCanonicalFieldStreamGrammarV1 {
    /// Obtain one authoritative descriptor and check it against the complete sole encoder.
    /// No parser fixture, callback, Native capability or proof acceptance is produced.
    /// # Errors
    /// Refuses codec flags/schema/alignment drift or any competing field ordering/encoding.
    pub fn from_original<T: KagemushaOrdinaryCanonicalFieldStreamV1>(
        value: &T,
    ) -> Result<Self, String> {
        let fields = value.canonical_field_payloads()?;
        let payload = bare(value)?;
        let mut reconstructed = Vec::new();
        for field in &fields {
            norito::core::write_len_to_vec_with_flags(
                &mut reconstructed,
                field.len() as u64,
                norito::core::header_flags::COMPACT_LEN,
            );
            reconstructed.extend_from_slice(field);
        }
        if reconstructed != payload {
            return Err("ordinary canonical field grammar differs from sole encoder".into());
        }
        let framing =
            ordinary_struct_frame_prefix_v1(value, &payload).map_err(|e| e.to_string())?;
        Ok(Self { framing, fields })
    }
    /// Exact schema/header/alignment. Payload length and CRC remain derived circuit holes.
    #[must_use]
    pub fn framing(&self) -> &KagemushaCanonicalFramePrefixV1 {
        &self.framing
    }
    /// Original field payloads, retained as data rather than caller-selected circuit constants.
    #[must_use]
    pub fn fields(&self) -> &[Vec<u8>] {
        &self.fields
    }
}
fn bare<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    norito::codec::encode_adaptive_into(value, &mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}
macro_rules! grammar {
    ($ty:ty,$v:ident,$($field:expr),+ $(,)?)=>{
        impl sealed::Sealed for $ty {}
        impl KagemushaOrdinaryCanonicalFieldStreamV1 for $ty {
            fn canonical_field_payloads(&self)->Result<Vec<Vec<u8>>,String>{let $v=self;Ok(vec![$($field),+])}
        }
    }
}
grammar!(
    KagemushaRetailEnrollmentRuntimeV1,
    v,
    bare(&v.fi_id)?,
    bare(&v.ledger_dataspace_id)?,
    bare(&v.authentication_namespace)?,
    bare(&v.network_id)?,
    bare(&v.asset)?,
    bare(&v.asset_incarnation)?,
    bare(&v.scale)?
);
grammar!(
    KagemushaRetailEnrollmentOwnerV1,
    v,
    bare(&v.account_id)?,
    bare(&v.runtime)?,
    v.lane_id.to_vec()
);
grammar!(
    KagemushaOrdinaryFinancialLineageV1,
    v,
    bare(&v.version)?,
    bare(&v.owner)?,
    v.financial_epoch_id.to_vec(),
    v.financial_authority_commitment.to_vec()
);
grammar!(
    KagemushaOrdinaryFinancialHeadV1,
    v,
    v.state_commitment.to_vec(),
    bare(&v.logical_sequence)?,
    v.state_original_sha256.to_vec()
);
grammar!(
    KagemushaOrdinaryCashClockContextV1,
    v,
    bare(&v.version)?,
    v.request_nonce.to_vec(),
    v.signed_observations_original_digest.to_vec(),
    bare(&v.lower_at_ms)?,
    bare(&v.upper_at_ms)?
);
grammar!(
    KagemushaOrdinaryMintAuthorizationContextV1,
    v,
    bare(&v.version)?,
    v.operation_id.to_vec(),
    bare(&v.lineage)?,
    bare(&v.predecessor)?,
    v.release_id.to_vec(),
    v.suite_id.to_vec(),
    v.vk_digest.to_vec(),
    v.artifact_manifest_digest.to_vec(),
    v.recipient_app_credential_digest.to_vec(),
    v.app_credential_profile_id.to_vec(),
    bare(&v.policy_epoch)?,
    bare(&v.amount)?,
    v.recipient_credential_commitment.to_vec(),
    v.credit_commitment.to_vec(),
    v.recipient_one_time_key.to_vec(),
    bare(&v.clock_context)?,
    v.financial_control_original_sha256.to_vec()
);
grammar!(
    KagemushaOrdinaryMintAuthorizationStatementV1,
    v,
    bare(&v.version)?,
    bare(&v.context)?,
    v.issuance_commitment.to_vec(),
    v.credit_id.to_vec(),
    v.ciphertext_digest.to_vec()
);
grammar!(
    KagemushaOrdinaryMintApprovalChallengeV1,
    v,
    bare(&v.version)?,
    v.operation_id.to_vec(),
    v.nonce.to_vec(),
    v.credential_digest.to_vec(),
    v.statement_digest.to_vec(),
    v.clock_context_digest.to_vec(),
    v.financial_control_original_sha256.to_vec(),
    bare(&v.issued_at_ms)?,
    bare(&v.expires_at_ms)?
);
grammar!(
    KagemushaOrdinaryMintApprovalV1,
    v,
    bare(&v.challenge)?,
    bare(&v.evidence)?
);
grammar!(
    KagemushaOrdinaryMintPairedProofV1,
    v,
    bare(&v.version)?,
    v.eq_protocol_digest.to_vec(),
    v.ep_protocol_digest.to_vec(),
    v.statement_digest.to_vec(),
    v.approval_original_digest.to_vec(),
    bare(&v.eq_proof)?,
    bare(&v.ep_proof)?,
    bare(&v.eq_history)?,
    bare(&v.ep_history)?
);
grammar!(
    KagemushaOrdinaryMintAuthorizationV1,
    v,
    bare(&v.version)?,
    bare(&v.statement)?,
    bare(&v.approval)?,
    bare(&v.proof)?
);
grammar!(
    KagemushaOrdinaryTopUpRequestV1,
    v,
    bare(&v.version)?,
    bare(&v.authorization)?,
    bare(&v.encrypted_credit)?
);
grammar!(
    KagemushaOrdinaryIncomingSelectionV1,
    v,
    bare(&v.version)?,
    bare(&v.lineage)?,
    v.operation_id.to_vec(),
    bare(&v.predecessor)?,
    bare(&v.source)?,
    v.credit_id.to_vec(),
    bare(&v.amount)?,
    bare(&v.scale)?,
    v.recipient_app_credential_digest.to_vec(),
    v.financial_control_original_sha256.to_vec(),
    v.clock_context_digest.to_vec()
);
grammar!(
    KagemushaOrdinaryIncomingReservationV1,
    v,
    bare(&v.selection)?,
    v.finalized_source_original_sha256.to_vec(),
    v.source_proof_original_sha256.to_vec(),
    v.source_semantic_digest.to_vec()
);

grammar!(
    crate::nexus::AxtAssetIncarnationV1,
    v,
    v.as_bytes().to_vec()
);
grammar!(
    KagemushaLifecycleBindingV1,
    v,
    bare(&v.version)?,
    bare(&v.network_id)?,
    bare(&v.protocol_version)?,
    v.suite_id.to_vec(),
    v.vk_digest.to_vec(),
    v.release_id.to_vec(),
    bare(&v.asset)?,
    bare(&v.asset_incarnation)?,
    bare(&v.scale)?,
    v.liability_pool_id.to_vec(),
    v.hardware_profile_id.to_vec(),
    bare(&v.policy_epoch)?,
    bare(&v.operation_kind)?,
    v.request_id.to_vec(),
    v.receiver_lane_commitment.to_vec(),
    v.credit_id.to_vec(),
    v.ciphertext_digest.to_vec()
);
grammar!(
    KagemushaMintCreditStatementV1,
    v,
    bare(&v.version)?,
    bare(&v.lifecycle)?,
    v.recipient_credential_commitment.to_vec(),
    v.authorization_context_digest.to_vec(),
    v.mint_authorization_digest.to_vec(),
    bare(&v.amount)?,
    v.issuance_commitment.to_vec(),
    bare(&v.recipient)?,
    v.credit_commitment.to_vec(),
    bare(&v.minted_at_ms)?
);
grammar!(
    KagemushaPairedProofV1,
    v,
    bare(&v.version)?,
    v.eq_protocol_digest.to_vec(),
    v.ep_protocol_digest.to_vec(),
    v.semantic_digest.to_vec(),
    v.guard_eq_credential_audit.to_vec(),
    v.guard_ep_credential_audit.to_vec(),
    v.eq_deferred_audit.to_vec(),
    v.ep_deferred_audit.to_vec(),
    bare(&v.eq_proof)?,
    bare(&v.ep_proof)?,
    bare(&v.eq_history)?,
    bare(&v.ep_history)?
);
grammar!(
    KagemushaMintCreditV1,
    v,
    bare(&v.version)?,
    bare(&v.statement)?,
    bare(&v.proof)?,
    v.finality_certificate_binding.to_vec(),
    v.finality_authority_head.to_vec(),
    v.finality_genesis_authorization_id.to_vec(),
    v.finality_proof_binding_digest.to_vec(),
    bare(&v.encrypted_credit)?,
    v.artifact_manifest_digest.to_vec()
);

/// Sole enum framing for actual Android/Apple evidence; this descriptor contains no signature.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaOrdinaryApprovalEvidenceStreamGrammarV1 {
    android_tag: [u8; 4],
    apple_tag: [u8; 4],
}
impl KagemushaOrdinaryApprovalEvidenceStreamGrammarV1 {
    /// Derive both discriminants from neutral empty codec specimens, never signed originals.
    /// # Errors
    /// Refuses any enum framing change or competing byte-vector layout.
    pub fn from_sole_encoder() -> Result<Self, String> {
        fn tag(v: KagemushaAppOperationApprovalEvidenceV1) -> Result<[u8; 4], String> {
            let raw = bare(&v)?;
            let mut suffix = Vec::new();
            norito::core::write_len_to_vec_with_flags(
                &mut suffix,
                8,
                norito::core::header_flags::COMPACT_LEN,
            );
            suffix.extend_from_slice(&0_u64.to_le_bytes());
            if raw.len() != 4 + suffix.len() || raw[4..] != suffix {
                return Err("ordinary evidence enum codec differs".into());
            }
            raw[..4]
                .try_into()
                .map_err(|_| "ordinary evidence tag width".into())
        }
        Ok(Self {
            android_tag: tag(KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: Vec::new(),
            })?,
            apple_tag: tag(KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                raw_assertion: Vec::new(),
            })?,
        })
    }
    /// Exact Android enum discriminant, without platform evidence or authority.
    #[must_use]
    pub fn android_tag(&self) -> [u8; 4] {
        self.android_tag
    }
    /// Exact Apple enum discriminant, without platform evidence or authority.
    #[must_use]
    pub fn apple_tag(&self) -> [u8; 4] {
        self.apple_tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1;
    fn check<T: KagemushaOrdinaryCanonicalFieldStreamV1>(v: &T) {
        let g = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(v).unwrap();
        let raw = norito::encode_canonical(v).unwrap();
        let mut payload = Vec::new();
        for f in g.fields() {
            norito::core::write_len_to_vec_with_flags(
                &mut payload,
                f.len() as u64,
                norito::core::header_flags::COMPACT_LEN,
            );
            payload.extend(f);
        }
        assert_eq!(&raw[g.framing().payload_offset()..], payload);
        assert!(g.framing().bytes()[23..39].iter().all(Option::is_none));
    }
    #[test]
    fn ordinary_canonical_field_grammar_matches_full_sole_mint_hierarchy() {
        let f = kagemusha_ordinary_mint_codec_fixture_v1();
        let a = &f.request.authorization;
        let c = &a.statement.context;
        check(&c.lineage.owner.runtime);
        check(&c.lineage.owner);
        check(&c.lineage);
        check(&c.predecessor);
        check(&c.clock_context);
        check(c);
        check(&a.statement);
        check(&a.approval.challenge);
        check(&a.approval);
        check(&a.proof);
        check(a);
        check(&f.request);
        let changed = KagemushaOrdinaryMintAuthorizationContextV1 {
            amount: c.amount + 1,
            ..c.clone()
        };
        check(&changed);
        let g = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(c).unwrap();
        let changed_g =
            KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(&changed).unwrap();
        assert_eq!(g.framing(), changed_g.framing());
        assert_ne!(g.fields(), changed_g.fields());
    }
}
