//! Pluggable schema registry used by hosts to encode/decode typed Norito payloads.
//!
//! Schemas are resolved only through explicitly registered names and families.
//! Unsupported names fail closed; registered schemas expose a stable 32-byte id
//! and version for host metadata.
use std::sync::Arc;
use iroha_crypto::Hash as IrohaHash;
use iroha_data_model::query::{
    QueryRequest, QueryResponse,
    json_wrappers::{QueryRequestJson, query_request_from_json, query_request_to_json},
};
use ivm_abi::codec::{decode_canonical_norito, encode_canonical_norito};
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
    /// Resolve a canonical family name from either that family or an exact schema name.
    fn resolve_family(&self, name: &str) -> Option<String>;
    /// Encode a JSON payload according to `name` into canonical V1 Norito bytes.
    fn encode_json(&self, name: &str, json: &[u8]) -> Option<Vec<u8>>;
    /// Decode canonical V1 Norito bytes according to `name` into minified JSON bytes.
    fn decode_to_json(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>>;
    /// Return all known versions for a base schema name (e.g., "Order").
    fn list_versions(&self, base: &str) -> Option<Vec<(String, SchemaInfo)>>;
    /// Return the canonical current version for a base name.
    fn current(&self, base: &str) -> Option<(String, SchemaInfo)>;
}
impl<T: SchemaRegistry + ?Sized> SchemaRegistry for Arc<T> {
    fn info(&self, name: &str) -> Option<SchemaInfo> {
        (**self).info(name)
    }
    fn resolve_family(&self, name: &str) -> Option<String> {
        (**self).resolve_family(name)
    }
    fn encode_json(&self, name: &str, json: &[u8]) -> Option<Vec<u8>> {
        (**self).encode_json(name, json)
    }
    fn decode_to_json(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        (**self).decode_to_json(name, bytes)
    }
    fn list_versions(&self, base: &str) -> Option<Vec<(String, SchemaInfo)>> {
        (**self).list_versions(base)
    }
    fn current(&self, base: &str) -> Option<(String, SchemaInfo)> {
        (**self).current(base)
    }
}
/// Production default registry for the host schema syscalls.
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
    fn exact_object<'a>(
        value: &'a norito::json::Value,
        expected_fields: &[&str],
    ) -> Option<&'a norito::json::Map> {
        let object = value.as_object()?;
        (object.len() == expected_fields.len()
            && expected_fields
                .iter()
                .all(|field| object.contains_key(*field)))
        .then_some(object)
    }
}
impl SchemaRegistry for DefaultRegistry {
    fn info(&self, name: &str) -> Option<SchemaInfo> {
        match name {
            "Order" => Some(self.info_order()),
            "OrderByTime" => Some(self.info_order_by_time()),
            "TradeV1" => Some(self.info_trade_v1()),
            "TradeV2" => Some(self.info_trade_v2()),
            "QueryRequest" => Some(self.info_query_request()),
            "QueryResponse" => Some(self.info_query_response()),
            _ => None,
        }
    }
    fn resolve_family(&self, name: &str) -> Option<String> {
        match name {
            "Order" | "OrderByTime" => Some("Order".to_owned()),
            "Trade" | "TradeV1" | "TradeV2" => Some("Trade".to_owned()),
            "QueryRequest" => Some("QueryRequest".to_owned()),
            "QueryResponse" => Some("QueryResponse".to_owned()),
            _ => None,
        }
    }
    fn encode_json(&self, name: &str, json: &[u8]) -> Option<Vec<u8>> {
        match name {
            "Order" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let object = Self::exact_object(&v, &["qty", "side"])?;
                let qty = object.get("qty")?.as_i64()?;
                let side = object.get("side")?.as_str()?.to_string();
                let order = OrderSchema { qty, side };
                encode_canonical_norito(&order).ok()
            }
            "OrderByTime" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let object = Self::exact_object(&v, &["qty", "side", "tif"])?;
                let qty = object.get("qty")?.as_i64()?;
                let side = object.get("side")?.as_str()?.to_string();
                let tif = u32::try_from(object.get("tif")?.as_u64()?).ok()?;
                let order = OrderByTimeSchema { qty, side, tif };
                encode_canonical_norito(&order).ok()
            }
            "TradeV1" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let object = Self::exact_object(&v, &["price", "qty", "side"])?;
                let qty = object.get("qty")?.as_i64()?;
                let price = object.get("price")?.as_i64()?;
                let side = object.get("side")?.as_str()?.to_string();
                let t = TradeV1Schema { qty, price, side };
                encode_canonical_norito(&t).ok()
            }
            "TradeV2" => {
                let v: norito::json::Value = norito::json::from_slice(json).ok()?;
                let object = Self::exact_object(&v, &["price", "qty", "side", "venue"])?;
                let qty = object.get("qty")?.as_i64()?;
                let price = object.get("price")?.as_i64()?;
                let side = object.get("side")?.as_str()?.to_string();
                let venue = object.get("venue")?.as_str()?.to_string();
                let t = TradeV2Schema {
                    qty,
                    price,
                    side,
                    venue,
                };
                encode_canonical_norito(&t).ok()
            }
            "QueryRequest" => {
                let req_json: QueryRequestJson = norito::json::from_slice(json).ok()?;
                let req = query_request_from_json(req_json).ok()?;
                encode_canonical_norito(&req).ok()
            }
            "QueryResponse" => {
                let resp: QueryResponse = norito::json::from_slice(json).ok()?;
                encode_canonical_norito(&resp).ok()
            }
            _ => None,
        }
    }
    fn decode_to_json(&self, name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        match name {
            "Order" => {
                let o: OrderSchema = decode_canonical_norito(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(o.qty));
                map.insert("side".to_owned(), norito::json::Value::from(o.side));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "OrderByTime" => {
                let o: OrderByTimeSchema = decode_canonical_norito(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(o.qty));
                map.insert("side".to_owned(), norito::json::Value::from(o.side));
                map.insert("tif".to_owned(), norito::json::Value::from(o.tif));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "TradeV1" => {
                let t: TradeV1Schema = decode_canonical_norito(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(t.qty));
                map.insert("price".to_owned(), norito::json::Value::from(t.price));
                map.insert("side".to_owned(), norito::json::Value::from(t.side));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "TradeV2" => {
                let t: TradeV2Schema = decode_canonical_norito(bytes).ok()?;
                let mut map = norito::json::Map::new();
                map.insert("qty".to_owned(), norito::json::Value::from(t.qty));
                map.insert("price".to_owned(), norito::json::Value::from(t.price));
                map.insert("side".to_owned(), norito::json::Value::from(t.side));
                map.insert("venue".to_owned(), norito::json::Value::from(t.venue));
                norito::json::to_vec(&norito::json::Value::Object(map)).ok()
            }
            "QueryRequest" => {
                let req: QueryRequest = decode_canonical_norito(bytes).ok()?;
                let req_json = query_request_to_json(&req);
                norito::json::to_vec(&req_json).ok()
            }
            "QueryResponse" => {
                let resp: QueryResponse = decode_canonical_norito(bytes).ok()?;
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
    fn registry_codec_is_ambient_independent_and_rejects_alternate_layout() {
        let reg = DefaultRegistry::new();
        let input = br#"{"qty":10,"side":"canonical"}"#;
        let canonical = reg
            .encode_json("Order", input)
            .expect("encode canonical Order");
        let value = OrderSchema {
            qty: 10,
            side: "canonical".to_owned(),
        };
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let alternate = norito::to_bytes(&value).expect("encode alternate-layout Order");
        assert_ne!(
            alternate, canonical,
            "fixture must distinguish alternate and canonical V1 layouts"
        );
        assert_eq!(
            reg.encode_json("Order", input),
            Some(canonical.clone()),
            "registry output must ignore ambient layout flags"
        );
        assert!(
            reg.decode_to_json("Order", &alternate).is_none(),
            "registry input must reject an ordinarily decodable alternate layout"
        );
        let decoded = reg
            .decode_to_json("Order", &canonical)
            .expect("decode canonical Order under alternate ambient flags");
        assert!(eq_json(input, &decoded));
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
    fn order_by_time_accepts_maximum_tif() {
        let reg = DefaultRegistry::new();
        let input = format!(r#"{{"qty":5,"side":"sell","tif":{}}}"#, u32::MAX);
        let enc = reg
            .encode_json("OrderByTime", input.as_bytes())
            .expect("encode maximum tif");
        let dec = reg
            .decode_to_json("OrderByTime", &enc)
            .expect("decode maximum tif");
        let value: njson::Value = njson::from_slice(&dec).expect("parse decoded JSON");
        assert_eq!(value["tif"].as_u64(), Some(u64::from(u32::MAX)));
    }
    #[test]
    fn order_by_time_rejects_tif_outside_u32() {
        let reg = DefaultRegistry::new();
        assert!(
            reg.encode_json(
                "OrderByTime",
                br#"{"qty":5,"side":"sell","tif":4294967296}"#,
            )
            .is_none()
        );
    }
    #[test]
    fn registry_metadata_is_coherent_for_base_and_exact_names() {
        let reg = DefaultRegistry::new();
        let families: [(&str, &[&str], &str); 4] = [
            ("Order", &["Order", "OrderByTime"], "OrderByTime"),
            ("Trade", &["TradeV1", "TradeV2"], "TradeV2"),
            ("QueryRequest", &["QueryRequest"], "QueryRequest"),
            ("QueryResponse", &["QueryResponse"], "QueryResponse"),
        ];
        for (family, exact_names, expected_current) in families {
            assert_eq!(reg.resolve_family(family).as_deref(), Some(family));
            let versions = reg.list_versions(family).expect("known family versions");
            assert_eq!(versions.len(), exact_names.len());
            for exact_name in exact_names {
                assert_eq!(
                    reg.resolve_family(exact_name).as_deref(),
                    Some(family),
                    "family for {exact_name}"
                );
                let listed = versions
                    .iter()
                    .find(|(name, _)| name == exact_name)
                    .expect("exact schema is listed");
                assert_eq!(reg.info(exact_name), Some(listed.1));
            }
            let current = reg.current(family).expect("known family current");
            assert_eq!(current.0, expected_current);
            assert_eq!(reg.info(&current.0), Some(current.1));
            assert!(versions.contains(&current));
        }
        for unknown in ["UnknownSchema", "OrderV2", "TradeV3", "Query"] {
            assert_eq!(reg.resolve_family(unknown), None);
            assert_eq!(reg.info(unknown), None);
            assert_eq!(reg.list_versions(unknown), None);
            assert_eq!(reg.current(unknown), None);
        }
    }
    #[test]
    fn manual_schema_encoders_require_exact_json_shapes() {
        let reg = DefaultRegistry::new();
        let invalid: [(&str, &str, &[u8]); 9] = [
            (
                "Order extra field",
                "Order",
                &br#"{"qty":5,"side":"buy","tif":30}"#[..],
            ),
            (
                "OrderByTime extra field",
                "OrderByTime",
                &br#"{"qty":5,"side":"buy","tif":30,"price":7}"#[..],
            ),
            (
                "TradeV1 extra field",
                "TradeV1",
                &br#"{"qty":5,"price":7,"side":"buy","venue":"X"}"#[..],
            ),
            (
                "TradeV2 extra field",
                "TradeV2",
                &br#"{"qty":5,"price":7,"side":"buy","venue":"X","tif":30}"#[..],
            ),
            (
                "duplicate field",
                "Order",
                &br#"{"qty":5,"qty":6,"side":"buy"}"#[..],
            ),
            (
                "wrong qty type",
                "Order",
                &br#"{"qty":"5","side":"buy"}"#[..],
            ),
            (
                "wrong tif type",
                "OrderByTime",
                &br#"{"qty":5,"side":"buy","tif":"30"}"#[..],
            ),
            (
                "wrong price type",
                "TradeV1",
                &br#"{"qty":5,"price":"7","side":"buy"}"#[..],
            ),
            (
                "wrong venue type",
                "TradeV2",
                &br#"{"qty":5,"price":7,"side":"buy","venue":9}"#[..],
            ),
        ];
        for (label, schema, json) in invalid {
            assert!(
                reg.encode_json(schema, json).is_none(),
                "{label} must fail closed"
            );
        }
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
