//! The one received frame, request and socket owner after the only network read.
use super::*;

#[path = "received_exchange/observer.rs"]
mod observer;
#[cfg(test)]
pub(super) use observer::test_hooks;

/// Inputs owned by the single transport attempt before its first write.
pub(super) struct OutboundExchangeV1<'a> {
    pub(super) binding: &'a ProviderBindingWireV1,
    pub(super) metadata_digest: [u8; 32],
    pub(super) operation: u16,
    pub(super) payload: ScrubbedBytes,
    pub(super) mutating: bool,
    pub(super) deadline: BrokerDeadlineV1,
}

pub(super) struct ReceivedExchangeV1<'a> {
    pub(super) request: &'a OperationRequestV1,
    pub(super) response_frame: ScrubbedBytes,
    pub(super) decode_admission: Arc<DecodeResourceAdmissionV1>,
    pub(super) connection: &'a mut BrokerConnection,
    pub(super) deadline: BrokerDeadlineV1,
    pub(super) network_id: &'a NetworkId,
    pub(super) mutating: bool,
    pub(super) transport_failure: BrokerConnectionFailure,
}
impl ReceivedExchangeV1<'_> {
    pub(super) fn observer(
        self,
        expected: &SignerStreamTokenObservationRequestV1,
    ) -> Result<StreamTokenObserverReplyV1, BrokerError> {
        observer::receive(self, expected)
    }

    pub(super) fn regular(self) -> Result<ScrubbedBytes, BrokerError> {
        let Self {
            request,
            response_frame,
            decode_admission,
            connection,
            deadline,
            network_id,
            mutating,
            transport_failure,
        } = self;
        let operation = request.operation;
        let Ok(mut response) = decode_operation_frame::<OperationResponseV1>(
            &response_frame,
            FRAME_KIND_OPERATION_RESPONSE_V1,
            operation,
        ) else {
            let error = if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::Protocol
            };
            connection.poison_reason = Some(BrokerConnectionFailure::Permanent(error));
            return Err(error);
        };
        if let Err(error) = validate_operation_response_for_client(request, &response, network_id) {
            let error = if mutating {
                BrokerError::Ambiguous
            } else {
                error
            };
            connection.poison_reason = Some(BrokerConnectionFailure::Permanent(error));
            return Err(error);
        }
        if deadline.remaining().is_err() {
            let error = if mutating {
                BrokerError::Ambiguous
            } else {
                BrokerError::Unavailable
            };
            connection.poison_reason = Some(transport_failure);
            return Err(error);
        }
        match response.status {
            STATUS_OK_V1 => {
                let result = std::mem::take(&mut response.result);
                Ok(ScrubbedBytes::with_decode_admission(
                    result,
                    decode_admission,
                ))
            }
            STATUS_REJECTED_V1 => Err(BrokerError::Rejected),
            STATUS_CONFLICT_V1 => Err(BrokerError::Conflict),
            STATUS_STALE_OR_REVOKED_V1 => {
                connection.poison_reason = Some(BrokerConnectionFailure::Permanent(
                    BrokerError::StaleOrRevoked,
                ));
                Err(BrokerError::StaleOrRevoked)
            }
            STATUS_AMBIGUOUS_V1 => {
                connection.poison_reason = Some(if mutating {
                    transport_failure
                } else {
                    BrokerConnectionFailure::Permanent(BrokerError::Ambiguous)
                });
                Err(BrokerError::Ambiguous)
            }
            STATUS_UNAVAILABLE_V1 => {
                connection.poison_reason = Some(if mutating {
                    BrokerConnectionFailure::Unavailable
                } else {
                    transport_failure
                });
                Err(BrokerError::Unavailable)
            }
            _ => {
                let error = if mutating {
                    BrokerError::Ambiguous
                } else {
                    BrokerError::Protocol
                };
                connection.poison_reason = Some(BrokerConnectionFailure::Permanent(error));
                Err(error)
            }
        }
    }
}
