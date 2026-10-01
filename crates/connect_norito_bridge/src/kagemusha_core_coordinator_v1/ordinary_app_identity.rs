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
    KagemushaPreparedOrdinaryAppEnrollmentV1 as Prepared,
};
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;
use std::{
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
/// Independently admitted native C owner and authenticated reference time.
/// No C/JNI DTO or decoder constructs this selection.
pub struct KagemushaNativeOrdinaryPreKeySelectionV1 {
    prepared: Prepared,
    trusted_reference_ms: u64,
    disposition: KagemushaOrdinaryEnrollmentDispositionV1,
}
impl KagemushaNativeOrdinaryPreKeySelectionV1 {
    /// Retain the actual cryptographically admitted pre-key owner under native source custody.
    /// Registration of Rust code is not account/release authority. The source must independently
    /// authenticate its current account reservation, signed original, policy and trusted time.
    /// # Errors
    /// Rejects stale original interval before filesystem or platform side effects.
    pub fn from_admitted_original(
        prepared: Prepared,
        trusted_reference_ms: u64,
        disposition: KagemushaOrdinaryEnrollmentDispositionV1,
    ) -> Result<Self, Error> {
        prepared
            .recheck_at_trusted_time(trusted_reference_ms)
            .map_err(|_| Error::Rejected)?;
        Ok(Self {
            prepared,
            trusted_reference_ms,
            disposition,
        })
    }
}
/// Rust-only ordinary identity source, registered before application installation.
///
/// Every returned pre-key owner must have independently authenticated native account/nonce/
/// financial-secret commitment reservations and actual issuer-signed C. The source selects
/// current release, policies and clock from held originals. It supplies no monetary backend,
/// final credential, hardware monotonicity or decoded-verdict capability.
pub trait KagemushaNativeOrdinaryAppIdentitySourceV1: Send + Sync + 'static {
    /// Recheck current native account approval, release/policy and original storage/issuer custody.
    /// Refuse uncertainty before each platform fence, read, protected issuer call and publication.
    fn recheck_originals(&self, storage_path: &Path) -> Result<(), Error>;
    /// Read the already reserved original native enrollment ID under held account/policy custody.
    /// This correlation projection must never allocate/generate/reselect an identity, nonce,
    /// secret or C preparation. Unavailable or uncertain original custody fails closed.
    fn original_enrollment_id(&self, storage_path: &Path) -> Result<[u8; 32], Error>;
    /// Resolve a caller-known selector to the same independently retained original native owner.
    /// This must not reserve another nonce, financial-secret commitment or issuer preparation.
    fn select_pre_key(
        &self,
        storage_path: &Path,
        enrollment_id: [u8; 32],
    ) -> Result<KagemushaNativeOrdinaryPreKeySelectionV1, Error>;
    /// Fetch the actual purpose-specific issuer raw admission for these native-held originals.
    /// Response bytes remain untrusted until native model signature/policy/time/C/key checks.
    /// The protected transport must be bounded and reject redirects, stale policy and substituted
    /// issuer custody. It must use the raw route and dedicated KRAC encoder, never final KOAC.
    fn fetch_raw_admission(
        &self,
        signed_c: &[u8],
        point: &KagemushaDevicePublicKeyV1,
        complete_raw_original: &[u8],
    ) -> Result<Vec<u8>, Error>;
}
/// Install-once ordinary source registration failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaOrdinaryAppIdentityInstallErrorV1 {
    /// The process already retains an independently selected ordinary source.
    AlreadyInstalled,
}
static SOURCE: OnceLock<Arc<dyn KagemushaNativeOrdinaryAppIdentitySourceV1>> = OnceLock::new();
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
    source: Arc<dyn KagemushaNativeOrdinaryAppIdentitySourceV1>,
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
    attempt: Option<Attempt>,
}
struct OrdinaryBackend {
    path: PathBuf,
    source: Arc<dyn KagemushaNativeOrdinaryAppIdentitySourceV1>,
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
        } else if phase == 1 {
            let id: [u8; 32] = fields[1]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?;
            if owner.attempted.is_none() {
                // Fence selection before calling native transport or touching attempt storage.
                owner.attempted = Some(id);
                let selected = self.source.select_pre_key(&self.path, id)?;
                self.source.recheck_originals(&self.path)?;
                if selected
                    .prepared
                    .original_preparation(selected.trusted_reference_ms)
                    .map_err(|_| Error::Rejected)?
                    .challenge
                    .enrollment_id
                    != id
                {
                    return Err(Error::Rejected);
                }
                let attempt = match selected.disposition {
                    KagemushaOrdinaryEnrollmentDispositionV1::Fresh => Attempt::create(
                        &self.path,
                        selected.prepared,
                        selected.trusted_reference_ms,
                    ),
                    KagemushaOrdinaryEnrollmentDispositionV1::Recover => Attempt::open_existing(
                        &self.path,
                        selected.prepared,
                        selected.trusted_reference_ms,
                    ),
                }
                .map_err(|_| Error::Rejected)?;
                self.source.recheck_originals(&self.path)?;
                owner.attempt = Some(attempt);
            } else if owner.attempted != Some(id) {
                return Err(Error::Rejected);
            }
            owner
                .attempt
                .as_ref()
                .ok_or(Error::Rejected)?
                .preparation_fields()
                .map_err(|_| Error::Rejected)?
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
                6 => {
                    if let Some(original_result) = attempt
                        .retained_raw_admission_result()
                        .map_err(|_| Error::Rejected)?
                    {
                        original_result
                    } else {
                        let (c, point, raw) = attempt
                            .raw_issuer_originals()
                            .map_err(|_| Error::Rejected)?;
                        self.source.recheck_originals(&self.path)?;
                        let original = self.source.fetch_raw_admission(&c, &point, &raw)?;
                        self.source.recheck_originals(&self.path)?;
                        attempt.recheck().map_err(|_| Error::Rejected)?;
                        if original.len() != 314 {
                            return Err(Error::Rejected);
                        }
                        attempt
                            .accept_raw_admission(&original)
                            .map_err(|_| Error::Rejected)?
                    }
                }
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
        if !matches!(phase, 9 | 11) {
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
    use sha2::{Digest as _, Sha256};
    struct Source {
        fixture: Fixture,
        path: PathBuf,
        calls: Mutex<u32>,
    }
    impl KagemushaNativeOrdinaryAppIdentitySourceV1 for Source {
        fn recheck_originals(&self, path: &Path) -> Result<(), Error> {
            if path == self.path {
                Ok(())
            } else {
                Err(Error::Rejected)
            }
        }
        fn original_enrollment_id(&self, path: &Path) -> Result<[u8; 32], Error> {
            self.recheck_originals(path)?;
            Ok(self.fixture.selection.preparation.challenge.enrollment_id)
        }
        fn select_pre_key(
            &self,
            path: &Path,
            id: [u8; 32],
        ) -> Result<KagemushaNativeOrdinaryPreKeySelectionV1, Error> {
            self.recheck_originals(path)?;
            let f = &self.fixture;
            let c = &f.selection.preparation.challenge;
            if c.enrollment_id != id {
                return Err(Error::Rejected);
            }
            let prepared = Prepared::authenticate_pre_key(
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
            .map_err(|_| Error::Rejected)?;
            KagemushaNativeOrdinaryPreKeySelectionV1::from_admitted_original(
                prepared,
                300,
                KagemushaOrdinaryEnrollmentDispositionV1::Fresh,
            )
        }
        fn fetch_raw_admission(
            &self,
            c: &[u8],
            point: &KagemushaDevicePublicKeyV1,
            raw: &[u8],
        ) -> Result<Vec<u8>, Error> {
            *self.calls.lock().unwrap() += 1;
            let f = &self.fixture;
            let expected = &f.selection.preparation.challenge;
            if c != f.selection.preparation.to_transport_bytes().unwrap() {
                return Err(Error::Rejected);
            }
            // Actual synthetic Ed signature isolates transport/custody only. No Apple attestation
            // verification or physical/platform qualification is claimed by this test source.
            let subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
                version: 1,
                enrollment_challenge_digest: expected.attestation_challenge().unwrap(),
                authority_policy_digest: f.app_authority.canonical_digest().unwrap(),
                platform_class: expected.platform_class,
                security_level: KagemushaAppKeySecurityLevelV1::AppleAppAttest,
                app_public_key: *point,
                attested_key_id: Sha256::digest(point.as_sec1_bytes()).into(),
                raw_platform_evidence_digest: Sha256::digest(raw).into(),
                app_signing_identity_digest: f.app_authority.app_signing_identity_digest,
                original_app_attest_counter: 0,
                issued_at_ms: expected.issued_at_ms,
                expires_at_ms: expected.expires_at_ms,
            };
            let key = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
            KagemushaRawAppAttestationAdmissionV1 {
                subject,
                signature: Signature::try_new(
                    key.private_key(),
                    &subject.canonical_signing_bytes().unwrap(),
                )
                .unwrap(),
            }
            .to_transport_bytes()
            .map_err(|_| Error::Rejected)
        }
    }
    fn backend() -> (tempfile::TempDir, OrdinaryBackend, Arc<Source>) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().canonicalize().unwrap();
        let source = Arc::new(Source {
            fixture: Fixture::new(true),
            path: path.clone(),
            calls: Mutex::new(0),
        });
        (
            temp,
            OrdinaryBackend {
                path,
                source: source.clone(),
                owner: Mutex::new(Owner::default()),
            },
            source,
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
    #[test]
    fn called_c21_fences_native_identity_without_a_financial_or_oem_backend() {
        let (_temp, b, source) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let id = source.fixture.selection.preparation.challenge.enrollment_id;
        let selected = call(&b, h, 11, vec![]).unwrap();
        assert_eq!(selected, vec![id.to_vec()]);
        assert!(b.owner.lock().unwrap().attempted.is_none());
        assert!(b.owner.lock().unwrap().attempt.is_none());
        let prepared = call(&b, h, 1, vec![id.to_vec()]).unwrap();
        assert_eq!(prepared.len(), 8);
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
        let recovered = call(&b, h, 7, vec![ticket.clone()]).unwrap();
        assert_eq!(recovered[0], vec![1]);
        assert!(call(&b, h, 9, vec![ticket]).is_err());
        assert_eq!(*source.calls.lock().unwrap(), 0);
    }
    #[test]
    fn called_c21_retains_two_chunks_before_native_raw_issuer_and_refuses_substitution() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let (_temp, b, source) = backend();
        let h = b.open(b.path.to_str().unwrap()).unwrap();
        let f = &source.fixture;
        let prepared = call(
            &b,
            h,
            1,
            vec![f.selection.preparation.challenge.enrollment_id.to_vec()],
        )
        .unwrap();
        let ticket = prepared[0].clone();
        call(&b, h, 2, vec![ticket.clone()]).unwrap();
        let app = f.selection.issuance.credential.subject;
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
        let result = call(&b, h, 6, vec![ticket.clone()]).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(*source.calls.lock().unwrap(), 1);
        assert_eq!(call(&b, h, 6, vec![ticket.clone()]).unwrap(), result);
        assert_eq!(*source.calls.lock().unwrap(), 1);
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
        assert!(call(&b, h, 6, vec![ticket.clone(), vec![1; 314]]).is_err());
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
