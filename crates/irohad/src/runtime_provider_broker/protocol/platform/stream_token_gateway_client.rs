//! Broker client facade for deployment-owned stream-token gateway admission.
use super::*;
fn map_error(error: BrokerError) -> iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1 {
    use iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1 as Error;
    match error {
        BrokerError::Unavailable => Error::Unavailable,
        BrokerError::Rejected => Error::Rejected,
        BrokerError::Conflict => Error::Conflict,
        BrokerError::StaleOrRevoked => Error::StaleOrRevoked,
        BrokerError::Ambiguous => Error::Ambiguous,
        BrokerError::Protocol => Error::SubstitutedOutcome,
        BrokerError::BindingMismatch => Error::BindingMismatch,
    }
}
#[derive(Clone)]
pub(super) struct StreamTokenGatewayAdmissionBrokerProvider {
    pub(super) session: Arc<BrokerSession>,
    pub(super) binding: ProviderBindingWireV1,
    pub(super) metadata_digest: [u8; 32],
    pub(super) qualification: iroha_torii::sorafs::StreamTokenGatewayAdmissionQualificationV1,
}
impl std::fmt::Debug for StreamTokenGatewayAdmissionBrokerProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StreamTokenGatewayAdmissionBrokerProvider")
            .field("handle", &self.binding.handle)
            .field("qualification", &self.qualification)
            .finish_non_exhaustive()
    }
}
impl iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1
    for StreamTokenGatewayAdmissionBrokerProvider
{
    fn handle(&self) -> &str {
        &self.binding.handle
    }
    fn qualification(
        &self,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionQualificationV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        live_exact_qualification(self.session.as_ref(), &self.binding, self.metadata_digest)
            .map_err(map_error)?;
        Ok(self.qualification)
    }
    fn admit(
        &self,
        request: &iroha_torii::sorafs::StreamTokenGatewayAdmissionRequestV1,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionResultV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        request.validate()?;
        let payload =
            encode_canonical(request, MAX_BROKER_UNARY_FRAME_BYTES_V1).map_err(map_error)?;
        let result = self
            .session
            .call(
                &self.binding,
                self.metadata_digest,
                OPERATION_STREAM_TOKEN_GATEWAY_ADMIT_V1,
                payload,
                true,
            )
            .map_err(map_error)?;
        let admission = self
            .session
            .decode_result::<iroha_torii::sorafs::StreamTokenGatewayAdmissionResultV1>(&result)
            .map_err(map_error)?;
        admission.validate_for_request(request, self.qualification)?;
        Ok(admission)
    }
    fn pending(
        &self,
        max_items: u32,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionReadbackV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        let configured = self
            .binding
            .stream_token_gateway_admission_reconcile_max_items
            .ok_or(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::BindingMismatch)?;
        if max_items == 0 || max_items > configured {
            return Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        let payload =
            encode_canonical(&max_items, MAX_BROKER_UNARY_FRAME_BYTES_V1).map_err(map_error)?;
        let result = self
            .session
            .call(
                &self.binding,
                self.metadata_digest,
                OPERATION_STREAM_TOKEN_GATEWAY_PENDING_V1,
                payload,
                false,
            )
            .map_err(map_error)?;
        let pending = self
            .session
            .decode_result::<iroha_torii::sorafs::StreamTokenGatewayAdmissionReadbackV1>(&result)
            .map_err(map_error)?;
        if pending.validate(max_items, self.qualification).is_err() {
            self.session.poison();
            return Err(
                iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::SubstitutedOutcome,
            );
        }
        Ok(pending)
    }
    fn acknowledge(
        &self,
        record: iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        self.ack_or_release(record, OPERATION_STREAM_TOKEN_GATEWAY_ACKNOWLEDGE_V1)
    }
    fn release_lease(
        &self,
        record: iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        if record.outcome.status
            != iroha_data_model::sorafs::reputation::StreamTokenValidationStatusV1::Accepted
        {
            return Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::InvalidRequest);
        }
        self.ack_or_release(record, OPERATION_STREAM_TOKEN_GATEWAY_RELEASE_LEASE_V1)
    }
}
impl StreamTokenGatewayAdmissionBrokerProvider {
    fn ack_or_release(
        &self,
        record: iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
        operation: u16,
    ) -> Result<
        iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1,
        iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
    > {
        record.validate_shape(self.qualification)?;
        let payload =
            encode_canonical(&record, MAX_BROKER_UNARY_FRAME_BYTES_V1).map_err(map_error)?;
        let result = self
            .session
            .call(
                &self.binding,
                self.metadata_digest,
                operation,
                payload,
                true,
            )
            .map_err(map_error)?;
        self.session
            .decode_result::<iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1>(&result)
            .map_err(map_error)
    }
}
