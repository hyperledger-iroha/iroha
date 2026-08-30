//! Native signed-query construction and finalized Exact12 state inspection for JavaScript.
//!
//! The managed layer supplies only a closed query discriminant and its exact fixed-width
//! identifiers. Native code constructs the typed singular query, signs it through the shared
//! canonical authority path, and accepts only the matching validated response on the requested
//! `NetworkId`. No generic query payload or caller-authored projection crosses this boundary.

use iroha_data_model::{
    NetworkId,
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

use super::{
    authenticated_transaction_details::build_signed_request_v1, parse_transaction_network_id_bytes,
};

pub(crate) const PRIVACY_STATE_QUERY_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrivacyStateQueryBindingV1 {
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

fn nonzero_chunk(binding: &[u8], offset: usize, label: &str) -> Result<[u8; 32], String> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| format!("{label} offset exceeds usize"))?;
    let chunk: [u8; 32] = binding
        .get(offset..end)
        .ok_or_else(|| format!("{label} is missing from the fixed query binding"))?
        .try_into()
        .map_err(|_| format!("{label} must contain exactly 32 bytes"))?;
    if chunk.iter().all(|byte| *byte == 0) {
        return Err(format!("{label} must not be all zero"));
    }
    Ok(chunk)
}

fn require_binding_len(binding: &[u8], expected: usize) -> Result<(), String> {
    if binding.len() != expected {
        return Err(format!(
            "privacy state-query binding must contain exactly {expected} bytes"
        ));
    }
    Ok(())
}

fn proof_managed_protocol(index: u32) -> Result<PrivacyProtocolIdV1, String> {
    match index {
        0 => Ok(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1),
        1 => Ok(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1),
        2 => Ok(PrivacyProtocolIdV1::PqMaspStarkV0),
        _ => Err("proof-managed state query protocol is outside its closed union".to_owned()),
    }
}

fn parse_binding(
    query_index: u32,
    protocol_index: u32,
    binding: &[u8],
) -> Result<PrivacyStateQueryBindingV1, String> {
    match query_index {
        0 => {
            require_binding_len(binding, 64)?;
            Ok(PrivacyStateQueryBindingV1::ZkAceReplay {
                policy_id: PrivacyPolicyIdV1::new(nonzero_chunk(binding, 0, "policy id")?),
                replay_nullifier: PrivacyNullifierV1::new(nonzero_chunk(
                    binding,
                    32,
                    "replay nullifier",
                )?),
            })
        }
        1 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::ProofManagedPool {
                protocol_id: proof_managed_protocol(protocol_index)?,
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        2 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::OrchardPool {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        3 => {
            require_binding_len(binding, 64)?;
            Ok(PrivacyStateQueryBindingV1::OrchardNullifier {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
                nullifier: nonzero_chunk(binding, 32, "Orchard nullifier")?,
            })
        }
        4 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::AnonymousPgcPool {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        5 => {
            require_binding_len(binding, 128)?;
            Ok(PrivacyStateQueryBindingV1::ZkAmsAdmission {
                issuer_id: PrivacyIssuerIdV1::new(nonzero_chunk(binding, 0, "issuer id")?),
                registry_id: PrivacyZkAmsRegistryIdV1::new(nonzero_chunk(
                    binding,
                    32,
                    "registry id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(nonzero_chunk(binding, 64, "policy id")?),
                phc_hash: PrivacyZkAmsPhcHashV1::new(nonzero_chunk(binding, 96, "PHC hash")?),
            })
        }
        6 => {
            require_binding_len(binding, 128)?;
            Ok(PrivacyStateQueryBindingV1::ZkAmsProvision {
                issuer_id: PrivacyIssuerIdV1::new(nonzero_chunk(binding, 0, "issuer id")?),
                registry_id: PrivacyZkAmsRegistryIdV1::new(nonzero_chunk(
                    binding,
                    32,
                    "registry id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(nonzero_chunk(binding, 64, "policy id")?),
                key_image: PrivacyZkAmsKeyImageV1::new(nonzero_chunk(binding, 96, "key image")?),
            })
        }
        7 => {
            require_binding_len(binding, 96)?;
            Ok(PrivacyStateQueryBindingV1::ZkX509Nullifier {
                trust_anchor_id: PrivacyIssuerIdV1::new(nonzero_chunk(
                    binding,
                    0,
                    "trust-anchor id",
                )?),
                policy_id: PrivacyPolicyIdV1::new(nonzero_chunk(binding, 32, "policy id")?),
                nullifier: PrivacyNullifierV1::new(nonzero_chunk(
                    binding,
                    64,
                    "certificate nullifier",
                )?),
            })
        }
        _ => Err("privacy state-query discriminant is outside the closed union".to_owned()),
    }
}

impl PrivacyStateQueryBindingV1 {
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

pub(crate) fn build_signed_query_v1(
    authority_literal: &str,
    private_key_bytes: &[u8],
    network_id_bytes: &[u8],
    query_index: u32,
    protocol_index: u32,
    request_binding: &[u8],
) -> Result<Vec<u8>, String> {
    let binding = parse_binding(query_index, protocol_index, request_binding)?;
    build_signed_request_v1(
        authority_literal,
        private_key_bytes,
        network_id_bytes,
        binding.query(),
    )
}

fn stringify_projection_numbers(value: &mut norito::json::Value) {
    match value {
        norito::json::Value::Number(number) => {
            *value = norito::json::Value::String(number.to_string());
        }
        norito::json::Value::Array(values) => {
            for value in values {
                // Fixed-byte fields use the historical JSON byte-array representation. Preserve
                // those bounded byte numbers; recurse only into structured array elements.
                if !matches!(value, norito::json::Value::Number(_)) {
                    stringify_projection_numbers(value);
                }
            }
        }
        norito::json::Value::Object(values) => {
            for value in values.values_mut() {
                stringify_projection_numbers(value);
            }
        }
        norito::json::Value::Null
        | norito::json::Value::Bool(_)
        | norito::json::Value::String(_) => {}
    }
}

fn encode_projection<T: norito::json::JsonSerialize>(value: &T) -> Result<String, String> {
    let mut value = norito::json::to_value(value)
        .map_err(|_| "privacy state-query result could not be projected as JSON".to_owned())?;
    // JavaScript JSON numbers cannot carry the complete u64 domain. Every native numeric leaf is
    // therefore emitted as a canonical decimal string; named JS views can narrow u32 fields after
    // checking their bounds without ever silently rounding a finalized height or count.
    stringify_projection_numbers(&mut value);
    let projection = norito::json::to_string(&value)
        .map_err(|_| "privacy state-query result could not be projected as JSON".to_owned())?;
    if projection.is_empty() || projection.len() > PRIVACY_STATE_QUERY_RESPONSE_MAX_BYTES_V1 {
        return Err(
            "privacy state-query JSON projection violates its closed byte bound".to_owned(),
        );
    }
    Ok(projection)
}

pub(crate) fn inspect_response_v1(
    network_id_bytes: &[u8],
    query_index: u32,
    protocol_index: u32,
    request_binding: &[u8],
    response: &[u8],
) -> Result<String, String> {
    let expected_network_id: NetworkId = parse_transaction_network_id_bytes(network_id_bytes)
        .map_err(|error| error.reason.clone())?;
    let binding = parse_binding(query_index, protocol_index, request_binding)?;
    if response.is_empty() || response.len() > PRIVACY_STATE_QUERY_RESPONSE_MAX_BYTES_V1 {
        return Err("privacy state-query response is outside its closed byte bound".to_owned());
    }
    let decoded: QueryResponse = norito::decode_canonical_with_limits(
        response,
        norito::canonical_decode_limits(response.len()),
    )
    .map_err(|_| "privacy state-query response is not canonical Norito".to_owned())?;
    let canonical = norito::to_bytes(&decoded)
        .map_err(|_| "privacy state-query response could not be re-encoded".to_owned())?;
    if canonical != response {
        return Err("privacy state-query response is not its exact canonical wire".to_owned());
    }
    match (binding, decoded) {
        (
            PrivacyStateQueryBindingV1::ZkAceReplay {
                policy_id,
                replay_nullifier,
            },
            QueryResponse::Singular(
                SingularQueryOutputBox::PrivacyZkAceReplayNullifierProvenanceV1(view),
            ),
        ) => {
            view.validate()
                .map_err(|_| "ZK-ACE replay provenance failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.policy_id != policy_id
                || view.replay_nullifier != replay_nullifier
            {
                return Err("ZK-ACE replay provenance differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::ProofManagedPool {
                protocol_id,
                pool_id,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyProofManagedPoolStateViewV1(
                view,
            )),
        ) => {
            view.validate()
                .map_err(|_| "proof-managed pool state failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.protocol_id != protocol_id
                || view.pool_id != pool_id
            {
                return Err("proof-managed pool state differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::OrchardPool { pool_id },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyOrchardPoolStateViewV1(view)),
        ) => {
            view.validate()
                .map_err(|_| "Orchard pool state failed native validation".to_owned())?;
            if view.network_id != expected_network_id || view.pool_id != pool_id {
                return Err("Orchard pool state differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::OrchardNullifier { pool_id, nullifier },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyOrchardNullifierProvenanceV1(
                view,
            )),
        ) => {
            view.validate()
                .map_err(|_| "Orchard nullifier provenance failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.pool_id != pool_id
                || view.nullifier != nullifier
            {
                return Err("Orchard nullifier provenance differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::AnonymousPgcPool { pool_id },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyAnonymousPgcPoolStateViewV1(
                view,
            )),
        ) => {
            view.validate()
                .map_err(|_| "Anonymous PGC pool state failed native validation".to_owned())?;
            if view.network_id != expected_network_id || view.pool_id != pool_id {
                return Err("Anonymous PGC pool state differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::ZkAmsAdmission {
                issuer_id,
                registry_id,
                policy_id,
                phc_hash,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyZkAmsAdmissionViewV1(view)),
        ) => {
            view.validate()
                .map_err(|_| "ZK-AMS admission failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.issuer_id != issuer_id
                || view.registry_id != registry_id
                || view.policy_id != policy_id
                || view.phc_hash != phc_hash
            {
                return Err("ZK-AMS admission differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::ZkAmsProvision {
                issuer_id,
                registry_id,
                policy_id,
                key_image,
            },
            QueryResponse::Singular(SingularQueryOutputBox::PrivacyZkAmsProvisionViewV1(view)),
        ) => {
            view.validate()
                .map_err(|_| "ZK-AMS provision failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.issuer_id != issuer_id
                || view.registry_id != registry_id
                || view.policy_id != policy_id
                || view.key_image != key_image
            {
                return Err("ZK-AMS provision differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        (
            PrivacyStateQueryBindingV1::ZkX509Nullifier {
                trust_anchor_id,
                policy_id,
                nullifier,
            },
            QueryResponse::Singular(
                SingularQueryOutputBox::PrivacyZkX509CertificateNullifierProvenanceV1(view),
            ),
        ) => {
            view.validate()
                .map_err(|_| "ZK-X509 nullifier provenance failed native validation".to_owned())?;
            if view.network_id != expected_network_id
                || view.trust_anchor_id != trust_anchor_id
                || view.policy_id != policy_id
                || view.nullifier != nullifier
            {
                return Err("ZK-X509 nullifier provenance differs from its request".to_owned());
            }
            encode_projection(&view)
        }
        _ => Err("privacy state query returned an unexpected typed response".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{PrivacyStateQueryBindingV1, parse_binding, proof_managed_protocol};
    use iroha_data_model::privacy::PrivacyProtocolIdV1;

    #[test]
    fn binding_parser_is_closed_and_rejects_zero_or_trailing_material() {
        let mut replay = [0x41_u8; 64];
        assert!(matches!(
            parse_binding(0, 0, &replay).expect("exact replay binding"),
            PrivacyStateQueryBindingV1::ZkAceReplay { .. }
        ));
        replay[32..64].fill(0);
        assert!(parse_binding(0, 0, &replay).is_err());
        assert!(parse_binding(0, 0, &[0x41; 65]).is_err());
        assert!(parse_binding(8, 0, &[0x41; 32]).is_err());
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
