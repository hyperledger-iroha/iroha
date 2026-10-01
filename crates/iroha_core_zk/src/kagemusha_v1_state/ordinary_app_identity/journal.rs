//! Descriptor-held C21 custody: separate generation and attestation invocation fences.
//! These bytes admit no final credential, hardware monotonicity or monetary operation.

use super::super::{PrivateJournal, PrivateJournalFormat};
use super::KagemushaOrdinaryIdentityErrorV1::UnknownOutcome;
use super::{
    Custody, KagemushaPendingAppIdentityV1, KagemushaPreparedOrdinaryAppEnrollmentV1, Rejected,
    Result,
};
use iroha_data_model::kagemusha::*;
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{path::Path, sync::Arc};
#[path = "continuous_clock.rs"]
pub(super) mod continuous_clock;

const MAX_RAW: usize = 128 * 1024;
const MAX_FRAME: usize = 160 * 1024;
const MAX_ROWS: usize = 6;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-app-enrollment.wal",
    magic: b"KGMCAPP1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-app-enrollment-wal\0",
    maximum_payload_bytes: MAX_FRAME as u64,
};

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::EnrollmentRecordV1")]
enum Record {
    Prepared {
        ticket: u64,
        scope: [u8; 32],
        signed_c: Vec<u8>,
    },
    GenerationInvoked,
    KeyReference(String),
    AttestationInvoked,
    Raw {
        point: KagemushaDevicePublicKeyV1,
        evidence: Vec<u8>,
    },
    RawAccepted {
        original: Vec<u8>,
        checked_at_ms: u64,
    },
    Cancelled,
}

/// One independently authenticated C attempt with descriptor-held append-only original custody.
/// No decoder, clone or platform-selected preparation can reconstruct this native owner.
pub struct KagemushaOrdinaryAppEnrollmentAttemptV1 {
    owner: Arc<KagemushaPreparedOrdinaryAppEnrollmentV1>,
    journal: PrivateJournal,
    ticket: u64,
    stage: u8,
    key_reference: Option<String>,
    raw: Option<(KagemushaDevicePublicKeyV1, Vec<u8>)>,
    pending: Option<KagemushaPendingAppIdentityV1>,
    admitted_original: Option<Vec<u8>>,
    reference_ms: u64,
    reference_clock: continuous_clock::Reading,
}
impl KagemushaOrdinaryAppEnrollmentAttemptV1 {
    /// Create a durable preparation only from the actual independently authenticated C owner.
    /// `reference_ms` is supplied by the native authenticated-time source, never C/JNI.
    /// # Errors
    /// Rejects existing/uncertain storage, expired originals or unavailable native randomness.
    pub fn create(
        root: &Path,
        owner: KagemushaPreparedOrdinaryAppEnrollmentV1,
        reference_ms: u64,
    ) -> Result<Self> {
        owner.recheck_at_trusted_time(reference_ms)?;
        let clock = continuous_clock::Reading::now()?;
        let mut random = [0; 8];
        OsRng.try_fill_bytes(&mut random).map_err(|_| Custody)?;
        let ticket = u64::from_le_bytes(random);
        if ticket == 0 {
            return Err(Custody);
        }
        let id = owner.preparation.challenge.enrollment_id;
        let mut journal =
            PrivateJournal::create_new(&root.join(hex::encode(id)), FORMAT).map_err(|_| Custody)?;
        journal
            .append(&encode(&Record::Prepared {
                ticket,
                scope: owner.native_scope,
                signed_c: owner
                    .preparation
                    .to_transport_bytes()
                    .map_err(|_| Rejected)?,
            })?)
            .map_err(|_| Custody)?;
        let this = Self {
            owner: Arc::new(owner),
            journal,
            ticket,
            stage: 0,
            key_reference: None,
            raw: None,
            pending: None,
            admitted_original: None,
            reference_ms,
            reference_clock: clock,
        };
        this.recheck()?;
        Ok(this)
    }

    /// Recover the exact C/key/raw prefix under fresh independently admitted native owner/time.
    /// A complete invocation fence without its original remains frozen; it is never retried.
    /// # Errors
    /// Rejects changed C, extra/torn/reordered frames, missing issuer admission or expired scope.
    pub fn open_existing(
        root: &Path,
        owner: KagemushaPreparedOrdinaryAppEnrollmentV1,
        reference_ms: u64,
    ) -> Result<Self> {
        Self::open_mode(root, owner, reference_ms, false)
    }
    /// Recover only an already accepted raw prefix; never create or resume a hardware effect.
    /// Current policies/descriptors remain required although the original C may have expired.
    /// # Errors
    /// Rejects every incomplete/unknown/cancelled prefix or invalid historical admission.
    pub fn open_retained_originals(
        root: &Path,
        owner: KagemushaPreparedOrdinaryAppEnrollmentV1,
        reference_ms: u64,
    ) -> Result<Self> {
        Self::open_mode(root, owner, reference_ms, true)
    }
    fn open_mode(
        root: &Path,
        owner: KagemushaPreparedOrdinaryAppEnrollmentV1,
        reference_ms: u64,
        retained_only: bool,
    ) -> Result<Self> {
        owner.recheck_retained_originals_at_trusted_time(reference_ms)?;
        if !retained_only {
            owner.recheck_at_trusted_time(reference_ms)?;
        }
        let clock = continuous_clock::Reading::now()?;
        let mut journal = PrivateJournal::open_existing(
            &root.join(hex::encode(owner.preparation.challenge.enrollment_id)),
            FORMAT,
        )
        .map_err(|_| Custody)?;
        let rows = replay_bounded(&mut journal)?;
        let Some(Record::Prepared {
            ticket,
            scope,
            signed_c,
        }) = rows.first()
        else {
            return Err(Custody);
        };
        if *ticket == 0
            || *scope != owner.native_scope
            || signed_c
                != &owner
                    .preparation
                    .to_transport_bytes()
                    .map_err(|_| Rejected)?
        {
            return Err(Custody);
        }
        let mut this = Self {
            owner: Arc::new(owner),
            journal,
            ticket: *ticket,
            stage: 0,
            key_reference: None,
            raw: None,
            pending: None,
            admitted_original: None,
            reference_ms,
            reference_clock: clock,
        };
        for row in rows.into_iter().skip(1) {
            match (this.stage, row) {
                (0, Record::GenerationInvoked) => this.stage = 1,
                (0, Record::Cancelled) => this.stage = 6,
                (1, Record::KeyReference(key)) => {
                    this.validate_reference(&key)?;
                    this.key_reference = Some(key);
                    this.stage = 2;
                }
                (2, Record::AttestationInvoked) => this.stage = 3,
                (3, Record::Raw { point, evidence }) => {
                    this.validate_raw(&point, &evidence)?;
                    this.raw = Some((point, evidence));
                    this.stage = 4;
                }
                (
                    4,
                    Record::RawAccepted {
                        original,
                        checked_at_ms,
                    },
                ) => {
                    if checked_at_ms > reference_ms {
                        return Err(Custody);
                    }
                    this.accept_checked_original(&original, checked_at_ms)?;
                    this.admitted_original = Some(original);
                    this.stage = 5;
                }
                _ => return Err(Custody),
            }
        }
        if retained_only {
            this.recheck_retained_originals()?;
        } else {
            this.recheck()?;
        }
        Ok(this)
    }
    fn now(&self) -> Result<u64> {
        self.reference_ms
            .checked_add(continuous_clock::Reading::now()?.elapsed_ms(self.reference_clock)?)
            .ok_or(Custody)
    }
    /// Require actual held storage and original current native scope before any platform effect.
    /// # Errors
    /// Rejects revoked/expired original custody, a changed descriptor or cancelled attempt.
    pub fn recheck(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        self.owner.recheck_at_trusted_time(self.now()?)?;
        if self.stage == 6 {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Recheck only an accepted raw original for possession/final credential recovery.
    /// No generation/attestation/signing method uses this check instead of fresh C admission.
    /// # Errors
    /// Rejects an incomplete prefix, stale current policy, changed descriptor or raw original.
    pub fn recheck_retained_originals(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        if self.stage != 5 {
            return Err(Rejected);
        }
        self.owner
            .recheck_retained_originals_at_trusted_time(self.now()?)?;
        self.pending
            .as_ref()
            .ok_or(Custody)?
            .recheck_retained_originals_at_trusted_time(self.now()?)
    }
    /// Borrow the already accepted exact pending raw holder for completed E recovery only.
    /// # Errors
    /// Rejects incomplete/unknown original custody or stale current policy.
    pub fn retained_pending_identity(&self) -> Result<&KagemushaPendingAppIdentityV1> {
        self.recheck_retained_originals()?;
        self.pending.as_ref().ok_or(Custody)
    }
    /// Process-local retained original ticket; exposing it does not admit another preparation.
    pub const fn ticket(&self) -> u64 {
        self.ticket
    }
    /// Exact eight C21 preparation fields, projected only after native owner/storage recheck.
    /// # Errors
    /// Rejects expired or uncertain custody.
    pub fn preparation_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        let mut fields = vec![self.ticket.to_le_bytes().to_vec()];
        fields.extend(self.owner.key_generation_projection(self.now()?)?);
        self.recheck()?;
        Ok(fields)
    }
    /// Durably fence exactly one generation before the platform invocation.
    /// Returns disposition1/new with empty key or disposition2/already retained with original key.
    /// # Errors
    /// A fence with no retained key is unknown and cannot invoke generation again.
    pub fn fence_generation(&mut self) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        match self.stage {
            0 => {
                self.append(Record::GenerationInvoked)?;
                self.stage = 1;
                Ok(vec![vec![1], vec![]])
            }
            1 => Err(UnknownOutcome),
            2..=5 => Ok(vec![
                vec![2],
                self.key_reference
                    .as_ref()
                    .ok_or(Custody)?
                    .as_bytes()
                    .to_vec(),
            ]),
            _ => Err(Rejected),
        }
    }
    /// Durably retain the actual generated key reference before any attestation invocation.
    /// # Errors
    /// Rejects a replaced reference or another native generation/attestation phase.
    pub fn retain_key_reference(&mut self, reference: &str) -> Result<[u8; 32]> {
        self.recheck()?;
        self.validate_reference(reference)?;
        if self.stage == 1 {
            self.append(Record::KeyReference(reference.into()))?;
            self.key_reference = Some(reference.into());
            self.stage = 2;
        } else if self.stage < 2 || self.key_reference.as_deref() != Some(reference) {
            return Err(Rejected);
        }
        self.recheck()?;
        Ok(Sha256::digest(reference.as_bytes()).into())
    }
    /// Fence original attestation separately, after the key reference is durably retained.
    /// Returns1/new with empty metadata, or2/already retained with point/hash/length metadata.
    /// # Errors
    /// An attestation fence without its original is unknown; no second platform call is admitted.
    pub fn fence_attestation(&mut self) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        match self.stage {
            2 => {
                self.append(Record::AttestationInvoked)?;
                self.stage = 3;
                Ok(vec![vec![1], vec![], vec![], vec![]])
            }
            3 => Err(UnknownOutcome),
            4..=5 => {
                let (point, evidence) = self.raw.as_ref().ok_or(Custody)?;
                Ok(vec![
                    vec![2],
                    point.as_sec1_bytes().to_vec(),
                    Sha256::digest(evidence).to_vec(),
                    (evidence.len() as u32).to_le_bytes().to_vec(),
                ])
            }
            _ => Err(Rejected),
        }
    }
    /// Retain complete original platform evidence before requesting the purpose-specific issuer.
    /// Structural validation is not certificate/root/platform admission.
    /// # Errors
    /// Rejects missing/changed key, original chain format, oversized data or another phase.
    pub fn retain_raw(
        &mut self,
        point: KagemushaDevicePublicKeyV1,
        evidence: &[u8],
    ) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        self.validate_raw(&point, evidence)?;
        if self.stage == 3 {
            self.append(Record::Raw {
                point,
                evidence: evidence.to_vec(),
            })?;
            self.raw = Some((point, evidence.to_vec()));
            self.stage = 4;
        } else if self.stage < 4
            || self
                .raw
                .as_ref()
                .is_none_or(|(p, e)| p != &point || e != evidence)
        {
            return Err(Rejected);
        }
        self.recheck()?;
        Ok(vec![
            Sha256::digest(evidence).to_vec(),
            Sha256::digest(point.as_sec1_bytes()).to_vec(),
        ])
    }
    /// Read exact native-held C, point and full raw evidence for the protected issuer transport.
    /// A callback response remains untrusted until `accept_raw_admission` authenticates it.
    /// # Errors
    /// Rejects missing originals, another lifecycle stage or expired/uncertain custody.
    pub fn raw_issuer_originals(&self) -> Result<(Vec<u8>, KagemushaDevicePublicKeyV1, Vec<u8>)> {
        self.recheck()?;
        if !matches!(self.stage, 4 | 5) {
            return Err(Rejected);
        }
        let (point, raw) = self.raw.as_ref().ok_or(Custody)?;
        Ok((
            self.owner
                .preparation
                .to_transport_bytes()
                .map_err(|_| Rejected)?,
            *point,
            raw.clone(),
        ))
    }
    /// Read the already accepted original result without another issuer invocation.
    /// # Errors
    /// Rejects stale/uncertain native custody; no final credential or financial capability exists.
    pub fn retained_raw_admission_result(&self) -> Result<Option<Vec<Vec<u8>>>> {
        self.recheck()?;
        if self.stage != 5 {
            return Ok(None);
        }
        Ok(Some(vec![
            self.pending
                .as_ref()
                .ok_or(Custody)?
                .native_scope()
                .to_vec(),
            Sha256::digest(self.admitted_original.as_ref().ok_or(Custody)?).to_vec(),
        ]))
    }
    /// Verify an independently fetched issuer raw original and durably admit it before E.
    /// The phase6 transport supplies an untrusted314-byte original; native admission is mandatory.
    /// # Errors
    /// Rejects altered original, issuer/time/policy/key/evidence or uncertain durable append.
    pub fn accept_raw_admission(&mut self, original: &[u8]) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        if self.stage == 4 {
            let now = self.now()?;
            self.accept_checked_original(original, now)?;
            if self
                .append(Record::RawAccepted {
                    original: original.to_vec(),
                    checked_at_ms: now,
                })
                .is_err()
            {
                self.pending = None;
                return Err(Custody);
            }
            self.admitted_original = Some(original.to_vec());
            self.stage = 5;
        } else if self.stage != 5 || self.admitted_original.as_deref() != Some(original) {
            return Err(Rejected);
        }
        self.recheck()?;
        Ok(vec![
            self.pending
                .as_ref()
                .ok_or(Custody)?
                .native_scope()
                .to_vec(),
            Sha256::digest(original).to_vec(),
        ])
    }
    fn accept_checked_original(&mut self, original: &[u8], checked: u64) -> Result<()> {
        let admission = KagemushaRawAppAttestationAdmissionV1::from_transport_bytes(original)
            .map_err(|_| Rejected)?;
        let (point, evidence) = self.raw.as_ref().ok_or(Custody)?;
        self.pending = Some(self.owner.clone().admit_raw_attestation(
            admission,
            evidence.clone(),
            point,
            self.key_reference.as_ref().ok_or(Custody)?.clone(),
            checked,
        )?);
        Ok(())
    }
    /// Borrow the actual pending raw owner for the connected native E20 factory only.
    /// # Errors
    /// Rejects unavailable or unconsumed raw admission; no final credential is supplied.
    pub fn pending_identity(&self) -> Result<&KagemushaPendingAppIdentityV1> {
        self.recheck()?;
        self.pending.as_ref().ok_or(Rejected)
    }
    /// Read exact recovery metadata, with state0..5 and no renewed hardware invocation.
    /// # Errors
    /// Rejects stale native scope or uncertain storage. Current first-release C interval remains
    /// required; this journal does not invent an expired-scope issuer recovery authority.
    pub fn recovery_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        let (point, raw_hash, raw_len) = self.raw.as_ref().map_or((vec![], vec![], 0), |(p, r)| {
            (
                p.as_sec1_bytes().to_vec(),
                Sha256::digest(r).to_vec(),
                r.len() as u32,
            )
        });
        Ok(vec![
            vec![self.stage],
            self.key_reference
                .as_ref()
                .map_or(vec![], |k| k.as_bytes().to_vec()),
            point,
            raw_hash,
            raw_len.to_le_bytes().to_vec(),
            self.admitted_original.clone().unwrap_or_default(),
            self.pending
                .as_ref()
                .map_or(vec![], |p| p.native_scope().to_vec()),
        ])
    }
    /// Read one original64KiB evidence chunk, preserving legacy response and field ceilings.
    /// # Errors
    /// Rejects a missing original, another chunk index or unavailable native held custody.
    pub fn raw_chunk_fields(&self, index: u32) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        if index > 1 {
            return Err(Rejected);
        }
        let (_, raw) = self.raw.as_ref().ok_or(Rejected)?;
        let start = (index as usize) * 65536;
        if start >= raw.len() {
            return Err(Rejected);
        }
        let end = start.saturating_add(65536).min(raw.len());
        Ok(vec![
            index.to_le_bytes().to_vec(),
            raw[start..end].to_vec(),
            Sha256::digest(raw).to_vec(),
            (raw.len() as u32).to_le_bytes().to_vec(),
        ])
    }
    /// Exact current native C scope and challenge digest; never a new preparation or clock.
    /// # Errors
    /// Rejects unavailable held custody.
    pub fn recheck_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck()?;
        Ok(vec![
            self.owner.native_scope.to_vec(),
            self.owner
                .preparation
                .challenge
                .attestation_challenge()
                .map_err(|_| Rejected)?
                .to_vec(),
        ])
    }
    /// Cancel only a never-invoked attempt; a hardware fence or original can never be erased.
    /// # Errors
    /// Rejects every invoked, unknown, raw or accepted stage.
    pub fn cancel(&mut self) -> Result<()> {
        self.recheck()?;
        if self.stage != 0 {
            return Err(Rejected);
        }
        self.append(Record::Cancelled)?;
        self.stage = 6;
        Ok(())
    }
    fn append(&mut self, row: Record) -> Result<()> {
        self.journal.append(&encode(&row)?).map_err(|_| Custody)
    }
    fn validate_reference(&self, key: &str) -> Result<()> {
        if key.is_empty() || key.len() > 255 || key.as_bytes().contains(&0) {
            return Err(Rejected);
        }
        let c = &self.owner.preparation.challenge;
        match c.platform_class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint
                if key
                    == kagemusha_ordinary_android_app_key_alias_v1(c).map_err(|_| Rejected)? =>
            {
                Ok(())
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                use base64::{Engine as _, engine::general_purpose::STANDARD};
                let decoded = STANDARD.decode(key).map_err(|_| Rejected)?;
                if decoded.len() != 32 || decoded == [0; 32] || STANDARD.encode(&decoded) != key {
                    return Err(Rejected);
                }
                Ok(())
            }
            _ => Err(Rejected),
        }
    }
    fn validate_raw(&self, point: &KagemushaDevicePublicKeyV1, raw: &[u8]) -> Result<()> {
        point.validate().map_err(|_| Rejected)?;
        if raw.is_empty() || raw.len() > MAX_RAW {
            return Err(Rejected);
        }
        super::validate_original_alias(
            &self.owner.preparation.challenge,
            Sha256::digest(point.as_sec1_bytes()).into(),
            self.key_reference.as_deref().ok_or(Rejected)?,
        )?;
        if self.owner.preparation.challenge.platform_class
            == KagemushaHardwarePlatformClassV1::AndroidKeyMint
        {
            validate_android_archive(raw)?;
        }
        Ok(())
    }
}
fn encode(record: &Record) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(record).map_err(|_| Custody)?;
    if bytes.len() > MAX_FRAME {
        return Err(Custody);
    }
    Ok(bytes)
}
fn replay_bounded(journal: &mut PrivateJournal) -> Result<Vec<Record>> {
    let mut rows = Vec::with_capacity(MAX_ROWS);
    for index in 0..=MAX_ROWS {
        let Some((sequence, bytes)) = journal.replay_next().map_err(|_| Custody)? else {
            return if rows.is_empty() {
                Err(Custody)
            } else {
                Ok(rows)
            };
        };
        if index == MAX_ROWS || sequence != index as u64 {
            return Err(Custody);
        }
        let row: Record = norito::decode_canonical_with_limits(
            &bytes,
            norito::DecodeLimits::new(MAX_FRAME, MAX_FRAME, MAX_FRAME * 2, MAX_FRAME * 8, 32),
        )
        .map_err(|_| Custody)?;
        if encode(&row)? != bytes {
            return Err(Custody);
        }
        rows.push(row);
    }
    Err(Custody)
}
fn validate_android_archive(raw: &[u8]) -> Result<()> {
    if raw.len() < 6 || &raw[..5] != b"KMCA\x01" || !(2..=8).contains(&raw[5]) {
        return Err(Rejected);
    }
    let mut offset = 6usize;
    for _ in 0..raw[5] {
        let end = offset.checked_add(4).ok_or(Rejected)?;
        let size = u32::from_be_bytes(
            raw.get(offset..end)
                .ok_or(Rejected)?
                .try_into()
                .map_err(|_| Rejected)?,
        ) as usize;
        offset = end;
        if size == 0 || size > 16 * 1024 {
            return Err(Rejected);
        }
        offset = offset
            .checked_add(size)
            .filter(|n| *n <= raw.len())
            .ok_or(Rejected)?;
    }
    if offset != raw.len() {
        return Err(Rejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    fn owner(f: &Fixture) -> KagemushaPreparedOrdinaryAppEnrollmentV1 {
        let c = &f.selection.preparation.challenge;
        KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
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
        .unwrap()
    }
    #[test]
    fn ordinary_c_journal_fences_each_invocation_and_recovers_same_original_reference() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let f = Fixture::new(true);
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut attempt =
            KagemushaOrdinaryAppEnrollmentAttemptV1::create(&root, owner(&f), 300).unwrap();
        let ticket = attempt.ticket();
        assert_eq!(attempt.preparation_fields().unwrap().len(), 8);
        assert_eq!(attempt.fence_generation().unwrap(), vec![vec![1], vec![]]);
        assert!(matches!(attempt.fence_generation(), Err(UnknownOutcome)));
        drop(attempt);
        let mut attempt =
            KagemushaOrdinaryAppEnrollmentAttemptV1::open_existing(&root, owner(&f), 300).unwrap();
        assert_eq!(attempt.ticket(), ticket);
        assert_eq!(attempt.recovery_fields().unwrap()[0], vec![1]);
        assert!(matches!(attempt.fence_generation(), Err(UnknownOutcome)));
        let key_id = f.selection.issuance.credential.subject.attested_key_id;
        let original = STANDARD.encode(key_id);
        attempt.retain_key_reference(&original).unwrap();
        assert_eq!(
            attempt.fence_generation().unwrap(),
            vec![vec![2], original.clone().into_bytes()]
        );
        assert_eq!(attempt.fence_attestation().unwrap()[0], vec![1]);
        assert!(matches!(attempt.fence_attestation(), Err(UnknownOutcome)));
        assert!(attempt.cancel().is_err());
        assert!(
            attempt
                .retain_key_reference(&STANDARD.encode([99; 32]))
                .is_err()
        );
        drop(attempt);
        let attempt =
            KagemushaOrdinaryAppEnrollmentAttemptV1::open_existing(&root, owner(&f), 300).unwrap();
        let fields = attempt.recovery_fields().unwrap();
        assert_eq!(fields[0], vec![3]);
        assert_eq!(fields[1], original.into_bytes());
        assert!(attempt.pending_identity().is_err());
    }
    #[test]
    fn ordinary_c_raw_admission_retains_full_original_and_has_no_final_credential() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use iroha_crypto::{Algorithm, KeyPair, Signature};
        let f = Fixture::new(true);
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut attempt =
            KagemushaOrdinaryAppEnrollmentAttemptV1::create(&root, owner(&f), 300).unwrap();
        let app = f.selection.issuance.credential.subject;
        attempt.fence_generation().unwrap();
        attempt
            .retain_key_reference(&STANDARD.encode(app.attested_key_id))
            .unwrap();
        attempt.fence_attestation().unwrap();
        // Synthetic raw carrier isolates issuer-signature/custody joins. It is never platform admission.
        let raw = vec![23; MAX_RAW];
        attempt.retain_raw(app.app_public_key, &raw).unwrap();
        let metadata = attempt.fence_attestation().unwrap();
        assert_eq!(metadata[0], vec![2]);
        assert_eq!(metadata[2], Sha256::digest(&raw).to_vec());
        let first = attempt.raw_chunk_fields(0).unwrap();
        let second = attempt.raw_chunk_fields(1).unwrap();
        assert_eq!(first[1].len(), 65536);
        assert_eq!(second[1].len(), 65536);
        let mut joined = first[1].clone();
        joined.extend_from_slice(&second[1]);
        assert_eq!(joined, raw);
        let c = &f.selection.preparation.challenge;
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
        let original = admission.to_transport_bytes().unwrap();
        let retained = attempt.accept_raw_admission(&original).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[1], Sha256::digest(&original).to_vec());
        assert_eq!(attempt.accept_raw_admission(&original).unwrap(), retained);
        let after_raw_admission = attempt.now().unwrap();
        let e = attempt
            .pending_identity()
            .unwrap()
            .possession_challenge(after_raw_admission)
            .unwrap();
        assert_eq!(e.enrollment_attempt_id, c.attestation_challenge().unwrap());
        assert_ne!(e.enrollment_attempt_id, c.enrollment_id);
        assert_eq!(
            e.raw_platform_evidence_digest,
            Sha256::digest(&raw).as_slice()
        );
        let recovery_time = attempt.now().unwrap();
        drop(attempt);
        let attempt =
            KagemushaOrdinaryAppEnrollmentAttemptV1::open_existing(&root, owner(&f), recovery_time)
                .unwrap();
        let recovered = attempt.recovery_fields().unwrap();
        assert_eq!(recovered[0], vec![5]);
        assert_eq!(recovered[5], original);
        assert_eq!(recovered[6], retained[0]);
        assert_eq!(attempt.pending_identity().unwrap().raw_attestation(), raw);
    }
    #[test]
    fn ordinary_c_replay_stops_after_six_frames_and_leaves_eighth_unread() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let path = root.join("bounded");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        for _ in 0..7 {
            journal
                .append(&encode(&Record::GenerationInvoked).unwrap())
                .unwrap();
        }
        journal.append(b"eighth remains unread").unwrap();
        drop(journal);
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(replay_bounded(&mut journal).is_err());
        assert_eq!(
            journal.replay_next().unwrap(),
            Some((7, b"eighth remains unread".to_vec()))
        );
        let empty_path = root.join("empty");
        drop(PrivateJournal::create_new(&empty_path, FORMAT).unwrap());
        let mut empty = PrivateJournal::open_existing(&empty_path, FORMAT).unwrap();
        assert!(replay_bounded(&mut empty).is_err());
    }
    #[test]
    fn ordinary_c_raw_archive_and_retention_reject_changed_original_boundaries() {
        let mut archive = b"KMCA\x01\x02".to_vec();
        for bytes in [
            b"synthetic DER one".as_slice(),
            b"synthetic DER two".as_slice(),
        ] {
            archive.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            archive.extend_from_slice(bytes);
        }
        // Only outer envelope structure is checked here; raw DER semantics remain issuer-owned.
        validate_android_archive(&archive).unwrap();
        let mut extra = archive.clone();
        extra.push(0);
        assert!(validate_android_archive(&extra).is_err());
        assert!(validate_android_archive(&archive[..archive.len() - 1]).is_err());
        archive[5] = 9;
        assert!(validate_android_archive(&archive).is_err());
    }
}
