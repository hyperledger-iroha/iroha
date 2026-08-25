#[test]
fn release_openapi_rendering_uses_one_final_newline() {
    let mut source = json::to_string_pretty(&empty_openapi_stub())
        .expect("render release OpenAPI document")
        .into_bytes();
    source.extend_from_slice(b"\r\n\n");
    let rendered = render_release_openapi(source);

    assert!(rendered.ends_with(b"}\n"));
    assert!(!rendered.ends_with(b"}\n\n"));
}

#[test]
fn manifest_writers_reject_empty_openapi_before_output() {
    let tmp = tempdir().expect("tempdir");
    let spec_path = tmp.path().join("torii.json");
    let stub_bytes = norito::json::to_vec(&empty_openapi_stub()).expect("serialize empty OpenAPI");
    fs::write(&spec_path, &stub_bytes).expect("write empty OpenAPI");
    let key_path = tmp.path().join("key.hex");
    fs::write(&key_path, hex::encode([0x42_u8; 32])).expect("write key");
    let detached =
        sign_manifest_payload_for_test(&stub_bytes, &key_path).expect("sign fixture payload");
    for (name, result, manifest_path) in [
        {
            let manifest_path = tmp.path().join("signed.json");
            let result = write_openapi_manifest(
                &spec_path,
                &manifest_path,
                &key_path,
                &clean_generator_provenance(),
            );
            ("signed", result, manifest_path)
        },
        {
            let manifest_path = tmp.path().join("detached.json");
            let result = write_openapi_manifest_with_signature(
                &spec_path,
                &manifest_path,
                detached,
                &clean_generator_provenance(),
                None,
            );
            ("detached", result, manifest_path)
        },
        {
            let manifest_path = tmp.path().join("unsigned.json");
            let result = write_openapi_manifest_unsigned(
                &spec_path,
                &manifest_path,
                &clean_generator_provenance(),
                None,
            );
            ("unsigned", result, manifest_path)
        },
    ] {
        let err = result.expect_err("empty OpenAPI must not produce a manifest");
        assert!(
            err.to_string()
                .contains("empty/stub specifications are forbidden"),
            "unexpected {name} manifest error: {err}"
        );
        assert!(
            !manifest_path.exists(),
            "{name} manifest must not be created for an empty OpenAPI document"
        );
    }
}
