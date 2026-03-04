//! Temporary utility to print sample `DataEventFilter` values as JSON and base64-encoded Norito bytes.

use base64::Engine;
use iroha_data_model::events::data::prelude::{AssetEventFilter, AssetEventSet, DataEventFilter};

fn print_one(name: &str, filter: &DataEventFilter) {
    let json = norito::json::to_json_pretty(filter).expect("json");
    let bytes = norito::to_bytes(filter).expect("norito");
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    println!("{} JSON={}", name, json);
    println!("{} B64=\"{}\"", name, b64);
}

fn main() {
    let any = DataEventFilter::Any;
    let asset_any = DataEventFilter::Asset(AssetEventFilter::new());
    let asset_added =
        DataEventFilter::Asset(AssetEventFilter::new().for_events(AssetEventSet::Added));
    let asset_added_removed = DataEventFilter::Asset(
        AssetEventFilter::new().for_events(AssetEventSet::Added | AssetEventSet::Removed),
    );
    let asset_all =
        DataEventFilter::Asset(AssetEventFilter::new().for_events(AssetEventSet::all()));
    let asset_created_added_removed_deleted =
        DataEventFilter::Asset(AssetEventFilter::new().for_events(
            AssetEventSet::Created
                | AssetEventSet::Added
                | AssetEventSet::Removed
                | AssetEventSet::Deleted,
        ));

    print_one("ANY", &any);
    print_one("ASSET_ANY", &asset_any);
    print_one("ASSET_ADDED", &asset_added);
    print_one("ASSET_ADDED_REMOVED", &asset_added_removed);
    print_one("ASSET_ALL", &asset_all);
    print_one("ASSET_C_A_R_D", &asset_created_added_removed_deleted);
}
