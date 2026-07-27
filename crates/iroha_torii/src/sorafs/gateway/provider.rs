//! Payload-free runtime-provider bindings shared by gateway security adapters.

use thiserror::Error;

const GATEWAY_PROVIDER_HANDLE_MAX_BYTES_V1: usize = 256;

/// Exact public identity configured for one runtime-owned gateway provider.
///
/// This binding intentionally contains no credentials, private keys, tokens,
/// endpoint authorization, or provider diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayProviderBindingV1 {
    provider_handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}

impl GatewayProviderBindingV1 {
    /// Construct one exact independently governed provider binding.
    ///
    /// # Errors
    ///
    /// Returns a payload-free error when the handle is malformed or
    /// test-marked, the revision is zero, or the policy digest is all zero.
    pub fn try_new(
        provider_handle: String,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Result<Self, GatewayProviderBindingErrorV1> {
        validate_gateway_provider_handle(&provider_handle)?;
        if revision == 0 {
            return Err(GatewayProviderBindingErrorV1::ZeroRevision);
        }
        if policy_digest == [0; 32] {
            return Err(GatewayProviderBindingErrorV1::ZeroPolicyDigest);
        }
        Ok(Self {
            provider_handle,
            revision,
            policy_digest,
        })
    }

    /// Return the stable non-secret provider handle.
    #[must_use]
    pub fn provider_handle(&self) -> &str {
        &self.provider_handle
    }

    /// Return the exact non-zero deployment revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return the exact non-zero public-policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> [u8; 32] {
        self.policy_digest
    }
}

/// Payload-free validation failures for gateway runtime-provider bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GatewayProviderBindingErrorV1 {
    /// The provider handle is empty, oversized, non-ASCII, or contains spacing
    /// or control bytes.
    #[error("gateway runtime provider handle is invalid")]
    InvalidHandle,
    /// The provider handle is explicitly marked for test or development use.
    #[error("gateway runtime provider handle is test-marked")]
    TestMarkedHandle,
    /// The deployment/provider revision is zero.
    #[error("gateway runtime provider revision is zero")]
    ZeroRevision,
    /// The public-policy digest is all zero.
    #[error("gateway runtime provider policy digest is zero")]
    ZeroPolicyDigest,
}

fn validate_gateway_provider_handle(handle: &str) -> Result<(), GatewayProviderBindingErrorV1> {
    if handle.is_empty()
        || handle.len() > GATEWAY_PROVIDER_HANDLE_MAX_BYTES_V1
        || !handle.is_ascii()
        || handle
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(GatewayProviderBindingErrorV1::InvalidHandle);
    }
    let lowercase = handle.to_ascii_lowercase();
    if lowercase
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            matches!(
                component,
                "null" | "mock" | "test" | "dev" | "fake" | "dummy" | "placeholder"
            )
        })
    {
        return Err(GatewayProviderBindingErrorV1::TestMarkedHandle);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_binding_rejects_test_markers_and_zero_qualification() {
        assert_eq!(
            GatewayProviderBindingV1::try_new("kms://gateway/acme/dummy".to_owned(), 1, [0x51; 32],),
            Err(GatewayProviderBindingErrorV1::TestMarkedHandle)
        );
        assert_eq!(
            GatewayProviderBindingV1::try_new(
                "kms://gateway/acme/primary".to_owned(),
                0,
                [0x51; 32],
            ),
            Err(GatewayProviderBindingErrorV1::ZeroRevision)
        );
        assert_eq!(
            GatewayProviderBindingV1::try_new("kms://gateway/acme/primary".to_owned(), 1, [0; 32],),
            Err(GatewayProviderBindingErrorV1::ZeroPolicyDigest)
        );
    }
}
