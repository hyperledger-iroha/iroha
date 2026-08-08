//! Iroha configuration parameters on different layers and their default values.

pub mod actual;
pub mod defaults;
pub mod user;

use url::{Host, Url};

/// Reason a runtime-provider handle cannot identify a production adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionRuntimeHandleError {
    /// The handle is empty, oversized, non-ASCII, or contains a forbidden byte.
    InvalidSyntax,
    /// One exact delimiter-separated component marks a non-production adapter.
    TestMarked,
}

/// Maximum UTF-8 byte length of a production runtime-provider handle.
pub const PRODUCTION_RUNTIME_HANDLE_MAX_BYTES: usize = 256;

/// Maximum appeal-finance submitter signers in one V1 runtime configuration.
pub const SORAFS_APPEAL_FINANCE_MAX_SUBMITTER_SIGNERS_V1: usize = 128;

/// Maximum UTF-8 byte length of a canonical V1 WebAuthn relying-party ID.
pub const WEBAUTHN_RP_ID_MAX_BYTES_V1: usize = 253;

/// Maximum UTF-8 byte length of a canonical V1 WebAuthn origin.
pub const WEBAUTHN_ORIGIN_MAX_BYTES_V1: usize = 512;

/// Reason a WebAuthn relying-party ID is not canonical under the V1 policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthnRpIdV1Error {
    /// The value is empty, oversized, non-ASCII, or not a valid DNS name.
    InvalidSyntax,
    /// The value is not its exact lowercase DNS serialization.
    NonCanonical,
    /// V1 does not admit a single-label relying-party ID such as `localhost`.
    SingleLabel,
    /// V1 requires a DNS name and does not admit an IP address literal.
    IpAddress,
}

/// Reason a WebAuthn origin is not canonical under the V1 policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthnOriginV1Error {
    /// The supplied relying-party ID is not canonical under V1.
    InvalidRpId,
    /// The origin is empty, oversized, non-ASCII, malformed, or has no DNS host.
    InvalidSyntax,
    /// V1 WebAuthn origins must use HTTPS.
    InsecureScheme,
    /// URL user information is forbidden, even when it does not contain a password.
    Credentials,
    /// Paths, queries, and fragments are not part of a V1 WebAuthn origin.
    NonOriginComponent,
    /// The value differs from the URL standard's exact origin serialization.
    NonCanonical,
    /// The origin host is neither the RP ID nor one of its subdomains.
    ForeignHost,
}

/// Validate one canonical WebAuthn relying-party ID under the public V1 policy.
///
/// V1 accepts only exact lowercase multi-label ASCII DNS names. IP address
/// literals, single-label development names, trailing dots, and DNS labels
/// with non-LDH bytes are rejected. This is a hard cut: callers must not trim,
/// lowercase, or otherwise repair an input before validation.
///
/// # Errors
///
/// Returns a [`WebAuthnRpIdV1Error`] describing the rejected shape.
pub fn validate_webauthn_rp_id_v1(value: &str) -> Result<(), WebAuthnRpIdV1Error> {
    if value.is_empty()
        || value.len() > WEBAUTHN_RP_ID_MAX_BYTES_V1
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(WebAuthnRpIdV1Error::InvalidSyntax);
    }
    if value != value.to_ascii_lowercase() || value.ends_with('.') {
        return Err(WebAuthnRpIdV1Error::NonCanonical);
    }
    if !value.contains('.') {
        return Err(WebAuthnRpIdV1Error::SingleLabel);
    }
    if !value.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && label
                .bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return Err(WebAuthnRpIdV1Error::InvalidSyntax);
    }

    let authority = format!("https://{value}");
    let parsed = Url::parse(&authority).map_err(|_| WebAuthnRpIdV1Error::InvalidSyntax)?;
    match parsed.host() {
        Some(Host::Domain(host)) if host == value => Ok(()),
        Some(Host::Domain(_)) => Err(WebAuthnRpIdV1Error::NonCanonical),
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => Err(WebAuthnRpIdV1Error::IpAddress),
        None => Err(WebAuthnRpIdV1Error::InvalidSyntax),
    }
}

/// Return whether `value` is a canonical V1 WebAuthn relying-party ID.
pub fn is_canonical_webauthn_rp_id_v1(value: &str) -> bool {
    validate_webauthn_rp_id_v1(value).is_ok()
}

/// Validate one canonical WebAuthn origin for `rp_id` under the public V1 policy.
///
/// The exact URL serialization must be HTTPS with no credentials, path,
/// query, fragment, or explicit default/non-canonical port. A canonical
/// explicit non-default port is allowed. The DNS host must equal `rp_id` or
/// be its label-bound subdomain.
///
/// # Errors
///
/// Returns a [`WebAuthnOriginV1Error`] describing the rejected shape.
pub fn validate_webauthn_origin_v1(value: &str, rp_id: &str) -> Result<(), WebAuthnOriginV1Error> {
    validate_webauthn_rp_id_v1(rp_id).map_err(|_| WebAuthnOriginV1Error::InvalidRpId)?;
    if value.is_empty()
        || value.len() > WEBAUTHN_ORIGIN_MAX_BYTES_V1
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(WebAuthnOriginV1Error::InvalidSyntax);
    }

    let parsed = Url::parse(value).map_err(|_| WebAuthnOriginV1Error::InvalidSyntax)?;
    if parsed.scheme() != "https" {
        return Err(WebAuthnOriginV1Error::InsecureScheme);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(WebAuthnOriginV1Error::Credentials);
    }
    if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(WebAuthnOriginV1Error::NonOriginComponent);
    }
    if parsed.origin().ascii_serialization() != value {
        return Err(WebAuthnOriginV1Error::NonCanonical);
    }
    let Some(Host::Domain(host)) = parsed.host() else {
        return Err(WebAuthnOriginV1Error::InvalidSyntax);
    };
    if host != rp_id
        && !host
            .strip_suffix(rp_id)
            .is_some_and(|prefix| prefix.ends_with('.'))
    {
        return Err(WebAuthnOriginV1Error::ForeignHost);
    }
    Ok(())
}

/// Return whether `value` is a canonical V1 WebAuthn origin for `rp_id`.
pub fn is_canonical_webauthn_origin_v1(value: &str, rp_id: &str) -> bool {
    validate_webauthn_origin_v1(value, rp_id).is_ok()
}

/// Validate a credential-free production runtime-provider handle.
///
/// Handles are stable public deployment identities rather than endpoint URLs or
/// credentials. V1 permits ASCII letters, digits, `.`, `_`, `:`, `/`, and `-`
/// so opaque `hsm://`, `sealed://`, and pinned-source identities remain
/// representable. URI userinfo, query, fragment, percent-encoding, whitespace,
/// controls, and components marking test adapters are rejected.
///
/// # Errors
///
/// Returns [`ProductionRuntimeHandleError::InvalidSyntax`] for malformed
/// handles and [`ProductionRuntimeHandleError::TestMarked`] when a component
/// identifies a non-production adapter.
pub fn validate_production_runtime_handle(value: &str) -> Result<(), ProductionRuntimeHandleError> {
    if value.is_empty()
        || value.len() > PRODUCTION_RUNTIME_HANDLE_MAX_BYTES
        || !value.is_ascii()
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-'))
        })
    {
        return Err(ProductionRuntimeHandleError::InvalidSyntax);
    }

    let lowercase = value.to_ascii_lowercase();
    if lowercase
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            matches!(
                component,
                "null" | "mock" | "test" | "dev" | "fake" | "dummy" | "placeholder"
            )
        })
    {
        return Err(ProductionRuntimeHandleError::TestMarked);
    }
    Ok(())
}

/// Return whether `value` is a credential-free production runtime-provider handle.
pub fn is_production_runtime_handle(value: &str) -> bool {
    validate_production_runtime_handle(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ProductionRuntimeHandleError, WebAuthnOriginV1Error, WebAuthnRpIdV1Error,
        is_canonical_webauthn_origin_v1, is_canonical_webauthn_rp_id_v1,
        is_production_runtime_handle, validate_production_runtime_handle,
        validate_webauthn_origin_v1, validate_webauthn_rp_id_v1,
    };

    #[test]
    fn production_runtime_handle_grammar_is_credential_free_and_canonical() {
        for accepted in [
            "hsm://sorafs/provider-ingest/primary",
            "sealed://sorafs/provider-ingest/checkpoint-a",
            "https-pinned-source-pool:eu-1",
        ] {
            assert!(
                is_production_runtime_handle(accepted),
                "{accepted:?} must be accepted"
            );
        }

        for rejected in [
            "",
            "https://operator:secret@host",
            "https://host/source?token=secret",
            "https://host/source#fragment",
            "https://host/%73ource",
            "hsm://sorafs/provider-ingest/dummy",
            "hsm://sorafs/provider-ingest/test",
            "provider ingest",
            "provider\\ingest",
            "provider\nprimary",
            "🗝️",
        ] {
            assert!(
                !is_production_runtime_handle(rejected),
                "{rejected:?} must be rejected"
            );
        }

        assert_eq!(
            validate_production_runtime_handle("https://operator:secret@host"),
            Err(ProductionRuntimeHandleError::InvalidSyntax)
        );
        assert_eq!(
            validate_production_runtime_handle("hsm://sorafs/provider-ingest/dummy"),
            Err(ProductionRuntimeHandleError::TestMarked)
        );
    }

    #[test]
    fn webauthn_rp_id_v1_requires_canonical_multilabel_dns() {
        for accepted in [
            "review.example",
            "review.test",
            "admin.review.example",
            "xn--bcher-kva.example",
        ] {
            assert!(
                is_canonical_webauthn_rp_id_v1(accepted),
                "{accepted:?} must be accepted"
            );
        }

        for rejected in [
            "",
            "Review.example",
            "localhost",
            "127.0.0.1",
            "127.1",
            "review.example.",
            "review..example",
            "-review.example",
            "review-.example",
            "review_example.com",
            "review.例",
        ] {
            assert!(
                !is_canonical_webauthn_rp_id_v1(rejected),
                "{rejected:?} must be rejected"
            );
        }

        assert_eq!(
            validate_webauthn_rp_id_v1("localhost"),
            Err(WebAuthnRpIdV1Error::SingleLabel)
        );
        assert_eq!(
            validate_webauthn_rp_id_v1("127.0.0.1"),
            Err(WebAuthnRpIdV1Error::IpAddress)
        );
    }

    #[test]
    fn webauthn_origin_v1_is_exact_https_and_rp_bound() {
        for (accepted, rp_id) in [
            ("https://review.example", "review.example"),
            ("https://review.test", "review.test"),
            ("https://login.review.example", "review.example"),
            ("https://login.review.example:8443", "review.example"),
        ] {
            assert!(
                is_canonical_webauthn_origin_v1(accepted, rp_id),
                "{accepted:?} must be accepted"
            );
        }

        for rejected in [
            "http://review.example",
            "https://operator@review.example",
            "https://operator:secret@review.example",
            "https://review.example/",
            "https://review.example/path",
            "https://review.example?challenge=1",
            "https://review.example#fragment",
            "https://review.example:443",
            "https://Review.example",
            "https://review.example:08443",
            "https://foreign.example",
            "https://notreview.example",
        ] {
            assert!(
                !is_canonical_webauthn_origin_v1(rejected, "review.example"),
                "{rejected:?} must be rejected"
            );
        }

        assert_eq!(
            validate_webauthn_origin_v1("http://review.example", "review.example"),
            Err(WebAuthnOriginV1Error::InsecureScheme)
        );
        assert_eq!(
            validate_webauthn_origin_v1("https://foreign.example", "review.example"),
            Err(WebAuthnOriginV1Error::ForeignHost)
        );
    }
}
