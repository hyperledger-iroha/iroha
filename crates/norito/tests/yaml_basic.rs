use norito::json::{self, Value};

#[test]
fn yaml_renders_plain_and_block_scalars() {
    let mut job = json::Map::new();
    job.insert(
        "items".into(),
        Value::Array(vec![
            Value::String("alpha".into()),
            Value::String("beta:1".into()),
        ]),
    );
    job.insert(
        "script".into(),
        Value::String("echo one\necho two\n".into()),
    );

    let mut root = json::Map::new();
    root.insert("stable".into(), Value::Bool(true));
    root.insert("job".into(), Value::Object(job));

    let yaml = norito::yaml::to_string_from_value(&Value::Object(root)).expect("yaml encode");
    let expected = "job:\n  items:\n    - alpha\n    - beta:1\n  script: |-\n    echo one\n    echo two\nstable: true\n";
    assert_eq!(yaml, expected);
}
