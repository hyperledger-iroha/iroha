//! Same-financial-owner current FI control with a private replay/floor journal.
//! Recovered originals retain history only; fresh Native reads alone lend current authority.
use super::*;
use iroha_primitives::time::NativeContinuousReading;
use iroha_torii_shared::kagemusha_state::{
    KagemushaAuthorityStateV1, decode_unverified_kagemusha_authority_state_v1,
};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
const CONTROL_FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-current-control.norito.wal",
    magic: b"KGMCCTL1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-current-control-journal\0",
    maximum_payload_bytes: 64 * 1024,
};
const MAX_ROWS: u64 = 1_000_000;
const MAX_HISTORICAL_CAPTURES: usize = 4096;
const MAX_HISTORICAL_ORIGINAL_BYTES: usize = 64 * 1024 * 1024;
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::CurrentControlRecordV1")]
enum ControlRecord {
    Initialize {
        enrollment_sha256: [u8; 32],
        credential_sha256: [u8; 32],
        issuer_policy_digest: [u8; 32],
    },
    Reserve(Box<KagemushaOrdinaryCurrentControlRequestV1>),
    AccountInvoked {
        nonce: [u8; 32],
    },
    AccountSigned {
        nonce: [u8; 32],
        signature: [u8; 64],
    },
    Accepted {
        nonce: [u8; 32],
        original: Vec<u8>,
        captured_lower_ms: u64,
        captured_upper_ms: u64,
    },
    ProofCapturePending {
        original_sha256: [u8; 32],
    },
    ProofCaptureAcknowledged {
        original_sha256: [u8; 32],
        lower_ms: u64,
        upper_ms: u64,
    },
}
struct PendingRead {
    request: KagemushaOrdinaryCurrentControlRequestV1,
    started: NativeContinuousReading,
    invoked: bool,
    signature: Option<[u8; 64]>,
}
#[derive(Clone)]
struct AdmittedControl {
    signed: KagemushaSignedOrdinaryCurrentControlV1,
    original: Vec<u8>,
    lower_ms: u64,
    upper_ms: u64,
}
struct CapturedDecision {
    admitted: AdmittedControl,
    lower_ms: u64,
    upper_ms: u64,
}
#[derive(Default)]
struct Floor {
    incarnation: Option<[u8; 32]>,
    revision: u64,
    policy_epoch: u64,
    schema_epoch: u64,
    issued_at_ms: u64,
}
/// Actual private current-control owner for one retained Native financial enrollment/selection.
/// No decoder, copied timestamp, callback, public key or offer can construct this custody.
pub struct KagemushaOrdinaryCurrentFinancialControlOwnerV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    journal: PrivateJournal,
    prefix: Option<crate::kagemusha_v1_state::KagemushaRecoveryJournalPrefixV1>,
    rows: u64,
    used: BTreeSet<[u8; 32]>,
    floor: Floor,
    pending: Option<PendingRead>,
    current: Option<AdmittedControl>,
    proof_capture_pending: Option<[u8; 32]>,
    proof_capture: Option<([u8; 32], u64, u64)>,
    historical: BTreeMap<[u8; 32], CapturedDecision>,
    historical_original_bytes: usize,
    // Process-local proof custody only. Cold replay always leaves this empty; a fresh live
    // control loan must acknowledge actual retained history before private proving resumes.
    historical_ready: BTreeSet<[u8; 32]>,
}
/// Live current-control loan borrowing this exact actual owner and financial owner.
/// It is never deserialized, copied, converted from Bootstrap or reconstructed from a receipt.
pub struct KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'a> {
    owner: &'a KagemushaOrdinaryCurrentFinancialControlOwnerV1,
    financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
}
/// Private immutable historical proof decision; its original may expire during slow proving.
/// This supplies no live money grant or clock, and exposure must obtain another fresh live loan.
pub(crate) struct KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a> {
    owner: &'a KagemushaOrdinaryCurrentFinancialControlOwnerV1,
    financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    original_sha256: [u8; 32],
    lower_ms: u64,
    upper_ms: u64,
}
impl KagemushaOrdinaryCurrentFinancialControlOwnerV1 {
    /// Create/fsync current-control custody from the actual completed financial owner.
    /// # Errors
    /// Refuses unsafe storage, another original enrollment/clock or expired original admission.
    pub fn create(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        financial.recheck()?;
        let journal =
            PrivateJournal::create_new(&root.join("ordinary-current-control"), CONTROL_FORMAT)
                .map_err(|_| Custody)?;
        let mut this = Self::new(journal, financial)?;
        let r = this.initialize_record()?;
        this.append(&r)?;
        this.require_same_financial(financial)?;
        Ok(this)
    }
    /// Recover only authentic original history and monotonic floors. A fresh actual request is
    /// mandatory before any current loan; a recovered live-looking timestamp is never lent.
    /// # Errors
    /// Refuses missing storage, replayed/noncanonical/substituted rows, invalid issuer signature
    /// or regressed DATA/history bounds. An interrupted old read is abandoned, not renewed.
    pub fn open_existing(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        financial.reservation.recheck_originals()?;
        let journal =
            PrivateJournal::open_existing(&root.join("ordinary-current-control"), CONTROL_FORMAT)
                .map_err(|_| Custody)?;
        let mut this = Self::new(journal, financial)?;
        let mut reserved = None;
        let mut last = None;
        while let Some((_, raw)) = this.journal.replay_next().map_err(|_| Custody)? {
            if this.rows >= MAX_ROWS {
                return Err(Rejected);
            }
            let r: ControlRecord = decode(&raw)?;
            match r {
                ControlRecord::Initialize {
                    enrollment_sha256,
                    credential_sha256,
                    issuer_policy_digest,
                } if this.rows == 0 => {
                    let ControlRecord::Initialize {
                        enrollment_sha256: e,
                        credential_sha256: c,
                        issuer_policy_digest: i,
                    } = this.initialize_record()?
                    else {
                        return Err(Rejected);
                    };
                    if e != enrollment_sha256 || c != credential_sha256 || i != issuer_policy_digest
                    {
                        return Err(Rejected);
                    }
                }
                ControlRecord::Reserve(request) if this.rows > 0 => {
                    this.require_request(&request)?;
                    if !this.used.insert(request.request_nonce) {
                        return Err(Rejected);
                    }
                    reserved = Some((*request, false, None));
                    last = None;
                    this.proof_capture = None;
                    this.proof_capture_pending = None;
                }
                ControlRecord::AccountInvoked { nonce } if this.rows > 0 => {
                    let (request, invoked, signature) = reserved.as_mut().ok_or(Rejected)?;
                    if request.request_nonce != nonce || *invoked || signature.is_some() {
                        return Err(Rejected);
                    }
                    *invoked = true;
                }
                ControlRecord::AccountSigned { nonce, signature } if this.rows > 0 => {
                    let (request, invoked, retained) = reserved.as_mut().ok_or(Rejected)?;
                    if request.request_nonce != nonce || !*invoked || retained.is_some() {
                        return Err(Rejected);
                    }
                    request
                        .verify_account_signature(&iroha_crypto::Signature::from_bytes(&signature))
                        .map_err(|_| Rejected)?;
                    *retained = Some(signature);
                }
                ControlRecord::Accepted {
                    nonce,
                    original,
                    captured_lower_ms,
                    captured_upper_ms,
                } if this.rows > 0 => {
                    let (request, invoked, signature) = reserved.take().ok_or(Rejected)?;
                    if !invoked || signature.is_none() {
                        return Err(Rejected);
                    }
                    let signed = decode_signed(&original)?;
                    signed
                        .verify_for_request(&request, &this.selected.issuer)
                        .map_err(|_| Rejected)?;
                    if nonce != request.request_nonce
                        || captured_lower_ms > captured_upper_ms
                        || captured_lower_ms < signed.subject.issued_at_ms
                        || captured_upper_ms >= signed.subject.expires_at_ms
                    {
                        return Err(Rejected);
                    }
                    this.require_static_scope(&signed.subject)?;
                    this.advance_floor(&signed.subject)?;
                    last = Some((
                        <[u8; 32]>::from(Sha256::digest(&original)),
                        signed,
                        original,
                        captured_lower_ms,
                        captured_upper_ms,
                    ));
                    this.proof_capture = None;
                    this.proof_capture_pending = None;
                }
                ControlRecord::ProofCapturePending { original_sha256 } if this.rows > 0 => {
                    let (digest, _, _, _, _) = last.as_ref().ok_or(Rejected)?;
                    if *digest != original_sha256
                        || this.proof_capture_pending.is_some()
                        || this.proof_capture.is_some()
                    {
                        return Err(Rejected);
                    }
                    this.proof_capture_pending = Some(original_sha256);
                }
                ControlRecord::ProofCaptureAcknowledged {
                    original_sha256,
                    lower_ms,
                    upper_ms,
                } if this.rows > 0 => {
                    let (digest, signed, original, admitted_lower, admitted_upper) =
                        last.as_ref().ok_or(Rejected)?;
                    if *digest != original_sha256
                        || this.proof_capture_pending != Some(original_sha256)
                        || this.proof_capture.is_some()
                        || lower_ms < *admitted_lower
                        || upper_ms < *admitted_upper
                    {
                        return Err(Rejected);
                    }
                    require_captured_interval(&signed.subject, lower_ms, upper_ms)?;
                    this.proof_capture = Some((original_sha256, lower_ms, upper_ms));
                    this.retain_historical_decision(
                        original_sha256,
                        CapturedDecision {
                            admitted: AdmittedControl {
                                signed: signed.clone(),
                                original: original.clone(),
                                lower_ms: *admitted_lower,
                                upper_ms: *admitted_upper,
                            },
                            lower_ms,
                            upper_ms,
                        },
                    )?;
                }
                _ => return Err(Rejected),
            }
            this.rows += 1;
        }
        if this.rows == 0 {
            return Err(Rejected);
        }
        this.prefix = Some(this.journal.recovery_prefix().map_err(|_| Custody)?);
        this.pending = None;
        this.current = None;
        this.proof_capture = None;
        this.proof_capture_pending = None;
        this.historical_ready.clear();
        this.require_same_financial(financial)?;
        Ok(this)
    }
    fn new(
        journal: PrivateJournal,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        financial.reservation.recheck_originals()?;
        financial
            .reservation
            .require_enrollment(&financial.enrollment)?;
        Ok(Self {
            selected: financial.reservation.selected.clone(),
            enrollment: financial.enrollment.clone(),
            journal,
            prefix: None,
            rows: 0,
            used: BTreeSet::new(),
            floor: Floor::default(),
            pending: None,
            current: None,
            proof_capture_pending: None,
            proof_capture: None,
            historical: BTreeMap::new(),
            historical_original_bytes: 0,
            historical_ready: BTreeSet::new(),
        })
    }
    fn initialize_record(&self) -> Result<ControlRecord> {
        Ok(ControlRecord::Initialize {
            enrollment_sha256: Sha256::digest(
                self.enrollment
                    .certificate()
                    .canonical_bytes()
                    .map_err(|_| Rejected)?,
            )
            .into(),
            credential_sha256: Sha256::digest(self.enrollment.app_credential().original()).into(),
            issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
                &self.selected.issuer,
            )
            .map_err(|_| Rejected)?,
        })
    }
    fn request(&self, nonce: [u8; 32]) -> Result<KagemushaOrdinaryCurrentControlRequestV1> {
        let ControlRecord::Initialize {
            enrollment_sha256,
            credential_sha256,
            issuer_policy_digest,
        } = self.initialize_record()?
        else {
            return Err(Rejected);
        };
        Ok(KagemushaOrdinaryCurrentControlRequestV1 {
            version: 1,
            request_nonce: nonce,
            owner: self.selected.owner.clone(),
            enrollment_original_sha256: enrollment_sha256,
            credential_original_sha256: credential_sha256,
            issuer_policy_digest,
        })
    }
    fn require_request(&self, request: &KagemushaOrdinaryCurrentControlRequestV1) -> Result<()> {
        request.validate_shape().map_err(|_| Rejected)?;
        if *request != self.request(request.request_nonce)? {
            return Err(Rejected);
        }
        Ok(())
    }
    fn require_same_financial(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        financial.reservation.recheck_originals()?;
        self.require_same_historical_financial(financial)
    }
    fn require_same_historical_financial(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.recheck_journal()?;
        financial.recheck_historical_proof_custody()?;
        if !Arc::ptr_eq(&self.enrollment, &financial.enrollment)
            || !Arc::ptr_eq(&self.selected, &financial.reservation.selected)
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Reserve/fsync fresh Native entropy and the full same-owner request before transport.
    /// Returned fields are canonical request and exact Native account signing message only.
    /// A later read abandons uncertainty and uses a new nonce; no previous control is renewed.
    /// # Errors
    /// Refuses unknown durability, foreign financial custody or unavailable genuine current clock.
    pub fn prepare_current_read(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.require_same_financial(financial)?;
        financial.recheck()?;
        let started = NativeContinuousReading::now().map_err(|_| Custody)?;
        let mut nonce = [0; 32];
        OsRng.try_fill_bytes(&mut nonce).map_err(|_| Custody)?;
        if nonce == [0; 32] || self.used.contains(&nonce) {
            return Err(Rejected);
        }
        let request = self.request(nonce)?;
        let raw = request.canonical_bytes().map_err(|_| Rejected)?;
        let message = request.account_signing_message().map_err(|_| Rejected)?;
        self.append(&ControlRecord::Reserve(Box::new(request.clone())))?;
        self.used.insert(nonce);
        self.current = None;
        self.proof_capture = None;
        self.proof_capture_pending = None;
        self.pending = Some(PendingRead {
            request,
            started,
            invoked: false,
            signature: None,
        });
        self.require_budget()?;
        self.require_same_financial(financial)?;
        financial.recheck()?;
        Ok(vec![raw, message])
    }
    /// Borrow the sole Native-prepared account signing subject under the same financial custody.
    /// # Errors
    /// Refuses absent/expired request, current financial mismatch or unknown journal custody.
    pub fn pending_account_request<'a>(
        &'a self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<&'a KagemushaOrdinaryCurrentControlRequestV1> {
        self.require_same_financial(financial)?;
        financial.recheck()?;
        self.require_budget()?;
        Ok(&self.pending.as_ref().ok_or(Rejected)?.request)
    }
    /// Fsync a same-account signature invocation fence, or return its exact retained original.
    /// # Errors
    /// Refuses an unknown prior invocation; retry never invokes the account again for that nonce.
    pub fn fence_account_request(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Option<[u8; 64]>> {
        self.pending_account_request(financial)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if let Some(signature) = pending.signature {
            return Ok(Some(signature));
        }
        if pending.invoked {
            return Err(KagemushaOrdinaryIdentityErrorV1::UnknownOutcome);
        }
        let nonce = pending.request.request_nonce;
        self.append(&ControlRecord::AccountInvoked { nonce })?;
        self.pending.as_mut().ok_or(Rejected)?.invoked = true;
        self.pending_account_request(financial)?;
        Ok(None)
    }
    /// Verify/fsync Ed64 over only the same actual pending account subject before transport.
    /// # Errors
    /// Refuses a missing fence, foreign signer/subject, changed original or failed durability.
    pub fn retain_account_request_original(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        signature: [u8; 64],
    ) -> Result<()> {
        let request = self.pending_account_request(financial)?;
        request
            .verify_account_signature(&iroha_crypto::Signature::from_bytes(&signature))
            .map_err(|_| Rejected)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if !pending.invoked {
            return Err(Rejected);
        }
        if let Some(original) = pending.signature {
            if original != signature {
                return Err(Rejected);
            }
            return self.pending_account_request(financial).map(|_| ());
        }
        let nonce = pending.request.request_nonce;
        self.append(&ControlRecord::AccountSigned { nonce, signature })?;
        self.pending.as_mut().ok_or(Rejected)?.signature = Some(signature);
        self.pending_account_request(financial).map(|_| ())
    }
    /// Project the exact full request and its durably retained Native account Ed64 for FI transport.
    /// These public originals create no FI/current/financial authority.
    /// # Errors
    /// Refuses missing original signature or stale actual Native custody.
    pub fn retained_current_read_fields(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Vec<u8>>> {
        let request = self.pending_account_request(financial)?;
        let signature = self
            .pending
            .as_ref()
            .ok_or(Rejected)?
            .signature
            .ok_or(Rejected)?;
        Ok(vec![
            request.canonical_bytes().map_err(|_| Rejected)?,
            signature.to_vec(),
        ])
    }
    /// Authenticate the exact signed reply and complete current authority originals, then fsync
    /// the small signed original before any loan. Post-fsync time/custody is freshly checked.
    /// # Errors
    /// Refuses substituted roots/policies/nonce/FI/C/PI, stale actual interval, replay/regression,
    /// unsafe durability or a reply beyond the original Native suspend-inclusive 10-second budget.
    pub fn accept_current_read(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        signed_original: &[u8],
        authority_original: &[u8],
    ) -> Result<()> {
        self.require_same_financial(financial)?;
        self.require_budget()?;
        let signed = decode_signed(signed_original)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if !pending.invoked || pending.signature.is_none() {
            return Err(Rejected);
        }
        let request = &pending.request;
        signed
            .verify_for_request(request, &self.selected.issuer)
            .map_err(|_| Rejected)?;
        self.require_static_scope(&signed.subject)?;
        let authority = decode_unverified_kagemusha_authority_state_v1(authority_original)
            .map_err(|_| Rejected)?;
        self.verify_current_world(&signed.subject, &authority)?;
        let interval = self.verify_live_subject(financial, &signed.subject)?;
        self.require_floor(&signed.subject)?;
        self.require_budget()?;
        // Consuming the read before append makes uncertain durability closed. No second
        // Accepted row can be written for the same reserved nonce after a failed fsync.
        let pending = self.pending.take().ok_or(Rejected)?;
        self.append(&ControlRecord::Accepted {
            nonce: pending.request.request_nonce,
            original: signed_original.to_vec(),
            captured_lower_ms: interval.lower_ms(),
            captured_upper_ms: interval.upper_ms(),
        })?;
        self.advance_floor(&signed.subject)?;
        // Accepted records retain authenticated history, but are not yet a lendable grant.
        // A slow fsync, clock projection or ownership check cannot leave a partially usable
        // current entry. All checks retain the original request's elapsed-time budget.
        require_read_budget(pending.started)?;
        self.verify_live_subject(financial, &signed.subject)?;
        self.require_same_financial(financial)?;
        require_read_budget(pending.started)?;
        self.current = Some(AdmittedControl {
            signed,
            original: signed_original.to_vec(),
            lower_ms: interval.lower_ms(),
            upper_ms: interval.upper_ms(),
        });
        if let Err(error) = self.loan(financial).map(|_| ()) {
            self.current = None;
            return Err(error);
        }
        Ok(())
    }
    /// Borrow the only genuine current control/financial pair. Expiry never renews an original.
    /// # Errors
    /// Refuses missing fresh admission or any actual current clock/owner/PI/revocation mismatch.
    pub fn loan<'a>(
        &'a self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'a>> {
        let loan = KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1 {
            owner: self,
            financial,
        };
        loan.recheck()?;
        Ok(loan)
    }
    /// Fsync original intent, sample actual current validity, then fsync its distinct historical
    /// capture acknowledgment. Pending intent alone never supplies proof custody.
    /// The returned private evidence can support slow proof work only; money exposure must
    /// obtain a separately fresh current loan after proving. Capture never extends expiry.
    /// # Errors
    /// Refuses stale admission, changed financial custody or failed original capture durability.
    pub(crate) fn capture_proof_decision<'a>(
        &'a mut self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a>> {
        self.loan(financial)?.recheck()?;
        if self.proof_capture.is_some() || self.proof_capture_pending.is_some() {
            return Err(Rejected);
        }
        let digest = Sha256::digest(&self.current.as_ref().ok_or(Rejected)?.original).into();
        self.require_historical_capacity(
            digest,
            self.current.as_ref().ok_or(Rejected)?.original.len(),
        )?;
        // Raw intent is durably retained first. It is not a captured proof decision.
        self.append(&ControlRecord::ProofCapturePending {
            original_sha256: digest,
        })?;
        self.proof_capture_pending = Some(digest);
        // Sample only after original-intent fsync. Expired control, slow persistence or changed
        // current owner leaves Pending alone; cold replay cannot promote it to proof custody.
        let c = self.current.as_ref().ok_or(Rejected)?;
        let interval = self.verify_live_subject(financial, &c.signed.subject)?;
        let lower = interval.lower_ms();
        let upper = interval.upper_ms();
        if lower < c.lower_ms || upper < c.upper_ms {
            return Err(Rejected);
        }
        require_captured_interval(&c.signed.subject, lower, upper)?;
        self.require_same_financial(financial)?;
        self.append(&ControlRecord::ProofCaptureAcknowledged {
            original_sha256: digest,
            lower_ms: lower,
            upper_ms: upper,
        })?;
        // Acknowledgment persistence may finish later. The immutable post-original-fsync
        // decision supplies historical proving custody only, never a renewed live grant.
        self.proof_capture = Some((digest, lower, upper));
        self.retain_historical_decision(
            digest,
            CapturedDecision {
                admitted: self.current.as_ref().ok_or(Rejected)?.clone(),
                lower_ms: lower,
                upper_ms: upper,
            },
        )?;
        self.require_same_historical_financial(financial)?;
        self.historical_ready.insert(digest);
        self.borrow_captured_proof_decision(financial, digest, lower, upper)
    }
    /// Resume the bounded complete authentic acknowledged history after one fresh live FI
    /// loan before and after all original/custody checks. Cold replay never marks history ready.
    /// Captured originals/times remain immutable; later money effects need another live loan.
    pub(crate) fn resume_all_retained_proof_decisions(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.loan(financial)?.recheck()?;
        let actual: Vec<_> = self.historical.keys().copied().collect();
        for digest in &actual {
            self.retained_historical_decision(financial, *digest)?
                .recheck_historical_originals()?;
        }
        self.loan(financial)?.recheck()?;
        self.historical_ready.extend(actual);
        self.require_same_historical_financial(financial)
    }
    /// Check only the privately fsynced capture selected by a Main Native attempt record.
    /// This returns no loan, sets no ready marker and lends no live grant or clock.
    pub(crate) fn recheck_retained_capture_identity(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original_sha256: [u8; 32],
        lower_ms: u64,
        upper_ms: u64,
    ) -> Result<()> {
        let actual = self.retained_historical_decision(financial, original_sha256)?;
        if actual.lower_ms != lower_ms || actual.upper_ms != upper_ms {
            return Err(Rejected);
        }
        actual.recheck_historical_originals()
    }
    /// Read the immutable validity window of the same privately acknowledged FI original.
    /// Cold identity checks neither mark history ready nor supply a fresh financial grant.
    pub(crate) fn recheck_retained_capture_original_window(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original_sha256: [u8; 32],
        lower_ms: u64,
        upper_ms: u64,
    ) -> Result<(u64, u64)> {
        self.recheck_retained_capture_identity(financial, original_sha256, lower_ms, upper_ms)?;
        let subject = &self
            .historical
            .get(&original_sha256)
            .ok_or(Rejected)?
            .admitted
            .signed
            .subject;
        let window = (subject.issued_at_ms, subject.expires_at_ms);
        self.recheck_retained_capture_identity(financial, original_sha256, lower_ms, upper_ms)?;
        Ok(window)
    }
    /// Borrow a same-owner actual acknowledged decision selected only by the immutable Main
    /// Native attempt original/bounds. Successful capture or explicit fresh cold resumption must
    /// first mark that exact original ready. No expiring live grant is held during slow proving.
    pub(crate) fn borrow_captured_proof_decision<'a>(
        &'a self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original_sha256: [u8; 32],
        lower_ms: u64,
        upper_ms: u64,
    ) -> Result<KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a>> {
        if !self.historical_ready.contains(&original_sha256) {
            return Err(Rejected);
        }
        self.recheck_retained_capture_identity(financial, original_sha256, lower_ms, upper_ms)?;
        self.retained_historical_decision(financial, original_sha256)
    }
    fn retained_historical_decision<'a>(
        &'a self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original_sha256: [u8; 32],
    ) -> Result<KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a>> {
        self.require_same_historical_financial(financial)?;
        let historical = self.historical.get(&original_sha256).ok_or(Rejected)?;
        let loan = KagemushaCapturedOrdinaryFinancialControlDecisionV1 {
            owner: self,
            financial,
            original_sha256,
            lower_ms: historical.lower_ms,
            upper_ms: historical.upper_ms,
        };
        loan.recheck_historical_originals()?;
        Ok(loan)
    }
    fn require_historical_capacity(&self, digest: [u8; 32], original_len: usize) -> Result<()> {
        if digest == [0; 32] || self.historical.contains_key(&digest) {
            return Err(Rejected);
        }
        validate_historical_capacity(
            self.historical.len(),
            self.historical_original_bytes,
            original_len,
        )
    }
    fn retain_historical_decision(
        &mut self,
        digest: [u8; 32],
        decision: CapturedDecision,
    ) -> Result<()> {
        self.require_historical_capacity(digest, decision.admitted.original.len())?;
        if <[u8; 32]>::from(Sha256::digest(&decision.admitted.original)) != digest {
            return Err(Rejected);
        }
        require_captured_interval(
            &decision.admitted.signed.subject,
            decision.lower_ms,
            decision.upper_ms,
        )?;
        self.historical_original_bytes = self
            .historical_original_bytes
            .checked_add(decision.admitted.original.len())
            .ok_or(Rejected)?;
        self.historical.insert(digest, decision);
        Ok(())
    }
    fn require_static_scope(&self, s: &KagemushaOrdinaryCurrentControlSubjectV1) -> Result<()> {
        self.require_request(&s.request)?;
        let release = self.selected.governed.release();
        let profile = self.selected.governed.profile_id();
        let enabled = release.enabled_profile(profile).ok_or(Rejected)?;
        if s.release_id != release.release_id()
            || s.hardware_profile_id != profile
            || s.profile_policy_epoch != enabled.policy_epoch
            || s.ordinary_trust_policy_digest
                != self
                    .selected
                    .governed
                    .trust()
                    .canonical_digest()
                    .map_err(|_| Rejected)?
            || s.app_authority_policy_digest
                != self
                    .selected
                    .governed
                    .authority()
                    .canonical_digest()
                    .map_err(|_| Rejected)?
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn verify_live_subject(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        s: &KagemushaOrdinaryCurrentControlSubjectV1,
    ) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        self.require_same_financial(financial)?;
        financial.recheck()?;
        let interval = financial.trusted_time_interval()?;
        interval
            .require_validity(s.issued_at_ms, s.expires_at_ms)
            .map_err(|_| Rejected)?;
        match (
            &s.latest_integrity_lease_original,
            financial.retained_integrity_lease(),
        ) {
            (Some(raw), Some(lease)) if raw.as_slice() == lease.original() => {
                interval
                    .check_both(|point| {
                        financial
                            .enrollment
                            .recheck_with_integrity_lease(lease, point)
                    })
                    .map_err(|_| Rejected)?;
            }
            (None, None) => {
                // FinancialOwner.recheck independently enforces the original baseline PI
                // at both actual clock bounds when PI is governed. None is not an exemption.
                interval
                    .check_both(|point| financial.enrollment.recheck_at_trusted_time(point))
                    .map_err(|_| Rejected)?;
            }
            _ => return Err(Rejected),
        }
        let clock = self.actual_clock()?;
        let mut clock = clock.lock().map_err(|_| Custody)?;
        clock.current_finality_verifier().map_err(|_| Custody)?;
        if clock.current_certified_height().map_err(|_| Custody)? != s.authority_height {
            return Err(Rejected);
        }
        // Intake already authenticated this exact context under this same retained verifier.
        // The immutable prefix has a unique decision at its height; Selected retains the same
        // actual clock/root and cannot swap it. Any advance retires this live control.
        Ok(interval)
    }
    fn actual_clock(&self) -> Result<&Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>> {
        match &self.selected.clock {
            SelectedClock::Native(clock) => Ok(clock),
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => Err(Rejected),
        }
    }
    fn verify_current_world(
        &self,
        s: &KagemushaOrdinaryCurrentControlSubjectV1,
        a: &KagemushaAuthorityStateV1,
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
            || block.context_id() != s.authority_context_id
            || block.execution().world_state_root != s.world_root
            || a.world_snapshot.schema_hash != s.world_schema_hash
            || self.selected.world_schema_hash != Some(s.world_schema_hash)
            || s.asset_definition_original_sha256
                != kagemusha_ordinary_current_control_original_sha256_v1(&a.asset_definition)
                    .map_err(|_| Rejected)?
            || s.verifier_registry_original_sha256
                != kagemusha_ordinary_current_control_original_sha256_v1(&a.verifier_registry)
                    .map_err(|_| Rejected)?
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
    fn require_floor(&self, s: &KagemushaOrdinaryCurrentControlSubjectV1) -> Result<()> {
        if self
            .floor
            .incarnation
            .is_some_and(|i| i != s.data_incarnation_digest)
            || s.data_revision < self.floor.revision
            || s.data_policy_epoch < self.floor.policy_epoch
            || s.data_schema_epoch < self.floor.schema_epoch
            || s.issued_at_ms < self.floor.issued_at_ms
        {
            return Err(Rejected);
        }
        Ok(())
    }
    fn advance_floor(&mut self, s: &KagemushaOrdinaryCurrentControlSubjectV1) -> Result<()> {
        self.require_floor(s)?;
        self.floor = Floor {
            incarnation: Some(s.data_incarnation_digest),
            revision: s.data_revision,
            policy_epoch: s.data_policy_epoch,
            schema_epoch: s.data_schema_epoch,
            issued_at_ms: s.issued_at_ms,
        };
        Ok(())
    }
    fn require_budget(&self) -> Result<()> {
        require_read_budget(self.pending.as_ref().ok_or(Rejected)?.started)
    }
    fn recheck_journal(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        if self
            .prefix
            .is_some_and(|p| self.journal.recovery_prefix().ok() != Some(p))
        {
            return Err(Custody);
        }
        Ok(())
    }
    fn append(&mut self, r: &ControlRecord) -> Result<()> {
        self.recheck_journal()?;
        if self.rows >= MAX_ROWS {
            return Err(Rejected);
        }
        self.journal
            .append(&norito::encode_canonical(r).map_err(|_| Rejected)?)
            .map_err(|_| Custody)?;
        self.rows += 1;
        self.prefix = Some(self.journal.recovery_prefix().map_err(|_| Custody)?);
        self.recheck_journal()
    }
}
impl KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_> {
    /// Exact full issuer-signed control original after another genuine live recheck.
    /// # Errors
    /// Refuses the same closed current-custody failures as `recheck`.
    pub fn original(&self) -> Result<&[u8]> {
        self.recheck()?;
        Ok(&self.owner.current.as_ref().ok_or(Rejected)?.original)
    }
    /// Recheck actual current FI/C/PI, signed interval and same clock-certified revocation cut.
    /// # Errors
    /// Rejects original drift, same-owner mismatch, expiry/regression or changed current context.
    pub fn recheck(&self) -> Result<()> {
        self.owner.require_same_financial(self.financial)?;
        let c = self.owner.current.as_ref().ok_or(Rejected)?;
        self.owner
            .verify_live_subject(self.financial, &c.signed.subject)?;
        self.owner.recheck_journal()
    }
}
impl KagemushaCapturedOrdinaryFinancialControlDecisionV1<'_> {
    /// Bind this captured decision to the same actual financial/selection/enrollment owner.
    /// Public fields or equal keys cannot substitute another Native holder.
    pub(crate) fn recheck_financial_owner(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.recheck_historical_originals()?;
        self.owner.require_same_historical_financial(financial)?;
        if !std::ptr::eq(self.financial, financial) {
            return Err(Rejected);
        }
        self.recheck_historical_originals()
    }

    pub(crate) fn original_sha256(&self) -> Result<[u8; 32]> {
        self.recheck_historical_originals()?;
        Ok(self.original_sha256)
    }
    pub(crate) fn original(&self) -> Result<&[u8]> {
        self.recheck_historical_originals()?;
        Ok(&self
            .owner
            .historical
            .get(&self.original_sha256)
            .ok_or(Rejected)?
            .admitted
            .original)
    }
    pub(crate) fn captured_lower_ms(&self) -> u64 {
        self.lower_ms
    }
    pub(crate) fn captured_upper_ms(&self) -> u64 {
        self.upper_ms
    }
    pub(crate) fn financial_secret(&self) -> Result<&[u8; 32]> {
        self.recheck_historical_originals()?;
        Ok(&self.financial.reservation.secret)
    }
    pub(crate) fn recheck_historical_originals(&self) -> Result<()> {
        self.owner
            .require_same_historical_financial(self.financial)?;
        let historical = self
            .owner
            .historical
            .get(&self.original_sha256)
            .ok_or(Rejected)?;
        let c = &historical.admitted;
        require_captured_interval(&c.signed.subject, self.lower_ms, self.upper_ms)?;
        if historical.lower_ms != self.lower_ms
            || historical.upper_ms != self.upper_ms
            || <[u8; 32]>::from(Sha256::digest(&c.original)) != self.original_sha256
        {
            return Err(Rejected);
        }
        c.signed
            .verify_for_request(&c.signed.subject.request, &self.owner.selected.issuer)
            .map_err(|_| Rejected)
    }
}
fn decode<T>(raw: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if raw.is_empty() || raw.len() > 64 * 1024 {
        return Err(Rejected);
    }
    norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
        .map_err(|_| Rejected)
}
fn decode_signed(raw: &[u8]) -> Result<KagemushaSignedOrdinaryCurrentControlV1> {
    if raw.len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1 {
        return Err(Rejected);
    }
    decode(raw)
}

fn validate_historical_capacity(
    count: usize,
    current_bytes: usize,
    original_len: usize,
) -> Result<()> {
    if count >= MAX_HISTORICAL_CAPTURES
        || original_len == 0
        || original_len > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
        || current_bytes
            .checked_add(original_len)
            .is_none_or(|bytes| bytes > MAX_HISTORICAL_ORIGINAL_BYTES)
    {
        return Err(Rejected);
    }
    Ok(())
}

fn require_read_budget(started: NativeContinuousReading) -> Result<()> {
    if started.elapsed().map_err(|_| Custody)?
        >= std::time::Duration::from_millis(KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_LIFETIME_MS_V1)
    {
        return Err(Rejected);
    }
    Ok(())
}
fn require_captured_interval(
    subject: &KagemushaOrdinaryCurrentControlSubjectV1,
    lower_ms: u64,
    upper_ms: u64,
) -> Result<()> {
    if lower_ms > upper_ms || lower_ms < subject.issued_at_ms || upper_ms >= subject.expires_at_ms {
        return Err(Rejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{bind, selected};
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    fn financial(root: &Path) -> KagemushaOrdinaryEnrolledFinancialOwnerV1 {
        let f = Fixture::with_single_member_wallet(false, false, [19; 32]);
        let original = selected(&f, 300);
        let mut held =
            KagemushaOrdinaryPreparationReservationV1::create(root, original, 300).unwrap();
        let f = bind(f, held.carrier().unwrap());
        held.retain_preparation(&f.selection.preparation.to_transport_bytes().unwrap())
            .unwrap();
        held.reference_ms = 600;
        held.reference_clock = Reading::now().unwrap();
        held.complete_enrollment(Arc::new(f.verify(600).unwrap()))
            .unwrap()
    }
    fn sign_pending(
        owner: &mut KagemushaOrdinaryCurrentFinancialControlOwnerV1,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> [u8; 64] {
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let request = owner.pending_account_request(financial).unwrap().clone();
        assert!(owner.fence_account_request(financial).unwrap().is_none());
        let raw = Signature::try_new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap()
        .payload()
        .try_into()
        .unwrap();
        owner
            .retain_account_request_original(financial, raw)
            .unwrap();
        raw
    }
    fn retained_control(
        owner: &mut KagemushaOrdinaryCurrentFinancialControlOwnerV1,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> [u8; 32] {
        owner.prepare_current_read(financial).unwrap();
        sign_pending(owner, financial);
        let request = owner.pending_account_request(financial).unwrap().clone();
        let release = owner.selected.governed.release();
        let profile = owner.selected.governed.profile_id();
        let subject = KagemushaOrdinaryCurrentControlSubjectV1 {
            request: request.clone(),
            release_id: release.release_id(),
            hardware_profile_id: profile,
            profile_policy_epoch: release.enabled_profile(profile).unwrap().policy_epoch,
            ordinary_trust_policy_digest: owner
                .selected
                .governed
                .trust()
                .canonical_digest()
                .unwrap(),
            app_authority_policy_digest: owner
                .selected
                .governed
                .authority()
                .canonical_digest()
                .unwrap(),
            authority_height: 2,
            authority_context_id: iroha_crypto::Hash::new(b"synthetic replay context"),
            world_root: iroha_crypto::Hash::new(b"synthetic replay World"),
            world_schema_hash: iroha_crypto::Hash::new(b"synthetic independent schema"),
            asset_definition_original_sha256: [9; 32],
            verifier_registry_original_sha256: [10; 32],
            data_incarnation_digest: [11; 32],
            data_revision: 1,
            data_policy_epoch: 1,
            data_schema_epoch: 1,
            latest_integrity_lease_original: None,
            issued_at_ms: 600,
            expires_at_ms: 1000,
        };
        let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        let signed = KagemushaSignedOrdinaryCurrentControlV1 {
            signature: Signature::try_new(
                issuer.private_key(),
                &subject.issuer_signing_message().unwrap(),
            )
            .unwrap(),
            subject,
        };
        let original = signed.canonical_bytes().unwrap();
        let digest = Sha256::digest(&original).into();
        // Test-only durable replay input. It creates no current loan/World authority: actual
        // runtime intake remains mandatory and this fixture has no Native clock owner.
        owner
            .append(&ControlRecord::Accepted {
                nonce: request.request_nonce,
                original,
                captured_lower_ms: 600,
                captured_upper_ms: 601,
            })
            .unwrap();
        digest
    }
    #[test]
    fn current_fi_control_pending_capture_cannot_be_promoted_by_cold_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let digest = retained_control(&mut owner, &financial);
        owner
            .append(&ControlRecord::ProofCapturePending {
                original_sha256: digest,
            })
            .unwrap();
        drop(owner);
        let mut recovered =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert!(recovered.historical.is_empty());
        assert!(recovered.historical_ready.is_empty());
        assert!(
            recovered
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_err()
        );
        assert!(recovered.current.is_none());
        assert!(
            recovered
                .resume_all_retained_proof_decisions(&financial)
                .is_err()
        );
    }
    #[test]
    fn current_fi_control_capture_ack_requires_the_same_pending_original_and_original_window() {
        for change in 0..4 {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().canonicalize().unwrap();
            let financial = financial(&root);
            let mut owner =
                KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
            let digest = retained_control(&mut owner, &financial);
            if change != 0 {
                owner
                    .append(&ControlRecord::ProofCapturePending {
                        original_sha256: digest,
                    })
                    .unwrap();
            }
            owner
                .append(&ControlRecord::ProofCaptureAcknowledged {
                    original_sha256: if change == 1 { [99; 32] } else { digest },
                    lower_ms: if change == 2 { 599 } else { 602 },
                    upper_ms: if change == 3 { 1000 } else { 603 },
                })
                .unwrap();
            drop(owner);
            assert!(
                KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                    .is_err()
            );
        }
    }
    #[test]
    fn current_fi_control_cold_ack_preserves_history_only_and_requires_a_new_live_grant() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let digest = retained_control(&mut owner, &financial);
        owner
            .append(&ControlRecord::ProofCapturePending {
                original_sha256: digest,
            })
            .unwrap();
        owner
            .append(&ControlRecord::ProofCaptureAcknowledged {
                original_sha256: digest,
                lower_ms: 602,
                upper_ms: 603,
            })
            .unwrap();
        drop(owner);
        let mut recovered =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        let historical = recovered.historical.get(&digest).unwrap();
        assert_eq!(historical.lower_ms, 602);
        assert_eq!(historical.upper_ms, 603);
        assert!(recovered.historical_ready.is_empty());
        assert!(
            recovered
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_err()
        );
        assert!(recovered.current.is_none());
        assert!(recovered.loan(&financial).is_err());
        assert!(
            recovered
                .resume_all_retained_proof_decisions(&financial)
                .is_err()
        );
    }
    #[test]
    fn current_fi_control_process_history_borrow_preserves_expired_window_without_a_live_grant() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let digest = retained_control(&mut owner, &financial);
        owner
            .append(&ControlRecord::ProofCapturePending {
                original_sha256: digest,
            })
            .unwrap();
        owner
            .append(&ControlRecord::ProofCaptureAcknowledged {
                original_sha256: digest,
                lower_ms: 602,
                upper_ms: 603,
            })
            .unwrap();
        drop(owner);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert!(
            owner
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_err()
        );
        assert!(
            owner
                .resume_all_retained_proof_decisions(&financial)
                .is_err()
        );
        assert!(owner.historical_ready.is_empty());
        // Test-only simulation of successful same-process capture/resumption. The fixture has
        // no actual Native clock, so production resume above cannot set this marker. This tests
        // private history behavior, not a current-world or monetary authority grant.
        owner.historical_ready.insert(digest);
        financial.reservation.reference_ms = 2500;
        financial.reservation.reference_clock = Reading::now().unwrap();
        // The completed enrollment outlives its preparation and the short FI control.
        financial.recheck().unwrap();
        assert!(owner.loan(&financial).is_err());
        // Expire the real current enrollment at its own signed deadline; history still borrows.
        financial.reservation.reference_ms =
            financial.enrollment().certificate().subject.expires_at_ms;
        financial.reservation.reference_clock = Reading::now().unwrap();
        let loan = owner
            .borrow_captured_proof_decision(&financial, digest, 602, 603)
            .unwrap();
        assert_eq!(loan.original_sha256().unwrap(), digest);
        assert_eq!(loan.captured_lower_ms(), 602);
        assert_eq!(loan.captured_upper_ms(), 603);
        assert_ne!(*loan.financial_secret().unwrap(), [0; 32]);
        let signed = decode_signed(loan.original().unwrap()).unwrap();
        assert_eq!(signed.subject.expires_at_ms, 1000);
        assert!(financial.recheck().is_err());
        assert!(owner.loan(&financial).is_err());
        owner.historical_ready.clear();
        owner.historical_ready.insert([99; 32]);
        assert!(
            owner
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_err()
        );
        owner.historical_ready.clear();
        owner.historical_ready.insert(digest);
        drop(owner);
        let recovered =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert!(recovered.historical_ready.is_empty());
        assert!(
            recovered
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_err()
        );
    }
    #[test]
    fn current_fi_control_process_history_rejects_another_actual_financial_owner() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let digest = retained_control(&mut owner, &financial);
        owner
            .append(&ControlRecord::ProofCapturePending {
                original_sha256: digest,
            })
            .unwrap();
        owner
            .append(&ControlRecord::ProofCaptureAcknowledged {
                original_sha256: digest,
                lower_ms: 602,
                upper_ms: 603,
            })
            .unwrap();
        drop(owner);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        // Test-only same-process marker; another independently retained financial owner has
        // different actual Arc/reservation custody even with equal fixture public originals.
        owner.historical_ready.insert(digest);
        let other_temp = tempfile::tempdir().unwrap();
        let other_root = other_temp.path().canonicalize().unwrap();
        let other_financial = self::financial(&other_root);
        assert!(
            owner
                .borrow_captured_proof_decision(&other_financial, digest, 602, 603)
                .is_err()
        );
        assert!(
            owner
                .borrow_captured_proof_decision(&financial, digest, 602, 603)
                .is_ok()
        );
    }
    #[test]
    fn current_fi_control_cold_recovery_keeps_nonce_history_and_never_lends_a_grant() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let first = owner.prepare_current_read(&financial).unwrap();
        let request: KagemushaOrdinaryCurrentControlRequestV1 =
            norito::decode_canonical(&first[0]).unwrap();
        let signature = sign_pending(&mut owner, &financial);
        assert_eq!(
            owner.retained_current_read_fields(&financial).unwrap(),
            vec![first[0].clone(), signature.to_vec()]
        );
        assert_eq!(
            owner.fence_account_request(&financial).unwrap(),
            Some(signature)
        );
        assert!(owner.loan(&financial).is_err());
        drop(owner);
        let mut recovered =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert!(recovered.pending.is_none());
        assert!(recovered.current.is_none());
        assert!(recovered.proof_capture.is_none());
        assert!(recovered.used.contains(&request.request_nonce));
        assert!(recovered.loan(&financial).is_err());
        let next = recovered.prepare_current_read(&financial).unwrap();
        let next: KagemushaOrdinaryCurrentControlRequestV1 =
            norito::decode_canonical(&next[0]).unwrap();
        assert_ne!(next.request_nonce, request.request_nonce);
        assert!(recovered.actual_clock().is_err()); // A fixture scalar never becomes a genuine current clock.
    }
    #[test]
    fn current_fi_control_all_captured_attempts_survive_replay_with_exact_original_bounds() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        let mut originals = Vec::new();
        for attempt in 0..3 {
            let digest = retained_control(&mut owner, &financial);
            let lower = 602 + attempt;
            let upper = 603 + attempt;
            owner
                .append(&ControlRecord::ProofCapturePending {
                    original_sha256: digest,
                })
                .unwrap();
            owner
                .append(&ControlRecord::ProofCaptureAcknowledged {
                    original_sha256: digest,
                    lower_ms: lower,
                    upper_ms: upper,
                })
                .unwrap();
            originals.push((digest, lower, upper));
        }
        drop(owner);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert_eq!(owner.historical.len(), 3);
        assert!(owner.historical_ready.is_empty());
        for (digest, lower, upper) in &originals {
            owner
                .recheck_retained_capture_identity(&financial, *digest, *lower, *upper)
                .unwrap();
            assert!(
                owner
                    .borrow_captured_proof_decision(&financial, *digest, *lower, *upper)
                    .is_err()
            );
            assert!(
                owner
                    .recheck_retained_capture_identity(&financial, *digest, *lower + 1, *upper)
                    .is_err()
            );
            assert!(
                owner
                    .recheck_retained_capture_identity(&financial, *digest, *lower, *upper + 1)
                    .is_err()
            );
        }
        assert!(
            owner
                .resume_all_retained_proof_decisions(&financial)
                .is_err()
        );
        assert!(owner.historical_ready.is_empty());
        // Test-only simulation of same-process successful resume; actual fixture lacks Native
        // clock/current World intake and cannot mark any history through the production API.
        owner
            .historical_ready
            .extend(originals.iter().map(|row| row.0));
        for (digest, lower, upper) in originals {
            let proof = owner
                .borrow_captured_proof_decision(&financial, digest, lower, upper)
                .unwrap();
            assert_eq!(proof.original_sha256().unwrap(), digest);
            assert_eq!(
                (proof.captured_lower_ms(), proof.captured_upper_ms()),
                (lower, upper)
            );
            assert!(owner.loan(&financial).is_err());
        }
    }
    #[test]
    fn current_fi_control_history_capacity_is_finite_and_never_overflows_or_drops_old_records() {
        assert!(validate_historical_capacity(0, 0, 1).is_ok());
        assert!(
            validate_historical_capacity(
                MAX_HISTORICAL_CAPTURES - 1,
                MAX_HISTORICAL_ORIGINAL_BYTES - 1,
                1
            )
            .is_ok()
        );
        assert!(validate_historical_capacity(MAX_HISTORICAL_CAPTURES, 0, 1).is_err());
        assert!(validate_historical_capacity(0, MAX_HISTORICAL_ORIGINAL_BYTES, 1).is_err());
        assert!(validate_historical_capacity(0, usize::MAX, 1).is_err());
        assert!(validate_historical_capacity(0, 0, 0).is_err());
        assert!(
            validate_historical_capacity(0, 0, KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1 + 1)
                .is_err()
        );
    }
    #[test]
    fn current_fi_control_unknown_account_invocation_never_signs_again() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        owner.prepare_current_read(&financial).unwrap();
        let nonce = owner
            .pending_account_request(&financial)
            .unwrap()
            .request_nonce;
        assert!(owner.fence_account_request(&financial).unwrap().is_none());
        assert_eq!(
            owner.fence_account_request(&financial).unwrap_err(),
            KagemushaOrdinaryIdentityErrorV1::UnknownOutcome
        );
        assert!(owner.retained_current_read_fields(&financial).is_err());
        drop(owner);
        let recovered =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                .unwrap();
        assert!(recovered.used.contains(&nonce));
        assert!(recovered.pending.is_none());
        assert!(recovered.loan(&financial).is_err());
    }
    #[test]
    fn current_fi_control_recovery_rejects_duplicate_signature_fence_or_nonce() {
        for duplicate_nonce in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().canonicalize().unwrap();
            let financial = financial(&root);
            let mut owner =
                KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
            owner.prepare_current_read(&financial).unwrap();
            let request = owner.pending_account_request(&financial).unwrap().clone();
            owner.fence_account_request(&financial).unwrap();
            let malformed = if duplicate_nonce {
                ControlRecord::Reserve(Box::new(request))
            } else {
                ControlRecord::AccountInvoked {
                    nonce: request.request_nonce,
                }
            };
            // Test-only corruption is written through the genuine private fsynced journal;
            // recovery must reject even a structurally valid record hash/prefix.
            owner.append(&malformed).unwrap();
            drop(owner);
            assert!(
                KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(&root, &financial)
                    .is_err()
            );
        }
    }
    #[test]
    fn current_fi_control_refuses_unfenced_or_foreign_wallet_signature() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root);
        let mut owner =
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(&root, &financial).unwrap();
        owner.prepare_current_read(&financial).unwrap();
        let request = owner.pending_account_request(&financial).unwrap().clone();
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let raw = Signature::try_new(
            wallet.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap()
        .payload()
        .try_into()
        .unwrap();
        assert!(
            owner
                .retain_account_request_original(&financial, raw)
                .is_err()
        );
        owner.fence_account_request(&financial).unwrap();
        let other = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        let raw = Signature::try_new(
            other.private_key(),
            &request.account_signing_message().unwrap(),
        )
        .unwrap()
        .payload()
        .try_into()
        .unwrap();
        assert!(
            owner
                .retain_account_request_original(&financial, raw)
                .is_err()
        );
        assert!(owner.current.is_none());
        assert!(owner.loan(&financial).is_err());
    }
}
