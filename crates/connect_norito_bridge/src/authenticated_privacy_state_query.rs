//! Native construction and inspection for authenticated finalized privacy-state queries.
//!
//! The mobile boundary accepts only one of singular query IDs 97 through 104 and the exact
//! fixed-width selector bytes for that query. Native code owns the Norito query shape, validates
//! the detached authority signature, and projects only the matching validated typed response.

use iroha_crypto::{HashOf, SignatureOf};
use iroha_data_model::{
    privacy::{
        PrivacyIssuerIdV1, PrivacyNullifierV1, PrivacyPolicyIdV1, PrivacyPoolIdV1,
        PrivacyProtocolIdV1, PrivacyZkAmsKeyImageV1, PrivacyZkAmsPhcHashV1,
        PrivacyZkAmsRegistryIdV1,
    },
    query::{
        QueryRequest, QueryRequestWithAuthority, QueryResponse, QuerySignature, SignedQuery,
        SingularQueryBox, SingularQueryOutputBox,
        privacy::prelude::{
            FindPrivacyAnonymousPgcPoolStateV1, FindPrivacyOrchardNullifierV1,
            FindPrivacyOrchardPoolStateV1, FindPrivacyProofManagedPoolStateV1,
            FindPrivacyZkAceReplayNullifierV1, FindPrivacyZkAmsAdmissionV1,
            FindPrivacyZkAmsProvisionV1, FindPrivacyZkX509CertificateNullifierV1,
        },
    },
};
use iroha_version::codec::EncodeVersioned as _;
use norito::{NoritoDeserialize, NoritoSerialize};
use std::num::NonZeroU64;

use super::{
    authenticated_transaction_details::canonical_authority, connect_signature_from_algorithm_bytes,
    network_id_from_raw_bytes,
};

pub(super) const AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_MAX_BYTES_V1: usize = 64 * 1024;
pub(super) const AUTHENTICATED_PRIVACY_STATE_QUERY_RESPONSE_MAX_BYTES_V1: usize = 256 * 1024;
pub(super) const AUTHENTICATED_PRIVACY_STATE_QUERY_SIGNATURE_MAX_BYTES_V1: usize = 16 * 1024;
pub(super) const AUTHENTICATED_PRIVACY_STATE_QUERY_RESULT_MAX_BYTES_V1: usize = 256 * 1024;
pub(super) const AUTHENTICATED_PRIVACY_STATE_QUERY_BINDING_MAX_BYTES_V1: usize = 128;
const AUTHENTICATED_PRIVACY_STATE_QUERY_SIGNED_QUERY_MAX_BYTES_V1: usize = 64 * 1024;
const AUTHENTICATED_PRIVACY_STATE_QUERY_TTL_MS_V1: u64 = 100_000;
const AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_VERSION_V1: u8 = 1;

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

#[derive(NoritoSerialize, NoritoDeserialize)]
struct AuthenticatedPrivacyStateQueryPreparationV1 {
    version: u8,
    authority_literal: String,
    query_id: u32,
    protocol_index: u32,
    request_binding: Vec<u8>,
    payload: QueryRequestWithAuthority,
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
    if chunk == [0; 32] {
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
    query_id: u32,
    protocol_index: u32,
    binding: &[u8],
) -> Result<PrivacyStateQueryBindingV1, String> {
    if query_id != 98 && protocol_index != 0 {
        return Err("protocol index is reserved to query ID 98".to_owned());
    }
    match query_id {
        97 => {
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
        98 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::ProofManagedPool {
                protocol_id: proof_managed_protocol(protocol_index)?,
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        99 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::OrchardPool {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        100 => {
            require_binding_len(binding, 64)?;
            Ok(PrivacyStateQueryBindingV1::OrchardNullifier {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
                nullifier: nonzero_chunk(binding, 32, "Orchard nullifier")?,
            })
        }
        101 => {
            require_binding_len(binding, 32)?;
            Ok(PrivacyStateQueryBindingV1::AnonymousPgcPool {
                pool_id: PrivacyPoolIdV1::new(nonzero_chunk(binding, 0, "pool id")?),
            })
        }
        102 => {
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
        103 => {
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
        104 => {
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
        _ => Err("privacy state-query ID is outside the closed 97-104 union".to_owned()),
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

fn validate_preparation(
    preparation: &AuthenticatedPrivacyStateQueryPreparationV1,
) -> Result<(), String> {
    if preparation.version != AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_VERSION_V1 {
        return Err("unsupported authenticated privacy state-query preparation version".to_owned());
    }
    let authority = canonical_authority(&preparation.authority_literal).map_err(str::to_owned)?;
    let binding = parse_binding(
        preparation.query_id,
        preparation.protocol_index,
        &preparation.request_binding,
    )?;
    let expected_request = binding.query();
    let observed_request_bytes = norito::to_bytes(preparation.payload.request())
        .map_err(|_| "authenticated privacy state query could not be re-encoded".to_owned())?;
    let expected_request_bytes = norito::to_bytes(&expected_request)
        .map_err(|_| "expected privacy state query could not be encoded".to_owned())?;
    if preparation.payload.authority() != &authority
        || preparation.payload.creation_time_ms() == 0
        || preparation.payload.time_to_live_ms().get()
            != AUTHENTICATED_PRIVACY_STATE_QUERY_TTL_MS_V1
        || preparation.payload.nonce() == &[0; 32]
        || observed_request_bytes != expected_request_bytes
    {
        return Err("authenticated privacy state-query preparation binding is invalid".to_owned());
    }
    Ok(())
}

fn decode_preparation(
    preparation: &[u8],
) -> Result<AuthenticatedPrivacyStateQueryPreparationV1, String> {
    if preparation.is_empty()
        || preparation.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_MAX_BYTES_V1
    {
        return Err("privacy state-query preparation is outside its closed byte bound".to_owned());
    }
    let decoded =
        norito::decode_canonical::<AuthenticatedPrivacyStateQueryPreparationV1>(preparation)
            .map_err(|_| "privacy state-query preparation is not canonical Norito".to_owned())?;
    validate_preparation(&decoded)?;
    Ok(decoded)
}

pub(super) fn authenticated_privacy_state_query_prepare_v1(
    network_id: &[u8],
    authority_literal: &str,
    query_id: u32,
    protocol_index: u32,
    request_binding: &[u8],
    creation_time_ms: u64,
    nonce: [u8; 32],
) -> Result<(Vec<u8>, [u8; 32]), String> {
    if creation_time_ms == 0 || nonce == [0; 32] {
        return Err("privacy state-query freshness must be positive and nonzero".to_owned());
    }
    let network_id = network_id_from_raw_bytes(network_id).map_err(str::to_owned)?;
    let authority = canonical_authority(authority_literal).map_err(str::to_owned)?;
    let binding = parse_binding(query_id, protocol_index, request_binding)?;
    let payload = binding.query().with_authority(
        network_id,
        authority,
        creation_time_ms,
        NonZeroU64::new(AUTHENTICATED_PRIVACY_STATE_QUERY_TTL_MS_V1)
            .expect("authenticated query TTL is nonzero"),
        nonce,
    );
    let preparation = AuthenticatedPrivacyStateQueryPreparationV1 {
        version: AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_VERSION_V1,
        authority_literal: authority_literal.to_owned(),
        query_id,
        protocol_index,
        request_binding: request_binding.to_vec(),
        payload,
    };
    validate_preparation(&preparation)?;
    let signing_digest = *HashOf::new(&preparation.payload).as_ref();
    let archive = norito::encode_canonical(&preparation)
        .map_err(|_| "failed to encode canonical privacy state-query preparation".to_owned())?;
    if archive.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_PREPARATION_MAX_BYTES_V1 {
        return Err("privacy state-query preparation exceeds its closed byte bound".to_owned());
    }
    Ok((archive, signing_digest))
}

pub(super) fn authenticated_privacy_state_query_finalize_v1(
    preparation: &[u8],
    signature_bytes: &[u8],
) -> Result<Vec<u8>, String> {
    if signature_bytes.is_empty()
        || signature_bytes.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_SIGNATURE_MAX_BYTES_V1
    {
        return Err("privacy state-query signature is outside its closed byte bound".to_owned());
    }
    let preparation = decode_preparation(preparation)?;
    let signatory = preparation
        .payload
        .authority()
        .try_signatory()
        .ok_or_else(|| "query authority must be single-key".to_owned())?;
    let algorithm = signatory
        .try_algorithm()
        .map_err(|_| "query authority signature algorithm is invalid".to_owned())?;
    let signature = connect_signature_from_algorithm_bytes(algorithm, signature_bytes)
        .ok_or_else(|| "query signature material is malformed".to_owned())?;
    let signature = SignatureOf::<QueryRequestWithAuthority>::from_signature(signature);
    signature
        .verify(signatory, &preparation.payload)
        .map_err(|_| "query signature does not authenticate the native payload".to_owned())?;
    let signed = SignedQuery {
        signature: QuerySignature(signature),
        payload: preparation.payload,
    };
    signed
        .verify_signature()
        .map_err(|_| "final signed query failed native verification".to_owned())?;
    let body = signed.encode_versioned();
    if body.is_empty() || body.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_SIGNED_QUERY_MAX_BYTES_V1 {
        return Err("final privacy state query violates its closed byte bound".to_owned());
    }
    Ok(body)
}

fn stringify_projection_numbers(value: &mut norito::json::Value) -> Result<(), String> {
    match value {
        norito::json::Value::Number(number) => {
            let number = match *number {
                norito::json::Number::I64(value) => value.to_string(),
                norito::json::Number::U64(value) => value.to_string(),
                norito::json::Number::F64(value) if value.is_finite() => norito::json::to_string(
                    &norito::json::Value::Number(*number),
                )
                .map_err(|_| "privacy state-query number could not be projected".to_owned())?,
                norito::json::Number::F64(_) => {
                    return Err(
                        "privacy state-query projection contains a non-finite number".to_owned(),
                    );
                }
            };
            *value = norito::json::Value::String(number);
        }
        norito::json::Value::Array(values) => {
            for value in values {
                if !matches!(value, norito::json::Value::Number(_)) {
                    stringify_projection_numbers(value)?;
                }
            }
        }
        norito::json::Value::Object(values) => {
            for value in values.values_mut() {
                stringify_projection_numbers(value)?;
            }
        }
        norito::json::Value::Null
        | norito::json::Value::Bool(_)
        | norito::json::Value::String(_) => {}
    }
    Ok(())
}

fn encode_projection<T: norito::json::JsonSerialize>(value: &T) -> Result<Vec<u8>, String> {
    let mut value = norito::json::to_value(value)
        .map_err(|_| "privacy state-query result could not be projected as JSON".to_owned())?;
    // Managed SDK number types cannot all represent the complete u64 domain. Preserve historical
    // fixed-byte arrays, but stringify every structured numeric leaf before it crosses the ABI.
    stringify_projection_numbers(&mut value)?;
    let projection = norito::json::to_vec(&value)
        .map_err(|_| "privacy state-query result could not be projected as JSON".to_owned())?;
    if projection.is_empty()
        || projection.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_RESULT_MAX_BYTES_V1
    {
        return Err(
            "privacy state-query JSON projection violates its closed byte bound".to_owned(),
        );
    }
    Ok(projection)
}

pub(super) fn authenticated_privacy_state_query_project_result_v1(
    preparation: &[u8],
    response: &[u8],
) -> Result<Vec<u8>, String> {
    let preparation = decode_preparation(preparation)?;
    if response.is_empty()
        || response.len() > AUTHENTICATED_PRIVACY_STATE_QUERY_RESPONSE_MAX_BYTES_V1
    {
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
    let expected_network_id = preparation.payload.network_id();
    let binding = parse_binding(
        preparation.query_id,
        preparation.protocol_index,
        &preparation.request_binding,
    )?;
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
    use std::collections::BTreeMap;

    use iroha_data_model::privacy::PrivacyProtocolIdV1;
    use norito::json::{Number, Value};

    use super::{
        PrivacyStateQueryBindingV1, parse_binding, proof_managed_protocol,
        stringify_projection_numbers,
    };

    #[test]
    fn binding_parser_is_closed_and_rejects_zero_trailing_or_reserved_material() {
        let mut replay = [0x41_u8; 64];
        assert!(matches!(
            parse_binding(97, 0, &replay).expect("exact replay binding"),
            PrivacyStateQueryBindingV1::ZkAceReplay { .. }
        ));
        replay[32..64].fill(0);
        assert!(parse_binding(97, 0, &replay).is_err());
        assert!(parse_binding(97, 0, &[0x41; 65]).is_err());
        assert!(parse_binding(105, 0, &[0x41; 32]).is_err());
        assert!(parse_binding(99, 1, &[0x41; 32]).is_err());
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

    #[test]
    fn result_projection_stringifies_structured_numbers_and_preserves_byte_arrays() {
        let mut object = BTreeMap::new();
        object.insert("height".to_owned(), Value::Number(Number::U64(u64::MAX)));
        object.insert(
            "bytes".to_owned(),
            Value::Array(vec![
                Value::Number(Number::U64(0)),
                Value::Number(Number::U64(255)),
            ]),
        );
        let mut value = Value::Object(object);

        stringify_projection_numbers(&mut value).expect("bounded numeric projection");

        let Value::Object(projected) = value else {
            panic!("object projection must preserve its shape");
        };
        assert_eq!(
            projected.get("height"),
            Some(&Value::String(u64::MAX.to_string()))
        );
        assert_eq!(
            projected.get("bytes"),
            Some(&Value::Array(vec![
                Value::Number(Number::U64(0)),
                Value::Number(Number::U64(255)),
            ]))
        );
    }

    #[test]
    fn result_projection_rejects_non_finite_numbers() {
        let mut value = Value::Number(Number::F64(f64::NAN));
        assert!(stringify_projection_numbers(&mut value).is_err());
    }
}
