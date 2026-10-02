//! Explicit operation-bound ordinary Mint pre-debit issuer assertion.
//!
//! The unsigned request and incoming selector exist before this decision. An actual Core issuer
//! signs only after the exact predecessor is durably reserved and independently current FI,
//! release, clock and ordinary proof gates pass. Node admission additionally requires the exact
//! current World `CanAuthorizeKagemushaOrdinaryMint` purpose, payer consent and genuine ordinary
//! Mint proof. This signed Core DATA-reservation assertion is never Taira Merkle membership,
//! finalized debit, a Native capability or a credit. No future finality or State original is an ID.

use super::{
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryIncomingSelectionV1,
    KagemushaOrdinaryIncomingSourceSelectionV1, KagemushaOrdinaryTopUpRequestV1,
    KagemushaRetailEnrollmentIssuerPolicyV1, kagemusha_ordinary_retail_issuer_policy_digest_v1,
};
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::Signature;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Complete signed pre-debit decision bound; unsigned ordinary proof/request travels separately.
pub const KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1: usize = 16 * 1024;
/// Finite issuer effect window. A stale decision cannot create another operation or renew consent.
pub const KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_LIFETIME_MS_V1: u64 = 10_000;
/// Sole purpose signing prefix, distinct from enrollment, FI read, DATA Commit and app Mint approval.
pub const KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_SIGNING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:ordinary-mint-pre-debit-issuer-decision\0";

/// Full exact Core decision after durable pre-debit predecessor reservation.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaOrdinaryMintDebitDecisionV1")]
pub struct KagemushaOrdinaryMintDebitDecisionV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact acyclic full selector, including full unsigned `TopUpRequest` SHA and predecessor.
    pub selection: KagemushaOrdinaryIncomingSelectionV1,
    /// Exact independently selected full FI issuer policy digest, also required by Node World grant.
    pub issuer_policy_digest: [u8; 32],
    /// Exact authenticated ordinary proof release selected by request, installed runtime and World.
    pub release_id: [u8; 32],
    /// SHA256 of the actual exclusive pre-debit DATA pending record original, not a caller revision.
    pub reserved_data_record_original_sha256: [u8; 32],
    /// Actual DATA revision after exclusive reservation; independent of financial logical u128.
    pub reserved_data_revision: u64,
    /// Actual signed DATA incarnation binding selected by the installed Core owner.
    pub data_incarnation_digest: [u8; 32],
    /// Actual DATA policy and schema at exclusive reservation.
    pub data_policy_epoch: u64,
    /// Actual DATA schema epoch at exclusive reservation.
    pub data_schema_epoch: u64,
    /// Actual current independently certified World cut under which Core checks release and issuer.
    pub authority_context_id: [u8; 32],
    /// Height of that exact independently certified cut, never a financial sequence.
    pub authority_height: u64,
    /// Exact complete certified World original root. This does not prove DATA membership.
    pub world_root: [u8; 32],
    /// SHA256 of separately fresh full signed FI-control original admitted after proof work.
    /// The immutable preparation FI decision remains in the request; it is never renewed here.
    pub current_financial_control_original_sha256: [u8; 32],
    /// Same actual current Native clock interval used by the issuing Core owner after proof work.
    /// Full observations and elapsed custody are authenticated and retained separately.
    pub decision_clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Actual post-reservation lower sample; immutable original inclusive start.
    pub issued_at_ms: u64,
    /// Immutable original exclusive end, never later than the finite decision lifetime.
    pub expires_at_ms: u64,
}
impl KagemushaOrdinaryMintDebitDecisionV1 {
    /// Validate bounded exact data and immutable interval; this grants no permission or effect.
    /// # Errors
    /// Refuses non-Mint source, zero scope, absent DATA identity or invalid finite time window.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.selection.validate_shape()?;
        self.decision_clock_context.validate_shape()?;
        if self.version != 1
            || !matches!(
                self.selection.source,
                KagemushaOrdinaryIncomingSourceSelectionV1::Mint { .. }
            )
            || self.reserved_data_revision == 0
            || self.data_policy_epoch == 0
            || self.data_schema_epoch == 0
            || self.authority_height == 0
            || self.issued_at_ms != self.decision_clock_context.lower_at_ms
            || self.decision_clock_context.upper_at_ms >= self.expires_at_ms
            || self
                .expires_at_ms
                .checked_sub(self.issued_at_ms)
                .is_none_or(|life| {
                    life == 0 || life > KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_LIFETIME_MS_V1
                })
        {
            return Err("ordinary Mint issuer decision scope or original interval differs".into());
        }
        if [
            self.issuer_policy_digest,
            self.release_id,
            self.reserved_data_record_original_sha256,
            self.data_incarnation_digest,
            self.authority_context_id,
            self.world_root,
            self.current_financial_control_original_sha256,
        ]
        .contains(&[0; 32])
        {
            return Err("ordinary Mint issuer decision original is absent".into());
        }
        Ok(())
    }
    /// Sole full canonical subject. No finality, future State proof or plaintext is serialized.
    /// # Errors
    /// Refuses malformed subject or finite canonical bound.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        bounded(self)
    }
    /// Exact purpose-bound signing message. Existing FI read signatures cannot verify this message.
    /// # Errors
    /// Refuses malformed subject, encoding or bounded full original.
    pub fn issuer_signing_message(&self) -> Result<Vec<u8>, String> {
        let raw = self.canonical_bytes()?;
        let mut message = KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_SIGNING_DOMAIN_V1.to_vec();
        message.extend((raw.len() as u64).to_le_bytes());
        message.extend(raw);
        Ok(message)
    }
    /// Join complete immutable request/selector and exact independently selected policy data.
    /// Caller must separately admit the actual Node World purpose and verify ordinary proof.
    /// # Errors
    /// Refuses changed full request, original selector/head, FI policy, runtime or release.
    pub fn validate_against_request(
        &self,
        request: &KagemushaOrdinaryTopUpRequestV1,
        selection: &KagemushaOrdinaryIncomingSelectionV1,
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        self.validate_against_selection(selection, issuer)?;
        selection.validate_against_topup(request)?;
        if request.authorization.statement.context.release_id != self.release_id {
            return Err("ordinary Mint issuer decision selects another proof release".into());
        }
        Ok(())
    }
    /// Exact selector/policy data join. It is not a World permission or DATA membership check.
    /// # Errors
    /// Refuses another selection, canonical policy/runtime or original issuer authorization window.
    pub fn validate_against_selection(
        &self,
        selection: &KagemushaOrdinaryIncomingSelectionV1,
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        self.validate_shape()?;
        issuer.validate().map_err(|e| e.to_string())?;
        if &self.selection != selection
            || self.selection.lineage.owner.runtime != issuer.runtime
            || self.issuer_policy_digest
                != kagemusha_ordinary_retail_issuer_policy_digest_v1(issuer)?
            || self.issued_at_ms < issuer.valid_from_ms
            || self.expires_at_ms > issuer.expires_at_ms
        {
            return Err("ordinary Mint issuer decision original selection/policy differs".into());
        }
        Ok(())
    }
}

/// Complete signed Core pre-debit assertion. Decoding/signature verification is data admission;
/// actual current Node World purpose and closed proof/debit owners remain mandatory.
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
#[norito_schema(name = "iroha_data_model::kagemusha::KagemushaSignedOrdinaryMintDebitDecisionV1")]
pub struct KagemushaSignedOrdinaryMintDebitDecisionV1 {
    /// Complete immutable signed operation/DATA/FI/clock body.
    pub subject: KagemushaOrdinaryMintDebitDecisionV1,
    /// Exact original Ed signature by the independently granted issuer key.
    pub signature: Signature,
}
impl KagemushaSignedOrdinaryMintDebitDecisionV1 {
    /// Sole full signed canonical original.
    /// # Errors
    /// Refuses malformed original or finite bound.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.subject.validate_shape()?;
        if self.signature.payload().len() != 64 {
            return Err("ordinary Mint issuer decision Ed signature width differs".into());
        }
        bounded(self)
    }
    /// Whole signed original SHA, for immutable Node execution/cold recovery records.
    /// # Errors
    /// Refuses malformed original or encoding.
    pub fn original_sha256(&self) -> Result<[u8; 32], String> {
        Ok(Sha256::digest(self.canonical_bytes()?).into())
    }
    /// Exact sole decoder. This cannot manufacture a World permission or Native owner.
    /// # Errors
    /// Refuses unsupported, noncanonical, oversized or trailing data.
    pub fn decode_canonical_exact(raw: &[u8]) -> Result<Self, String> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1 {
            return Err("ordinary Mint issuer decision bound differs".into());
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != raw {
            return Err("ordinary Mint issuer decision canonical original differs".into());
        }
        Ok(value)
    }
    /// Verify full signed request/policy/selector equation, independent of current World permission.
    /// # Errors
    /// Refuses changed full request/scope or another exact purpose signature.
    pub fn verify_for_request(
        &self,
        request: &KagemushaOrdinaryTopUpRequestV1,
        selection: &KagemushaOrdinaryIncomingSelectionV1,
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        self.subject
            .validate_against_request(request, selection, issuer)?;
        self.verify_for_selection(selection, issuer)
    }
    /// Verify the original Ed issuer equation over the exact complete purpose-bound selector.
    /// This result supplies no current World permission, debit, finality or Native authority.
    /// # Errors
    /// Refuses selector/policy drift or a signature from another message/purpose/key.
    pub fn verify_for_selection(
        &self,
        selection: &KagemushaOrdinaryIncomingSelectionV1,
        issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    ) -> Result<(), String> {
        self.subject.validate_against_selection(selection, issuer)?;
        self.signature
            .verify(
                &issuer.issuer_public_key,
                &self.subject.issuer_signing_message()?,
            )
            .map_err(|e| e.to_string())
    }
}
fn bounded<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let raw = norito::encode_canonical(value).map_err(|e| e.to_string())?;
    if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1 {
        return Err("ordinary Mint issuer decision original bound differs".into());
    }
    Ok(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha::{
        KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryFinancialLineageV1,
        kagemusha_ordinary_financial_epoch_id_v1,
    };
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use iroha_crypto::{Algorithm, KeyPair};

    // Genuine known-public Ed equations over data-only test selectors. No Mint proof, observed
    // clock, DATA reservation, World permission, account debit or finalized credit is asserted.
    fn original() -> (
        KagemushaRetailEnrollmentIssuerPolicyV1,
        KagemushaSignedOrdinaryMintDebitDecisionV1,
    ) {
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let verified = f.verify(1000).unwrap();
        let c = verified.app_credential();
        let policy = f.issuer_policy.clone();
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: KagemushaOrdinaryFinancialLineageV1 {
                version: 1,
                owner: f.selection.owner.clone(),
                financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(c.subject()).unwrap(),
                financial_authority_commitment: c.subject().financial_authority_commitment,
            },
            operation_id: [81; 32],
            predecessor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: [82; 32],
                logical_sequence: (1_u128 << 101) + 7,
                state_original_sha256: [83; 32],
            },
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: [84; 32],
            },
            credit_id: [85; 32],
            amount: 177,
            scale: policy.runtime.scale,
            recipient_app_credential_digest: c.digest(),
            financial_control_original_sha256: [86; 32],
            clock_context_digest: [87; 32],
        };
        let subject = KagemushaOrdinaryMintDebitDecisionV1 {
            version: 1,
            selection,
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(&policy)
                .unwrap(),
            release_id: c.subject().release_id,
            reserved_data_record_original_sha256: [88; 32],
            reserved_data_revision: 11,
            data_incarnation_digest: [89; 32],
            data_policy_epoch: 3,
            data_schema_epoch: 2,
            authority_context_id: [90; 32],
            authority_height: 101,
            world_root: [91; 32],
            current_financial_control_original_sha256: [92; 32],
            decision_clock_context: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [93; 32],
                signed_observations_original_digest: [94; 32],
                lower_at_ms: 1000,
                upper_at_ms: 1010,
            },
            issued_at_ms: 1000,
            expires_at_ms: 1100,
        };
        let key = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        assert_eq!(key.public_key(), &policy.issuer_public_key);
        let signature = Signature::new(
            key.private_key(),
            &subject.issuer_signing_message().unwrap(),
        );
        (
            policy,
            KagemushaSignedOrdinaryMintDebitDecisionV1 { subject, signature },
        )
    }
    #[test]
    fn ordinary_mint_issuer_decision_full_original_equation_roundtrip_and_scope_mutation() {
        let (policy, signed) = original();
        let selection = &signed.subject.selection;
        signed.verify_for_selection(selection, &policy).unwrap();
        let raw = signed.canonical_bytes().unwrap();
        let decoded =
            KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(&raw).unwrap();
        assert_eq!(decoded, signed);
        assert_eq!(
            signed.original_sha256().unwrap(),
            <[u8; 32]>::from(Sha256::digest(&raw))
        );
        let mut changed = signed.clone();
        changed.subject.selection.amount += 1;
        changed
            .verify_for_selection(&changed.subject.selection, &policy)
            .expect_err("valid-looking amount cannot reuse issuer signature");
        let mut changed = signed.clone();
        changed.subject.selection.predecessor.logical_sequence += 1;
        changed
            .verify_for_selection(&changed.subject.selection, &policy)
            .expect_err("full u128 predecessor cannot reuse issuer signature");
        let mut changed = signed.clone();
        changed.subject.reserved_data_record_original_sha256[0] ^= 1;
        changed
            .verify_for_selection(selection, &policy)
            .expect_err("another reserved DATA original cannot reuse issuer signature");
        let mut changed = signed.clone();
        changed.subject.decision_clock_context.request_nonce[0] ^= 1;
        changed
            .verify_for_selection(selection, &policy)
            .expect_err("another current clock cannot reuse issuer signature");
        let mut changed_policy = policy;
        changed_policy.expires_at_ms += 1;
        signed
            .verify_for_selection(selection, &changed_policy)
            .expect_err("another complete policy cannot lend existing signature");
    }
    #[test]
    fn ordinary_mint_issuer_decision_other_purpose_time_and_noncanonical_refusal() {
        let (policy, signed) = original();
        let key = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        let mut wrong_purpose = signed.clone();
        wrong_purpose.signature = Signature::new(
            key.private_key(),
            &signed.subject.canonical_bytes().unwrap(),
        );
        wrong_purpose
            .verify_for_selection(&signed.subject.selection, &policy)
            .expect_err("bare subject signature cannot use Mint purpose");
        let mut changed = signed.clone();
        changed.subject.issued_at_ms += 1;
        changed
            .subject
            .validate_shape()
            .expect_err("issued sample is actual lower endpoint");
        let mut changed = signed.clone();
        changed.subject.expires_at_ms = changed.subject.decision_clock_context.upper_at_ms;
        changed
            .subject
            .validate_shape()
            .expect_err("upper endpoint must remain before exclusive expiry");
        let mut changed = signed.clone();
        changed.subject.expires_at_ms = changed.subject.issued_at_ms
            + KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_LIFETIME_MS_V1
            + 1;
        changed
            .subject
            .validate_shape()
            .expect_err("decision cannot widen original finite interval");
        let mut changed = signed.clone();
        changed.subject.version = 2;
        KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &norito::encode_canonical(&changed).unwrap(),
        )
        .expect_err("no later version fallback");
        let mut trailing = signed.canonical_bytes().unwrap();
        trailing.push(0);
        KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(&trailing)
            .expect_err("whole original framing retained");
        KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(&vec![
            0;
            KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1
                + 1
        ])
        .expect_err("bound checked before decode");
    }
    #[test]
    fn ordinary_mint_issuer_decision_refuses_separate_app_and_core_enrollment_signers() {
        let (policy, signed) = original();
        signed
            .verify_for_selection(&signed.subject.selection, &policy)
            .unwrap();
        // The retained fixture separates app authority61, Core enrollment63 and FI issuer64.
        // Signing the exact complete Mint message cannot upgrade either unrelated role.
        let message = signed.subject.issuer_signing_message().unwrap();
        for seed in [61, 63] {
            let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            assert_ne!(key.public_key(), &policy.issuer_public_key);
            let mut other_role = signed.clone();
            other_role.signature = Signature::new(key.private_key(), &message);
            other_role
                .verify_for_selection(&signed.subject.selection, &policy)
                .expect_err(
                    "another actual fixture role cannot sign the selected FI Mint decision",
                );
        }
    }
    #[test]
    fn ordinary_mint_issuer_decision_pins_complete_unsigned_topup_original() {
        let f = crate::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let context = &f.request.authorization.statement.context;
        let selection = KagemushaOrdinaryIncomingSelectionV1 {
            version: 1,
            lineage: context.lineage.clone(),
            operation_id: context.operation_id,
            predecessor: context.predecessor.clone(),
            source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256: Sha256::digest(f.request.canonical_bytes().unwrap())
                    .into(),
            },
            credit_id: f.request.authorization.statement.credit_id,
            amount: context.amount,
            scale: context.lineage.owner.runtime.scale,
            recipient_app_credential_digest: context.recipient_app_credential_digest,
            financial_control_original_sha256: context.financial_control_original_sha256,
            clock_context_digest: context.clock_context.binding_digest().unwrap(),
        };
        let policy = &f.enrollment_fixture.issuer_policy;
        let (_, template) = original();
        let mut subject = template.subject;
        subject.selection = selection.clone();
        subject.release_id = context.release_id;
        subject.issuer_policy_digest =
            kagemusha_ordinary_retail_issuer_policy_digest_v1(policy).unwrap();
        let key = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        let signed = KagemushaSignedOrdinaryMintDebitDecisionV1 {
            signature: Signature::new(
                key.private_key(),
                &subject.issuer_signing_message().unwrap(),
            ),
            subject,
        };
        f.request
            .verify_account_signature(&f.account_consent)
            .unwrap();
        signed
            .verify_for_request(&f.request, &selection, policy)
            .unwrap();
        // Independently shape-valid another proof carrier keeps every public context field but
        // must still have a different full unsigned request original and cannot reuse a decision.
        let mut swapped = f.request.clone();
        swapped.authorization.proof.eq_proof.push(17);
        swapped.canonical_bytes().unwrap();
        signed
            .verify_for_request(&swapped, &selection, policy)
            .expect_err("another complete proof original cannot reuse pre-debit decision");
        let mut absent = signed;
        absent.signature = Signature::from_bytes(&[]);
        absent
            .canonical_bytes()
            .expect_err("complete decision has exact64 Ed signature");
    }
}
