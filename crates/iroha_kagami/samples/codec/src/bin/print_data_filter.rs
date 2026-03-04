use iroha_data_model::events::data::prelude::{AssetEventFilter, AssetEventSet, DataEventFilter};

fn main() {
    let any = DataEventFilter::Any;
    let asset_any = DataEventFilter::Asset(AssetEventFilter::new());
    let asset_added = DataEventFilter::Asset(AssetEventFilter::new().for_events(AssetEventSet::Added));
    let asset_created_or_updated = DataEventFilter::Asset(
        AssetEventFilter::new().for_events(AssetEventSet::Added | AssetEventSet::Removed),
    );

    println!("ANY={}", norito::json::to_json_pretty(&any).expect("json any"));
    println!("ASSET_ANY={}", norito::json::to_json_pretty(&asset_any).expect("json asset_any"));
    println!("ASSET_ADDED={}", norito::json::to_json_pretty(&asset_added).expect("json asset_added"));
    println!("ASSET_ADD_OR_REMOVE={}", norito::json::to_json_pretty(&asset_created_or_updated).expect("json asset_set"));
}
