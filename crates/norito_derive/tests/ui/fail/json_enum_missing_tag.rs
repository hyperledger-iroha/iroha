use norito::derive::{JsonDeserialize, JsonSerialize};

#[derive(JsonSerialize, JsonDeserialize)]
enum NoTag {
    Unit,
}

fn main() {}
