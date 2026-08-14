// SoraFS bridge regressions are included at crate scope to preserve test-only visibility.
#[cfg(test)]
mod sorafs_tests {
    use std::{ffi::CString, fs, ptr, slice};
    use sorafs_car::{CarBuildPlan, fetch_plan::chunk_fetch_plan_to_string};
    use sorafs_chunker::ChunkProfile;
    use tempfile::tempdir;
    use super::*;
    fn transport_hint_json(priority: JsonValue) -> JsonValue {
        let mut hint = JsonMap::new();
        hint.insert("protocol".into(), JsonValue::from("quic"));
        hint.insert("protocol_id".into(), JsonValue::from(1u64));
        hint.insert("priority".into(), priority);
        JsonValue::Array(vec![JsonValue::Object(hint)])
    }
    #[test]
    fn transport_hint_priority_rejects_u8_wrapping() {
        for (priority, expected) in [(0_u64, 0), (u64::from(u8::MAX), u8::MAX)] {
            let hints = transport_hints_from_json(&transport_hint_json(JsonValue::from(priority)))
                .expect("u8 priority boundary must parse");
            assert_eq!(hints.len(), 1);
            assert_eq!(hints[0].priority, expected);
        }
        for (label, priority) in [
            ("negative", JsonValue::from(-1_i64)),
            ("overflow", JsonValue::from(u64::from(u8::MAX) + 1)),
        ] {
            assert!(
                matches!(
                    transport_hints_from_json(&transport_hint_json(priority)),
                    Err(ERR_FETCH_PROVIDERS_JSON)
                ),
                "{label} priority must not alias through u8"
            );
        }
    }
    #[test]
    fn sorafs_local_fetch_via_ffi() {
        let tempdir = tempdir().expect("tempdir");
        let payload: Vec<u8> = (0..(4 * 1024_usize))
            .map(|idx| u8::try_from(idx % 251).expect("within u8"))
            .collect();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
        let plan_json = chunk_fetch_plan_to_string(&plan).expect("plan json render");
        let alpha_path = tempdir.path().join("alpha.bin");
        fs::write(&alpha_path, &payload).expect("write payload");
        let mut provider = JsonMap::new();
        provider.insert("name".into(), JsonValue::from("alpha"));
        provider.insert(
            "path".into(),
            JsonValue::from(alpha_path.display().to_string()),
        );
        provider.insert("max_concurrent".into(), JsonValue::from(2u64));
        provider.insert("weight".into(), JsonValue::from(1u64));
        let providers_json =
            norito::json::to_string(&JsonValue::Array(vec![JsonValue::Object(provider)]))
                .expect("providers json render");
        let plan_c = CString::new(plan_json).expect("plan cstring");
        let providers_c = CString::new(providers_json).expect("providers cstring");
        let options_c = CString::new("{}").expect("options cstring");
        let mut out_payload_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_payload_len: c_ulong = 0;
        let mut out_report_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_report_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_local_fetch(
                plan_c.as_ptr(),
                plan_c.as_bytes().len() as c_ulong,
                providers_c.as_ptr(),
                providers_c.as_bytes().len() as c_ulong,
                options_c.as_ptr(),
                options_c.as_bytes().len() as c_ulong,
                &mut out_payload_ptr,
                &mut out_payload_len,
                &mut out_report_ptr,
                &mut out_report_len,
            )
        };
        assert_eq!(rc, 0, "ffi call should succeed");
        let assembled = unsafe {
            let bytes = slice::from_raw_parts(out_payload_ptr, out_payload_len as usize);
            bytes.to_vec()
        };
        assert_eq!(assembled, payload, "payload must match input bytes");
        let report_value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(out_report_ptr, out_report_len as usize);
            norito::json::from_slice(bytes).expect("report json")
        };
        let chunk_count = report_value
            .get("chunk_count")
            .and_then(JsonValue::as_u64)
            .expect("chunk_count present");
        assert_eq!(
            chunk_count as usize,
            plan.try_chunk_fetch_specs().expect("valid CAR plan").len(),
            "chunk count matches plan"
        );
        let reports = report_value
            .get("provider_reports")
            .and_then(JsonValue::as_array)
            .expect("provider reports");
        assert_eq!(reports.len(), 1);
        let report = reports[0].as_object().expect("report object");
        assert_eq!(
            report
                .get("provider")
                .and_then(JsonValue::as_str)
                .expect("provider name"),
            "alpha"
        );
        assert_eq!(
            report
                .get("failures")
                .and_then(JsonValue::as_u64)
                .expect("failures"),
            0
        );
        let receipts = report_value
            .get("chunk_receipts")
            .and_then(JsonValue::as_array)
            .expect("chunk receipts");
        assert_eq!(
            receipts.len(),
            plan.try_chunk_fetch_specs().expect("valid CAR plan").len()
        );
        assert!(receipts.iter().all(|entry| {
            entry
                .get("provider")
                .and_then(JsonValue::as_str)
                .map(|name| name == "alpha")
                .unwrap_or(false)
        }));
        assert!(
            report_value
                .get("scoreboard")
                .map(JsonValue::is_null)
                .unwrap_or(false),
            "scoreboard should be null when not requested"
        );
        if !out_payload_ptr.is_null() {
            connect_norito_free(out_payload_ptr);
        }
        if !out_report_ptr.is_null() {
            connect_norito_free(out_report_ptr);
        }
    }
    fn repo_fixture(path: &str) -> Vec<u8> {
        fs::read(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
            .expect("read repository fixture")
    }
    unsafe fn take_bridge_json(ptr_: *mut c_uchar, len: c_ulong) -> JsonValue {
        let value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(ptr_, len as usize);
            norito::json::from_slice(bytes).expect("parse outcome JSON")
        };
        if !ptr_.is_null() {
            connect_norito_free(ptr_);
        }
        value
    }
    unsafe fn take_bridge_json_usize(ptr_: *mut c_uchar, len: usize) -> JsonValue {
        let value: JsonValue = unsafe {
            let bytes = slice::from_raw_parts(ptr_, len);
            norito::json::from_slice(bytes).expect("parse outcome JSON")
        };
        if !ptr_.is_null() {
            connect_norito_free(ptr_);
        }
        value
    }
    #[test]
    fn sorafs_reference_orderbook_validator_via_bridge_ffi() {
        let payload = repo_fixture("fixtures/sorafs_manifest/orderbook/order_request_v1.to");
        let label = b"order-request.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_orderbook_json(
                sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge validator call should succeed");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
    }
    #[test]
    fn sorafs_reference_orderbook_validator_rejects_bad_signature_via_bridge_ffi() {
        let payload = repo_fixture(
            "fixtures/sorafs_manifest/orderbook/negative/order_request_bad_signature_v1.to",
        );
        let label = b"order_request_bad_signature_v1.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_orderbook_json(
                sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge validator call should succeed");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Error")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-SIG-007")
        );
        assert_eq!(
            outcome.get("category").and_then(JsonValue::as_str),
            Some("signature")
        );
        assert_eq!(
            outcome.get("generated_at").and_then(JsonValue::as_u64),
            Some(123)
        );
        assert_eq!(
            outcome
                .get("inputs")
                .and_then(JsonValue::as_array)
                .and_then(|inputs| inputs.first())
                .and_then(|input| input.get("path"))
                .and_then(JsonValue::as_str),
            Some("order_request_bad_signature_v1.to")
        );
    }
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
    #[test]
    fn sorafs_reference_pop_validator_via_bridge_ffi() {
        let payload = norito::to_bytes(&sorafs_manifest::PopEnrollmentRequestV1 {
            version: sorafs_manifest::POP_ENROLLMENT_REQUEST_VERSION_V1,
            request_id: [0x21; 32],
            applicant_id: "alice@sora".to_owned(),
            requested_class: sorafs_manifest::PopEligibilityClassV1::General,
            requested_attributes: vec!["residency".to_owned()],
            attestation_digest: [0x22; 32],
            submitted_at_epoch: 100,
            expires_at_epoch: 200,
        })
        .expect("encode PoP enrollment request");
        let label = b"pop-enrollment-request.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_pop_json(
                sorafs_reference_ffi::SORAFS_REFERENCE_POP_KIND_ENROLLMENT_REQUEST,
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge PoP validator call should succeed");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
    }
    #[test]
    fn sorafs_reference_hedging_validator_via_bridge_ffi() {
        let payload = norito::to_bytes(&sorafs_manifest::HedgingPriceFeedV1 {
            version: sorafs_manifest::HEDGING_PRICE_FEED_VERSION_V1,
            feed_id: "primary".to_owned(),
            source: "primary-oracle".to_owned(),
            observed_at_unix: 1_800,
            xor_usd_price: "2".parse().expect("canonical exact XOR/USD price"),
            weight_bps: 5_000,
            evidence_digest: [0x32; 32],
            status: sorafs_manifest::HedgingFeedStatusV1::Ok,
        })
        .expect("encode hedging price feed");
        let label = b"hedging-price-feed.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_hedging_json(
                sorafs_reference_ffi::SORAFS_REFERENCE_HEDGING_KIND_PRICE_FEED,
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge hedging validator call should succeed");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
    }
    #[test]
    fn sorafs_reference_bundle_validator_via_bridge_ffi() {
        let order = repo_fixture("fixtures/sorafs_manifest/replication_order/order_v1.to");
        let proof = repo_fixture("fixtures/sorafs_manifest/por/proof_v1.to");
        let order_label = b"replication-order.to";
        let proof_label = b"por-proof.to";
        let payloads = [
            ConnectNoritoSorafsReferenceBundlePayload {
                kind: sorafs_reference_ffi::SORAFS_REFERENCE_BUNDLE_KIND_REPLICATION_ORDER,
                bytes_ptr: order.as_ptr(),
                bytes_len: order.len(),
                label_ptr: order_label.as_ptr(),
                label_len: order_label.len(),
            },
            ConnectNoritoSorafsReferenceBundlePayload {
                kind: sorafs_reference_ffi::SORAFS_REFERENCE_BUNDLE_KIND_POR_PROOF,
                bytes_ptr: proof.as_ptr(),
                bytes_len: proof.len(),
                label_ptr: proof_label.as_ptr(),
                label_len: proof_label.len(),
            },
        ];
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len = 0usize;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_bundle_json(
                payloads.as_ptr(),
                payloads.len(),
                1_700_000_001,
                126,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge fixture-bundle validator call");
        let outcome = unsafe { take_bridge_json_usize(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
        assert_eq!(
            outcome.get("generated_at").and_then(JsonValue::as_u64),
            Some(126)
        );
    }
    #[test]
    fn sorafs_reference_governance_dag_block_validator_via_bridge_ffi() {
        let payload = [0xA5];
        let label = b"governance-block.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len = 0usize;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_governance_dag_block_json(
                payload.as_ptr(),
                payload.len(),
                label.as_ptr(),
                label.len(),
                ptr::null(),
                0,
                124,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge governance block validator call");
        let outcome = unsafe { take_bridge_json_usize(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Error")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-NORITO-001")
        );
        assert_eq!(
            outcome.get("generated_at").and_then(JsonValue::as_u64),
            Some(124)
        );
    }
    #[test]
    fn sorafs_reference_governance_log_node_validator_via_bridge_ffi() {
        let payload = repo_fixture("fixtures/sorafs_manifest/moderation/governance_node_v1.to");
        let expected_node_cid =
            hex::decode("9a2dc9a930494cbc70f0e4cab25df893fb607e83f1fa52520ed62dabca918d5a")
                .expect("fixture node CID");
        let label = b"moderation/governance_node_v1.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len = 0usize;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_governance_json(
                payload.as_ptr(),
                payload.len(),
                label.as_ptr(),
                label.len(),
                expected_node_cid.as_ptr(),
                expected_node_cid.len(),
                1_700_001_234,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge governance log-node validator call");
        let outcome = unsafe { take_bridge_json_usize(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
        assert_eq!(
            outcome.get("generated_at").and_then(JsonValue::as_u64),
            Some(1_700_001_234)
        );
    }
    #[test]
    fn sorafs_reference_governance_dag_head_chain_validator_via_bridge_ffi() {
        let head = [0xA5];
        let head_label = b"governance-head.to";
        let block = [0x5A];
        let block_label = b"governance-block-0.to";
        let blocks = [ConnectNoritoSorafsReferenceInput {
            bytes_ptr: block.as_ptr(),
            bytes_len: block.len(),
            label_ptr: block_label.as_ptr(),
            label_len: block_label.len(),
        }];
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len = 0usize;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_governance_dag_head_chain_json(
                head.as_ptr(),
                head.len(),
                head_label.as_ptr(),
                head_label.len(),
                blocks.as_ptr(),
                blocks.len(),
                125,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge governance head-chain validator call");
        let outcome = unsafe { take_bridge_json_usize(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Error")
        );
        let inputs = outcome
            .get("inputs")
            .and_then(JsonValue::as_array)
            .expect("validation inputs");
        assert_eq!(inputs.len(), 2);
        assert_eq!(
            inputs[1].get("path").and_then(JsonValue::as_str),
            Some("governance-block-0.to")
        );
    }
    #[test]
    fn sorafs_reference_orderbook_signing_via_bridge_ffi() {
        let payload = repo_fixture("fixtures/sorafs_manifest/orderbook/order_request_v1.to");
        let private_key = [0xB7; 32];
        let mut signed_ptr: *mut c_uchar = ptr::null_mut();
        let mut signed_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_sign_orderbook_payload(
                sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
                payload.as_ptr(),
                payload.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut signed_ptr,
                &mut signed_len,
            )
        };
        assert_eq!(rc, 0, "bridge signer call should succeed");
        let signed = unsafe { slice::from_raw_parts(signed_ptr, signed_len as usize).to_vec() };
        assert!(!signed.is_empty(), "signed payload should be returned");
        assert_eq!(
            signed, payload,
            "signing the canonical fixture with its deterministic key must be byte-identical"
        );
        if !signed_ptr.is_null() {
            connect_norito_free(signed_ptr);
        }
        let label = b"signed-order-request.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let validate_rc = unsafe {
            connect_norito_sorafs_reference_validate_orderbook_json(
                sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
                signed.as_ptr(),
                signed.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(validate_rc, 0, "signed payload should validate");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
    }
    #[test]
    fn sorafs_reference_orderbook_signing_rejects_retired_snapshot_selector_via_bridge_ffi() {
        let payload = b"retired runtime snapshot";
        let private_key = [0xB7; 32];
        let mut signed_ptr: *mut c_uchar = ptr::null_mut();
        let mut signed_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_sign_orderbook_payload(
                6,
                payload.as_ptr(),
                payload.len() as c_ulong,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut signed_ptr,
                &mut signed_len,
            )
        };
        assert_eq!(rc, ERR_SORAFS_REFERENCE);
        assert!(signed_ptr.is_null());
        assert_eq!(signed_len, 0);
    }
    fn validate_signed_orderbook_payload(kind: u32, payload: &[u8], label: &[u8]) -> JsonValue {
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let validate_rc = unsafe {
            connect_norito_sorafs_reference_validate_orderbook_json(
                kind,
                payload.as_ptr(),
                payload.len() as c_ulong,
                label.as_ptr(),
                label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(validate_rc, 0, "signed payload should validate");
        unsafe { take_bridge_json(out_ptr, out_len) }
    }
    #[test]
    fn sorafs_reference_orderbook_field_builders_via_bridge_ffi() {
        let private_key = [0xB7; 32];
        let owner = b"merchant@paynet";
        let price = b"340282366920938463463374607431768211456.000000001";
        let order_id = derive_orderbook_order_id_v1(owner, 7);
        let mut derived_order_id = [0_u8; 32];
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    7,
                    derived_order_id.as_mut_ptr(),
                    derived_order_id.len() as c_ulong,
                )
            },
            0
        );
        assert_eq!(derived_order_id, order_id);
        let mut order_ptr: *mut c_uchar = ptr::null_mut();
        let mut order_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                order_id.as_ptr(),
                32,
                SORAFS_ORDERBOOK_SIDE_BID,
                SORAFS_ORDERBOOK_TIER_HOT,
                price.as_ptr(),
                price.len() as c_ulong,
                12,
                12,
                owner.as_ptr(),
                owner.len() as c_ulong,
                ptr::null(),
                0,
                1_700_010_000,
                7,
                25,
                30,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut order_ptr,
                &mut order_len,
            )
        };
        assert_eq!(rc, 0, "order request builder should succeed");
        let order_bytes = unsafe { slice::from_raw_parts(order_ptr, order_len as usize).to_vec() };
        assert!(!order_bytes.is_empty());
        if !order_ptr.is_null() {
            connect_norito_free(order_ptr);
        }
        let order_outcome = validate_signed_orderbook_payload(
            sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
            &order_bytes,
            b"built-order-request.to",
        );
        assert_eq!(
            order_outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        let provider_id = [0x72; 32];
        let ask_order_id = derive_orderbook_order_id_v1(owner, 8);
        order_ptr = ptr::null_mut();
        order_len = 0;
        let ask_rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                ask_order_id.as_ptr(),
                ask_order_id.len() as c_ulong,
                SORAFS_ORDERBOOK_SIDE_ASK,
                SORAFS_ORDERBOOK_TIER_HOT,
                price.as_ptr(),
                price.len() as c_ulong,
                4,
                4,
                owner.as_ptr(),
                owner.len() as c_ulong,
                provider_id.as_ptr(),
                provider_id.len() as c_ulong,
                1_700_010_000,
                8,
                25,
                30,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut order_ptr,
                &mut order_len,
            )
        };
        assert_eq!(ask_rc, 0, "provider-bound ask builder should succeed");
        let ask_bytes = unsafe { slice::from_raw_parts(order_ptr, order_len as usize).to_vec() };
        connect_norito_free(order_ptr);
        let ask_outcome = validate_signed_orderbook_payload(
            sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_REQUEST,
            &ask_bytes,
            b"built-provider-bound-ask.to",
        );
        assert_eq!(
            ask_outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        let mut cancel_ptr: *mut c_uchar = ptr::null_mut();
        let mut cancel_len: c_ulong = 0;
        let cancel_rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_cancel(
                order_id.as_ptr(),
                32,
                owner.as_ptr(),
                owner.len() as c_ulong,
                SORAFS_ORDERBOOK_CANCEL_REASON_OWNER_REQUESTED,
                8,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut cancel_ptr,
                &mut cancel_len,
            )
        };
        assert_eq!(cancel_rc, 0, "order cancel builder should succeed");
        let cancel_bytes =
            unsafe { slice::from_raw_parts(cancel_ptr, cancel_len as usize).to_vec() };
        if !cancel_ptr.is_null() {
            connect_norito_free(cancel_ptr);
        }
        let cancel_outcome = validate_signed_orderbook_payload(
            sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_ORDER_CANCEL,
            &cancel_bytes,
            b"built-order-cancel.to",
        );
        assert_eq!(
            cancel_outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        let debit = b"340282366920938463463374607431768211456.000000001";
        let credit = b"340282366920938463463374607431768211456";
        let fee = b"0.000000001";
        let mut receipt_ptr: *mut c_uchar = ptr::null_mut();
        let mut receipt_len: c_ulong = 0;
        let receipt_rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt(
                [0x21; 32].as_ptr(),
                32,
                [0x22; 32].as_ptr(),
                32,
                [0x23; 32].as_ptr(),
                32,
                0,
                4096,
                [0x24; 32].as_ptr(),
                32,
                4096,
                debit.as_ptr(),
                debit.len() as c_ulong,
                credit.as_ptr(),
                credit.len() as c_ulong,
                fee.as_ptr(),
                fee.len() as c_ulong,
                1_700_000_999,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut receipt_ptr,
                &mut receipt_len,
            )
        };
        assert_eq!(receipt_rc, 0, "settlement receipt builder should succeed");
        let receipt_bytes =
            unsafe { slice::from_raw_parts(receipt_ptr, receipt_len as usize).to_vec() };
        if !receipt_ptr.is_null() {
            connect_norito_free(receipt_ptr);
        }
        let receipt_outcome = validate_signed_orderbook_payload(
            sorafs_reference_ffi::SORAFS_REFERENCE_ORDERBOOK_KIND_SETTLEMENT_RECEIPT,
            &receipt_bytes,
            b"built-settlement-receipt.to",
        );
        assert_eq!(
            receipt_outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
    }
    #[test]
    fn sorafs_reference_order_id_bridge_rejects_noncanonical_inputs() {
        let owner = b"merchant@paynet";
        let mut output = [0_u8; 32];
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    0,
                    1,
                    output.as_mut_ptr(),
                    output.len() as c_ulong,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    1,
                    ptr::null_mut(),
                    output.len() as c_ulong,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    0,
                    output.as_mut_ptr(),
                    output.len() as c_ulong,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    1,
                    output.as_mut_ptr(),
                    31,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        let price = b"1000000";
        let private_key = [0xB7; 32];
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                [0x11; 32].as_ptr(),
                32,
                SORAFS_ORDERBOOK_SIDE_BID,
                SORAFS_ORDERBOOK_TIER_HOT,
                price.as_ptr(),
                price.len() as c_ulong,
                12,
                12,
                owner.as_ptr(),
                owner.len() as c_ulong,
                ptr::null(),
                0,
                1_700_010_000,
                7,
                25,
                30,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, ERR_SORAFS_REFERENCE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }
    #[test]
    fn sorafs_reference_orderbook_bridge_enforces_provider_side_binding() {
        let owner = b"merchant@paynet";
        let price = b"1";
        let private_key = [0xB7; 32];
        let provider_id = [0x72; 32];
        let order_id = derive_orderbook_order_id_v1(owner, 17);
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let bid_with_provider = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                order_id.as_ptr(),
                order_id.len() as c_ulong,
                SORAFS_ORDERBOOK_SIDE_BID,
                SORAFS_ORDERBOOK_TIER_HOT,
                price.as_ptr(),
                price.len() as c_ulong,
                1,
                1,
                owner.as_ptr(),
                owner.len() as c_ulong,
                provider_id.as_ptr(),
                provider_id.len() as c_ulong,
                1_700_010_000,
                17,
                0,
                0,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(bid_with_provider, ERR_SORAFS_REFERENCE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
        let ask_without_provider = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                order_id.as_ptr(),
                order_id.len() as c_ulong,
                SORAFS_ORDERBOOK_SIDE_ASK,
                SORAFS_ORDERBOOK_TIER_HOT,
                price.as_ptr(),
                price.len() as c_ulong,
                1,
                1,
                owner.as_ptr(),
                owner.len() as c_ulong,
                ptr::null(),
                0,
                1_700_010_000,
                17,
                0,
                0,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(ask_without_provider, ERR_SORAFS_REFERENCE);
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }
    #[test]
    fn sorafs_reference_xor_quantity_bridge_requires_canonical_exact_text() {
        const MAX_SCALED: &[u8] = b"6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824.503042047";
        assert_eq!(MAX_SCALED.len(), 155);
        assert!(sorafs_xor_quantity_from_bytes(MAX_SCALED).is_ok());
        assert!(sorafs_xor_quantity_from_bytes(b"0.000000001").is_ok());
        assert!(
            sorafs_xor_quantity_from_bytes(b"340282366920938463463374607431768211456.000000001")
                .is_ok()
        );
        assert_eq!(
            sorafs_xor_quantity_from_bytes(b"1.0"),
            Err(ERR_SORAFS_REFERENCE)
        );
        assert_eq!(
            sorafs_xor_quantity_from_bytes(b" 1"),
            Err(ERR_SORAFS_REFERENCE)
        );
        assert_eq!(
            sorafs_xor_quantity_from_bytes(b"0.0000000001"),
            Err(ERR_SORAFS_REFERENCE)
        );
        assert_eq!(
            sorafs_xor_quantity_from_bytes(&[b'1'; 156]),
            Err(ERR_SORAFS_REFERENCE)
        );
    }
    #[test]
    fn sorafs_reference_orderbook_bridge_enforces_owner_account_v1_byte_ceiling() {
        let owner = vec![0x45; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1];
        let private_key = [0xB7; 32];
        let price = b"1";
        let order_id = derive_orderbook_order_id_v1(&owner, 1);
        let mut derived_order_id = [0_u8; 32];
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    1,
                    derived_order_id.as_mut_ptr(),
                    derived_order_id.len() as c_ulong,
                )
            },
            0
        );
        assert_eq!(derived_order_id, order_id);
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                    order_id.as_ptr(),
                    order_id.len() as c_ulong,
                    SORAFS_ORDERBOOK_SIDE_BID,
                    SORAFS_ORDERBOOK_TIER_HOT,
                    price.as_ptr(),
                    price.len() as c_ulong,
                    1,
                    1,
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    ptr::null(),
                    0,
                    1,
                    1,
                    0,
                    0,
                    private_key.as_ptr(),
                    private_key.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            },
            0
        );
        assert!(!out_ptr.is_null());
        assert!(out_len > 0);
        connect_norito_free(out_ptr);
        out_ptr = ptr::null_mut();
        out_len = 0;
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_build_signed_orderbook_order_cancel(
                    order_id.as_ptr(),
                    order_id.len() as c_ulong,
                    owner.as_ptr(),
                    owner.len() as c_ulong,
                    SORAFS_ORDERBOOK_CANCEL_REASON_OWNER_REQUESTED,
                    2,
                    private_key.as_ptr(),
                    private_key.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            },
            0
        );
        assert!(!out_ptr.is_null());
        assert!(out_len > 0);
        connect_norito_free(out_ptr);
        let oversized = vec![0x45; ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1];
        let oversized_order_id = derive_orderbook_order_id_v1(&oversized, 1);
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_derive_orderbook_order_id(
                    oversized.as_ptr(),
                    oversized.len() as c_ulong,
                    1,
                    derived_order_id.as_mut_ptr(),
                    derived_order_id.len() as c_ulong,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        out_ptr = ptr::null_mut();
        out_len = 0;
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_build_signed_orderbook_order_request(
                    oversized_order_id.as_ptr(),
                    oversized_order_id.len() as c_ulong,
                    SORAFS_ORDERBOOK_SIDE_BID,
                    SORAFS_ORDERBOOK_TIER_HOT,
                    price.as_ptr(),
                    price.len() as c_ulong,
                    1,
                    1,
                    oversized.as_ptr(),
                    oversized.len() as c_ulong,
                    ptr::null(),
                    0,
                    1,
                    1,
                    0,
                    0,
                    private_key.as_ptr(),
                    private_key.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
        assert_eq!(
            unsafe {
                connect_norito_sorafs_reference_build_signed_orderbook_order_cancel(
                    oversized_order_id.as_ptr(),
                    oversized_order_id.len() as c_ulong,
                    oversized.as_ptr(),
                    oversized.len() as c_ulong,
                    SORAFS_ORDERBOOK_CANCEL_REASON_OWNER_REQUESTED,
                    2,
                    private_key.as_ptr(),
                    private_key.len() as c_ulong,
                    &mut out_ptr,
                    &mut out_len,
                )
            },
            ERR_SORAFS_REFERENCE
        );
        assert!(out_ptr.is_null());
        assert_eq!(out_len, 0);
    }
    #[test]
    fn sorafs_reference_orderbook_field_builder_rejects_imbalanced_receipt_via_bridge_ffi() {
        let private_key = [0xB7; 32];
        let debit = b"100";
        let credit = b"91";
        let fee = b"10";
        let mut receipt_ptr: *mut c_uchar = ptr::null_mut();
        let mut receipt_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_build_signed_orderbook_settlement_receipt(
                [0x31; 32].as_ptr(),
                32,
                [0x32; 32].as_ptr(),
                32,
                [0x33; 32].as_ptr(),
                32,
                0,
                4096,
                [0x34; 32].as_ptr(),
                32,
                4096,
                debit.as_ptr(),
                debit.len() as c_ulong,
                credit.as_ptr(),
                credit.len() as c_ulong,
                fee.as_ptr(),
                fee.len() as c_ulong,
                1_700_000_999,
                private_key.as_ptr(),
                private_key.len() as c_ulong,
                &mut receipt_ptr,
                &mut receipt_len,
            )
        };
        assert_eq!(rc, ERR_SORAFS_REFERENCE);
        assert!(receipt_ptr.is_null());
        assert_eq!(receipt_len, 0);
    }
    #[test]
    fn sorafs_reference_pdp_bundle_validator_via_bridge_ffi() {
        let commitment = repo_fixture("fixtures/sorafs_manifest/pdp/commitment_v1.to");
        let challenge = repo_fixture("fixtures/sorafs_manifest/pdp/challenge_v1.to");
        let proof = repo_fixture("fixtures/sorafs_manifest/pdp/proof_v1.to");
        let commitment_label = b"commitment.to";
        let challenge_label = b"challenge.to";
        let proof_label = b"proof.to";
        let mut out_ptr: *mut c_uchar = ptr::null_mut();
        let mut out_len: c_ulong = 0;
        let rc = unsafe {
            connect_norito_sorafs_reference_validate_pdp_bundle_json(
                commitment.as_ptr(),
                commitment.len() as c_ulong,
                commitment_label.as_ptr(),
                commitment_label.len() as c_ulong,
                challenge.as_ptr(),
                challenge.len() as c_ulong,
                challenge_label.as_ptr(),
                challenge_label.len() as c_ulong,
                proof.as_ptr(),
                proof.len() as c_ulong,
                proof_label.as_ptr(),
                proof_label.len() as c_ulong,
                123,
                &mut out_ptr,
                &mut out_len,
            )
        };
        assert_eq!(rc, 0, "bridge PDP bundle validator call should succeed");
        let outcome = unsafe { take_bridge_json(out_ptr, out_len) };
        assert_eq!(
            outcome.get("status").and_then(JsonValue::as_str),
            Some("Ok")
        );
        assert_eq!(
            outcome.get("code").and_then(JsonValue::as_str),
            Some("SFS-OK-000")
        );
    }
}
