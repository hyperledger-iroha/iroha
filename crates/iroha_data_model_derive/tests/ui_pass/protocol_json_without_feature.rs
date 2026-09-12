//! Model and registration-builder JSON require no consumer feature or root JSON aliases.
use iroha_data_model_derive::{IdEqOrdHash, RegistrableBuilder, model};
use iroha_schema::IntoSchema;
use norito::{Decode, Encode};

/// Minimal authority owner used by the registration trait contract.
mod account {
    /// The authority passed to the generated builder.
    #[derive(Clone)]
    pub struct AccountId(
        /// Populated authority identity used to check builder initialization.
        pub u32,
    );
}

trait Identifiable: Eq + Ord {
    type Id: Eq + Ord + core::hash::Hash;
    fn id(&self) -> &Self::Id;
}
trait Registered {
    type With;
}
trait Registrable {
    type Target;
    fn build(self, authority: &account::AccountId) -> Self::Target;
}

#[derive(RegistrableBuilder)]
#[registrable_builder(schema_name = "norito::tests::protocol_json_without_feature::NewEntry")]
struct Entry {
    id: u64,
    #[registrable_builder(default = Vec::new())]
    tags: Vec<String>,
    #[registrable_builder(skip, init = authority.clone())]
    authority: account::AccountId,
}

#[model]
mod model {
    /// Reconstruct a scalar using both JSON traits injected from Norito.
    pub(super) fn json_roundtrip(value: u32) -> u32 {
        let mut encoded = String::new();
        value.json_serialize(&mut encoded);
        let mut parser = norito::json::Parser::new(&encoded);
        u32::json_deserialize(&mut parser).expect("decode JSON scalar")
    }
}

fn main() {
    assert_eq!(model::json_roundtrip(42), 42);
    let defaults: <Entry as Registered>::With =
        norito::json::from_str(r#"{"id":7}"#).expect("defaulted builder JSON");
    assert_eq!(defaults.id, 7);
    assert!(defaults.tags.is_empty());
    let built = defaults.build(&account::AccountId(19));
    assert_eq!(built.id, 7);
    assert!(built.tags.is_empty());
    assert_eq!(built.authority.0, 19);

    let populated = NewEntry::new(9).with_tags(vec!["first".into(), "second".into()]);
    let encoded = norito::json::to_json(&populated).expect("encode builder JSON");
    let decoded: NewEntry = norito::json::from_str(&encoded).expect("decode builder JSON");
    assert_eq!(decoded.id, populated.id);
    assert_eq!(decoded.tags, populated.tags);
    assert_eq!(
        norito::json::to_json_bounded(&populated, encoded.len()).unwrap(),
        encoded
    );
    assert!(norito::json::to_json_bounded(&populated, encoded.len() - 1).is_err());
    for invalid in [
        r#"{"tags":[]}"#,
        r#"{"id":9,"unknown":true}"#,
        r#"{"id":9,"id":9}"#,
        r#"{"id":9,"tags":[],"tags":[]}"#,
    ] {
        assert!(
            norito::json::from_str::<NewEntry>(invalid).is_err(),
            "{invalid}"
        );
    }
}
