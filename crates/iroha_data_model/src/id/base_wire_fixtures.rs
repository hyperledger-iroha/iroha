//! Capture private ChainId wire helpers without adding a public codec surface.

use super::{ChainId, ChainIdText, ChainIdWire};
use iroha_crypto::KeyPair;
use norito::json::Value;

pub(crate) fn append(records: &mut Vec<Value>, signer: &KeyPair) {
    crate::base_wire_fixtures::envelopes(
        records,
        "chain/private-text",
        &ChainIdText(ChainId::from("base-fixture-1")),
        signer,
    );
    crate::base_wire_fixtures::envelopes(
        records,
        "chain/private-wire",
        &ChainIdWire(ChainIdText(ChainId::from("base-fixture-1"))),
        signer,
    );
}
