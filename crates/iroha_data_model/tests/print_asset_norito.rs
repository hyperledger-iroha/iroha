//! Test utility to print the norito-encoded asset ID for CLI minting.

use iroha_data_model::prelude::*;

#[test]
fn print_usd_asset_norito() {
    // User account — I105 literal (no @domain)
    let account = AccountId::parse_encoded(
        "sorauﾛ1PｻﾃGrEYaxoﾂ3ﾍjﾗﾁ4ﾂfﾀtｼKPDﾕy2MYnｵFﾋvoGﾂﾛ7Q2V2L",
    )
    .expect("parse account I105")
    .into_account_id();
    let def: AssetDefinitionId = "usd#wonderland".parse().expect("parse asset def");
    let asset_id = AssetId::new(def, account);
    println!("\n\nNORITO_ASSET_ID={}\n\n", asset_id.canonical_encoded());
}
