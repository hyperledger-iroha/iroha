//! Simple pluggable schema registry used by hosts to encode/decode
//! typed Norito payloads.
//!
//! This is a development scaffold: schemas are identified by a string name and
//! return a stable 32-byte id (hash) and a simple version. Encoding/decoding
//! is currently implemented for a couple of example types.

use iroha_crypto::Hash as IrohaHash;
use iroha_data_model::query::{
    QueryRequest, QueryResponse,
    json_wrappers::{QueryRequestJson, query_request_from_json, query_request_to_json},
};

// Canonical schema type definitions used by the default registry for encoding/decoding.
// Keep these at module scope so Norito type identity remains stable across encode/decode.
#[derive(norito::Decode, norito::Encode, Clone, Debug)]
struct OrderSchema {
    qty: i64,
    side: String,
}

#[derive(norito::Decode, norito::Encode, Clone, Debug)]
struct OrderByTimeSchema {
    qty: i64,
    side: String,
    tif: u32,
}

#[derive(norito::Decode, norito::Encode, Clone, Debug)]
struct TradeV1Schema {
    qty: i64,
    price: i64,
    side: String,
}

#[derive(norito::Decode, norito::Encode, Clone, Debug)]
struct TradeV2Schema {
    qty: i64,
    price: i64,
    side: String,
    venue: String,
}

/// Public schema info (id + version).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaInfo {
    pub id: [u8; 32],
    pub version: u16,
}

/// Registry interface used by hosts.
pub trait SchemaRegistry {
    /// Return schema info for the given name.
    fn info(&self, name: &str) -> Option<SchemaInfo>;
    /// Encode a JSON payload according to `name` into Norito bytes.
    fn encode_json(&self, name: &str, json: &[u8]) -> Option<Vec<u8>>;
    /// Decode Norito bytes according to `name` into minified JSON bytes.
    fn decode_to_json(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>>;
    /// Return all known versions for a base schema name (e.g., "Order").
    fn list_versions(&self, base: &str) -> Option<Vec<(String, SchemaInfo)>>;
    /// Return the canonical current version for a base name.
    fn current(&self, base: &str) -> Option<(String, SchemaInfo)>;
}

/// Default in-memory registry with a couple of example schemas.
#[derive(Default)]
pub struct DefaultRegistry;

impl DefaultRegistry {
    pub fn new() -> Self {
        Self
    }

    fn info_order(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"Order@1").into(),
            version: 1,
        }
    }
    fn info_trade_v1(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"TradeV1@1").into(),
            version: 1,
        }
    }
    fn info_order_by_time(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"OrderByTime@2").into(),
            version: 2,
        }
    }
    fn info_trade_v2(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"TradeV2@2").into(),
            version: 2,
        }
    }
    fn info_query_request(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"QueryRequest@1").into(),
            version: 1,
        }
    }
    fn info_query_response(&self) -> SchemaInfo {
        SchemaInfo {
            id: IrohaHash::new(b"QueryResponse@1").into(),
            version: 1,
        }
    }
}

impl SchemaRegistry for DefaultRegistry {
    fn info(&self, name: &str) -> Option<SchemaInfo> {
        match name {
            "Order" => Some(self.info_order()),
            "OrderByTime" => Some(self.info_order_by_time()),
            "TradeV1" => Some(self.info_trade_v1()),
            "TradeV2" => Some(self.info_trade_v2()),
            _ => None,
        }
    }

    fn encode_json(&self, name: &str, json: &[u8]) -> Option<Vec<u8>> {
        match name {
            "Order" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let qty = v.get("qty")?.as_i64()?;
                let side = v.get("side")?.as_str()?.to_string();
                let order = OrderSchema { qty, side };
                norito::to_bytes(&order).ok()
            }
            "OrderByTime" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let qty = v.get("qty")?.as_i64()?;
                let side = v.get("side")?.as_str()?.to_string();
                let tif = v.get("tif")?.as_u64()? as u32;
                let order = OrderByTimeSchema { qty, side, tif };
                norito::to_bytes(&order).ok()
            }
            "TradeV1" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let qty = v.get("qty")?.as_i64()?;
                let price = v.get("price")?.as_i64()?;
                let side = v.get("side")?.as_str()?.to_string();
                let t = TradeV1Schema { qty, price, side };
                norito::to_bytes(&t).ok()
            }
            "TradeV2" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let qty = v.get("qty")?.as_i64()?;
                let price = v.get("price")?.as_i64()?;
                let side = v.get("side")?.as_str()?.to_string();
                let venue = v.get("venue")?.as_str()?.to_string();
                let t = TradeV2Schema {
                    qty,
                    price,
                    side,
                    venue,
                };
                norito::to_bytes(&t).ok()
            }
            "QueryRequest" => {
                let req_json: QueryRequestJson = norito::json::from_slice(json).ok()?;
                let req = query_request_from_json(req_json).ok()?;
                norito::to_bytes(&req).ok()
            }
            "QueryResponse" => {
                let resp: QueryResponse = norito::json::from_slice(json).ok()?;
                norito::to_bytes(&resp).ok()
            }
            _ => None,
        }
    }

    fn decode_to_json(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        match name {
            "Order" => {
                let o: OrderSchema = norito::decode_from_bytes(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(o.qty));
                map.insert("side".to_owned(), norito::json::Value::from(o.side));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "OrderByTime" => {
                let o: OrderByTimeSchema = norito::decode_from_bytes(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(o.qty));
                map.insert("side".to_owned(), norito::json::Value::from(o.side));
                map.insert("tif".to_owned(), norito::json::Value::from(o.tif));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "TradeV1" => {
                let t: TradeV1Schema = norito::decode_from_bytes(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(t.qty));
                map.insert("price".to_owned(), norito::json::Value::from(t.price));
                map.insert("side".to_owned(), norito::json::Value::from(t.side));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "TradeV2" => {
                let t: TradeV2Schema = norito::decode_from_bytes(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(t.qty));
                map.insert("price".to_owned(), norito::json::Value::from(t.price));
                map.insert("side".to_owned(), norito::json::Value::from(t.side));
                map.insert("venue".to_owned(), norito::json::Value::from(t.venue));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "QueryRequest" => {
                let req: QueryRequest = norito::decode_from_bytes(bytes).ok()?;
                let req_json = query_request_to_json(&req);
                norito::json::to_vec(&req_json).ok()
            }
            "QueryResponse" => {
                let resp: QueryResponse = norito::decode_from_bytes(bytes).ok()?;
                norito::json::to_vec(&resp).ok()
            }
            _ => None,
        }
    }

    fn list_versions(&self, base: &str) -> Option<Vec<(String, SchemaInfo)>> {
        let mut out = Vec::new();
        match base {
            "Order" => {
                out.push(("Order".to_string(), self.info_order()));
                out.push(("OrderByTime".to_string(), self.info_order_by_time()));
            }
            "Trade" => {
                out.push(("TradeV1".to_string(), self.info_trade_v1()));
                out.push(("TradeV2".to_string(), self.info_trade_v2()));
            }
            "QueryRequest" => {
                out.push(("QueryRequest".to_string(), self.info_query_request()));
            }
            "QueryResponse" => {
                out.push(("QueryResponse".to_string(), self.info_query_response()));
            }
            _ => return None,
        }
        Some(out)
    }

    fn current(&self, base: &str) -> Option<(String, SchemaInfo)> {
        match base {
            "Order" => Some(("OrderByTime".to_string(), self.info_order_by_time())),
            "Trade" => Some(("TradeV2".to_string(), self.info_trade_v2())),
            "QueryRequest" => Some(("QueryRequest".to_string(), self.info_query_request())),
            "QueryResponse" => Some(("QueryResponse".to_string(), self.info_query_response())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use norito::json as njson;

    use super::*;
    use iroha_data_model::query::{
        QueryRequest, QueryResponse, SingularQueryBox, SingularQueryOutputBox,
        executor::prelude::FindParameters, runtime::AbiVersion,
    };

    fn eq_json(a: &[u8], b: &[u8]) -> bool {
        let va: njson::Value = match njson::from_slice(a) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let vb: njson::Value = match njson::from_slice(b) {
            Ok(v) => v,
            Err(_) => return false,
        };
        va == vb
    }

    #[test]
    fn order_roundtrip() {
        let reg = DefaultRegistry::new();
        let input = br#"{"qty":10, "side":"buy"}"#;
        let enc = reg.encode_json("Order", input).expect("encode");
        let dec = reg.decode_to_json("Order", &enc).expect("decode to json");
        assert!(eq_json(input, &dec));
    }

    #[test]
    fn order_by_time_roundtrip() {
        let reg = DefaultRegistry::new();
        let input = br#"{"qty":5, "side":"sell", "tif": 30}"#;
        let enc = reg.encode_json("OrderByTime", input).expect("encode");
        let dec = reg
            .decode_to_json("OrderByTime", &enc)
            .expect("decode to json");
        assert!(eq_json(input, &dec));
    }

    #[test]
    fn trade_v1_roundtrip_fields() {
        let reg = DefaultRegistry::new();
        let input = br#"{"qty":7, "price": 1001, "side":"buy"}"#;
        let enc = reg.encode_json("TradeV1", input).expect("encode");
        let dec = reg.decode_to_json("TradeV1", &enc).expect("decode to json");
        let v: njson::Value = njson::from_slice(&dec).expect("parse json");
        assert_eq!(v["qty"].as_i64().unwrap(), 7);
        assert_eq!(v["price"].as_i64().unwrap(), 1001);
        assert_eq!(v["side"].as_str().unwrap(), "buy");
    }

    #[test]
    fn trade_v2_roundtrip_fields() {
        let reg = DefaultRegistry::new();
        let input = br#"{"qty":3, "price": 42, "side":"sell", "venue":"X"}"#;
        let enc = reg.encode_json("TradeV2", input).expect("encode");
        let dec = reg.decode_to_json("TradeV2", &enc).expect("decode to json");
        let v: njson::Value = njson::from_slice(&dec).expect("parse json");
        assert_eq!(v["qty"].as_i64().unwrap(), 3);
        assert_eq!(v["price"].as_i64().unwrap(), 42);
        assert_eq!(v["side"].as_str().unwrap(), "sell");
        assert_eq!(v["venue"].as_str().unwrap(), "X");
    }

    #[test]
    fn query_request_roundtrip() {
        let reg = DefaultRegistry::new();
        let req = QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters));
        let json = query_request_to_json(&req);
        let json_bytes = njson::to_vec(&json).expect("serialize json");
        let enc = reg
            .encode_json("QueryRequest", &json_bytes)
            .expect("encode");
        let dec = reg
            .decode_to_json("QueryRequest", &enc)
            .expect("decode to json");
        assert!(eq_json(&json_bytes, &dec));
    }

    #[test]
    fn query_response_roundtrip() {
        let reg = DefaultRegistry::new();
        let resp = QueryResponse::Singular(SingularQueryOutputBox::AbiVersion(AbiVersion {
            abi_version: 1,
        }));
        let json_bytes = njson::to_vec(&resp).expect("serialize json");
        let enc = reg
            .encode_json("QueryResponse", &json_bytes)
            .expect("encode");
        let dec = reg
            .decode_to_json("QueryResponse", &enc)
            .expect("decode to json");
        assert!(eq_json(&json_bytes, &dec));
    }
}
