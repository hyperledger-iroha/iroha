//! Typed KAGEMUSHA instruction values for generated-record identity capture.

use norito::json::Value;

use super::super::generated_record_identity_tests::capture;
use super::{RedeemKagemushaV1, TopUpKagemushaV1};

/// Build the missing KAGEMUSHA generated-record capture rows from validated requests.
pub fn values() -> Vec<Value> {
    let top_up = TopUpKagemushaV1::new(super::tests::top_up_request())
        .expect("canonical KAGEMUSHA top-up fixture");
    let redemption = RedeemKagemushaV1::new(super::tests::redemption_request())
        .expect("canonical KAGEMUSHA redemption fixture");
    vec![
        capture::<TopUpKagemushaV1>(top_up),
        capture::<RedeemKagemushaV1>(redemption),
    ]
}
