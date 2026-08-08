//! Source-bound Python view of the canonical Exact12 capability manifest.
//!
//! This module deliberately has no constructor from a local compiled-profile
//! catalog.  The only accepted input is the exact canonical Norito archive
//! returned by Torii (or a separately authenticated candidate receipt).  The
//! archive self-digest detects drift; the caller remains responsible for
//! authenticating the transport or receipt that supplied the bytes.

use iroha_core::privacy_profiles::{CompiledPrivacyProfileV1, compiled_privacy_profile_v1};
use iroha_data_model::privacy::{
    PrivacyCapabilityActivationStateV1, PrivacyCapabilityLimitationV1,
    PrivacyCapabilityReadinessV1, PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
    PrivacyCompiledProfileUnavailableReasonV1, PrivacyEngineIdV1,
    PrivacyExact12CapabilityManifestV1, PrivacyExact12CapabilityRowV1, PrivacyProofSystemIdV1,
    PrivacyProtocolIdV1, validate_privacy_capability_archive_v1,
};
use pyo3::{
    exceptions::{PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyDict, PyList},
};

/// Validated canonical manifest bytes retained for transaction construction.
///
/// The Python class intentionally exposes no public constructor.  Instances
/// are created only by [`privacy_exact12_capability_manifest_v1_py`], which
/// applies the native bounded canonical decoder and all semantic invariants.
#[pyclass(
    name = "PrivacyExact12CapabilityManifestV1",
    frozen,
    module = "iroha_python._crypto"
)]
#[derive(Clone)]
pub(crate) struct PyPrivacyExact12CapabilityManifestV1 {
    manifest: PrivacyExact12CapabilityManifestV1,
    canonical_archive: Vec<u8>,
}

impl PyPrivacyExact12CapabilityManifestV1 {
    pub(crate) fn decode(archive: &[u8]) -> PyResult<Self> {
        let status = validate_privacy_capability_archive_v1(archive);
        if !status.is_valid() {
            return Err(PyValueError::new_err(format!(
                "invalid canonical Exact12 capability manifest archive (native status {})",
                status.code()
            )));
        }
        let manifest: PrivacyExact12CapabilityManifestV1 = norito::decode_from_bytes(archive)
            .map_err(|_| {
                PyRuntimeError::new_err(
                    "native Exact12 capability manifest validation/decode disagreement",
                )
            })?;
        let canonical_archive = manifest.canonical_bytes().map_err(|_| {
            PyRuntimeError::new_err(
                "native Exact12 capability manifest validation/re-encoding disagreement",
            )
        })?;
        if canonical_archive.as_slice() != archive {
            return Err(PyRuntimeError::new_err(
                "native Exact12 capability manifest canonical bytes changed after validation",
            ));
        }
        Ok(Self {
            manifest,
            canonical_archive,
        })
    }

    pub(crate) fn require_network_profile(
        &self,
        protocol_id: PrivacyProtocolIdV1,
    ) -> PyResult<CompiledPrivacyProfileV1> {
        let row = self
            .manifest
            .protocols
            .iter()
            .find(|row| row.protocol_id == protocol_id)
            .ok_or_else(|| {
                PyRuntimeError::new_err(format!(
                    "validated Exact12 manifest omitted protocol {}",
                    protocol_id.canonical_label()
                ))
            })?;
        if !row.is_network_available() {
            return Err(PyRuntimeError::new_err(format!(
                "privacy protocol {} is not available in the committed Exact12 manifest",
                protocol_id.canonical_label()
            )));
        }
        let PrivacyCompiledProfileResultV1::Available(network_profile) = row.compiled_profile
        else {
            return Err(PyRuntimeError::new_err(format!(
                "privacy protocol {} has no available committed compiled profile",
                protocol_id.canonical_label()
            )));
        };
        let local_profile = compiled_privacy_profile_v1(protocol_id).map_err(|error| {
            PyRuntimeError::new_err(format!(
                "local native profile for {} is unavailable: {error}",
                protocol_id.canonical_label()
            ))
        })?;
        let local_snapshot = PrivacyCompiledProfileSnapshotV1::from(local_profile);
        if network_profile != local_snapshot {
            return Err(PyRuntimeError::new_err(format!(
                "local native profile for {} does not match the committed Exact12 capability tuple",
                protocol_id.canonical_label()
            )));
        }
        Ok(local_profile)
    }

    #[cfg(test)]
    pub(crate) fn test_binding_for_protocol(protocol_id: PrivacyProtocolIdV1) -> Self {
        use iroha_data_model::privacy::{
            PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1, PrivacyActiveLifecycleV1,
            PrivacyCapabilityRowV1, PrivacyCapabilitySnapshotV1, PrivacyConsensusPolicyV1,
            PrivacyProtocolLifecycleV1,
        };

        let catalog = iroha_core::privacy_profiles::compiled_privacy_profile_catalog_v1()
            .expect("test compiled-profile catalog");
        let protocols = catalog
            .protocols
            .into_iter()
            .map(|row| {
                let activation = (row.protocol_id == protocol_id).then(|| {
                    compiled_privacy_profile_v1(protocol_id)
                        .expect("selected test profile is compiled")
                        .activation_record(PrivacyProtocolLifecycleV1::Active(
                            PrivacyActiveLifecycleV1 {
                                proposed_at_height: 1,
                                activated_at_height: 2,
                                state_since_height: 2,
                            },
                        ))
                });
                PrivacyCapabilityRowV1 {
                    protocol_id: row.protocol_id,
                    compiled_profile: row.compiled_profile,
                    activation,
                }
            })
            .collect();
        let manifest = PrivacyCapabilitySnapshotV1 {
            version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
            committed_height: 3,
            consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
            protocols,
        }
        .exact12_capability_manifest_v1()
        .expect("test Exact12 manifest");
        let archive = manifest.canonical_bytes().expect("test manifest archive");
        Self::decode(&archive).expect("test manifest binding")
    }
}

#[pymethods]
impl PyPrivacyExact12CapabilityManifestV1 {
    #[getter]
    fn canonical_archive<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.canonical_archive)
    }

    #[getter]
    fn manifest_digest<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, self.manifest.manifest_digest.as_bytes())
    }

    #[getter]
    const fn version(&self) -> u32 {
        self.manifest.version
    }

    #[getter]
    const fn committed_height(&self) -> u64 {
        self.manifest.committed_height
    }

    /// Return the exact twelve validated public capability tuples.
    fn protocol_tuples(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let rows = PyList::empty(py);
        for row in &self.manifest.protocols {
            rows.append(capability_tuple_dict(py, &self.manifest, row)?)?;
        }
        Ok(rows.unbind())
    }

    /// Require one active committed row and exact equality with this binary.
    fn require_network_capability(
        &self,
        py: Python<'_>,
        protocol_id: &str,
    ) -> PyResult<Py<PyDict>> {
        let protocol_id = parse_protocol_id(protocol_id)?;
        self.require_network_profile(protocol_id)?;
        let row = self
            .manifest
            .protocols
            .iter()
            .find(|row| row.protocol_id == protocol_id)
            .expect("validated Exact12 manifest contains every canonical row");
        capability_tuple_dict(py, &self.manifest, row)
    }
}

fn parse_protocol_id(label: &str) -> PyResult<PrivacyProtocolIdV1> {
    PrivacyProtocolIdV1::from_canonical_label(label).ok_or_else(|| {
        PyValueError::new_err(
            "protocol_id must be one exact active Exact12 identifier; aliases and retired identifiers are rejected",
        )
    })
}

fn readiness_label(value: PrivacyCapabilityReadinessV1) -> &'static str {
    match value {
        PrivacyCapabilityReadinessV1::Available => "available",
        PrivacyCapabilityReadinessV1::AvailableExperimental => "available-experimental",
        PrivacyCapabilityReadinessV1::Unavailable(_) => "unavailable",
    }
}

fn unavailable_reason_label(value: PrivacyCompiledProfileUnavailableReasonV1) -> &'static str {
    match value {
        PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable => "engine-unavailable",
        PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed => {
            "profile-initialization-failed"
        }
        PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(_) => {
            "statement-schema-invalid"
        }
    }
}

fn activation_state_label(value: PrivacyCapabilityActivationStateV1) -> &'static str {
    match value {
        PrivacyCapabilityActivationStateV1::NotRegistered => "not-registered",
        PrivacyCapabilityActivationStateV1::Proposed => "proposed",
        PrivacyCapabilityActivationStateV1::Active => "active",
        PrivacyCapabilityActivationStateV1::Suspended => "suspended",
        PrivacyCapabilityActivationStateV1::Retired => "retired",
    }
}

fn limitation_label(value: PrivacyCapabilityLimitationV1) -> &'static str {
    match value {
        PrivacyCapabilityLimitationV1::MissingDistributionWideKnowledgeSoundnessEvidence => {
            "missing-distribution-wide-knowledge-soundness-evidence"
        }
    }
}

fn proof_system_label(value: PrivacyProofSystemIdV1) -> &'static str {
    match value {
        PrivacyProofSystemIdV1::StarkFriSha256Goldilocks => "stark-fri-sha256-goldilocks",
        PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512 => {
            "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512"
        }
        PrivacyProofSystemIdV1::AnonymousPgcP256 => "anonymous-pgc-p256",
        PrivacyProofSystemIdV1::IrohaVeRangeP256 => "iroha-verange-p256",
        PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256 => {
            "vega-neutron-nova-spartan-hyrax-t256"
        }
        PrivacyProofSystemIdV1::JindoPolynomialCommitment => "jindo-polynomial-commitment",
        PrivacyProofSystemIdV1::Halo2IpaPasta => "halo2-ipa-pasta",
        PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs => {
            "fcmp-plus-plus-curve-tree-bulletproofs"
        }
        PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm => "lantern-lnp22-module-linear-norm",
    }
}

fn engine_label(value: PrivacyEngineIdV1) -> &'static str {
    match value {
        PrivacyEngineIdV1::NativeGoldilocksStarkFri => "native-goldilocks-stark-fri",
        PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255 => {
            "native-zk-ams-masked-relaxed-spartan-t256-ristretto255"
        }
        PrivacyEngineIdV1::NativeAnonymousPgcP256 => "native-anonymous-pgc-p256",
        PrivacyEngineIdV1::NativeVeRangeP256 => "native-verange-p256",
        PrivacyEngineIdV1::NativeVega => "native-vega",
        PrivacyEngineIdV1::NativeJindo => "native-jindo",
        PrivacyEngineIdV1::NativeHalo2Orchard => "native-halo2-orchard",
        PrivacyEngineIdV1::NativeFcmpPlusPlus => "native-fcmp-plus-plus",
        PrivacyEngineIdV1::NativeLanternLnp22 => "native-lantern-lnp22",
    }
}

fn capability_tuple_dict(
    py: Python<'_>,
    manifest: &PrivacyExact12CapabilityManifestV1,
    row: &PrivacyExact12CapabilityRowV1,
) -> PyResult<Py<PyDict>> {
    let output = PyDict::new(py);
    output.set_item(
        "manifest_digest",
        PyBytes::new(py, manifest.manifest_digest.as_bytes()),
    )?;
    output.set_item("committed_height", manifest.committed_height)?;
    output.set_item("protocol_id", row.protocol_id.canonical_label())?;
    output.set_item("operation_schema", row.operation_schema.canonical_label())?;
    output.set_item("execution_mode", row.execution_mode.canonical_label())?;
    output.set_item("privacy_feature_mask", row.privacy_feature_mask.bits())?;
    output.set_item("readiness", readiness_label(row.readiness))?;
    let unavailable_reason = match row.readiness {
        PrivacyCapabilityReadinessV1::Unavailable(reason) => Some(unavailable_reason_label(reason)),
        PrivacyCapabilityReadinessV1::Available
        | PrivacyCapabilityReadinessV1::AvailableExperimental => None,
    };
    output.set_item("unavailable_reason", unavailable_reason)?;
    output.set_item(
        "activation_state",
        activation_state_label(row.activation_state),
    )?;
    output.set_item("network_available", row.is_network_available())?;
    output.set_item("limitation", row.limitation.map(limitation_label))?;
    match row.compiled_profile {
        PrivacyCompiledProfileResultV1::Available(profile) => {
            output.set_item("compiled_profile_status", "available")?;
            output.set_item(
                "proof_system_id",
                proof_system_label(profile.proof_system_id),
            )?;
            output.set_item("engine_id", engine_label(profile.engine_id))?;
            output.set_item(
                "parameter_id",
                PyBytes::new(py, profile.parameter_id.as_bytes()),
            )?;
            output.set_item(
                "parameter_digest",
                PyBytes::new(py, profile.parameter_digest.as_bytes()),
            )?;
            output.set_item(
                "verifier_digest",
                PyBytes::new(py, profile.verifier_digest.as_bytes()),
            )?;
            output.set_item(
                "statement_schema_digest",
                PyBytes::new(py, profile.statement_schema_digest.as_bytes()),
            )?;
            output.set_item(
                "engine_manifest_digest",
                PyBytes::new(py, profile.engine_manifest_digest.as_bytes()),
            )?;
        }
        PrivacyCompiledProfileResultV1::Unavailable(reason) => {
            output.set_item("compiled_profile_status", "unavailable")?;
            output.set_item("unavailable_reason", unavailable_reason_label(reason))?;
            for key in [
                "proof_system_id",
                "engine_id",
                "parameter_id",
                "parameter_digest",
                "verifier_digest",
                "statement_schema_digest",
                "engine_manifest_digest",
            ] {
                output.set_item(key, py.None())?;
            }
        }
    }
    Ok(output.unbind())
}

#[pyfunction]
#[pyo3(name = "privacy_validate_exact12_capability_manifest_v1")]
pub(crate) fn privacy_validate_exact12_capability_manifest_v1_py(archive: &[u8]) -> i32 {
    validate_privacy_capability_archive_v1(archive).code()
}

#[pyfunction]
#[pyo3(name = "privacy_exact12_capability_manifest_v1")]
pub(crate) fn privacy_exact12_capability_manifest_v1_py(
    py: Python<'_>,
    archive: &[u8],
) -> PyResult<Py<PyPrivacyExact12CapabilityManifestV1>> {
    Py::new(py, PyPrivacyExact12CapabilityManifestV1::decode(archive)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::privacy::PrivacyCapabilityArchiveValidationStatusV1;

    #[test]
    fn validation_status_codes_remain_the_data_model_contract() {
        assert_eq!(
            privacy_validate_exact12_capability_manifest_v1_py(&[]),
            PrivacyCapabilityArchiveValidationStatusV1::Empty.code()
        );
        assert_ne!(
            privacy_validate_exact12_capability_manifest_v1_py(b"local-catalog-shell"),
            PrivacyCapabilityArchiveValidationStatusV1::Valid.code()
        );
    }

    #[test]
    fn retired_and_alias_protocol_labels_are_not_selectable() {
        for label in [
            "jindo-lattice-pcs-zk-v0",
            "sis-with-hints",
            "zk-ams-recursive-admission-v0",
            "IROHA-ZK-AMS-V1",
            " iroha-zk-ams-v1",
        ] {
            assert!(parse_protocol_id(label).is_err(), "accepted {label}");
        }
    }

    #[test]
    fn validated_binding_preserves_bytes_and_does_not_authorize_another_row() {
        let binding = PyPrivacyExact12CapabilityManifestV1::test_binding_for_protocol(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        );
        assert_eq!(
            binding.canonical_archive,
            binding
                .manifest
                .canonical_bytes()
                .expect("validated manifest re-encodes")
        );
        assert!(
            binding
                .require_network_profile(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                .is_ok()
        );
        assert!(
            binding
                .require_network_profile(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
                .is_err()
        );
    }
}
