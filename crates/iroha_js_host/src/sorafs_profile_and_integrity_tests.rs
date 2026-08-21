#[test]
fn da_manifest_chunker_handle_binding_resolves_profile() {
    ensure_packed_struct_disabled();
    let fixture = build_da_manifest_fixture();
    let handle = da_manifest_chunker_handle(Buffer::from(fixture.manifest_bytes.clone()).into())
        .expect("chunker handle");
    assert_eq!(handle, "sorafs.sf1@1.0.0");
}
#[test]
fn local_fetch_integrity_error_maps_to_invalid_argument() {
    let error = map_local_fetch_error(LocalFetchError::IntegrityVerificationDisabled(
        "verify_digests",
    ));
    assert_eq!(error.status, napi::Status::InvalidArg);
    assert_eq!(
        error.reason,
        "verify_digests must remain enabled for first-release SoraFS fetch integrity"
    );
}
