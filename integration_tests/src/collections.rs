//! Typed projections of canonical Torii collection rows used by query regressions.
use eyre::{Result, eyre};
use iroha::data_model::prelude::*;
use iroha::{
    blocking::Client,
    collections::{Collection, ListQuery},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use norito::json::{JsonDeserialize, Value};

/// Identity and metadata retained from a canonical account row.
#[derive(Debug, JsonDeserialize)]
pub struct AccountRow {
    id: AccountId,
    metadata: Metadata,
}
impl AccountRow {
    /// Canonical account identifier.
    pub fn id(&self) -> &AccountId {
        &self.id
    }
    /// Committed account metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

/// Read all canonical account rows.
pub fn accounts(client: &Client) -> Result<Vec<AccountRow>> {
    client
        .list(Collection::Accounts, ListQuery::new())
        .map(|row| Ok(norito::json::from_value(row?)?))
        .collect()
}

/// Read complete canonical asset definitions.
pub fn definitions(client: &Client) -> Result<Vec<AssetDefinition>> {
    definitions_with_query(client, ListQuery::new())
}

/// Read complete asset definitions using server-side filtering, ordering and paging.
pub fn definitions_with_query(client: &Client, query: ListQuery) -> Result<Vec<AssetDefinition>> {
    client
        .list(Collection::AssetDefinitions, query)
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

/// Read complete roles from the contacted root as an authenticated global reader.
pub fn roles(client: &Client) -> Result<Vec<Role>> {
    client
        .list(Collection::Roles, ListQuery::new())
        .map(|row| Ok(norito::json::from_value(row?)?))
        .collect()
}

/// Read exact role identifiers from the role or assignment collection.
pub fn role_ids(client: &Client, collection: Collection) -> Result<Vec<RoleId>> {
    client
        .list(collection, ListQuery::new().select(["id"]))
        .map(|row| field(&row?, "id"))
        .collect()
}

/// Decode one required collection field, refusing missing or mismatched values.
pub fn field<T: JsonDeserialize>(row: &Value, name: &str) -> Result<T> {
    let value = row
        .get(name)
        .ok_or_else(|| eyre!("collection row is missing {name}"))?;
    Ok(norito::json::from_value(value.clone())?)
}

/// Test identity membership through a complete authenticated collection query.
///
/// Only a successful empty result proves absence; transport, decoding, duplicate identity,
/// and mismatched filter results remain errors.
pub fn contains_exact_id(client: &Client, collection: Collection, id: &str) -> Result<bool> {
    exact_identity_presence(
        client
            .list(
                collection,
                ListQuery::new()
                    .filter(iroha::collections::field("id").eq(id))
                    .select(["id"]),
            )
            .map(|row| Ok(row?)),
        id,
    )
}

fn exact_identity_presence(
    rows: impl IntoIterator<Item = Result<Value>>,
    expected: &str,
) -> Result<bool> {
    let mut present = false;
    for row in rows {
        let actual: String = field(&row?, "id")?;
        eyre::ensure!(
            actual == expected,
            "exact identity filter returned {actual}, expected {expected}"
        );
        eyre::ensure!(
            !present,
            "exact identity filter returned duplicate {expected}"
        );
        present = true;
    }
    Ok(present)
}

#[test]
fn exact_identity_presence_requires_complete_valid_matching_rows() {
    assert!(!exact_identity_presence([], "expected").unwrap());
    let row = norito::json!({"id": "expected"});
    assert!(exact_identity_presence([Ok(row.clone())], "expected").unwrap());
    assert!(exact_identity_presence([Ok(row.clone()), Ok(row)], "expected").is_err());
    assert!(exact_identity_presence([Ok(norito::json!({"id": "other"}))], "expected").is_err());
    assert!(exact_identity_presence([Ok(norito::json!({"id": 3}))], "expected").is_err());
    assert!(exact_identity_presence([Ok(norito::json!({}))], "expected").is_err());
    assert!(exact_identity_presence([Err(eyre!("query failed"))], "expected").is_err());
    assert!(
        exact_identity_presence(
            [
                Ok(norito::json!({"id": "expected"})),
                Err(eyre!("next page failed"))
            ],
            "expected"
        )
        .is_err()
    );
}

/// Read the account identities and exact quantities of all definition holders.
pub fn holder_balances(
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

/// Read typed rows without changing their server-side order.
pub fn rows<T: JsonDeserialize>(
    client: &Client,
    collection: Collection,
    query: ListQuery,
) -> Result<Vec<T>> {
    client
        .list(collection, query)
        .map(|row| Ok(norito::json::from_value(row?)?))
        .collect()
}

/// Read direct and role-derived permissions from the canonical account collection.
pub fn permissions(client: &Client, account: &AccountId) -> Result<Vec<Permission>> {
    rows(
        client,
        Collection::AccountPermissions(account.clone()),
        ListQuery::new(),
    )
}

/// Identity and metadata retained from a canonical domain row.
#[derive(Debug, JsonDeserialize)]
pub struct DomainRow {
    id: DomainId,
    metadata: Metadata,
}
impl DomainRow {
    /// Domain identifier, distinct from universal account identity.
    pub fn id(&self) -> &DomainId {
        &self.id
    }
    /// Committed domain metadata.
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

#[test]
fn account_row_decodes_metadata_and_rejects_missing_identity() {
    let id = iroha_test_samples::ALICE_ID.clone();
    let row = norito::json!({"id": id, "metadata": {"key": "value"}, "label": null, "uaid": null});
    let decoded: AccountRow = norito::json::from_value(row).unwrap();
    assert_eq!(decoded.id(), &id);
    assert_eq!(decoded.metadata().get("key"), Some(&Json::from("value")));
    assert!(norito::json::from_value::<AccountRow>(norito::json!({"metadata": {}})).is_err());
}

#[test]
fn required_collection_field_preserves_type_and_refuses_missing_or_wrong_type() {
    let row = norito::json!({"quantity": "13"});
    assert_eq!(
        field::<Quantity>(&row, "quantity").unwrap(),
        Quantity::from(13_u32)
    );
    assert!(field::<Quantity>(&row, "absent").is_err());
    assert!(field::<Quantity>(&norito::json!({"quantity": []}), "quantity").is_err());
}

#[test]
fn domain_row_preserves_metadata_and_requires_identity() {
    let id = DomainId::try_new("wonderland", "universal").unwrap();
    let owner = iroha_test_samples::ALICE_ID.clone();
    let row = norito::json!({"id": id, "metadata": {"rank": 3}, "owned_by": owner, "logo": null});
    let decoded: DomainRow = norito::json::from_value(row).unwrap();
    assert_eq!(decoded.id(), &id);
    assert_eq!(decoded.metadata().get("rank"), Some(&Json::from(3_u32)));
    assert!(norito::json::from_value::<DomainRow>(norito::json!({"metadata": {}})).is_err());
}
