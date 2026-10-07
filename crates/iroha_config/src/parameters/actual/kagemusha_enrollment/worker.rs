//! Concrete operator-selected Linux private verifier installation.

use super::*;

/// Exact installed program originals and exclusive private verifier storage.
///
/// These pins select trusted programs. They do not attest the operating system or prove the
/// complete Python/OpenSSL dependency closure; those remain trusted deployment components.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaEnrollmentWorker {
    /// Existing Python interpreter executable; the process owner executes the retained file.
    pub python_executable: PathBuf,
    /// Exact authenticated operator pin for that interpreter original.
    pub python_sha256: [u8; 32],
    /// Sole frozen wallet E1 verifier zipapp original.
    pub verifier_archive: PathBuf,
    /// Exact authenticated operator pin for the complete zipapp original.
    pub verifier_sha256: [u8; 32],
    /// Existing public OpenSSL executable used by the verifier.
    pub openssl_executable: PathBuf,
    /// Exact authenticated operator pin for that executable original.
    pub openssl_sha256: [u8; 32],
    /// Exact public platform root original, also checked against the enrollment policy pin.
    pub attestation_root: PathBuf,
    /// Existing private, explicitly initialized verifier journal directory.
    pub store_directory: PathBuf,
    /// Complete bound for one private process exchange, including its external checks.
    pub exchange_timeout: Duration,
    /// Required only by the Android platform policy; never a caller-supplied credential.
    pub google: Option<KagemushaEnrollmentGoogle>,
}

/// Exact public Google decoder policy and independently held private OAuth credential.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaEnrollmentGoogle {
    /// Complete existing public decoder-policy JSON original.
    pub policy_original: PathBuf,
    /// Exact authenticated operator pin for that original.
    pub policy_sha256: [u8; 32],
    /// Existing private Google OAuth credential original; no secret appears in config.
    pub oauth_credential: PathBuf,
}

impl std::fmt::Debug for KagemushaEnrollmentWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaEnrollmentWorker")
            .field("exchange_timeout", &self.exchange_timeout)
            .field("google_selected", &self.google.is_some())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for KagemushaEnrollmentGoogle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaEnrollmentGoogle").finish_non_exhaustive()
    }
}
