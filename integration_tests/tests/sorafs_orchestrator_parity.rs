#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! JS host parity helpers (requires `js_host_parity` feature to compile).

#[cfg(feature = "js_host_parity")]
mod js_host_parity {
    //! SoraFS orchestrator parity coverage across Rust, JS, and FFI bridges.

    use core::ffi::c_ulong;
    use std::{collections::HashMap, convert::TryFrom, mem, path::Path, time::Instant};

    use connect_norito_bridge::{connect_norito_free, connect_norito_sorafs_local_fetch};
    use iroha_js_host::{
        JsLocalProviderSpec, JsMultiFetchChunkReceipt, JsMultiFetchOptions,
        JsMultiFetchProviderReport, JsProviderMetadata, JsRangeCapability, JsStreamBudget,
        JsTransportHint, JsU64,
    };
    use sorafs_car::{
        fetch_plan::{chunk_fetch_specs_to_json, chunk_fetch_specs_to_string},
        fixtures::MultiPeerFixture,
        local_fetch::{
            self, LocalFetchOptions, LocalFetchScoreboardEntry, LocalProviderInput,
            ProviderMetadataInput,
        },
    };
    use tempfile::tempdir;

    fn js_metadata_from_provider(
        metadata: &sorafs_car::multi_fetch::ProviderMetadata,
    ) -> JsProviderMetadata {
        JsProviderMetadata {
            provider_id: metadata.provider_id.clone(),
            profile_id: metadata.profile_id.clone(),
            profile_aliases: Some(metadata.profile_aliases.clone()),
            availability: metadata.availability.clone(),
            stake_amount: metadata.stake_amount.clone(),
            max_streams: metadata.max_streams.map(u16::from).map(u32::from),
            refresh_deadline: metadata.refresh_deadline.map(JsU64::from),
            expires_at: metadata.expires_at.map(JsU64::from),
            ttl_secs: metadata.ttl_secs.map(JsU64::from),
            allow_unknown_capabilities: Some(metadata.allow_unknown_capabilities),
            capability_names: Some(metadata.capability_names.clone()),
            rendezvous_topics: Some(metadata.rendezvous_topics.clone()),
            notes: metadata.notes.clone(),
            range_capability: metadata
                .range_capability
                .as_ref()
                .map(|range| JsRangeCapability {
                    max_chunk_span: range.max_chunk_span,
                    min_granularity: range.min_granularity,
                    supports_sparse_offsets: Some(range.supports_sparse_offsets),
                    requires_alignment: Some(range.requires_alignment),
                    supports_merkle_proof: Some(range.supports_merkle_proof),
                }),
            stream_budget: metadata
                .stream_budget
                .as_ref()
                .map(|budget| JsStreamBudget {
                    max_in_flight: budget.max_in_flight,
                    max_bytes_per_sec: JsU64(budget.max_bytes_per_sec),
                    burst_bytes: budget.burst_bytes.map(JsU64),
                }),
            transport_hints: Some(
                metadata
                    .transport_hints
                    .iter()
                    .map(|hint| JsTransportHint {
                        protocol: hint.protocol.clone(),
                        protocol_id: hint.protocol_id,
                        priority: hint.priority,
                    })
                    .collect(),
            ),
        }
    }

    fn providers_json_from_metadata(
        metadata: &sorafs_car::multi_fetch::ProviderMetadata,
        path: &Path,
    ) -> norito::json::Value {
        let mut obj = norito::json::Map::new();
        obj.insert(
            "name".into(),
            norito::json::Value::from(metadata.provider_id.clone().unwrap()),
        );
        obj.insert(
            "path".into(),
            norito::json::Value::from(path.to_string_lossy().to_string()),
        );
        obj.insert("max_concurrent".into(), norito::json::Value::from(2u32));
        obj.insert("weight".into(), norito::json::Value::from(1u32));

        let meta = ProviderMetadataInput::from_metadata(metadata);
        let mut meta_obj = norito::json::Map::new();
        if let Some(id) = meta.provider_id {
            meta_obj.insert("provider_id".into(), norito::json::Value::from(id));
        }
        if let Some(profile) = meta.profile_id {
            meta_obj.insert("profile_id".into(), norito::json::Value::from(profile));
        }
        if let Some(aliases) = meta.profile_aliases {
            let alias_values = aliases
                .into_iter()
                .map(norito::json::Value::from)
                .collect::<Vec<_>>();
            meta_obj.insert(
                "profile_aliases".into(),
                norito::json::Value::Array(alias_values),
            );
        }
        if let Some(availability) = meta.availability {
            meta_obj.insert(
                "availability".into(),
                norito::json::Value::from(availability),
            );
        }
        if let Some(stake) = meta.stake_amount {
            meta_obj.insert("stake_amount".into(), norito::json::Value::from(stake));
        }
        if let Some(max_streams) = meta.max_streams {
            meta_obj.insert(
                "max_streams".into(),
                norito::json::Value::from(u32::from(max_streams)),
            );
        }
        if let Some(refresh) = meta.refresh_deadline {
            meta_obj.insert(
                "refresh_deadline".into(),
                norito::json::Value::from(refresh),
            );
        }
        if let Some(expires_at) = meta.expires_at {
            meta_obj.insert("expires_at".into(), norito::json::Value::from(expires_at));
        }
        if let Some(ttl) = meta.ttl_secs {
            meta_obj.insert("ttl_secs".into(), norito::json::Value::from(ttl));
        }
        meta_obj.insert(
            "allow_unknown_capabilities".into(),
            norito::json::Value::from(meta.allow_unknown_capabilities.unwrap_or(false)),
        );
        if let Some(names) = meta.capability_names {
            let name_values = names
                .into_iter()
                .map(norito::json::Value::from)
                .collect::<Vec<_>>();
            meta_obj.insert(
                "capability_names".into(),
                norito::json::Value::Array(name_values),
            );
        }
        if let Some(topics) = meta.rendezvous_topics {
            let topic_values = topics
                .into_iter()
                .map(norito::json::Value::from)
                .collect::<Vec<_>>();
            meta_obj.insert(
                "rendezvous_topics".into(),
                norito::json::Value::Array(topic_values),
            );
        }
        if let Some(notes) = meta.notes {
            meta_obj.insert("notes".into(), norito::json::Value::from(notes));
        }
        if let Some(range) = meta.range_capability {
            let mut range_obj = norito::json::Map::new();
            range_obj.insert(
                "max_chunk_span".into(),
                norito::json::Value::from(range.max_chunk_span),
            );
            range_obj.insert(
                "min_granularity".into(),
                norito::json::Value::from(range.min_granularity),
            );
            range_obj.insert(
                "supports_sparse_offsets".into(),
                norito::json::Value::from(range.supports_sparse_offsets.unwrap_or(true)),
            );
            range_obj.insert(
                "requires_alignment".into(),
                norito::json::Value::from(range.requires_alignment.unwrap_or(false)),
            );
            range_obj.insert(
                "supports_merkle_proof".into(),
                norito::json::Value::from(range.supports_merkle_proof.unwrap_or(true)),
            );
            meta_obj.insert(
                "range_capability".into(),
                norito::json::Value::Object(range_obj),
            );
        }
        if let Some(budget) = meta.stream_budget {
            let mut budget_obj = norito::json::Map::new();
            budget_obj.insert(
                "max_in_flight".into(),
                norito::json::Value::from(budget.max_in_flight),
            );
            budget_obj.insert(
                "max_bytes_per_sec".into(),
                norito::json::Value::from(budget.max_bytes_per_sec),
            );
            if let Some(burst) = budget.burst_bytes {
                budget_obj.insert("burst_bytes".into(), norito::json::Value::from(burst));
            }
            meta_obj.insert(
                "stream_budget".into(),
                norito::json::Value::Object(budget_obj),
            );
        }
        if let Some(hints) = meta.transport_hints {
            let mut list = Vec::with_capacity(hints.len());
            for hint in hints {
                let mut obj = norito::json::Map::new();
                obj.insert("protocol".into(), norito::json::Value::from(hint.protocol));
                obj.insert(
                    "protocol_id".into(),
                    norito::json::Value::from(hint.protocol_id),
                );
                obj.insert("priority".into(), norito::json::Value::from(hint.priority));
                list.push(norito::json::Value::Object(obj));
            }
            meta_obj.insert("transport_hints".into(), norito::json::Value::Array(list));
        }

        obj.insert("metadata".into(), norito::json::Value::Object(meta_obj));
        norito::json::Value::Object(obj)
    }

    fn provider_input_from_metadata(
        metadata: &sorafs_car::multi_fetch::ProviderMetadata,
        path: &Path,
    ) -> LocalProviderInput {
        LocalProviderInput {
            name: metadata.provider_id.clone().unwrap(),
            path: path.to_path_buf(),
            max_concurrent: Some(2),
            weight: Some(1),
            metadata: Some(ProviderMetadataInput::from_metadata(metadata)),
        }
    }

    fn scoreboard_map_from_js(
        entries: &[iroha_js_host::JsScoreboardEntry],
    ) -> HashMap<String, (f64, String)> {
        entries
            .iter()
            .map(|entry| {
                (
                    entry.alias.clone(),
                    (entry.normalized_weight, entry.eligibility.clone()),
                )
            })
            .collect()
    }

    fn scoreboard_map_from_result(
        entries: Option<&[LocalFetchScoreboardEntry]>,
    ) -> HashMap<String, (f64, String)> {
        entries
            .into_iter()
            .flat_map(|entries| entries.iter())
            .map(|entry| {
                (
                    entry.alias.clone(),
                    (entry.normalized_weight, format!("{}", entry.eligibility)),
                )
            })
            .collect()
    }

    fn scoreboard_map_from_json(value: &norito::json::Value) -> HashMap<String, (f64, String)> {
        match value {
            norito::json::Value::Array(entries) => entries
                .iter()
                .filter_map(|entry| entry.as_object())
                .filter_map(|obj| {
                    let alias = obj.get("alias")?.as_str()?.to_owned();
                    let weight = obj.get("normalized_weight")?.as_f64()?;
                    let eligibility = obj.get("eligibility")?.as_str()?.to_owned();
                    Some((alias, (weight, eligibility)))
                })
                .collect(),
            _ => HashMap::new(),
        }
    }

    #[test]
    fn orchestrator_parity_across_runtimes() {
        let fixture = MultiPeerFixture::with_providers(2);
        let tempdir = tempdir().expect("tempdir");

        let plan = fixture.plan();
        let plan_json = chunk_fetch_specs_to_json(&plan);
        let plan_specs = plan.chunk_fetch_specs();
        let plan_json_string = chunk_fetch_specs_to_string(&plan_specs).expect("plan json string");

        let mut local_inputs = Vec::new();
        let mut js_inputs = Vec::new();
        let mut ffi_inputs = Vec::new();

        for (idx, metadata) in fixture.providers().iter().enumerate() {
            let name = metadata.provider_id.clone().expect("provider id");
            let path = tempdir.path().join(format!("{name}.bin"));
            std::fs::write(&path, &fixture.provider_payloads()[idx]).expect("write payload");

            local_inputs.push(provider_input_from_metadata(metadata, &path));
            js_inputs.push(JsLocalProviderSpec {
                name,
                path: path.to_string_lossy().to_string(),
                max_concurrent: Some(2),
                weight: Some(1),
                metadata: Some(js_metadata_from_provider(metadata)),
            });
            ffi_inputs.push(providers_json_from_metadata(metadata, &path));
        }

        let local_options = LocalFetchOptions {
            verify_digests: Some(true),
            verify_lengths: Some(true),
            use_scoreboard: Some(true),
            return_scoreboard: Some(true),
            ..Default::default()
        };

        let rust_start = Instant::now();
        let baseline = local_fetch::execute_local_fetch(
            &plan_json,
            local_inputs.clone(),
            local_options.clone(),
        )
        .expect("rust baseline");
        let rust_duration = rust_start.elapsed();

        let js_options = JsMultiFetchOptions {
            verify_digests: Some(true),
            verify_lengths: Some(true),
            use_scoreboard: Some(true),
            return_scoreboard: Some(true),
            ..Default::default()
        };
        let js_start = Instant::now();
        let js_result = iroha_js_host::sorafs_multi_fetch_local(
            plan_json_string.clone(),
            mem::take(&mut js_inputs),
            Some(js_options),
        )
        .expect("js fetch");
        let js_duration = js_start.elapsed();

        assert_eq!(
            js_result.payload.to_vec(),
            baseline.outcome.assemble_payload(),
            "JS payload mismatch"
        );

        let rust_scoreboard = scoreboard_map_from_result(baseline.scoreboard.as_deref());
        let js_scoreboard = scoreboard_map_from_js(js_result.scoreboard.as_deref().unwrap_or(&[]));
        assert_eq!(rust_scoreboard, js_scoreboard, "JS scoreboard mismatch");

        let rust_reports: HashMap<_, _> = baseline
            .outcome
            .provider_reports
            .iter()
            .map(|report| {
                (
                    report.provider.id().as_str().to_owned(),
                    (report.successes, report.failures, report.disabled),
                )
            })
            .collect();
        let js_reports: HashMap<_, _> = js_result
            .provider_reports
            .iter()
            .map(|report: &JsMultiFetchProviderReport| {
                (
                    report.provider.clone(),
                    (
                        usize::try_from(report.successes).expect("success count fits in usize"),
                        usize::try_from(report.failures).expect("failure count fits in usize"),
                        report.disabled,
                    ),
                )
            })
            .collect();
        assert_eq!(rust_reports, js_reports, "JS provider reports mismatch");

        let rust_receipts: Vec<_> = baseline
            .outcome
            .chunk_receipts
            .iter()
            .map(|receipt| {
                (
                    receipt.chunk_index,
                    receipt.provider.to_string(),
                    receipt.attempts,
                )
            })
            .collect();
        let js_receipts: Vec<_> = js_result
            .chunk_receipts
            .iter()
            .map(|receipt: &JsMultiFetchChunkReceipt| {
                (
                    usize::try_from(receipt.chunk_index).expect("chunk index fits in usize"),
                    receipt.provider.clone(),
                    usize::try_from(receipt.attempts).expect("attempts fit in usize"),
                )
            })
            .collect();
        assert_eq!(rust_receipts, js_receipts, "JS chunk receipts mismatch");

        let ffi_plan = plan_json_string.clone();
        let ffi_providers = norito::json::Value::Array(ffi_inputs.clone());
        let ffi_providers_str =
            norito::json::to_string(&ffi_providers).expect("providers json string");
        let ffi_options = norito::json!({
            "verify_digests": true,
            "verify_lengths": true,
            "use_scoreboard": true,
            "return_scoreboard": true,
        });
        let ffi_options_str = norito::json::to_string(&ffi_options).expect("options json string");

        let mut out_payload_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_payload_len: c_ulong = 0;
        let mut out_report_ptr: *mut u8 = std::ptr::null_mut();
        let mut out_report_len: c_ulong = 0;

        let ffi_start = Instant::now();
        let ffi_rc = {
            #[allow(unsafe_code)]
            unsafe {
                connect_norito_sorafs_local_fetch(
                    ffi_plan.as_ptr().cast(),
                    ffi_plan.len() as c_ulong,
                    ffi_providers_str.as_ptr().cast(),
                    ffi_providers_str.len() as c_ulong,
                    ffi_options_str.as_ptr().cast(),
                    ffi_options_str.len() as c_ulong,
                    &mut out_payload_ptr,
                    &mut out_payload_len,
                    &mut out_report_ptr,
                    &mut out_report_len,
                )
            }
        };
        let ffi_duration = ffi_start.elapsed();
        assert_eq!(ffi_rc, 0, "FFI fetch returned error {ffi_rc}");

        let ffi_payload = {
            #[allow(unsafe_code)]
            unsafe {
                let slice = std::slice::from_raw_parts(out_payload_ptr, out_payload_len as usize);
                let data = slice.to_vec();
                connect_norito_free(out_payload_ptr);
                data
            }
        };
        assert_eq!(
            ffi_payload,
            baseline.outcome.assemble_payload(),
            "FFI payload mismatch"
        );

        let ffi_report_json = {
            #[allow(unsafe_code)]
            unsafe {
                let slice = std::slice::from_raw_parts(out_report_ptr, out_report_len as usize);
                let json = String::from_utf8(slice.to_vec()).expect("utf-8 report");
                connect_norito_free(out_report_ptr);
                json
            }
        };

        let ffi_report_value: norito::json::Value =
            norito::json::from_str(&ffi_report_json).expect("ffi report json");
        let ffi_scoreboard = scoreboard_map_from_json(
            ffi_report_value
                .get("scoreboard")
                .unwrap_or(&norito::json::Value::Null),
        );
        assert_eq!(rust_scoreboard, ffi_scoreboard, "FFI scoreboard mismatch");

        let ffi_reports_map: HashMap<_, _> = ffi_report_value
            .get("provider_reports")
            .and_then(norito::json::Value::as_array)
            .unwrap()
            .iter()
            .map(|entry| {
                let obj = entry.as_object().expect("provider report object");
                let provider = obj
                    .get("provider")
                    .and_then(norito::json::Value::as_str)
                    .expect("provider id")
                    .to_owned();
                let successes = obj
                    .get("successes")
                    .and_then(norito::json::Value::as_u64)
                    .expect("successes") as usize;
                let failures = obj
                    .get("failures")
                    .and_then(norito::json::Value::as_u64)
                    .expect("failures") as usize;
                let disabled = obj
                    .get("disabled")
                    .and_then(norito::json::Value::as_bool)
                    .expect("disabled");
                (provider, (successes, failures, disabled))
            })
            .collect();
        assert_eq!(
            rust_reports, ffi_reports_map,
            "FFI provider reports mismatch"
        );

        let ffi_receipts: Vec<_> = ffi_report_value
            .get("chunk_receipts")
            .and_then(norito::json::Value::as_array)
            .unwrap()
            .iter()
            .map(|entry| {
                let obj = entry.as_object().expect("receipt object");
                let idx = obj
                    .get("chunk_index")
                    .and_then(norito::json::Value::as_u64)
                    .expect("chunk index") as usize;
                let provider = obj
                    .get("provider")
                    .and_then(norito::json::Value::as_str)
                    .expect("provider")
                    .to_owned();
                let attempts = obj
                    .get("attempts")
                    .and_then(norito::json::Value::as_u64)
                    .expect("attempts") as usize;
                (idx, provider, attempts)
            })
            .collect();
        assert_eq!(rust_receipts, ffi_receipts, "FFI chunk receipts mismatch");

        assert!(rust_duration > std::time::Duration::ZERO);
        assert!(js_duration > std::time::Duration::ZERO);
        assert!(ffi_duration > std::time::Duration::ZERO);

        let rust_ms = rust_duration.as_micros() as f64 / 1000.0;
        let js_ms = js_duration.as_micros() as f64 / 1000.0;
        let ffi_ms = ffi_duration.as_micros() as f64 / 1000.0;

        assert!(
            js_ms <= rust_ms * 10.0,
            "JS path is unexpectedly slow: {js_ms}ms vs Rust {rust_ms}ms"
        );
        assert!(
            ffi_ms <= rust_ms * 10.0,
            "FFI path is unexpectedly slow: {ffi_ms}ms vs Rust {rust_ms}ms"
        );
    }
}
