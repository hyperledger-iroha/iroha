//! Bounded concrete verifier installation parsing; no process authority comes from a digest.

use super::*;

/// Existing operator-selected private verifier installation; no default programs or stores.
#[derive(Clone, ReadConfig, norito::JsonDeserialize)]
pub struct KagemushaEnrollmentWorker {
    /// Existing Python executable original.
    pub python_executable: PathBuf,
    /// Nonzero exact interpreter SHA-256, canonical lowercase hex.
    pub python_sha256_hex: String,
    /// Existing sole wallet E1 verifier zipapp original.
    pub verifier_archive: PathBuf,
    /// Nonzero exact zipapp SHA-256, canonical lowercase hex.
    pub verifier_sha256_hex: String,
    /// Existing public OpenSSL executable original.
    pub openssl_executable: PathBuf,
    /// Nonzero exact OpenSSL SHA-256, canonical lowercase hex.
    pub openssl_sha256_hex: String,
    /// Existing public platform attestation root original.
    pub attestation_root: PathBuf,
    /// Existing private journal initialized by an explicit installation action.
    pub store_directory: PathBuf,
    /// Whole private exchange timeout, between one millisecond and five minutes.
    #[config(default = "defaults::torii::kagemusha_enrollment::WORKER_EXCHANGE_TIMEOUT_MS")]
    pub exchange_timeout_ms: u64,
    /// Android decoder policy and OAuth custody. Apple routes must omit this field.
    pub google: Option<KagemushaEnrollmentGoogle>,
}

/// Public Google decoder selection and a private credential location.
#[derive(Clone, ReadConfig, norito::JsonDeserialize)]
pub struct KagemushaEnrollmentGoogle {
    /// Existing public decoder JSON original.
    pub policy_original: PathBuf,
    /// Nonzero exact public decoder SHA-256, canonical lowercase hex.
    pub policy_sha256_hex: String,
    /// Existing private OAuth credential original.
    pub oauth_credential: PathBuf,
}

impl KagemushaEnrollmentWorker {
    pub(super) fn checked(self, platform: KagemushaWalletEnrollmentPlatformV1)
        -> Result<actual::KagemushaEnrollmentWorker, &'static str>
    {
        if !(1..=300_000).contains(&self.exchange_timeout_ms) {
            return Err("private verifier exchange timeout is outside its bound");
        }
        for path in [&self.python_executable, &self.verifier_archive, &self.openssl_executable,
            &self.attestation_root, &self.store_directory] {
            absolute_path(path)?;
        }
        let google = match (platform, self.google) {
            (KagemushaWalletEnrollmentPlatformV1::Apple { .. }, None) => None,
            (KagemushaWalletEnrollmentPlatformV1::Android { .. }, Some(value)) => {
                absolute_path(&value.policy_original)?;
                absolute_path(&value.oauth_credential)?;
                Some(actual::KagemushaEnrollmentGoogle {
                    policy_original: value.policy_original,
                    policy_sha256: digest(&value.policy_sha256_hex)?,
                    oauth_credential: value.oauth_credential,
                })
            }
            _ => return Err("Google decoder custody must match the selected platform"),
        };
        Ok(actual::KagemushaEnrollmentWorker {
            python_executable: self.python_executable,
            python_sha256: digest(&self.python_sha256_hex)?,
            verifier_archive: self.verifier_archive,
            verifier_sha256: digest(&self.verifier_sha256_hex)?,
            openssl_executable: self.openssl_executable,
            openssl_sha256: digest(&self.openssl_sha256_hex)?,
            attestation_root: self.attestation_root,
            store_directory: self.store_directory,
            exchange_timeout: Duration::from_millis(self.exchange_timeout_ms),
            google,
        })
    }
}
impl std::fmt::Debug for KagemushaEnrollmentWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaEnrollmentWorker")
            .field("exchange_timeout_ms", &self.exchange_timeout_ms)
            .field("google_selected", &self.google.is_some())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for KagemushaEnrollmentGoogle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KagemushaEnrollmentGoogle").finish_non_exhaustive()
    }
}
