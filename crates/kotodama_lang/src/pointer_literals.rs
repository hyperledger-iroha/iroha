//! Shared decoding and validation of encoded pointer-constructor literals.
//!
//! These checks validate canonical frames and their internal consistency. Runtime admission
//! still owns authoritative State, issuer signatures, finality, expiry, and replay checks.

/// Decode the compiler's hexadecimal byte carrier or preserve ordinary UTF-8 text.
pub(crate) fn decode_hex_or_raw_bytes(raw: &str) -> Result<Vec<u8>, String> {
    if let Some(trimmed) = raw.strip_prefix("0x") {
        if trimmed.len() % 2 == 0 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut out = Vec::with_capacity(trimmed.len() / 2);
            for chunk in trimmed.as_bytes().chunks(2) {
                let byte_str = std::str::from_utf8(chunk)
                    .map_err(|e| format!("invalid hex literal `{raw}`: {e}"))?;
                let byte = u8::from_str_radix(byte_str, 16)
                    .map_err(|e| format!("invalid hex literal `{raw}`: {e}"))?;
                out.push(byte);
            }
            return Ok(out);
        }
        return Err(format!(
            "invalid hex literal `{raw}`: expected even-length hex digits"
        ));
    }
    Ok(raw.as_bytes().to_vec())
}

/// Decode one canonical AXT descriptor and validate its declared data spaces and touches.
pub(crate) fn axt_descriptor(raw: &str) -> Result<crate::axt::AxtDescriptor, String> {
    let bytes = decode_hex_or_raw_bytes(raw)
        .map_err(|error| format!("invalid AxtDescriptor literal `{raw}`: {error}"))?;
    let value = ivm_abi::codec::decode_canonical_norito(&bytes).map_err(|error| {
        format!("invalid AxtDescriptor literal `{raw}`: cannot decode ({error})")
    })?;
    crate::axt::validate_descriptor(&value).map_err(|error| {
        format!("invalid AxtDescriptor literal `{raw}`: invalid descriptor ({error})")
    })?;
    Ok(value)
}

/// Decode one canonical anchored spend and validate its internal signed-payload binding.
pub(crate) fn axt_anchored_spend(
    raw: &str,
) -> Result<iroha_data_model::nexus::AxtAnchoredSpendV1, String> {
    let bytes = decode_hex_or_raw_bytes(raw)
        .map_err(|error| format!("invalid AxtAnchoredSpendV1 literal `{raw}`: {error}"))?;
    let value: iroha_data_model::nexus::AxtAnchoredSpendV1 =
        ivm_abi::codec::decode_canonical_norito(&bytes).map_err(|error| {
            format!("invalid AxtAnchoredSpendV1 literal `{raw}`: cannot decode ({error})")
        })?;
    value.issuer_payload_v1().map_err(|error| {
        format!("invalid AxtAnchoredSpendV1 literal `{raw}`: inconsistent signed binding ({error})")
    })?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagnostic::DiagnosticPhase,
        session::{CompileRequest, CompilerSession},
    };

    fn fixtures() -> (
        crate::axt::AxtDescriptor,
        iroha_data_model::nexus::AxtAnchoredSpendV1,
    ) {
        let descriptor: norito::json::Value = norito::json::from_str(include_str!(
            "../../iroha_data_model/tests/fixtures/axt_descriptor_multi_ds.json"
        ))
        .expect("current AXT descriptor fixture");
        let spend: norito::json::Value = norito::json::from_str(include_str!(
            "../../iroha_data_model/tests/fixtures/axt_envelope_multi_ds.json"
        ))
        .expect("current signed-spend fixture");
        (
            norito::json::from_value(descriptor["descriptor"].clone()).unwrap(),
            norito::json::from_value(spend["spends"]["happy"][0].clone()).unwrap(),
        )
    }

    fn encoded<T: norito::NoritoSerialize>(value: &T) -> String {
        format!(
            "0x{}",
            hex::encode(ivm_abi::codec::encode_canonical_norito(value).unwrap())
        )
    }

    fn source(descriptor: &str, spend: &str, style: &str) -> String {
        let (declarations, locals, descriptor_arg, spend_arg) = match style {
            "literal" => (
                String::new(),
                String::new(),
                format!("\"{descriptor}\""),
                format!("\"{spend}\""),
            ),
            "const" => (
                format!(
                    "const string DESCRIPTOR = \"{descriptor}\"; const string SPEND = \"{spend}\";"
                ),
                String::new(),
                "DESCRIPTOR".into(),
                "SPEND".into(),
            ),
            "local" => (
                String::new(),
                format!(
                    "let descriptor_literal = \"{descriptor}\"; let spend_literal = \"{spend}\";"
                ),
                "descriptor_literal".into(),
                "spend_literal".into(),
            ),
            "private" => (
                String::new(),
                String::new(),
                "descriptor_literal".into(),
                "spend_literal".into(),
            ),
            _ => unreachable!("known fixture style"),
        };
        let body = format!(
            r#"{locals}
            let descriptor = AxtDescriptor::parse({descriptor_arg});
            let spend = AxtAnchoredSpendV1::parse({spend_arg});
            axt::begin(descriptor: descriptor);
            axt::stage_anchored_spend(spend: spend);
            axt::commit();"#
        );
        let functions = if style == "private" {
            format!(
                r#"fn stage(string descriptor_literal, string spend_literal) {{ {body} }}
                kotoage fn run() authorize(UseAxt) {{ stage(descriptor_literal: "{descriptor}", spend_literal: "{spend}"); }}"#
            )
        } else {
            format!("kotoage fn run() authorize(UseAxt) {{ {body} }}")
        };
        format!("seiyaku PointerLiterals {{ permission UseAxt; {declarations} {functions} }}")
    }

    #[test]
    fn pointer_constructors_reject_external_runtime_parameters_during_check() {
        let session = CompilerSession::default();
        let mut cases = [
            "AccountId",
            "AssetDefinitionId",
            "AssetId",
            "NftId",
            "Name",
            "DomainId",
            "DataSpaceId",
        ].into_iter().map(|ty| (ty, format!(
                "seiyaku Runtime {{ view fn parse(string raw) authorize(anyone) -> {ty} {{ return {ty}::parse(raw); }} }}"
            ))).collect::<Vec<_>>();
        let (descriptor, spend) = fixtures();
        let descriptor = encoded(&descriptor);
        let spend = encoded(&spend);
        for (ty, literal) in [
            ("AxtDescriptor", &descriptor),
            ("AxtAnchoredSpendV1", &spend),
        ] {
            cases.push((
                ty,
                source(&descriptor, &spend, "literal")
                    .replace("kotoage fn run()", "kotoage fn run(string raw)")
                    .replace(
                        &format!("{ty}::parse(\"{literal}\")"),
                        &format!("{ty}::parse(raw)"),
                    ),
            ));
        }
        for (ty, source) in cases {
            let request = CompileRequest {
                source: &source,
                source_name: Some("runtime_pointer.ko"),
            };
            let built = session
                .build(request)
                .expect_err("runtime text cannot build");
            let checked = session
                .check(request)
                .expect_err("check must reject the runtime constructor argument");
            assert_eq!(checked, built, "{ty}");
            assert_eq!(checked.diagnostics.len(), 1, "{ty}: {checked:?}");
            let diagnostic = &checked.diagnostics[0];
            assert_eq!(diagnostic.code, "E_POINTER_LITERAL_REQUIRED", "{ty}");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
            assert!(diagnostic.help.is_some());
            let span = diagnostic.primary_span.as_ref().expect("argument span");
            assert_eq!(span.source.as_deref(), Some("runtime_pointer.ko"));
            let range = span.byte_range.expect("argument byte range");
            assert_eq!(&source[range.start as usize..range.end as usize], "raw");
        }
    }

    #[test]
    fn json_runtime_parameter_preserves_its_literal_diagnostic() {
        let session = CompilerSession::default();
        let request = CompileRequest {
            source: "seiyaku Runtime { view fn parse(string raw) authorize(anyone) -> Json { return Json::parse(raw); } }",
            source_name: Some("runtime_json.ko"),
        };
        let checked = session.check(request).expect_err("Json requires a literal");
        let built = session.build(request).expect_err("Json requires a literal");
        assert_eq!(checked, built);
        assert_eq!(checked.diagnostics.len(), 1);
        assert_eq!(checked.diagnostics[0].code, "E_JSON_LITERAL_REQUIRED");
        assert_eq!(checked.diagnostics[0].phase, DiagnosticPhase::Semantic);
    }

    #[test]
    fn pointer_constructors_preserve_literals_specialization_and_typed_parameters() {
        let session = CompilerSession::default();
        for source in [
            r#"seiyaku Direct { view fn run() authorize(anyone) -> Name { return Name::parse("key"); } }"#,
            r#"seiyaku Constant { const string KEY = "key"; view fn run() authorize(anyone) -> Name { return Name::parse(KEY); } }"#,
            r#"seiyaku Local { view fn run() authorize(anyone) -> Name { let key = "key"; return Name::parse(key); } }"#,
            r#"seiyaku Private {
                fn parse(string raw) -> Name { return Name::parse(raw); }
                view fn run() authorize(anyone) -> Name { return parse(raw: "key"); }
            }"#,
            "seiyaku Typed { view fn run(Name raw) authorize(anyone) -> Name { return raw; } }",
        ] {
            let request = CompileRequest {
                source,
                source_name: Some("valid_pointer.ko"),
            };
            session.check(request).expect("valid pointer source checks");
            session.build(request).expect("valid pointer source builds");
        }
    }

    #[test]
    fn encoded_pointer_bytes_preserve_the_carrier_contract() {
        for (raw, expected) in [
            ("0x", Vec::new()),
            ("0x00aF", vec![0, 175]),
            ("text", b"text".to_vec()),
            ("投票", "投票".as_bytes().to_vec()),
        ] {
            assert_eq!(decode_hex_or_raw_bytes(raw).unwrap(), expected);
        }
        for raw in ["0x0", "0xgg", "0x 00", "0x投票"] {
            assert!(
                decode_hex_or_raw_bytes(raw)
                    .unwrap_err()
                    .contains("expected even-length hex digits")
            );
        }
    }

    #[test]
    fn axt_pointer_literals_check_and_build_reject_invalid_payloads_identically() {
        let (descriptor, spend) = fixtures();
        let valid = [encoded(&descriptor), encoded(&spend)];
        let invalid_descriptor = crate::axt::AxtDescriptor {
            dsids: Vec::new(),
            touches: Vec::new(),
        };
        let mut invalid_spend = spend;
        invalid_spend.authorization.expiry_slot = 0;
        let structurally_invalid = [encoded(&invalid_descriptor), encoded(&invalid_spend)];
        let session = CompilerSession::default();
        for kind in 0..2 {
            let trailing = format!("{}00", valid[kind]);
            for raw in [
                "0x0",
                "0xgg",
                "0x",
                "not-a-frame",
                &trailing,
                &structurally_invalid[kind],
            ] {
                for style in ["literal", "const"] {
                    let mut args = valid.clone();
                    args[kind] = raw.to_owned();
                    let source = source(&args[0], &args[1], style);
                    let request = CompileRequest {
                        source: &source,
                        source_name: Some("pointer.ko"),
                    };
                    let checked = session
                        .check(request)
                        .expect_err("check must reject the malformed pointer literal");
                    let built = session
                        .build(request)
                        .expect_err("build must reject the malformed pointer literal");
                    assert_eq!(checked, built, "kind={kind}, style={style}");
                    assert_eq!(checked.diagnostics.len(), 1, "{checked:?}");
                    let diagnostic = &checked.diagnostics[0];
                    assert_eq!(diagnostic.code, "E_INVALID_POINTER_LITERAL");
                    assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
                    assert!(
                        diagnostic.help.is_some(),
                        "pointer errors must be explainable"
                    );
                    let span = diagnostic
                        .primary_span
                        .as_ref()
                        .expect("pointer argument span");
                    assert_eq!(span.source.as_deref(), Some("pointer.ko"));
                    let range = span.byte_range.expect("exact pointer argument range");
                    let expected = if style == "literal" {
                        format!("\"{raw}\"")
                    } else if kind == 0 {
                        "DESCRIPTOR".into()
                    } else {
                        "SPEND".into()
                    };
                    assert_eq!(&source[range.start as usize..range.end as usize], expected);
                }
            }
        }
    }

    #[test]
    fn axt_pointer_literals_preserve_valid_bindings_and_specialized_calls() {
        let (descriptor, spend) = fixtures();
        let session = CompilerSession::default();
        for style in ["literal", "const", "local", "private"] {
            let source = source(&encoded(&descriptor), &encoded(&spend), style);
            let request = CompileRequest {
                source: &source,
                source_name: Some("pointer.ko"),
            };
            session
                .check(request)
                .unwrap_or_else(|error| panic!("{style} check: {error:?}"));
            let built = session
                .build(request)
                .unwrap_or_else(|error| panic!("{style} build: {error:?}"));
            assert!(!built.artifact.is_empty());
        }
    }
}
