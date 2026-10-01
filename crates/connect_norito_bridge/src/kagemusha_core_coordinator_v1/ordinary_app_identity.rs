//! Called native C21 owner. App frames supply an original enrollment selector and bounded raw
//! platform originals; they cannot create C, replace policy/issuer/time or approve themselves.
//! Monetary methods stay unavailable until a separate genuine constrained financial owner exists.
use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1 as Method, install_kagemusha_core_coordinator_backend_v1,
    kagemusha_core_coordinator_decode_request_v1, kagemusha_core_coordinator_encode_response_v1,
    kagemusha_core_coordinator_validate_method_request_v1,
    kagemusha_core_coordinator_validate_method_response_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
};
use iroha_core_zk::kagemusha_v1_state::{
    KagemushaOrdinaryAppEnrollmentAttemptV1 as Attempt,
    KagemushaOrdinaryPreparationReservationV1 as Reservation,
    KagemushaOrdinaryPreparationSelectedOriginalsV1 as Selected,
};
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;
use std::{
    fs::{File, OpenOptions},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

/// Actual native choice between a never-created attempt and recovery of its original WAL.
/// This is supplied by the installed account/custody source, never a mobile request flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryEnrollmentDispositionV1 {
    /// Reserve exactly one fresh original; existing storage is refused without a fallback.
    Fresh,
    /// Reopen the exact previously retained original; missing storage is never recreated.
    Recover,
}
/// Concrete retained native account/release/policy selection and original storage directory.
/// This source performs no issuer HTTP callback. Mobile transports return untrusted originals
/// through explicit C21 intake, and the actual reservation/Attempt owners authenticate them.
pub struct KagemushaNativeOrdinaryAppIdentitySourceV1 {
    path: PathBuf,
    directory: File,
    selected: Arc<Selected>,
    preparation_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    platform_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    integrity_policy_original: Option<Vec<u8>>,
}
impl KagemushaNativeOrdinaryAppIdentitySourceV1 {
    /// Bind actual independently selected native originals before any mobile call.
    /// Dispositions originate in native startup's exact original recovery state, never frame data.
    /// No decoded account/settings or unsigned policy can supply the selected opaque holder.
    /// # Errors
    /// Rejects another directory, expired selection or an Integrity original not pinned by trust.
    pub fn from_native_selected_originals(
        path: PathBuf,
        selected: Arc<Selected>,
        preparation_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
        platform_disposition: KagemushaOrdinaryEnrollmentDispositionV1,
        integrity_policy_original: Option<Vec<u8>>,
    ) -> Result<Self, Error> {
        use sha2::{Digest as _, Sha256};
        kagemusha_core_coordinator_validate_storage_path_v1(
            path.to_str().ok_or(Error::Rejected)?.as_bytes(),
        )
        .map_err(|_| Error::Rejected)?;
        if path.canonicalize().map_err(|_| Error::Rejected)? != path {
            return Err(Error::Rejected);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| Error::Rejected)?;
        match (
            selected
                .integrity_policy_digest()
                .map_err(|_| Error::Rejected)?,
            &integrity_policy_original,
        ) {
            (None, None) => {}
            (Some(expected), Some(raw))
                if !raw.is_empty()
                    && raw.len() <= 16 * 1024
                    && <[u8; 32]>::from(Sha256::digest(raw)) == expected => {}
            _ => return Err(Error::Rejected),
        }
        let this = Self {
            path,
            directory,
            selected,
            preparation_disposition,
            platform_disposition,
            integrity_policy_original,
        };
        this.recheck_originals(&this.path)?;
        Ok(this)
    }
    fn recheck_originals(&self, path: &Path) -> Result<(), Error> {
        self.selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        let held = self.directory.metadata().map_err(|_| Error::Rejected)?;
        let current = std::fs::symlink_metadata(&self.path).map_err(|_| Error::Rejected)?;
        if path != self.path
            || !held.is_dir()
            || !current.is_dir()
            || current.file_type().is_symlink()
            || held.dev() != current.dev()
            || held.ino() != current.ino()
            || held.uid() != current.uid()
            || held.gid() != current.gid()
            || held.mode() != current.mode()
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
    fn original_enrollment_id(&self, path: &Path) -> Result<[u8; 32], Error> {
        self.recheck_originals(path)?;
        self.selected.enrollment_id().map_err(|_| Error::Rejected)
    }
    fn reserve_preparation(&self, path: &Path) -> Result<Reservation, Error> {
        self.recheck_originals(path)?;
        let now = self
            .selected
            .trusted_time_ms()
            .map_err(|_| Error::Rejected)?;
        let result = match self.preparation_disposition {
            KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                Reservation::create(path, self.selected.clone(), now)
            }
            KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                Reservation::open_existing(path, self.selected.clone(), now)
            }
        }
        .map_err(|_| Error::Rejected)?;
        self.recheck_originals(path)?;
        Ok(result)
    }
}
/// Install-once ordinary source registration failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryAppIdentityInstallErrorV1 {
    /// The process already retains an independently selected ordinary source.
    AlreadyInstalled,
}
static SOURCE: OnceLock<Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>> = OnceLock::new();
struct Installation {
    attempted_path: Option<Box<str>>,
    succeeded: bool,
}
static INSTALL: Mutex<Installation> = Mutex::new(Installation {
    attempted_path: None,
    succeeded: false,
});
/// Register the actual native original source before the application calls install.
/// C/JNI cannot register policy/keys/providers or construct an opaque C owner from decoded bytes.
/// This does not install financial capability or qualify a deployment.
/// # Errors
/// Refuses replacing an existing source.
pub fn register_kagemusha_native_ordinary_app_identity_source_v1(
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
) -> Result<(), KagemushaOrdinaryAppIdentityInstallErrorV1> {
    SOURCE
        .set(source)
        .map_err(|_| KagemushaOrdinaryAppIdentityInstallErrorV1::AlreadyInstalled)
}
pub(super) fn has_registered_source() -> bool {
    SOURCE.get().is_some()
}
pub(super) fn provision_and_install(path: &str) -> Result<(), Error> {
    kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
        .map_err(|_| Error::Rejected)?;
    let source = SOURCE.get().ok_or(Error::Unavailable)?.clone();
    let path_buf = PathBuf::from(path);
    source.recheck_originals(&path_buf)?;
    let mut installed = INSTALL.lock().map_err(|_| Error::Rejected)?;
    if let Some(original) = installed.attempted_path.as_deref() {
        return if original == path && installed.succeeded {
            source.recheck_originals(&path_buf)
        } else {
            Err(Error::Rejected)
        };
    }
    let backend = Arc::new(OrdinaryBackend {
        path: path_buf,
        source,
        owner: Mutex::new(Owner::default()),
    });
    backend.source.recheck_originals(&backend.path)?;
    // Any uncertain installation is frozen. Never retry replacement of process-global ownership.
    installed.attempted_path = Some(path.into());
    install_kagemusha_core_coordinator_backend_v1(backend).map_err(|_| Error::Rejected)?;
    installed.succeeded = true;
    Ok(())
}
#[derive(Default)]
struct Owner {
    opened: bool,
    handle: Option<u64>,
    attempted: Option<[u8; 32]>,
    reservation_started: bool,
    reservation: Option<Reservation>,
    attempt: Option<Attempt>,
}
struct OrdinaryBackend {
    path: PathBuf,
    source: Arc<KagemushaNativeOrdinaryAppIdentitySourceV1>,
    owner: Mutex<Owner>,
}
impl OrdinaryBackend {
    fn invoke_identity(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>, Error> {
        let method = Method::PreparedOrdinaryAppIdentity;
        kagemusha_core_coordinator_validate_method_request_v1(method, frame)
            .map_err(|_| Error::Rejected)?;
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(
            fields[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
        );
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        let response = if phase == 11 {
            let id = self.source.original_enrollment_id(&self.path)?;
            self.source.recheck_originals(&self.path)?;
            if id == [0; 32] || owner.attempted.is_some_and(|original| original != id) {
                return Err(Error::Rejected);
            }
            vec![id.to_vec()]
        } else if phase == 12 {
            if owner.reservation.is_none() {
                if owner.reservation_started {
                    return Err(Error::Rejected);
                }
                owner.reservation_started = true;
                owner.reservation = Some(self.source.reserve_preparation(&self.path)?);
            }
            let held = owner.reservation.as_ref().ok_or(Error::Rejected)?;
            let carrier = held.carrier().map_err(|_| Error::Rejected)?;
            let mut uuid = carrier.client_nonce[..16].to_vec();
            uuid[6] = (uuid[6] & 0x0f) | 0x40;
            uuid[8] = (uuid[8] & 0x3f) | 0x80;
            let u = hex::encode(uuid);
            let request_id = format!(
                "{}-{}-{}-{}-{}",
                &u[..8],
                &u[8..12],
                &u[12..16],
                &u[16..20],
                &u[20..]
            );
            vec![
                held.ticket()
                    .map_err(|_| Error::Rejected)?
                    .to_le_bytes()
                    .to_vec(),
                carrier.account_i105.as_bytes().to_vec(),
                carrier.client_nonce.to_vec(),
                carrier.release_id.to_vec(),
                carrier.hardware_profile_id.to_vec(),
                carrier.lane_id.to_vec(),
                carrier.financial_authority_commitment.to_vec(),
                request_id.into_bytes(),
            ]
        } else if phase == 13 {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            let reservation = owner.reservation.as_mut().ok_or(Error::Rejected)?;
            if reservation.ticket().map_err(|_| Error::Rejected)? != ticket {
                return Err(Error::Rejected);
            }
            reservation
                .retain_preparation(&fields[2])
                .map_err(|_| Error::Rejected)?;
            if owner.attempt.is_none() {
                let id = self.source.original_enrollment_id(&self.path)?;
                if owner.attempted.is_some() {
                    return Err(Error::Rejected);
                }
                owner.attempted = Some(id);
                let prepared = owner
                    .reservation
                    .as_ref()
                    .ok_or(Error::Rejected)?
                    .prepared_owner()
                    .map_err(|_| Error::Rejected)?;
                let now = self
                    .source
                    .selected
                    .trusted_time_ms()
                    .map_err(|_| Error::Rejected)?;
                let attempt = match self.source.platform_disposition {
                    KagemushaOrdinaryEnrollmentDispositionV1::Fresh => {
                        Attempt::create(&self.path, prepared, now)
                    }
                    KagemushaOrdinaryEnrollmentDispositionV1::Recover => {
                        Attempt::open_existing(&self.path, prepared, now)
                    }
                }
                .map_err(|_| Error::Rejected)?;
                owner.attempt = Some(attempt);
            }
            owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .preparation_fields()
                .map_err(|_| Error::Rejected)?
        } else if phase == 14 {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            if owner
                .reservation
                .as_ref()
                .ok_or(Error::Rejected)?
                .ticket()
                .map_err(|_| Error::Rejected)?
                != ticket
            {
                return Err(Error::Rejected);
            }
            vec![
                self.source
                    .integrity_policy_original
                    .clone()
                    .unwrap_or_default(),
            ]
        } else {
            let ticket = u64::from_le_bytes(
                fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?,
            );
            let attempt = owner.attempt.as_mut().ok_or(Error::Rejected)?;
            if attempt.ticket() != ticket {
                return Err(Error::Rejected);
            }
            attempt.recheck().map_err(|_| Error::Rejected)?;
            match phase {
                2 => attempt.fence_generation().map_err(|_| Error::Rejected)?,
                3 => vec![
                    attempt
                        .retain_key_reference(
                            std::str::from_utf8(&fields[2]).map_err(|_| Error::Rejected)?,
                        )
                        .map_err(|_| Error::Rejected)?
                        .to_vec(),
                ],
                4 => attempt.fence_attestation().map_err(|_| Error::Rejected)?,
                5 => {
                    let point = KagemushaDevicePublicKeyV1::from_sec1_bytes(&fields[2])
                        .map_err(|_| Error::Rejected)?;
                    // Shape checked above before allocation; at most128KiB original retained once.
                    let mut original = Vec::with_capacity(fields[3].len() + fields[4].len());
                    original.extend_from_slice(&fields[3]);
                    original.extend_from_slice(&fields[4]);
                    attempt
                        .retain_raw(point, &original)
                        .map_err(|_| Error::Rejected)?
                }
                6 => attempt
                    .accept_raw_admission(&fields[2])
                    .map_err(|_| Error::Rejected)?,
                7 => attempt.recovery_fields().map_err(|_| Error::Rejected)?,
                8 => attempt.recheck_fields().map_err(|_| Error::Rejected)?,
                9 => {
                    attempt.cancel().map_err(|_| Error::Rejected)?;
                    Vec::new()
                }
                10 => attempt
                    .raw_chunk_fields(u32::from_le_bytes(
                        fields[2]
                            .as_slice()
                            .try_into()
                            .map_err(|_| Error::Rejected)?,
                    ))
                    .map_err(|_| Error::Rejected)?,
                _ => return Err(Error::Rejected),
            }
        };
        self.source.recheck_originals(&self.path)?;
        if !matches!(phase, 9 | 11 | 12 | 14) {
            owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .recheck()
                .map_err(|_| Error::Rejected)?;
        }
        let response = kagemusha_core_coordinator_encode_response_v1(&response)
            .map_err(|_| Error::Rejected)?;
        kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
            .map_err(|_| Error::Rejected)?;
        Ok(response)
    }
}
impl KagemushaCoreCoordinatorBackendV1 for OrdinaryBackend {
    fn open(&self, path: &str) -> Result<u64, Error> {
        if self.path != Path::new(path) {
            return Err(Error::Rejected);
        }
        self.source.recheck_originals(&self.path)?;
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.opened {
            return Err(Error::Rejected);
        }
        owner.opened = true;
        owner.handle = Some(1);
        self.source.recheck_originals(&self.path)?;
        Ok(1)
    }
    fn invoke(&self, handle: u64, method: Method, frame: &[u8]) -> Result<Vec<u8>, Error> {
        if method != Method::PreparedOrdinaryAppIdentity {
            // Identity enrollment supplies no recursive proof, mint or financial StateGuard.
            // TODO: compose19/20 with genuine admitted original owners and constrained Eq/Ep/mint
            // before changing monetary rejection; no host-decoded archive is that owner.
            return Err(Error::Unavailable);
        }
        self.invoke_identity(handle, frame)
    }
    fn close(&self, handle: u64) -> Result<(), Error> {
        let mut owner = self.owner.lock().map_err(|_| Error::Rejected)?;
        if owner.handle != Some(handle) {
            return Err(Error::Rejected);
        }
        owner.handle = None;
        owner.attempt = None;
        owner.reservation = None;
        self.source.recheck_originals(&self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        kagemusha::*,
        testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture,
    };
    use p256::ecdsa::SigningKey;
    use sha2::{Digest as _, Sha256};
    fn backend() -> (tempfile::TempDir, OrdinaryBackend, Fixture) {
        let f = Fixture::new(true);
        let key = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
        let core_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap();
        let selected = Arc::new(
            Selected::from_selected_originals(
                f.selection.owner.clone(),
                f.release.clone(),
                f.issuer_policy.clone(),
                f.trust.clone(),
                f.app_authority.clone(),
                f.selection.preparation.challenge.hardware_profile_id,
                &core_key,
                300,
            )
            .unwrap(),
        );
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        let source = Arc::new(
            KagemushaNativeOrdinaryAppIdentitySourceV1::from_native_selected_originals(
                path.clone(),
                selected,
                KagemushaOrdinaryEnrollmentDispositionV1::Fresh,
                KagemushaOrdinaryEnrollmentDispositionV1::Fresh,
                None,
            )
            .unwrap(),
        );
        (
            temp,
            OrdinaryBackend {
                path,
                source,
                owner: Mutex::new(Owner::default()),
            },
            f,
        )
    }
    fn call(
        b: &OrdinaryBackend,
        h: u64,
        phase: u32,
        mut fields: Vec<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>, Error> {
        fields.insert(0, phase.to_le_bytes().to_vec());
        let q = super::super::kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        let result = b.invoke(h, Method::PreparedOrdinaryAppIdentity, &q)?;
        super::super::kagemusha_core_coordinator_decode_response_v1(&result)
            .map_err(|_| Error::Rejected)
    }
    fn prepare(b: &OrdinaryBackend, h: u64, f: &Fixture) -> Vec<Vec<u8>> {
        let carrier = call(b, h, 12, vec![]).unwrap();
        assert_eq!(carrier.len(), 8);
        assert_eq!(call(b, h, 12, vec![]).unwrap(), carrier);
        let mut c = f.selection.preparation.challenge;
        c.client_nonce = carrier[2].as_slice().try_into().unwrap();
        c.financial_authority_commitment = carrier[6].as_slice().try_into().unwrap();
        c.issued_at_ms = b.source.selected.trusted_time_ms().unwrap();
        c.expires_at_ms = c.issued_at_ms + 1900;
        let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let signed = KagemushaSignedOrdinaryAppEnrollmentChallengeV1 {
            challenge: c,
            signature: Signature::try_new(key.private_key(), &c.canonical_signing_bytes().unwrap())
                .unwrap(),
        }
        .to_transport_bytes()
        .unwrap();
        let result = call(b, h, 13, vec![carrier[0].clone(), signed.clone()]).unwrap();
        assert_eq!(result.len(), 8);
        assert_eq!(result[1], signed);
        assert_eq!(
            call(b, h, 13, vec![carrier[0].clone(), signed]).unwrap(),
            result
        );
        assert_eq!(
            call(b, h, 14, vec![carrier[0].clone()]).unwrap(),
            vec![vec![]]
        );
        result
    }
    #[test]
    fn called_c21_reserves_before_http_and_fences_identity_without_money() {
        let (_temp, b, f) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let id = call(&b, h, 11, vec![]).unwrap();
        assert_eq!(
            id,
            vec![f.selection.preparation.challenge.enrollment_id.to_vec()]
        );
        assert!(b.owner.lock().unwrap().reservation.is_none());
        assert!(call(&b, h, 1, vec![id[0].clone()]).is_err());
        let prepared = prepare(&b, h, &f);
        let ticket = prepared[0].clone();
        assert_eq!(
            call(&b, h, 2, vec![ticket.clone()]).unwrap(),
            vec![vec![1], vec![]]
        );
        assert!(call(&b, h, 2, vec![ticket.clone()]).is_err());
        assert!(b.invoke(h, Method::InitialEnrollment, &[]).is_err());
        assert!(b.invoke(h, Method::BeginSenderTransition, &[]).is_err());
        assert!(
            b.invoke(h, Method::PreparedAppOperationApproval, &[])
                .is_err()
        );
        assert_eq!(call(&b, h, 7, vec![ticket.clone()]).unwrap()[0], vec![1]);
        assert!(call(&b, h, 9, vec![ticket]).is_err());
    }
    #[test]
    fn called_c21_authenticates_explicit_raw_original_and_preserves_two_chunks() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let (_temp, b, f) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let prepared = prepare(&b, h, &f);
        let ticket = prepared[0].clone();
        let c = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(&prepared[1])
            .unwrap()
            .challenge;
        let app = f.selection.issuance.credential.subject;
        call(&b, h, 2, vec![ticket.clone()]).unwrap();
        call(
            &b,
            h,
            3,
            vec![
                ticket.clone(),
                STANDARD.encode(app.attested_key_id).into_bytes(),
            ],
        )
        .unwrap();
        call(&b, h, 4, vec![ticket.clone()]).unwrap();
        assert!(call(&b, h, 4, vec![ticket.clone()]).is_err());
        let raw = vec![23; 131072];
        let retained = call(
            &b,
            h,
            5,
            vec![
                ticket.clone(),
                app.app_public_key.as_sec1_bytes().to_vec(),
                raw[..65536].to_vec(),
                raw[65536..].to_vec(),
            ],
        )
        .unwrap();
        assert_eq!(retained[0], Sha256::digest(&raw).to_vec());
        // A real governed Ed signature isolates raw issuer/custody joins; synthetic raw bytes do
        // not establish Apple attestation, a financial proof or a physically qualified deployment.
        let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
            version: 1,
            enrollment_challenge_digest: c.attestation_challenge().unwrap(),
            authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
            platform_class: c.platform_class,
            security_level: KagemushaAppKeySecurityLevelV1::AppleAppAttest,
            app_public_key: app.app_public_key,
            attested_key_id: app.attested_key_id,
            raw_platform_evidence_digest: Sha256::digest(&raw).into(),
            app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
            original_app_attest_counter: 0,
            issued_at_ms: c.issued_at_ms,
            expires_at_ms: c.expires_at_ms,
        };
        let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let original = KagemushaRawAppAttestationAdmissionV1 {
            subject,
            signature: Signature::try_new(
                key.private_key(),
                &subject.canonical_signing_bytes().unwrap(),
            )
            .unwrap(),
        }
        .to_transport_bytes()
        .unwrap();
        assert!(call(&b, h, 6, vec![ticket.clone()]).is_err());
        assert!(call(&b, h, 6, vec![ticket.clone(), vec![1; 314]]).is_err());
        let result = call(&b, h, 6, vec![ticket.clone(), original.clone()]).unwrap();
        assert_eq!(
            call(&b, h, 6, vec![ticket.clone(), original]).unwrap(),
            result
        );
        let recovered = call(&b, h, 7, vec![ticket.clone()]).unwrap();
        assert_eq!(recovered[0], vec![5]);
        assert_eq!(recovered[5].len(), 314);
        assert_eq!(recovered[6], result[0]);
        let mut joined =
            call(&b, h, 10, vec![ticket.clone(), 0u32.to_le_bytes().to_vec()]).unwrap()[1].clone();
        joined.extend(
            call(&b, h, 10, vec![ticket.clone(), 1u32.to_le_bytes().to_vec()]).unwrap()[1].iter(),
        );
        assert_eq!(joined, raw);
        assert!(
            call(
                &b,
                h,
                5,
                vec![
                    ticket.clone(),
                    app.app_public_key.as_sec1_bytes().to_vec(),
                    vec![1],
                    vec![]
                ]
            )
            .is_err()
        );
        assert!(call(&b, h, 8, vec![2u64.to_le_bytes().to_vec()]).is_err());
    }
}
