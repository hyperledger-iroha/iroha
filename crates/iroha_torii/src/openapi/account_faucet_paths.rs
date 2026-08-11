//! OpenAPI paths for the exact-network account faucet protocol.

use norito::json::{Map, Value};

/// Insert the faucet puzzle and claim paths.
pub(super) fn insert(paths: &mut Map) {
    paths.insert(
        "/v1/accounts/faucet/puzzle".to_owned(),
        Value::Object(super::json_get_operation(
            "Accounts",
            "Fetch the faucet proof-of-work puzzle.",
            "Return the current decentralized faucet proof-of-work puzzle, bound to the exact genesis-derived NetworkId and anchored to recent committed block data; difficulty is always non-zero and adapts to recent committed and queued faucet claim volume, the work predicate is memory-hard scrypt, and finalized VRF seed material is required in the challenge when that mode is enabled.",
            "#/components/schemas/AccountFaucetPuzzle",
            Vec::new(),
        )),
    );
    paths.insert(
        "/v1/accounts/faucet".to_owned(),
        Value::Object(super::json_post_operation_with_success_status(
            "Accounts",
            "Request faucet funds.",
            "Queue a server-signed account registration and fixed-amount testnet transfer when the configured faucet is enabled and a valid memory-hard scrypt proof-of-work solution for the exact-network, queue-aware puzzle is supplied. The proof is the protocol-native authenticated principal for this mutation.",
            "#/components/schemas/AccountFaucetRequest",
            "#/components/schemas/AccountFaucetResponse",
            Vec::new(),
            "202",
        )),
    );
}
