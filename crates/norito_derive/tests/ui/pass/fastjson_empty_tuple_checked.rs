//! pass: a zero-field tuple variant retains its tuple pattern in checked JSON output.
#[derive(norito::derive::JsonSerialize)]
#[norito(tag = "kind", content = "body")]
enum EmptyTuple {
    Empty(),
}
fn main() {
    let value = EmptyTuple::Empty();
    let ordinary = norito::json::to_json(&value).expect("ordinary empty tuple variant");
    assert_eq!(ordinary, r#"{"kind":"Empty","body":null}"#);
    assert_eq!(
        norito::json::to_json_bounded(&value, ordinary.len()),
        Ok(ordinary)
    );
}
