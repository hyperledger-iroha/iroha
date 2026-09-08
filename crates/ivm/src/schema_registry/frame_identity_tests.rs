//! Canonical registry owners use the same captured frames as their host callers.
use super::*;
use crate::frame_identity_tests::{assert_group, groups};

#[test]
fn captured_original_ivm_registry_frames() {
    let groups = groups("registry", 8);
    assert_group(
        &groups[0],
        "order_min",
        OrderSchema {
            qty: i64::MIN,
            side: "雪\\\"".to_owned(),
        },
    );
    assert_group(
        &groups[1],
        "order_max",
        OrderSchema {
            qty: i64::MAX,
            side: "buy".to_owned(),
        },
    );
    assert_group(
        &groups[2],
        "order_time_min",
        OrderByTimeSchema {
            qty: i64::MIN,
            side: "sell".to_owned(),
            tif: 0,
        },
    );
    assert_group(
        &groups[3],
        "order_time_max",
        OrderByTimeSchema {
            qty: i64::MAX,
            side: "buy".to_owned(),
            tif: u32::MAX,
        },
    );
    assert_group(
        &groups[4],
        "trade_v1_min",
        TradeV1Schema {
            qty: i64::MIN,
            price: i64::MAX,
            side: "sell".to_owned(),
        },
    );
    assert_group(
        &groups[5],
        "trade_v1_max",
        TradeV1Schema {
            qty: i64::MAX,
            price: i64::MIN,
            side: "buy".to_owned(),
        },
    );
    assert_group(
        &groups[6],
        "trade_v2_min",
        TradeV2Schema {
            qty: i64::MIN,
            price: i64::MAX,
            side: "sell".to_owned(),
            venue: "雪".to_owned(),
        },
    );
    assert_group(
        &groups[7],
        "trade_v2_max",
        TradeV2Schema {
            qty: i64::MAX,
            price: i64::MIN,
            side: "buy".to_owned(),
            venue: "iroha".to_owned(),
        },
    );
}
