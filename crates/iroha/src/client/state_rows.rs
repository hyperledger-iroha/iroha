//! Signed exact-row reads of the current certified Global keyed State.

use super::{AccountClient, Client, dispatch, join_torii_url};
use crate::{Error, Result, http::Method};
use iroha_data_model::sumeragi_finality::{
    KeyedStateRowsV1, MAX_STATE_PROOF_KEY_BYTES_V1, MAX_STATE_PROOF_ROWS_V1,
    MAX_STATE_PROOF_TABLE_BYTES_V1, MAX_STATE_ROWS_FRAME_BYTES_V1, keyed_state::StateKeyV1,
};
use iroha_executor_data_model::permission::query::CanReadStateRow;

const OP: &str = "ledger.state_rows.read";
const MAX_REQUEST_BYTES: usize = 64 * 1024;

impl AccountClient {
    /// Read exactly the ascending `(table, bare Norito key)` positions at `height`.
    ///
    /// The canonical network signature covers the POST target, timestamp, nonce and
    /// complete JSON body. Torii requires a direct `CanReadStateRow` grant for each
    /// position (or all-ledger read authority) and serves only its current certified
    /// Global keyed State. This performs one bounded asynchronous request.
    ///
    /// The result is transport data. Authenticate it with [`KeyedStateRowsV1::verify`]
    /// against an independently verified block and its selected schema before using any
    /// row; a successful transport or a proven absence is not payout acknowledgement.
    ///
    /// # Errors
    /// Invalid height/selection/signer, signing or transport failure, HTTP refusal,
    /// malformed canonical response, or a different response height or row selection.
    pub async fn state_rows(&self, height: u64, rows: &[StateKeyV1]) -> Result<KeyedStateRowsV1> {
        let body = selection_body(height, rows)?;
        let client = &self.context;
        if client.account.controller.single_signatory() != Some(client.key_pair.public_key())
            || client
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("X-Iroha-Witness"))
        {
            return Err(invalid(
                "State reads require the direct account signer without witness headers",
            ));
        }
        let path = iroha_torii_shared::route_catalog::core::LEDGER_STATE_ROWS_POST
            .path()
            .replace("{height}", &height.to_string());
        let builder = client
            .account_signed_request(Method::POST, join_torii_url(&client.torii_url, &path), body)
            .map_err(|error| Error::RequestSigning {
                operation: OP,
                details: error.to_string(),
            })?
            .replace_header(http::header::CONTENT_TYPE, "application/json")
            .max_response_bytes(MAX_STATE_ROWS_FRAME_BYTES_V1);
        let response = dispatch::send(client, OP, builder, "application/x-norito").await?;
        if response.status() != http::StatusCode::OK {
            return Err(Error::Http {
                operation: OP,
                status: response.status().as_u16(),
                retry_after: crate::error::retry_after(response.headers()),
                body: response.into_body(),
            });
        }
        let result: KeyedStateRowsV1 =
            Client::decode_canonical_norito_response(&response, MAX_STATE_ROWS_FRAME_BYTES_V1, OP)?;
        if result.height != height
            || result.rows.len() != rows.len()
            || result
                .rows
                .iter()
                .zip(rows)
                .any(|(actual, expected)| actual.position != *expected)
        {
            return Err(Error::ResponseBinding {
                operation: OP,
                field: "State row selection",
            });
        }
        Ok(result)
    }
}

fn invalid(details: &str) -> Error {
    Error::InvalidRequest {
        operation: OP,
        details: details.into(),
    }
}

fn selection_body(height: u64, rows: &[StateKeyV1]) -> Result<Vec<u8>> {
    if height < 2
        || rows.is_empty()
        || rows.len() > MAX_STATE_PROOF_ROWS_V1
        || rows.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "State rows require non-genesis height and a bounded strictly ascending selection",
        ));
    }
    // Bound source strings and keys before allocating hexadecimal copies.
    if rows.iter().any(|row| {
        row.table.len() > MAX_STATE_PROOF_TABLE_BYTES_V1
            || row.key.len() > MAX_STATE_PROOF_KEY_BYTES_V1
            || !StateKeyV1::valid_table_id(&row.table)
    }) {
        return Err(invalid("State row key or table is invalid"));
    }
    let selectors: Vec<_> = rows
        .iter()
        .map(|row| CanReadStateRow {
            table: row.table.clone(),
            key_hex: hex::encode(&row.key),
        })
        .collect();
    if !selectors.iter().all(CanReadStateRow::is_valid) {
        return Err(invalid("State row key or table is invalid"));
    }
    #[derive(norito::derive::JsonSerialize)]
    struct Selection {
        rows: Vec<CanReadStateRow>,
    }
    let body = norito::json::to_vec(&Selection { rows: selectors })
        .map_err(|error| invalid(&error.to_string()))?;
    if body.len() > MAX_REQUEST_BYTES {
        return Err(invalid("State row request exceeds its body bound"));
    }
    Ok(body)
}

#[cfg(test)]
mod tests;
