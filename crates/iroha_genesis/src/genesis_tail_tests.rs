fn complete_test_builder(builder: GenesisBuilder) -> GenesisBuilder {
    builder
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(
            deterministic_test_kagemusha_mint_finality_genesis_parameters(),
        )
}

fn complete_test_builder_for_peers(
    builder: GenesisBuilder,
    peers: Vec<iroha_data_model::peer::PeerId>,
) -> GenesisBuilder {
    builder
        .with_sumeragi_v2_context_parameters(SumeragiV2GenesisContextParameters::recommended())
        .with_kagemusha_mint_finality_genesis_parameters(
            deterministic_test_kagemusha_mint_finality_genesis_parameters_for(peers),
        )
}

fn load_default_genesis_source_template_for_test() -> Result<RawGenesisTransaction> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults/genesis.template.json");
    GenesisSourceTemplate::from_path(path)?
        .materialize(deterministic_test_kagemusha_mint_finality_genesis_parameters())
}

#[test]
fn roundtrip_raw_genesis_serialization() -> Result<()> {
    let (_tmp_dir, builder) = test_builder();
    let raw = builder
        .build_raw()?
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
    let raw = complete_test_builder(
        GenesisBuilder::new_without_executor(
            ChainId::from("iroha:test:build-raw-authoritative"),
            ".",
        )
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(100)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(667)))
        .next_transaction()
        .append_parameter(Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(333))),
    )
    .build_raw()?
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
fn default_genesis_source_template_is_not_a_raw_manifest() {
    init_instruction_registry();
    let genesis_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../defaults/genesis.template.json");
    let result = RawGenesisTransaction::from_path(&genesis_path);
    assert!(result.is_err());
}
#[test]
fn completed_default_genesis_source_template_proposal_roundtrips() -> Result<()> {
    use iroha_data_model::parameter::system::SumeragiNposParameters;
    init_instruction_registry();
    if norito::debug_trace_enabled() {
        // Debug tracing interferes with ConstVec decode guards; skip engineering checks in this mode.
        return Ok(());
    }
    let genesis = with_test_signing_topology(load_default_genesis_source_template_for_test()?);
    let kp = checked_genesis_fixture_keypair();
    let proposal = genesis.build_and_sign(&kp)?;
    assert!(
        proposal.0.is_resultless_proposal(),
        "raw manifest builders must emit a resultless proposal for runtime execution"
    );
    let mut saw_handshake_mode = false;
    let mut saw_npos_custom = false;
    for tx in proposal.0.external_transactions() {
        if let iroha_data_model::transaction::Executable::Instructions(instrs) = tx.instructions() {
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
    let encoded = proposal.0.encode_versioned();
    norito::core::reset_decode_state();
    let decoded = SignedBlock::decode_all_versioned(&encoded)
        .wrap_err("default genesis block should decode via canonical layout")?;
    assert_eq!(
        decoded, proposal.0,
        "encoded + decoded default genesis proposal must preserve all fields"
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
fn prepared_proposal_fixture() -> (RawGenesisTransaction, KeyPair, SignedBlock, Vec<u8>) {
    init_instruction_registry();
    let topology = (0..4)
        .map(|_| {
            let key_pair = checked_genesis_fixture_keypair_with_algorithm(Algorithm::BlsNormal);
            let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key())
                .expect("generate validator PoP");
            GenesisTopologyEntry::new(PeerId::new(key_pair.public_key().clone()), pop)
        })
        .collect::<Vec<_>>();
    let mint_finality_peers = topology.iter().map(|entry| entry.peer.clone()).collect();
    let manifest = complete_test_builder_for_peers(
        GenesisBuilder::new_without_executor(ChainId::from("prepared-verifier-fixture"), ".")
            .set_topology(topology),
        mint_finality_peers,
    )
    .build_raw()
    .expect("complete prepared-verifier fixture genesis")
    .with_consensus_meta();
    let genesis_key = checked_genesis_fixture_keypair();
    let proposal = manifest
        .clone()
        .build_and_sign(&genesis_key)
        .expect("sign verifier fixture")
        .0;
    assert!(proposal.is_resultless_proposal());
    let wire = proposal.encode_wire().expect("encode verifier fixture");
    (manifest, genesis_key, proposal, wire)
}
fn sign_modified_batches(
    manifest: &RawGenesisTransaction,
    key_pair: &KeyPair,
    mutate: impl FnOnce(&mut Vec<Vec<InstructionBox>>),
) -> SignedBlock {
    let mut batches = manifest
        .clone()
        .parse()
        .expect("expand verifier fixture manifest");
    mutate(&mut batches);
    let authority = AccountId::new(key_pair.public_key().clone());
    let transactions = batches
        .into_iter()
        .enumerate()
        .map(|(index, instructions)| {
            let mut builder = TransactionBuilder::new_genesis(
                authority.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions);
            builder.set_creation_time(Duration::from_millis(
                u64::try_from(index).expect("fixture transaction index fits") + 1,
            ));
            builder
                .try_sign(key_pair.private_key())
                .expect("sign modified verifier transaction")
        })
        .collect();
    let proposal = SignedBlock::genesis(transactions, key_pair.private_key(), None, None);
    assert!(proposal.is_resultless_proposal());
    proposal
}
fn sign_modified_envelopes(
    manifest: &RawGenesisTransaction,
    key_pair: &KeyPair,
    mut mutate: impl FnMut(usize, &mut TransactionBuilder),
) -> SignedBlock {
    let batches = manifest
        .clone()
        .parse()
        .expect("expand verifier fixture manifest");
    let authority = AccountId::new(key_pair.public_key().clone());
    let transactions = batches
        .into_iter()
        .enumerate()
        .map(|(index, instructions)| {
            let mut builder = TransactionBuilder::new_genesis(
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions);
            builder.set_creation_time(Duration::from_millis(
                u64::try_from(index).expect("fixture transaction index fits") + 1,
            ));
            mutate(index, &mut builder);
            builder
                .try_sign(key_pair.private_key())
                .expect("sign modified verifier transaction")
        })
        .collect();
    let proposal = SignedBlock::genesis(transactions, key_pair.private_key(), None, None);
    assert!(proposal.is_resultless_proposal());
    proposal
}
#[test]
fn prepared_bundle_verifier_accepts_exact_canonical_resultless_proposal() {
    let (manifest, key_pair, block, wire) = prepared_proposal_fixture();
    let validated =
        validate_prepared_genesis_bundle(&wire, &manifest, key_pair.public_key(), block.hash())
            .expect("exact bundle validates");
    assert_eq!(validated.canonical_wire(), wire);
    assert!(validated.block().is_resultless_proposal());
    assert_eq!(validated.validator_pops().len(), 4);
}
#[test]
fn prepared_bundle_verifier_rejects_noncanonical_wrong_hash_and_key() {
    let (manifest, key_pair, block, wire) = prepared_proposal_fixture();
    let wrong_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong genesis hash"));
    let error =
        validate_prepared_genesis_bundle(&wire, &manifest, key_pair.public_key(), wrong_hash)
            .expect_err("wrong exact hash must fail");
    assert!(error.to_string().contains("hashes to"));
    let wrong_key = checked_genesis_fixture_keypair();
    let error =
        validate_prepared_genesis_bundle(&wire, &manifest, wrong_key.public_key(), block.hash())
            .expect_err("wrong verifier key must fail");
    assert!(error.to_string().contains("differs from verifier key"));
    let mut noncanonical = wire;
    noncanonical.push(0);
    let _ = validate_prepared_genesis_bundle(
        &noncanonical,
        &manifest,
        key_pair.public_key(),
        block.hash(),
    )
    .expect_err("trailing bytes must not be admitted as canonical Norito");
}
#[test]
fn prepared_bundle_verifier_rejects_missing_and_duplicate_consensus_metadata() {
    let (manifest, key_pair, _, _) = prepared_proposal_fixture();
    let missing = sign_modified_batches(&manifest, &key_pair, |batches| {
        for batch in batches {
            batch.retain(|instruction| {
                instruction
                    .as_any()
                    .downcast_ref::<SetParameter>()
                    .and_then(|set| match set.inner() {
                        Parameter::Custom(custom) => Some(custom.id()),
                        _ => None,
                    })
                    != Some(&consensus_metadata::handshake_meta_id())
            });
        }
    });
    let missing_wire = missing
        .encode_wire()
        .expect("encode missing-metadata block");
    let error = validate_prepared_genesis_bundle(
        &missing_wire,
        &manifest,
        key_pair.public_key(),
        missing.hash(),
    )
    .expect_err("missing consensus metadata must fail");
    assert!(error.to_string().contains("no consensus metadata"));
    let duplicate = sign_modified_batches(&manifest, &key_pair, |batches| {
        let metadata = batches
                .iter()
                .flatten()
                .find(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<SetParameter>()
                        .is_some_and(|set| {
                            matches!(set.inner(), Parameter::Custom(custom) if custom.id() == &consensus_metadata::handshake_meta_id())
                        })
                })
                .expect("fixture consensus metadata")
                .clone();
        batches[0].push(metadata);
    });
    let duplicate_wire = duplicate
        .encode_wire()
        .expect("encode duplicate-metadata block");
    let error = validate_prepared_genesis_bundle(
        &duplicate_wire,
        &manifest,
        key_pair.public_key(),
        duplicate.hash(),
    )
    .expect_err("duplicate consensus metadata must fail");
    assert!(
        error
            .to_string()
            .contains("more than one consensus metadata")
    );
}
#[test]
fn prepared_bundle_verifier_rejects_noncanonical_transaction_envelopes() {
    let (manifest, key_pair, _, _) = prepared_proposal_fixture();
    let with_nonce = sign_modified_envelopes(&manifest, &key_pair, |index, builder| {
        if index == 0 {
            builder.set_nonce(core::num::NonZeroU32::new(1).expect("non-zero nonce"));
        }
    });
    let wire = with_nonce
        .encode_wire()
        .expect("encode nonce-bearing block");
    let error = validate_prepared_genesis_bundle(
        &wire,
        &manifest,
        key_pair.public_key(),
        with_nonce.hash(),
    )
    .expect_err("a genesis transaction nonce must fail closed");
    assert!(error.to_string().contains("non-canonical envelope fields"));
    let with_wrong_ttl = sign_modified_envelopes(&manifest, &key_pair, |index, builder| {
        if index == 0 {
            builder.set_ttl(Duration::from_secs(1));
        }
    });
    let wire = with_wrong_ttl
        .encode_wire()
        .expect("encode wrong-TTL block");
    let error = validate_prepared_genesis_bundle(
        &wire,
        &manifest,
        key_pair.public_key(),
        with_wrong_ttl.hash(),
    )
    .expect_err("a non-canonical genesis transaction TTL must fail closed");
    assert!(error.to_string().contains("non-canonical envelope fields"));
}
#[test]
fn prepared_bundle_verifier_rejects_nonconsecutive_transaction_times() {
    let (manifest, key_pair, _, _) = prepared_proposal_fixture();
    assert!(
        manifest.clone().parse().expect("expand fixture").len() > 1,
        "timestamp fixture needs multiple transaction batches"
    );
    let block = sign_modified_envelopes(&manifest, &key_pair, |index, builder| {
        if index == 1 {
            builder.set_creation_time(Duration::from_millis(1));
        }
    });
    let wire = block.encode_wire().expect("encode timestamp-drift block");
    let error =
        validate_prepared_genesis_bundle(&wire, &manifest, key_pair.public_key(), block.hash())
            .expect_err("non-consecutive genesis transaction times must fail closed");
    assert!(error.to_string().contains("next canonical millisecond"));
}
#[test]
fn prepared_bundle_verifier_rejects_manifest_semantics_and_validator_pops() {
    let (manifest, key_pair, block, wire) = prepared_proposal_fixture();
    let drifted_manifest = manifest
        .clone()
        .into_builder()
        .append_instruction(Register::domain(Domain::new(
            DomainId::try_new("drift", "universal").expect("domain id"),
        )))
        .build_raw()
        .expect("preserve complete prepared-verifier genesis authority")
        .with_consensus_meta();
    let error = validate_prepared_genesis_bundle(
        &wire,
        &drifted_manifest,
        key_pair.public_key(),
        block.hash(),
    )
    .expect_err("semantic manifest drift must fail");
    assert!(error.to_string().contains("differs from genesis manifest"));
    let mut bad_entries = (0..4)
        .map(|_| {
            let validator = checked_genesis_fixture_keypair_with_algorithm(Algorithm::BlsNormal);
            let pop = iroha_crypto::bls_normal_pop_prove(validator.private_key())
                .expect("generate validator PoP");
            GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
        })
        .collect::<Vec<_>>();
    bad_entries[0].pop_hex = Some(hex::encode([0_u8; 8]));
    let mint_finality_peers = bad_entries.iter().map(|entry| entry.peer.clone()).collect();
    let bad_manifest = complete_test_builder_for_peers(
        GenesisBuilder::new_without_executor(ChainId::from("prepared-verifier-bad-pop"), ".")
            .set_topology(bad_entries),
        mint_finality_peers,
    )
    .build_raw()
    .expect("complete bad-PoP verifier fixture genesis")
    .with_consensus_meta();
    let bad_proposal = bad_manifest
        .clone()
        .build_and_sign(&key_pair)
        .expect("sign bad-PoP fixture")
        .0;
    assert!(bad_proposal.is_resultless_proposal());
    let bad_wire = bad_proposal.encode_wire().expect("encode bad-PoP fixture");
    let error = validate_prepared_genesis_bundle(
        &bad_wire,
        &bad_manifest,
        key_pair.public_key(),
        bad_proposal.hash(),
    )
    .expect_err("bad validator PoP must fail");
    assert!(error.to_string().contains("invalid PoP"));
    let duplicate_block = sign_modified_batches(&manifest, &key_pair, |batches| {
        let duplicate = batches
            .iter()
            .flatten()
            .find(|instruction| {
                matches!(
                    instruction.as_any().downcast_ref::<RegisterBox>(),
                    Some(RegisterBox::Peer(_))
                )
            })
            .expect("fixture validator registration")
            .clone();
        let mut registrations = 0;
        for instruction in batches.iter_mut().flatten() {
            if matches!(
                instruction.as_any().downcast_ref::<RegisterBox>(),
                Some(RegisterBox::Peer(_))
            ) {
                registrations += 1;
                if registrations == 2 {
                    *instruction = duplicate;
                    return;
                }
            }
        }
        panic!("fixture must contain a second validator registration");
    });
    let duplicate_wire = duplicate_block
        .encode_wire()
        .expect("encode duplicate-validator fixture");
    let error = validate_prepared_genesis_bundle(
        &duplicate_wire,
        &manifest,
        key_pair.public_key(),
        duplicate_block.hash(),
    )
    .expect_err("duplicate validator PoP must fail");
    assert!(error.to_string().contains("more than once"));
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
