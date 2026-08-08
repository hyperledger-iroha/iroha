//! Appeal-finance validation tests for the native SDK bridge.

use super::*;

#[test]
fn sorafs_reference_appeal_finance_cancel_asset_lock_profiles_via_bridge_ffi() {
    for (relative_path, status, code, category) in [
        (
            "fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.to",
            "Ok",
            "SFS-OK-000",
            "validation",
        ),
        (
            "fixtures/sorafs_manifest/appeal_finance/negative/cancel_asset_lock_legacy_missing_expected_v1.to",
            "Error",
            "SFS-NORITO-001",
            "norito",
        ),
        (
            "fixtures/sorafs_manifest/appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
            "Error",
            "SFS-VAL-001",
            "validation",
        ),
    ] {
        let payload = repo_fixture(relative_path);
        let label = relative_path
            .rsplit('/')
            .next()
            .expect("fixture path contains a file name")
            .as_bytes();
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;

        let rc = unsafe {
            connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json(
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "{relative_path}: bridge validator call");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some(status),
            "{relative_path}"
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some(code),
            "{relative_path}"
        );
        assert_eq!(
            outcome.get("category").and_then(JsonValue::as_str),
            Some(category),
            "{relative_path}"
        );
        assert_eq!(
            outcome.get("generated_at").and_then(JsonValue::as_u64),
            Some(123),
            "{relative_path}"
        );
    }
}
