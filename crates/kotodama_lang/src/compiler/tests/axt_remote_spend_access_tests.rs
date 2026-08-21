#[test]
fn codegen_rejects_noncanonical_or_invalid_literal_remote_spend_intents() {
    use crate::axt::{RemoteSpendIntent, SpendOp};
    use iroha_data_model::nexus::DataSpaceId;
    let invalid = RemoteSpendIntent {
        asset_dsid: DataSpaceId::new(7),
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([
                0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
            ])
            .expect("valid AXT fixture asset id"),
            kind: String::new(),
            from: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".to_owned(),
            to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_owned(),
            amount: Some("1".parse().expect("canonical quantity")),
        },
    };
    let valid = RemoteSpendIntent {
        asset_dsid: DataSpaceId::new(7),
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([
                0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
            ])
            .expect("valid AXT fixture asset id"),
            kind: "transfer".to_owned(),
            from: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV".to_owned(),
            to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_owned(),
            amount: Some("1".parse().expect("canonical quantity")),
        },
    };
    let alternate = alternate_norito_hex(&valid);
    assert_ne!(alternate, canonical_norito_hex(&valid));
    let handle = canonical_norito_hex(&sample_asset_handle());
    for intent in [canonical_norito_hex(&invalid), alternate] {
        let error = compile_with_injected_ir(vec![
            ir::Instr::DataRef {
                dest: ir::Temp(0),
                kind: ir::DataRefKind::AssetHandle,
                value: handle.clone(),
            },
            ir::Instr::DataRef {
                dest: ir::Temp(1),
                kind: ir::DataRefKind::NoritoBytes,
                value: intent,
            },
            ir::Instr::UseAssetHandle {
                handle: ir::Temp(0),
                intent: ir::Temp(1),
                proof: None,
            },
        ])
        .expect_err("literal remote spend intents must fail closed during full codegen");
        assert!(
            error.contains("invalid AXT remote spend intent literal")
                && error.contains("canonical, context-valid RemoteSpendIntent frame"),
            "unexpected compiler error: {error}"
        );
    }
}
#[test]
fn asset_handle_access_hints_read_the_issuer_signed_asset_definition() {
    use crate::axt::{RemoteSpendIntent, SpendOp};
    use iroha_data_model::nexus::DataSpaceId;

    let handle = sample_asset_handle();
    let intent = RemoteSpendIntent {
        asset_dsid: DataSpaceId::new(7),
        op: SpendOp {
            asset_definition_id: handle.asset_definition_id.clone(),
            kind: "transfer".to_owned(),
            from: handle.subject.account.clone(),
            to: "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_owned(),
            amount: Some("1".parse().expect("canonical quantity")),
        },
    };
    let mut access = AccessSets::default();
    super::add_asset_handle_access(&mut access, &handle, &intent);

    assert!(
        access
            .reads
            .contains(&super::key_asset_def(&handle.asset_definition_id)),
        "the policy read for the exact issuer-signed asset must be declared"
    );
}
