// Focused tests for App API routed-read request-body deadlines.
fn table_with_app_routed_read_body_timeout(timeout_ms: i64) -> Table {
    let mut table = base_table();
    table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table")
        .insert(
            "app_api_routed_read_body_read_timeout_ms".into(),
            Value::Integer(timeout_ms),
        );
    table
}
#[test]
fn app_routed_read_body_timeout_defaults_and_accepts_one_millisecond() {
    let default = load_root(base_table());
    assert_eq!(
        default.torii.app_api_routed_read_body_read_timeout,
        std::time::Duration::from_millis(defaults::torii::APP_API_ROUTED_READ_BODY_READ_TIMEOUT_MS)
    );
    let exact = load_root(table_with_app_routed_read_body_timeout(1));
    assert_eq!(
        exact.torii.app_api_routed_read_body_read_timeout,
        std::time::Duration::from_millis(1)
    );
}
#[test]
fn app_routed_read_body_timeout_rejects_zero() {
    let error = actual::Root::from_toml_source(TomlSource::inline(
        table_with_app_routed_read_body_timeout(0),
    ))
    .expect_err("a zero App API routed-read body-read deadline must fail closed");
    assert!(
        format!("{error:?}")
            .contains("torii.app_api_routed_read_body_read_timeout_ms must be at least 1 ms")
    );
}
fn table_with_routed_read_frame_geometry(max_content_len: i64) -> Table {
    let mut table = base_table();
    let torii = table
        .get_mut("torii")
        .and_then(Value::as_table_mut)
        .expect("torii table");
    torii.insert("max_content_len".into(), Value::Integer(max_content_len));
    torii.insert(
        "query_fanout_max_retained_bytes".into(),
        Value::Integer(
            i64::try_from(defaults::torii::QUERY_FANOUT_MIN_POOL_BYTES_V1)
                .expect("query pool minimum fits TOML integer"),
        ),
    );
    table
}
#[test]
fn app_routed_read_transport_frame_phase_accepts_exact_and_rejects_plus_one() {
    let exact = i64::try_from(defaults::torii::HTTP_READ_CHUNK_BYTES_V1)
        .expect("HTTP read chunk fits TOML integer");
    let root = load_root(table_with_routed_read_frame_geometry(exact));
    assert_eq!(root.torii.max_content_len.get(), exact.cast_unsigned());
    let error = actual::Root::from_toml_source(TomlSource::inline(
        table_with_routed_read_frame_geometry(exact - 1),
    ))
    .expect_err("a Torii read chunk above the routed-read phase must fail closed");
    assert!(format!("{error:?}").contains(
        "Torii's fixed HTTP read chunk exceeds the App API routed-read transport-frame phase"
    ));
}
#[test]
fn default_app_routed_read_transport_frame_fits_derived_phase() {
    let default = load_root(base_table());
    let phase = defaults::torii::app_api_routed_read_route_body_phase_bytes(
        default.torii.query_fanout_max_retained_bytes.get(),
        default.torii.max_content_len.get(),
    )
    .expect("default routed-read phase");
    let frame = defaults::torii::HTTP_READ_CHUNK_BYTES_V1;
    assert!(frame <= phase);
}
