/// Native custody, real recipient encryption and authenticated committed-receipt regressions.
mod ironwood {
    use super::*;
    use iroha_core_privacy::privacy_engines::orchard::{
        ironwood_wallet as wallet, orchard_empty_root_v1,
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        asset::{
            AssetBalancePolicy, AssetBalanceScope, AssetDefinition, AssetDefinitionId, AssetId,
        },
        domain::Domain,
        isi::{Mint, privacy::BootstrapPrivacyIronwoodPoolV1},
        privacy::{
            OrchardHalo2ActionsStatementV1, PrivacyIronwoodPoolBootstrapV1, PrivacyPoolIdV1,
            PrivacyRuntimeStatementV1, PrivacyValueBalanceDirectionV1,
        },
        transaction::FeePaymentIntent,
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;
    use norito::json::Value;
    use pyo3::Python;
    const PROTOCOL: &str = "ironwood-halo2-actions-v1";
    const OPERATION: &str = "ironwood_note_action_v1";
    const MERCHANT_KEY: [u8; 32] = [0x62; 32];
    const MEMO: [u8; 512] = [0x31; 512];
    const POOL: [u8; 32] = [0xd2; 32];
    fn json(value: &Value) -> Vec<u8> {
        norito::json::to_json(value).unwrap().into_bytes()
    }
    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("ironwood", "universal").unwrap(),
            "merchant".parse().unwrap(),
        )
    }
    fn public_action(pool: [u8; 32]) -> Value {
        norito::json!({"anchor_epoch": 1u64, "anchor_hex": (hex::encode(orchard_empty_root_v1())), "asset_definition_id": (asset().to_string()), "expiry_height": 100u64, "minimum_action_count": 1u8, "pool_id_hex": (hex::encode(pool)), "public_balance_scope": "global", "public_debit_decimal": "17", "changes": [], "outputs": [{"recipient_hex": (hex::encode(wallet::recipient_address_v1(MERCHANT_KEY, false, 0).unwrap())), "value_decimal": "17", "memo_hex": (hex::encode(MEMO))}]})
    }
    fn public_intent(action: &Value, ordered: bool) -> Value {
        let mut value: Value = norito::json::from_slice(CANONICAL_JINDO_PUBLIC_INTENT).unwrap();
        let object = value.as_object_mut().unwrap();
        for field in ["algorithm_id", "protocol_id"] {
            object.insert(field.into(), Value::String(PROTOCOL.into()));
        }
        object.insert("operation_schema".into(), Value::String(OPERATION.into()));
        object.insert("public_action".into(), action.clone());
        for field in ["selected_features", "selected_criteria"] {
            let flags = object.get_mut(field).unwrap().as_object_mut().unwrap();
            for bit in ["hide_amount", "hide_sender", "hide_receiver"] {
                flags.insert(bit.into(), Value::Bool(true));
            }
        }
        if ordered {
            object.insert("orchard_contract_calls".into(), orchard_ordered::calls());
        }
        value
    }
    fn imported(
        action: &Value,
        network: [u8; 32],
        ordered: bool,
    ) -> (WitnessVault, WitnessLease, WitnessBinding, Value, Value) {
        let public = public_intent(action, ordered);
        let mut bound = binding();
        bound.protocol = PROTOCOL.into();
        bound.network_id = network;
        bound.profile_digest = ironwood_profile_binding_v1().unwrap();
        bound.public_intent_digest = canonical_public_intent_digest(&json(&public));
        let (_, _, authority) = test_signer();
        let mut bundle = b"IPWB\x01".to_vec();
        put_text(&mut bundle, &bound.signer);
        put_text(&mut bundle, &authority.to_string());
        put_text(&mut bundle, PROTOCOL);
        put_text(&mut bundle, OPERATION);
        put_bytes_u32(&mut bundle, &json(action)).unwrap();
        bundle.extend_from_slice(&TEST_SIGNER_SEED);
        // The payer receives only the public merchant address. There is no merchant key or
        // previously shielded payer note anywhere in the payer's owner-only bundle.
        put_bytes_u32(&mut bundle, br#"{"changes":[],"spends":[]}"#).unwrap();
        let directory = TempDir::new().unwrap();
        let path = credential_file(&directory, "payer", &bundle);
        let mut vault = WitnessVault::default();
        let lease = vault
            .import_credential(ImportRequest {
                credential_path: path,
                binding: bound.clone(),
                ttl_millis: 120_000,
            })
            .unwrap();
        let mut plan: Value = norito::json::from_slice(&canonical_execution_plan()).unwrap();
        let object = plan.as_object_mut().unwrap();
        object.insert("protocol_id".into(), Value::String(PROTOCOL.into()));
        object.insert("operation_schema".into(), Value::String(OPERATION.into()));
        object.insert("network_id_hex".into(), Value::String(hex::encode(network)));
        object.insert("public_action".into(), action.clone());
        if ordered {
            object.insert("orchard_contract_calls".into(), orchard_ordered::calls());
        }
        (vault, lease, bound, public, plan)
    }
    fn execute(action: &Value, network: [u8; 32], ordered: bool) -> SignedTransaction {
        let (mut vault, lease, bound, public, plan) = imported(action, network, ordered);
        let payload =
            encode_execute_payload(lease.handle, &bound, &json(&public), &json(&plan)).unwrap();
        let encoded = encode_frame(
            &AuthenticatedFrame {
                kind: CommandKind::Execute,
                sequence: 1,
                payload: payload.clone(),
            },
            &KEY,
        )
        .unwrap();
        let decoded = decode_frame(&encoded, &KEY).unwrap();
        assert_eq!(encode_frame(&decoded, &KEY).unwrap(), encoded);
        let result = match dispatch(&mut vault, decoded.kind, &decoded.payload) {
            CommandResponse::SignedAction(value) => value,
            CommandResponse::Error(error) => panic!("Ironwood worker failed: {error:?}"),
            _ => panic!("wrong worker response"),
        };
        assert!(matches!(
            dispatch(&mut vault, CommandKind::Execute, &payload),
            CommandResponse::Error(WorkerError::UnknownHandle)
        ));
        let signed =
            SignedTransaction::decode_all_versioned(&result.versioned_signed_transaction).unwrap();
        let mapping = result.ironwood_output_mapping.as_ref().unwrap();
        assert_eq!(
            mapping.direct_instruction_index,
            ironwood_direct_index(&signed).unwrap()
        );
        let incoming = wallet::incoming_viewing_key_v1(&MERCHANT_KEY, false).unwrap();
        let received =
            wallet::decrypt_outputs_with_incoming_viewing_key_v1(&statement(&signed), &incoming)
                .unwrap();
        assert_eq!(
            mapping.output_action_indices,
            vec![received[0].action_index()]
        );
        assert_eq!(encode_response(CommandResponse::SignedAction(result))[0], 7);
        signed
    }
    fn statement(signed: &SignedTransaction) -> OrchardHalo2ActionsStatementV1 {
        let (_, submission) = signed
            .privacy_transaction_intent_binding_if_present_v1()
            .unwrap()
            .unwrap();
        let PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(statement) =
            submission.runtime_envelope().statement
        else {
            panic!("Ironwood statement")
        };
        statement
    }
    #[test]
    fn ironwood_local_ipc_appends_ordinals_and_uses_bounded_authenticated_responses() {
        let commands = [
            CommandKind::Ping,
            CommandKind::Import,
            CommandKind::Inspect,
            CommandKind::Cancel,
            CommandKind::Execute,
            CommandKind::ImportPrivateSettlement,
            CommandKind::InspectPrivateSettlement,
            CommandKind::CancelPrivateSettlement,
            CommandKind::ProvePrivateSettlement,
            CommandKind::DeriveIronwoodAddress,
            CommandKind::ScanIronwoodReceipts,
            CommandKind::ExportIronwoodViewingKey,
            CommandKind::ScanIronwoodReceiptsWithViewingKey,
            CommandKind::ImportIronwoodDeposit,
            CommandKind::ImportIronwoodDepositSeed,
        ];
        for (index, command) in commands.into_iter().enumerate() {
            let ordinal = u8::try_from(index + 1).unwrap();
            assert_eq!(command as u8, ordinal);
            assert!(CommandKind::try_from(ordinal).unwrap() == command);
        }
        assert_eq!(encode_response(CommandResponse::Pong), [0]);
        assert_eq!(encode_response(CommandResponse::Cancelled), [2]);
        let directory = TempDir::new().unwrap();
        let path = credential_file(&directory, "merchant-native-key", &MERCHANT_KEY);
        let mut request = Vec::new();
        put_text(&mut request, path.to_str().unwrap());
        request.extend_from_slice(&0u32.to_be_bytes());
        request.push(0);
        let mut input = encode_frame(
            &AuthenticatedFrame {
                kind: CommandKind::DeriveIronwoodAddress,
                sequence: 1,
                payload: request,
            },
            &KEY,
        )
        .unwrap();
        input.extend_from_slice(
            &encode_frame(
                &AuthenticatedFrame {
                    kind: CommandKind::ScanIronwoodReceipts,
                    sequence: 2,
                    payload: Vec::new(),
                },
                &KEY,
            )
            .unwrap(),
        );
        let mut output = Vec::new();
        run_pipe_session(&mut IoCursor::new(input), &mut output, Zeroizing::new(KEY)).unwrap();
        let mut stream = IoCursor::new(output);
        let address = read_frame(&mut stream, &KEY).unwrap().unwrap();
        assert_eq!(address.kind as u8, 10);
        assert_eq!(address.sequence, 1);
        assert_eq!(address.payload[0], 6);
        assert_eq!(address.payload.len(), 1 + 4 + 75);
        assert_eq!(
            u32::from_be_bytes(address.payload[1..5].try_into().unwrap()),
            75
        );
        assert_eq!(
            &address.payload[5..48],
            &wallet::recipient_address_v1(MERCHANT_KEY, false, 0).unwrap()
        );
        let scan = read_frame(&mut stream, &KEY).unwrap().unwrap();
        assert_eq!(scan.kind as u8, 11);
        assert_eq!(scan.sequence, 2);
        assert_eq!(
            scan.payload,
            encode_response(CommandResponse::Error(WorkerError::InvalidPayload))
        );
        assert!(read_frame(&mut stream, &KEY).unwrap().is_none());
        let response = encode_response(CommandResponse::IronwoodPublic(vec![
            0x61;
            MAX_FRAME_BYTES - 8192
        ]));
        assert_eq!(response[0], 6);
        assert!(
            encode_frame(
                &AuthenticatedFrame {
                    kind: CommandKind::ScanIronwoodReceipts,
                    sequence: 1,
                    payload: response
                },
                &KEY
            )
            .is_ok()
        );
    }

    #[test]
    fn ironwood_viewing_credentials_are_restricted_scoped_and_canonical() {
        let directory = TempDir::new().unwrap();
        let spending = credential_file(&directory, "spending", &MERCHANT_KEY);
        let mut exports = Vec::new();
        for internal in [false, true] {
            let destination = directory.path().join(if internal {
                "internal.ivk"
            } else {
                "external.ivk"
            });
            let mut payload = Vec::new();
            put_text(&mut payload, spending.to_str().unwrap());
            put_text(&mut payload, destination.to_str().unwrap());
            payload.push(u8::from(internal));
            assert_eq!(ironwood_export_viewing_key(&payload).unwrap().len(), 32);
            let material = read_credential_file(&destination).unwrap();
            assert_eq!(material.len(), 65);
            assert_eq!(material[0], u8::from(internal));
            let incoming: &[u8; 64] = material[1..].try_into().unwrap();
            assert_eq!(
                wallet::recipient_from_incoming_viewing_key_v1(incoming, 7).unwrap(),
                wallet::recipient_address_v1(MERCHANT_KEY, internal, 7).unwrap()
            );
            assert!(!material.windows(32).any(|bytes| bytes == MERCHANT_KEY));
            assert!(ironwood_export_viewing_key(&payload).is_err());
            assert_eq!(*read_credential_file(&destination).unwrap(), *material);
            exports.push(material);
        }
        assert_ne!(exports[0][1..], exports[1][1..]);
        for (name, bytes) in [
            ("spending-not-viewing", vec![0x62; 32]),
            ("zero-ivk", vec![0; 65]),
            ("wrong-scope", vec![2; 65]),
        ] {
            let path = credential_file(&directory, name, &bytes);
            let mut payload = Vec::new();
            put_text(&mut payload, path.to_str().unwrap());
            put_bytes_u32(&mut payload, b"{}").unwrap();
            assert_eq!(
                ironwood_scan_with_viewing_key(&payload).unwrap_err(),
                WorkerError::InvalidExecutionBundle
            );
        }
    }

    #[test]
    fn ironwood_native_deposit_import_and_exact_registry_original_round_trip() {
        use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};
        let original_prefix = chain_discriminant();
        let selected_prefix = if original_prefix == 369 { 73 } else { 369 };
        let caller_scope = ChainDiscriminantGuard::enter(selected_prefix);
        let action = public_action(POOL);
        let (_, _, mut bound, mut public, mut plan) = imported(&action, [1; 32], true);
        let mut calls = orchard_ordered::calls();
        for field in ["before", "after"] {
            calls
                .as_object_mut()
                .unwrap()
                .get_mut(field)
                .unwrap()
                .as_array_mut()
                .unwrap()
                .truncate(1);
        }
        public
            .as_object_mut()
            .unwrap()
            .insert("orchard_contract_calls".into(), calls.clone());
        plan.as_object_mut()
            .unwrap()
            .insert("orchard_contract_calls".into(), calls);
        bound.public_intent_digest = canonical_public_intent_digest(&json(&public));
        let approved_authority = plan["authority"].as_str().unwrap().to_owned();
        drop(caller_scope);
        assert_eq!(chain_discriminant(), original_prefix);
        let directory = TempDir::new().unwrap();
        let path = credential_file(&directory, "payer-seed", &TEST_SIGNER_SEED);
        let mut payload = Vec::new();
        put_text(&mut payload, path.to_str().unwrap());
        encode_binding(&mut payload, &bound);
        payload.extend_from_slice(&120_000u64.to_be_bytes());
        payload.extend_from_slice(&selected_prefix.to_be_bytes());
        put_bytes_u32(&mut payload, &json(&action)).unwrap();
        let mut vault = WitnessVault::default();
        let file_lease = ironwood_import_deposit(&mut vault, &payload).unwrap();
        vault.cancel(file_lease.handle, &bound).unwrap();
        let mut pipe_payload = Zeroizing::new(Vec::new());
        encode_binding(&mut pipe_payload, &bound);
        pipe_payload.extend_from_slice(&120_000u64.to_be_bytes());
        pipe_payload.extend_from_slice(&selected_prefix.to_be_bytes());
        put_bytes_u32(&mut pipe_payload, &json(&action)).unwrap();
        pipe_payload.reserve_exact(32);
        pipe_payload.extend_from_slice(&TEST_SIGNER_SEED);
        let lease = ironwood_import_deposit_seed(&mut vault, &pipe_payload).unwrap();
        assert_eq!(lease.chain_discriminant, Some(selected_prefix));
        assert_eq!(chain_discriminant(), original_prefix);
        let response = encode_response(CommandResponse::Lease(lease.clone()));
        assert!(
            response
                .windows(approved_authority.len())
                .any(|part| part == approved_authority.as_bytes())
        );
        let mut wrong_plan = plan.clone();
        wrong_plan.as_object_mut().unwrap().insert(
            "authority".into(),
            Value::String(lease.manifest.authority.to_string()),
        );
        let wrong_execute =
            encode_execute_payload(lease.handle, &bound, &json(&public), &json(&wrong_plan))
                .unwrap();
        assert!(matches!(
            execute_native_action_v1(&mut vault, decode_execute_payload(&wrong_execute).unwrap()),
            Err(WorkerError::WrongBinding)
        ));
        assert!(vault.inspect(lease.handle, &bound).is_ok());
        let execute =
            encode_execute_payload(lease.handle, &bound, &json(&public), &json(&plan)).unwrap();
        let signed =
            execute_native_action_v1(&mut vault, decode_execute_payload(&execute).unwrap())
                .unwrap();
        assert!(vault.inspect(lease.handle, &bound).is_err());
        assert_eq!(signed.authority, approved_authority);
        assert_eq!(chain_discriminant(), original_prefix);
        let model =
            SignedTransaction::decode_all_versioned(&signed.versioned_signed_transaction).unwrap();
        let inspection = crate::ironwood_inspection::inspected(&model, selected_prefix).unwrap();
        let inspected: Value = norito::json::from_str(&inspection).unwrap();
        assert_eq!(inspected["direct_instruction_index"].as_u64(), Some(1));
        assert_eq!(
            inspected["value_balance"],
            norito::json!({"direction": {"direction": "IntoPool", "value": null}, "amount": 17u128})
        );
        assert_eq!(
            inspected["transaction_hash_hex"].as_str(),
            Some(hex::encode(model.hash().as_ref()).as_str())
        );
        assert!(
            inspected["unsigned_payload_json"]
                .as_str()
                .unwrap()
                .contains("Batch")
        );
        let notes = wallet::decrypt_outputs_v1(&statement(&model), MERCHANT_KEY, false).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].value(), 17);
        let mut wrong = action.clone();
        wrong
            .as_object_mut()
            .unwrap()
            .insert("changes".into(), norito::json!([{}]));
        let mut bad = Vec::new();
        put_text(&mut bad, path.to_str().unwrap());
        encode_binding(&mut bad, &bound);
        bad.extend_from_slice(&120_000u64.to_be_bytes());
        bad.extend_from_slice(&selected_prefix.to_be_bytes());
        put_bytes_u32(&mut bad, &json(&wrong)).unwrap();
        assert!(ironwood_import_deposit(&mut vault, &bad).is_err());
    }

    #[test]
    fn ironwood_bootstrap_export_is_typed_json_and_does_not_submit() {
        let (_, _, reserve) = test_signer();
        let prefix = iroha_data_model::account::address::chain_discriminant();
        let output = crate::ironwood_pool_cut::ironwood_pool_bootstrap_instruction_json_v1_py(
            &hex::encode(POOL),
            &asset().to_string(),
            &reserve.to_string(),
            prefix,
        )
        .unwrap();
        let instructions: Vec<iroha_data_model::isi::InstructionBox> =
            norito::json::from_str(&output).unwrap();
        assert_eq!(instructions.len(), 1);
        let bootstrap = &instructions[0]
            .as_any()
            .downcast_ref::<BootstrapPrivacyIronwoodPoolV1>()
            .unwrap()
            .bootstrap;
        assert_eq!(
            bootstrap,
            &PrivacyIronwoodPoolBootstrapV1::new(
                PrivacyPoolIdV1::new(POOL),
                asset(),
                AssetBalanceScope::Global,
                reserve.clone()
            )
            .unwrap()
        );
        assert!(
            crate::ironwood_pool_cut::ironwood_pool_bootstrap_instruction_json_v1_py(
                &hex::encode([0; 32]),
                &asset().to_string(),
                &reserve.to_string(),
                prefix
            )
            .is_err()
        );
    }

    #[test]
    fn ironwood_import_gates_ttl_and_capacity_before_opening_seed() {
        let action = public_action(POOL);
        let (_, _, bound, _, _) = imported(&action, [1; 32], false);
        for (ttl, capacity, expected) in [
            (0u64, 128, WorkerError::InvalidTtl),
            (120_000, 0, WorkerError::CapacityExceeded),
        ] {
            let mut payload = Vec::new();
            encode_binding(&mut payload, &bound);
            payload.extend_from_slice(&ttl.to_be_bytes());
            payload.extend_from_slice(&753u16.to_be_bytes());
            put_bytes_u32(&mut payload, &json(&action)).unwrap();
            let mut cursor = Cursor::new(&payload);
            let mut vault = WitnessVault::default();
            vault.maximum_handles = capacity;
            let result = ironwood_import_deposit_from(
                &mut vault,
                &mut cursor,
                || panic!("seed opened before resource/TTL gate"),
                false,
            );
            assert!(matches!(result, Err(error) if error == expected));
        }
    }

    #[test]
    fn ironwood_excess_outputs_refuse_before_consuming_the_lease() {
        for (payments, changes) in [(1, 2), (2, 1)] {
            let mut action = public_action(POOL);
            let output = action.get("outputs").unwrap().as_array().unwrap()[0].clone();
            action.as_object_mut().unwrap().insert(
                "outputs".into(),
                Value::Array(vec![output.clone(); payments]),
            );
            action
                .as_object_mut()
                .unwrap()
                .insert("changes".into(), Value::Array(vec![output; changes]));
            let (mut vault, lease, bound, public, plan) = imported(&action, [1; 32], false);
            let payload =
                encode_execute_payload(lease.handle, &bound, &json(&public), &json(&plan)).unwrap();
            assert!(matches!(
                execute_native_action_v1(&mut vault, decode_execute_payload(&payload).unwrap()),
                Err(WorkerError::InvalidExecutionBundle)
            ));
            assert!(
                vault.inspect(lease.handle, &bound).is_ok(),
                "{payments}+{changes} must leave the single-use lease intact"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn ironwood_viewing_export_failure_is_atomic_and_retryable() {
        use std::io::{Error, ErrorKind, Write};
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("viewing-key");
        assert!(
            write_ironwood_viewing_credential(&path, |file| {
                file.write_all(b"partial secret")?;
                Err(Error::new(ErrorKind::Other, "injected write failure"))
            })
            .is_err()
        );
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        write_ironwood_viewing_credential(&path, |file| file.write_all(&[7; 65])).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [7; 65]);
        assert!(write_ironwood_viewing_credential(&path, |file| file.write_all(&[8; 65])).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), [7; 65]);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn ironwood_owner_bundle_derives_payer_change_without_recipient_key() {
        let mut action = public_action(POOL);
        action
            .as_object_mut()
            .unwrap()
            .insert("public_debit_decimal".into(), Value::String("46".into()));
        action.as_object_mut().unwrap().insert("changes".into(), norito::json!([{
            "recipient_hex": (hex::encode(wallet::recipient_address_v1([0x61; 32], true, 7).unwrap())),
            "value_decimal": "29", "memo_hex": (hex::encode([0; 512]))
        }]));
        let (_, _, authority) = test_signer();
        let public = json(&action);
        let mut bundle = b"IPWB\x01".to_vec();
        for field in [
            "alice@wonderland",
            &authority.to_string(),
            PROTOCOL,
            OPERATION,
        ] {
            put_text(&mut bundle, field);
        }
        put_bytes_u32(&mut bundle, &public).unwrap();
        bundle.extend_from_slice(&TEST_SIGNER_SEED);
        let change = norito::json!({"spends": [], "changes": [{"spending_key_hex": (hex::encode([0x61;32])), "scope": "internal", "diversifier_index": 7u32, "value_decimal": "29", "memo_hex": (hex::encode([0;512]))}]});
        for (field, replacement) in [
            ("scope", Value::String("external".into())),
            ("diversifier_index", Value::from(8u64)),
            ("value_decimal", Value::String("28".into())),
            ("memo_hex", Value::String(hex::encode([1; 512]))),
            ("spending_key_hex", Value::String(hex::encode([0x64; 32]))),
        ] {
            let mut changed = change.clone();
            changed
                .as_object_mut()
                .unwrap()
                .get_mut("changes")
                .unwrap()
                .as_array_mut()
                .unwrap()[0]
                .as_object_mut()
                .unwrap()
                .insert(field.into(), replacement);
            let mut wrong = bundle.clone();
            put_bytes_u32(&mut wrong, &json(&changed)).unwrap();
            assert!(
                decode_privacy_wallet_execution_bundle_v1(&mut wrong, &public).is_err(),
                "unapproved {field}"
            );
        }
        put_bytes_u32(&mut bundle, &json(&change)).unwrap();
        let decoded = decode_privacy_wallet_execution_bundle_v1(&mut bundle, &public).unwrap();
        let PrivacyNativeActionRequestV1::Ironwood(request) = decoded.request else {
            panic!("Ironwood request")
        };
        assert!(request.spends.is_empty());
        assert_eq!(request.outputs.len(), 2);
        assert_eq!(request.public_debit, 46);
        let (signer, _, authority) = test_signer();
        let context = crate::privacy_native_actions::PrivacyActionTransactionContextV1 {
            network_id: network_id_from_genesis_hash_bytes([1; 32]),
            authority,
            creation_time: Duration::from_millis(unix_time_millis().unwrap()),
            time_to_live: Some(Duration::from_secs(120)),
            nonce: NonZeroU32::new(17),
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            metadata: Default::default(),
        };
        let signed =
            build_signed_ironwood_note_action_v1(context, request, None, [1; 32], &signer).unwrap();
        let mapping = signed.ironwood_output_mapping().unwrap();
        assert_eq!(mapping.output_action_indices.len(), 2);
        assert_eq!(mapping.direct_instruction_index, 0);
        let public = statement(signed.signed_transaction());
        assert_eq!(public.value_balance.amount, 46);
        let merchant = wallet::incoming_viewing_key_v1(&MERCHANT_KEY, false).unwrap();
        let payer_change = wallet::incoming_viewing_key_v1(&[0x61; 32], true).unwrap();
        let payment =
            wallet::decrypt_outputs_with_incoming_viewing_key_v1(&public, &merchant).unwrap();
        let change =
            wallet::decrypt_outputs_with_incoming_viewing_key_v1(&public, &payer_change).unwrap();
        assert_eq!(payment[0].action_index(), mapping.output_action_indices[0]);
        assert_eq!(change[0].action_index(), mapping.output_action_indices[1]);
        assert_ne!(
            mapping.output_action_indices[0],
            mapping.output_action_indices[1]
        );
        assert_eq!(payment[0].value(), 17);
        assert_eq!(change[0].value(), 29);
        assert!(payment[0].nullifier().is_none());
    }

    #[test]
    fn ironwood_worker_ordered_zero_spend_pays_public_recipient_and_binds_every_call() {
        let directory = TempDir::new().unwrap();
        let merchant = credential_file(&directory, "merchant-key", &MERCHANT_KEY);
        let mut payload = Vec::new();
        put_text(&mut payload, merchant.to_str().unwrap());
        payload.extend_from_slice(&0u32.to_be_bytes());
        payload.push(0);
        let exported = ironwood_address(&payload).unwrap();
        assert_eq!(
            exported[..43],
            wallet::recipient_address_v1(MERCHANT_KEY, false, 0).unwrap()
        );
        assert_eq!(exported[43..], ironwood_profile_binding_v1().unwrap());
        let action = public_action(POOL);
        let (mut vault, lease, bound, public, plan) = imported(&action, [1; 32], true);
        for field in ["public_action", "orchard_contract_calls"] {
            let mut changed = plan.clone();
            let target = changed.as_object_mut().unwrap().get_mut(field).unwrap();
            if field == "public_action" {
                target
                    .as_object_mut()
                    .unwrap()
                    .insert("public_debit_decimal".into(), Value::String("18".into()));
            } else {
                target
                    .as_object_mut()
                    .unwrap()
                    .get_mut("after")
                    .unwrap()
                    .as_array_mut()
                    .unwrap()
                    .reverse();
            }
            let payload =
                encode_execute_payload(lease.handle, &bound, &json(&public), &json(&changed))
                    .unwrap();
            assert_ne!(json(&changed), json(&plan), "mutation must change {field}");
            let expected = if field == "public_action" {
                WorkerError::PublicActionMismatch
            } else {
                WorkerError::WrongBinding
            };
            match dispatch(&mut vault, CommandKind::Execute, &payload) {
                CommandResponse::Error(actual) => assert_eq!(actual, expected, "mutation {field}"),
                _ => panic!("mutated {field} unexpectedly accepted"),
            }
            assert!(vault.inspect(lease.handle, &bound).is_ok());
        }
        let signed = execute(&action, [1; 32], true);
        assert_eq!(ironwood_direct_index(&signed).unwrap(), 2);
        let statement = statement(&signed);
        assert_eq!(
            statement.value_balance.direction,
            PrivacyValueBalanceDirectionV1::IntoPool
        );
        assert_eq!(statement.value_balance.amount, 17);
        let notes = wallet::decrypt_outputs_v1(&statement, MERCHANT_KEY, false).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].value(), 17);
        assert_eq!(notes[0].memo(), &MEMO);
        assert_eq!(
            notes[0].commitment(),
            statement.actions[notes[0].action_index()].note_commitment
        );
        assert!(
            wallet::decrypt_outputs_v1(&statement, [0x63; 32], false)
                .unwrap()
                .is_empty()
        );
        let mut bad = statement.clone();
        bad.actions[0].encrypted_note[64] ^= 1;
        assert!(
            wallet::decrypt_outputs_v1(&bad, MERCHANT_KEY, false)
                .unwrap()
                .is_empty()
        );
        let mut mismatch = action.clone();
        mismatch
            .as_object_mut()
            .unwrap()
            .insert("public_debit_decimal".into(), Value::String("18".into()));
        let (mut vault, lease, bound, public, plan) = imported(&mismatch, [1; 32], false);
        let payload =
            encode_execute_payload(lease.handle, &bound, &json(&public), &json(&plan)).unwrap();
        assert!(matches!(
            dispatch(&mut vault, CommandKind::Execute, &payload),
            CommandResponse::Error(WorkerError::NativeActionFailed)
        ));
    }

    #[test]
    fn ironwood_merged_both_orders_authenticate_payment_effect_and_merchant_receipt() {
        // Core owns governed transition qualification; these derived exact selections
        // exercise real Ironwood proofs and receipts through actual V2 block outputs.
        for selection in crate::scheduled_execution_verification::merged_selections_for_testing() {
            payment_effect_case(Some(&selection));
            merchant_receipt_case(Some(&selection));
        }
    }

    #[test]
    fn ironwood_payment_effect_survives_interleaved_blocks_and_resume_after_tip_advance() {
        payment_effect_case(None);
    }

    fn payment_effect_case(
        selection: Option<&iroha_data_model::runtime::definition::RootSelectionV1>,
    ) {
        Python::initialize();
        use iroha_core::{
            state::World,
            sumeragi::{
                finality::{build_checkpoint, build_proof},
                test_chain::{CertifiedTestChain, TestChainConfig},
            },
        };
        let now = unix_time_millis().unwrap();
        let (_, _, payer) = test_signer();
        let reserve_key =
            iroha_crypto::KeyPair::from_seed(vec![0x56; 32], iroha_crypto::Algorithm::Ed25519);
        let reserve = AccountId::new(reserve_key.public_key().clone());
        let merchant_signer =
            iroha_crypto::KeyPair::from_seed(vec![0x64; 32], iroha_crypto::Algorithm::Ed25519);
        let merchant_authority = AccountId::new(merchant_signer.public_key().clone());
        let mut world = World::with(
            [Domain::new(DomainId::try_new("ironwood", "universal").unwrap()).build(&payer)],
            [
                Account::new(payer.clone()).build(&payer),
                Account::new(reserve.clone()).build(&payer),
                Account::new(merchant_authority.clone()).build(&payer),
            ],
            [AssetDefinition::numeric(
                asset(),
                "Ironwood merchant fixture",
                AssetBalancePolicy::Global,
                None,
            )
            .build(&payer)],
        );
        iroha_core::runtime::ironwood::install_for_test(&mut world);
        let mut config = TestChainConfig::new(world, now);
        config.genesis_instructions = vec![
            BootstrapPrivacyIronwoodPoolV1::new(
                PrivacyIronwoodPoolBootstrapV1::new(
                    PrivacyPoolIdV1::new(POOL),
                    asset(),
                    AssetBalanceScope::Global,
                    reserve.clone(),
                )
                .unwrap(),
            )
            .into(),
            Mint::asset_quantity(100u32, AssetId::new(asset(), payer.clone())).into(),
        ];
        let mut chain = CertifiedTestChain::start(config).unwrap();
        if let Some(selection) = selection {
            chain.select_runtime_for_testing(now, selection);
        }
        chain.commit(Vec::new());
        assert!(
            chain
                .committed(2)
                .block()
                .execution_outputs()
                .iter()
                .all(|row| row.trigger_lifecycle().is_some() == selection.is_some())
        );
        let checkpoint = build_checkpoint(&chain.state().view(), 2).unwrap();
        let network = *chain.network_id().as_bytes();
        let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
        let cut = chain
            .state()
            .with_native_ironwood_pool_cut_v1(
                &chain.committed(2),
                PrivacyPoolIdV1::new(POOL),
                &payer,
                &budget,
                |proof| norito::encode_canonical(&proof).map_err(|e| e.to_string()),
            )
            .unwrap();
        let prepare_frames = vec![hex::encode(
            norito::encode_canonical(&build_proof(&chain.state().view(), 2).unwrap()).unwrap(),
        )];
        // The fixture supplies unrelated submitted work for an otherwise empty commit.
        chain.commit(Vec::new());
        let mut action = public_action(POOL);
        action
            .as_object_mut()
            .unwrap()
            .insert("expiry_height".into(), Value::from(34u64));
        let paid = execute(&action, network, false);
        let original = iroha_version::codec::EncodeVersioned::encode_versioned(&paid);
        assert_eq!(chain.commit(vec![paid.clone()]), vec![true]);
        let retained = (2..=4)
            .map(|h| {
                hex::encode(
                    norito::encode_canonical(&build_proof(&chain.state().view(), h).unwrap())
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        chain.commit(Vec::new());
        chain.commit(Vec::new());
        assert_eq!(chain.height(), 6);
        let verify = |original: &[u8], frames: Vec<String>, amount: &str| {
            Python::attach(|py| {
                crate::ironwood_pool_cut::verify_ironwood_payment_effect_v1_py(
                    py,
                    &cut,
                    &hex::encode(checkpoint.encode_canonical().unwrap()),
                    prepare_frames.clone(),
                    frames,
                    original,
                    &hex::encode(network),
                    iroha_data_model::account::address::chain_discriminant(),
                    &hex::encode(POOL),
                    &asset().to_string(),
                    &payer.to_string(),
                    &hex::encode(ironwood_profile_binding_v1().unwrap()),
                    amount,
                )
            })
        };
        // The barrier holds the main GIL until the actual exported verifier detaches;
        // only Python work completed before that call returns counts as progress.
        let effect_json = crate::ironwood_pool_cut::tests::assert_python_thread_progress(|| {
            verify(&original, retained.clone(), "17")
        })
        .unwrap();
        let effect: Value = norito::json::from_str(&effect_json).unwrap();
        assert_eq!(effect["height"].as_u64(), Some(4));
        assert_eq!(effect["payer_debit_decimal"].as_str(), Some("17"));
        assert_eq!(effect["reserve_credit_decimal"].as_str(), Some("17"));
        assert_eq!(
            effect["reserve_account_id"].as_str(),
            Some(reserve.to_string().as_str())
        );
        assert_eq!(
            effect["transaction_hash_hex"].as_str(),
            Some(hex::encode(paid.hash_as_entrypoint().as_ref()).as_str())
        );
        assert_eq!(effect["direct_instruction_index"].as_u64(), Some(0));
        assert!(verify(&original, retained.clone(), "18").is_err());
        let mut gap = retained.clone();
        gap.remove(1);
        assert!(verify(&original, gap, "17").is_err());
        let mut tampered = original.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(verify(&tampered, retained.clone(), "17").is_err());
        // A valid proof with insufficient public funds commits as rejected, never an effect.
        let mut unaffordable = action.clone();
        unaffordable
            .as_object_mut()
            .unwrap()
            .insert("public_debit_decimal".into(), Value::String("99".into()));
        unaffordable
            .as_object_mut()
            .unwrap()
            .get_mut("outputs")
            .unwrap()
            .as_array_mut()
            .unwrap()[0]
            .as_object_mut()
            .unwrap()
            .insert("value_decimal".into(), Value::String("99".into()));
        let rejected = execute(&unaffordable, network, false);
        let rejected_original = iroha_version::codec::EncodeVersioned::encode_versioned(&rejected);
        assert_eq!(chain.commit(vec![rejected]), vec![false]);
        let rejected_frames = (2..=7)
            .map(|h| {
                hex::encode(
                    norito::encode_canonical(&build_proof(&chain.state().view(), h).unwrap())
                        .unwrap(),
                )
            })
            .collect();
        assert!(verify(&rejected_original, rejected_frames, "99").is_err());
        // Reopening requires only retained original/evidence, independent of the new tip.
        assert_eq!(
            verify(&original, retained, "17").unwrap(),
            norito::json::to_json(&effect).unwrap()
        );
    }

    #[test]
    fn ironwood_worker_merchant_receipt_requires_real_finality_success_and_exact_fields() {
        merchant_receipt_case(None);
    }

    fn merchant_receipt_case(
        selection: Option<&iroha_data_model::runtime::definition::RootSelectionV1>,
    ) {
        Python::initialize();
        use iroha_core::{
            state::{StorageReadOnly, World, WorldReadOnly},
            sumeragi::{
                finality::{build_checkpoint, build_proof},
                test_chain::{CertifiedTestChain, TestChainConfig},
            },
        };
        let now = unix_time_millis().unwrap();
        let (_, _, payer) = test_signer();
        let reserve_key =
            iroha_crypto::KeyPair::from_seed(vec![0x56; 32], iroha_crypto::Algorithm::Ed25519);
        let reserve = AccountId::new(reserve_key.public_key().clone());
        let merchant_signer =
            iroha_crypto::KeyPair::from_seed(vec![0x64; 32], iroha_crypto::Algorithm::Ed25519);
        let merchant_authority = AccountId::new(merchant_signer.public_key().clone());
        let mut world = World::with(
            [Domain::new(DomainId::try_new("ironwood", "universal").unwrap()).build(&payer)],
            [
                Account::new(payer.clone()).build(&payer),
                Account::new(reserve.clone()).build(&payer),
                Account::new(merchant_authority.clone()).build(&payer),
            ],
            [AssetDefinition::numeric(
                asset(),
                "Ironwood merchant fixture",
                AssetBalancePolicy::Global,
                None,
            )
            .build(&payer)],
        );
        iroha_core::runtime::ironwood::install_for_test(&mut world);
        let mut config = TestChainConfig::new(world, now);
        config.genesis_instructions = vec![
            BootstrapPrivacyIronwoodPoolV1::new(
                PrivacyIronwoodPoolBootstrapV1::new(
                    PrivacyPoolIdV1::new(POOL),
                    asset(),
                    AssetBalanceScope::Global,
                    reserve.clone(),
                )
                .unwrap(),
            )
            .into(),
            Mint::asset_quantity(100u32, AssetId::new(asset(), payer.clone())).into(),
        ];
        let mut chain = CertifiedTestChain::start(config).unwrap();
        if let Some(selection) = selection {
            chain.select_runtime_for_testing(now, selection);
        }
        chain.commit(Vec::new());
        assert!(
            chain
                .committed(2)
                .block()
                .execution_outputs()
                .iter()
                .all(|row| row.trigger_lifecycle().is_some() == selection.is_some())
        );
        let checkpoint = build_checkpoint(&chain.state().view(), 2).unwrap();
        let network = *chain.network_id().as_bytes();
        let paid = execute(&public_action(POOL), network, false);
        let paid_statement = statement(&paid);
        let transaction = hex::encode(paid.hash_as_entrypoint().as_ref());
        assert_eq!(chain.commit(vec![paid]), vec![true]);
        {
            let view = chain.state().view();
            assert_eq!(
                view.world()
                    .assets()
                    .get(&AssetId::new(asset(), payer.clone()))
                    .unwrap()
                    .as_ref(),
                &Quantity::from(83u32)
            );
            assert_eq!(
                view.world()
                    .assets()
                    .get(&AssetId::new(asset(), reserve.clone()))
                    .unwrap()
                    .as_ref(),
                &Quantity::from(17u32)
            );
        }
        let budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
        let cut = chain
            .state()
            .with_native_ironwood_pool_cut_v1(
                &chain.committed(3),
                PrivacyPoolIdV1::new(POOL),
                &payer,
                &budget,
                |proof| norito::encode_canonical(&proof).map_err(|e| e.to_string()),
            )
            .unwrap();
        let frames = (2..=3)
            .map(|h| {
                hex::encode(
                    norito::encode_canonical(&build_proof(&chain.state().view(), h).unwrap())
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let check_cut = |cut: &[u8]| {
            Python::attach(|py| {
                crate::ironwood_pool_cut::verify_ironwood_pool_cut_v1_py(
                    py,
                    cut,
                    &hex::encode(checkpoint.encode_canonical().unwrap()),
                    frames.clone(),
                    &hex::encode(network),
                    iroha_data_model::account::address::chain_discriminant(),
                    &hex::encode(POOL),
                    &asset().to_string(),
                    &payer.to_string(),
                    &hex::encode(ironwood_profile_binding_v1().unwrap()),
                )
            })
        };
        let verified_json =
            crate::ironwood_pool_cut::tests::assert_python_thread_progress(|| check_cut(&cut))
                .unwrap();
        let verified: Value = norito::json::from_str(&verified_json).unwrap();
        assert_eq!(verified["height"].as_u64(), Some(3));
        assert_eq!(verified["payer_balance_decimal"].as_str(), Some("83"));
        assert_eq!(verified["reserve_balance_decimal"].as_str(), Some("17"));
        assert_eq!(verified["anchor_epoch"].as_u64(), Some(2));
        let authenticated_page = iroha_data_model::sumeragi_finality::verify_checkpoint_page(
            chain.network_id(),
            &checkpoint,
            &(2..=3)
                .map(|h| build_proof(&chain.state().view(), h).unwrap())
                .collect::<Vec<_>>(),
            64,
            16 * 1024 * 1024,
        )
        .unwrap();
        assert_eq!(
            verified["block_hash_hex"].as_str(),
            Some(hex::encode(authenticated_page.tip().header().hash().as_ref()).as_str())
        );
        assert_eq!(
            verified["core_hash_hex"].as_str(),
            Some(hex::encode(authenticated_page.tip().core_hash().0).as_str())
        );
        assert_eq!(
            verified["context_id_hex"].as_str(),
            Some(hex::encode(authenticated_page.tip().context_id().as_ref()).as_str())
        );
        for (pool, selected_payer, profile, selected_frames) in [
            (
                hex::encode([0xd3; 32]),
                payer.to_string(),
                hex::encode(ironwood_profile_binding_v1().unwrap()),
                frames.clone(),
            ),
            (
                hex::encode(POOL),
                reserve.to_string(),
                hex::encode(ironwood_profile_binding_v1().unwrap()),
                frames.clone(),
            ),
            (
                hex::encode(POOL),
                payer.to_string(),
                hex::encode([0; 32]),
                frames.clone(),
            ),
            (
                hex::encode(POOL),
                payer.to_string(),
                hex::encode(ironwood_profile_binding_v1().unwrap()),
                frames[..1].to_vec(),
            ),
        ] {
            assert!(
                Python::attach(
                    |py| crate::ironwood_pool_cut::verify_ironwood_pool_cut_v1_py(
                        py,
                        &cut,
                        &hex::encode(checkpoint.encode_canonical().unwrap()),
                        selected_frames,
                        &hex::encode(network),
                        iroha_data_model::account::address::chain_discriminant(),
                        &pool,
                        &asset().to_string(),
                        &selected_payer,
                        &profile
                    )
                )
                .is_err()
            );
        }

        let mut forged: iroha_core_privacy::ironwood_pool_cut::IronwoodPoolCutProofV1 =
            norito::decode_canonical(&cut).unwrap();
        forged.payer_balance = None;
        assert!(check_cut(&norito::encode_canonical(&forged).unwrap()).is_err());
        assert_eq!(budget.reserved_bytes(), 0);
        let directory = TempDir::new().unwrap();
        let key_path = credential_file(&directory, "merchant-key", &MERCHANT_KEY);
        let expected = norito::json!({"transaction_hash_hex": transaction, "direct_instruction_index": 0u32, "action_index": 0u32, "recipient_hex": (hex::encode(wallet::recipient_address_v1(MERCHANT_KEY, false, 0).unwrap())), "amount_decimal": "17", "memo_hex": (hex::encode(MEMO))});
        let mut request = norito::json!({"network_id_hex": (hex::encode(network)), "chain_id": (checkpoint.chain_id()), "checkpoint_hex": (hex::encode(checkpoint.encode_canonical().unwrap())), "proofs_hex": ((2..=3).map(|h| hex::encode(norito::to_bytes(&build_proof(&chain.state().view(), h).unwrap()).unwrap())).collect::<Vec<_>>()), "asset_definition_id": (asset().to_string()), "pool_id_hex": (hex::encode(POOL)), "compiled_profile_digest_hex": (hex::encode(ironwood_profile_binding_v1().unwrap())), "internal_scope": false, "expected_receipt": expected});
        let mut payload = Vec::new();
        put_text(&mut payload, key_path.to_str().unwrap());
        put_bytes_u32(&mut payload, &json(&request)).unwrap();
        let receipt: Value = norito::json::from_slice(&ironwood_scan(&payload).unwrap()).unwrap();
        // Export is a one-time native owner operation. All subsequent receipt verification can
        // run with only the incoming credential, after the spending file is unavailable.
        let viewing_path = directory.path().join("merchant-incoming.ivk");
        let mut export = Vec::new();
        put_text(&mut export, key_path.to_str().unwrap());
        put_text(&mut export, viewing_path.to_str().unwrap());
        export.push(0);
        assert_eq!(
            ironwood_export_viewing_key(&export).unwrap(),
            ironwood_profile_binding_v1().unwrap()
        );
        assert!(
            ironwood_export_viewing_key(&export).is_err(),
            "exclusive creation must not overwrite"
        );
        fs::remove_file(&key_path).unwrap();
        let scan_with_viewing = |value: &Value| {
            let mut payload = Vec::new();
            put_text(&mut payload, viewing_path.to_str().unwrap());
            put_bytes_u32(&mut payload, &json(value)).unwrap();
            ironwood_scan_with_viewing_key(&payload)
        };
        let viewed: Value =
            norito::json::from_slice(&scan_with_viewing(&request).unwrap()).unwrap();
        assert_eq!(viewed, receipt);
        let mut wrong_scope = request.clone();
        wrong_scope
            .as_object_mut()
            .unwrap()
            .insert("internal_scope".into(), Value::Bool(true));
        assert_eq!(
            scan_with_viewing(&wrong_scope).unwrap_err(),
            WorkerError::WrongBinding
        );
        assert_eq!(
            receipt["receipts"][0]["receipt_identity"],
            norito::json!({"transaction_hash_hex": transaction, "action_index": 0u64})
        );
        let replayed: Value =
            norito::json::from_slice(&scan_with_viewing(&request).unwrap()).unwrap();
        assert_eq!(
            replayed["receipts"][0]["receipt_identity"],
            receipt["receipts"][0]["receipt_identity"]
        );
        assert_eq!(receipt["receipts"].as_array().unwrap().len(), 1);
        assert_eq!(receipt["receipts"][0]["height"].as_u64(), Some(3));
        assert_eq!(
            receipt["receipts"][0]["amount_decimal"].as_str(),
            Some("17")
        );
        for (field, replacement) in [
            (
                "recipient_hex",
                hex::encode(wallet::recipient_address_v1([0x63; 32], false, 0).unwrap()),
            ),
            ("amount_decimal", "18".to_owned()),
            ("memo_hex", hex::encode([0; 512])),
        ] {
            let mut wrong = request.clone();
            wrong
                .as_object_mut()
                .unwrap()
                .get_mut("expected_receipt")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), Value::String(replacement));
            assert_eq!(
                scan_ironwood_request(&json(&wrong), &MERCHANT_KEY).unwrap_err(),
                WorkerError::PublicActionMismatch
            );
        }
        assert_eq!(
            scan_ironwood_request(&json(&request), &[0x63; 32]).unwrap_err(),
            WorkerError::PublicActionMismatch
        );
        for (field, replacement) in [
            ("pool_id_hex", Value::String(hex::encode([0xd3; 32]))),
            (
                "asset_definition_id",
                Value::String(
                    AssetDefinitionId::derive_from_components(
                        DomainId::try_new("ironwood", "universal").unwrap(),
                        "other".parse().unwrap(),
                    )
                    .to_string(),
                ),
            ),
        ] {
            let mut wrong = request.clone();
            wrong
                .as_object_mut()
                .unwrap()
                .insert(field.into(), replacement);
            assert_eq!(
                scan_ironwood_request(&json(&wrong), &MERCHANT_KEY).unwrap_err(),
                WorkerError::PublicActionMismatch
            );
        }
        let mut alias = request.clone();
        let mut alias_network = network;
        alias_network[31] &= !1;
        alias.as_object_mut().unwrap().insert(
            "network_id_hex".into(),
            Value::String(hex::encode(alias_network)),
        );
        assert_eq!(
            scan_ironwood_request(&json(&alias), &MERCHANT_KEY).unwrap_err(),
            WorkerError::InvalidPayload
        );
        let mut wrong_profile = request.clone();
        wrong_profile.as_object_mut().unwrap().insert(
            "compiled_profile_digest_hex".into(),
            Value::String(hex::encode([0x33; 32])),
        );
        assert_eq!(
            scan_ironwood_request(&json(&wrong_profile), &MERCHANT_KEY).unwrap_err(),
            WorkerError::WrongBinding
        );
        for field in ["direct_instruction_index", "action_index"] {
            let mut wrong = request.clone();
            wrong
                .as_object_mut()
                .unwrap()
                .get_mut("expected_receipt")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field.into(), Value::from(1u64));
            assert_eq!(
                scan_ironwood_request(&json(&wrong), &MERCHANT_KEY).unwrap_err(),
                WorkerError::PublicActionMismatch
            );
        }
        let mut discovery = request.clone();
        discovery
            .as_object_mut()
            .unwrap()
            .insert("expected_receipt".into(), Value::Null);
        let discovered: Value = norito::json::from_slice(
            &scan_ironwood_request(&json(&discovery), &MERCHANT_KEY).unwrap(),
        )
        .unwrap();
        assert_eq!(discovered["receipts"], receipt["receipts"]);
        let mut tampered = request.clone();
        let mut bad_proof = build_proof(&chain.state().view(), 3).unwrap();
        let last = bad_proof.block_wire.len() - 1;
        bad_proof.block_wire[last] ^= 1;
        tampered
            .as_object_mut()
            .unwrap()
            .get_mut("proofs_hex")
            .unwrap()
            .as_array_mut()
            .unwrap()[1] = Value::String(hex::encode(norito::to_bytes(&bad_proof).unwrap()));
        assert!(scan_ironwood_request(&json(&tampered), &MERCHANT_KEY).is_err());
        // A real rejected transaction can contain perfectly decryptable ciphertext. It is never a receipt.
        let rejected = execute(&public_action([0xd3; 32]), network, false);
        let rejected_hash = hex::encode(rejected.hash_as_entrypoint().as_ref());
        assert_eq!(chain.commit(vec![rejected]), vec![false]);
        request.as_object_mut().unwrap().insert(
            "proofs_hex".into(),
            Value::Array(
                (2..=4)
                    .map(|h| {
                        Value::String(hex::encode(
                            norito::to_bytes(&build_proof(&chain.state().view(), h).unwrap())
                                .unwrap(),
                        ))
                    })
                    .collect(),
            ),
        );
        // Adjacent page starts with the authenticated paid block again, but it is now only the
        // checkpoint boundary. Discovery emits no duplicate, and an exact check at that boundary
        // requires an independently trusted earlier checkpoint instead of crediting again.
        let mut next_page = request.clone();
        next_page
            .as_object_mut()
            .unwrap()
            .insert("checkpoint_hex".into(), receipt["checkpoint_hex"].clone());
        next_page.as_object_mut().unwrap().insert(
            "proofs_hex".into(),
            Value::Array(
                (3..=4)
                    .map(|h| {
                        Value::String(hex::encode(
                            norito::to_bytes(&build_proof(&chain.state().view(), h).unwrap())
                                .unwrap(),
                        ))
                    })
                    .collect(),
            ),
        );
        assert_eq!(
            scan_with_viewing(&next_page).unwrap_err(),
            WorkerError::PublicActionMismatch
        );
        next_page
            .as_object_mut()
            .unwrap()
            .insert("expected_receipt".into(), Value::Null);
        let next_receipts: Value =
            norito::json::from_slice(&scan_with_viewing(&next_page).unwrap()).unwrap();
        assert!(next_receipts["receipts"].as_array().unwrap().is_empty());
        let mut rejected_request = request.clone();
        rejected_request
            .as_object_mut()
            .unwrap()
            .insert("pool_id_hex".into(), Value::String(hex::encode([0xd3; 32])));
        rejected_request
            .as_object_mut()
            .unwrap()
            .get_mut("expected_receipt")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("transaction_hash_hex".into(), Value::String(rejected_hash));
        assert_eq!(
            scan_ironwood_request(&json(&rejected_request), &MERCHANT_KEY).unwrap_err(),
            WorkerError::PublicActionMismatch
        );
        // The merchant can spend the discovered V3 note; all openings and the path stay native.
        let commitments = paid_statement
            .actions
            .iter()
            .map(|action| action.note_commitment)
            .collect::<Vec<_>>();
        let (anchor, path) = wallet::authentication_path_v1(&commitments, 0).unwrap();
        let open = || {
            wallet::decrypt_outputs_v1(&paid_statement, MERCHANT_KEY, false)
                .unwrap()
                .remove(0)
        };
        assert!(
            open()
                .into_spend(
                    Zeroizing::new(MERCHANT_KEY),
                    0,
                    &path,
                    orchard_empty_root_v1()
                )
                .is_err()
        );
        let spend = |epoch, nonce| {
            let input = open()
                .into_spend(Zeroizing::new(MERCHANT_KEY), 0, &path, anchor)
                .unwrap();
            let output = wallet::output_from_wallet_parts_v1(
                wallet::recipient_address_v1([0x63; 32], false, 0).unwrap(),
                11,
                &MEMO,
            )
            .unwrap();
            let change = wallet::output_from_wallet_parts_v1(
                wallet::recipient_address_v1(MERCHANT_KEY, true, 7).unwrap(),
                6,
                &MEMO,
            )
            .unwrap();
            let context = crate::privacy_native_actions::PrivacyActionTransactionContextV1 {
                network_id: chain.network_id(),
                authority: merchant_authority.clone(),
                creation_time: Duration::from_millis(unix_time_millis().unwrap()),
                time_to_live: Some(Duration::from_secs(120)),
                nonce: std::num::NonZeroU32::new(nonce),
                fee_payment: FeePaymentIntent::authority(Vec::new(), None),
                metadata: Default::default(),
            };
            let request = crate::privacy_native_actions::IronwoodNoteActionRequestV1 {
                asset_definition_id: asset(),
                public_balance_scope: AssetBalanceScope::Global,
                pool_id: PrivacyPoolIdV1::new(POOL),
                anchor: iroha_data_model::privacy::PrivacyRootV1::new(anchor),
                anchor_epoch: epoch,
                expiry_height: 100,
                spends: vec![input],
                outputs: vec![output, change],
                public_debit: 0,
                minimum_action_count: 1,
            };
            build_signed_ironwood_note_action_v1(
                context,
                request,
                None,
                network,
                merchant_signer.private_key(),
            )
            .unwrap()
            .into_signed_transaction()
        };
        let stale = spend(1, 10);
        let first = spend(2, 11);
        let duplicate = spend(2, 12);
        let paid_forward =
            wallet::decrypt_outputs_v1(&statement(&first), [0x63; 32], false).unwrap();
        assert_eq!(paid_forward.len(), 1);
        assert_eq!(paid_forward[0].value(), 11);
        let payer_change =
            wallet::decrypt_outputs_v1(&statement(&first), MERCHANT_KEY, true).unwrap();
        assert_eq!(payer_change.len(), 1);
        assert_eq!(payer_change[0].value(), 6);
        assert_eq!(
            payer_change[0].recipient(),
            wallet::recipient_address_v1(MERCHANT_KEY, true, 7).unwrap()
        );
        assert_eq!(statement(&first).anchor, statement(&duplicate).anchor);
        assert_eq!(statement(&duplicate).anchor_epoch, 2);
        assert_eq!(statement(&duplicate).anchor.as_bytes(), &anchor);
        assert_eq!(chain.commit(vec![stale]), vec![false]);
        assert_eq!(chain.commit(vec![first]), vec![true]);
        // Only bootstrap, deposit and this spend have produced roots. The independently
        // committed retention policy keeps the epoch-2 anchor available after epoch 3.
        assert!(
            chain
                .state()
                .view()
                .world()
                .privacy_consensus_policy()
                .admission_retained_root_count()
                >= 3
        );
        assert_eq!(chain.commit(vec![duplicate]), vec![false]);
        let committed = chain.committed(chain.height());
        let rejection = &committed.block().network_output_at(0).unwrap().1.result.0;
        // The early nullifier preflight determines the committed rejection. The identical
        // successful epoch-2 anchor and retained-root bound above separately establish that
        // advancing to epoch 3 did not make this a stale-anchor attempt.
        assert!(
            matches!(rejection,
                Err(iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                    iroha_data_model::ValidationFail::InstructionFailed(
                        iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                            iroha_data_model::isi::error::InvalidParameterError::SmartContract(message)
                        )
                    )
                )) if message == "Orchard nullifier was already consumed"
            ),
            "unexpected duplicate rejection: {rejection:?}"
        );
    }
}
