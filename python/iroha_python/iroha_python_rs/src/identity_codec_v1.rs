//! Canonical identity validation and framed encoding for Python instruction codecs.

use iroha_data_model::{domain::DomainId, name::Name};
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::{PyRuntimeError, PyValueError},
    types::{PyBytes, PyModule, PyModuleMethods},
    wrap_pyfunction,
};

// Keep the native producer ceiling identical to kaigi.py's complete-frame limit.
const MAX_IDENTITY_FRAME_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(encode_account_id_v1, module)?)?;
    module.add_function(wrap_pyfunction!(encode_domain_id_v1, module)?)?;
    module.add_function(wrap_pyfunction!(encode_name_v1, module)?)?;
    Ok(())
}

fn framed<T: norito::NoritoSerialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyBytes>> {
    let frame_len = norito::canonical_frame_len(value).map_err(|error| {
        PyRuntimeError::new_err(format!("canonical identity frame sizing: {error}"))
    })?;
    if frame_len > MAX_IDENTITY_FRAME_BYTES {
        return Err(PyValueError::new_err(format!(
            "canonical identity frame exceeds the {MAX_IDENTITY_FRAME_BYTES}-byte V1 limit"
        )));
    }
    let bytes = norito::encode_canonical(value).map_err(|error| {
        PyRuntimeError::new_err(format!("canonical identity encoding: {error}"))
    })?;
    Ok(Py::from(PyBytes::new(py, &bytes)))
}

/// Validate an exact I105 controller and return its complete domainless account frame.
#[pyo3::pyfunction]
#[pyo3(name = "_encode_account_id_v1")]
fn encode_account_id_v1(py: Python<'_>, value: &str) -> PyResult<(String, Py<PyBytes>)> {
    let account = super::parse_exact_i105_account_id(value, "account_id")?;
    // Preserve the caller's validated display prefix; identity comparisons use the frame.
    Ok((value.to_owned(), framed(py, &account)?))
}

/// Apply the consensus-pinned domain profile and return its canonical spelling and frame.
#[pyo3::pyfunction]
#[pyo3(name = "_encode_domain_id_v1")]
fn encode_domain_id_v1(py: Python<'_>, value: &str) -> PyResult<(String, Py<PyBytes>)> {
    super::require_non_blank_unpadded(value, "domain_id")?;
    let domain = DomainId::parse_fully_qualified(value)
        .map_err(|error| PyValueError::new_err(format!("invalid domain_id: {error}")))?;
    Ok((domain.to_string(), framed(py, &domain)?))
}

/// Validate a Name using the consensus-pinned NFC profile without normalizing its spelling.
#[pyo3::pyfunction]
#[pyo3(name = "_encode_name_v1")]
fn encode_name_v1(py: Python<'_>, value: &str) -> PyResult<(String, Py<PyBytes>)> {
    let name: Name = value
        .parse()
        .map_err(|error| PyValueError::new_err(format!("invalid Name: {error}")))?;
    Ok((name.to_string(), framed(py, &name)?))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::account::{AccountId, MultisigMember, MultisigPolicy};
    use pyo3::types::PyAnyMethods;

    #[test]
    fn oversized_identity_frame_is_rejected_before_output_allocation() {
        struct OversizedPayload {
            serialization_passes: Cell<usize>,
        }

        impl norito::NoritoSerialize for OversizedPayload {
            fn serialize(
                &self,
                encoder: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::Error> {
                self.serialization_passes
                    .set(self.serialization_passes.get() + 1);
                // The header takes the complete frame over the limit. Reuse a
                // small stack chunk so the counting pass allocates no payload.
                let chunk = [0_u8; 1024];
                for _ in 0..MAX_IDENTITY_FRAME_BYTES / chunk.len() {
                    encoder.write_all(&chunk)?;
                }
                Ok(())
            }

            fn encoded_len_exact(&self) -> Option<usize> {
                // Admission must count emitted bytes rather than trust a hint.
                Some(1)
            }
        }

        Python::initialize();
        Python::attach(|py| {
            let value = OversizedPayload {
                serialization_passes: Cell::new(0),
            };
            let error = framed(py, &value).unwrap_err();
            assert!(error.is_instance_of::<PyValueError>(py));
            assert!(error.to_string().contains("67108864-byte V1 limit"));
            assert_eq!(value.serialization_passes.get(), 1);
        });
    }

    #[test]
    fn identity_functions_preserve_full_controllers_and_pinned_unicode() {
        Python::initialize();
        Python::attach(|py| {
            let module = PyModule::new(py, "identity_test").unwrap();
            register(&module).unwrap();
            assert!(module.hasattr("_encode_account_id_v1").unwrap());
            assert!(module.hasattr("_encode_domain_id_v1").unwrap());
            assert!(module.hasattr("_encode_name_v1").unwrap());

            let members = (1..=2)
                .map(|seed| {
                    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap();
                    MultisigMember::new(key.public_key().clone(), u16::from(seed)).unwrap()
                })
                .collect();
            let account = AccountId::new_multisig(MultisigPolicy::new(2, members).unwrap());
            let literal = account.canonical_i105().unwrap();
            let (canonical, frame) = encode_account_id_v1(py, &literal).unwrap();
            assert_eq!(canonical, literal);
            assert_eq!(
                frame.as_bytes(py),
                norito::encode_canonical(&account).unwrap()
            );
            assert!(encode_account_id_v1(py, &format!(" {literal}")).is_err());
            assert!(encode_account_id_v1(py, "merchant@banka.paynet").is_err());

            let (canonical, frame) = encode_domain_id_v1(py, "例え.SORA").unwrap();
            assert_eq!(canonical, "xn--r8jz45g.sora");
            let domain = DomainId::parse_fully_qualified(&canonical).unwrap();
            assert_eq!(
                frame.as_bytes(py),
                norito::encode_canonical(&domain).unwrap()
            );
            assert!(encode_domain_id_v1(py, "sora").is_err());
            assert!(encode_domain_id_v1(py, " sora.paynet").is_err());

            let (canonical, frame) = encode_name_v1(py, "éclair").unwrap();
            assert_eq!(canonical, "éclair");
            let name: Name = canonical.parse().unwrap();
            assert_eq!(frame.as_bytes(py), norito::encode_canonical(&name).unwrap());
            assert!(encode_name_v1(py, "e\u{301}clair").is_err());
            assert!(encode_name_v1(py, "hidden\u{202e}name").is_err());
            assert!(encode_name_v1(py, "name with space").is_err());
        });
    }
}
