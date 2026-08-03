    #[test]
    fn roundtrip_raw_genesis_serialization() -> Result<()> {
        let (_tmp_dir, builder) = test_builder();
        let raw = builder
            .build_raw()
            .with_consensus_mode(SumeragiConsensusMode::Permissioned);
        let json = norito::json::to_json(&raw)?;
        let de: RawGenesisTransaction = norito::json::from_str(&json)?;
        let json2 = norito::json::to_json(&de)?;
        assert_eq!(json, json2);

        Ok(())
    }

    #[test]
    fn build_raw_coalesces_parameters_into_one_authoritative_snapshot() -> Result<()> {
        use iroha_data_model::parameter::system::SumeragiParameter;

        init_instruction_registry();
        let raw = GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:build-raw-authoritative"),
            ".",
        )
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(667)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333)))
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Permissioned);

        let transactions = &raw.transactions;
        assert_eq!(transactions.len(), 3);
        let parameter_positions = transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        assert_eq!(parameter_positions, vec![0]);

        let authoritative = transactions[0]
            .parameters
            .as_ref()
            .expect("first transaction must carry the authoritative parameter snapshot");
        assert_eq!(authoritative.sumeragi().max_clock_drift_ms(), 333);
        assert!(transactions[1..].iter().all(|tx| tx.parameters.is_none()));
        assert_eq!(
            raw.effective_parameters()?.sumeragi().max_clock_drift_ms(),
            333
        );
        raw.clone().parse()?;

        let json = norito::json::to_json(&raw)?;
        let decoded: RawGenesisTransaction = norito::json::from_str(&json)?;
        let decoded_positions = decoded
            .transactions
            .iter()
            .enumerate()
            .filter_map(|(index, tx)| tx.parameters.as_ref().map(|_| index))
            .collect::<Vec<_>>();
        assert_eq!(decoded_positions, vec![0]);
        assert_eq!(
            decoded.transactions[0]
                .parameters
                .as_ref()
                .expect("decoded first transaction should carry authoritative params")
                .sumeragi()
                .max_clock_drift_ms(),
            333
        );
        assert_eq!(
            decoded
                .effective_parameters()?
                .sumeragi()
                .max_clock_drift_ms(),
            333
        );
        decoded.parse()?;

        Ok(())
    }

    #[test]
    fn default_genesis_deserializes() {
        init_instruction_registry();
        let genesis_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
        let result = RawGenesisTransaction::from_path(&genesis_path);
        assert!(result.is_ok());
    }

    #[test]
    fn default_genesis_block_roundtrips() -> Result<()> {
        use iroha_data_model::parameter::system::SumeragiNposParameters;

        init_instruction_registry();
        if norito::debug_trace_enabled() {
            // Debug tracing interferes with ConstVec decode guards; skip engineering checks in this mode.
            return Ok(());
        }
        let genesis_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults/genesis.json");
        let genesis = RawGenesisTransaction::from_path(&genesis_path)?;

        let kp = checked_genesis_fixture_keypair();
        let block = genesis.build_and_sign(&kp)?;

        let mut saw_handshake_mode = false;
        let mut saw_npos_custom = false;
        for tx in block.0.external_transactions() {
            if let iroha_data_model::transaction::Executable::Instructions(instrs) =
                tx.instructions()
            {
                for instr in instrs {
                    if let Some(set_param) = instr.as_any().downcast_ref::<SetParameter>() {
                        match set_param.inner() {
                            Parameter::Transaction(_) | Parameter::SmartContract(_) => {
                                panic!("unexpected high-level parameter instruction generated")
                            }
                            Parameter::Executor(_) => {
                                panic!("unexpected executor parameter instruction generated")
                            }
                            Parameter::Custom(custom)
                                if custom.id() == &consensus_metadata::handshake_meta_id() =>
                            {
                                let payload: norito::json::Value = custom
                                    .payload()
                                    .try_into_any_norito()
                                    .expect("decode handshake metadata payload");
                                let mode = payload
                                    .get("mode")
                                    .and_then(norito::json::Value::as_str)
                                    .expect("handshake metadata must carry mode");
                                assert_eq!(
                                    mode, "Npos",
                                    "Default genesis should advertise NPoS consensus mode"
                                );
                                saw_handshake_mode = true;
                            }
                            Parameter::Custom(custom)
                                if *custom.id() == SumeragiNposParameters::parameter_id() =>
                            {
                                saw_npos_custom = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        assert!(
            saw_handshake_mode,
            "Default genesis must emit SetParameter for consensus handshake metadata"
        );
        assert!(
            saw_npos_custom,
            "Default genesis must emit SetParameter for `sumeragi_npos_parameters`"
        );

        let encoded = block.0.encode_versioned();
        norito::core::reset_decode_state();
        let decoded = SignedBlock::decode_all_versioned(&encoded)
            .wrap_err("default genesis block should decode via canonical layout")?;
        assert_eq!(
            decoded, block.0,
            "Encoded + decoded default genesis block must preserve all fields"
        );

        Ok(())
    }

    #[test]
    fn instruction_registry_decodes_register_domain_box() {
        let registry = default_instruction_registry();
        let instruction = RegisterBox::Domain(Register::domain(Domain::new(
            DomainId::try_new("test", "universal").unwrap(),
        )));
        let (payload, flags) = norito::codec::encode_with_header_flags(&instruction);
        let bytes = norito::core::frame_bare_with_header_flags::<RegisterBox>(&payload, flags)
            .expect("frame register-domain instruction");
        registry
            .decode(RegisterBox::WIRE_ID, &bytes)
            .expect("entry")
            .expect("decode register-domain instruction");
    }

    #[test]
    fn uses_shared_instruction_registry() {
        let shared = iroha_data_model::instruction_registry::default();
        let local = default_instruction_registry();

        assert_eq!(local.len(), shared.len());
        for name in shared.names() {
            assert!(local.contains(name), "missing {name}");
        }
    }
