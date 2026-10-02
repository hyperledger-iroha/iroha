//! Distinct first-release global incoming credit selectors and financial-head CAS data.
//!
//! Mint preparation selects the full unsigned pre-debit request before Core reserves the
//! predecessor and debits real online funds. Receive selects already authenticated immutable
//! sender evidence. Finalized source admission is acknowledged before the successor State proof;
//! none of these public records, their account signatures or decoders creates an incoming grant.
use super::{
    KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1, KagemushaOperationKindV1,
    KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryFinancialLineageV1,
    KagemushaOrdinaryTopUpRequestV1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

#[path = "kagemusha_ordinary_incoming_v1/preparation.rs"]
mod preparation;
pub use preparation::{
    KAGEMUSHA_ORDINARY_INCOMING_PREPARATION_BYTES_V1, KagemushaOrdinaryIncomingPreparationV1,
};

#[path = "kagemusha_ordinary_incoming_v1/terminal.rs"]
mod terminal;
pub use terminal::{
    KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_BODY_BYTES_V1,
    KAGEMUSHA_ORDINARY_INCOMING_TERMINAL_INTENT_BYTES_V1, KagemushaOrdinaryIncomingTerminalBodyV1,
    KagemushaOrdinaryIncomingTerminalIntentV1,
};

/// Immutable source selectors known before the incoming State proof. Future receiver State
/// proofs and DATA results are deliberately excluded. Core must authenticate the source fully.
#[allow(variant_size_differences)]
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(
    tag = "source",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingSourceSelectionV1")]
// Small fixed-size Copy selectors intentionally retain all four Receive digests inline.
// This bounded canonical model avoids an allocation solely to equalize enum variants.
pub enum KagemushaOrdinaryIncomingSourceSelectionV1 {
    /// Complete unsigned ordinary `TopUpRequest` after the actual app approval/proof and AEAD.
    /// The later Core pre-debit decision/signature and debit/finality are excluded from this SHA.
    Mint {
        /// SHA256 of the complete sole canonical ordinary `TopUpRequest` original.
        topup_request_original_sha256: [u8; 32],
    },
    /// A genuine immutable sender Commit and its independently verified compact Wrapper.
    Receive {
        /// SHA256 of the complete signed Core result + exact DATA record + finality envelope.
        sender_commit_transport_original_sha256: [u8; 32],
        /// SHA256 of the complete pre-receipt ordinary outgoing original authenticated by Commit.
        sender_outgoing_original_sha256: [u8; 32],
        /// Sole purpose-bound original request digest, not raw SHA of a request projection.
        recipient_request_original_digest: [u8; 32],
        /// SHA256 of the complete canonical ciphertext selected by the retained request key.
        encrypted_credit_original_sha256: [u8; 32],
    },
}
impl KagemushaOrdinaryIncomingSourceSelectionV1 {
    /// Exact incoming operation. This carries no source or financial authority.
    #[must_use]
    pub const fn operation(&self) -> KagemushaOperationKindV1 {
        match self {
            Self::Mint { .. } => KagemushaOperationKindV1::MintFold,
            Self::Receive { .. } => KagemushaOperationKindV1::ReceiveFold,
        }
    }
    /// Check data selectors only. Full source proof/finality and actual key custody are mandatory.
    /// # Errors
    /// Refuses an absent full-original selector.
    pub fn validate_shape(&self) -> Result<(), String> {
        match self {
            Self::Mint {
                topup_request_original_sha256,
            } => nonzero(&[*topup_request_original_sha256]),
            Self::Receive {
                sender_commit_transport_original_sha256,
                sender_outgoing_original_sha256,
                recipient_request_original_digest,
                encrypted_credit_original_sha256,
            } => nonzero(&[
                *sender_commit_transport_original_sha256,
                *sender_outgoing_original_sha256,
                *recipient_request_original_digest,
                *encrypted_credit_original_sha256,
            ]),
        }
    }
}
/// Exact incoming preparation selected by the Native owner. For Mint this is available before
/// the real debit. Global DATA must exclusively hold this same predecessor before that debit.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingSelectionV1")]
pub struct KagemushaOrdinaryIncomingSelectionV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Actual independently selected account/FI/runtime/lane and financial epoch/secret commitment.
    pub lineage: KagemushaOrdinaryFinancialLineageV1,
    /// Fresh Native incoming attempt identity retained before any account/platform invocation.
    pub operation_id: [u8; 32],
    /// Exact globally admitted full PUBLIC predecessor State original and financial sequence.
    pub predecessor: KagemushaOrdinaryFinancialHeadV1,
    /// Source kind and full immutable originals; never an OEM mint/payment projection.
    pub source: KagemushaOrdinaryIncomingSourceSelectionV1,
    /// Exact derived source credit identity, consumed once under this receiver financial lineage.
    pub credit_id: [u8; 32],
    /// Positive exact source amount; successor arithmetic must prove this same value.
    pub amount: u128,
    /// Exact authoritative asset scale from the original lineage runtime.
    pub scale: u32,
    /// Complete same ordinary recipient C digest; no hardware credential ID is asserted.
    pub recipient_app_credential_digest: [u8; 32],
    /// SHA256 of the actual acknowledged original FI control selected for this preparation.
    pub financial_control_original_sha256: [u8; 32],
    /// Purpose-bound original preparation clock context digest; full observations travel separately.
    pub clock_context_digest: [u8; 32],
}
impl KagemushaOrdinaryIncomingSelectionV1 {
    /// Validate data-only scope. A valid selector is not a pending DATA cell or Native grant.
    /// # Errors
    /// Refuses missing selectors, zero amount, wrong scale or malformed full predecessor/lineage.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.lineage.validate_shape()?;
        self.predecessor.validate_shape()?;
        self.source.validate_shape()?;
        nonzero(&[
            self.operation_id,
            self.credit_id,
            self.recipient_app_credential_digest,
            self.financial_control_original_sha256,
            self.clock_context_digest,
        ])?;
        if self.version != 1 || self.amount == 0 || self.scale != self.lineage.owner.runtime.scale {
            return Err("ordinary incoming version/amount/scale differs".into());
        }
        Ok(())
    }
    /// Exact complete selector digest; no future debit decision/finality/State proof is hashed here.
    /// # Errors
    /// Refuses malformed scope or bounded canonical encoding.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-incoming-selection\0", self)
    }
    /// Join the complete unsigned Mint request and all pre-debit scope data. This is a data
    /// check after actual ordinary authorization proof admission, not that admission itself.
    /// # Errors
    /// Refuses another source, original request, credit/amount/C, lineage/head, FI or clock.
    pub fn validate_against_topup(
        &self,
        request: &KagemushaOrdinaryTopUpRequestV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        let KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256,
        } = self.source
        else {
            return Err("ordinary incoming source is not a Mint request".into());
        };
        let original = request.canonical_bytes()?;
        let statement = &request.authorization.statement;
        let context = &statement.context;
        if topup_request_original_sha256 != <[u8; 32]>::from(Sha256::digest(&original))
            || self.lineage != context.lineage
            || self.operation_id != context.operation_id
            || self.predecessor != context.predecessor
            || self.credit_id != statement.credit_id
            || self.amount != context.amount
            || self.scale != context.lineage.owner.runtime.scale
            || self.recipient_app_credential_digest != context.recipient_app_credential_digest
            || self.financial_control_original_sha256 != context.financial_control_original_sha256
            || self.clock_context_digest != context.clock_context.binding_digest()?
        {
            return Err("ordinary incoming Mint request originals/scope differ".into());
        }
        Ok(())
    }
}
/// Data projection of an acknowledged genuine source and exclusive predecessor reservation.
/// Mint source admission follows actual finalized debit; Receive follows complete sender proof
/// and independently installed immutable Commit admission. Source bytes travel separately.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingReservationV1")]
pub struct KagemushaOrdinaryIncomingReservationV1 {
    /// Same immutable pre-debit/pre-intake selection held in the exclusive DATA pending cell.
    pub selection: KagemushaOrdinaryIncomingSelectionV1,
    /// SHA256 of complete finalized Mint funding original or complete received Commit envelope.
    pub finalized_source_original_sha256: [u8; 32],
    /// SHA256 of the entire independently verified canonical `MintCredit` original for Mint,
    /// including both genuine `MintAuthority` proofs and complete histories; for Receive, the
    /// exact retained compact sender outgoing carrier. This later selector is excluded from
    /// the pre-debit `IncomingSelection` and cannot change the deterministic credit identity.
    pub source_proof_original_sha256: [u8; 32],
    /// Exact finalized Mint statement or sender output semantic digest from the genuine source.
    pub source_semantic_digest: [u8; 32],
}
impl KagemushaOrdinaryIncomingReservationV1 {
    /// Check data-only source/selection shape; an offered signature never creates Native custody.
    /// # Errors
    /// Refuses absent source evidence or a Receive receipt substitution.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.selection.validate_shape()?;
        nonzero(&[
            self.finalized_source_original_sha256,
            self.source_proof_original_sha256,
            self.source_semantic_digest,
        ])?;
        if let KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
            sender_commit_transport_original_sha256,
            sender_outgoing_original_sha256,
            ..
        } = self.selection.source
            && (sender_commit_transport_original_sha256 != self.finalized_source_original_sha256
                || sender_outgoing_original_sha256 != self.source_proof_original_sha256)
        {
            return Err("ordinary incoming finalized receipt differs from exact source".into());
        }
        Ok(())
    }
    /// Exact full reservation digest; does not authenticate source finality or lend a DATA receipt.
    /// # Errors
    /// Refuses malformed data or bounded canonical encoding.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-incoming-reservation\0", self)
    }
}
/// Incoming whole ordinary State commit. Actual Core must verify both State parities and complete
/// history, source credit/replay insertion, financial possession and exact predecessor/successor;
/// then atomically consume `credit_id` and advance the reserved DATA head before Native effects.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryIncomingCommitV1")]
pub struct KagemushaOrdinaryIncomingCommitV1 {
    /// Same independently authenticated acknowledged source reservation and predecessor.
    pub reservation: KagemushaOrdinaryIncomingReservationV1,
    /// Exact full successor PUBLIC State proof checkpoint and financial sequence.
    pub successor: KagemushaOrdinaryFinancialHeadV1,
    /// SHA256 of the complete self-contained ordinary State/source/Guard proof bundle original.
    pub state_proof_bundle_original_sha256: [u8; 32],
    /// SHA256 of the full selected State transition statement, never a truncated State projection.
    pub transition_statement_original_sha256: [u8; 32],
    /// SHA256 of the complete separately captured purpose1 incoming approval original.
    pub purpose1_approval_original_sha256: [u8; 32],
    /// SHA256 of the exact acknowledged current FI-control original captured for State intake.
    pub financial_control_original_sha256: [u8; 32],
    /// SHA256 of the exact admission clock context canonical original; actual full clock is separate.
    pub admission_clock_context_original_sha256: [u8; 32],
}
impl KagemushaOrdinaryIncomingCommitV1 {
    /// Check data-only complete financial edge, with independent u128 State sequence arithmetic.
    /// # Errors
    /// Refuses missing evidence or a reused/overflowed predecessor financial sequence/head.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.reservation.validate_shape()?;
        self.successor.validate_shape()?;
        nonzero(&[
            self.state_proof_bundle_original_sha256,
            self.transition_statement_original_sha256,
            self.purpose1_approval_original_sha256,
            self.financial_control_original_sha256,
            self.admission_clock_context_original_sha256,
        ])?;
        let before = &self.reservation.selection.predecessor;
        if before.logical_sequence.checked_add(1) != Some(self.successor.logical_sequence)
            || before.state_commitment == self.successor.state_commitment
            || before.state_original_sha256 == self.successor.state_original_sha256
        {
            return Err("ordinary incoming full financial edge differs".into());
        }
        Ok(())
    }
    /// Exact complete incoming Commit digest; DATA effects require actual installed proof admission.
    /// # Errors
    /// Refuses malformed data or bounded canonical encoding.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        digest(b"iroha:kagemusha:v1:ordinary-incoming-commit\0", self)
    }
}
fn nonzero(values: &[[u8; 32]]) -> Result<(), String> {
    if values.contains(&[0; 32]) {
        return Err("ordinary incoming original selector absent".into());
    }
    Ok(())
}
fn digest<T: norito::NoritoSerialize>(domain: &[u8], value: &T) -> Result<[u8; 32], String> {
    let raw = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1 {
        return Err("ordinary incoming original exceeds bounded lineage frame".into());
    }
    let mut h = Sha256::new();
    h.update(domain);
    h.update((raw.len() as u64).to_le_bytes());
    h.update(raw);
    Ok(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha::{
        KagemushaOrdinaryLineageRequestOperationV1, KagemushaOrdinaryLineageRequestV1,
    };
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    fn selection(mint: bool) -> KagemushaOrdinaryIncomingSelectionV1 {
        let fixture = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let c = &fixture.selection.issuance.credential.subject;
        KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: KagemushaOrdinaryFinancialLineageV1 {
                version: 1,
                owner: fixture.selection.owner.clone(),
                financial_epoch_id: crate::kagemusha::kagemusha_ordinary_financial_epoch_id_v1(c)
                    .unwrap(),
                financial_authority_commitment: c.financial_authority_commitment,
            },
            operation_id: [40; 32],
            predecessor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: [41; 32],
                logical_sequence: (1_u128 << 110) + 77,
                state_original_sha256: [42; 32],
            },
            source: if mint {
                KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                    topup_request_original_sha256: [43; 32],
                }
            } else {
                KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
                    sender_commit_transport_original_sha256: [44; 32],
                    sender_outgoing_original_sha256: [45; 32],
                    recipient_request_original_digest: [46; 32],
                    encrypted_credit_original_sha256: [47; 32],
                }
            },
            credit_id: [48; 32],
            amount: 179,
            scale: fixture.selection.owner.runtime.scale,
            recipient_app_credential_digest: [49; 32],
            financial_control_original_sha256: [50; 32],
            clock_context_digest: [51; 32],
        }
    }
    fn reservation(mint: bool) -> KagemushaOrdinaryIncomingReservationV1 {
        KagemushaOrdinaryIncomingReservationV1 {
            selection: selection(mint),
            finalized_source_original_sha256: if mint { [52; 32] } else { [44; 32] },
            source_proof_original_sha256: if mint { [61; 32] } else { [45; 32] },
            source_semantic_digest: [53; 32],
        }
    }
    #[test]
    fn incoming_reservation_pins_full_current_source_proof_original_without_rekeying_credit() {
        let original = reservation(true);
        let key = original.selection.credit_id;
        let selection = original.selection.digest().unwrap();
        let digest = original.digest().unwrap();
        let mut changed = original.clone();
        changed.source_proof_original_sha256[0] ^= 1;
        assert_eq!(changed.selection.credit_id, key);
        assert_eq!(changed.selection.digest().unwrap(), selection);
        assert_ne!(changed.digest().unwrap(), digest);
        changed.source_proof_original_sha256 = [0; 32];
        assert!(changed.digest().is_err());
        let mut receive = reservation(false);
        assert!(receive.validate_shape().is_ok());
        receive.source_proof_original_sha256[0] ^= 1;
        assert!(receive.validate_shape().is_err());
    }
    fn commit() -> KagemushaOrdinaryIncomingCommitV1 {
        let r = reservation(false);
        let sequence = r.selection.predecessor.logical_sequence + 1;
        KagemushaOrdinaryIncomingCommitV1 {
            reservation: r,
            successor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: [54; 32],
                logical_sequence: sequence,
                state_original_sha256: [55; 32],
            },
            state_proof_bundle_original_sha256: [56; 32],
            transition_statement_original_sha256: [57; 32],
            purpose1_approval_original_sha256: [58; 32],
            financial_control_original_sha256: [59; 32],
            admission_clock_context_original_sha256: [60; 32],
        }
    }
    #[test]
    fn incoming_selector_is_acyclic_and_preserves_full_financial_head() {
        let mut r = reservation(true);
        let selected = r.selection.digest().unwrap();
        let final_digest = r.digest().unwrap();
        r.finalized_source_original_sha256[0] ^= 1;
        r.source_semantic_digest[0] ^= 1;
        assert_eq!(r.selection.digest().unwrap(), selected);
        assert_ne!(r.digest().unwrap(), final_digest);
        for field in 0..5 {
            let mut changed = r.selection.clone();
            match field {
                0 => changed.predecessor.logical_sequence ^= 1_u128 << 109,
                1 => changed.predecessor.state_original_sha256[0] ^= 1,
                2 => changed.financial_control_original_sha256[0] ^= 1,
                3 => changed.clock_context_digest[0] ^= 1,
                _ => changed.credit_id[0] ^= 1,
            }
            assert_ne!(changed.digest().unwrap(), selected);
        }
    }
    #[test]
    fn receive_source_retains_complete_receipt_and_cipher_selectors() {
        let mut r = reservation(false);
        assert_eq!(
            r.selection.source.operation(),
            KagemushaOperationKindV1::ReceiveFold
        );
        r.validate_shape().unwrap();
        r.finalized_source_original_sha256[0] ^= 1;
        assert!(r.validate_shape().is_err());
        r.finalized_source_original_sha256 = [44; 32];
        r.selection.source = KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
            sender_commit_transport_original_sha256: [44; 32],
            sender_outgoing_original_sha256: [45; 32],
            recipient_request_original_digest: [46; 32],
            encrypted_credit_original_sha256: [0; 32],
        };
        assert!(r.validate_shape().is_err());
        assert_eq!(
            selection(true).source.operation(),
            KagemushaOperationKindV1::MintFold
        );
    }
    #[test]
    fn incoming_commit_requires_exact_new_financial_sequence_and_public_state_original() {
        let original = commit();
        original.validate_shape().unwrap();
        for field in 0..5 {
            let mut changed = original.clone();
            let before = changed.reservation.selection.predecessor;
            match field {
                0 => changed.successor.logical_sequence ^= 1_u128 << 109,
                1 => changed.successor.state_commitment = before.state_commitment,
                2 => changed.successor.state_original_sha256 = before.state_original_sha256,
                3 => changed.state_proof_bundle_original_sha256 = [0; 32],
                _ => changed.purpose1_approval_original_sha256 = [0; 32],
            }
            assert!(changed.validate_shape().is_err());
        }
        let mut overflow = original;
        overflow.reservation.selection.predecessor.logical_sequence = u128::MAX;
        overflow.successor.logical_sequence = 0;
        assert!(overflow.validate_shape().is_err());
    }
    #[test]
    fn incoming_requests_have_distinct_kinds_and_actual_account_consent_binds_all_source_bytes() {
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let mut request = KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [61; 32],
            issuer_policy_digest: [62; 32],
            operation: KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
                reservation(false),
            )),
        };
        assert_eq!(
            request.operation.lineage(),
            &reservation(false).selection.lineage
        );
        let signature = Signature::new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        );
        request.verify_account_signature(&signature).unwrap();
        let reserved = request.canonical_bytes().unwrap();
        request.operation =
            KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(Box::new(commit()));
        request.operation.validate_shape().unwrap();
        assert_ne!(request.canonical_bytes().unwrap(), reserved);
        assert!(request.verify_account_signature(&signature).is_err());
    }
    #[test]
    fn incoming_full_canonical_request_roundtrip_refuses_trailing_or_zero_source() {
        let mut request = KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [63; 32],
            issuer_policy_digest: [64; 32],
            operation: KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(Box::new(
                commit(),
            )),
        };
        let raw = request.canonical_bytes().unwrap();
        let decoded: KagemushaOrdinaryLineageRequestV1 =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        assert_eq!(decoded, request);
        let mut trailing = raw;
        trailing.push(0);
        assert!(
            norito::decode_canonical_with_limits::<KagemushaOrdinaryLineageRequestV1>(
                &trailing,
                norito::canonical_decode_limits(trailing.len())
            )
            .is_err()
        );
        let mut r = reservation(true);
        r.selection.source = KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: [0; 32],
        };
        request.operation =
            KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(r));
        assert!(request.canonical_bytes().is_err());
    }
}
