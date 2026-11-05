//! Canonical collection inventories used as the typed contract-query reference.

use eyre::{Result, eyre};
use integration_tests::collections::field;
use iroha::data_model::{asset::AssetBalanceScope, prelude::*};
use iroha_model_base::topology::DataSpaceId;
use norito::json::Value;

fn asset_id_from_row(row: &Value, expected_account: &AccountId) -> Result<AssetId> {
    let definition: AssetDefinitionId = field(row, "asset")?;
    let account: AccountId = field(row, "account_id")?;
    if &account != expected_account {
        return Err(eyre!("asset collection returned a different account"));
    }
    let scope: String = field(row, "scope")?;
    let scope = if scope == "global" {
        AssetBalanceScope::Global
    } else if let Some(raw) = scope.strip_prefix("dataspace:") {
        let id: u64 = raw.parse()?;
        if id.to_string() != raw {
            return Err(eyre!(
                "asset balance scope requires canonical decimal data space ID"
            ));
        }
        AssetBalanceScope::Dataspace(DataSpaceId::new(id))
    } else {
        return Err(eyre!("unknown asset balance scope"));
    };
    Ok(AssetId::with_scope(definition, account, scope))
}

#[test]
fn asset_inventory_preserves_exact_scope_and_refuses_missing_or_foreign_identity() -> Result<()> {
    let account = iroha_test_samples::ALICE_ID.clone();
    let definition = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("wonderland", "universal")?,
        "inventory".parse()?,
    );
    let row = |scope| norito::json!({"asset": definition, "account_id": account, "scope": scope});
    for (literal, scope) in [
        ("global", AssetBalanceScope::Global),
        (
            "dataspace:7",
            AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        ),
    ] {
        assert_eq!(
            asset_id_from_row(&row(literal), &account)?,
            AssetId::with_scope(definition.clone(), account.clone(), scope),
        );
    }
    for invalid in [
        "",
        "global ",
        "dataspace:07",
        "dataspace:+7",
        "dataspace:-1",
        "dataspace:",
        "lane:7",
    ] {
        assert!(
            asset_id_from_row(&row(invalid), &account).is_err(),
            "{invalid}"
        );
    }
    assert!(asset_id_from_row(&row("global"), &iroha_test_samples::BOB_ID).is_err());
    for missing in ["asset", "account_id", "scope"] {
        let mut incomplete = row("global");
        incomplete.as_object_mut().unwrap().remove(missing);
        assert!(
            asset_id_from_row(&incomplete, &account).is_err(),
            "{missing}"
        );
    }
    Ok(())
}
