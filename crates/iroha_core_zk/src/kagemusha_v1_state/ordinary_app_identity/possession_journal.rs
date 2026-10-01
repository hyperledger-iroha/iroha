//! Native E20 possession custody under the same genuinely admitted pending C21 raw original.
//! This WAL retains one platform invocation/original/consumption and one authenticated final
//! app identity original. It grants no hardware monotonicity, StateGuard or monetary authority.
//! Unknown effects never re-sign.
use super::super::{PrivateJournal, PrivateJournalFormat};
use super::{Custody, KagemushaPendingAppIdentityV1, Rejected, Result};
use iroha_data_model::kagemusha::*;
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::path::Path;

const MAX_ROWS: usize = 5;
const MAX_FRAME: usize = 20 * 1024;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-app-possession.wal",
    magic: b"KGMAPOP1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-app-possession-wal\0",
    maximum_payload_bytes: MAX_FRAME as u64,
};
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::PossessionRecordV1")]
#[expect(
    variant_size_differences,
    reason = "The bounded WAL retains the original fixed-size challenge inline without a replacement heap allocation"
)]
enum Record {
    Prepared {
        ticket: u64,
        pending_scope: [u8; 32],
        challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    },
    Invoked,
    Original(Vec<u8>),
    Consumed {
        checked_at_ms: u64,
        counter: Option<u32>,
        receipt: Vec<u8>,
    },
    Cancelled,
    FinalCredential {
        original: Vec<u8>,
        published_at_ms: u64,
    },
}
/// Descriptor-held non-cloneable possession attempt; every operation borrows actual pending C21.
/// Decoding a message, platform callback or an issuer DTO cannot construct this owner.
pub struct KagemushaOrdinaryAppPossessionAttemptV1 {
    journal: PrivateJournal,
    ticket: u64,
    pending_scope: [u8; 32],
    challenge: KagemushaAppEnrollmentPossessionChallengeV1,
    stage: u8,
    raw_original: Option<Vec<u8>>,
    receipt: Option<Vec<u8>>,
    consumed_at_ms: Option<u64>,
    final_credential: Option<KagemushaVerifiedOrdinaryAppCredentialV1>,
}
impl KagemushaOrdinaryAppPossessionAttemptV1 {
    /// Durably prepare only from the genuine pending issuer original and native current time.
    /// # Errors
    /// Refuses existing storage, uncertain originals, expiry or unavailable native randomness.
    pub fn create(root: &Path, pending: &KagemushaPendingAppIdentityV1, now: u64) -> Result<Self> {
        pending.recheck_at_trusted_time(now)?;
        let challenge = pending.possession_challenge(now)?;
        let mut entropy = [0; 8];
        OsRng.try_fill_bytes(&mut entropy).map_err(|_| Custody)?;
        let ticket = u64::from_le_bytes(entropy);
        if ticket == 0 {
            return Err(Custody);
        }
        let pending_scope = pending.native_scope();
        let mut this = Self {
            journal: PrivateJournal::create_new(root, FORMAT).map_err(|_| Custody)?,
            ticket,
            pending_scope,
            challenge,
            stage: 0,
            raw_original: None,
            receipt: None,
            consumed_at_ms: None,
            final_credential: None,
        };
        this.append(Record::Prepared {
            ticket,
            pending_scope,
            challenge,
        })?;
        this.recheck(pending, now)?;
        Ok(this)
    }
    /// Recover the exact bounded complete WAL under the same genuine pending raw owner.
    /// Historical signatures use retained native consumption time. An expired C permits only
    /// already consumed originals and current final-credential admission/recovery; incomplete
    /// hardware/signature prefixes remain unavailable after their original interval.
    /// # Errors
    /// Refuses changed E/scope, malformed or torn prefixes, extra rows or invalid old signatures.
    pub fn open_existing(
        root: &Path,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<Self> {
        pending.recheck_retained_originals_at_trusted_time(now)?;
        let challenge = pending.retained_possession_challenge(now)?;
        let pending_scope = pending.native_scope();
        let mut journal = PrivateJournal::open_existing(root, FORMAT).map_err(|_| Custody)?;
        let mut rows = replay_bounded(&mut journal)?.into_iter();
        let Some(Record::Prepared {
            ticket,
            pending_scope: selected,
            challenge: retained,
        }) = rows.next()
        else {
            return Err(Custody);
        };
        if ticket == 0 || selected != pending_scope || retained != challenge {
            return Err(Custody);
        }
        let mut this = Self {
            journal,
            ticket,
            pending_scope,
            challenge,
            stage: 0,
            raw_original: None,
            receipt: None,
            consumed_at_ms: None,
            final_credential: None,
        };
        for record in rows {
            match record {
                Record::Invoked if this.stage == 0 => this.stage = 1,
                Record::Original(raw) if this.stage == 1 => {
                    check_raw(pending, &raw)?;
                    this.raw_original = Some(raw);
                    this.stage = 2;
                }
                Record::Consumed {
                    checked_at_ms,
                    counter,
                    receipt,
                } if this.stage == 2 => {
                    if checked_at_ms > now {
                        return Err(Custody);
                    }
                    let checked = this.verify(pending, checked_at_ms)?;
                    if checked.app_attest_counter() != counter
                        || receipt != this.make_receipt(counter)?
                    {
                        return Err(Custody);
                    }
                    this.receipt = Some(receipt);
                    this.consumed_at_ms = Some(checked_at_ms);
                    this.stage = 3;
                }
                Record::FinalCredential {
                    original,
                    published_at_ms,
                } if this.stage == 3 => {
                    if published_at_ms > now
                        || published_at_ms < this.consumed_at_ms.ok_or(Custody)?
                    {
                        return Err(Custody);
                    }
                    this.final_credential =
                        Some(this.verify_final(pending, &original, published_at_ms)?);
                    this.stage = 5;
                }
                Record::Cancelled if this.stage == 0 => this.stage = 4,
                _ => return Err(Custody),
            }
        }
        if this.stage == 3 {
            this.recheck_consumed_originals(pending, now)?;
        } else {
            this.recheck(pending, now)?;
        }
        Ok(this)
    }
    /// Native ticket only; it grants no scope without the still-held native pending owner.
    pub const fn ticket(&self) -> u64 {
        self.ticket
    }
    /// Recheck actual current C21 raw owner, exact E and owned WAL before exposing/calling anything.
    /// # Errors
    /// Rejects stale/changed/uncertain originals or a cancelled attempt.
    pub fn recheck(&self, pending: &KagemushaPendingAppIdentityV1, now: u64) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        if matches!(self.stage, 3 | 5) {
            self.recheck_consumed_originals(pending, now)?;
            if self.stage == 5 {
                self.final_credential
                    .as_ref()
                    .ok_or(Custody)?
                    .recheck_at_trusted_time(now)
                    .map_err(|_| Custody)?;
            }
        } else {
            pending.recheck_at_trusted_time(now)?;
            if self.stage == 4
                || self.pending_scope != pending.native_scope()
                || self.challenge != pending.possession_challenge(now)?
            {
                return Err(Custody);
            }
        }
        Ok(())
    }
    fn recheck_consumed_originals(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        pending.recheck_retained_originals_at_trusted_time(now)?;
        if !matches!(self.stage, 3 | 5)
            || self.pending_scope != pending.native_scope()
            || self.challenge != pending.retained_possession_challenge(now)?
        {
            return Err(Custody);
        }
        let checked_at = self.consumed_at_ms.ok_or(Custody)?;
        if checked_at > now {
            return Err(Custody);
        }
        let verified = self.verify(pending, checked_at)?;
        if self.receipt.as_deref()
            != Some(self.make_receipt(verified.app_attest_counter())?.as_slice())
        {
            return Err(Custody);
        }
        Ok(())
    }
    /// Exactly fourteen E20 fields from actual admitted C/key/alias/policy and this durable attempt.
    /// # Errors
    /// Rejects a foreign selector, stale owner or unavailable original point/policy.
    pub fn preparation_fields(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        selector: [u8; 32],
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        if self.stage == 3 {
            self.recheck_consumed_originals(pending, now)?;
        } else {
            self.recheck(pending, now)?;
        }
        if selector != self.challenge.enrollment_attempt_id {
            return Err(Rejected);
        }
        let c = pending.preparation().retained_preparation(now)?;
        let platform = vec![match c.challenge.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => 5,
            KagemushaHardwarePlatformClassV1::AppleAppAttest => 4,
            _ => return Err(Rejected),
        }];
        let raw = pending.raw_admission().subject();
        let floor = if platform == [4] {
            0u32.to_le_bytes().to_vec()
        } else {
            Vec::new()
        };
        let fields = vec![
            self.ticket.to_le_bytes().to_vec(),
            self.challenge
                .canonical_signing_bytes()
                .map_err(|_| Rejected)?,
            platform,
            pending.original_alias().as_bytes().to_vec(),
            c.challenge
                .attestation_challenge()
                .map_err(|_| Rejected)?
                .to_vec(),
            raw.app_public_key.as_sec1_bytes().to_vec(),
            raw.attested_key_id.to_vec(),
            c.challenge
                .canonical_signing_bytes()
                .map_err(|_| Rejected)?,
            Vec::new(),
            self.pending_scope.to_vec(),
            floor,
            vec![pending.preparation().allowed_levels_mask],
            raw.app_signing_identity_digest.to_vec(),
            Vec::new(),
        ];
        if self.stage == 3 {
            self.recheck_consumed_originals(pending, now)?;
        } else {
            self.recheck(pending, now)?;
        }
        Ok(fields)
    }
    /// Fsync the single invocation fence before a platform call; an unknown outcome never repeats.
    /// # Errors
    /// Rejects an already invoked but unretained original, stale owner or append uncertainty.
    pub fn fence(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, now)?;
        pending.recheck_at_trusted_time(now)?;
        let result = match self.stage {
            0 => {
                self.append(Record::Invoked)?;
                self.stage = 1;
                vec![vec![1], vec![], vec![]]
            }
            1 => return Err(super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome),
            2 => vec![vec![2], self.raw_original.clone().ok_or(Custody)?, vec![]],
            3 | 5 => vec![
                vec![3],
                self.raw_original.clone().ok_or(Custody)?,
                self.receipt.clone().ok_or(Custody)?,
            ],
            _ => return Err(Rejected),
        };
        self.recheck(pending, now)?;
        Ok(result)
    }
    /// Retain byte-for-byte original DER/CBOR durably before cryptographic consumption.
    /// # Errors
    /// Rejects an uninvoked/new original, another platform or append uncertainty.
    pub fn retain(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        original: &[u8],
        now: u64,
    ) -> Result<[u8; 32]> {
        self.recheck(pending, now)?;
        pending.recheck_at_trusted_time(now)?;
        check_raw(pending, original)?;
        if self.stage == 1 {
            self.append(Record::Original(original.to_vec()))?;
            self.raw_original = Some(original.to_vec());
            self.stage = 2;
        } else if !matches!(self.stage, 2 | 3 | 5) || self.raw_original.as_deref() != Some(original)
        {
            return Err(Rejected);
        }
        self.recheck(pending, now)?;
        Ok(Sha256::digest(original).into())
    }
    /// Verify actual platform signature under native E/key/RP/counter/time, then fsync completion.
    /// This receipt is non-monetary and cannot activate a final identity credential.
    /// # Errors
    /// Rejects missing/changed actual signature, key/application/time, invalid counter or custody.
    pub fn consume(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<Vec<u8>> {
        let before = super::journal::continuous_clock::Reading::now()?;
        self.recheck(pending, now)?;
        pending.recheck_at_trusted_time(now)?;
        if self.stage == 2 {
            let checked = self.verify(pending, now)?;
            let counter = checked.app_attest_counter();
            let receipt = self.make_receipt(counter)?;
            // Crypto/storage admission uses a fresh suspend-inclusive Native instant.
            // The recorded admission cannot reuse the value sampled before verification.
            let checked_at_ms = now
                .checked_add(super::journal::continuous_clock::Reading::now()?.elapsed_ms(before)?)
                .ok_or(Custody)?;
            pending.recheck_at_trusted_time(checked_at_ms)?;
            self.append(Record::Consumed {
                checked_at_ms,
                counter,
                receipt: receipt.clone(),
            })?;
            self.receipt = Some(receipt);
            self.consumed_at_ms = Some(checked_at_ms);
            self.stage = 3;
        } else if !matches!(self.stage, 3 | 5) {
            return Err(Rejected);
        }
        let after = now
            .checked_add(super::journal::continuous_clock::Reading::now()?.elapsed_ms(before)?)
            .ok_or(Custody)?;
        self.recheck(pending, after)?;
        self.receipt.clone().ok_or(Custody)
    }
    /// Recover only the same retained platform original/receipt. Never invokes hardware.
    /// # Errors
    /// Rejects stale/uncertain native C21 custody.
    pub fn recovery_fields(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, now)?;
        Ok(match self.stage {
            0 => vec![vec![0], vec![], vec![]],
            1 => return Err(super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome),
            2 => vec![vec![1], self.raw_original.clone().ok_or(Custody)?, vec![]],
            3 | 5 => vec![
                vec![2],
                self.raw_original.clone().ok_or(Custody)?,
                self.receipt.clone().ok_or(Custody)?,
            ],
            _ => return Err(Rejected),
        })
    }
    /// Current native signing scope and exact message digest, with no caller-selected time.
    /// # Errors
    /// Rejects stale/uncertain owner or a changed pending original.
    pub fn recheck_fields(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck(pending, now)?;
        Ok(vec![
            self.pending_scope.to_vec(),
            Sha256::digest(
                self.challenge
                    .canonical_signing_bytes()
                    .map_err(|_| Rejected)?,
            )
            .to_vec(),
        ])
    }
    /// Cancel only before any platform invocation. Unknown effects cannot be reset.
    /// # Errors
    /// Rejects any invocation/signature/completed state or uncertain append.
    pub fn cancel(&mut self, pending: &KagemushaPendingAppIdentityV1, now: u64) -> Result<()> {
        self.recheck(pending, now)?;
        if self.stage != 0 {
            return Err(Rejected);
        }
        self.append(Record::Cancelled)?;
        self.stage = 4;
        Ok(())
    }
    /// Authenticate and fsync one original signed final app identity after durable verified E.
    /// A retry must supply identical bytes. This creates no FI enrollment, State or money grant.
    /// Current policies, credential and Integrity remain current; original E is checked at its
    /// durably recorded consumption time, never at a caller timestamp or renewed C interval.
    /// # Errors
    /// Rejects unconsumed E, issuer/evidence/security/counter drift, stale current policies,
    /// replacement credential or uncertain append. No new issuer issuance is requested here.
    pub fn accept_final_credential(
        &mut self,
        pending: &KagemushaPendingAppIdentityV1,
        original: &[u8],
        now: u64,
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck_consumed_originals(pending, now)?;
        let checked = self.verify_final(pending, original, now)?;
        if self.stage == 5 {
            if self.final_credential.as_ref().ok_or(Custody)?.original() != original {
                return Err(Rejected);
            }
        } else {
            self.append(Record::FinalCredential {
                original: original.to_vec(),
                published_at_ms: now,
            })?;
            self.final_credential = Some(checked);
            self.stage = 5;
        }
        self.recheck(pending, now)?;
        Ok(vec![
            self.final_credential
                .as_ref()
                .ok_or(Custody)?
                .digest()
                .to_vec(),
            self.pending_scope.to_vec(),
        ])
    }
    /// Borrow the actual admitted current final app identity; possession receipt alone is insufficient.
    /// # Errors
    /// Rejects absent final publication, current credential/Integrity expiry or original drift.
    pub fn final_identity<'a>(
        &'a self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<&'a KagemushaVerifiedOrdinaryAppCredentialV1> {
        self.recheck(pending, now)?;
        self.final_credential.as_ref().ok_or(Rejected)
    }
    /// Borrow only the consumed original platform signature under the same descriptor custody.
    /// This read-only projection does not authorize another platform invocation.
    /// # Errors
    /// Rejects unconsumed, substituted or stale pending possession originals.
    pub fn original_platform_evidence<'a>(
        &'a self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<&'a [u8]> {
        self.recheck_consumed_originals(pending, now)?;
        self.raw_original.as_deref().ok_or(Custody)
    }
    fn verify_final(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        original: &[u8],
        now: u64,
    ) -> Result<KagemushaVerifiedOrdinaryAppCredentialV1> {
        let checked_at = self.consumed_at_ms.ok_or(Custody)?;
        let possession = self.verify(pending, checked_at)?;
        let raw = pending.raw_admission().subject();
        let checked = pending.authenticate_final_credential(original, now)?;
        let s = checked.subject();
        if s.issued_at_ms < checked_at
            || s.security_level != raw.security_level
            || s.attested_key_id != raw.attested_key_id
            || s.app_public_key != raw.app_public_key
            || s.app_signing_identity_digest != raw.app_signing_identity_digest
            || s.app_attest_counter_floor != possession.app_attest_counter().unwrap_or(0)
            || s.platform_evidence_digest
                != kagemusha_ordinary_app_enrollment_evidence_digest_v1(
                    pending.raw_attestation(),
                    self.raw_original.as_deref().ok_or(Custody)?,
                )
                .map_err(|_| Rejected)?
        {
            return Err(Rejected);
        }
        Ok(checked)
    }
    fn verify(
        &self,
        pending: &KagemushaPendingAppIdentityV1,
        now: u64,
    ) -> Result<KagemushaVerifiedAppEnrollmentPossessionV1> {
        let raw = self.raw_original.as_ref().ok_or(Custody)?;
        let subject = pending.raw_admission().subject();
        let evidence = match subject.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: raw.clone(),
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                    raw_assertion: raw.clone(),
                }
            }
            _ => return Err(Rejected),
        };
        KagemushaAppEnrollmentPossessionV1 {
            challenge: self.challenge,
            evidence,
        }
        .authenticate(
            &self.challenge,
            &subject.app_public_key,
            subject.platform_class,
            subject.app_signing_identity_digest,
            pending.preparation().app_authority().app_release_digest,
            (subject.platform_class == KagemushaHardwarePlatformClassV1::AppleAppAttest)
                .then_some(0),
            now,
        )
        .map_err(|_| Rejected)
    }
    fn make_receipt(&self, counter: Option<u32>) -> Result<Vec<u8>> {
        let original = self.raw_original.as_ref().ok_or(Custody)?;
        let mut bytes = b"KGMAPP1\0".to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.push(2);
        bytes.extend_from_slice(&self.ticket.to_le_bytes());
        for digest in [
            self.challenge.enrollment_attempt_id,
            self.pending_scope,
            Sha256::digest(
                self.challenge
                    .canonical_signing_bytes()
                    .map_err(|_| Rejected)?,
            )
            .into(),
            Sha256::digest(original).into(),
            self.pending_scope,
        ] {
            bytes.extend_from_slice(&digest);
        }
        bytes.push(u8::from(counter.is_some()));
        bytes.extend_from_slice(&counter.unwrap_or(0).to_le_bytes());
        if bytes.len() != 184 {
            return Err(Custody);
        }
        Ok(bytes)
    }
    fn append(&mut self, row: Record) -> Result<()> {
        self.journal.append(&encode(&row)?).map_err(|_| Custody)
    }
}
fn check_raw(pending: &KagemushaPendingAppIdentityV1, raw: &[u8]) -> Result<()> {
    let valid = match pending.raw_admission().subject().platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => (8..=72).contains(&raw.len()),
        KagemushaHardwarePlatformClassV1::AppleAppAttest => !raw.is_empty() && raw.len() <= 4096,
        _ => false,
    };
    if valid { Ok(()) } else { Err(Rejected) }
}
fn encode(row: &Record) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(row).map_err(|_| Custody)?;
    if bytes.len() > MAX_FRAME {
        return Err(Custody);
    }
    Ok(bytes)
}
fn replay_bounded(journal: &mut PrivateJournal) -> Result<Vec<Record>> {
    let mut records = Vec::with_capacity(MAX_ROWS);
    for index in 0..=MAX_ROWS {
        let Some((sequence, bytes)) = journal.replay_next().map_err(|_| Custody)? else {
            return if records.is_empty() {
                Err(Custody)
            } else {
                Ok(records)
            };
        };
        if index == MAX_ROWS || sequence != index as u64 {
            return Err(Custody);
        }
        let row = norito::decode_canonical_with_limits(
            &bytes,
            norito::DecodeLimits::new(MAX_FRAME, MAX_FRAME, MAX_FRAME * 2, MAX_FRAME * 8, 32),
        )
        .map_err(|_| Custody)?;
        if encode(&row)? != bytes {
            return Err(Custody);
        }
        records.push(row);
    }
    Err(Custody)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
    use std::sync::Arc;
    fn pending() -> KagemushaPendingAppIdentityV1 {
        let f = Fixture::new(false);
        let c = &f.selection.preparation.challenge;
        let prepared =
            super::super::KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
                f.selection.preparation.clone(),
                f.selection.owner.clone(),
                f.release.clone(),
                f.trust.clone(),
                f.app_authority.clone(),
                f.issuer_policy.clone(),
                c.hardware_profile_id,
                c.client_nonce,
                c.financial_authority_commitment,
                c.hardware_epoch,
                300,
            )
            .unwrap();
        let app = f.selection.issuance.credential.subject;
        // Synthetic full raw original isolates real issuer/key/signature/custody joins, not device qualification.
        let raw = vec![23; 100];
        let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: app.platform_class,
            security_level: app.security_level,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&raw).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let admission = KagemushaRawAppAttestationAdmissionV1 {
            subject,
            signature: Signature::try_new(
                issuer.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        };
        let alias = kagemusha_ordinary_android_app_key_alias_v1(c).unwrap();
        Arc::new(prepared)
            .admit_raw_attestation(admission, raw, &app.app_public_key, alias, 300)
            .unwrap()
    }
    #[test]
    fn possession_once_only_retains_verifies_and_recovers_actual_signature() {
        let p = pending();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut attempt = KagemushaOrdinaryAppPossessionAttemptV1::create(&root, &p, 300).unwrap();
        let selector = p.possession_challenge(300).unwrap().enrollment_attempt_id;
        let fields = attempt.preparation_fields(&p, selector, 300).unwrap();
        assert_eq!(fields.len(), 14);
        assert!(fields[8].is_empty());
        assert_eq!(fields[9], p.native_scope());
        assert_eq!(
            fields[12],
            p.raw_admission().subject().app_signing_identity_digest
        );
        assert_eq!(attempt.recheck_fields(&p, 300).unwrap()[0], fields[9]);
        assert_eq!(
            attempt.fence(&p, 300).unwrap(),
            vec![vec![1], vec![], vec![]]
        );
        assert!(matches!(
            attempt.fence(&p, 300),
            Err(super::super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome)
        ));
        assert!(matches!(
            attempt.recovery_fields(&p, 300),
            Err(super::super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome)
        ));
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let signature: P256Signature = key.sign(&fields[1]);
        let raw = signature.to_der().as_bytes().to_vec();
        assert_eq!(
            attempt.retain(&p, &raw, 300).unwrap(),
            <[u8; 32]>::from(Sha256::digest(&raw))
        );
        let receipt = attempt.consume(&p, 300).unwrap();
        // Read the actual native checked time retained before fsync; 300 may now be backdated.
        let consumed_at = attempt.consumed_at_ms.unwrap();
        assert!(consumed_at >= 300);
        assert_eq!(receipt.len(), 184);
        assert_eq!(receipt[10], 2);
        assert_eq!(&receipt[19..51], &selector);
        assert_eq!(&receipt[51..83], &p.native_scope());
        assert_eq!(&receipt[147..179], fields[9].as_slice());
        assert_eq!(&receipt[115..147], Sha256::digest(&raw).as_slice());
        assert_eq!(attempt.consume(&p, consumed_at).unwrap(), receipt);
        drop(attempt);
        let recovered =
            KagemushaOrdinaryAppPossessionAttemptV1::open_existing(&root, &p, consumed_at).unwrap();
        assert_eq!(
            recovered.recovery_fields(&p, consumed_at).unwrap(),
            vec![vec![2], raw, receipt]
        );
    }
    #[test]
    fn possession_invalid_original_cannot_reset_or_resign() {
        let p = pending();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut attempt = KagemushaOrdinaryAppPossessionAttemptV1::create(&root, &p, 300).unwrap();
        attempt.fence(&p, 300).unwrap();
        let raw = vec![1; 8];
        attempt.retain(&p, &raw, 300).unwrap();
        assert!(attempt.consume(&p, 300).is_err());
        assert!(attempt.retain(&p, &[2; 8], 300).is_err());
        assert!(attempt.cancel(&p, 300).is_err());
        assert_eq!(attempt.fence(&p, 300).unwrap(), vec![vec![2], raw, vec![]]);
    }
    #[test]
    fn possession_replay_bound_refuses_sixth_and_leaves_seventh_unread() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut wal = PrivateJournal::create_new(&root, FORMAT).unwrap();
        for _ in 0..6 {
            wal.append(&encode(&Record::Invoked).unwrap()).unwrap();
        }
        wal.append(b"seventh remains unread").unwrap();
        drop(wal);
        let mut wal = PrivateJournal::open_existing(&root, FORMAT).unwrap();
        assert!(replay_bounded(&mut wal).is_err());
        assert_eq!(
            wal.replay_next().unwrap(),
            Some((6, b"seventh remains unread".to_vec()))
        );
    }
    fn final_original(p: &KagemushaPendingAppIdentityV1, raw_e: &[u8], issue: u64) -> Vec<u8> {
        let mut subject = Fixture::new(false).selection.issuance.credential.subject;
        subject.issued_at_ms = issue;
        subject.expires_at_ms = 9000;
        subject.platform_evidence_digest =
            kagemusha_ordinary_app_enrollment_evidence_digest_v1(p.raw_attestation(), raw_e)
                .unwrap();
        signed_subject(subject)
    }
    fn signed_subject(subject: KagemushaOrdinaryAppCredentialSubjectV1) -> Vec<u8> {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let signature = Signature::try_new(
            issuer.private_key(),
            &subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        let circuit_admission =
            iroha_data_model::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
                KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(
                    &subject, &signature,
                )
                .unwrap(),
            );
        norito::encode_canonical(&KagemushaOrdinaryAppCredentialV1 {
            subject,
            signature,
            circuit_admission,
        })
        .unwrap()
    }
    fn consumed(
        root: &Path,
        p: &KagemushaPendingAppIdentityV1,
    ) -> (
        KagemushaOrdinaryAppPossessionAttemptV1,
        Vec<u8>,
        Vec<u8>,
        u64,
    ) {
        let mut a = KagemushaOrdinaryAppPossessionAttemptV1::create(root, p, 300).unwrap();
        a.fence(p, 300).unwrap();
        let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        let sig: P256Signature = key.sign(&a.challenge.canonical_signing_bytes().unwrap());
        let raw = sig.to_der().as_bytes().to_vec();
        a.retain(p, &raw, 300).unwrap();
        let receipt = a.consume(p, 300).unwrap();
        let consumed_at = a.consumed_at_ms.unwrap();
        assert!(consumed_at >= 300);
        (a, raw, receipt, consumed_at)
    }
    #[test]
    fn final_identity_requires_consumed_e_and_recovers_original_after_c_expiry() {
        let p = pending();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let (mut a, raw, receipt, consumed_at) = consumed(&root, &p);
        assert!(a.final_identity(&p, consumed_at).is_err());
        let issued_at = consumed_at.max(400);
        assert!(issued_at < a.challenge.expires_at_ms);
        let original = final_original(&p, &raw, issued_at);
        // The final credential was originally signed within C, but its response was lost.
        // No new E, signing, clock backdating or issuer call occurs during this intake.
        assert!(p.recheck_at_trusted_time(2100).is_err());
        assert!(a.fence(&p, 2100).is_err());
        assert!(a.consume(&p, 2100).is_err());
        let result = a.accept_final_credential(&p, &original, 2100).unwrap();
        assert_eq!(result[1], p.native_scope());
        assert_eq!(a.final_identity(&p, 2100).unwrap().original(), original);
        assert_eq!(
            a.recovery_fields(&p, 2100).unwrap(),
            vec![vec![2], raw.clone(), receipt.clone()]
        );
        assert_eq!(
            a.accept_final_credential(&p, &original, 2200).unwrap(),
            result
        );
        drop(a);
        let mut a =
            KagemushaOrdinaryAppPossessionAttemptV1::open_existing(&root, &p, 2200).unwrap();
        assert_eq!(a.final_identity(&p, 2200).unwrap().original(), original);
        assert_eq!(
            a.accept_final_credential(&p, &original, 2300).unwrap(),
            result
        );
        assert!(a.final_identity(&p, 9000).is_err());
        assert!(a.recovery_fields(&p, 9000).is_err());
        assert!(a.accept_final_credential(&p, &original, 9000).is_err());
    }
    #[test]
    fn final_identity_refuses_signed_evidence_counter_level_and_time_substitutions() {
        let p = pending();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let (mut a, raw, _, consumed_at) = consumed(&root, &p);
        let issued_at = consumed_at.max(400);
        assert!(issued_at < a.challenge.expires_at_ms);
        let original = final_original(&p, &raw, issued_at);
        let baseline = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&original)
            .unwrap()
            .subject;
        for mutation in 0..6 {
            let mut subject = baseline;
            match mutation {
                0 => subject.platform_evidence_digest[0] ^= 1,
                1 => subject.app_attest_counter_floor = 1,
                2 => {
                    subject.security_level =
                        KagemushaAppKeySecurityLevelV1::TrustedExecutionEnvironment
                }
                3 => subject.issued_at_ms = 299,
                4 => subject.client_nonce[0] ^= 1,
                _ => subject.app_release_digest[0] ^= 1,
            }
            assert!(
                a.accept_final_credential(&p, &signed_subject(subject), issued_at)
                    .is_err()
            );
        }
        let mut forged =
            KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&original).unwrap();
        let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
        forged.signature = Signature::try_new(
            foreign.private_key(),
            &forged.subject.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        assert!(
            a.accept_final_credential(&p, &norito::encode_canonical(&forged).unwrap(), issued_at)
                .is_err()
        );
        let mut wrong_purpose =
            KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&original).unwrap();
        wrong_purpose.circuit_admission.subject.purpose = 2;
        wrong_purpose.circuit_admission =
            iroha_data_model::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
                wrong_purpose.circuit_admission.subject,
            );
        assert!(
            a.accept_final_credential(
                &p,
                &norito::encode_canonical(&wrong_purpose).unwrap(),
                issued_at
            )
            .is_err()
        );
        let mut wrong_issuer =
            KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&original).unwrap();
        let foreign = SigningKey::from_bytes((&[99; 32]).into()).unwrap();
        let signature: P256Signature = foreign.sign(
            &wrong_issuer
                .circuit_admission
                .subject
                .canonical_signing_bytes()
                .unwrap(),
        );
        let signature = signature.normalize_s().unwrap_or(signature);
        wrong_issuer.circuit_admission.signature =
            KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
        assert!(
            a.accept_final_credential(
                &p,
                &norito::encode_canonical(&wrong_issuer).unwrap(),
                issued_at
            )
            .is_err()
        );
        a.accept_final_credential(&p, &original, issued_at).unwrap();
        // Even a valid newly issued replacement for the same key cannot replace the original row.
        assert!(
            a.accept_final_credential(
                &p,
                &final_original(&p, &raw, issued_at + 1),
                issued_at + 100
            )
            .is_err()
        );
        assert_eq!(
            a.final_identity(&p, issued_at + 100).unwrap().original(),
            original
        );
        let fresh = tempfile::tempdir().unwrap();
        let mut unconsumed = KagemushaOrdinaryAppPossessionAttemptV1::create(
            &fresh.path().canonicalize().unwrap(),
            &p,
            300,
        )
        .unwrap();
        assert!(
            unconsumed
                .accept_final_credential(&p, &original, issued_at)
                .is_err()
        );
    }
    #[test]
    fn possession_frame_charge_covers_bounded_full_final_original() {
        let record = Record::FinalCredential {
            original: vec![23; 16 * 1024],
            published_at_ms: 400,
        };
        let bytes = encode(&record).unwrap();
        assert!(bytes.len() > 16 * 1024 && bytes.len() <= MAX_FRAME);
        let decoded: Record = norito::decode_canonical_with_limits(
            &bytes,
            norito::DecodeLimits::new(MAX_FRAME, MAX_FRAME, MAX_FRAME * 2, MAX_FRAME * 8, 32),
        )
        .unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert!(
            encode(&Record::FinalCredential {
                original: vec![23; MAX_FRAME],
                published_at_ms: 400
            })
            .is_err()
        );
    }
    #[test]
    fn consumed_original_recovery_after_c_expiry_never_resumes_incomplete_signing() {
        let p = pending();
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let (a, raw, receipt, consumed_at) = consumed(&root, &p);
        let issued_at = consumed_at.max(400);
        assert!(issued_at < a.challenge.expires_at_ms);
        let scope = a.pending_scope;
        let message_hash = Sha256::digest(a.challenge.canonical_signing_bytes().unwrap()).to_vec();
        drop(a);
        let mut recovered =
            KagemushaOrdinaryAppPossessionAttemptV1::open_existing(&root, &p, 2100).unwrap();
        assert_eq!(
            recovered.recovery_fields(&p, 2100).unwrap(),
            vec![vec![2], raw.clone(), receipt.clone()]
        );
        assert_eq!(
            recovered.recheck_fields(&p, 2100).unwrap(),
            vec![scope.to_vec(), message_hash]
        );
        assert!(recovered.final_identity(&p, 2100).is_err());
        assert!(recovered.fence(&p, 2100).is_err());
        assert!(recovered.retain(&p, &raw, 2100).is_err());
        assert!(recovered.consume(&p, 2100).is_err());
        recovered
            .accept_final_credential(&p, &final_original(&p, &raw, issued_at), 2100)
            .unwrap();
        assert_eq!(
            recovered.recovery_fields(&p, 2200).unwrap(),
            vec![vec![2], raw, receipt]
        );
        for retained_raw in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().canonicalize().unwrap();
            let mut a = KagemushaOrdinaryAppPossessionAttemptV1::create(&root, &p, 300).unwrap();
            a.fence(&p, 300).unwrap();
            if retained_raw {
                a.retain(&p, &[1; 8], 300).unwrap();
            }
            assert!(a.recovery_fields(&p, 2100).is_err());
            assert!(a.recheck_fields(&p, 2100).is_err());
            assert!(a.fence(&p, 2100).is_err());
            assert!(a.consume(&p, 2100).is_err());
            drop(a);
            assert!(
                KagemushaOrdinaryAppPossessionAttemptV1::open_existing(&root, &p, 2100).is_err()
            );
        }
    }
}
