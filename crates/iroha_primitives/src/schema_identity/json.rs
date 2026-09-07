//! Private JSON wire views participate in the same explicit identity contract.
use super::*;
use crate::schema_identity::{record_encode, record_nominal};

pub(crate) fn records() -> Vec<json::Value> {
    vec![
        record_encode(
            &JsonWireRef(Cow::Borrowed("{\"a\":7}")),
            std::any::type_name::<JsonWireRef<'_>>(),
        ),
        record_nominal(JsonWireOwned {
            value: "{\"a\":7}".to_owned(),
        }),
    ]
}
