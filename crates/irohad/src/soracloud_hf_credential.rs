//! Exact production qualification boundary for authenticated Hugging Face inference.
//!
//! Configuration carries only a stable provider handle, revision, and public
//! policy digest. Bearer credentials and vendor connection material remain
//! inside the deployment-owned provider, which executes the authenticated
//! request without returning credential material to `irohad`.

use std::{fmt, sync::Arc};

use iroha_config::parameters::validate_production_runtime_handle;

const MAX_HF_INFERENCE_URL_BYTES_V1: usize = 8 * 1024;
const MAX_HF_INFERENCE_HEADER_BYTES_V1: usize = 8 * 1024;
const MAX_HF_INFERENCE_BODY_BYTES_V1: usize = 64 * 1024 * 1024;
const MAX_HF_INFERENCE_RESPONSE_BYTES_V1: u64 = 64 * 1024 * 1024;

/// Public liveness and policy identity reported by the credential provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoracloudHfCredentialProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
    active: bool,
    test_only: bool,
}

impl SoracloudHfCredentialProviderQualificationV1 {
    /// Construct one public qualification report.
    #[must_use]
    pub const fn new(
        revision: u64,
        policy_digest: [u8; 32],
        active: bool,
        test_only: bool,
    ) -> Self {
        Self {
            revision,
            policy_digest,
            active,
            test_only,
        }
    }

    /// Return the exact adapter and public-policy revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Return the exact public-policy digest.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }

    /// Return whether the provider reports an active, non-revoked posture.
    #[must_use]
    pub const fn active(self) -> bool {
        self.active
    }

    /// Return whether the provider reports a test-only implementation.
    #[must_use]
    pub const fn test_only(self) -> bool {
        self.test_only
    }

    fn validate(self) -> Result<(), SoracloudHfCredentialProviderQualificationErrorV1> {
        if self.revision == 0 || self.policy_digest == [0; 32] {
            return Err(
                SoracloudHfCredentialProviderQualificationErrorV1::InvalidProviderQualification,
            );
        }
        if !self.active {
            return Err(SoracloudHfCredentialProviderQualificationErrorV1::ProviderInactive);
        }
        if self.test_only {
            return Err(SoracloudHfCredentialProviderQualificationErrorV1::TestProviderRejected);
        }
        Ok(())
    }
}

/// Exact non-secret identity expected from the deployment-owned provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoracloudHfCredentialProviderBindingV1 {
    handle: String,
    qualification: SoracloudHfCredentialProviderQualificationV1,
}

impl SoracloudHfCredentialProviderBindingV1 {
    /// Project and validate one parsed public provider binding.
    ///
    /// # Errors
    ///
    /// Rejects malformed, credential-bearing, test-marked, or zero-qualified
    /// bindings.
    pub fn try_from_config(
        binding: &iroha_config::parameters::actual::SoracloudRuntimeHfCredentialProviderBinding,
    ) -> Result<Self, SoracloudHfCredentialProviderQualificationErrorV1> {
        Self::try_new(
            binding.handle.clone(),
            SoracloudHfCredentialProviderQualificationV1::new(
                binding.revision,
                binding.policy_digest,
                true,
                false,
            ),
        )
    }

    /// Validate and construct an exact production provider binding.
    ///
    /// # Errors
    ///
    /// Rejects malformed or test-marked handles and invalid qualifications.
    pub fn try_new(
        handle: impl Into<String>,
        qualification: SoracloudHfCredentialProviderQualificationV1,
    ) -> Result<Self, SoracloudHfCredentialProviderQualificationErrorV1> {
        let handle = handle.into();
        validate_production_runtime_handle(&handle).map_err(|_| {
            SoracloudHfCredentialProviderQualificationErrorV1::InvalidProviderHandle
        })?;
        qualification.validate()?;
        Ok(Self {
            handle,
            qualification,
        })
    }

    /// Return the stable opaque provider handle.
    #[must_use]
    pub fn handle(&self) -> &str {
        &self.handle
    }

    /// Return the exact active, non-test qualification.
    #[must_use]
    pub const fn qualification(&self) -> SoracloudHfCredentialProviderQualificationV1 {
        self.qualification
    }
}

/// Payload-free failure while probing the deployment-owned provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudHfCredentialProviderProbeErrorV1 {
    /// Provider or backing credential service is unavailable.
    Unavailable,
    /// Provider refused or could not answer the public readiness probe.
    Refused,
}

/// Payload-free authenticated inference failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudHfCredentialProviderOperationErrorV1 {
    /// Provider, credential service, or authenticated endpoint is unavailable.
    Unavailable,
    /// Provider rejected the bounded request or the credential was refused.
    Refused,
    /// Provider identity, policy, or posture changed around the operation.
    QualificationChanged,
    /// Provider returned a malformed or oversized response.
    InvalidResponse,
}

/// Failure while qualifying an injected provider against exact public config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoracloudHfCredentialProviderQualificationErrorV1 {
    /// Configured or observed provider handle is malformed or test-marked.
    InvalidProviderHandle,
    /// Revision or policy digest is zero.
    InvalidProviderQualification,
    /// Provider reports an inactive or revoked posture.
    ProviderInactive,
    /// Provider reports a test-only implementation.
    TestProviderRejected,
    /// Provider readiness probing failed.
    ProviderUnavailable,
    /// Provider returned another handle.
    HandleMismatch,
    /// Provider returned another revision.
    RevisionMismatch,
    /// Provider returned another public-policy digest.
    PolicyDigestMismatch,
    /// Provider identity or posture changed across startup probes.
    ProviderDrift,
}

/// Bounded authenticated Hugging Face inference request.
pub struct SoracloudHfAuthenticatedInferenceRequestV1 {
    url: String,
    content_type: String,
    accept: Option<String>,
    body: Vec<u8>,
    maximum_response_bytes: u64,
}

impl fmt::Debug for SoracloudHfAuthenticatedInferenceRequestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoracloudHfAuthenticatedInferenceRequestV1")
            .field("url_len", &self.url.len())
            .field("content_type_len", &self.content_type.len())
            .field("has_accept", &self.accept.is_some())
            .field("body_len", &self.body.len())
            .field("maximum_response_bytes", &self.maximum_response_bytes)
            .finish_non_exhaustive()
    }
}

impl SoracloudHfAuthenticatedInferenceRequestV1 {
    /// Validate and construct one authenticated inference request.
    ///
    /// # Errors
    ///
    /// Returns [`SoracloudHfCredentialProviderOperationErrorV1::Refused`] for
    /// empty, malformed, or oversized public metadata and request bodies.
    pub fn try_new(
        url: impl Into<String>,
        content_type: impl Into<String>,
        accept: Option<String>,
        body: Vec<u8>,
        maximum_response_bytes: u64,
    ) -> Result<Self, SoracloudHfCredentialProviderOperationErrorV1> {
        let url = url.into();
        let content_type = content_type.into();
        if !valid_bounded_text(&url, MAX_HF_INFERENCE_URL_BYTES_V1)
            || !valid_bounded_text(&content_type, MAX_HF_INFERENCE_HEADER_BYTES_V1)
            || accept
                .as_deref()
                .is_some_and(|value| !valid_bounded_text(value, MAX_HF_INFERENCE_HEADER_BYTES_V1))
            || body.len() > MAX_HF_INFERENCE_BODY_BYTES_V1
            || maximum_response_bytes == 0
            || maximum_response_bytes > MAX_HF_INFERENCE_RESPONSE_BYTES_V1
        {
            return Err(SoracloudHfCredentialProviderOperationErrorV1::Refused);
        }
        Ok(Self {
            url,
            content_type,
            accept,
            body,
            maximum_response_bytes,
        })
    }

    /// Return the exact public inference URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Return the exact request content type.
    #[must_use]
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Return the optional exact response media type.
    #[must_use]
    pub fn accept(&self) -> Option<&str> {
        self.accept.as_deref()
    }

    /// Return the private request body without copying it.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Return the maximum admitted response body length.
    #[must_use]
    pub const fn maximum_response_bytes(&self) -> u64 {
        self.maximum_response_bytes
    }
}

impl Drop for SoracloudHfAuthenticatedInferenceRequestV1 {
    fn drop(&mut self) {
        self.body.fill(0);
        let _ = std::hint::black_box(&self.body);
    }
}

/// Bounded response returned by the authenticated provider.
pub struct SoracloudHfAuthenticatedInferenceResponseV1 {
    status: u16,
    content_type: Option<String>,
    content_encoding: Option<String>,
    body: Vec<u8>,
}

impl fmt::Debug for SoracloudHfAuthenticatedInferenceResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoracloudHfAuthenticatedInferenceResponseV1")
            .field("status", &self.status)
            .field("has_content_type", &self.content_type.is_some())
            .field("has_content_encoding", &self.content_encoding.is_some())
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}

impl SoracloudHfAuthenticatedInferenceResponseV1 {
    /// Validate and construct one bounded provider response.
    ///
    /// # Errors
    ///
    /// Rejects invalid status codes, unsafe header values, and responses larger
    /// than `maximum_response_bytes`.
    pub fn try_new(
        status: u16,
        content_type: Option<String>,
        content_encoding: Option<String>,
        body: Vec<u8>,
        maximum_response_bytes: u64,
    ) -> Result<Self, SoracloudHfCredentialProviderOperationErrorV1> {
        if !(100..=599).contains(&status)
            || maximum_response_bytes == 0
            || content_type
                .as_deref()
                .is_some_and(|value| !valid_bounded_text(value, MAX_HF_INFERENCE_HEADER_BYTES_V1))
            || content_encoding
                .as_deref()
                .is_some_and(|value| !valid_bounded_text(value, MAX_HF_INFERENCE_HEADER_BYTES_V1))
            || !u64::try_from(body.len()).is_ok_and(|length| {
                length <= maximum_response_bytes
                    && maximum_response_bytes <= MAX_HF_INFERENCE_RESPONSE_BYTES_V1
            })
        {
            return Err(SoracloudHfCredentialProviderOperationErrorV1::InvalidResponse);
        }
        Ok(Self {
            status,
            content_type,
            content_encoding,
            body,
        })
    }

    /// Return the HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Return the optional response content type.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Return the optional response content encoding.
    #[must_use]
    pub fn content_encoding(&self) -> Option<&str> {
        self.content_encoding.as_deref()
    }

    /// Consume the response and return its private body.
    #[must_use]
    pub fn into_body(mut self) -> Vec<u8> {
        std::mem::take(&mut self.body)
    }
}

impl Drop for SoracloudHfAuthenticatedInferenceResponseV1 {
    fn drop(&mut self) {
        self.body.fill(0);
        let _ = std::hint::black_box(&self.body);
    }
}

/// Deployment-owned Hugging Face inference credential provider.
///
/// Implementations retain bearer credentials internally and perform the
/// authenticated request themselves. Credential bytes must never be returned,
/// logged, persisted, or committed to ledger state.
pub trait SoracloudHfInferenceCredentialProviderV1: Send + Sync {
    /// Return the stable opaque production-provider handle.
    fn handle(&self) -> &str;

    /// Probe the exact public qualification and active/test posture.
    fn qualification(
        &self,
    ) -> Result<
        SoracloudHfCredentialProviderQualificationV1,
        SoracloudHfCredentialProviderProbeErrorV1,
    >;

    /// Confirm that the backing credential is available without exposing it.
    fn check_readiness(&self) -> Result<(), SoracloudHfCredentialProviderProbeErrorV1>;

    /// Execute one bounded authenticated inference request.
    ///
    /// Implementations must authenticate only endpoints admitted by the exact
    /// qualified public policy, reject redirects or endpoint substitution, and
    /// keep request metadata, bodies, and credentials out of diagnostics.
    fn execute_authenticated(
        &self,
        request: &SoracloudHfAuthenticatedInferenceRequestV1,
    ) -> Result<
        SoracloudHfAuthenticatedInferenceResponseV1,
        SoracloudHfCredentialProviderOperationErrorV1,
    >;
}

struct QualifiedSoracloudHfInferenceCredentialProviderV1 {
    binding: SoracloudHfCredentialProviderBindingV1,
    provider: Arc<dyn SoracloudHfInferenceCredentialProviderV1>,
}

impl QualifiedSoracloudHfInferenceCredentialProviderV1 {
    fn try_new(
        binding: SoracloudHfCredentialProviderBindingV1,
        provider: Arc<dyn SoracloudHfInferenceCredentialProviderV1>,
    ) -> Result<Self, SoracloudHfCredentialProviderQualificationErrorV1> {
        let first = probe_provider(provider.as_ref())?;
        validate_snapshot(&binding, provider.handle(), first)?;
        provider
            .check_readiness()
            .map_err(|_| SoracloudHfCredentialProviderQualificationErrorV1::ProviderUnavailable)?;
        let second = probe_provider(provider.as_ref())?;
        if first != second {
            return Err(SoracloudHfCredentialProviderQualificationErrorV1::ProviderDrift);
        }
        validate_snapshot(&binding, provider.handle(), second)?;
        Ok(Self { binding, provider })
    }

    fn revalidate(&self) -> Result<(), SoracloudHfCredentialProviderQualificationErrorV1> {
        let qualification = probe_provider(self.provider.as_ref())?;
        validate_snapshot(&self.binding, self.provider.handle(), qualification)
    }
}

impl SoracloudHfInferenceCredentialProviderV1
    for QualifiedSoracloudHfInferenceCredentialProviderV1
{
    fn handle(&self) -> &str {
        self.binding.handle()
    }

    fn qualification(
        &self,
    ) -> Result<
        SoracloudHfCredentialProviderQualificationV1,
        SoracloudHfCredentialProviderProbeErrorV1,
    > {
        self.revalidate().map_err(qualification_probe_error)?;
        Ok(self.binding.qualification())
    }

    fn check_readiness(&self) -> Result<(), SoracloudHfCredentialProviderProbeErrorV1> {
        self.revalidate().map_err(qualification_probe_error)?;
        self.provider.check_readiness()?;
        self.revalidate().map_err(qualification_probe_error)
    }

    fn execute_authenticated(
        &self,
        request: &SoracloudHfAuthenticatedInferenceRequestV1,
    ) -> Result<
        SoracloudHfAuthenticatedInferenceResponseV1,
        SoracloudHfCredentialProviderOperationErrorV1,
    > {
        self.revalidate()
            .map_err(|_| SoracloudHfCredentialProviderOperationErrorV1::QualificationChanged)?;
        let response = self.provider.execute_authenticated(request)?;
        if !u64::try_from(response.body.len())
            .is_ok_and(|length| length <= request.maximum_response_bytes())
        {
            return Err(SoracloudHfCredentialProviderOperationErrorV1::InvalidResponse);
        }
        self.revalidate()
            .map_err(|_| SoracloudHfCredentialProviderOperationErrorV1::QualificationChanged)?;
        Ok(response)
    }
}

/// Qualify an injected credential provider against exact public configuration.
///
/// The provider is probed twice with an intervening readiness check. The
/// returned facade revalidates the exact binding before and after every
/// authenticated inference request.
///
/// # Errors
///
/// Returns a payload-free error for missing, substituted, stale, revoked,
/// test-marked, unavailable, or unstable providers.
pub fn qualify_soracloud_hf_inference_credential_provider_v1(
    binding: SoracloudHfCredentialProviderBindingV1,
    provider: Arc<dyn SoracloudHfInferenceCredentialProviderV1>,
) -> Result<
    Arc<dyn SoracloudHfInferenceCredentialProviderV1>,
    SoracloudHfCredentialProviderQualificationErrorV1,
> {
    Ok(Arc::new(
        QualifiedSoracloudHfInferenceCredentialProviderV1::try_new(binding, provider)?,
    ))
}

fn valid_bounded_text(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn probe_provider(
    provider: &dyn SoracloudHfInferenceCredentialProviderV1,
) -> Result<
    SoracloudHfCredentialProviderQualificationV1,
    SoracloudHfCredentialProviderQualificationErrorV1,
> {
    validate_production_runtime_handle(provider.handle())
        .map_err(|_| SoracloudHfCredentialProviderQualificationErrorV1::InvalidProviderHandle)?;
    let qualification = provider
        .qualification()
        .map_err(|_| SoracloudHfCredentialProviderQualificationErrorV1::ProviderUnavailable)?;
    qualification.validate()?;
    Ok(qualification)
}

fn validate_snapshot(
    binding: &SoracloudHfCredentialProviderBindingV1,
    observed_handle: &str,
    observed: SoracloudHfCredentialProviderQualificationV1,
) -> Result<(), SoracloudHfCredentialProviderQualificationErrorV1> {
    if observed_handle != binding.handle() {
        return Err(SoracloudHfCredentialProviderQualificationErrorV1::HandleMismatch);
    }
    if observed.revision() != binding.qualification().revision() {
        return Err(SoracloudHfCredentialProviderQualificationErrorV1::RevisionMismatch);
    }
    if observed.policy_digest() != binding.qualification().policy_digest() {
        return Err(SoracloudHfCredentialProviderQualificationErrorV1::PolicyDigestMismatch);
    }
    Ok(())
}

fn qualification_probe_error(
    error: SoracloudHfCredentialProviderQualificationErrorV1,
) -> SoracloudHfCredentialProviderProbeErrorV1 {
    if error == SoracloudHfCredentialProviderQualificationErrorV1::ProviderUnavailable {
        SoracloudHfCredentialProviderProbeErrorV1::Unavailable
    } else {
        SoracloudHfCredentialProviderProbeErrorV1::Refused
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    const QUALIFICATION: SoracloudHfCredentialProviderQualificationV1 =
        SoracloudHfCredentialProviderQualificationV1::new(7, [0xA7; 32], true, false);

    struct TestProvider {
        handle: String,
        qualification: Mutex<SoracloudHfCredentialProviderQualificationV1>,
        ready: bool,
    }

    impl SoracloudHfInferenceCredentialProviderV1 for TestProvider {
        fn handle(&self) -> &str {
            &self.handle
        }

        fn qualification(
            &self,
        ) -> Result<
            SoracloudHfCredentialProviderQualificationV1,
            SoracloudHfCredentialProviderProbeErrorV1,
        > {
            Ok(*self.qualification.lock().expect("qualification lock"))
        }

        fn check_readiness(&self) -> Result<(), SoracloudHfCredentialProviderProbeErrorV1> {
            self.ready
                .then_some(())
                .ok_or(SoracloudHfCredentialProviderProbeErrorV1::Unavailable)
        }

        fn execute_authenticated(
            &self,
            request: &SoracloudHfAuthenticatedInferenceRequestV1,
        ) -> Result<
            SoracloudHfAuthenticatedInferenceResponseV1,
            SoracloudHfCredentialProviderOperationErrorV1,
        > {
            SoracloudHfAuthenticatedInferenceResponseV1::try_new(
                200,
                Some("application/json".to_owned()),
                None,
                request.body().to_vec(),
                request.maximum_response_bytes(),
            )
        }
    }

    fn fixture(
        qualification: SoracloudHfCredentialProviderQualificationV1,
        ready: bool,
    ) -> (SoracloudHfCredentialProviderBindingV1, Arc<TestProvider>) {
        let handle = "kms://soracloud/hf-inference-primary".to_owned();
        (
            SoracloudHfCredentialProviderBindingV1::try_new(handle.clone(), QUALIFICATION)
                .expect("valid exact binding"),
            Arc::new(TestProvider {
                handle,
                qualification: Mutex::new(qualification),
                ready,
            }),
        )
    }

    #[test]
    fn qualification_rejects_stale_test_and_unavailable_providers() {
        let (binding, stale) = fixture(
            SoracloudHfCredentialProviderQualificationV1::new(6, [0xA7; 32], true, false),
            true,
        );
        assert_eq!(
            qualify_soracloud_hf_inference_credential_provider_v1(binding, stale)
                .err()
                .expect("stale provider must fail"),
            SoracloudHfCredentialProviderQualificationErrorV1::RevisionMismatch
        );

        let (binding, test_only) = fixture(
            SoracloudHfCredentialProviderQualificationV1::new(7, [0xA7; 32], true, true),
            true,
        );
        assert_eq!(
            qualify_soracloud_hf_inference_credential_provider_v1(binding, test_only)
                .err()
                .expect("test provider must fail"),
            SoracloudHfCredentialProviderQualificationErrorV1::TestProviderRejected
        );

        let (binding, unavailable) = fixture(QUALIFICATION, false);
        assert_eq!(
            qualify_soracloud_hf_inference_credential_provider_v1(binding, unavailable)
                .err()
                .expect("unavailable credential must fail startup"),
            SoracloudHfCredentialProviderQualificationErrorV1::ProviderUnavailable
        );
    }

    #[test]
    fn qualified_provider_revalidates_around_operation() {
        let (binding, provider) = fixture(QUALIFICATION, true);
        let raw: Arc<dyn SoracloudHfInferenceCredentialProviderV1> = provider.clone();
        let qualified = qualify_soracloud_hf_inference_credential_provider_v1(binding, raw)
            .expect("provider must qualify");
        let request = SoracloudHfAuthenticatedInferenceRequestV1::try_new(
            "https://router.huggingface.co/models/example",
            "application/json",
            Some("application/json".to_owned()),
            br#"{"input":"private"}"#.to_vec(),
            1024,
        )
        .expect("valid request");
        let response = qualified
            .execute_authenticated(&request)
            .expect("qualified request");
        assert_eq!(response.status(), 200);

        *provider.qualification.lock().expect("qualification lock") =
            SoracloudHfCredentialProviderQualificationV1::new(8, [0xA8; 32], true, false);
        assert_eq!(
            qualified
                .execute_authenticated(&request)
                .err()
                .expect("drift must fail"),
            SoracloudHfCredentialProviderOperationErrorV1::QualificationChanged
        );
    }

    #[test]
    fn debug_output_redacts_private_request_and_response_bodies() {
        let request = SoracloudHfAuthenticatedInferenceRequestV1::try_new(
            "https://router.huggingface.co/models/example?private-query-value",
            "application/json",
            None,
            b"private-model-input".to_vec(),
            1024,
        )
        .expect("valid request");
        let response = SoracloudHfAuthenticatedInferenceResponseV1::try_new(
            200,
            Some("application/json".to_owned()),
            None,
            b"private-model-output".to_vec(),
            1024,
        )
        .expect("valid response");
        let request_debug = format!("{request:?}");
        assert!(!request_debug.contains("private-model-input"));
        assert!(!request_debug.contains("private-query-value"));
        assert!(!format!("{response:?}").contains("private-model-output"));
    }
}
