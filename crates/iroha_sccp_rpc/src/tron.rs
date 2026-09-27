//! TRON HTTP API client (spec §7.1, §7.2, §8).
//!
//! [`TronClient`] calls the java-tron HTTP routes the builders and wallet
//! flows need: head and solidified blocks, blocks by number and by range with
//! full transactions, solidified transaction info, contract runtime code,
//! constant calls and `broadcasthex`.
//!
//! Transactions keep the exact bytes of `raw_data_hex`, their signatures and
//! their `ret` objects as returned, because the `txTrieRoot` path is rebuilt
//! from the protobuf encoding of those parts and the `ret` re-encoding is
//! pinned by captured fixtures. Block headers keep every `BlockHeader.raw`
//! field the endpoint printed (protobuf JSON omits zero values, so absent
//! numbers read as zero and absent bytes as empty).
//!
//! TRON hex has no prefix; it must have an even number of hex digits.
//! Addresses are 21 bytes starting with `0x41`, except log addresses, which are
//! the 20-byte EVM form. A success response carrying `{"Error": …}` is an
//! [`RpcError::Api`]. Nothing is verified here.

use norito::json::{Map, Value};

use crate::{
    evm::HexError,
    http::{
        HttpTransport, MEDIA_TYPE_JSON, RpcError, encode_json, expect_object, invalid_response,
        optional, optional_array, optional_str, required, required_str, sanitize_message,
    },
};

/// First byte of every 21-byte TRON address.
pub const TRON_ADDRESS_PREFIX: u8 = 0x41;
/// Most blocks one `getblockbylimitnext` request may return.
pub const MAX_BLOCKS_PER_RANGE: u64 = 100;

/// Parses TRON hex: an even number of hex digits without prefix (empty is
/// empty bytes).
///
/// # Errors
/// [`HexError`] for a prefix, an odd length or a non-hex digit.
pub fn parse_tron_hex(text: &str) -> Result<Vec<u8>, HexError> {
    if text.starts_with("0x") || text.starts_with("0X") {
        return Err(HexError::InvalidDigit);
    }
    if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HexError::InvalidDigit);
    }
    if text.len() % 2 != 0 {
        return Err(HexError::OddLength);
    }
    hex::decode(text).map_err(|_| HexError::InvalidDigit)
}

/// Parses TRON hex of exactly `N` bytes.
///
/// # Errors
/// [`HexError`] as [`parse_tron_hex`], or for another length.
pub fn parse_tron_hex_array<const N: usize>(text: &str) -> Result<[u8; N], HexError> {
    let bytes = parse_tron_hex(text)?;
    let found = bytes.len();
    bytes.try_into().map_err(|_| HexError::WrongLength {
        expected: N,
        found,
    })
}

/// Parses a 21-byte TRON address in hex (`41…`).
///
/// # Errors
/// [`HexError`] for malformed hex or another length; a wrong prefix is
/// [`HexError::InvalidDigit`].
pub fn parse_tron_address(text: &str) -> Result<[u8; 21], HexError> {
    let address = parse_tron_hex_array::<21>(text)?;
    if address[0] != TRON_ADDRESS_PREFIX {
        return Err(HexError::InvalidDigit);
    }
    Ok(address)
}

fn address_param(address: &[u8; 21], what: &str) -> Result<Value, RpcError> {
    if address[0] != TRON_ADDRESS_PREFIX {
        return Err(RpcError::InvalidRequest(format!(
            "{what} must be a TRON address starting with 0x41"
        )));
    }
    Ok(Value::from(hex::encode(address)))
}

/// `BlockHeader.raw` fields and the witness signature of a TRON block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronBlockHeader {
    /// `number`.
    pub number: u64,
    /// `timestamp` in milliseconds.
    pub timestamp: u64,
    /// `txTrieRoot`.
    pub tx_trie_root: [u8; 32],
    /// `parentHash`.
    pub parent_hash: [u8; 32],
    /// `witness_address`.
    pub witness_address: [u8; 21],
    /// `witness_id` (zero when absent).
    pub witness_id: u64,
    /// `version` (zero when absent).
    pub version: u32,
    /// `accountStateRoot` (empty when absent).
    pub account_state_root: Vec<u8>,
    /// `witness_signature` (empty when absent).
    pub witness_signature: Vec<u8>,
}

/// A transaction of a TRON block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronTransaction {
    /// `txID` reported by the endpoint.
    pub tx_id: [u8; 32],
    /// Exact `Transaction.raw` bytes (`raw_data_hex`).
    pub raw_data_hex: Vec<u8>,
    /// Signatures, in order.
    pub signatures: Vec<Vec<u8>>,
    /// `ret` objects exactly as returned.
    pub ret: Vec<Value>,
    /// `raw_data` as returned (decoded contract parameters, for discovery).
    pub raw_data: Value,
}

/// A TRON block with its transactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronBlock {
    /// `blockID` reported by the endpoint.
    pub block_id: [u8; 32],
    /// Header fields.
    pub header: TronBlockHeader,
    /// Transactions, in block order.
    pub transactions: Vec<TronTransaction>,
}

/// A log of a TRON transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronLog {
    /// Emitting contract (20-byte EVM form).
    pub address: [u8; 20],
    /// Topics.
    pub topics: Vec<[u8; 32]>,
    /// Data.
    pub data: Vec<u8>,
}

/// `gettransactioninfobyid` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronTransactionInfo {
    /// Transaction id.
    pub id: [u8; 32],
    /// Block number.
    pub block_number: u64,
    /// Block timestamp in milliseconds.
    pub block_timestamp: u64,
    /// `contractResult` entries.
    pub contract_result: Vec<Vec<u8>>,
    /// Called contract, when a contract was called.
    pub contract_address: Option<[u8; 21]>,
    /// Logs, in order.
    pub logs: Vec<TronLog>,
    /// `result` (`FAILED` for failed transactions; absent on success).
    pub result: Option<String>,
    /// `resMessage` bytes, when present.
    pub res_message: Option<Vec<u8>>,
    /// `receipt` object as returned.
    pub receipt: Option<Value>,
    /// The whole object as returned.
    pub raw: Value,
}

/// `getcontractinfo` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronContractInfo {
    /// Deployed runtime code.
    pub runtime_code: Vec<u8>,
    /// `smart_contract` object as returned.
    pub smart_contract: Option<Value>,
    /// `contract_state` object as returned.
    pub contract_state: Option<Value>,
}

/// `triggerconstantcontract` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronConstantCall {
    /// `constant_result` entries (return data).
    pub constant_result: Vec<Vec<u8>>,
    /// `result.result`.
    pub result: bool,
    /// `result.code`, when present.
    pub code: Option<String>,
    /// `result.message` bytes (hex-decoded), when present.
    pub message: Option<Vec<u8>>,
    /// `energy_used`.
    pub energy_used: u64,
    /// `transaction.ret` objects as returned.
    pub transaction_ret: Vec<Value>,
}

impl TronConstantCall {
    /// Whether the call did not succeed: the node refused it, or the
    /// simulated transaction reverted.
    pub fn reverted(&self) -> bool {
        !self.result
            || self.code.is_some()
            || self.transaction_ret.iter().any(|ret| {
                ret.get("ret").and_then(Value::as_str) == Some("FAILED")
            })
    }
}

/// `broadcasthex` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TronBroadcast {
    /// Whether the node accepted the transaction.
    pub result: bool,
    /// `code` (`SUCCESS`, `SIGERROR`, …), when present.
    pub code: Option<String>,
    /// Sanitized `message`, when present and non-empty.
    pub message: Option<String>,
    /// `txid` computed by the node, when present.
    pub txid: Option<[u8; 32]>,
}

/// TRON HTTP API client.
#[derive(Debug)]
pub struct TronClient {
    transport: HttpTransport,
}

impl TronClient {
    /// A client over `transport`.
    pub fn new(transport: HttpTransport) -> Self {
        Self { transport }
    }

    /// The underlying transport.
    pub fn transport(&self) -> &HttpTransport {
        &self.transport
    }

    /// `POST /wallet/getnowblock`: the head block.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn now_block(&self) -> Result<TronBlock, RpcError> {
        parse_block(&self.call("/wallet/getnowblock", Map::new())?)
    }

    /// `POST /wallet/getblockbynum` with full transactions; `None` for an
    /// unknown block.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn block_by_num(&self, number: u64) -> Result<Option<TronBlock>, RpcError> {
        let mut body = Map::new();
        body.insert("num".to_owned(), Value::from(number));
        let value = self.call("/wallet/getblockbynum", body)?;
        if is_empty_object(&value) {
            return Ok(None);
        }
        parse_block(&value).map(Some)
    }

    /// `POST /wallet/getblockbylimitnext`: blocks `start..end` (end exclusive,
    /// at most [`MAX_BLOCKS_PER_RANGE`]), in number order.
    ///
    /// # Errors
    /// [`RpcError::InvalidRequest`] for an empty or oversized range, or any
    /// [`RpcError`].
    pub fn blocks_by_limit_next(&self, start: u64, end: u64) -> Result<Vec<TronBlock>, RpcError> {
        if start >= end || end - start > MAX_BLOCKS_PER_RANGE {
            return Err(RpcError::InvalidRequest(format!(
                "a TRON block range holds 1..={MAX_BLOCKS_PER_RANGE} blocks"
            )));
        }
        let mut body = Map::new();
        body.insert("startNum".to_owned(), Value::from(start));
        body.insert("endNum".to_owned(), Value::from(end));
        let value = self.call("/wallet/getblockbylimitnext", body)?;
        let map = expect_object(&value, "block range")?;
        let blocks = optional_array(map, "block", "block range")?
            .iter()
            .map(parse_block)
            .collect::<Result<Vec<_>, _>>()?;
        if u64::try_from(blocks.len()).map_or(true, |len| len > end - start) {
            return Err(invalid_response(
                "getblockbylimitnext returned more blocks than requested",
            ));
        }
        Ok(blocks)
    }

    /// `POST /walletsolidity/getnowblock`: the newest solidified block.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn solidity_now_block(&self) -> Result<TronBlock, RpcError> {
        parse_block(&self.call("/walletsolidity/getnowblock", Map::new())?)
    }

    /// `POST /walletsolidity/gettransactioninfobyid`; `None` while the
    /// transaction is unknown or not yet solidified.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn solidity_transaction_info(
        &self,
        txid: &[u8; 32],
    ) -> Result<Option<TronTransactionInfo>, RpcError> {
        let mut body = Map::new();
        body.insert("value".to_owned(), Value::from(hex::encode(txid)));
        let value = self.call("/walletsolidity/gettransactioninfobyid", body)?;
        if is_empty_object(&value) {
            return Ok(None);
        }
        parse_transaction_info(value).map(Some)
    }

    /// `POST /wallet/getcontractinfo`: the runtime code of `address`; `None`
    /// for an address without a contract.
    ///
    /// java-tron serves no `/walletsolidity/getcontractinfo` (it answers HTTP
    /// 405), so the code is read from the head state; SCCP contracts cannot
    /// change their code.
    ///
    /// # Errors
    /// Any [`RpcError`].
    pub fn contract_info(&self, address: &[u8; 21]) -> Result<Option<TronContractInfo>, RpcError> {
        let mut body = Map::new();
        body.insert("value".to_owned(), address_param(address, "the contract")?);
        let value = self.call("/wallet/getcontractinfo", body)?;
        if is_empty_object(&value) {
            return Ok(None);
        }
        let what = "contract info";
        let map = expect_object(&value, what)?;
        Ok(Some(TronContractInfo {
            runtime_code: tron_hex_field(map, "runtimecode", what)?,
            smart_contract: optional(map, "smart_contract").cloned(),
            contract_state: optional(map, "contract_state").cloned(),
        }))
    }

    /// `POST /wallet/triggerconstantcontract` of `contract` with full call
    /// `data` from `owner`.
    ///
    /// # Errors
    /// Any [`RpcError`]; a revert is reported through
    /// [`TronConstantCall::reverted`], not as an error.
    pub fn trigger_constant_contract(
        &self,
        owner: &[u8; 21],
        contract: &[u8; 21],
        data: &[u8],
    ) -> Result<TronConstantCall, RpcError> {
        let mut body = Map::new();
        body.insert("owner_address".to_owned(), address_param(owner, "the owner")?);
        body.insert(
            "contract_address".to_owned(),
            address_param(contract, "the contract")?,
        );
        body.insert("data".to_owned(), Value::from(hex::encode(data)));
        let value = self.call("/wallet/triggerconstantcontract", body)?;
        parse_constant_call(&value)
    }

    /// `POST /wallet/broadcasthex` of a signed `Transaction` protobuf.
    ///
    /// # Errors
    /// Any [`RpcError`]; a rejection is a [`TronBroadcast`] with
    /// `result == false`, and an unparsable transaction is an
    /// [`RpcError::Api`].
    pub fn broadcast_hex(&self, transaction: &[u8]) -> Result<TronBroadcast, RpcError> {
        if transaction.is_empty() {
            return Err(RpcError::InvalidRequest(
                "a TRON transaction must not be empty".to_owned(),
            ));
        }
        let mut body = Map::new();
        body.insert(
            "transaction".to_owned(),
            Value::from(hex::encode(transaction)),
        );
        let value = self.call("/wallet/broadcasthex", body)?;
        let what = "broadcast";
        let map = expect_object(&value, what)?;
        Ok(TronBroadcast {
            result: optional_bool(map, "result", what)?.unwrap_or(false),
            code: optional_str(map, "code", what)?.map(sanitize_message),
            message: optional_str(map, "message", what)?
                .filter(|message| !message.is_empty())
                .map(sanitize_message),
            txid: optional_str(map, "txid", what)?
                .map(|text| {
                    parse_tron_hex_array::<32>(text)
                        .map_err(|error| field_error(what, "txid", error))
                })
                .transpose()?,
        })
    }

    fn call(&self, path: &str, body: Map) -> Result<Value, RpcError> {
        let body = encode_json(&Value::Object(body))?;
        let response = self
            .transport
            .post(path, MEDIA_TYPE_JSON, &body, MEDIA_TYPE_JSON)?;
        let value = response.json()?;
        if let Some(message) = value
            .as_object()
            .and_then(|map| map.get("Error"))
            .and_then(Value::as_str)
        {
            return Err(RpcError::Api {
                endpoint: response.endpoint,
                message: sanitize_message(message),
            });
        }
        Ok(value)
    }
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().is_some_and(Map::is_empty)
}

fn field_error(what: &str, key: &str, error: HexError) -> RpcError {
    invalid_response(format!("{what}.{key}: {error}"))
}

fn tron_hex_field(map: &Map, key: &str, what: &str) -> Result<Vec<u8>, RpcError> {
    parse_tron_hex(required_str(map, key, what)?).map_err(|error| field_error(what, key, error))
}

fn optional_tron_hex(map: &Map, key: &str, what: &str) -> Result<Option<Vec<u8>>, RpcError> {
    optional_str(map, key, what)?
        .map(|text| parse_tron_hex(text).map_err(|error| field_error(what, key, error)))
        .transpose()
}

fn tron_hex_array_field<const N: usize>(
    map: &Map,
    key: &str,
    what: &str,
) -> Result<[u8; N], RpcError> {
    parse_tron_hex_array::<N>(required_str(map, key, what)?)
        .map_err(|error| field_error(what, key, error))
}

fn tron_hex_list<const N: usize>(values: &[Value], what: &str) -> Result<Vec<[u8; N]>, RpcError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = value
                .as_str()
                .ok_or_else(|| invalid_response(format!("{what}[{index}] is not a string")))?;
            parse_tron_hex_array::<N>(text)
                .map_err(|error| invalid_response(format!("{what}[{index}]: {error}")))
        })
        .collect()
}

fn tron_bytes_list(values: &[Value], what: &str) -> Result<Vec<Vec<u8>>, RpcError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let text = value
                .as_str()
                .ok_or_else(|| invalid_response(format!("{what}[{index}] is not a string")))?;
            parse_tron_hex(text)
                .map_err(|error| invalid_response(format!("{what}[{index}]: {error}")))
        })
        .collect()
}

/// A non-negative integer member printed as a JSON number; absent is zero.
fn number_or_zero(map: &Map, key: &str, what: &str) -> Result<u64, RpcError> {
    optional(map, key).map_or(Ok(0), |value| {
        value
            .as_u64()
            .ok_or_else(|| invalid_response(format!("{what}.{key} is not a non-negative integer")))
    })
}

fn optional_bool(map: &Map, key: &str, what: &str) -> Result<Option<bool>, RpcError> {
    optional(map, key)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| invalid_response(format!("{what}.{key} is not a boolean")))
        })
        .transpose()
}

fn parse_block(value: &Value) -> Result<TronBlock, RpcError> {
    let what = "block";
    let map = expect_object(value, what)?;
    let header = expect_object(required(map, "block_header", what)?, "block_header")?;
    let raw = expect_object(
        required(header, "raw_data", "block_header")?,
        "block_header.raw_data",
    )?;
    let raw_what = "block_header.raw_data";
    let witness_address = tron_hex_array_field::<21>(raw, "witness_address", raw_what)?;
    if witness_address[0] != TRON_ADDRESS_PREFIX {
        return Err(invalid_response(
            "block_header.raw_data.witness_address lacks the 0x41 prefix",
        ));
    }
    let version = u32::try_from(number_or_zero(raw, "version", raw_what)?)
        .map_err(|_| invalid_response("block_header.raw_data.version exceeds 32 bits"))?;
    let header = TronBlockHeader {
        number: number_or_zero(raw, "number", raw_what)?,
        timestamp: number_or_zero(raw, "timestamp", raw_what)?,
        tx_trie_root: tron_hex_array_field::<32>(raw, "txTrieRoot", raw_what)?,
        parent_hash: tron_hex_array_field::<32>(raw, "parentHash", raw_what)?,
        witness_address,
        witness_id: number_or_zero(raw, "witness_id", raw_what)?,
        version,
        account_state_root: optional_tron_hex(raw, "accountStateRoot", raw_what)?
            .unwrap_or_default(),
        witness_signature: optional_tron_hex(header, "witness_signature", "block_header")?
            .unwrap_or_default(),
    };
    let transactions = optional_array(map, "transactions", what)?
        .iter()
        .map(parse_transaction)
        .collect::<Result<_, _>>()?;
    Ok(TronBlock {
        block_id: tron_hex_array_field::<32>(map, "blockID", what)?,
        header,
        transactions,
    })
}

fn parse_transaction(value: &Value) -> Result<TronTransaction, RpcError> {
    let what = "transaction";
    let map = expect_object(value, what)?;
    let raw_data = required(map, "raw_data", what)?;
    expect_object(raw_data, "transaction.raw_data")?;
    let ret = optional_array(map, "ret", what)?;
    if let Some(index) = ret.iter().position(|entry| !entry.is_object()) {
        return Err(invalid_response(format!(
            "transaction.ret[{index}] is not an object"
        )));
    }
    Ok(TronTransaction {
        tx_id: tron_hex_array_field::<32>(map, "txID", what)?,
        raw_data_hex: tron_hex_field(map, "raw_data_hex", what)?,
        signatures: tron_bytes_list(
            optional_array(map, "signature", what)?,
            "transaction.signature",
        )?,
        ret: ret.to_vec(),
        raw_data: raw_data.clone(),
    })
}

fn parse_transaction_info(value: Value) -> Result<TronTransactionInfo, RpcError> {
    let what = "transaction info";
    let map = expect_object(&value, what)?;
    let logs = optional_array(map, "log", what)?
        .iter()
        .map(|log| {
            let what = "transaction info.log";
            let log = expect_object(log, what)?;
            Ok(TronLog {
                address: tron_hex_array_field::<20>(log, "address", what)?,
                topics: tron_hex_list::<32>(
                    optional_array(log, "topics", what)?,
                    "transaction info.log.topics",
                )?,
                data: optional_tron_hex(log, "data", what)?.unwrap_or_default(),
            })
        })
        .collect::<Result<_, RpcError>>()?;
    let contract_address = optional_str(map, "contract_address", what)?
        .map(|text| {
            parse_tron_address(text).map_err(|error| field_error(what, "contract_address", error))
        })
        .transpose()?;
    let info = TronTransactionInfo {
        id: tron_hex_array_field::<32>(map, "id", what)?,
        block_number: number_or_zero(map, "blockNumber", what)?,
        block_timestamp: number_or_zero(map, "blockTimeStamp", what)?,
        contract_result: tron_bytes_list(
            optional_array(map, "contractResult", what)?,
            "transaction info.contractResult",
        )?,
        contract_address,
        logs,
        result: optional_str(map, "result", what)?.map(sanitize_message),
        res_message: optional_tron_hex(map, "resMessage", what)?,
        receipt: optional(map, "receipt").cloned(),
        raw: Value::Null,
    };
    Ok(TronTransactionInfo { raw: value, ..info })
}

fn parse_constant_call(value: &Value) -> Result<TronConstantCall, RpcError> {
    let what = "constant call";
    let map = expect_object(value, what)?;
    let result = expect_object(required(map, "result", what)?, "constant call.result")?;
    let result_what = "constant call.result";
    let transaction_ret = match optional(map, "transaction") {
        None => Vec::new(),
        Some(transaction) => {
            let transaction = expect_object(transaction, "constant call.transaction")?;
            optional_array(transaction, "ret", "constant call.transaction")?.to_vec()
        }
    };
    Ok(TronConstantCall {
        constant_result: tron_bytes_list(
            optional_array(map, "constant_result", what)?,
            "constant call.constant_result",
        )?,
        result: optional_bool(result, "result", result_what)?.unwrap_or(false),
        code: optional_str(result, "code", result_what)?.map(sanitize_message),
        message: optional_tron_hex(result, "message", result_what)?,
        energy_used: number_or_zero(map, "energy_used", what)?,
        transaction_ret,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Value {
        norito::json::parse_value(text).expect("test JSON")
    }

    #[test]
    fn tron_hex_is_unprefixed_and_even() {
        assert_eq!(parse_tron_hex(""), Ok(Vec::new()));
        assert_eq!(parse_tron_hex("00ff"), Ok(vec![0, 0xff]));
        assert_eq!(parse_tron_hex("0x00"), Err(HexError::InvalidDigit));
        assert_eq!(parse_tron_hex("abc"), Err(HexError::OddLength));
        assert_eq!(parse_tron_hex("zz"), Err(HexError::InvalidDigit));
        assert_eq!(parse_tron_hex(" 00"), Err(HexError::InvalidDigit));
        assert_eq!(parse_tron_hex_array::<2>("0102"), Ok([1, 2]));
        assert_eq!(
            parse_tron_hex_array::<3>("0102"),
            Err(HexError::WrongLength {
                expected: 3,
                found: 2
            })
        );
        let usdt = "41a614f803b6fd780986a42c78ec9c7f77e6ded13c";
        assert_eq!(parse_tron_address(usdt).expect("address")[0], 0x41);
        assert!(parse_tron_address("a614f803b6fd780986a42c78ec9c7f77e6ded13c").is_err());
        assert!(parse_tron_address(&format!("42{}", &usdt[2..])).is_err());
    }

    #[test]
    fn address_parameters_require_the_tron_prefix() {
        let mut address = [0_u8; 21];
        assert!(address_param(&address, "x").is_err());
        address[0] = TRON_ADDRESS_PREFIX;
        assert_eq!(
            address_param(&address, "x").expect("address"),
            Value::from(format!("41{}", "00".repeat(20)))
        );
    }

    fn block_json(extra_raw: &str) -> String {
        format!(
            r#"{{"blockID":"{h}","block_header":{{"raw_data":{{"number":5,"txTrieRoot":"{h}","witness_address":"41{a}","parentHash":"{h}"{extra_raw}}},"witness_signature":"{s}"}},"transactions":[{{"ret":[{{"contractRet":"SUCCESS"}}],"signature":["{s}"],"txID":"{h}","raw_data":{{"contract":[]}},"raw_data_hex":"0a02"}}]}}"#,
            h = "11".repeat(32),
            a = "22".repeat(20),
            s = "33".repeat(65),
        )
    }

    #[test]
    fn blocks_keep_header_fields_and_transaction_bytes() {
        let block = parse_block(&parse(&block_json(r#","version":37,"timestamp":1000"#)))
            .expect("block");
        assert_eq!(block.header.number, 5);
        assert_eq!(block.header.version, 37);
        assert_eq!(block.header.timestamp, 1000);
        assert_eq!(block.header.witness_id, 0);
        assert!(block.header.account_state_root.is_empty());
        assert_eq!(block.header.witness_signature.len(), 65);
        assert_eq!(block.transactions.len(), 1);
        assert_eq!(block.transactions[0].raw_data_hex, vec![0x0a, 0x02]);
        assert_eq!(block.transactions[0].signatures[0].len(), 65);
        assert_eq!(
            block.transactions[0].ret,
            vec![parse(r#"{"contractRet":"SUCCESS"}"#)]
        );
        assert!(parse_block(&parse(&block_json(r#","version":-1"#))).is_err());
        assert!(parse_block(&parse(&block_json(r#","witness_id":1.5"#))).is_err());
        let bad_prefix = block_json("").replace(&format!("41{}", "22".repeat(20)), &"22".repeat(21));
        assert!(parse_block(&parse(&bad_prefix)).is_err());
        let bad_ret = block_json("").replace(r#"[{"contractRet":"SUCCESS"}]"#, r#"["SUCCESS"]"#);
        assert!(parse_block(&parse(&bad_ret)).is_err());
    }

    #[test]
    fn constant_calls_report_reverts() {
        let ok = parse_constant_call(&parse(
            r#"{"result":{"result":true},"constant_result":["0006"],"energy_used":5,"transaction":{"ret":[{}]}}"#,
        ))
        .expect("call");
        assert!(!ok.reverted());
        assert_eq!(ok.constant_result, vec![vec![0, 6]]);
        assert_eq!(ok.energy_used, 5);
        let reverted = parse_constant_call(&parse(
            r#"{"result":{"result":true,"message":"5245"},"constant_result":[""],"transaction":{"ret":[{"ret":"FAILED"}]}}"#,
        ))
        .expect("call");
        assert!(reverted.reverted());
        assert_eq!(reverted.message, Some(b"RE".to_vec()));
        assert_eq!(reverted.constant_result, vec![Vec::<u8>::new()]);
        let refused = parse_constant_call(&parse(
            r#"{"result":{"code":"OTHER_ERROR","message":"6f"}}"#,
        ))
        .expect("call");
        assert!(refused.reverted());
        assert_eq!(refused.code.as_deref(), Some("OTHER_ERROR"));
        assert!(parse_constant_call(&parse(r#"{"constant_result":[]}"#)).is_err());
    }

    #[test]
    fn transaction_info_parses_logs_and_failures() {
        let info = parse_transaction_info(parse(&format!(
            r#"{{"id":"{h}","blockNumber":7,"blockTimeStamp":9,"contractResult":[""],"contract_address":"41{a}","log":[{{"address":"{a}","topics":["{h}"]}}],"result":"FAILED","resMessage":"6f6b","receipt":{{"result":"REVERT"}}}}"#,
            h = "11".repeat(32),
            a = "22".repeat(20),
        )))
        .expect("info");
        assert_eq!(info.block_number, 7);
        assert_eq!(info.logs[0].address, [0x22; 20]);
        assert!(info.logs[0].data.is_empty());
        assert_eq!(info.result.as_deref(), Some("FAILED"));
        assert_eq!(info.res_message, Some(b"ok".to_vec()));
        assert!(info.receipt.is_some());
        assert!(info.raw.is_object());
        assert!(parse_transaction_info(parse(r#"{"id":"11"}"#)).is_err());
    }

    #[test]
    fn empty_objects_mean_unknown() {
        assert!(is_empty_object(&parse("{}")));
        assert!(!is_empty_object(&parse(r#"{"a":1}"#)));
        assert!(!is_empty_object(&parse("[]")));
    }
}
