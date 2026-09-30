//! Potr operation handlers for authenticated broker dispatch.

use super::*;

pub(super) fn potr_sign(
    state: &BrokerServerStateV1,
    request: &OperationRequestV1,
) -> Result<Vec<u8>, BrokerError> {
    let slot = request.binding.slot;
    let potr_gateway_slot = IrohaRuntimeProviderSlotV1::PotrGatewaySigner.wire_id();
    let requalify =
        || qualify_server_binding(state, &request.binding, request.provider_metadata_digest);
    let sign =
        decode_canonical::<PotrSignRequestWireV1>(&request.payload, MAX_POTR_FRAME_BYTES_V1)?;
    let runtime = required_binding_ref!(&request.binding, potr_runtime_binding);
    validate_potr_signing_payload(&sign.payload, runtime.baseline_admission_policy.provider_id)?;
    let (role, algorithm, signature) = if slot == potr_gateway_slot {
        let signer = broker_backend!(state, potr_gateway_signer);
        let public_key = signer.public_key().map_err(|_| BrokerError::Unavailable)?;
        if sign.expected_public_key.as_slice() != public_key.as_slice() {
            return Err(BrokerError::BindingMismatch);
        }
        (
            "gateway",
            sorafs_manifest::potr::PotrSignatureAlgorithm::Ed25519,
            signer.sign(&sign.payload).map_err(|error| match error {
                iroha_torii::sorafs::PotrSignerServiceError::Unavailable => {
                    BrokerError::Unavailable
                }
                iroha_torii::sorafs::PotrSignerServiceError::Refused => BrokerError::Rejected,
            })?,
        )
    } else {
        let signer = broker_backend!(state, potr_provider_signer);
        let public_key = signer.public_key().map_err(|_| BrokerError::Unavailable)?;
        if sign.expected_public_key != public_key {
            return Err(BrokerError::BindingMismatch);
        }
        (
            "provider",
            sorafs_manifest::potr::PotrSignatureAlgorithm::MlDsa65,
            signer.sign(&sign.payload).map_err(|error| match error {
                iroha_torii::sorafs::PotrSignerServiceError::Unavailable => {
                    BrokerError::Unavailable
                }
                iroha_torii::sorafs::PotrSignerServiceError::Refused => BrokerError::Rejected,
            })?,
        )
    };
    if signature.is_empty() || signature.len() > MAX_POTR_SIGNATURE_BYTES_V1 {
        return Err(BrokerError::Rejected);
    }
    sorafs_manifest::potr::PotrSignatureV1 {
        algorithm,
        public_key: sign.expected_public_key.clone(),
        signature: signature.clone(),
    }
    .verify(role, &sign.payload)
    .map_err(|_| BrokerError::Rejected)?;
    requalify()?;
    encode_canonical(
        &VariableSignatureResultWireV1 { signature },
        MAX_POTR_FRAME_BYTES_V1,
    )
}
