//! Bounded owner-signed FI receipt candidates; no execution or finality authority.
use super::*;
use iroha_data_model::{
    asset::fi_link::AssetFiTransferReceiptV1,
    query::{QueryOutputBatchBox, asset_fi::FindAssetFiTransferReceiptV1, parameters::FetchSize},
};

const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

impl Client {
    /// Read zero or one immutable FI occurrence candidate by its exact historical key.
    ///
    /// Signs as this client's authority and requests ephemeral, bounded-count
    /// execution. Enforces the response byte limit before decoding, never follows
    /// a continuation, and never retransmits the query. Only a complete empty
    /// typed result means absence; HTTP failures do not.
    ///
    /// `None` is a complete zero-row candidate response, not certified absence.
    ///
    /// The returned candidate is not authenticated execution or finality evidence.
    /// Consumers must independently verify certified membership and join the
    /// original execution and exact native transcript occurrence before use.
    ///
    /// # Errors
    /// Refuses invalid bounds or incarnation, transport/admission failures,
    /// noncanonical media/encoding, incomplete results and changed network or key.
    pub fn get_asset_fi_transfer_receipt(
        &self,
        query: FindAssetFiTransferReceiptV1,
        maximum_bytes: usize,
    ) -> Result<Option<AssetFiTransferReceiptV1>, QueryError> {
        if maximum_bytes == 0 || maximum_bytes > MAX_RESPONSE_BYTES {
            return Err(eyre!("FI receipt response limit must be in 1..=64 MiB").into());
        }
        query
            .expected_incarnation
            .validate()
            .map_err(|error| QueryError::Other(eyre!(error)))?;
        self.ensure_data_model_compatibility()
            .map_err(QueryError::from)?;
        let (_, request) = self
            .query(query.clone())
            .with_fetch_size(FetchSize {
                fetch_size: NonZeroU64::new(1),
            })
            .into_request();
        let head = self.get_query_request_head();
        let body = head.sign_and_encode(QueryRequest::Start(request))?;
        let route = format!(
            "{}?cursor_mode=ephemeral&count_mode=bounded",
            torii_routes::pipeline::QUERY.path()
        );
        let response = send_once(|| {
            Ok(head
                .assemble_canonical_norito_body_at(body.clone(), &route)
                .max_response_bytes(maximum_bytes))
        })?;
        decode_receipt_response(&response, &query, self.network_id, maximum_bytes)
    }
}

fn decode_receipt_response(
    response: &http::Response<Vec<u8>>,
    query: &FindAssetFiTransferReceiptV1,
    network: NetworkId,
    maximum_bytes: usize,
) -> Result<Option<AssetFiTransferReceiptV1>, QueryError> {
    if response.body().len() > maximum_bytes {
        return Err(eyre!("FI receipt response exceeds its byte limit").into());
    }
    if response.status() != StatusCode::OK {
        return Err(decode_query_failure(response));
    }
    let decoded: QueryResponse = Client::decode_canonical_norito_response(
        response,
        maximum_bytes,
        "query.asset_fi_transfer_receipt",
    )
    .map_err(QueryError::from)?;
    let QueryResponse::Iterable(output) = decoded else {
        return Err(eyre!("FI receipt response must be iterable").into());
    };
    let (batch, remaining, has_more, cursor) = output.into_parts_with_count_mode();
    if has_more
        || cursor.is_some()
        || remaining.is_some_and(|count| count != 0)
        || batch.column_count() != 1
        || batch.len() > 1
    {
        return Err(
            eyre!("FI receipt response must contain zero or one complete candidate").into(),
        );
    }
    let mut columns = batch.into_columns();
    let QueryOutputBatchBox::AssetFiTransferReceiptV1(mut receipts) = columns.remove(0) else {
        return Err(eyre!("FI receipt response has a different item type").into());
    };
    let Some(receipt) = receipts.pop() else {
        return Ok(None);
    };
    if receipt.network != network
        || receipt.instruction.asset_definition_id != query.asset_definition_id
        || receipt.instruction.account_id != query.account_id
        || receipt.asset_incarnation != query.expected_incarnation
        || receipt.execution_call_hash != query.execution_call_hash
        || receipt.transcript_index != query.transcript_index
    {
        return Err(
            eyre!("FI receipt response differs from the exact network or historical key").into(),
        );
    }
    Ok(Some(receipt))
}

#[cfg(test)]
mod tests;
