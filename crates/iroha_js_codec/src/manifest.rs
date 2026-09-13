//! Model-owned recursive schema checks at both native manifest codec boundaries.

use std::collections::BTreeMap;

use iroha_data_model::smart_contract::{
    entrypoint::{
        EntrypointValueTypeNodeV1, EntrypointValueTypeV1, MAX_ENTRYPOINT_RETURN_WORDS,
        is_canonical_kotodama_identifier,
    },
    manifest::{ContractErrorTypeDescriptor, ContractManifest},
};

use crate::{CodecError, CodecErrorKind, CodecResult};

fn invalid(reason: impl Into<String>) -> CodecError {
    CodecError::new(CodecErrorKind::InvalidArgument, reason)
}

/// Check the schema semantics that canonical Norito serialization alone cannot establish.
pub(super) fn validate_manifest_schemas(manifest: &ContractManifest) -> CodecResult<()> {
    if manifest
        .seiyaku_name
        .as_deref()
        .is_some_and(|name| !is_canonical_kotodama_identifier(name))
    {
        return Err(invalid(
            "manifest.seiyaku_name must be a canonical Kotodama V1 identifier",
        ));
    }
    if manifest
        .provenance
        .as_ref()
        .is_some_and(|provenance| provenance.signature.payload().iter().all(|byte| *byte == 0))
    {
        return Err(invalid(
            "manifest.provenance.signature must not be all zero",
        ));
    }
    let mut catalog = BTreeMap::new();
    for descriptor in manifest.error_types.as_deref().unwrap_or_default() {
        if !descriptor.validate()
            || catalog
                .insert(descriptor.identity.as_str(), descriptor)
                .is_some()
        {
            return Err(invalid(
                "manifest.error_types must contain unique canonical error schemas",
            ));
        }
    }
    for (index, entrypoint) in manifest
        .entrypoints
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let context = format!("manifest.entrypoints[{index}]");
        match (
            entrypoint.params.as_slice(),
            entrypoint.argument_schema.as_ref(),
        ) {
            ([], None) => {}
            (params, Some(schema))
                if schema.validate()
                    && schema.fields.len() == params.len()
                    && schema.fields.iter().zip(params).all(|(field, parameter)| {
                        field.name == parameter.name
                            && field.ty.canonical_type_name().as_deref()
                                == Some(parameter.type_name.as_str())
                    }) =>
            {
                for field in &schema.fields {
                    validate_value_schema(&field.ty, &catalog, &context)?;
                }
            }
            _ => {
                return Err(invalid(format!(
                    "{context}.argument_schema must be a canonical V1 schema matching every declared parameter"
                )));
            }
        }
        let (Some(return_type), Some(return_schema)) = (
            entrypoint.return_type.as_deref(),
            entrypoint.return_schema.as_ref(),
        ) else {
            return Err(invalid(format!(
                "{context} requires return_type and return_schema, including () and Unit"
            )));
        };
        validate_value_schema(return_schema, &catalog, &context)?;
        if return_schema.canonical_type_name().as_deref() != Some(return_type)
            || return_schema
                .word_count()
                .is_none_or(|words| words > MAX_ENTRYPOINT_RETURN_WORDS)
        {
            return Err(invalid(format!(
                "{context}.return_schema must match return_type within the V1 {MAX_ENTRYPOINT_RETURN_WORDS}-word return window"
            )));
        }
    }
    Ok(())
}

fn validate_value_schema(
    schema: &EntrypointValueTypeV1,
    catalog: &BTreeMap<&str, &ContractErrorTypeDescriptor>,
    context: &str,
) -> CodecResult<()> {
    // The model owns flat tape completeness, recursive limits, cursor key types,
    // exact reserved query views/QueryPage/StatePage, and nominal struct identities.
    if !schema.validate() {
        return Err(invalid(format!(
            "{context} contains an invalid canonical V1 entrypoint value schema"
        )));
    }
    for node in &schema.nodes {
        if let EntrypointValueTypeNodeV1::Error(descriptor) = node {
            if catalog.get(descriptor.identity.as_str()).copied() != Some(descriptor) {
                return Err(invalid(format!(
                    "{context} boundary error schema does not match its error_types catalog"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        isi::{InstructionBox, smart_contract_code::RegisterSmartContractCode},
        smart_contract::entrypoint::{EntrypointValueKindV1, EntrypointValueTypeNodeV1 as Node},
    };
    use norito::json::{self, Value};

    use super::*;
    use crate::{
        decode_instruction_archive, decode_instruction_frame, encode_instruction_archive,
        encode_instruction_frame,
    };

    const BASE_FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../javascript/iroha_js/test/fixtures/contract_manifest_v1.json"
    ));
    const NOMINAL_FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kotodama/nominal_errors_v1.json"
    ));
    const STRUCT_FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/kotodama/exported_structs_v1.json"
    ));

    fn fixture_manifest(overlay: Option<&str>) -> ContractManifest {
        let base: Value = json::from_json(BASE_FIXTURE).expect("Rust manifest fixture");
        let mut manifest = base.get("manifest").expect("manifest").clone();
        if let Some(overlay) = overlay {
            let extra: Value = json::from_json(overlay).expect("shared nominal fixture");
            manifest.as_object_mut().expect("manifest object").extend(
                extra
                    .get("manifest")
                    .and_then(Value::as_object)
                    .expect("overlay object")
                    .clone(),
            );
        }
        json::from_value(manifest).expect("current model manifest")
    }

    fn instruction_json(manifest: &ContractManifest) -> String {
        json::to_json(&norito::json!({ "RegisterSmartContractCode": { "manifest": manifest } }))
            .expect("manifest instruction JSON")
    }

    fn assert_roundtrip(manifest: &ContractManifest) {
        validate_manifest_schemas(manifest).expect("current schema preflight");
        let source = instruction_json(manifest);
        let expected: Value = json::from_json(&source).expect("instruction JSON");
        let frame = encode_instruction_frame(&source).expect("native frame");
        let archive = encode_instruction_archive(&source).expect("native archive");
        for decoded in [
            decode_instruction_frame(&frame).expect("native frame decode"),
            decode_instruction_archive(&archive).expect("native archive decode"),
        ] {
            assert_eq!(
                json::from_json::<Value>(&decoded).expect("decoded JSON"),
                expected
            );
            assert_eq!(
                encode_instruction_frame(&decoded).expect("frame re-encode"),
                frame
            );
            assert_eq!(
                encode_instruction_archive(&decoded).expect("archive re-encode"),
                archive
            );
        }
    }

    fn assert_rejected_everywhere(manifest: &ContractManifest) {
        assert!(validate_manifest_schemas(manifest).is_err());
        let source = instruction_json(manifest);
        assert!(
            encode_instruction_frame(&source).is_err(),
            "invalid manifest frame accepted"
        );
        assert!(
            encode_instruction_archive(&source).is_err(),
            "invalid manifest archive accepted"
        );
        // Serialize directly through the model to bypass the JSON preflight. The
        // frame/archive remain canonical and checksummed, so rejection exercises
        // the native decoded-manifest schema boundary rather than CRC handling.
        let instruction: InstructionBox = RegisterSmartContractCode {
            manifest: manifest.clone(),
        }
        .into();
        let frame = norito::encode_canonical(&instruction).expect("canonical adversarial frame");
        let mut archive = Vec::new();
        norito::codec::encode_adaptive_into(&instruction, &mut archive)
            .expect("canonical adversarial archive");
        assert!(
            decode_instruction_frame(&frame).is_err(),
            "invalid manifest frame decoded"
        );
        assert!(
            decode_instruction_archive(&archive).is_err(),
            "invalid manifest archive decoded"
        );
    }

    fn with_return_schema(schema: EntrypointValueTypeV1) -> ContractManifest {
        let mut manifest = fixture_manifest(None);
        let entrypoint = &mut manifest.entrypoints.as_mut().expect("entrypoints")[0];
        entrypoint.return_type = schema.canonical_type_name();
        entrypoint.return_schema = Some(schema);
        manifest
    }

    fn query_view() -> EntrypointValueTypeV1 {
        json::from_value(norito::json!({ "nodes": [
            { "kind": "Struct", "value": { "name": "AccountView", "fields": ["id", "metadata"] } },
            { "kind": "Leaf", "value": { "kind": "AccountId", "value": null } },
            { "kind": "Leaf", "value": { "kind": "Json", "value": null } }
        ] }))
        .expect("canonical AccountView")
    }

    fn query_page() -> EntrypointValueTypeV1 {
        json::from_value(norito::json!({ "nodes": [
            { "kind": "Struct", "value": { "name": "QueryPage", "fields": ["items", "next_offset"] } },
            { "kind": "List", "value": { "capacity": 64 } },
            { "kind": "Struct", "value": { "name": "AccountView", "fields": ["id", "metadata"] } },
            { "kind": "Leaf", "value": { "kind": "AccountId", "value": null } },
            { "kind": "Leaf", "value": { "kind": "Json", "value": null } },
            { "kind": "Option", "value": null },
            { "kind": "Leaf", "value": { "kind": "Int", "value": null } }
        ] })).expect("canonical QueryPage")
    }

    #[test]
    fn native_manifest_preserves_unit_nominal_cursor_struct_and_reserved_layouts() {
        for overlay in [None, Some(NOMINAL_FIXTURE), Some(STRUCT_FIXTURE)] {
            assert_roundtrip(&fixture_manifest(overlay));
        }
        for schema in [query_view(), query_page()] {
            assert_roundtrip(&with_return_schema(schema));
        }
        let mut nodes = vec![Node::Tuple(MAX_ENTRYPOINT_RETURN_WORDS as u16)];
        nodes.extend(vec![
            Node::Leaf(EntrypointValueKindV1::Int);
            MAX_ENTRYPOINT_RETURN_WORDS
        ]);
        assert_roundtrip(&with_return_schema(EntrypointValueTypeV1 { nodes }));
    }

    #[test]
    fn native_manifest_rejects_absent_unit_descriptors_on_encode_and_decode() {
        for missing in [1_u8, 2, 3] {
            let mut manifest = fixture_manifest(None);
            let entrypoint = &mut manifest.entrypoints.as_mut().expect("entrypoints")[0];
            if missing & 1 != 0 {
                entrypoint.return_type = None;
            }
            if missing & 2 != 0 {
                entrypoint.return_schema = None;
            }
            assert_rejected_everywhere(&manifest);
        }
        let base: Value = json::from_json(BASE_FIXTURE).expect("fixture");
        for fields in [
            vec!["return_type"],
            vec!["return_schema"],
            vec!["return_type", "return_schema"],
        ] {
            for omitted in [false, true] {
                let mut manifest = base.get("manifest").expect("manifest").clone();
                let entrypoint = manifest
                    .get_mut("entrypoints")
                    .and_then(Value::as_array_mut)
                    .expect("entrypoints")[0]
                    .as_object_mut()
                    .expect("entrypoint object");
                for field in &fields {
                    if omitted {
                        entrypoint.remove(*field);
                    } else {
                        entrypoint.insert((*field).to_owned(), Value::Null);
                    }
                }
                let source = json::to_json(
                    &norito::json!({ "RegisterSmartContractCode": { "manifest": manifest } }),
                )
                .expect("JSON");
                assert!(encode_instruction_frame(&source).is_err());
                assert!(encode_instruction_archive(&source).is_err());
            }
        }
    }

    #[test]
    fn native_manifest_rejects_noncanonical_tapes_reserved_shapes_and_return_names() {
        let mut malformed = fixture_manifest(None);
        malformed.entrypoints.as_mut().unwrap()[0]
            .return_schema
            .as_mut()
            .unwrap()
            .nodes
            .push(Node::Leaf(EntrypointValueKindV1::Bool));
        assert_rejected_everywhere(&malformed);
        let mut forged = with_return_schema(query_view());
        forged.entrypoints.as_mut().unwrap()[0]
            .return_schema
            .as_mut()
            .unwrap()
            .nodes[1] = Node::Leaf(EntrypointValueKindV1::Bool);
        assert_rejected_everywhere(&forged);
        let mut page = with_return_schema(query_page());
        if let Node::List(list) = &mut page.entrypoints.as_mut().unwrap()[0]
            .return_schema
            .as_mut()
            .unwrap()
            .nodes[1]
        {
            list.capacity = 32;
        } else {
            panic!("QueryPage list");
        }
        assert_rejected_everywhere(&page);
        let mut cursor = fixture_manifest(Some(NOMINAL_FIXTURE));
        cursor.entrypoints.as_mut().unwrap()[1]
            .return_schema
            .as_mut()
            .unwrap()
            .nodes[1] = Node::StateCursor(EntrypointValueKindV1::Json);
        assert_rejected_everywhere(&cursor);
        let mut wrong_name = fixture_manifest(None);
        wrong_name.entrypoints.as_mut().unwrap()[0].return_type = Some("bool".to_owned());
        assert_rejected_everywhere(&wrong_name);
        let mut nodes = vec![Node::Tuple((MAX_ENTRYPOINT_RETURN_WORDS + 1) as u16)];
        nodes.extend(vec![
            Node::Leaf(EntrypointValueKindV1::Int);
            MAX_ENTRYPOINT_RETURN_WORDS + 1
        ]);
        assert_rejected_everywhere(&with_return_schema(EntrypointValueTypeV1 { nodes }));
    }

    #[test]
    fn native_manifest_rejects_parameter_and_nominal_catalog_substitution() {
        let mut manifest = fixture_manifest(Some(STRUCT_FIXTURE));
        manifest.entrypoints.as_mut().unwrap()[0].params[0].type_name = "bool".to_owned();
        assert_rejected_everywhere(&manifest);
        let mut manifest = fixture_manifest(Some(STRUCT_FIXTURE));
        manifest.error_types = None;
        assert_rejected_everywhere(&manifest);
        let mut manifest = fixture_manifest(Some(STRUCT_FIXTURE));
        manifest.error_types.as_mut().unwrap()[0].variants[0].name = "Changed".to_owned();
        assert_rejected_everywhere(&manifest);
        let mut manifest = fixture_manifest(Some(STRUCT_FIXTURE));
        let duplicate = manifest.error_types.as_ref().unwrap()[0].clone();
        manifest.error_types.as_mut().unwrap().push(duplicate);
        assert_rejected_everywhere(&manifest);
        let mut manifest = fixture_manifest(None);
        manifest.seiyaku_name = Some("Injected<Type>".to_owned());
        assert_rejected_everywhere(&manifest);
        let mut manifest = fixture_manifest(None);
        let base: Value = json::from_json(BASE_FIXTURE).expect("fixture");
        let mut provenance: iroha_data_model::smart_contract::manifest::ManifestProvenance =
            json::from_value(base.get("signed_provenance").expect("provenance").clone())
                .expect("valid provenance");
        provenance.signature = iroha_crypto::Signature::from_bytes(&[0; 64]);
        manifest.provenance = Some(provenance);
        assert_rejected_everywhere(&manifest);
    }

    #[test]
    fn manifest_and_upload_cancellation_reject_generic_json_envelopes() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use iroha_data_model::isi::smart_contract_code::CancelSmartContractCodeUpload;

        let manifest = fixture_manifest(None);
        let mut malformed = manifest.clone();
        malformed.entrypoints.as_mut().unwrap()[0].return_schema = None;
        let instructions: [InstructionBox; 3] = [
            RegisterSmartContractCode { manifest }.into(),
            RegisterSmartContractCode {
                manifest: malformed,
            }
            .into(),
            CancelSmartContractCodeUpload {
                code_hash: iroha_crypto::Hash::new(b"manifest codec cancellation"),
            }
            .into(),
        ];
        for instruction in instructions {
            let frame = norito::encode_canonical(&instruction).unwrap();
            let source = json::to_json(&Value::String(STANDARD.encode(frame))).unwrap();
            for encode in [encode_instruction_frame, encode_instruction_archive] {
                let error = encode(&source).expect_err("generic string envelope must be rejected");
                assert_eq!(error.kind(), CodecErrorKind::InvalidArgument);
            }
        }
        let canonical: Value = json::from_json(&instruction_json(&fixture_manifest(None))).unwrap();
        for nested in [false, true] {
            let mut extra = canonical.clone();
            let fields = if nested {
                extra.get_mut("RegisterSmartContractCode").unwrap()
            } else {
                &mut extra
            };
            fields
                .as_object_mut()
                .unwrap()
                .insert("unexpected".to_owned(), Value::Null);
            let source = json::to_json(&extra).unwrap();
            assert!(encode_instruction_frame(&source).is_err());
            assert!(encode_instruction_archive(&source).is_err());
        }
    }
}
