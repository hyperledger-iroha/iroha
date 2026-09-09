//! Canonical identity validation and framed encoding for Python instruction codecs.

use iroha_data_model::{account::AccountAddress, domain::DomainId, name::Name};
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
    module.add_function(wrap_pyfunction!(validate_account_address_v1, module)?)?;
    module.add_function(wrap_pyfunction!(parse_account_address_v1, module)?)?;
    module.add_function(wrap_pyfunction!(render_account_address_v1, module)?)?;
    module.add_function(wrap_pyfunction!(validate_sccp_account_id_v1, module)?)?;
    Ok(())
}

fn canonical_address(bytes: &[u8]) -> PyResult<AccountAddress> {
    if bytes.is_empty() || bytes.len() > MAX_IDENTITY_FRAME_BYTES {
        return Err(PyValueError::new_err(
            "account address must contain 1..67108864 bytes",
        ));
    }
    AccountAddress::from_canonical_bytes(bytes)
        .map_err(|error| PyValueError::new_err(format!("invalid account address: {error}")))
}

/// Admit complete canonical address bytes using the Rust key and policy owner.
#[pyo3::pyfunction]
#[pyo3(name = "_validate_account_address_v1")]
fn validate_account_address_v1(bytes: &[u8]) -> PyResult<()> {
    canonical_address(bytes).map(|_| ())
}

/// Decode one exact I105 literal into its complete canonical controller bytes.
#[pyo3::pyfunction]
#[pyo3(name = "_parse_account_address_v1", signature = (value, expected_discriminant=None))]
fn parse_account_address_v1(
    py: Python<'_>,
    value: &str,
    expected_discriminant: Option<u16>,
) -> PyResult<Py<PyBytes>> {
    super::require_non_blank_unpadded(value, "account address")?;
    if value.len() > MAX_IDENTITY_FRAME_BYTES {
        return Err(PyValueError::new_err(
            "account address literal exceeds the V1 byte limit",
        ));
    }
    let address = AccountAddress::parse_encoded(value, expected_discriminant)
        .map_err(|error| PyValueError::new_err(format!("invalid account address: {error}")))?;
    let canonical_hex = address
        .canonical_hex()
        .map_err(|error| PyValueError::new_err(format!("invalid account address: {error}")))?;
    let bytes = hex::decode(canonical_hex.strip_prefix("0x").ok_or_else(|| {
        PyRuntimeError::new_err("Rust account address omitted its canonical hex prefix")
    })?)
    .map_err(|error| PyRuntimeError::new_err(format!("Rust account address hex: {error}")))?;
    Ok(Py::from(PyBytes::new(py, &bytes)))
}

/// Render admitted canonical controller bytes using an exact chain discriminant.
#[pyo3::pyfunction]
#[pyo3(name = "_render_account_address_v1")]
fn render_account_address_v1(bytes: &[u8], discriminant: u16) -> PyResult<String> {
    canonical_address(bytes)?
        .to_i105_for_discriminant(discriminant)
        .map_err(|error| PyValueError::new_err(format!("invalid account address: {error}")))
}

/// Admit the SCCP V1 bare AccountId layout: exactly COMPACT_LEN and a u16 byte ceiling.
#[pyo3::pyfunction]
#[pyo3(name = "_validate_sccp_account_id_v1")]
fn validate_sccp_account_id_v1(payload: &[u8]) -> PyResult<()> {
    if payload.is_empty() || payload.len() > usize::from(u16::MAX) {
        return Err(PyValueError::new_err(
            "SCCP AccountId must contain 1..65535 bytes",
        ));
    }
    let frame = norito::core::frame_bare_with_header_flags::<iroha_data_model::account::AccountId>(
        payload, 0x02,
    )
    .map_err(|error| PyValueError::new_err(format!("invalid SCCP AccountId: {error}")))?;
    let account: iroha_data_model::account::AccountId = norito::decode_canonical(&frame)
        .map_err(|error| PyValueError::new_err(format!("invalid SCCP AccountId: {error}")))?;
    let encoded = norito::encode_canonical(&account)
        .map_err(|error| PyValueError::new_err(format!("invalid SCCP AccountId: {error}")))?;
    if encoded != frame {
        return Err(PyValueError::new_err(
            "SCCP AccountId must use exact canonical COMPACT_LEN bytes",
        ));
    }
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
        #[derive(norito::NoritoSchema)]
        #[norito_schema(
            name = "iroha_python_rs::identity_codec_v1::tests::oversized_identity_frame_is_rejected_before_output_allocation::OversizedPayload"
        )]
        struct OversizedPayload {
            serialization_passes: Cell<usize>,
        }

        impl norito::SerializePayload for OversizedPayload {
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
            assert!(module.hasattr("_validate_account_address_v1").unwrap());
            assert!(module.hasattr("_parse_account_address_v1").unwrap());
            assert!(module.hasattr("_render_account_address_v1").unwrap());
            assert!(module.hasattr("_validate_sccp_account_id_v1").unwrap());

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
            let raw = parse_account_address_v1(py, &literal, None).unwrap();
            validate_account_address_v1(raw.as_bytes(py)).unwrap();
            let rendered = render_account_address_v1(raw.as_bytes(py), 753).unwrap();
            assert_eq!(
                parse_account_address_v1(py, &rendered, Some(753))
                    .unwrap()
                    .as_bytes(py),
                raw.as_bytes(py)
            );
            assert!(parse_account_address_v1(py, &format!(" {rendered}"), Some(753)).is_err());
            assert!(parse_account_address_v1(py, &rendered, Some(1)).is_err());
            assert!(super::super::PyAccountId::new(&rendered).is_ok());
            assert!(super::super::PyAccountId::new(&format!("{rendered} ")).is_err());
            let _layout = norito::core::DecodeFlagsGuard::enter(0x02);
            let (bare, flags) = norito::codec::encode_with_header_flags(&account);
            assert_eq!(flags, 0x02);
            validate_sccp_account_id_v1(&bare).unwrap();
            assert!(validate_sccp_account_id_v1(b"x").is_err());
            let mut trailing = bare;
            trailing.push(0);
            assert!(validate_sccp_account_id_v1(&trailing).is_err());
            let identity = [vec![2, 0, 1, 32, 1], vec![0; 31]].concat();
            assert!(validate_account_address_v1(&identity).is_err());
            assert!(render_account_address_v1(&identity, 753).is_err());

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
