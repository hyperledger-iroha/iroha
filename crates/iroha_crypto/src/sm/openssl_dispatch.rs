//! OpenSSL dispatch with canonical SM4 failure semantics.
//!
//! Backend calls borrow the original inputs. Every backend error discards the
//! attempt and recomputes through Rust before returning a result. An operational
//! error on canonically valid input quarantines only that provider operation for
//! the process lifetime; invalid input and unavailable capabilities do not.

use super::{Error, OpenSslSmError, openssl_provider};

/// Independently quarantined operations of the linked OpenSSL provider.
#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum Operation {
    GcmEncrypt,
    GcmDecrypt,
    #[cfg(feature = "sm-ccm")]
    CcmEncrypt,
    #[cfg(feature = "sm-ccm")]
    CcmDecrypt,
}

impl Operation {
    pub(super) const fn mask(self) -> u8 {
        1 << self as u8
    }
}

/// Execute one provider attempt, publishing only success or the canonical result.
pub(super) fn execute<T>(
    operation: Operation,
    backend: impl FnOnce() -> Result<T, OpenSslSmError>,
    canonical: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    if !with_state(|state| state.can_attempt(operation)) {
        return canonical_result(operation, canonical);
    }
    match attempt(operation, backend) {
        Ok(result) => Ok(result),
        Err(error) => {
            let unavailable = matches!(
                error,
                OpenSslSmError::PreviewDisabled
                    | OpenSslSmError::Sm4GcmNotImplemented
                    | OpenSslSmError::Sm4CcmNotImplemented
            );
            let result = canonical_result(operation, canonical);
            // Authentication failure alone is not evidence of a faulty backend.
            // Canonical success distinguishes operational failure from bad input.
            if !unavailable && result.is_ok() {
                with_state(|state| state.quarantine(operation));
            }
            result
        }
    }
}

fn with_state<T>(action: impl FnOnce(&openssl_provider::RuntimeState) -> T) -> T {
    #[cfg(test)]
    if tests::active() {
        return tests::with_state(action);
    }
    action(openssl_provider::runtime_state())
}

fn attempt<T>(
    operation: Operation,
    backend: impl FnOnce() -> Result<T, OpenSslSmError>,
) -> Result<T, OpenSslSmError> {
    #[cfg(not(test))]
    let _ = operation;
    #[cfg(test)]
    if let Some(error) = tests::intercept(operation) {
        return Err(error);
    }
    backend()
}

fn canonical_result<T>(
    operation: Operation,
    canonical: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    #[cfg(not(test))]
    let _ = operation;
    #[cfg(test)]
    tests::record_canonical(operation);
    canonical()
}

#[cfg(test)]
mod tests;
