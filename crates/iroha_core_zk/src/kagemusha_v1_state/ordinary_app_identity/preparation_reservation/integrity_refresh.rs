//! Private current Integrity refresh custody. This never lends a current FI/money grant.
//! The original C/FI are immutable; only this refresh ceremony may proceed after old PI expiry.
use super::*;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

const PI_FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-integrity-refresh.norito.wal",
    magic: b"KGMCPIR1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-integrity-refresh-journal\0",
    maximum_payload_bytes: 128 * 1024,
};
const MAX_ATTEMPTS: usize = 4096;
const MAX_LEASES: usize = 1024;
const MAX_ROWS: u64 = 40961;
const MAX_WAL_BYTES: u64 = 64 * 1024 * 1024;
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::IntegrityRefreshRecordV1")]
enum Record {
    Initialize(Box<Identity>),
    Reserve {
        nonce: [u8; 32],
    },
    Challenge {
        original: Vec<u8>,
        lower_ms: u64,
        upper_ms: u64,
    },
    PlatformInvoked {
        nonce: [u8; 32],
    },
    SignaturePending {
        original: Vec<u8>,
    },
    SignatureAcknowledged {
        lower_ms: u64,
        upper_ms: u64,
    },
    IntegrityToken {
        original: Vec<u8>,
    },
    LeasePending {
        original: Vec<u8>,
    },
    LeaseAcknowledged {
        lower_ms: u64,
        upper_ms: u64,
    },
    Abandon {
        nonce: [u8; 32],
    },
}
#[derive(PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::IntegrityRefreshIdentityV1")]
struct Identity {
    enrollment_sha256: [u8; 32],
    credential_sha256: [u8; 32],
    issuer_policy_digest: [u8; 32],
    core_issuer_policy_digest: [u8; 32],
    ordinary_identity_policy_id: [u8; 32],
}
struct Pending {
    nonce: [u8; 32],
    challenge: Option<KagemushaSignedPlayIntegrityRefreshChallengeV1>,
    invoked: bool,
    signature_pending: Option<Vec<u8>>,
    signature: Option<Vec<u8>>,
    token: Option<Vec<u8>>,
    lease_pending: Option<Vec<u8>>,
}
impl Pending {
    fn new(nonce: [u8; 32]) -> Self {
        Self {
            nonce,
            challenge: None,
            invoked: false,
            signature_pending: None,
            signature: None,
            token: None,
            lease_pending: None,
        }
    }
}
/// Actual same-owner private Integrity journal. No decoded lease, status, nonce, clock or
/// application-supplied policy can construct this holder; it borrows the actual financial owner.
pub struct KagemushaOrdinaryIntegrityRefreshOwnerV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    journal: PrivateJournal,
    prefix: Option<crate::kagemusha_v1_state::KagemushaRecoveryJournalPrefixV1>,
    rows: u64,
    used: BTreeSet<[u8; 32]>,
    pending: Option<Pending>,
    leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
}
impl KagemushaOrdinaryIntegrityRefreshOwnerV1 {
    /// Create and fsync original identity from the same actual C/FI/clock custody.
    /// # Errors
    /// Refuses changed storage, unsupported profile, original static expiry or unknown durability.
    pub fn create(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        static_current_interval(financial)?;
        let journal =
            PrivateJournal::create_new(&root.join("ordinary-integrity-refresh"), PI_FORMAT)
                .map_err(|_| Custody)?;
        let mut this = Self::new(journal, financial)?;
        this.append(&Record::Initialize(Box::new(this.identity()?)))?;
        this.require_financial(financial)?;
        static_current_interval(financial)?;
        Ok(this)
    }
    /// Recover exact acknowledged originals. A pending signature/lease alone is never admitted.
    /// Current intake/exposure separately obtains a fresh genuine Native interval.
    /// # Errors
    /// Refuses noncanonical/order/substitution errors, unsafe storage, or bounded history overflow.
    pub fn open_existing(
        root: &Path,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        financial.recheck_historical_proof_custody()?;
        let journal =
            PrivateJournal::open_existing(&root.join("ordinary-integrity-refresh"), PI_FORMAT)
                .map_err(|_| Custody)?;
        let mut this = Self::new(journal, financial)?;
        while let Some((sequence, bytes)) = this.journal.replay_next().map_err(|_| Custody)? {
            if sequence != this.rows
                || this.rows >= MAX_ROWS
                || u64::try_from(bytes.len()).map_err(|_| Rejected)?
                    > PI_FORMAT.maximum_payload_bytes
            {
                return Err(Custody);
            }
            let row: Record = norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .map_err(|_| Custody)?;
            if norito::encode_canonical(&row).map_err(|_| Rejected)? != bytes {
                return Err(Custody);
            }
            this.replay(row, financial)?;
            this.rows += 1;
        }
        if this.rows == 0 {
            return Err(Custody);
        }
        let prefix = this.journal.recovery_prefix().map_err(|_| Custody)?;
        if prefix.byte_len > MAX_WAL_BYTES {
            return Err(Rejected);
        }
        this.prefix = Some(prefix);
        this.require_financial(financial)?;
        Ok(this)
    }
    fn new(
        journal: PrivateJournal,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Self> {
        financial.recheck_historical_proof_custody()?;
        let c = financial.enrollment.app_credential().subject();
        if c.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint
            || c.play_integrity.is_none()
        {
            return Err(Rejected);
        }
        Ok(Self {
            selected: Arc::clone(&financial.reservation.selected),
            enrollment: Arc::clone(&financial.enrollment),
            journal,
            prefix: None,
            rows: 0,
            used: BTreeSet::new(),
            pending: None,
            leases: Vec::new(),
        })
    }
    fn identity(&self) -> Result<Identity> {
        Ok(Identity {
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
            core_issuer_policy_digest: self
                .selected
                .ordinary
                .issuer_policy()
                .policy()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            ordinary_identity_policy_id: self.selected.ordinary.identity_policy().policy_id(),
        })
    }
    fn require_financial(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.recheck_journal()?;
        financial.recheck_historical_proof_custody()?;
        if !Arc::ptr_eq(&self.selected, &financial.reservation.selected)
            || !Arc::ptr_eq(&self.enrollment, &financial.enrollment)
        {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Reserve/fsync fresh Native nonce before the Core prepare request. Exact pending retry
    /// returns the original enrollment and nonce; it never reinvokes the platform key.
    /// # Errors
    /// Refuses unavailable current static custody, capacity overflow or unknown durability.
    pub fn prepare(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.require_financial(financial)?;
        static_current_interval(financial)?;
        if self.pending.is_none() {
            if self.used.len() >= MAX_ATTEMPTS || self.leases.len() >= MAX_LEASES {
                return Err(Rejected);
            }
            let mut nonce = [0; 32];
            OsRng.try_fill_bytes(&mut nonce).map_err(|_| Custody)?;
            if nonce == [0; 32] || self.used.contains(&nonce) {
                return Err(Rejected);
            }
            self.append(&Record::Reserve { nonce })?;
            self.used.insert(nonce);
            self.pending = Some(Pending::new(nonce));
        }
        static_current_interval(financial)?;
        let pending = self.pending.as_ref().ok_or(Custody)?;
        Ok(vec![
            self.enrollment.certificate().subject.enrollment_id.to_vec(),
            pending.nonce.to_vec(),
        ])
    }
    /// Admit the complete Core-signed challenge under the original nonce and installed policies.
    /// # Errors
    /// Refuses wrong nonce/key/scope, original expiry, reordered phase or failed durability.
    pub fn accept_challenge(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        self.require_financial(financial)?;
        let signed = KagemushaSignedPlayIntegrityRefreshChallengeV1::from_transport_bytes(original)
            .map_err(|_| Rejected)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if let Some(held) = &pending.challenge {
            if held.to_transport_bytes().map_err(|_| Rejected)? != original {
                return Err(Rejected);
            }
        } else {
            let interval = static_current_interval(financial)?;
            self.verify_challenge(
                &signed,
                pending.nonce,
                interval.lower_ms(),
                interval.upper_ms(),
            )?;
            self.append(&Record::Challenge {
                original: original.to_vec(),
                lower_ms: interval.lower_ms(),
                upper_ms: interval.upper_ms(),
            })?;
            self.pending.as_mut().ok_or(Custody)?.challenge = Some(signed);
        }
        let interval = static_current_interval(financial)?;
        let pending = self.pending.as_ref().ok_or(Custody)?;
        let signed = pending.challenge.as_ref().ok_or(Custody)?;
        self.verify_challenge(
            signed,
            pending.nonce,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        challenge_fields(signed)
    }
    /// Durably fence the sole platform invocation before exposing its exact signing message.
    /// # Errors
    /// Refuses repeated/unknown invocation, substituted current owner or expired original challenge.
    pub fn fence_platform_invocation(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.require_financial(financial)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if pending.invoked {
            return Err(super::super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome);
        }
        let interval = static_current_interval(financial)?;
        let signed = pending.challenge.as_ref().ok_or(Rejected)?;
        self.verify_challenge(
            signed,
            pending.nonce,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        let nonce = pending.nonce;
        self.append(&Record::PlatformInvoked { nonce })?;
        self.pending.as_mut().ok_or(Custody)?.invoked = true;
        let interval = static_current_interval(financial)?;
        let pending = self.pending.as_ref().ok_or(Custody)?;
        let signed = pending.challenge.as_ref().ok_or(Custody)?;
        self.verify_challenge(signed, nonce, interval.lower_ms(), interval.upper_ms())?;
        challenge_fields(signed)
    }
    /// Retain full DER then sample both bounds after its fsync and acknowledge that exact capture.
    /// Pending original alone cannot become a completed capture during cold replay.
    /// # Errors
    /// Refuses missing fence, wrong signature, expired capture or nonidentical original retry.
    pub fn capture_signature(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        der: &[u8],
    ) -> Result<()> {
        self.require_financial(financial)?;
        require_der(der)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if !pending.invoked {
            return Err(Rejected);
        }
        self.verify_signature(pending, der)?;
        if let Some(held) = &pending.signature {
            if held != der {
                return Err(Rejected);
            }
            return Ok(());
        }
        if let Some(held) = &pending.signature_pending {
            if held != der {
                return Err(Rejected);
            }
        } else {
            self.append(&Record::SignaturePending {
                original: der.to_vec(),
            })?;
            self.pending.as_mut().ok_or(Custody)?.signature_pending = Some(der.to_vec());
        }
        let interval = static_current_interval(financial)?;
        let pending = self.pending.as_ref().ok_or(Custody)?;
        self.verify_challenge(
            pending.challenge.as_ref().ok_or(Rejected)?,
            pending.nonce,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        self.append(&Record::SignatureAcknowledged {
            lower_ms: interval.lower_ms(),
            upper_ms: interval.upper_ms(),
        })?;
        self.pending.as_mut().ok_or(Custody)?.signature = Some(der.to_vec());
        self.require_financial(financial)?;
        static_current_interval(financial)?;
        Ok(())
    }
    /// Retain the complete Google token for exact HTTP retry; token shape proves no verdict.
    /// # Errors
    /// Refuses absent acknowledged DER, changed retry, invalid token or expired challenge.
    pub fn retain_integrity_token(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        token: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        self.require_financial(financial)?;
        require_token(token)?;
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if pending.signature.is_none() {
            return Err(Rejected);
        }
        let interval = static_current_interval(financial)?;
        self.verify_challenge(
            pending.challenge.as_ref().ok_or(Rejected)?,
            pending.nonce,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        if let Some(held) = &pending.token {
            if held != token {
                return Err(Rejected);
            }
        } else {
            self.append(&Record::IntegrityToken {
                original: token.to_vec(),
            })?;
            self.pending.as_mut().ok_or(Custody)?.token = Some(token.to_vec());
        }
        let interval = static_current_interval(financial)?;
        let pending = self.pending.as_ref().ok_or(Custody)?;
        let signed = pending.challenge.as_ref().ok_or(Custody)?;
        self.verify_challenge(
            signed,
            pending.nonce,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        Ok(vec![
            signed
                .challenge
                .attempt_id()
                .map_err(|_| Rejected)?
                .to_vec(),
            signed.to_transport_bytes().map_err(|_| Rejected)?,
            pending.signature.as_ref().ok_or(Custody)?.clone(),
            pending.token.as_ref().ok_or(Custody)?.clone(),
        ])
    }
    /// Authenticate full issuer/platform/circuit originals and acknowledge admission after raw
    /// fsync. Only this genuine held Arc can be passed to the actual cash/publication PI intake.
    /// # Errors
    /// Refuses foreign challenge/DER/policy, expired lease, invalid signatures or unknown durability.
    pub fn accept_lease(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        original: &[u8],
    ) -> Result<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>> {
        self.require_financial(financial)?;
        if original.is_empty() || original.len() > 4096 {
            return Err(Rejected);
        }
        if self.pending.is_none() {
            let held = self.leases.last().ok_or(Rejected)?;
            if held.original() != original {
                return Err(Rejected);
            }
            let interval = static_current_interval(financial)?;
            interval.check_both(|point| {
                self.enrollment
                    .recheck_with_integrity_lease(held, point)
                    .map_err(|_| Rejected)
            })?;
            return Ok(Arc::clone(held));
        }
        let pending = self.pending.as_ref().ok_or(Rejected)?;
        if pending.token.is_none() || pending.signature.is_none() || self.leases.len() >= MAX_LEASES
        {
            return Err(Rejected);
        }
        if let Some(held) = &pending.lease_pending {
            if held != original {
                return Err(Rejected);
            }
        } else {
            self.append(&Record::LeasePending {
                original: original.to_vec(),
            })?;
            self.pending.as_mut().ok_or(Custody)?.lease_pending = Some(original.to_vec());
        }
        let interval = static_current_interval(financial)?;
        let held = self.verify_lease(
            self.pending.as_ref().ok_or(Custody)?,
            original,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        self.append(&Record::LeaseAcknowledged {
            lower_ms: interval.lower_ms(),
            upper_ms: interval.upper_ms(),
        })?;
        let held = Arc::new(held);
        self.leases.push(Arc::clone(&held));
        self.pending = None;
        let current = static_current_interval(financial)?;
        current.check_both(|point| {
            self.enrollment
                .recheck_with_integrity_lease(&held, point)
                .map_err(|_| Rejected)
        })?;
        self.require_financial(financial)?;
        Ok(held)
    }
    /// Recover authentic acknowledged lease capabilities for Native semantic replay only.
    /// This does not revalidate old lease expiry or lend a live PI/FI grant.
    /// # Errors
    /// Refuses changed held owner, private journal or original policy identity.
    pub fn retained_verified_leases(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>> {
        self.require_financial(financial)?;
        Ok(self.leases.clone())
    }
    /// Durably abandon uncertainty without reusing nonce or invoking the old platform attempt.
    /// # Errors
    /// Refuses substituted custody, absent pending attempt or unavailable current static context.
    pub fn abandon_pending(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        self.require_financial(financial)?;
        static_current_interval(financial)?;
        let nonce = self.pending.as_ref().ok_or(Rejected)?.nonce;
        self.append(&Record::Abandon { nonce })?;
        self.pending = None;
        self.require_financial(financial)
    }
    fn verify_challenge(
        &self,
        signed: &KagemushaSignedPlayIntegrityRefreshChallengeV1,
        nonce: [u8; 32],
        lower: u64,
        upper: u64,
    ) -> Result<()> {
        require_interval(lower, upper)?;
        if signed.challenge.nonce != nonce {
            return Err(Rejected);
        }
        for point in [lower, upper] {
            static_point(&self.enrollment, &self.selected, point)?;
            signed
                .authenticate(
                    &signed.challenge,
                    self.enrollment.app_credential(),
                    self.selected.governed.release(),
                    self.selected.governed.trust(),
                    self.selected.governed.authority(),
                    self.selected.preparation_issuer_key()?,
                    point,
                )
                .map_err(|_| Rejected)?;
        }
        Ok(())
    }
    fn verify_signature(&self, pending: &Pending, der: &[u8]) -> Result<()> {
        require_der(der)?;
        let credential = self.enrollment.app_credential().subject();
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: der.to_vec(),
        }
        .authenticate_signature(
            credential.platform_class,
            &credential.app_public_key,
            credential.app_signing_identity_digest,
            credential.app_release_digest,
            None,
            &pending
                .challenge
                .as_ref()
                .ok_or(Rejected)?
                .challenge
                .possession_signing_bytes()
                .map_err(|_| Rejected)?,
        )
        .map_err(|_| Rejected)
        .and_then(|(counter, measurement)| {
            if counter.is_some() || measurement.is_some() {
                Err(Rejected)
            } else {
                Ok(())
            }
        })
    }
    fn verify_lease(
        &self,
        pending: &Pending,
        original: &[u8],
        lower: u64,
        upper: u64,
    ) -> Result<KagemushaVerifiedPlayIntegrityRefreshLeaseV1> {
        require_interval(lower, upper)?;
        let raw: KagemushaPlayIntegrityRefreshLeaseV1 = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|_| Rejected)?;
        if raw.canonical_bytes().map_err(|_| Rejected)? != original {
            return Err(Rejected);
        }
        match &raw.app_possession {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                if Some(signature_der) != pending.signature.as_ref() {
                    return Err(Rejected);
                }
            }
            _ => return Err(Rejected),
        }
        let held = raw
            .authenticate(
                self.enrollment.app_credential(),
                self.selected.governed.release(),
                self.selected.governed.trust(),
                self.selected.governed.authority(),
                pending.challenge.as_ref().ok_or(Rejected)?,
                self.selected.preparation_issuer_key()?,
                lower,
            )
            .map_err(|_| Rejected)?;
        for point in [lower, upper] {
            static_point(&self.enrollment, &self.selected, point)?;
            self.enrollment
                .recheck_with_integrity_lease(&held, point)
                .map_err(|_| Rejected)?;
        }
        Ok(held)
    }
    fn replay(
        &mut self,
        row: Record,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        match row {
            Record::Initialize(identity) if self.rows == 0 => {
                if *identity != self.identity()? {
                    return Err(Custody);
                }
            }
            Record::Reserve { nonce } if self.rows > 0 && self.pending.is_none() => {
                if nonce == [0; 32]
                    || self.used.len() >= MAX_ATTEMPTS
                    || self.leases.len() >= MAX_LEASES
                    || !self.used.insert(nonce)
                {
                    return Err(Custody);
                }
                self.pending = Some(Pending::new(nonce));
            }
            Record::Challenge {
                original,
                lower_ms,
                upper_ms,
            } => {
                let pending = self.pending.as_ref().ok_or(Custody)?;
                if pending.challenge.is_some() {
                    return Err(Custody);
                }
                let signed =
                    KagemushaSignedPlayIntegrityRefreshChallengeV1::from_transport_bytes(&original)
                        .map_err(|_| Custody)?;
                self.verify_challenge(&signed, pending.nonce, lower_ms, upper_ms)?;
                self.pending.as_mut().ok_or(Custody)?.challenge = Some(signed);
            }
            Record::PlatformInvoked { nonce } => {
                let pending = self.pending.as_mut().ok_or(Custody)?;
                if pending.nonce != nonce || pending.challenge.is_none() || pending.invoked {
                    return Err(Custody);
                }
                pending.invoked = true;
            }
            Record::SignaturePending { original } => {
                let pending = self.pending.as_ref().ok_or(Custody)?;
                if !pending.invoked || pending.signature_pending.is_some() {
                    return Err(Custody);
                }
                self.verify_signature(pending, &original)?;
                self.pending.as_mut().ok_or(Custody)?.signature_pending = Some(original);
            }
            Record::SignatureAcknowledged { lower_ms, upper_ms } => {
                let pending = self.pending.as_ref().ok_or(Custody)?;
                if pending.signature.is_some() {
                    return Err(Custody);
                }
                let der = pending.signature_pending.as_ref().ok_or(Custody)?.clone();
                self.verify_signature(pending, &der)?;
                self.verify_challenge(
                    pending.challenge.as_ref().ok_or(Custody)?,
                    pending.nonce,
                    lower_ms,
                    upper_ms,
                )?;
                self.pending.as_mut().ok_or(Custody)?.signature = Some(der);
            }
            Record::IntegrityToken { original } => {
                require_token(&original)?;
                let pending = self.pending.as_mut().ok_or(Custody)?;
                if pending.signature.is_none() || pending.token.is_some() {
                    return Err(Custody);
                }
                pending.token = Some(original);
            }
            Record::LeasePending { original } => {
                let pending = self.pending.as_mut().ok_or(Custody)?;
                if pending.token.is_none()
                    || pending.lease_pending.is_some()
                    || original.is_empty()
                    || original.len() > 4096
                {
                    return Err(Custody);
                }
                pending.lease_pending = Some(original);
            }
            Record::LeaseAcknowledged { lower_ms, upper_ms } => {
                let pending = self.pending.as_ref().ok_or(Custody)?;
                if self.leases.len() >= MAX_LEASES {
                    return Err(Custody);
                }
                let held = self.verify_lease(
                    pending,
                    pending.lease_pending.as_ref().ok_or(Custody)?,
                    lower_ms,
                    upper_ms,
                )?;
                self.leases.push(Arc::new(held));
                self.pending = None;
            }
            Record::Abandon { nonce } => {
                if self.pending.as_ref().ok_or(Custody)?.nonce != nonce {
                    return Err(Custody);
                }
                self.pending = None;
            }
            _ => return Err(Custody),
        }
        financial.recheck_historical_proof_custody()
    }
    fn recheck_journal(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        if self
            .prefix
            .is_some_and(|prefix| self.journal.recovery_prefix().ok() != Some(prefix))
        {
            return Err(Custody);
        }
        Ok(())
    }
    fn append(&mut self, row: &Record) -> Result<()> {
        self.recheck_journal()?;
        let bytes = norito::encode_canonical(row).map_err(|_| Rejected)?;
        let payload_len = u64::try_from(bytes.len()).map_err(|_| Rejected)?;
        let next_len = self
            .prefix
            .map_or(0, |prefix| prefix.byte_len)
            .checked_add(payload_len)
            .and_then(|len| len.checked_add(128))
            .ok_or(Rejected)?;
        if self.rows >= MAX_ROWS
            || payload_len > PI_FORMAT.maximum_payload_bytes
            || next_len > MAX_WAL_BYTES
        {
            return Err(Rejected);
        }
        self.journal.append(&bytes).map_err(|_| Custody)?;
        self.rows += 1;
        self.prefix = Some(self.journal.recovery_prefix().map_err(|_| Custody)?);
        self.recheck_journal()
    }
}
fn challenge_fields(
    signed: &KagemushaSignedPlayIntegrityRefreshChallengeV1,
) -> Result<Vec<Vec<u8>>> {
    Ok(vec![
        signed.to_transport_bytes().map_err(|_| Rejected)?,
        signed
            .challenge
            .possession_signing_bytes()
            .map_err(|_| Rejected)?,
        signed
            .challenge
            .request_hash()
            .map_err(|_| Rejected)?
            .to_vec(),
        signed.challenge.attested_key_id.to_vec(),
    ])
}
fn require_interval(lower: u64, upper: u64) -> Result<()> {
    if lower == 0 || lower > upper {
        return Err(Rejected);
    }
    Ok(())
}
fn require_der(der: &[u8]) -> Result<()> {
    if !(8..=72).contains(&der.len()) {
        return Err(Rejected);
    }
    Ok(())
}
fn require_token(token: &[u8]) -> Result<()> {
    if token.is_empty()
        || token.len() > 64 * 1024
        || token
            .iter()
            .any(|byte| !byte.is_ascii() || byte.is_ascii_control())
    {
        return Err(Rejected);
    }
    Ok(())
}
fn static_point(
    enrollment: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    selected: &KagemushaOrdinaryPreparationSelectedOriginalsV1,
    now: u64,
) -> Result<()> {
    selected.recheck_at_trusted_time(now)?;
    let fi = &enrollment.certificate().subject;
    let c = enrollment.app_credential().subject();
    if c.platform_class != KagemushaHardwarePlatformClassV1::AndroidKeyMint
        || c.play_integrity.is_none()
        || now < enrollment.authenticated_at_ms()
        || now < fi.issued_at_ms
        || now >= fi.expires_at_ms
        || now < c.issued_at_ms
        || now >= c.expires_at_ms
    {
        return Err(Rejected);
    }
    Ok(())
}
impl KagemushaOrdinaryEnrolledFinancialOwnerV1 {
    /// Refresh ceremony custody only; overdue old PI does not lend a current financial grant.
    pub(crate) fn recheck_integrity_refresh_custody(
        &self,
    ) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        static_current_interval(self)
    }
}

fn static_current_interval(
    financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
    financial.recheck_historical_proof_custody()?;
    let interval = financial.reservation.interval()?;
    interval.check_both(|point| {
        static_point(
            &financial.enrollment,
            &financial.reservation.selected,
            point,
        )
    })?;
    financial.reservation.recheck_originals()?;
    Ok(interval)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{bind, selected};
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::testing::ordinary_app_enrollment::{
        KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture, ordinary_test_issuer_admission_v1,
    };
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    // Actual original cryptography under known-public synthetic fixture keys. No installed
    // root, hardware, current money grant or Native qualification is manufactured by these tests.
    fn financial(root: &Path, integrity: bool) -> KagemushaOrdinaryEnrolledFinancialOwnerV1 {
        let mut f = Fixture::with_single_member_wallet(false, integrity, [19; 32]);
        let original = selected(&f, 300);
        let mut held =
            KagemushaOrdinaryPreparationReservationV1::create(root, original, 300).unwrap();
        let carrier = held.carrier().unwrap();
        if integrity {
            let mut c = f.selection.preparation.challenge;
            c.client_nonce = carrier.client_nonce;
            c.financial_authority_commitment = carrier.financial_authority_commitment;
            c.issued_at_ms = 300;
            let attested = f.selection.issuance.credential.subject.attested_key_id;
            let pi = f
                .selection
                .issuance
                .credential
                .subject
                .play_integrity
                .as_mut()
                .unwrap();
            pi.request_hash = c.play_integrity_request_hash(attested).unwrap();
            pi.verified_at_ms = 400;
        }
        let f = bind(f, carrier);
        held.retain_preparation(&f.selection.preparation.to_transport_bytes().unwrap())
            .unwrap();
        held.reference_ms = 600;
        held.reference_clock = Reading::now().unwrap();
        held.complete_enrollment(Arc::new(f.verify(600).unwrap()))
            .unwrap()
    }
    fn time(financial: &mut KagemushaOrdinaryEnrolledFinancialOwnerV1, now: u64) {
        financial.reservation.reference_ms = now;
        financial.reservation.reference_clock = Reading::now().unwrap();
    }
    fn originals(
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        nonce: [u8; 32],
    ) -> (
        KagemushaSignedPlayIntegrityRefreshChallengeV1,
        Vec<u8>,
        KagemushaPlayIntegrityRefreshLeaseV1,
    ) {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let core_issuer = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        let c = financial.enrollment.app_credential();
        let s = c.subject();
        let challenge = KagemushaPlayIntegrityRefreshChallengeV1 {
            version: 1,
            credential_digest: c.digest(),
            attested_key_id: s.attested_key_id,
            account_binding: s.account_binding,
            network_id: s.network_id,
            lane_id: s.lane_id,
            release_id: s.release_id,
            hardware_profile_id: s.hardware_profile_id,
            suite_id: s.suite_id,
            trust_policy_digest: s.trust_policy_digest,
            app_authority_policy_digest: s.app_authority_policy_digest,
            play_integrity_policy_digest: financial
                .reservation
                .selected
                .governed
                .trust()
                .play_integrity_policy
                .unwrap()
                .policy_digest,
            nonce,
            original_enrollment_challenge_digest: s.enrollment_challenge_digest,
            policy_epoch: s.policy_epoch,
            hardware_epoch: s.hardware_epoch,
            issued_at_ms: 1300,
            expires_at_ms: 2000,
        };
        let signed = KagemushaSignedPlayIntegrityRefreshChallengeV1 {
            signature: Signature::new(
                core_issuer.private_key(),
                &challenge.canonical_signing_bytes().unwrap(),
            ),
            challenge,
        };
        let platform = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let der: P256Signature = platform.sign(&challenge.possession_signing_bytes().unwrap());
        let der = der.to_der().as_bytes().to_vec();
        let subject = KagemushaPlayIntegrityRefreshLeaseSubjectV1 {
            version: 1,
            credential_digest: c.digest(),
            challenge_digest: challenge.attempt_id().unwrap(),
            attested_key_id: s.attested_key_id,
            release_id: s.release_id,
            hardware_profile_id: s.hardware_profile_id,
            trust_policy_digest: s.trust_policy_digest,
            app_authority_policy_digest: s.app_authority_policy_digest,
            binding: KagemushaPlayIntegrityBindingV1 {
                request_hash: challenge.request_hash().unwrap(),
                evidence_digest: [60; 32],
                policy_digest: challenge.play_integrity_policy_digest,
                verified_at_ms: 1500,
                refresh_before_ms: 2400,
            },
            possession_original_digest: Sha256::digest(&der).into(),
            policy_epoch: s.policy_epoch,
            hardware_epoch: s.hardware_epoch,
            issued_at_ms: 1500,
            expires_at_ms: 2400,
        };
        let signature = Signature::new(
            issuer.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        );
        let app_possession = KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: der.clone(),
        };
        let circuit_admission = ordinary_test_issuer_admission_v1(
            KagemushaPlayIntegrityRefreshLeaseV1::circuit_admission_subject_for(
                &subject,
                &signature,
                &app_possession,
            )
            .unwrap(),
        );
        (
            signed,
            der,
            KagemushaPlayIntegrityRefreshLeaseV1 {
                subject,
                signature,
                app_possession,
                circuit_admission,
            },
        )
    }
    fn prepare_platform(
        root: &Path,
        financial: &mut KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> (
        KagemushaOrdinaryIntegrityRefreshOwnerV1,
        Vec<u8>,
        KagemushaPlayIntegrityRefreshLeaseV1,
    ) {
        time(financial, 1400);
        assert!(financial.recheck().is_err()); // the original PI has expired; money remains refused.
        let mut owner = KagemushaOrdinaryIntegrityRefreshOwnerV1::create(root, financial).unwrap();
        let prepare = owner.prepare(financial).unwrap();
        let nonce: [u8; 32] = prepare[1].as_slice().try_into().unwrap();
        let (signed, der, lease) = originals(financial, nonce);
        let fields = owner
            .accept_challenge(financial, &signed.to_transport_bytes().unwrap())
            .unwrap();
        assert_eq!(
            fields[1],
            signed.challenge.possession_signing_bytes().unwrap()
        );
        assert_eq!(owner.fence_platform_invocation(financial).unwrap(), fields);
        (owner, der, lease)
    }
    #[test]
    fn overdue_initial_pi_refreshes_exact_original_and_acknowledged_lease_survives_cold_replay() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        let (mut owner, der, lease) = prepare_platform(&root, &mut financial);
        owner.capture_signature(&financial, &der).unwrap();
        let finish = owner
            .retain_integrity_token(&financial, b"untrusted-google-token-fixture")
            .unwrap();
        assert_eq!(finish[2], der);
        assert_eq!(finish[3], b"untrusted-google-token-fixture");
        time(&mut financial, 1500);
        let bytes = lease.canonical_bytes().unwrap();
        let actual = owner.accept_lease(&financial, &bytes).unwrap();
        assert_eq!(actual.original(), bytes);
        assert!(financial.recheck().is_err());
        assert!(Arc::ptr_eq(
            &actual,
            &owner.accept_lease(&financial, &bytes).unwrap()
        ));
        drop(owner);
        time(&mut financial, 2500);
        let mut owner =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing(&root, &financial).unwrap();
        assert_eq!(
            owner.retained_verified_leases(&financial).unwrap()[0].original(),
            bytes
        );
        assert!(owner.accept_lease(&financial, &bytes).is_err()); // historical original grants no expired lease.
    }
    #[test]
    fn invoked_fence_survives_cold_without_signature_and_pending_raw_lease_never_admits() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        let (owner, der, lease) = prepare_platform(&root, &mut financial);
        drop(owner);
        let mut owner =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing(&root, &financial).unwrap();
        assert!(owner.fence_platform_invocation(&financial).is_err());
        owner.capture_signature(&financial, &der).unwrap();
        owner
            .retain_integrity_token(&financial, b"untrusted-google-token-fixture")
            .unwrap();
        let bytes = lease.canonical_bytes().unwrap();
        // Stop at the real raw-fsync boundary, before any authenticated acknowledgement.
        // A lease becoming current while this test runs must not turn that interrupted write
        // into a verified capability during cold replay.
        owner
            .append(&Record::LeasePending {
                original: bytes.clone(),
            })
            .unwrap();
        drop(owner);
        let mut owner =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing(&root, &financial).unwrap();
        assert!(
            owner
                .retained_verified_leases(&financial)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            owner.pending.as_ref().unwrap().lease_pending.as_deref(),
            Some(bytes.as_slice())
        );
        time(&mut financial, 1500);
        assert_eq!(
            owner.accept_lease(&financial, &bytes).unwrap().original(),
            bytes
        );
    }
    #[test]
    fn integrity_refresh_lease_authenticates_both_future_and_expiry_bounds() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        let (mut owner, der, lease) = prepare_platform(&root, &mut financial);
        owner.capture_signature(&financial, &der).unwrap();
        owner
            .retain_integrity_token(&financial, b"untrusted-google-token-fixture")
            .unwrap();
        let original = lease.canonical_bytes().unwrap();
        let pending = owner.pending.as_ref().unwrap();
        // Authentication uses the supplied trusted interval. These exact boundary assertions
        // do not depend on how long signing, fsync or the test scheduler takes.
        assert!(owner.verify_lease(pending, &original, 1400, 1400).is_err());
        assert_eq!(
            owner
                .verify_lease(pending, &original, 1500, 1500)
                .unwrap()
                .original(),
            original
        );
        assert!(owner.verify_lease(pending, &original, 2399, 2400).is_err());
        assert!(owner.verify_lease(pending, &original, 2400, 2400).is_err());
        assert!(owner.verify_lease(pending, &original, 1501, 1500).is_err());
        assert!(owner.leases.is_empty());
    }
    #[test]
    fn late_signature_pending_never_becomes_a_capture_and_abandon_reserves_new_nonce() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        let (mut owner, der, _) = prepare_platform(&root, &mut financial);
        let nonce = owner.prepare(&financial).unwrap()[1].clone();
        time(&mut financial, 2000);
        assert!(owner.capture_signature(&financial, &der).is_err());
        drop(owner);
        let mut owner =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing(&root, &financial).unwrap();
        assert!(owner.pending.as_ref().unwrap().signature.is_none());
        assert!(owner.retain_integrity_token(&financial, b"token").is_err());
        owner.abandon_pending(&financial).unwrap();
        assert_ne!(owner.prepare(&financial).unwrap()[1], nonce);
    }
    #[test]
    fn unsupported_non_pi_profile_and_both_future_or_expired_capture_bounds_refuse() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let non_pi = financial(&root, false);
        assert!(KagemushaOrdinaryIntegrityRefreshOwnerV1::create(&root, &non_pi).is_err());
        assert!(require_interval(0, 1).is_err());
        assert!(require_interval(9, 8).is_err());
        assert!(require_token(b"line\nfeed").is_err());
        assert!(require_der(&[0; 7]).is_err());
    }
    #[test]
    fn integrity_refresh_oversized_record_refuses_before_changing_the_owned_wal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root, true);
        let mut owner =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::create(&root, &financial).unwrap();
        let before = owner.journal.recovery_prefix().unwrap();
        let rows = owner.rows;
        let record = Record::IntegrityToken {
            original: vec![1; usize::try_from(PI_FORMAT.maximum_payload_bytes).unwrap()],
        };
        assert!(
            u64::try_from(norito::encode_canonical(&record).unwrap().len()).unwrap()
                > PI_FORMAT.maximum_payload_bytes
        );
        assert!(owner.append(&record).is_err());
        assert_eq!(owner.rows, rows);
        assert_eq!(owner.journal.recovery_prefix().unwrap(), before);
        drop(owner);
        let restored =
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing(&root, &financial).unwrap();
        assert_eq!(restored.rows, rows);
        assert_eq!(restored.journal.recovery_prefix().unwrap(), before);
    }

    #[test]
    fn integrity_refresh_signature_requires_the_exact_retained_original_challenge() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        let (mut owner, der, _) = prepare_platform(&root, &mut financial);
        owner
            .verify_signature(owner.pending.as_ref().unwrap(), &der)
            .unwrap();
        let mut changed = der.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(
            owner
                .verify_signature(owner.pending.as_ref().unwrap(), &changed)
                .is_err()
        );
        owner
            .pending
            .as_mut()
            .unwrap()
            .challenge
            .as_mut()
            .unwrap()
            .challenge
            .nonce[0] ^= 1;
        assert!(
            owner
                .verify_signature(owner.pending.as_ref().unwrap(), &der)
                .is_err()
        );
        assert!(
            owner
                .verify_signature(&Pending::new([1; 32]), &der)
                .is_err()
        );
    }
}
