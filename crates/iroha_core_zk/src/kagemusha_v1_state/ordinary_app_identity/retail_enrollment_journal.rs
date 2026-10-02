//! Native wallet/FI ceremony over the same consumed app-possession original.
//!
//! A decoded challenge cannot grant signing authority. This holder requires the genuine
//! pending C, final app credential and consumed E, retains the exact challenge before the
//! wallet action, and admits the full FI certificate without creating a State or spend lease.

use super::super::{PrivateJournal, PrivateJournalFormat};
use super::possession_journal::KagemushaOrdinaryAppPossessionAttemptV1;
use super::preparation_reservation::{
    KagemushaOrdinaryPreparationReservationV1, KagemushaOrdinaryPreparationSelectedOriginalsV1,
};
use super::{Custody, KagemushaPendingAppIdentityV1, Rejected, Result};
use iroha_crypto::{Signature, SignatureOf};
use iroha_data_model::kagemusha::*;
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{path::Path, sync::Arc};

const MAX_ROWS: usize = 4;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-retail-enrollment.wal",
    magic: b"KGMORET1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-retail-enrollment-wal\0",
    maximum_payload_bytes: 40 * 1024,
};
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::RetailEnrollmentRecordV1")]
enum Record {
    Prepared {
        ticket: u64,
        pending_scope: [u8; 32],
        credential_digest: [u8; 32],
        challenge_original: Vec<u8>,
        admitted_at_ms: u64,
    },
    Invoked,
    AccountSignature([u8; 64]),
    Certificate {
        original: Vec<u8>,
        authenticated_at_ms: u64,
    },
    Cancelled,
}

/// Descriptor-held wallet invocation and FI-original custody, independent of monetary state.
/// No decoder, raw challenge constructor, clone or signing callback recreates this capability.
pub struct KagemushaOrdinaryRetailEnrollmentAttemptV1 {
    journal: PrivateJournal,
    rows: Vec<Vec<u8>>,
    ticket: u64,
    pending_scope: [u8; 32],
    credential_digest: [u8; 32],
    challenge: KagemushaOrdinaryRetailEnrollmentChallengeV1,
    challenge_original: Vec<u8>,
    admitted_at_ms: u64,
    signature: Option<[u8; 64]>,
    enrollment: Option<Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>>,
    completed_at_ms: Option<u64>,
    stage: u8,
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
}
impl KagemushaOrdinaryRetailEnrollmentAttemptV1 {
    /// Authenticate and fsync the exact FI challenge before exposing its wallet signing digest.
    /// Time and the directory originate in the retained native provisioning owner, not frames.
    /// # Errors
    /// Rejects foreign challenge/message/attempt, stale originals or existing/uncertain storage.
    pub fn create(
        root: &Path,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        reservation: &KagemushaOrdinaryPreparationReservationV1,
        original: &[u8],
        offered_message: [u8; 32],
    ) -> Result<Self> {
        let selected = reservation.selected_originals()?.clone();
        let now = selected.trusted_time_ms()?;
        if reservation.retained_prepared_owner()?.native_scope != pending.preparation.native_scope {
            return Err(Rejected);
        }
        let challenge = decode_challenge(original)?;
        let app = possession.final_identity(pending, now)?;
        let selection = selection(
            pending,
            app,
            selected.core_authorization_key_reference()?,
            now,
        )?;
        challenge
            .validate(
                &selection,
                &pending.preparation.issuer,
                &pending.preparation.release,
                app,
                now,
            )
            .map_err(|_| Rejected)?;
        if challenge.account_signing_message().map_err(|_| Rejected)? != offered_message {
            return Err(Rejected);
        }
        let mut random = [0; 8];
        OsRng.try_fill_bytes(&mut random).map_err(|_| Custody)?;
        let ticket = u64::from_le_bytes(random);
        if ticket == 0 {
            return Err(Custody);
        }
        let mut this = Self {
            journal: PrivateJournal::create_new(root, FORMAT).map_err(|_| Custody)?,
            rows: Vec::new(),
            ticket,
            pending_scope: pending.native_scope(),
            credential_digest: app.digest(),
            challenge,
            challenge_original: original.to_vec(),
            admitted_at_ms: now,
            signature: None,
            enrollment: None,
            completed_at_ms: None,
            stage: 0,
            selected,
        };
        this.append(Record::Prepared {
            ticket,
            pending_scope: this.pending_scope,
            credential_digest: this.credential_digest,
            challenge_original: original.to_vec(),
            admitted_at_ms: now,
        })?;
        this.recheck(pending, possession)?;
        Ok(this)
    }
    /// Reopen the same complete original prefix. An invocation without its signature stays closed.
    /// Completed originals are authenticated at their retained native instant and checked now.
    /// # Errors
    /// Rejects missing/torn/reordered records, changed originals, issuer or current credential.
    pub fn open_existing(
        root: &Path,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        reservation: &KagemushaOrdinaryPreparationReservationV1,
    ) -> Result<Self> {
        let selected = reservation.selected_originals()?.clone();
        let now = selected.trusted_time_ms()?;
        if reservation.retained_prepared_owner()?.native_scope != pending.preparation.native_scope {
            return Err(Rejected);
        }
        let mut journal = PrivateJournal::open_existing(root, FORMAT).map_err(|_| Custody)?;
        replay_complete_bounded(&mut journal)?;
        let mut rows = Vec::new();
        journal
            .scan_complete(|_, raw| {
                if rows.len() == MAX_ROWS {
                    return Err(super::super::PrivateJournalError::Corrupt);
                }
                rows.push(raw.to_vec());
                Ok(())
            })
            .map_err(|_| Custody)?;
        let records = rows
            .iter()
            .map(|r| decode_record(r))
            .collect::<Result<Vec<_>>>()?;
        let Some(Record::Prepared {
            ticket,
            pending_scope,
            credential_digest,
            challenge_original,
            admitted_at_ms,
        }) = records.first()
        else {
            return Err(Custody);
        };
        if *ticket == 0 || *pending_scope != pending.native_scope() || *admitted_at_ms > now {
            return Err(Custody);
        }
        let mut this = Self {
            journal,
            rows,
            ticket: *ticket,
            pending_scope: *pending_scope,
            credential_digest: *credential_digest,
            challenge: decode_challenge(challenge_original)?,
            challenge_original: challenge_original.clone(),
            admitted_at_ms: *admitted_at_ms,
            signature: None,
            enrollment: None,
            completed_at_ms: None,
            stage: 0,
            selected,
        };
        for record in records.into_iter().skip(1) {
            match (this.stage, record) {
                (0, Record::Invoked) => this.stage = 1,
                (0, Record::Cancelled) => this.stage = 4,
                (1, Record::AccountSignature(signature)) => {
                    this.signature = Some(signature);
                    this.stage = 2;
                }
                (
                    2,
                    Record::Certificate {
                        original,
                        authenticated_at_ms,
                    },
                ) => {
                    if authenticated_at_ms < this.admitted_at_ms || authenticated_at_ms > now {
                        return Err(Custody);
                    }
                    this.enrollment = Some(Arc::new(this.authenticate_certificate(
                        pending,
                        possession,
                        &original,
                        authenticated_at_ms,
                    )?));
                    this.completed_at_ms = Some(authenticated_at_ms);
                    this.stage = 3;
                }
                _ => return Err(Custody),
            }
        }
        this.recheck(pending, possession)?;
        Ok(this)
    }
    /// Exact native ticket; public bytes do not recreate the holder.
    pub const fn ticket(&self) -> u64 {
        self.ticket
    }
    fn now(&self) -> Result<u64> {
        self.selected.trusted_time_ms()
    }
    fn append(&mut self, record: Record) -> Result<()> {
        let raw = norito::encode_canonical(&record).map_err(|_| Rejected)?;
        self.journal.append(&raw).map_err(|_| Custody)?;
        self.rows.push(raw);
        Ok(())
    }
    /// Recheck original descriptors, actual pending credential and exact original challenge.
    /// # Errors
    /// Rejects current expiry, cancellation, changed native scope or held WAL bytes.
    pub fn recheck(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<()> {
        self.selected
            .trusted_time_interval()?
            .check_both(|now| self.recheck_at_reference(pending, possession, now))
    }
    fn recheck_at_reference(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        now: u64,
    ) -> Result<()> {
        if self.stage == 4 || self.pending_scope != pending.native_scope() {
            return Err(Custody);
        }
        let app = possession.final_identity(pending, now)?;
        if app.digest() != self.credential_digest {
            return Err(Custody);
        }
        let check_at = self.completed_at_ms.unwrap_or(now);
        let historical_app = pending.authenticate_final_credential(app.original(), check_at)?;
        let selected = selection(
            pending,
            &historical_app,
            self.selected.core_authorization_key_reference()?,
            check_at,
        )?;
        self.challenge
            .validate(
                &selected,
                &pending.preparation.issuer,
                &pending.preparation.release,
                &historical_app,
                check_at,
            )
            .map_err(|_| Custody)?;
        if self.challenge.canonical_bytes().map_err(|_| Custody)? != self.challenge_original {
            return Err(Custody);
        }
        let mut count = 0;
        self.journal
            .scan_complete(|_, raw| {
                if self.rows.get(count).map(Vec::as_slice) != Some(raw) {
                    return Err(super::super::PrivateJournalError::Corrupt);
                }
                count += 1;
                Ok(())
            })
            .map_err(|_| Custody)?;
        if count != self.rows.len() {
            return Err(Custody);
        }
        if let Some(enrollment) = &self.enrollment {
            enrollment
                .recheck_at_trusted_time(now)
                .map_err(|_| Custody)?;
        }
        Ok(())
    }
    /// Return only this held challenge and independently derived HashOf digest before wallet action.
    /// # Errors
    /// Rejects stale/changed native custody.
    pub fn preparation_fields(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, possession)?;
        let fields = vec![
            self.ticket.to_le_bytes().to_vec(),
            self.challenge_original.clone(),
            self.challenge
                .account_signing_message()
                .map_err(|_| Rejected)?
                .to_vec(),
            self.pending_scope.to_vec(),
            self.credential_digest.to_vec(),
        ];
        self.recheck(pending, possession)?;
        Ok(fields)
    }
    /// Fsync one wallet invocation fence. A lost original cannot invoke the wallet again.
    /// # Errors
    /// Rejects stale originals or an unknown previous invocation.
    pub fn fence(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, possession)?;
        match self.stage {
            0 => {
                self.append(Record::Invoked)?;
                self.stage = 1;
                Ok(vec![vec![1], vec![]])
            }
            1 => Err(super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome),
            2 | 3 => Ok(vec![vec![2], self.signature.ok_or(Custody)?.to_vec()]),
            _ => Err(Rejected),
        }
    }
    /// Verify and retain the original wallet Ed64 after the held invocation, before HTTP exposure.
    /// # Errors
    /// Rejects wrong key/message, new signatures, invalid platform originals or uncertain append.
    pub fn retain_account_signature(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        raw: [u8; 64],
    ) -> Result<[u8; 32]> {
        self.recheck(pending, possession)?;
        if self.stage == 1 {
            self.possession_proof(pending, possession, raw, self.now()?)?;
            self.recheck(pending, possession)?;
            self.append(Record::AccountSignature(raw))?;
            self.signature = Some(raw);
            self.stage = 2;
        } else if !matches!(self.stage, 2 | 3) || self.signature != Some(raw) {
            return Err(Rejected);
        }
        self.recheck(pending, possession)?;
        Ok(Sha256::digest(raw).into())
    }
    fn possession_proof(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        raw: [u8; 64],
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentPossessionV1> {
        let app = possession.final_identity(pending, self.now()?)?;
        let raw_platform = possession.original_platform_evidence(pending, self.now()?)?;
        let evidence = match app.subject().platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: raw_platform.to_vec(),
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                    raw_assertion: raw_platform.to_vec(),
                }
            }
            _ => return Err(Rejected),
        };
        KagemushaOrdinaryRetailEnrollmentPossessionProofV1 {
            challenge: self.challenge.clone(),
            account_signature: SignatureOf::from_signature(Signature::from_bytes(&raw)),
            raw_attestation: pending.raw_attestation().to_vec(),
            app_possession: evidence,
        }
        .authenticate(
            &self.challenge,
            &selection(
                pending,
                app,
                self.selected.core_authorization_key_reference()?,
                now,
            )?,
            &pending.preparation.issuer,
            &pending.preparation.release,
            app,
            app.subject()
                .platform_class
                .eq(&KagemushaHardwarePlatformClassV1::AppleAppAttest)
                .then_some(0),
            now,
        )
        .map_err(|_| Rejected)
    }
    fn authenticate_certificate(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        raw: &[u8],
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1> {
        if raw.is_empty() || raw.len() > 16 * 1024 {
            return Err(Rejected);
        }
        let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 =
            norito::decode_from_bytes(raw).map_err(|_| Rejected)?;
        if certificate.canonical_bytes().map_err(|_| Rejected)? != raw {
            return Err(Rejected);
        }
        let app = possession.final_identity(pending, self.now()?)?;
        let selected = selection(
            pending,
            app,
            self.selected.core_authorization_key_reference()?,
            now,
        )?;
        let owned_app = pending.authenticate_final_credential(app.original(), now)?;
        let proof =
            self.possession_proof(pending, possession, self.signature.ok_or(Custody)?, now)?;
        certificate
            .authenticate(
                &selected,
                &pending.preparation.issuer,
                &pending.preparation.release,
                owned_app,
                proof,
                now,
            )
            .map_err(|_| Rejected)
    }
    /// Admit and fsync only the full actual FI certificate under this exact wallet/app ceremony.
    /// # Errors
    /// Rejects missing wallet original, foreign FI signature/scope or replacement completion.
    pub fn accept_certificate(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
        raw: &[u8],
    ) -> Result<Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>> {
        self.recheck(pending, possession)?;
        if self.stage == 3 {
            let held = self.enrollment.as_ref().ok_or(Custody)?;
            if held.certificate().canonical_bytes().map_err(|_| Custody)? != raw {
                return Err(Rejected);
            }
            return Ok(held.clone());
        }
        if self.stage != 2 {
            return Err(Rejected);
        }
        let now = self.now()?;
        let verified = self.authenticate_certificate(pending, possession, raw, now)?;
        self.recheck(pending, possession)?;
        let fresh = self.now()?;
        verified
            .recheck_at_trusted_time(fresh)
            .map_err(|_| Custody)?;
        self.append(Record::Certificate {
            original: raw.to_vec(),
            authenticated_at_ms: now,
        })?;
        self.completed_at_ms = Some(now);
        self.enrollment = Some(Arc::new(verified));
        self.stage = 3;
        self.recheck(pending, possession)?;
        Ok(self.enrollment.as_ref().ok_or(Custody)?.clone())
    }
    /// Borrow the same fully authenticated FI token retained by this exact native ceremony.
    /// # Errors
    /// Rejects missing completion, expired current admission or changed original custody.
    pub fn enrolled_original(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<&Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>> {
        self.recheck(pending, possession)?;
        self.enrollment.as_ref().ok_or(Custody)
    }
    /// Read exact retained action originals without invoking a signer or issuer again.
    /// # Errors
    /// Rejects stale/changed native custody.
    pub fn recovery_fields(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, possession)?;
        Ok(vec![
            vec![self.stage],
            self.signature.map(|s| s.to_vec()).unwrap_or_default(),
            self.enrollment
                .as_ref()
                .map(|e| e.certificate().canonical_bytes().map_err(|_| Custody))
                .transpose()?
                .unwrap_or_default(),
        ])
    }
    /// Cancel only an uninvoked action; uncertain or signed effects cannot be discarded.
    /// # Errors
    /// Rejects previously invoked, cancelled or stale custody.
    pub fn cancel(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        possession: &KagemushaOrdinaryAppPossessionAttemptV1,
    ) -> Result<()> {
        self.recheck(pending, possession)?;
        if self.stage != 0 {
            return Err(Rejected);
        }
        self.append(Record::Cancelled)?;
        self.stage = 4;
        Ok(())
    }
}
fn selection(
    pending: &KagemushaPendingAppIdentityV1,
    app: &KagemushaVerifiedOrdinaryAppCredentialV1,
    core_key_reference: [u8; 32],
    now: u64,
) -> Result<KagemushaOrdinaryRetailEnrollmentSelectionV1> {
    pending.recheck_retained_originals_at_trusted_time(now)?;
    let prepared = &pending.preparation;
    let credential: KagemushaOrdinaryAppCredentialV1 =
        norito::decode_from_bytes(app.original()).map_err(|_| Rejected)?;
    if credential.canonical_bytes().map_err(|_| Rejected)? != app.original() {
        return Err(Rejected);
    }
    Ok(KagemushaOrdinaryRetailEnrollmentSelectionV1 {
        owner: prepared.owner.clone(),
        issuance: KagemushaOrdinaryRetailEnrollmentIssuanceV1 {
            release_id: prepared.release.release_id(),
            hardware_policy_digest: prepared.release.hardware_policy_digest(),
            core_authorization_key_reference: core_key_reference,
            credential,
        },
        preparation: prepared.preparation.clone(),
    })
}
fn decode_challenge(raw: &[u8]) -> Result<KagemushaOrdinaryRetailEnrollmentChallengeV1> {
    if raw.is_empty() || raw.len() > 32 * 1024 {
        return Err(Rejected);
    }
    let value: KagemushaOrdinaryRetailEnrollmentChallengeV1 =
        norito::decode_from_bytes(raw).map_err(|_| Rejected)?;
    if value.canonical_bytes().map_err(|_| Rejected)? != raw {
        return Err(Rejected);
    }
    Ok(value)
}
// Establish byte custody of the complete bounded prefix before positional scans. This
// admits no retail owner: exact originals, stage semantics and authority are checked below.
fn replay_complete_bounded(journal: &mut PrivateJournal) -> Result<()> {
    for index in 0..=MAX_ROWS {
        let Some((sequence, raw)) = journal.replay_next().map_err(|_| Custody)? else {
            return if index == 0 { Err(Custody) } else { Ok(()) };
        };
        if index == MAX_ROWS || sequence != index as u64 {
            return Err(Custody);
        }
        decode_record(&raw)?;
    }
    Err(Custody)
}

fn decode_record(raw: &[u8]) -> Result<Record> {
    let value: Record = norito::decode_from_bytes(raw).map_err(|_| Custody)?;
    if norito::encode_canonical(&value).map_err(|_| Custody)? != raw {
        return Err(Custody);
    }
    Ok(value)
}

#[cfg(test)]
#[path = "retail_enrollment_journal_tests.rs"]
mod tests;
