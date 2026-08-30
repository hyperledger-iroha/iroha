//! Signed-query construction and finalized Exact12 state inspection for Python.
//!
//! The managed layer supplies only a stable query ID and the exact fixed-width
//! request binding. Native code constructs the corresponding typed singular
//! query, signs it through the shared callback path, and accepts only the
//! matching validated response on the requested network.

use iroha_data_model::{
    privacy::{
        PrivacyIssuerIdV1, PrivacyNullifierV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
        PrivacyProtocolIdV1, PrivacyZkAmsKeyImageV1, PrivacyZkAmsPhcHashV1,
        PrivacyZkAmsRegistryIdV1,
    },
    query::{
        QueryRequest, QueryResponse, SingularQueryBox, SingularQueryOutputBox,
        privacy::prelude::{
            FindPrivacyAnonymousPgcPoolStateV1, FindPrivacyOrchardNullifierV1,
            FindPrivacyOrchardPoolStateV1, FindPrivacyProofManagedPoolStateV1,
            FindPrivacyZkAceReplayNullifierV1, FindPrivacyZkAmsAdmissionV1,
            FindPrivacyZkAmsProvisionV1, FindPrivacyZkX509CertificateNullifierV1,
        },
    },
};
use pyo3::{
    Bound, Py, PyResult, Python,
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    pyfunction,
    types::{PyAny, PyAnyMethods, PyBytes},
};

use super::{PyNetworkId, sign_query_request_with_signer};

const PRIVACY_FINALIZED_STATE_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrivacyFinalizedStateBindingV1 {
    ZkAceReplay {
        policy_id: PrivacyPolicyIdV1,
        replay_nullifier: PrivacyNullifierV1,
    },
    ProofManagedPool {
        protocol_id: PrivacyProtocolIdV1,
        pool_id: PrivacyPoolIdV1,
    },
    OrchardPool {
        pool_id: PrivacyPoolIdV1,
    },
    OrchardNullifier {
        pool_id: PrivacyPoolIdV1,
        nullifier: [u8; 32],
    },
    AnonymousPgcPool {
        pool_id: PrivacyPoolIdV1,
    },
    ZkAmsAdmission {
        issuer_id: PrivacyIssuerIdV1,
        registry_id: PrivacyZkAmsRegistryIdV1,
        policy_id: PrivacyPolicyIdV1,
        phc_hash: PrivacyZkAmsPhcHashV1,
    },
    ZkAmsProvision {
        issuer_id: PrivacyIssuerIdV1,
        registry_id: PrivacyZkAmsRegistryIdV1,
        policy_id: PrivacyPolicyIdV1,
        key_image: PrivacyZkAmsKeyImageV1,
    },
    ZkX509Nullifier {
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        nullifier: PrivacyNullifierV1,
    },
}

fn exact_nonzero_chunk(binding: &[u8], offset: usize, label: &str) -> PyResult<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| PyValueError::new_err(format!("{label} offset exceeds usize")))?;
    let chunk: [u8; 32] = binding
        .get(offset..end)
        .ok_or_else(|| {
            PyValueError::new_err(format!(
                "{label} is missing from the fixed privacy state-query binding"
            ))
        })?
        .try_into()
        .map_err(|_| PyValueError::new_err(format!("{label} must contain exactly 32 bytes")))?;
    if chunk.iter().all(|byte| *byte == 0) {
        return Err(PyValueError::new_err(format!(
            "{label} must not be all zero"
        )));
    }
    Ok(chunk)
}

fn require_binding_len(binding: &[u8], expected: usize) -> PyResult<()> {
    if binding.len() != expected {
        return Err(PyValueError::new_err(format!(
            "privacy state-query binding must contain exactly {expected} bytes"
        )));
    }
    Ok(())
}

fn proof_managed_protocol(index: u32) -> PyResult<PrivacyProtocolIdV1> {
    match index {
        0 => Ok(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1),
        1 => Ok(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1),
        2 => Ok(PrivacyProtocolIdV1::PqMaspStarkV0),
        _ => Err(PyValueError::new_err(
            "proof-managed state query protocol is outside its closed union",
        )),
    }
}

fn parse_binding(
    query_id: u32,
    protocol_index: u32,
    binding: &[u8],
) -> PyResult<PrivacyFinalizedStateBindingV1> {
    if query_id != 98 && protocol_index != 0 {
        return Err(PyValueError::new_err(
            "proof-managed protocol index must be zero outside privacy query ID 98",
        ));
    }
    match query_id {
        97 => {
            require_binding_len(binding, 64)?;
            Ok(PrivacyFinalizedStateBindingV1::ZkAceReplay {
                policy_id: PrivacyPolicyIdV1::new(exact_nonzero_chunk(binding, 0, "policy id")?),
                replay_nullifier: PrivacyNullifierV1::new(exact_nonzero_chunk(
                    binding,
                    32,
                    "replay nullifier",
                )?),
            })
        }
        98 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyFinalizedStateBindingV1::ProofManagedPool {
                protocol_id: proof_managed_protocol(protocol_index)?,
                pool_id: PrivacyPoolIdV1::new(exact_nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        99 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyFinalizedStateBindingV1::OrchardPool {
                pool_id: PrivacyPoolIdV1::new(exact_nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        100 => {
            require_binding_len(binding, 64)?;
            Ok(PrivacyFinalizedStateBindingV1::OrchardNullifier {
                pool_id: PrivacyPoolIdV1::new(exact_nonzero_chunk(binding, 0, "pool id")?),
                nullifier: exact_nonzero_chunk(binding, 32, "Orchard nullifier")?,
            })
        }
        101 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyFinalizedStateBindingV1::AnonymousPgcPool {
                pool_id: PrivacyPoolIdV1::new(exact_nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        102 => {
            require_binding_len(binding, 128)?;
            Ok(PrivacyFinalizedStateBindingV1::ZkAmsAdmission {
                issuer_id: PrivacyIssuerIdV1::new(exact_nonzero_chunk(binding, 0, "issuer id")?),
                registry_id: PrivacyZkAmsRegistryIdV1::new(exact_nonzero_chunk(
                    binding,
                    32,
                    "registry id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(exact_nonzero_chunk(binding, 64, "policy id")?),
                phc_hash: PrivacyZkAmsPhcHashV1::new(exact_nonzero_chunk(binding, 96, "PHC hash")?),
            })
        }
        103 => {
            require_binding_len(binding, 128)?;
            Ok(PrivacyFinalizedStateBindingV1::ZkAmsProvision {
                issuer_id: PrivacyIssuerIdV1::new(exact_nonzero_chunk(binding, 0, "issuer id")?),
                registry_id: PrivacyZkAmsRegistryIdV1::new(exact_nonzero_chunk(
                    binding,
                    32,
                    "registry id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(exact_nonzero_chunk(binding, 64, "policy id")?),
                key_image: PrivacyZkAmsKeyImageV1::new(exact_nonzero_chunk(
                    binding,
                    96,
                    "key image",
                )?),
            })
        }
        104 => {
            require_binding_len(binding, 96)?;
            Ok(PrivacyFinalizedStateBindingV1::ZkX509Nullifier {
                trust_anchor_id: PrivacyIssuerIdV1::new(exact_nonzero_chunk(
                    binding,
                    0,
                    "trust-anchor id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(exact_nonzero_chunk(binding, 32, "policy id")?),
                nullifier: PrivacyNullifierV1::new(exact_nonzero_chunk(
                    binding,
                    64,
                    "certificate nullifier",
                )?),
            })
        }
        _ => Err(PyValueError::new_err(
            "privacy state-query stable ID is outside 97 through 104",
        )),
    }
}

impl PrivacyFinalizedStateBindingV1 {
    fn query(self) -> QueryRequest {
        let query: SingularQueryBox = match self {
            Self::ZkAceReplay {
                policy_id,
                replay_nullifier,
            } => FindPrivacyZkAceReplayNullifierV1::new(policy_id, replay_nullifier).into(),
            Self::ProofManagedPool {
                protocol_id,
                pool_id,
            } => FindPrivacyProofManagedPoolStateV1::new(protocol_id, pool_id).into(),
            Self::OrchardPool { pool_id } => FindPrivacyOrchardPoolStateV1::new(pool_id).into(),
            Self::OrchardNullifier { pool_id, nullifier } => {
                FindPrivacyOrchardNullifierV1::new(pool_id, nullifier).into()
            }
            Self::AnonymousPgcPool { pool_id } => {
                FindPrivacyAnonymousPgcPoolStateV1::new(pool_id).into()
            }
            Self::ZkAmsAdmission {
                issuer_id,
                registry_id,
                policy_id,
                phc_hash,
            } => {
                FindPrivacyZkAmsAdmissionV1::new(issuer_id, registry_id, policy_id, phc_hash).into()
            }
            Self::ZkAmsProvision {
                issuer_id,
                registry_id,
                policy_id,
                key_image,
            } => FindPrivacyZkAmsProvisionV1::new(issuer_id, registry_id, policy_id, key_image)
                .into(),
            Self::ZkX509Nullifier {
                trust_anchor_id,
                policy_id,
                nullifier,
            } => {
                FindPrivacyZkX509CertificateNullifierV1::new(trust_anchor_id, policy_id, nullifier)
                    .into()
            }
        };
        QueryRequest::Singular(query)
    }
}

/// Build one authority-signed typed finalized privacy-state query.
#[pyfunction]
#[pyo3(name = "build_privacy_finalized_state_query_with_signer")]
pub(crate) fn build_query_with_signer(
    py: Python<'_>,
    authority: &str,
    signer: &Bound<'_, PyAny>,
    network_id: &PyNetworkId,
    query_id: u32,
    protocol_index: u32,
    request_binding: &[u8],
) -> PyResult<Py<PyBytes>> {
    if !signer.is_callable() {
        return Err(PyTypeError::new_err("query signer must be callable"));
    }
    let binding = parse_binding(query_id, protocol_index, request_binding)?;
    let signed = sign_query_request_with_signer(
        py,
        authority,
        signer,
        network_id.as_inner(),
        binding.query(),
    )?;
    Ok(Py::from(PyBytes::new(py, &signed)))
}

fn encode_projection<T: norito::json::JsonSerialize>(value: &T) -> PyResult<String> {
    let projection = norito::json::to_value(value).map_err(|_| {
        PyRuntimeError::new_err("privacy state-query result could not be projected as JSON")
    })?;
    let encoded = norito::json::to_string(&projection).map_err(|_| {
        PyRuntimeError::new_err("privacy state-query result JSON could not be encoded")
    })?;
    if encoded.is_empty() || encoded.len() > PRIVACY_FINALIZED_STATE_RESPONSE_MAX_BYTES_V1 {
        return Err(PyRuntimeError::new_err(
            "privacy state-query JSON projection violates its closed byte bound",
        ));
    }
    Ok(encoded)
}

/// Decode, validate, and request-bind one canonical finalized privacy-state response.
#[pyfunction]
#[pyo3(name = "inspect_privacy_finalized_state_query_response")]
pub(crate) fn inspect_response(
    network_id: &PyNetworkId,
    query_id: u32,
    protocol_index: u32,
    request_binding: &[u8],
    response: &[u8],
) -> PyResult<String> {
    let binding = parse_binding(query_id, protocol_index, request_binding)?;
    if response.is_empty() || response.len() > PRIVACY_FINALIZED_STATE_RESPONSE_MAX_BYTES_V1 {
        return Err(PyValueError::new_err(
            "privacy state-query response is outside its closed byte bound",
        ));
    }
    let decoded: QueryResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| PyValueError::new_err("privacy state-query response is not canonical Norito"))?;
    let canonical = norito::to_bytes(&decoded).map_err(|_| {
        PyRuntimeError::new_err("privacy state-query response could not be canonically re-encoded")
    })?;
    if canonical != response {
        return Err(PyValueError::new_err(
            "privacy state-query response is not its exact canonical wire",
        ));
    }
    let expected_network_id = network_id.as_inner();
    match (binding, decoded) {
        (
            PrivacyFinalizedStateBindingV1::ZkAceReplay {
                policy_id,
                replay_nullifier,
            },
            QueryResponse::Singular(
                SingularQueryOutputBox::PrivacyZkAceReplayNullifierProvenanceV1(view),
            ),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("ZK-ACE replay provenance failed native validation")
            })?;
            if view.network_id != *expected_network_id
                || view.policy_id != policy_id
                || view.replay_nullifier != replay_nullifier
            {
                return Err(PyValueError::new_err(
                    "ZK-ACE replay provenance differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::ProofManagedPool {
                protocol_id,
                pool_id,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyProofManagedPoolStateViewV1(
                view,
            )),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("proof-managed pool state failed native validation")
            })?;
            if view.network_id != *expected_network_id
                || view.protocol_id != protocol_id
                || view.pool_id != pool_id
            {
                return Err(PyValueError::new_err(
                    "proof-managed pool state differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::OrchardPool { pool_id },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyOrchardPoolStateViewV1(view)),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("Orchard pool state failed native validation")
            })?;
            if view.network_id != *expected_network_id || view.pool_id != pool_id {
                return Err(PyValueError::new_err(
                    "Orchard pool state differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::OrchardNullifier { pool_id, nullifier },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyOrchardNullifierProvenanceV1(
                view,
            )),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("Orchard nullifier provenance failed native validation")
            })?;
            if view.network_id != *expected_network_id
                || view.pool_id != pool_id
                || view.nullifier != nullifier
            {
                return Err(PyValueError::new_err(
                    "Orchard nullifier provenance differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::AnonymousPgcPool { pool_id },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyAnonymousPgcPoolStateViewV1(
                view,
            )),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("Anonymous PGC pool state failed native validation")
            })?;
            if view.network_id != *expected_network_id || view.pool_id != pool_id {
                return Err(PyValueError::new_err(
                    "Anonymous PGC pool state differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::ZkAmsAdmission {
                issuer_id,
                registry_id,
                policy_id,
                phc_hash,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyZkAmsAdmissionViewV1(view)),
        ) => {
            view.validate()
                .map_err(|_| PyValueError::new_err("ZK-AMS admission failed native validation"))?;
            if view.network_id != *expected_network_id
                || view.issuer_id != issuer_id
                || view.registry_id != registry_id
                || view.policy_id != policy_id
                || view.phc_hash != phc_hash
            {
                return Err(PyValueError::new_err(
                    "ZK-AMS admission differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::ZkAmsProvision {
                issuer_id,
                registry_id,
                policy_id,
                key_image,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyZkAmsProvisionViewV1(view)),
        ) => {
            view.validate()
                .map_err(|_| PyValueError::new_err("ZK-AMS provision failed native validation"))?;
            if view.network_id != *expected_network_id
                || view.issuer_id != issuer_id
                || view.registry_id != registry_id
                || view.policy_id != policy_id
                || view.key_image != key_image
            {
                return Err(PyValueError::new_err(
                    "ZK-AMS provision differs from its request",
                ));
            }
            encode_projection(&view)
        }
        (
            PrivacyFinalizedStateBindingV1::ZkX509Nullifier {
                trust_anchor_id,
                policy_id,
                nullifier,
            },
            QueryResponse::Singular(
                SingularQueryOutputBox::PrivacyZkX509CertificateNullifierProvenanceV1(view),
            ),
        ) => {
            view.validate().map_err(|_| {
                PyValueError::new_err("ZK-X509 nullifier provenance failed native validation")
            })?;
            if view.network_id != *expected_network_id
                || view.trust_anchor_id != trust_anchor_id
                || view.policy_id != policy_id
                || view.nullifier != nullifier
            {
                return Err(PyValueError::new_err(
                    "ZK-X509 nullifier provenance differs from its request",
                ));
            }
            encode_projection(&view)
        }
        _ => Err(PyValueError::new_err(
            "privacy state query returned an unexpected typed response",
        )),
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::PrivacyProtocolIdV1;

    use super::{PrivacyFinalizedStateBindingV1, parse_binding, proof_managed_protocol};

    #[test]
    fn binding_parser_is_closed_and_rejects_zero_or_trailing_material() {
        let mut replay = [0x41_u8; 64];
        assert!(matches!(
            parse_binding(97, 0, &replay).expect("exact replay binding"),
            PrivacyFinalizedStateBindingV1::ZkAceReplay { .. }
        ));
        replay[32..64].fill(0);
        assert!(parse_binding(97, 0, &replay).is_err());
        assert!(parse_binding(97, 0, &[0x41; 65]).is_err());
        assert!(parse_binding(105, 0, &[0x41; 32]).is_err());
    }

    #[test]
    fn proof_managed_protocol_union_is_exact() {
        assert_eq!(
            proof_managed_protocol(0).expect("FCMP++"),
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        );
        assert_eq!(
            proof_managed_protocol(1).expect("private IVM"),
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        );
        assert_eq!(
            proof_managed_protocol(2).expect("PQ-MASP"),
            PrivacyProtocolIdV1::PqMaspStarkV0
        );
        assert!(proof_managed_protocol(3).is_err());
    }
}
