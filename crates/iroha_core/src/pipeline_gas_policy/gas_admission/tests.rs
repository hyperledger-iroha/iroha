//! Direct gas ingress, precedence, original custody and physical refusal controls.

use super::*;
use crate::test_allocations::{allocations_during, refuse_one_layout_during};
use iroha_data_model::parameter::{CustomParameter, Parameter};
use norito::core::{DecodeAttemptErrorKind, DecodeBudgetContext};
use std::str::FromStr as _;

fn asset(seed: u8) -> String {
    let mut bytes = [seed; 16];
    bytes[6] = 0x40;
    bytes[8] = 0x80;
    AssetDefinitionId::from_uuid_bytes(bytes)
        .unwrap()
        .canonical_address()
}
fn parameters(id: &str, source: &str) -> Parameters {
    let mut value = Parameters::default();
    value.set_parameter(Parameter::Custom(CustomParameter::new(
        id.parse().unwrap(),
        Json::from_str(source).unwrap(),
    )));
    value
}
fn record(asset: &str, twap: Option<&str>) -> CommittedGasRate {
    CommittedGasRate {
        asset: asset.to_owned(),
        units_per_gas: 0,
        twap_local_per_xor: twap.map(str::to_owned),
        liquidity_profile: None,
        volatility_class: None,
    }
}
fn rates_source(values: &[CommittedGasRate]) -> Json {
    Json::new(
        values
            .iter()
            .map(|value| json::to_value(value).unwrap())
            .collect::<Vec<_>>(),
    )
}
fn rate_parameters(values: &[CommittedGasRate]) -> Parameters {
    parameters(RATES, rates_source(values).get())
}
fn limits(bytes: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn core_error(error: GasAdmissionError) -> norito::Error {
    match error {
        GasAdmissionError::Decode(error) => error,
        other => panic!("unexpected allocation refusal: {other}"),
    }
}

#[test]
fn admitted_gas_protocol_defaults_charge_only_original_technical_text() {
    let source = Parameters::default();
    let expected = Gas::default();
    let bytes = expected.tech_account_id.len();
    let pool = AllocationBudget::new(bytes);
    let foreign = AllocationBudget::new(bytes);
    let owner = AdmittedGas::try_from_parameters(&source, &pool).unwrap();
    assert_eq!(owner.get(), &expected);
    assert_eq!(pool.reserved_bytes(), bytes);
    assert_eq!(pool.peak_reserved_bytes(), bytes);
    assert!(owner.belongs_to(&pool));
    assert!(!owner.belongs_to(&foreign));
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(bytes - 1);
    let requests = allocations_during(|| {
        assert!(matches!(
            AdmittedGas::try_from_parameters(&source, &pool),
            Err(GasAdmissionError::Allocation(
                ChargedBufferError::Admission(_)
            ))
        ));
    });
    assert_eq!(requests, 0);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn admitted_gas_complete_projection_matches_ordinary_and_preserves_source() {
    let mut expected = Gas::default();
    expected.accepted_assets = vec![asset(1), asset(2)];
    expected.units_per_gas = vec![GasRate {
        asset: asset(2),
        units_per_gas: 0,
        twap_local_per_xor: "12.125".parse().unwrap(),
        liquidity: GasLiquidity::Tier3,
        volatility: GasVolatility::Dislocated,
    }];
    let mut source = Parameters::default();
    for value in super::super::parameters_for_testing(expected.clone()) {
        source.set_parameter(value);
    }
    let before = norito::json::to_json(&source).unwrap();
    let pool = AllocationBudget::new(1 << 20);
    let owner = AdmittedGas::try_from_parameters(&source, &pool).unwrap();
    assert_eq!(owner.get(), &expected);
    assert_eq!(
        owner.get(),
        &super::super::from_parameters(&source).unwrap()
    );
    assert_eq!(norito::json::to_json(&source).unwrap(), before);
    assert!(owner.belongs_to(&pool));
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug, crate::json_macros::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CapturedRate {
    asset: String,
    units_per_gas: u64,
    twap_local_per_xor: Option<String>,
    liquidity_profile: Option<String>,
    volatility_class: Option<String>,
}
fn parse_raw(source: &str, pool: &AllocationBudget) -> Result<RawRate<Text>, GasAdmissionError> {
    let mut parser = json::Parser::new(source);
    parser.preflight_document()?;
    parser.skip_ws();
    let value = raw_rate(&mut parser, &Admitted(pool))?;
    parser.finish_document()?;
    Ok(value)
}

#[test]
fn gas_record_shared_kernel_matches_captured_derive_fields_and_error_precedence() {
    let good = json::to_json(&record(&asset(3), Some(" +001.2500 "))).unwrap();
    let cases = [
        good.clone(),
        "{}".to_owned(),
        "[]".to_owned(),
        good.replace("\"units_per_gas\":0", "\"units_per_gas\":true"),
        good.replacen('}', ",\"asset\":false}", 1),
        good.replacen('}', ",\"unknown\":true}", 1),
        good.replace("\"liquidity_profile\":null,", ""),
        good.replace("\"asset\"", "\"a\\u0073set\""),
        good.replace("\"volatility_class\":null", "\"volatility_class\":[]"),
        format!("{good} false"),
    ];
    for source in cases {
        let pool = AllocationBudget::new(1 << 20);
        match (
            json::from_str::<CapturedRate>(&source),
            parse_raw(&source, &pool),
        ) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.asset, b.asset.as_ref());
                assert_eq!(a.units_per_gas, b.units_per_gas);
                assert_eq!(
                    a.twap_local_per_xor.as_deref(),
                    b.twap_local_per_xor.as_ref().map(AsRef::as_ref)
                );
                assert_eq!(
                    a.liquidity_profile.as_deref(),
                    b.liquidity_profile.as_ref().map(AsRef::as_ref)
                );
                assert_eq!(
                    a.volatility_class.as_deref(),
                    b.volatility_class.as_ref().map(AsRef::as_ref)
                );
            }
            (Err(a), Err(b)) => assert_eq!(
                format!("{:?}", a.into_core_error()),
                format!("{:?}", core_error(b)),
                "{source}"
            ),
            (a, _) => panic!("shared parser disagrees with captured derive: {a:?}, {source}"),
        }
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn gas_rate_optional_fields_preserve_absence_null_and_duplicate_precedence() {
    let address = asset(16);
    let fields = ["twap_local_per_xor", "liquidity_profile", "volatility_class"];
    for mask in 0_u8..8 {
        let mut entries = vec![
            format!(r#""asset":"{address}""#),
            r#""units_per_gas":0"#.to_owned(),
        ];
        for (index, field) in fields.iter().enumerate() {
            if mask & (1 << index) != 0 {
                entries.push(format!(r#""{field}":null"#));
            }
        }
        let source = format!("{{{}}}", entries.join(","));
        let captured = json::from_str::<CapturedRate>(&source).unwrap();
        let pool = AllocationBudget::new(1 << 20);
        let actual = parse_raw(&source, &pool).unwrap();
        assert_eq!(captured.asset, address);
        assert_eq!(actual.asset.as_ref(), address);
        assert_eq!(captured.units_per_gas, 0);
        assert_eq!(actual.units_per_gas, 0);
        assert!(captured.twap_local_per_xor.is_none());
        assert!(captured.liquidity_profile.is_none());
        assert!(captured.volatility_class.is_none());
        assert!(actual.twap_local_per_xor.is_none());
        assert!(actual.liquidity_profile.is_none());
        assert!(actual.volatility_class.is_none());
        drop(actual);
        assert_eq!(pool.reserved_bytes(), 0);
    }
    for field in fields {
        // A present null is still present: reject the duplicate before trying
        // to parse its invalid boolean value or applying any semantic defaults.
        let source = format!(
            r#"{{"asset":"{address}","units_per_gas":0,"{field}":null,"{field}":false}}"#
        );
        let expected = json::from_str::<CapturedRate>(&source)
            .unwrap_err()
            .into_core_error();
        assert!(matches!(
            &expected,
            norito::Error::Json(json::Error::DuplicateField { field: actual }) if actual == field
        ));
        let pool = AllocationBudget::new(1 << 20);
        let Err(actual) = parse_raw(&source, &pool) else {
            panic!("duplicate optional field was accepted: {source}");
        };
        assert_eq!(format!("{:?}", core_error(actual)), format!("{expected:?}"));
        assert_eq!(pool.reserved_bytes(), 0);
    }
    for (source, missing) in [
        ("{}".to_owned(), "asset"),
        (format!(r#"{{"asset":"{address}"}}"#), "units_per_gas"),
    ] {
        let expected = json::from_str::<CapturedRate>(&source)
            .unwrap_err()
            .into_core_error();
        assert!(matches!(
            &expected,
            norito::Error::Json(json::Error::MissingField { field }) if field == missing
        ));
        let pool = AllocationBudget::new(1 << 20);
        let Err(actual) = parse_raw(&source, &pool) else {
            panic!("missing required field was accepted: {source}");
        };
        assert_eq!(format!("{:?}", core_error(actual)), format!("{expected:?}"));
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn gas_rates_preserve_complete_raw_parse_then_semantic_record_order() {
    let first = asset(4);
    let second = asset(5);
    let mut malformed_later =
        json::to_json(&vec![record(&first, Some("bad")), record(&second, None)]).unwrap();
    let marker = malformed_later.rfind("\"units_per_gas\":0").unwrap();
    malformed_later.replace_range(
        marker..marker + "\"units_per_gas\":0".len(),
        "\"units_per_gas\":false",
    );
    let cases = [
        malformed_later,
        json::to_json(&vec![
            record(&first, None),
            record(&first, None),
            record("bad", None),
        ])
        .unwrap(),
        json::to_json(&vec![record(&first, Some("bad")), record(&first, None)]).unwrap(),
        json::to_json(&vec![
            record("bad", None),
            record(&first, None),
            record(&first, None),
        ])
        .unwrap(),
        json::to_json(&vec![record(&first, Some("0"))]).unwrap(),
        json::to_json(&vec![record(&first, Some("-1"))]).unwrap(),
    ];
    for source in cases {
        // This fixture authors a value, while Json ingress requires canonical
        // lexical text. Keep array order and all values when encoding it.
        let canonical = Json::new(json::from_str::<json::Value>(&source).unwrap());
        let p = parameters(RATES, canonical.get());
        let expected = super::super::from_parameters(&p).unwrap_err();
        let pool = AllocationBudget::new(1 << 20);
        let actual = core_error(AdmittedGas::try_from_parameters(&p, &pool).unwrap_err());
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "{source}");
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn gas_asset_duplicate_order_and_canonical_parser_need_no_rendered_copy() {
    let first = asset(6);
    let second = asset(7);
    let requests = allocations_during(|| {
        super::super::canonical_asset(&first, ACCEPTED_ASSETS).unwrap();
    });
    assert_eq!(requests, 0);
    let noncanonical = format!("1{first}");
    assert!(super::super::canonical_asset(&noncanonical, ACCEPTED_ASSETS).is_err());
    for values in [
        vec![first.clone(), second.clone(), first.clone(), "bad".into()],
        vec!["bad".into(), first.clone(), first.clone()],
        vec![noncanonical],
    ] {
        let p = parameters(ACCEPTED_ASSETS, Json::new(values).get());
        let expected = super::super::from_parameters(&p).unwrap_err();
        let pool = AllocationBudget::new(1 << 20);
        let actual = core_error(AdmittedGas::try_from_parameters(&p, &pool).unwrap_err());
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn gas_decimal_and_classification_use_original_permissive_source_relation() {
    for text in [None, Some(" +001.2500 "), Some(".125"), Some("1.000")] {
        for liquidity in [
            None,
            Some("tier1"),
            Some("tier2"),
            Some("tier3"),
            Some("invalid"),
        ] {
            for volatility in [
                None,
                Some("stable"),
                Some("elevated"),
                Some("dislocated"),
                Some("invalid"),
            ] {
                let mut value = record(&asset(8), text);
                value.liquidity_profile = liquidity.map(str::to_owned);
                value.volatility_class = volatility.map(str::to_owned);
                let p = rate_parameters(&[value]);
                let pool = AllocationBudget::new(1 << 20);
                match (
                    super::super::from_parameters(&p),
                    AdmittedGas::try_from_parameters(&p, &pool),
                ) {
                    (Ok(a), Ok(b)) => assert_eq!(&a, b.get()),
                    (Err(a), Err(b)) => {
                        assert_eq!(format!("{a:?}"), format!("{:?}", core_error(b)))
                    }
                    _ => panic!("ordinary/admitted semantic disagreement"),
                }
                assert_eq!(pool.reserved_bytes(), 0);
            }
        }
    }
}

#[test]
fn gas_exact_peak_refusal_and_retry_leave_original_parameters_unchanged() {
    let p = rate_parameters(&[
        record(&asset(9), Some("123456789012345678901.125")),
        record(&asset(10), None),
    ]);
    let before = json::to_json(&p).unwrap();
    let measure = AllocationBudget::new(1 << 20);
    drop(AdmittedGas::try_from_parameters(&p, &measure).unwrap());
    let peak = measure.peak_reserved_bytes();
    let pool = AllocationBudget::new(peak - 1);
    assert!(matches!(
        AdmittedGas::try_from_parameters(&p, &pool),
        Err(GasAdmissionError::Allocation(
            ChargedBufferError::Admission(_)
        ))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(json::to_json(&p).unwrap(), before);
    pool.set_limit_bytes(peak);
    let value = AdmittedGas::try_from_parameters(&p, &pool).unwrap();
    assert_eq!(value.get(), &super::super::from_parameters(&p).unwrap());
    assert_eq!(pool.peak_reserved_bytes(), peak);
    drop(value);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn gas_actual_allocator_refusal_covers_raw_final_and_ledger_arrays() {
    let assets = Json::new(vec![asset(11), asset(12), asset(13)]);
    let rates = rates_source(&[
        record(&asset(11), Some("12.5")),
        record(&asset(12), None),
        record(&asset(13), None),
    ]);
    let pool = AllocationBudget::new(1 << 20);
    for layout in [
        Layout::array::<String>(3).unwrap(),
        Layout::array::<AllocationCharge>(3).unwrap(),
        Layout::array::<(Option<[u8; 16]>, usize)>(3).unwrap(),
        Layout::array::<u8>(asset(11).len()).unwrap(),
    ] {
        let (result, refused) = refuse_one_layout_during(layout, || decode_assets(&assets, &pool));
        assert!(refused, "{layout:?}");
        assert!(
            matches!(result, Err(GasAdmissionError::Allocation(ChargedBufferError::Allocator { requested_bytes })) if requested_bytes == layout.size())
        );
        assert_eq!(pool.reserved_bytes(), 0);
        drop(decode_assets(&assets, &pool).unwrap());
    }
    for layout in [
        Layout::array::<RawRate<Text>>(3).unwrap(),
        Layout::array::<GasRate>(3).unwrap(),
        Layout::array::<[AllocationCharge; 2]>(3).unwrap(),
    ] {
        let (result, refused) = refuse_one_layout_during(layout, || decode_rates(&rates, &pool));
        assert!(refused, "{layout:?}");
        assert!(
            matches!(result, Err(GasAdmissionError::Allocation(ChargedBufferError::Allocator { requested_bytes })) if requested_bytes == layout.size())
        );
        assert_eq!(pool.reserved_bytes(), 0);
        drop(decode_rates(&rates, &pool).unwrap());
    }
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
#[allow(unsafe_code)]
fn gas_move_keeps_original_allocation_pointers_and_refunds_after_owner_drop() {
    let p = rate_parameters(&[record(&asset(14), Some("12.5"))]);
    let pool = AllocationBudget::new(1 << 20);
    let owner = AdmittedGas::try_from_parameters(&p, &pool).unwrap();
    let pointers = (
        owner.get().tech_account_id.as_ptr(),
        owner.get().units_per_gas.as_ptr(),
        owner.get().units_per_gas[0].asset.as_ptr(),
    );
    let retained = pool.reserved_bytes();
    struct Closed {
        value: Gas,
        custody: GasCustody,
    }
    let mut moved = None;
    let requests = allocations_during(|| {
        // SAFETY: immediately pair exact original value and custody; no mutator escapes.
        let (value, custody) = unsafe { owner.into_allocation_parts() };
        moved = Some(Closed { value, custody });
    });
    assert_eq!(requests, 0);
    let moved = moved.unwrap();
    assert_eq!(
        pointers,
        (
            moved.value.tech_account_id.as_ptr(),
            moved.value.units_per_gas.as_ptr(),
            moved.value.units_per_gas[0].asset.as_ptr()
        )
    );
    assert!(moved.custody.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), retained);
    drop(moved);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn gas_original_decoder_scope_survives_projection_and_cannot_be_replayed() {
    let p = rate_parameters(&[record(&asset(15), Some("12.5"))]);
    let pool = AllocationBudget::new(1 << 20);
    let context = DecodeBudgetContext::new(limits(0));
    let mut original = None;
    let attempt = context
        .with(|| {
            norito::core::classify_decode_attempt(|| {
                AdmittedGas::try_from_parameters(&p, &pool).map_err(|e| {
                    let error = core_error(e);
                    let norito::Error::ScopedDecodeResource(scope) = &error else {
                        panic!("original scoped decoder cause required");
                    };
                    original = Some(scope.clone());
                    error
                })
            })
        })
        .unwrap_err();
    assert_eq!(attempt.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(pool.reserved_bytes(), 0);
    let error = attempt.into_local_refusal().unwrap().into_error();
    let norito::Error::ScopedDecodeResource(scope) = &error else {
        panic!("same original cause required");
    };
    assert_eq!(scope, &original.unwrap());
    let replay = norito::core::classify_decode_attempt::<()>(|| Err(error)).unwrap_err();
    assert_eq!(replay.kind(), DecodeAttemptErrorKind::Invalid);
    assert!(replay.local_refusal().is_none());
    drop(AdmittedGas::try_from_parameters(&p, &pool).unwrap());
    assert_eq!(pool.reserved_bytes(), 0);
}
