//! Protected Native custody of genuine globally serialized DATA lineage results.
//! A signed immutable DATA assertion is distinct from Taira Merkle membership and from a live FI loan.
use super::*;
#[path = "lineage_cas/received_assertion.rs"]
mod received_assertion;
use crate::kagemusha_v1_recursion::{
    KagemushaVerifiedOrdinaryLineageAnchorProofV1, KagemushaVerifiedOrdinaryLineageCommitProofV1,
    KagemushaVerifiedOrdinaryLineageReservationProofV1,
};
use crate::kagemusha_v1_state::KagemushaRecoveryJournalPrefixV1;
use iroha_crypto::Signature;
use iroha_torii_shared::kagemusha_state::decode_unverified_kagemusha_authority_state_v1;
pub use received_assertion::{
    KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
    KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1,
    KagemushaOrdinaryReceivedLineageCommitOriginalV1,
};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[path = "lineage_cas/account_signing.rs"]
mod account_signing;
#[path = "lineage_cas/effect_capture.rs"]
mod effect_capture;
pub use account_signing::KagemushaAuthenticatedOrdinaryLineageAccountSigningV1;

const MAX_RECORD: usize = 20 * 1024 * 1024;
const MAX_DATA_RECORD: usize = 128 * 1024;
const MAX_FINALITY_ORIGINAL: usize = 16 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const MAX_ACKNOWLEDGED: usize = 4096;
const MAX_ACKNOWLEDGED_BYTES: usize = 64 * 1024 * 1024;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-lineage-cas.norito.wal",
    magic: b"KGMCAS01",
    hash_domain: b"iroha:kagemusha:v1:ordinary-lineage-cas-wal\0",
    maximum_payload_bytes: MAX_RECORD as u64,
};
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::LineageCasInitializeV1")]
struct Initialize {
    lineage: KagemushaOrdinaryFinancialLineageV1,
    policy_original: Vec<u8>,
    enrollment_original_sha256: [u8; 32],
    credential_original_sha256: [u8; 32],
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::LineageCasResultOriginalsV1")]
struct ResultOriginals {
    signed_result: Vec<u8>,
    data_record: Vec<u8>,
    finality_original: Vec<u8>,
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::LineageCasAcknowledgementV1")]
struct Acknowledgement {
    originals_sha256: [u8; 32],
    // Actual Native sample after original fsync, never an offered timestamp or expiry extension.
    captured_clock: KagemushaOrdinaryCashClockContextV1,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::LineageCasRecordV1")]
enum Record {
    Initialize(Box<Initialize>),
    Reserve(Box<KagemushaOrdinaryLineageRequestV1>),
    AccountInvoked(Box<[u8; 32]>),
    AccountSigned(Box<([u8; 32], [u8; 64])>),
    ResultOriginals(Box<ResultOriginals>),
    Acknowledged(Box<Acknowledgement>),
}
struct Pending {
    request: KagemushaOrdinaryLineageRequestV1,
    invoked: bool,
    signature: Option<[u8; 64]>,
    originals: Option<ResultOriginals>,
}
struct Acknowledged {
    request: KagemushaOrdinaryLineageRequestV1,
    originals: ResultOriginals,
    acknowledgement: Acknowledgement,
    result: KagemushaSignedOrdinaryLineageResultV1,
}
#[derive(Default)]
struct Floor {
    incarnation: Option<[u8; 32]>,
    revision: u64,
    policy_epoch: u64,
    schema_epoch: u64,
}
/// Exclusive private lineage journal under the actual installed financial/runtime owner.
/// This type cannot be constructed from a transported result, decoded receipt, caller clock or
/// Boolean proof status. Its installing Rust owner must retain the exact signed inventory policy.
pub struct KagemushaOrdinaryLineageCasOwnerV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    initialize: Initialize,
    policy: KagemushaOrdinaryLineageIssuerPolicyV1,
    journal: PrivateJournal,
    prefix: Option<KagemushaRecoveryJournalPrefixV1>,
    rows: usize,
    // A possible durable suffix without synchronized semantic memory permanently closes this process.
    persistence_uncertain: bool,
    used_nonces: BTreeSet<[u8; 32]>,
    pending: Option<Pending>,
    acknowledged: BTreeMap<[u8; 32], Acknowledged>,
    acknowledged_bytes: usize,
    floor: Floor,
}
/// Actual original zero anchor acknowledged only after genuine durable global admission.
pub(crate) struct KagemushaAuthenticatedOrdinaryLineageAnchorReceiptV1<'a> {
    owner: &'a KagemushaOrdinaryLineageCasOwnerV1,
    request_sha256: [u8; 32],
}
/// Actual predecessor/successor reservation acknowledged before the purpose1 W1 operation.
pub(crate) struct KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'a> {
    owner: &'a KagemushaOrdinaryLineageCasOwnerV1,
    request_sha256: [u8; 32],
}
/// Actual globally serialized financial commit, required before State/outbox financial effects.
pub(crate) struct KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1<'a> {
    owner: &'a KagemushaOrdinaryLineageCasOwnerV1,
    request_sha256: [u8; 32],
}
impl KagemushaOrdinaryLineageCasOwnerV1 {
    /// Create actual custody using the exact policy original retained by the independently signed
    /// installed inventory. No mobile ABI accepts that original as authority.
    pub(crate) fn create(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        installed_policy_original: &[u8],
    ) -> Result<Self> {
        financial.recheck()?;
        let journal = PrivateJournal::create_new(&root.join("ordinary-lineage-cas"), FORMAT)
            .map_err(|_| Custody)?;
        let mut owner = Self::new(journal, financial, installed_policy_original)?;
        owner.append(&Record::Initialize(Box::new(owner.initialize.clone())))?;
        Ok(owner)
    }
    /// Replay exact original authority and acknowledged immutable results, never a live FI or
    /// elapsed clock. A pending raw result cannot be converted into an acknowledged receipt.
    pub(crate) fn open_existing(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        installed_policy_original: &[u8],
    ) -> Result<Self> {
        financial.recheck_historical_proof_custody()?;
        let journal = PrivateJournal::open_existing(&root.join("ordinary-lineage-cas"), FORMAT)
            .map_err(|_| Custody)?;
        let mut owner = Self::new(journal, financial, installed_policy_original)?;
        while let Some((_, raw)) = owner.journal.replay_next().map_err(|_| Custody)? {
            if owner.rows >= MAX_ROWS {
                return Err(Rejected);
            }
            match decode::<Record>(&raw, MAX_RECORD)? {
                Record::Initialize(value) if owner.rows == 0 && *value == owner.initialize => {}
                Record::Reserve(request) if owner.rows > 0 && owner.pending.is_none() => {
                    owner.install_pending(*request)?
                }
                Record::AccountInvoked(nonce) if owner.rows > 0 => {
                    let pending = owner.pending.as_mut().ok_or(Rejected)?;
                    if pending.invoked || pending.request.request_nonce != *nonce {
                        return Err(Rejected);
                    }
                    pending.invoked = true;
                }
                Record::AccountSigned(value) if owner.rows > 0 => {
                    owner.install_signature(value.0, value.1)?
                }
                Record::ResultOriginals(originals) if owner.rows > 0 => {
                    owner.validate_result_originals(&originals)?;
                    let pending = owner.pending.as_mut().ok_or(Rejected)?;
                    if pending.originals.is_some() {
                        return Err(Rejected);
                    }
                    pending.originals = Some(*originals);
                }
                Record::Acknowledged(ack) if owner.rows > 0 => {
                    financial.retained_cash_clock_originals(&ack.captured_clock)?;
                    owner.install_acknowledgement(*ack)?
                }
                _ => return Err(Rejected),
            }
            owner.rows += 1;
        }
        if owner.rows == 0 {
            return Err(Rejected);
        }
        owner.prefix = Some(owner.journal.recovery_prefix().map_err(|_| Custody)?);
        owner.recheck_historical(financial)?;
        Ok(owner)
    }
    fn new(
        journal: PrivateJournal,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original: &[u8],
    ) -> Result<Self> {
        financial.recheck_historical_proof_custody()?;
        let policy: KagemushaOrdinaryLineageIssuerPolicyV1 =
            decode(original, KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1)?;
        let selected = financial.reservation.selected.clone();
        policy
            .validate_for_issuer(&selected.issuer)
            .map_err(|_| Rejected)?;
        let lineage = KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: selected.owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(
                financial.enrollment.app_credential().subject(),
            )
            .map_err(|_| Rejected)?,
            financial_authority_commitment: financial
                .historical_financial_authority_commitment()?,
        };
        lineage.validate_shape().map_err(|_| Rejected)?;
        let initialize = Initialize {
            lineage,
            policy_original: original.to_vec(),
            enrollment_original_sha256: kagemusha_ordinary_current_control_original_sha256_v1(
                financial.enrollment.certificate(),
            )
            .map_err(|_| Rejected)?,
            credential_original_sha256: Sha256::digest(
                financial.enrollment.app_credential().original(),
            )
            .into(),
        };
        Ok(Self {
            selected,
            enrollment: financial.enrollment.clone(),
            initialize,
            policy,
            journal,
            prefix: None,
            rows: 0,
            persistence_uncertain: false,
            used_nonces: BTreeSet::new(),
            pending: None,
            acknowledged: BTreeMap::new(),
            acknowledged_bytes: 0,
            floor: Floor::default(),
        })
    }
    pub(crate) fn reserve_anchor(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryLineageAnchorProofV1,
    ) -> Result<Vec<u8>> {
        if proof.state_proof().normalized_statement().release_id
            != self.selected.governed.release().release_id()
        {
            return Err(Rejected);
        }

        self.reserve(
            financial,
            current,
            KagemushaOrdinaryLineageRequestOperationV1::Anchor(Box::new(proof.anchor().clone())),
        )
    }
    pub(crate) fn reserve_transition(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryLineageReservationProofV1,
    ) -> Result<Vec<u8>> {
        if proof.state_proof().normalized_statement().release_id
            != self.selected.governed.release().release_id()
        {
            return Err(Rejected);
        }

        self.reserve(
            financial,
            current,
            KagemushaOrdinaryLineageRequestOperationV1::Reserve(Box::new(
                proof.reservation().clone(),
            )),
        )
    }
    pub(crate) fn reserve_commit(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryLineageCommitProofV1,
        reserve_request_sha256_from_native_intent: [u8; 32],
    ) -> Result<Vec<u8>> {
        let acknowledged =
            self.acknowledged(reserve_request_sha256_from_native_intent, financial)?;
        match &acknowledged.request.operation {
            KagemushaOrdinaryLineageRequestOperationV1::Reserve(reservation)
                if reservation.as_ref() == &proof.commit().reservation => {}
            _ => return Err(Rejected),
        }
        self.reserve(
            financial,
            current,
            KagemushaOrdinaryLineageRequestOperationV1::Commit(Box::new(proof.commit().clone())),
        )
    }
    fn reserve(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        operation: KagemushaOrdinaryLineageRequestOperationV1,
    ) -> Result<Vec<u8>> {
        self.recheck_live(financial, current)?;
        if operation.lineage() != &self.initialize.lineage {
            return Err(Rejected);
        }
        if let Some(pending) = &self.pending {
            if pending.request.operation != operation {
                return Err(Rejected);
            }
            return pending.request.canonical_bytes().map_err(|_| Rejected);
        }
        if self.acknowledged.len() >= MAX_ACKNOWLEDGED
            || self.acknowledged_bytes > MAX_ACKNOWLEDGED_BYTES.saturating_sub(MAX_RECORD)
        {
            return Err(Rejected);
        }
        let mut nonce = [0; 32];
        OsRng.try_fill_bytes(&mut nonce).map_err(|_| Custody)?;
        let request = KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: nonce,
            issuer_policy_digest: self.policy.issuer_policy_digest,
            operation,
        };
        self.require_request(&request)?;
        self.append(&Record::Reserve(Box::new(request.clone())))?;
        self.recheck_live(financial, current)?;
        request.canonical_bytes().map_err(|_| Rejected)
    }
    /// The shared Native AccountClient/session signs only these exact retained bytes; managed
    /// fields cannot replace its session/key. Fence survives an uncertain signing reply.
    pub(crate) fn fence_account_signing(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
    ) -> Result<Vec<u8>> {
        self.recheck_live(financial, current)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if pending.invoked || pending.signature.is_some() {
            return Err(Rejected);
        }
        let nonce = pending.request.request_nonce;
        let message = pending
            .request
            .account_signing_message()
            .map_err(|_| Rejected)?;
        self.append(&Record::AccountInvoked(Box::new(nonce)))?;
        self.recheck_live(financial, current)?;
        Ok(message)
    }
    pub(crate) fn capture_account_signature(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        signature: [u8; 64],
    ) -> Result<()> {
        self.recheck_live(financial, current)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        let nonce = pending.request.request_nonce;
        if !pending.invoked || pending.signature.is_some() {
            return Err(Rejected);
        }
        pending
            .request
            .verify_account_signature(&Signature::from_bytes(&signature))
            .map_err(|_| Rejected)?;
        self.append(&Record::AccountSigned(Box::new((nonce, signature))))?;
        self.recheck_live(financial, current)
    }
    pub(crate) fn signed_request_originals(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
    ) -> Result<(Vec<u8>, [u8; 64])> {
        self.recheck_live(financial, current)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        Ok((
            pending.request.canonical_bytes().map_err(|_| Rejected)?,
            pending.signature.ok_or(Rejected)?,
        ))
    }
    /// Authenticate complete signed result and actual World first. Raw fsync followed by a
    /// distinct post-raw-fsync Native acknowledgement is required before any receipt is lent.
    pub(crate) fn accept_result(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        signed_result: &[u8],
        data_record: &[u8],
        authority_original: &[u8],
    ) -> Result<[u8; 32]> {
        self.recheck_live(financial, current)?;
        let authority = decode_unverified_kagemusha_authority_state_v1(authority_original)
            .map_err(|_| Rejected)?;
        let original: KagemushaSignedOrdinaryLineageResultV1 = decode(
            signed_result,
            KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        )?;
        self.verify_world(&original.subject, &authority)?;
        let finality_original =
            norito::encode_canonical(&authority.attestation.body.finality_proof)
                .map_err(|_| Rejected)?;
        let originals = ResultOriginals {
            signed_result: signed_result.to_vec(),
            data_record: data_record.to_vec(),
            finality_original,
        };
        self.validate_result_originals(&originals)?;
        self.require_current_data(current, &original.subject)?;
        let digest = originals_digest(&originals)?;
        if let Some(existing) = self.pending.as_ref().ok_or(Rejected)?.originals.as_ref() {
            if originals_digest(existing)? != digest {
                return Err(Rejected);
            }
        } else {
            self.append(&Record::ResultOriginals(Box::new(originals.clone())))?;
        }
        // The original must still be admitted at actual BOTH bounds after its original fsync.
        self.recheck_live(financial, current)?;
        let captured_clock = financial.current_cash_clock_context()?;
        if captured_clock.lower_at_ms < original.subject.issued_at_ms {
            return Err(Rejected);
        }
        let ack = Acknowledgement {
            originals_sha256: digest,
            captured_clock,
        };
        self.append(&Record::Acknowledged(Box::new(ack.clone())))?;
        self.recheck_live(financial, current)?;
        Ok(request_digest(&original.subject.request)?)
    }
    fn require_request(&self, request: &KagemushaOrdinaryLineageRequestV1) -> Result<()> {
        request.canonical_bytes().map_err(|_| Rejected)?;
        if request.issuer_policy_digest != self.policy.issuer_policy_digest
            || request.operation.lineage() != &self.initialize.lineage
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn install_pending(&mut self, request: KagemushaOrdinaryLineageRequestV1) -> Result<()> {
        self.require_request(&request)?;
        if self.pending.is_some() || !self.used_nonces.insert(request.request_nonce) {
            return Err(Rejected);
        }
        self.pending = Some(Pending {
            request,
            invoked: false,
            signature: None,
            originals: None,
        });
        Ok(())
    }
    fn install_signature(&mut self, nonce: [u8; 32], signature: [u8; 64]) -> Result<()> {
        let pending = self.pending.as_mut().ok_or(Rejected)?;
        if !pending.invoked || pending.signature.is_some() || pending.request.request_nonce != nonce
        {
            return Err(Rejected);
        }
        pending
            .request
            .verify_account_signature(&Signature::from_bytes(&signature))
            .map_err(|_| Rejected)?;
        pending.signature = Some(signature);
        Ok(())
    }
    fn validate_result_originals(
        &self,
        originals: &ResultOriginals,
    ) -> Result<KagemushaSignedOrdinaryLineageResultV1> {
        if originals.data_record.is_empty()
            || originals.data_record.len() > MAX_DATA_RECORD
            || originals.finality_original.is_empty()
            || originals.finality_original.len() > MAX_FINALITY_ORIGINAL
        {
            return Err(Rejected);
        }
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if pending.signature.is_none() {
            return Err(Rejected);
        }
        let signed: KagemushaSignedOrdinaryLineageResultV1 = decode(
            &originals.signed_result,
            KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
        )?;
        signed
            .verify_for_request(&pending.request, &self.policy.issuer_public_key)
            .map_err(|_| Rejected)?;
        let s = &signed.subject;
        if s.cas_policy_digest != self.policy.digest().map_err(|_| Rejected)?
            || s.release_id != self.selected.governed.release().release_id()
            || s.data_record_original_sha256
                != <[u8; 32]>::from(Sha256::digest(&originals.data_record))
        {
            return Err(Rejected);
        }
        self.verify_retained_finality(s, &originals.finality_original)?;
        self.require_floor(s)?;
        Ok(signed)
    }
    fn install_acknowledgement(&mut self, acknowledgement: Acknowledgement) -> Result<()> {
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        let originals = pending.originals.as_ref().ok_or(Rejected)?.clone();
        let result = self.validate_result_originals(&originals)?;
        acknowledgement
            .captured_clock
            .validate_shape()
            .map_err(|_| Rejected)?;
        if originals_digest(&originals)? != acknowledgement.originals_sha256
            || acknowledgement.captured_clock.lower_at_ms < result.subject.issued_at_ms
        {
            return Err(Rejected);
        }
        let encoded_len = encode(&Record::ResultOriginals(Box::new(originals.clone())))?.len();
        let bytes = self
            .acknowledged_bytes
            .checked_add(encoded_len)
            .ok_or(Rejected)?;
        let key = request_digest(&pending.request)?;
        if self.acknowledged.len() >= MAX_ACKNOWLEDGED
            || bytes > MAX_ACKNOWLEDGED_BYTES
            || self.acknowledged.contains_key(&key)
        {
            return Err(Rejected);
        }
        let request = pending.request.clone();
        self.advance_floor(&result.subject)?;
        self.acknowledged.insert(
            key,
            Acknowledged {
                request,
                originals,
                acknowledgement,
                result,
            },
        );
        self.acknowledged_bytes = bytes;
        self.pending = None;
        Ok(())
    }
    /// Data-only original lineage/purpose digest borrowed from this actual held private owner.
    /// Decoding matching bytes cannot construct this custody or an acknowledged receipt.
    pub(crate) fn retained_lineage_originals(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<(&KagemushaOrdinaryFinancialLineageV1, [u8; 32])> {
        self.recheck_historical(financial)?;
        Ok((
            &self.initialize.lineage,
            Sha256::digest(&self.initialize.policy_original).into(),
        ))
    }
    /// Recheck the exact already-held private WAL/financial/purpose identity without a live grant.
    /// This cannot create a receipt or make a cold acknowledgement current.
    pub(crate) fn recheck_retained_custody(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.recheck_historical(financial)
    }
    fn recheck_historical(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        financial.recheck_historical_proof_custody()?;
        if !Arc::ptr_eq(&self.selected, &financial.reservation.selected)
            || !Arc::ptr_eq(&self.enrollment, &financial.enrollment)
        {
            return Err(Rejected);
        }
        self.policy
            .validate_for_issuer(&self.selected.issuer)
            .map_err(|_| Rejected)?;
        if self.initialize.lineage.financial_authority_commitment
            != financial.historical_financial_authority_commitment()?
        {
            return Err(Rejected);
        }
        self.recheck_journal()
    }
    fn recheck_live(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
    ) -> Result<()> {
        self.recheck_historical(financial)?;
        financial.recheck()?;
        current.recheck()?;
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode(
            current.original()?,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        if control.subject.request.owner != self.initialize.lineage.owner
            || control.subject.request.enrollment_original_sha256
                != self.initialize.enrollment_original_sha256
            || control.subject.request.credential_original_sha256
                != self.initialize.credential_original_sha256
            || control.subject.request.issuer_policy_digest != self.policy.issuer_policy_digest
        {
            return Err(Rejected);
        }
        self.recheck_journal()
    }
    fn require_current_data(
        &self,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        result: &KagemushaOrdinaryLineageResultSubjectV1,
    ) -> Result<()> {
        current.recheck()?;
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode(
            current.original()?,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        if self
            .floor
            .incarnation
            .is_some_and(|incarnation| incarnation != control.subject.data_incarnation_digest)
            || control.subject.data_revision < self.floor.revision
            || control.subject.data_policy_epoch < self.floor.policy_epoch
            || control.subject.data_schema_epoch < self.floor.schema_epoch
            || control.subject.data_incarnation_digest != result.data_incarnation_digest
            || control.subject.data_policy_epoch != result.data_policy_epoch
            || control.subject.data_schema_epoch != result.data_schema_epoch
            || control.subject.release_id != result.release_id
            || control.subject.authority_height < result.authority_height
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn actual_clock(&self) -> Result<&Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>> {
        match &self.selected.clock {
            SelectedClock::Native(clock) => Ok(clock),
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => Err(Rejected),
        }
    }
    fn verify_retained_finality(
        &self,
        s: &KagemushaOrdinaryLineageResultSubjectV1,
        raw: &[u8],
    ) -> Result<()> {
        let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
            decode(raw, MAX_FINALITY_ORIGINAL)?;
        let clock = self.actual_clock()?.lock().map_err(|_| Custody)?;
        let verifier = clock
            .retained_finality_verifier_for_original_custody()
            .map_err(|_| Custody)?;
        let block = verifier
            .verify_retained_decision(&proof)
            .map_err(|_| Rejected)?;
        if block.height() != s.authority_height
            || block.context_id().as_ref() != &s.authority_context_id
            || block.execution().world_state_root.as_ref() != &s.authority_world_root
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn verify_world(
        &self,
        s: &KagemushaOrdinaryLineageResultSubjectV1,
        a: &iroha_torii_shared::kagemusha_state::KagemushaAuthorityStateV1,
    ) -> Result<()> {
        let verifier = self
            .actual_clock()?
            .lock()
            .map_err(|_| Custody)?
            .current_finality_verifier()
            .map_err(|_| Custody)?;
        let block = verifier
            .verify_retained_decision(&a.attestation.body.finality_proof)
            .map_err(|_| Rejected)?;
        if block.height() != s.authority_height
            || block.context_id().as_ref() != &s.authority_context_id
            || block.execution().world_state_root.as_ref() != &s.authority_world_root
            || self.selected.world_schema_hash != Some(a.world_snapshot.schema_hash)
            || a.asset_definition.id != self.selected.owner.runtime.asset
            || a.asset_incarnation != self.selected.owner.runtime.asset_incarnation
            || a.asset_definition.spec().scale() != Some(self.selected.owner.runtime.scale)
        {
            return Err(Rejected);
        }
        let world = a
            .world_snapshot
            .authenticate(&block)
            .map_err(|_| Rejected)?;
        world
            .verify_table_value(
                "world.asset_definitions",
                &a.asset_definition.id,
                &a.asset_definition,
            )
            .map_err(|_| Rejected)?;
        world
            .verify_table_value(
                "world.axt_asset_incarnations",
                &a.asset_definition.id,
                &a.asset_incarnation,
            )
            .map_err(|_| Rejected)?;
        world
            .verify_cell_value("world.kagemusha_verifier_registry", &a.verifier_registry)
            .map_err(|_| Rejected)?;
        let release = self.selected.governed.release();
        a.verifier_registry.validate().map_err(|_| Rejected)?;
        if a.verifier_registry.active_release_id != Some(release.release_id())
            || a.verifier_registry
                .authority_policy
                .as_ref()
                .ok_or(Rejected)?
                .canonical_digest()
                .map_err(|_| Rejected)?
                != release.authority_policy_digest()
            || a.verifier_registry
                .releases
                .iter()
                .find(|r| r.release_id == release.release_id())
                != Some(&KagemushaGovernedVerifierReleaseV1::from_authenticated(
                    release,
                    KAGEMUSHA_RELEASE_ACTIVE_V1,
                ))
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn require_floor(&self, s: &KagemushaOrdinaryLineageResultSubjectV1) -> Result<()> {
        if self
            .floor
            .incarnation
            .is_some_and(|i| i != s.data_incarnation_digest)
            || s.data_revision < self.floor.revision
            || s.data_policy_epoch < self.floor.policy_epoch
            || s.data_schema_epoch < self.floor.schema_epoch
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn advance_floor(&mut self, s: &KagemushaOrdinaryLineageResultSubjectV1) -> Result<()> {
        self.require_floor(s)?;
        self.floor = Floor {
            incarnation: Some(s.data_incarnation_digest),
            revision: s.data_revision,
            policy_epoch: s.data_policy_epoch,
            schema_epoch: s.data_schema_epoch,
        };
        Ok(())
    }
    fn recheck_journal(&self) -> Result<()> {
        if self.persistence_uncertain {
            return Err(Custody);
        }
        self.journal.check_owned().map_err(|_| Custody)?;
        if self
            .prefix
            .is_some_and(|p| self.journal.recovery_prefix().ok() != Some(p))
        {
            return Err(Custody);
        }
        Ok(())
    }
    fn append(&mut self, r: &Record) -> Result<()> {
        self.recheck_journal()?;
        if self.rows >= MAX_ROWS {
            return Err(Rejected);
        }
        let bytes = encode(r)?;
        // Keep this poison set on every error after the first potential write. The private
        // journal also poisons its own uncertain I/O, but holder memory has its own boundary.
        self.persistence_uncertain = true;
        self.journal.append(&bytes).map_err(|_| Custody)?;
        self.rows += 1;
        self.prefix = Some(self.journal.recovery_prefix().map_err(|_| Custody)?);
        // Durable memory follows the actual fsync before any post-write freshness check.
        // Semantic rejection here cannot release another nonce or signing invocation.
        match r {
            Record::Initialize(value) if self.rows == 1 && **value == self.initialize => {}
            Record::Reserve(request) => self.install_pending((**request).clone())?,
            Record::AccountInvoked(nonce) => {
                let pending = self.pending.as_mut().ok_or(Custody)?;
                if pending.invoked || pending.request.request_nonce != **nonce {
                    return Err(Custody);
                }
                pending.invoked = true;
            }
            Record::AccountSigned(value) => self.install_signature(value.0, value.1)?,
            Record::ResultOriginals(originals) => {
                self.validate_result_originals(originals)?;
                let pending = self.pending.as_mut().ok_or(Custody)?;
                if pending.originals.is_some() {
                    return Err(Custody);
                }
                pending.originals = Some((**originals).clone());
            }
            Record::Acknowledged(ack) => self.install_acknowledgement((**ack).clone())?,
            _ => return Err(Custody),
        }
        self.journal.check_owned().map_err(|_| Custody)?;
        if self.journal.recovery_prefix().map_err(|_| Custody)? != self.prefix.ok_or(Custody)? {
            return Err(Custody);
        }
        self.persistence_uncertain = false;
        self.recheck_journal()
    }
    fn acknowledged(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<&Acknowledged> {
        self.recheck_historical(financial)?;
        let value = self.acknowledged.get(&key).ok_or(Rejected)?;
        if originals_digest(&value.originals)? != value.acknowledgement.originals_sha256
            || value.result.subject.request != value.request
        {
            return Err(Rejected);
        }
        value
            .result
            .verify_for_request(&value.request, &self.policy.issuer_public_key)
            .map_err(|_| Rejected)?;
        self.verify_retained_finality(&value.result.subject, &value.originals.finality_original)?;
        financial.retained_cash_clock_originals(&value.acknowledgement.captured_clock)?;
        self.recheck_journal()?;
        Ok(value)
    }
    pub(crate) fn anchor_receipt(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryLineageAnchorV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryLineageAnchorReceiptV1<'_>> {
        if self.acknowledged(key, financial)?.request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::Anchor(Box::new(expected.clone()))
        {
            return Err(Rejected);
        }
        Ok(KagemushaAuthenticatedOrdinaryLineageAnchorReceiptV1 {
            owner: self,
            request_sha256: key,
        })
    }
    pub(crate) fn reservation_receipt(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryLineageReservationV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'_>> {
        if self.acknowledged(key, financial)?.request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::Reserve(Box::new(expected.clone()))
        {
            return Err(Rejected);
        }
        Ok(KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1 {
            owner: self,
            request_sha256: key,
        })
    }
    pub(crate) fn commit_receipt(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryLineageCommitV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1<'_>> {
        if self.acknowledged(key, financial)?.request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::Commit(Box::new(expected.clone()))
        {
            return Err(Rejected);
        }
        Ok(KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1 {
            owner: self,
            request_sha256: key,
        })
    }
}
macro_rules! receipt {
    ($t:ident, $variant:ident, $selector:ty, $getter:ident) => {
        impl $t<'_> {
            pub(crate) fn $getter(&self) -> Result<&$selector> {
                self.owner.recheck_journal()?;
                match &self
                    .owner
                    .acknowledged
                    .get(&self.request_sha256)
                    .ok_or(Rejected)?
                    .request
                    .operation
                {
                    KagemushaOrdinaryLineageRequestOperationV1::$variant(value) => Ok(value),
                    _ => Err(Rejected),
                }
            }
            pub(crate) fn original(&self) -> Result<&[u8]> {
                self.owner.recheck_journal()?;
                Ok(&self
                    .owner
                    .acknowledged
                    .get(&self.request_sha256)
                    .ok_or(Rejected)?
                    .originals
                    .signed_result)
            }
            pub(crate) fn recheck_historical(
                &self,
                financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
            ) -> Result<()> {
                self.owner.acknowledged(self.request_sha256, financial)?;
                self.$getter()?;
                Ok(())
            }
            pub(crate) fn recheck_for_effect(
                &self,
                financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
                current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
            ) -> Result<()> {
                self.recheck_historical(financial)?;
                self.owner.recheck_live(financial, current)?;
                self.owner.require_current_data(
                    current,
                    &self
                        .owner
                        .acknowledged
                        .get(&self.request_sha256)
                        .ok_or(Rejected)?
                        .result
                        .subject,
                )?;
                self.owner.recheck_live(financial, current)
            }
            pub(crate) fn request_original_sha256(&self) -> [u8; 32] {
                self.request_sha256
            }
        }
    };
}
receipt!(
    KagemushaAuthenticatedOrdinaryLineageAnchorReceiptV1,
    Anchor,
    KagemushaOrdinaryLineageAnchorV1,
    anchor
);
receipt!(
    KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1,
    Reserve,
    KagemushaOrdinaryLineageReservationV1,
    reservation
);
receipt!(
    KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1,
    Commit,
    KagemushaOrdinaryLineageCommitV1,
    commit
);
fn encode(r: &Record) -> Result<Vec<u8>> {
    let raw = norito::encode_canonical(r).map_err(|_| Rejected)?;
    if raw.is_empty() || raw.len() > MAX_RECORD {
        return Err(Rejected);
    }
    Ok(raw)
}
fn decode<T>(raw: &[u8], maximum: usize) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if raw.is_empty() || raw.len() > maximum {
        return Err(Rejected);
    }
    let v: T = norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(maximum))
        .map_err(|_| Rejected)?;
    if norito::encode_canonical(&v).map_err(|_| Rejected)? != raw {
        return Err(Rejected);
    }
    Ok(v)
}
fn originals_digest(originals: &ResultOriginals) -> Result<[u8; 32]> {
    let raw = norito::encode_canonical(originals).map_err(|_| Rejected)?;
    if raw.len() > MAX_RECORD {
        return Err(Rejected);
    }
    let mut h = Sha256::new();
    h.update(b"iroha:kagemusha:v1:ordinary-lineage-result-originals\0");
    h.update(
        u64::try_from(raw.len())
            .map_err(|_| Rejected)?
            .to_le_bytes(),
    );
    h.update(raw);
    Ok(h.finalize().into())
}
fn request_digest(request: &KagemushaOrdinaryLineageRequestV1) -> Result<[u8; 32]> {
    Ok(Sha256::digest(request.canonical_bytes().map_err(|_| Rejected)?).into())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{bind, selected};
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    fn financial(root: &Path) -> KagemushaOrdinaryEnrolledFinancialOwnerV1 {
        let fixture = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let originals = selected(&fixture, 300);
        let mut held =
            KagemushaOrdinaryPreparationReservationV1::create(root, originals, 300).unwrap();
        let fixture = bind(fixture, held.carrier().unwrap());
        held.retain_preparation(&fixture.selection.preparation.to_transport_bytes().unwrap())
            .unwrap();
        held.reference_ms = 600;
        held.reference_clock = Reading::now().unwrap();
        held.complete_enrollment(Arc::new(fixture.verify(600).unwrap()))
            .unwrap()
    }
    fn policy(financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1) -> Vec<u8> {
        let issuer = &financial.reservation.selected.issuer;
        KagemushaOrdinaryLineageIssuerPolicyV1 {
            version: 1,
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(issuer)
                .unwrap(),
            issuer_public_key: issuer.issuer_public_key.clone(),
            runtime: issuer.runtime.clone(),
            purpose_domain_digest: KagemushaOrdinaryLineageIssuerPolicyV1::purpose_domain_digest(),
            enabled: true,
        }
        .canonical_bytes()
        .unwrap()
    }
    // Codec/private journal fixture only. No actual mathematical admission or DATA receipt is
    // fabricated: production reserve accepts a separate closed real proof type.
    fn request(owner: &KagemushaOrdinaryLineageCasOwnerV1) -> KagemushaOrdinaryLineageRequestV1 {
        KagemushaOrdinaryLineageRequestV1 {
            version: 1,
            request_nonce: [31; 32],
            issuer_policy_digest: owner.policy.issuer_policy_digest,
            operation: KagemushaOrdinaryLineageRequestOperationV1::Anchor(Box::new(
                KagemushaOrdinaryLineageAnchorV1 {
                    lineage: owner.initialize.lineage.clone(),
                    initial_head: KagemushaOrdinaryFinancialHeadV1 {
                        state_commitment: [32; 32],
                        logical_sequence: 0,
                        state_original_sha256: [33; 32],
                    },
                    proof_bundle_original_sha256: [34; 32],
                },
            )),
        }
    }
    #[test]
    fn lineage_private_reopen_requires_exact_enabled_installed_purpose_original() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        drop(owner);
        let owner = KagemushaOrdinaryLineageCasOwnerV1::open_existing(&root, &financial, &original)
            .unwrap();
        assert!(owner.acknowledged.is_empty());
        drop(owner);
        let mut changed: KagemushaOrdinaryLineageIssuerPolicyV1 =
            decode(&original, 32 * 1024).unwrap();
        changed.enabled = false;
        assert!(
            KagemushaOrdinaryLineageCasOwnerV1::open_existing(
                &root,
                &financial,
                &changed.canonical_bytes().unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn lineage_unacknowledged_request_replays_without_receipt_or_current_grant() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let mut owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        let request = request(&owner);
        let key = request_digest(&request).unwrap();
        owner
            .append(&Record::Reserve(Box::new(request.clone())))
            .unwrap();
        owner
            .append(&Record::AccountInvoked(Box::new(request.request_nonce)))
            .unwrap();
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let signature: [u8; 64] = Signature::try_new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap()
        .payload()
        .try_into()
        .unwrap();
        owner
            .append(&Record::AccountSigned(Box::new((
                request.request_nonce,
                signature,
            ))))
            .unwrap();
        drop(owner);
        let owner = KagemushaOrdinaryLineageCasOwnerV1::open_existing(&root, &financial, &original)
            .unwrap();
        assert_eq!(owner.pending.as_ref().unwrap().request, request);
        assert_eq!(owner.pending.as_ref().unwrap().signature, Some(signature));
        assert!(owner.acknowledged(key, &financial).is_err());
        let KagemushaOrdinaryLineageRequestOperationV1::Anchor(anchor) = &request.operation else {
            panic!()
        };
        assert!(owner.anchor_receipt(key, &financial, anchor).is_err());
        assert!(
            owner.actual_clock().is_err(),
            "synthetic fixture scalar never becomes shipping Native time"
        );
    }
    #[test]
    fn lineage_original_pending_cannot_be_acknowledged_without_real_signed_result_and_finality() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let mut owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        let request = request(&owner);
        owner.install_pending(request).unwrap();
        let originals = ResultOriginals {
            signed_result: vec![1; 64],
            data_record: vec![2; 32],
            finality_original: vec![3; 32],
        };
        assert!(owner.validate_result_originals(&originals).is_err());
        owner.pending.as_mut().unwrap().originals = Some(originals);
        let ack = Acknowledgement {
            originals_sha256: [4; 32],
            captured_clock: KagemushaOrdinaryCashClockContextV1 {
                version: 1,
                request_nonce: [5; 32],
                signed_observations_original_digest: [6; 32],
                lower_at_ms: 600,
                upper_at_ms: 600,
            },
        };
        assert!(owner.install_acknowledgement(ack).is_err());
        assert!(owner.acknowledged.is_empty());
    }
    #[test]
    fn lineage_durable_memory_failure_freezes_until_actual_cold_recovery() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let mut owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        let request = request(&owner);
        owner
            .append(&Record::Reserve(Box::new(request.clone())))
            .unwrap();
        assert_eq!(owner.pending.as_ref().unwrap().request, request);
        assert!(!owner.persistence_uncertain);
        // Model the exact post-fsync/pre-memory boundary: the held WAL receives a genuine
        // invoked row while the higher holder retains the old prefix/memory. No fake receipt.
        owner
            .journal
            .append(&encode(&Record::AccountInvoked(Box::new(request.request_nonce))).unwrap())
            .unwrap();
        owner.persistence_uncertain = true;
        assert!(owner.recheck_journal().is_err());
        assert!(
            owner
                .append(&Record::AccountInvoked(Box::new(request.request_nonce)))
                .is_err()
        );
        assert!(!owner.pending.as_ref().unwrap().invoked);
        drop(owner);
        let owner = KagemushaOrdinaryLineageCasOwnerV1::open_existing(&root, &financial, &original)
            .unwrap();
        assert!(owner.pending.as_ref().unwrap().invoked);
        assert!(!owner.persistence_uncertain);
        assert!(owner.acknowledged.is_empty());
    }
    #[test]
    fn lineage_post_fsync_semantic_rejection_permanently_closes_holder() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let mut owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        let request = request(&owner);
        owner
            .append(&Record::Reserve(Box::new(request.clone())))
            .unwrap();
        // An impossible duplicate is not a new request authority even when its write reaches
        // durable storage. The holder refuses every later operation, and strict replay rejects.
        assert!(
            owner
                .append(&Record::Reserve(Box::new(request.clone())))
                .is_err()
        );
        assert!(owner.persistence_uncertain);
        assert!(owner.recheck_journal().is_err());
        assert!(
            owner
                .append(&Record::AccountInvoked(Box::new(request.request_nonce)))
                .is_err()
        );
        drop(owner);
        assert!(
            KagemushaOrdinaryLineageCasOwnerV1::open_existing(&root, &financial, &original)
                .is_err()
        );
    }
    #[test]
    fn lineage_retained_account_signature_binds_complete_request_and_one_invocation() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let financial = financial(&root);
        let original = policy(&financial);
        let mut owner =
            KagemushaOrdinaryLineageCasOwnerV1::create(&root, &financial, &original).unwrap();
        let request = request(&owner);
        owner.install_pending(request.clone()).unwrap();
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let signature: [u8; 64] = Signature::try_new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap()
        .payload()
        .try_into()
        .unwrap();
        assert!(
            owner
                .install_signature(request.request_nonce, signature)
                .is_err()
        );
        owner.pending.as_mut().unwrap().invoked = true;
        let mut foreign_nonce = request.request_nonce;
        foreign_nonce[0] ^= 1;
        assert!(owner.install_signature(foreign_nonce, signature).is_err());
        owner
            .install_signature(request.request_nonce, signature)
            .unwrap();
        assert!(
            owner
                .install_signature(request.request_nonce, signature)
                .is_err()
        );
        let encoded = encode(&Record::Reserve(Box::new(request))).unwrap();
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode::<Record>(&trailing, MAX_RECORD).is_err());
        assert!(decode::<Record>(&encoded[..encoded.len() - 1], MAX_RECORD).is_err());
    }
}
