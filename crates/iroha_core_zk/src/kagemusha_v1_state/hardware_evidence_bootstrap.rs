//! Closed, nonmonetary first-device hardware evidence owner.
//! No financial reservation, wallet owner, State/Guard, account session or money API is held.
use super::{
    KagemushaOrdinaryNativeClockOwnerV1, KagemushaOrdinaryNativeTimeIntervalV1, PrivateJournal,
    PrivateJournalFormat,
};
use iroha_data_model::kagemusha::*;
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;
#[path = "hardware_evidence_bootstrap/lifecycle.rs"]
mod lifecycle;
use lifecycle::{Lifecycle, Step};
/// Refusal classes of the first-device hardware evidence owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaHardwareEvidenceErrorV1 {
    /// An original, selection or interval failed validation.
    #[error("hardware bootstrap original rejected")]
    Rejected,
    /// Journal, clock or randomness custody is unavailable.
    #[error("hardware bootstrap custody unavailable")]
    Custody,
    /// A fenced invocation may have executed; its outcome is not known.
    #[error("hardware bootstrap original invocation outcome unknown")]
    UnknownOutcome,
}
use KagemushaHardwareEvidenceErrorV1::{Custody, Rejected, UnknownOutcome};
type Result<T> = std::result::Result<T, KagemushaHardwareEvidenceErrorV1>;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "hardware-evidence-bootstrap.wal",
    magic: b"KGMCHWB1",
    hash_domain: b"iroha:kagemusha:v1:hardware-evidence-bootstrap-wal\0",
    maximum_payload_bytes: 256 * 1024,
};
const MAX_ROWS: usize = 16;

/// Exact runtime observations lent only by the trusted Native startup measurement factory.
/// No DTO decoder, clone or managed selector. The Rust startup data constructor alone
/// authenticates nothing; only the separately authenticated binding can create an owner.
pub struct KagemushaHardwareBootstrapArtifactMeasurementsV1 {
    app_package: String,
    app_version_code: u64,
    app_signing_identity_digest: [u8; 32],
    app_source_sha256: [u8; 32],
    app_code_sha256: [u8; 32],
    sdk_source_sha256: [u8; 32],
    android_abi: String,
    native_artifact_sha256: [u8; 32],
    native_abi: u32,
}
impl KagemushaHardwareBootstrapArtifactMeasurementsV1 {
    /// Exact observations from the independently trusted Rust platform startup. This data value
    /// alone grants no binding; the factory below additionally authenticates static admitted
    /// authority/release originals and the actual installed signed Native clock. No C/JNI path
    /// accepts these arguments. Source identities are semantic build inputs outside the binary;
    /// native_artifact_sha256 is the measured loaded JNI file, avoiding a self-hash cycle.
    #[allow(clippy::too_many_arguments)]
    pub fn from_native_startup_measurement(
        app_package: String,
        app_version_code: u64,
        app_signing_identity_digest: [u8; 32],
        app_source_sha256: [u8; 32],
        app_code_sha256: [u8; 32],
        sdk_source_sha256: [u8; 32],
        android_abi: String,
        native_artifact_sha256: [u8; 32],
        native_abi: u32,
    ) -> Result<Self> {
        if app_package.is_empty()
            || app_version_code == 0
            || native_abi == 0
            || !matches!(
                android_abi.as_str(),
                "arm64-v8a" | "armeabi-v7a" | "x86" | "x86_64"
            )
            || [
                app_signing_identity_digest,
                app_source_sha256,
                app_code_sha256,
                sdk_source_sha256,
                native_artifact_sha256,
            ]
            .contains(&[0; 32])
        {
            return Err(Rejected);
        }
        Ok(Self {
            app_package,
            app_version_code,
            app_signing_identity_digest,
            app_source_sha256,
            app_code_sha256,
            sdk_source_sha256,
            android_abi,
            native_artifact_sha256,
            native_abi,
        })
    }
}
/// Threshold-authenticated artifact purpose plus independently installed signed Native clock.
/// Runtime activation requires a real independently signed external release and compiled
/// authority-policy admission; this implementation creates neither original.
pub struct KagemushaCompiledHardwareBootstrapBindingV1 {
    signed_original: Vec<u8>,
    manifest: KagemushaHardwareEvidenceBootstrapManifestV1,
    digest: [u8; 32],
    clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
}
impl KagemushaCompiledHardwareBootstrapBindingV1 {
    /// Native startup only. The authority policy digest is an independently compiled admission
    /// input. The signed manifest stays outside this binary: it includes the measured JNI hash
    /// and therefore cannot itself be embedded or hash-pinned inside the same JNI.
    /// Measurements come from actual package/code/JNI custody, not manifest echoes. This separate
    /// purpose uses the existing threshold release-authority verifier without a money receipt.
    pub fn authenticate_compiled_originals(
        signed_original: &[u8],
        authority_original: &[u8],
        compiled_authority_policy_digest: [u8; 32],
        measured: KagemushaHardwareBootstrapArtifactMeasurementsV1,
        clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
    ) -> Result<Self> {
        if compiled_authority_policy_digest == [0; 32] {
            return Err(Rejected);
        }
        let authority =
            KagemushaReleaseAuthorityPolicyV1::decode_canonical_exact(authority_original)
                .map_err(|_| Rejected)?;
        if authority.canonical_digest().map_err(|_| Rejected)? != compiled_authority_policy_digest {
            return Err(Rejected);
        }
        let signed: KagemushaSignedHardwareBootstrapReleaseV1 = decode(signed_original)?;
        signed.authenticate(&authority).map_err(|_| Rejected)?;
        let m = signed.manifest;
        if m.app_package != measured.app_package
            || m.app_version_code != measured.app_version_code
            || m.app_signing_identity_digest != measured.app_signing_identity_digest
            || m.app_source_sha256 != measured.app_source_sha256
            || m.app_code_sha256 != measured.app_code_sha256
            || m.sdk_source_sha256 != measured.sdk_source_sha256
            || m.jni_artifacts
                .binary_search_by(|a| a.android_abi.cmp(&measured.android_abi))
                .ok()
                .is_none_or(|index| {
                    m.jni_artifacts[index].sha256 != measured.native_artifact_sha256
                })
            || m.native_abi != measured.native_abi
        {
            return Err(Rejected);
        }
        let digest = m.digest().map_err(|_| Rejected)?;
        let this = Self {
            signed_original: signed_original.to_vec(),
            manifest: m,
            digest,
            clock,
        };
        this.interval()?;
        Ok(this)
    }
    fn interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        self.manifest.validate().map_err(|_| Rejected)?;
        let mut c = self.clock.lock().map_err(|_| Custody)?;
        if *c.network_id().map_err(|_| Custody)?.as_bytes() != self.manifest.network_id
            || c.installed_selection_digest().map_err(|_| Custody)?
                != self.manifest.native_clock_selection_digest
        {
            return Err(Rejected);
        }
        let i = c.current_native_time_interval().map_err(|_| Custody)?;
        Ok(i)
    }
    fn current_interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        let i = self.interval()?;
        i.require_validity(self.manifest.not_before_ms, self.manifest.expires_at_ms)
            .map_err(|_| Rejected)?;
        Ok(i)
    }
    /// Return the signed release original after rechecking the network, clock selection and interval.
    pub fn public_signed_release_original(&self) -> Result<&[u8]> {
        self.interval()?;
        Ok(&self.signed_original)
    }
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::HardwareEvidenceBootstrapRecordV1")]
enum Record {
    Reserved(KagemushaHardwareEvidenceReservationV1),
    Invoked { step: u8, input: Vec<u8> },
    Captured { step: u8, original: Vec<u8> },
    CancelRequested,
    Disposed,
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::HardwareEvidenceCapturedAndroidOriginalV1")]
struct Raw {
    key: KagemushaDevicePublicKeyV1,
    archive: Vec<u8>,
}

/// Exclusive descriptor-owned alias/nonce/evidence custody. Closing the handle does not erase
/// its WAL or key, certify a pending call ceased, or authorize another reservation at this path.
pub struct KagemushaFirstDeviceHardwareEvidenceOwnerV1 {
    binding: Arc<KagemushaCompiledHardwareBootstrapBindingV1>,
    journal: PrivateJournal,
    reservation: KagemushaHardwareEvidenceReservationV1,
    state: Lifecycle,
    oauth_sha: Option<[u8; 32]>,
    c: Option<Vec<u8>>,
    raw: Option<Raw>,
    raw_admission: Option<Vec<u8>>,
    der: Option<Vec<u8>>,
    token: Option<Zeroizing<Vec<u8>>>,
    receipt: Option<Vec<u8>>,
}
impl KagemushaFirstDeviceHardwareEvidenceOwnerV1 {
    /// The startup factory selects one fixed directory for this compiled app/bootstrap purpose.
    /// Creation fails if that directory already exists, including uncertain/expired attempts.
    pub fn create(
        root: &Path,
        binding: Arc<KagemushaCompiledHardwareBootstrapBindingV1>,
    ) -> Result<Self> {
        let i = binding.current_interval()?;
        let mut operation = [0; 32];
        let mut nonce = [0; 32];
        OsRng.try_fill_bytes(&mut operation).map_err(|_| Custody)?;
        OsRng.try_fill_bytes(&mut nonce).map_err(|_| Custody)?;
        let deadline = i
            .lower_ms()
            .checked_add(binding.manifest.maximum_attempt_lifetime_ms)
            .ok_or(Rejected)?
            .min(binding.manifest.expires_at_ms);
        i.require_validity(i.lower_ms(), deadline)
            .map_err(|_| Rejected)?;
        let reservation = KagemushaHardwareEvidenceReservationV1 {
            version: 1,
            manifest_digest: binding.digest,
            operation_id: operation,
            client_nonce: nonce,
            alias: KagemushaHardwareEvidenceReservationV1::alias_for(
                binding.digest,
                operation,
                nonce,
            ),
            issued_at_ms: i.lower_ms(),
            deadline_ms: deadline,
        };
        reservation.validate().map_err(|_| Rejected)?;
        let mut journal = PrivateJournal::create_new(root, FORMAT).map_err(|_| Custody)?;
        journal
            .append(&encode(&Record::Reserved(reservation.clone()))?)
            .map_err(|_| Custody)?;
        Ok(Self::blank(binding, journal, reservation))
    }
    /// Reopen the existing journal for this binding and replay its bounded records.
    pub fn recover(
        root: &Path,
        binding: Arc<KagemushaCompiledHardwareBootstrapBindingV1>,
    ) -> Result<Self> {
        binding.interval()?;
        let mut journal = PrivateJournal::open_existing(root, FORMAT).map_err(|_| Custody)?;
        // Bound the physical transcript before semantic replay. No unbounded history is retained.
        let mut count = 0;
        while journal.replay_next().map_err(|_| Custody)?.is_some() {
            count += 1;
            if count > MAX_ROWS {
                return Err(Custody);
            }
        }
        let mut cursor = journal.replay_cursor().map_err(|_| Custody)?;
        let (_, first) = journal
            .read_cursor_next(&mut cursor)
            .map_err(|_| Custody)?
            .ok_or(Custody)?;
        let Record::Reserved(reservation) = decode(&first)? else {
            return Err(Custody);
        };
        reservation.validate().map_err(|_| Rejected)?;
        if reservation.manifest_digest != binding.digest {
            return Err(Rejected);
        }
        let mut this = Self::blank(binding, journal, reservation);
        while let Some((_, b)) = this
            .journal
            .read_cursor_next(&mut cursor)
            .map_err(|_| Custody)?
        {
            let record: Record = decode(&b)?;
            this.apply(record)?;
        }
        this.recheck_custody()?;
        Ok(this)
    }
    fn blank(
        binding: Arc<KagemushaCompiledHardwareBootstrapBindingV1>,
        journal: PrivateJournal,
        reservation: KagemushaHardwareEvidenceReservationV1,
    ) -> Self {
        Self {
            binding,
            journal,
            reservation,
            state: Lifecycle::default(),
            oauth_sha: None,
            c: None,
            raw: None,
            raw_admission: None,
            der: None,
            token: None,
            receipt: None,
        }
    }
    /// Require journal ownership and a valid bound native clock interval.
    pub fn recheck_custody(&self) -> Result<()> {
        self.journal.check_owned().map_err(|_| Custody)?;
        self.binding.interval()?;
        Ok(())
    }
    fn effect_interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        self.recheck_custody()?;
        let i = self.binding.interval()?;
        i.require_validity(
            self.binding.manifest.not_before_ms,
            self.binding.manifest.expires_at_ms,
        )
        .map_err(|_| Rejected)?;
        i.require_validity(self.reservation.issued_at_ms, self.reservation.deadline_ms)
            .map_err(|_| Rejected)?;
        if let Some(_) = self.c {
            let c = self.challenge()?.challenge;
            i.require_validity(c.issued_at_ms, c.expires_at_ms)
                .map_err(|_| Rejected)?;
        }
        Ok(i)
    }
    fn start(&mut self, s: Step, input: Vec<u8>) -> Result<()> {
        self.effect_interval()?;
        self.state.require_start(s).map_err(|_| UnknownOutcome)?;
        let r = Record::Invoked {
            step: s as u8,
            input,
        };
        self.validate_invocation(&r)?;
        self.persist(r)
    }
    fn persist(&mut self, r: Record) -> Result<()> {
        self.recheck_custody()?;
        let bytes = Zeroizing::new(encode(&r)?);
        self.journal.append(&bytes).map_err(|_| UnknownOutcome)?;
        self.apply(r)?;
        self.recheck_custody()
    }
    /// Native retains the original OAuth token hash before any protected prepare HTTP call.
    /// The independent Core issuer verifies Google's issuer/audience/subject; Native checks C.
    pub fn fence_prepare(&mut self, google_id_token_original: &[u8]) -> Result<Vec<u8>> {
        if google_id_token_original.is_empty()
            || google_id_token_original.len() > 16 * 1024
            || !google_id_token_original
                .iter()
                .all(|b| b.is_ascii_graphic())
        {
            return Err(Rejected);
        }
        self.start(Step::Prepare, sha(google_id_token_original).to_vec())?;
        encode(&self.reservation)
    }
    /// Capture the challenge original for the pending Prepare step.
    pub fn accept_challenge(&mut self, original: &[u8]) -> Result<()> {
        self.recheck_custody()?;
        self.check_capture(Step::Prepare, original)?;
        self.persist(Record::Captured {
            step: 1,
            original: original.to_vec(),
        })
    }
    /// Alias/policy/challenge are Native originals. The managed platform call is fenced first.
    pub fn fence_android_key(
        &mut self,
    ) -> Result<(String, [u8; 32], Vec<KagemushaAppKeySecurityLevelV1>)> {
        self.start(Step::Key, vec![])?;
        let c = self.challenge()?;
        Ok((
            self.reservation.alias.clone(),
            c.original_digest().map_err(|_| Rejected)?,
            self.binding
                .manifest
                .allowed_android_security_levels
                .clone(),
        ))
    }
    /// Recovery may inspect only the same alias/C. Missing key does not permit generation.
    pub fn recover_android_key_selection(
        &self,
    ) -> Result<(String, [u8; 32], Vec<KagemushaAppKeySecurityLevelV1>)> {
        self.recheck_custody()?;
        if self.state.pending() != Some(Step::Key) {
            return Err(Rejected);
        }
        Ok((
            self.reservation.alias.clone(),
            self.challenge()?.original_digest().map_err(|_| Rejected)?,
            self.binding
                .manifest
                .allowed_android_security_levels
                .clone(),
        ))
    }
    /// Capture the Android key and attestation archive for the pending Key step.
    pub fn capture_android_original(
        &mut self,
        key: KagemushaDevicePublicKeyV1,
        archive: &[u8],
    ) -> Result<()> {
        self.recheck_custody()?;
        let b = encode(&Raw {
            key,
            archive: archive.to_vec(),
        })?;
        self.check_capture(Step::Key, &b)?;
        self.persist(Record::Captured {
            step: 2,
            original: b,
        })
    }
    /// Fence the RawIssuer step and return the challenge and attestation archive originals.
    pub fn fence_raw_issuer(&mut self) -> Result<(Vec<u8>, Vec<u8>)> {
        self.start(Step::RawIssuer, vec![])?;
        Ok((
            self.c.clone().ok_or(Rejected)?,
            self.raw.as_ref().ok_or(Rejected)?.archive.clone(),
        ))
    }
    /// Capture the raw issuer admission original for the pending RawIssuer step.
    pub fn accept_raw_admission(&mut self, original: &[u8]) -> Result<()> {
        self.recheck_custody()?;
        self.check_capture(Step::RawIssuer, original)?;
        self.persist(Record::Captured {
            step: 3,
            original: original.to_vec(),
        })
    }
    fn possession(&self) -> Result<KagemushaHardwareEvidencePossessionV1> {
        let c = self.challenge()?.challenge;
        let raw = self.raw.as_ref().ok_or(Rejected)?;
        Ok(KagemushaHardwareEvidencePossessionV1 {
            manifest_digest: self.binding.digest,
            operation_id: self.reservation.operation_id,
            challenge_digest: self.challenge()?.original_digest().map_err(|_| Rejected)?,
            alias_digest: sha(self.reservation.alias.as_bytes()),
            raw_original_sha256: sha(&raw.archive),
            attested_key_id: sha(raw.key.as_sec1_bytes()),
            google_owner_binding: c.google_owner_binding,
            app_public_key: *raw.key.as_sec1_bytes(),
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        })
    }
    /// Fence the Possession step and return the exact possession signing bytes.
    pub fn fence_possession(&mut self) -> Result<Vec<u8>> {
        self.start(Step::Possession, vec![])?;
        self.possession()?.signing_bytes().map_err(|_| Rejected)
    }
    /// Capture the DER possession signature for the pending Possession step.
    pub fn capture_possession(&mut self, der: &[u8]) -> Result<()> {
        self.recheck_custody()?;
        self.check_capture(Step::Possession, der)?;
        self.persist(Record::Captured {
            step: 4,
            original: der.to_vec(),
        })
    }
    /// Return the cloud project number and integrity request hash over all captured originals.
    pub fn integrity_selection(&self) -> Result<(u64, [u8; 32])> {
        self.recheck_custody()?;
        let e = self.possession()?.signing_bytes().map_err(|_| Rejected)?;
        let raw = self.raw.as_ref().ok_or(Rejected)?;
        Ok((
            self.binding.manifest.google_cloud_project_number,
            kagemusha_hardware_evidence_integrity_request_hash_v1(
                self.c.as_deref().ok_or(Rejected)?,
                &raw.archive,
                self.raw_admission.as_deref().ok_or(Rejected)?,
                &e,
                self.der.as_deref().ok_or(Rejected)?,
            ),
        ))
    }
    /// Fence the Integrity step and return its exact integrity selection.
    pub fn fence_integrity(&mut self) -> Result<(u64, [u8; 32])> {
        self.start(Step::Integrity, vec![])?;
        self.integrity_selection()
    }
    /// Capture the opaque integrity token for the pending Integrity step.
    pub fn capture_integrity_original(&mut self, opaque_token: &[u8]) -> Result<()> {
        self.recheck_custody()?;
        self.check_capture(Step::Integrity, opaque_token)?;
        self.persist(Record::Captured {
            step: 5,
            original: opaque_token.to_vec(),
        })
    }
    /// Fence the Receipt step and return every captured original for the issuer request.
    pub fn fence_receipt(
        &mut self,
    ) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> {
        self.start(Step::Receipt, vec![])?;
        Ok((
            self.c.clone().ok_or(Rejected)?,
            self.raw.as_ref().ok_or(Rejected)?.archive.clone(),
            self.raw_admission.clone().ok_or(Rejected)?,
            self.possession()?.signing_bytes().map_err(|_| Rejected)?,
            self.der.clone().ok_or(Rejected)?,
            self.token.as_ref().ok_or(Rejected)?.to_vec(),
        ))
    }
    /// Capture the hardware receipt original for the pending Receipt step.
    pub fn accept_hardware_receipt(&mut self, original: &[u8]) -> Result<()> {
        self.recheck_custody()?;
        self.check_capture(Step::Receipt, original)?;
        self.persist(Record::Captured {
            step: 6,
            original: original.to_vec(),
        })
    }
    /// Original data only: no monetary credential or readiness conversion exists.
    /// Exact pending E and held public key, never an arbitrary signing subject/key selector.
    pub fn pending_possession_selection(
        &self,
    ) -> Result<(
        String,
        [u8; 32],
        [u8; 65],
        [u8; 32],
        Vec<u8>,
        Vec<KagemushaAppKeySecurityLevelV1>,
    )> {
        self.effect_interval()?;
        if self.state.pending() != Some(Step::Possession) {
            return Err(Rejected);
        }
        let e = self.possession()?;
        Ok((
            self.reservation.alias.clone(),
            self.challenge()?.original_digest().map_err(|_| Rejected)?,
            e.app_public_key,
            e.attested_key_id,
            e.signing_bytes().map_err(|_| Rejected)?,
            self.binding
                .manifest
                .allowed_android_security_levels
                .clone(),
        ))
    }
    /// Private managed effect guard; cancelled/elapsed owners cannot start another platform call.
    pub fn recheck_pending_effect(&self, step: u8) -> Result<()> {
        self.effect_interval()?;
        let s = Step::from_tag(step).ok_or(Rejected)?;
        if self.state.pending() != Some(s) || self.state.cancelled() || self.state.disposed() {
            return Err(Rejected);
        }
        Ok(())
    }
    /// Return the pending step tag, if any.
    pub fn pending_step(&self) -> Result<Option<u8>> {
        self.recheck_custody()?;
        Ok(self.state.pending().map(|s| s as u8))
    }
    /// Return the last completed step tag.
    pub fn completed_step(&self) -> Result<u8> {
        self.recheck_custody()?;
        Ok(self.state.completed())
    }
    /// Return the captured hardware receipt original, if any.
    pub fn original_receipt(&self) -> Result<Option<&[u8]>> {
        self.recheck_custody()?;
        Ok(self.receipt.as_deref())
    }
    /// Public route and OAuth selections from the same authenticated hardware-only manifest.
    /// These are data originals, not wallet/session or financial authority.
    pub fn transport_scope(&self) -> Result<(&str, &str, &str)> {
        self.recheck_custody()?;
        Ok((
            &self.binding.manifest.core_origin,
            &self.binding.manifest.google_oauth_client_id,
            &self.binding.manifest.google_oauth_issuer,
        ))
    }
    /// Return the encoded reservation original.
    pub fn reservation_original(&self) -> Result<Vec<u8>> {
        self.recheck_custody()?;
        encode(&self.reservation)
    }
    /// Return the reservation deadline in milliseconds.
    pub fn authoritative_deadline_ms(&self) -> Result<u64> {
        self.recheck_custody()?;
        Ok(self.reservation.deadline_ms)
    }
    /// Persist a cancellation request when the lifecycle permits it.
    pub fn request_cancel(&mut self) -> Result<()> {
        let mut next = self.state;
        next.cancel().map_err(|_| Rejected)?;
        self.persist(Record::CancelRequested)
    }
    /// Only a known settled or untouched/cancelled attempt can be disposed. Unknown remains held.
    /// This retains WAL and alias intent; it never deletes a key or creates a successor.
    pub fn dispose_terminal(&mut self) -> Result<()> {
        let mut next = self.state;
        next.dispose().map_err(|_| UnknownOutcome)?;
        self.persist(Record::Disposed)
    }
    fn challenge(&self) -> Result<KagemushaSignedHardwareEvidenceChallengeV1> {
        decode(self.c.as_deref().ok_or(Rejected)?)
    }
    fn validate_invocation(&self, r: &Record) -> Result<()> {
        let Record::Invoked { step, input } = r else {
            return Err(Rejected);
        };
        let s = Step::from_tag(*step).ok_or(Rejected)?;
        self.state.require_start(s).map_err(|_| UnknownOutcome)?;
        if s == Step::Prepare {
            if input.len() != 32 || input.iter().all(|b| *b == 0) {
                return Err(Rejected);
            }
        } else if !input.is_empty() {
            return Err(Rejected);
        }
        Ok(())
    }
    fn check_capture(&self, s: Step, b: &[u8]) -> Result<()> {
        if self.state.pending() != Some(s) {
            return Err(Rejected);
        }
        let m = &self.binding.manifest;
        let now = self.binding.interval()?;
        match s {
            Step::Prepare => {
                let c: KagemushaSignedHardwareEvidenceChallengeV1 = decode(b)?;
                let v = &c.challenge;
                c.signature
                    .verify(
                        &m.evidence_issuer,
                        &v.signing_bytes().map_err(|_| Rejected)?,
                    )
                    .map_err(|_| Rejected)?;
                if v.reservation_digest != self.reservation.digest().map_err(|_| Rejected)?
                    || v.manifest_digest != self.binding.digest
                    || v.operation_id != self.reservation.operation_id
                    || v.client_nonce != self.reservation.client_nonce
                    || v.alias_digest != sha(self.reservation.alias.as_bytes())
                    || Some(v.google_id_token_original_sha256) != self.oauth_sha
                    || v.issued_at_ms < self.reservation.issued_at_ms
                    || v.issued_at_ms > now.upper_ms()
                    || v.expires_at_ms > self.reservation.deadline_ms
                {
                    return Err(Rejected);
                }
            }
            Step::Key => {
                let raw: Raw = decode(b)?;
                raw.key.validate().map_err(|_| Rejected)?;
                let archive =
                    KagemushaPlatformAttestationOriginalV1::decode_canonical_exact(&raw.archive)
                        .map_err(|_| Rejected)?;
                if archive.android_certificate_chain_der().is_none()
                    || archive.canonical_bytes().map_err(|_| Rejected)? != raw.archive
                {
                    return Err(Rejected);
                }
            }
            Step::RawIssuer => {
                let a: KagemushaSignedHardwareEvidenceRawAdmissionV1 = decode(b)?;
                let v = &a.admission;
                a.signature
                    .verify(
                        &m.evidence_issuer,
                        &v.signing_bytes().map_err(|_| Rejected)?,
                    )
                    .map_err(|_| Rejected)?;
                let raw = self.raw.as_ref().ok_or(Rejected)?;
                let c = self.challenge()?.challenge;
                if v.challenge_digest
                    != self.challenge()?.original_digest().map_err(|_| Rejected)?
                    || v.raw_original_sha256 != sha(&raw.archive)
                    || v.app_public_key != raw.key
                    || v.attested_key_id != sha(raw.key.as_sec1_bytes())
                    || v.raw_verifier_policy_digest != m.raw_verifier_policy_digest
                    || !m
                        .allowed_android_security_levels
                        .contains(&v.security_level)
                    || v.checked_at_ms < c.issued_at_ms
                    || v.checked_at_ms > now.upper_ms()
                    || v.checked_at_ms >= c.expires_at_ms
                {
                    return Err(Rejected);
                }
            }
            Step::Possession => {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: b.to_vec(),
                }
                .authenticate_signature(
                    KagemushaHardwarePlatformClassV1::AndroidKeyMint,
                    &self.raw.as_ref().ok_or(Rejected)?.key,
                    m.app_signing_identity_digest,
                    m.app_distribution_digest,
                    None,
                    &self.possession()?.signing_bytes().map_err(|_| Rejected)?,
                )
                .map_err(|_| Rejected)?;
            }
            Step::Integrity => {
                if b.is_empty() || b.len() > 64 * 1024 || !b.iter().all(|v| v.is_ascii_graphic()) {
                    return Err(Rejected);
                }
            }
            Step::Receipt => {
                let signed: KagemushaSignedHardwareEvidenceReceiptV1 = decode(b)?;
                let v = &signed.receipt;
                signed
                    .signature
                    .verify(
                        &m.evidence_issuer,
                        &v.signing_bytes().map_err(|_| Rejected)?,
                    )
                    .map_err(|_| Rejected)?;
                let raw = self.raw.as_ref().ok_or(Rejected)?;
                let c = self.challenge()?.challenge;
                let e = self.possession()?.signing_bytes().map_err(|_| Rejected)?;
                if v.manifest_digest != self.binding.digest
                    || v.reservation_digest != self.reservation.digest().map_err(|_| Rejected)?
                    || v.challenge_digest
                        != self.challenge()?.original_digest().map_err(|_| Rejected)?
                    || v.raw_original_sha256 != sha(&raw.archive)
                    || v.raw_admission_original_sha256
                        != sha(self.raw_admission.as_deref().ok_or(Rejected)?)
                    || v.possession_message_sha256 != sha(&e)
                    || v.possession_der_sha256 != sha(self.der.as_deref().ok_or(Rejected)?)
                    || v.integrity_token_original_sha256
                        != sha(self.token.as_deref().ok_or(Rejected)?)
                    || v.integrity_request_hash != self.integrity_selection()?.1
                    || v.integrity_policy_digest != m.play_integrity_policy.policy_digest
                    || v.google_owner_binding != c.google_owner_binding
                    || v.verified_at_ms < c.issued_at_ms
                    || v.verified_at_ms > now.upper_ms()
                    || v.verified_at_ms >= c.expires_at_ms
                    || v.expires_at_ms > m.expires_at_ms
                    || v.expires_at_ms - v.verified_at_ms
                        > m.play_integrity_policy.maximum_refresh_interval_ms
                {
                    return Err(Rejected);
                }
            }
        }
        Ok(())
    }
    fn apply(&mut self, r: Record) -> Result<()> {
        match r {
            Record::Reserved(_) => return Err(Custody),
            Record::Invoked { step, input } => {
                let r = Record::Invoked { step, input };
                self.validate_invocation(&r)?;
                let Record::Invoked { input, .. } = r else {
                    unreachable!()
                };
                let s = Step::from_tag(step).ok_or(Rejected)?;
                if s == Step::Prepare {
                    self.oauth_sha = Some(input.as_slice().try_into().map_err(|_| Rejected)?);
                }
                self.state.invoked(s).map_err(|_| Custody)?;
            }
            Record::Captured { step, original } => {
                let s = Step::from_tag(step).ok_or(Rejected)?;
                self.check_capture(s, &original)?;
                match s {
                    Step::Prepare => self.c = Some(original),
                    Step::Key => self.raw = Some(decode(&original)?),
                    Step::RawIssuer => self.raw_admission = Some(original),
                    Step::Possession => self.der = Some(original),
                    Step::Integrity => self.token = Some(Zeroizing::new(original)),
                    Step::Receipt => self.receipt = Some(original),
                }
                self.state.captured(s).map_err(|_| Custody)?;
            }
            Record::CancelRequested => self.state.cancel().map_err(|_| Rejected)?,
            Record::Disposed => self.state.dispose().map_err(|_| UnknownOutcome)?,
        }
        Ok(())
    }
}
fn sha(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}
fn encode<T: norito::NoritoSerialize>(v: &T) -> Result<Vec<u8>> {
    hardware_bootstrap_encode_v1(v).map_err(|_| Rejected)
}
fn decode<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
    b: &[u8],
) -> Result<T> {
    hardware_bootstrap_decode_v1(b).map_err(|_| Rejected)
}
