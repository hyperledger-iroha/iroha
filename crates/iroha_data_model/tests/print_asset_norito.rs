//! Test utility to print the norito-encoded asset ID for CLI minting.

use iroha_data_model::prelude::*;

#[test]
fn print_usd_asset_norito() {
    // User account — i105 literal (no @domain)
    let account = AccountId::parse_encoded("soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ")
        .expect("parse account I105")
        .into_account_id();
    let def: AssetDefinitionId = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
        .parse()
        .expect("parse asset def");
    let asset_id = AssetId::new(def, account);
    println!("\n\nNORITO_ASSET_ID={}\n\n", asset_id.canonical_literal());
}
