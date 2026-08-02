//! Iroha configuration parameters on different layers and their default values.

pub mod actual;
pub mod defaults;
pub mod user;

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
        ProductionRuntimeHandleError, is_production_runtime_handle,
        validate_production_runtime_handle,
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
}
