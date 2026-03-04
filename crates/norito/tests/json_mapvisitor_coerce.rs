use norito::json;

#[test]
fn mapvisitor_coerce_key_helpers() {
    let mut parser = json::Parser::new("{\"10\":true,\"42\":7}");
    let mut map = json::MapVisitor::new(&mut parser).expect("map visitor");

    let first_key: Option<u8> = map.coerce_key().expect("coerce first key");
    assert_eq!(first_key, Some(10));
    let first_value = map.parse_value::<bool>().expect("parse bool value");
    assert!(first_value);

    let second = map.next_entry_coerced::<u32, u64>().expect("coerced entry");
    assert_eq!(second, Some((42, 7)));

    assert!(
        map.next_entry::<json::Value>()
            .expect("no more entries")
            .is_none()
    );
    map.finish().expect("finish map visitor");
}
