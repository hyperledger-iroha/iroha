//! Typed projections of canonical Torii collection rows used by query regressions.
use eyre::{Result, eyre};
use iroha::data_model::prelude::*;
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery},
};
use iroha_model_base::metadata::Metadata;
use norito::json::{JsonDeserialize, Value};

#[derive(Debug, JsonDeserialize)]
pub(super) struct AccountRow {
    id: AccountId,
    metadata: Metadata,
}
impl AccountRow {
    pub(super) fn id(&self) -> &AccountId {
        &self.id
    }
    pub(super) fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

pub(super) fn accounts(client: &Client) -> Result<Vec<AccountRow>> {
    client
        .list(Collection::Accounts, ListQuery::new())
        .map(|row| Ok(norito::json::from_value(row?)?))
        .collect()
}

pub(super) fn definitions(client: &Client) -> Result<Vec<AssetDefinition>> {
    client
        .list(Collection::AssetDefinitions, ListQuery::new())
        .map(|row| {
            let mut row = row?;
            let object = row
                .as_object_mut()
                .ok_or_else(|| eyre!("asset definition row must be an object"))?;
            object.remove("owning_dataspace");
            object.remove("alias_binding");
            Ok(norito::json::from_value(row)?)
        })
        .collect()
}

pub(super) fn roles(client: &Client) -> Result<Vec<Role>> {
    client
        .list(Collection::Roles, ListQuery::new())
        .map(|row| Ok(norito::json::from_value(row?)?))
        .collect()
}

pub(super) fn role_ids(client: &Client, collection: Collection) -> Result<Vec<RoleId>> {
    client
        .list(collection, ListQuery::new().select(["id"]))
        .map(|row| field(&row?, "id"))
        .collect()
}

pub(super) fn field<T: JsonDeserialize>(row: &Value, name: &str) -> Result<T> {
    let value = row
        .get(name)
        .ok_or_else(|| eyre!("collection row is missing {name}"))?;
    Ok(norito::json::from_value(value.clone())?)
}

pub(super) fn holder_balances(
    client: &Client,
    definition: &AssetDefinitionId,
) -> Result<Vec<(AccountId, Quantity)>> {
    client
        .list(
            Collection::AssetHolders(definition.clone()),
            ListQuery::new(),
        )
        .map(|row| {
            let row = row?;
            Ok((field(&row, "account_id")?, field(&row, "quantity")?))
        })
        .collect()
}

#[test]
fn account_row_decodes_metadata_and_rejects_missing_identity() {
    let id = iroha_test_samples::ALICE_ID.clone();
    let row =
        norito::json::json!({"id": id, "metadata": {"key": "value"}, "label": null, "uaid": null});
    let decoded: AccountRow = norito::json::from_value(row).unwrap();
    assert_eq!(decoded.id(), &id);
    assert_eq!(
        decoded.metadata().get(&"key".parse().unwrap()),
        Some(&Json::from("value"))
    );
    assert!(norito::json::from_value::<AccountRow>(norito::json::json!({"metadata": {}})).is_err());
}

#[test]
fn required_collection_field_preserves_type_and_refuses_missing_or_wrong_type() {
    let row = norito::json::json!({"quantity": "13"});
    assert_eq!(
        field::<Quantity>(&row, "quantity").unwrap(),
        Quantity::from(13_u32)
    );
    assert!(field::<Quantity>(&row, "absent").is_err());
    assert!(field::<Quantity>(&norito::json::json!({"quantity": []}), "quantity").is_err());
}
