#[test]
#[cfg(debug_assertions)]
fn prepared_public_arguments_decode_once_ignore_guest_descriptors_and_require_precharge() {
    let compiler = kotodama_lang::compiler::Compiler::new_with_options(
        kotodama_lang::compiler::CompilerOptions {
            mode: kotodama_lang::compiler::CompilerMode::Production,
            ..kotodama_lang::compiler::CompilerOptions::default()
        },
    );
    let (program, _) = compiler
        .compile_source_with_manifest(
            r#"
seiyaku PreparedArguments { permission Invoke;
  kotoage fn invoke(int count, Name label) authorize(Invoke) {
  }
}
"#,
        )
        .expect("compile parameterized contract");
    let metadata = ivm::ProgramMetadata::parse(&program).expect("parse contract metadata");
    let schema = metadata
        .contract_interface
        .as_ref()
        .expect("contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "invoke")
        .and_then(|entrypoint| entrypoint.argument_schema.as_ref())
        .expect("argument schema");
    let canonical = ivm_abi::arguments::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({"count": "7", "label": "ready"})),
    )
    .expect("encode arguments");
    ivm::reset_argument_record_decode_count();
    let prepared =
        ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), u64::MAX)
            .expect("prepare arguments");
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority.clone()]),
        Some(prepared.clone()),
    );
    let mut vm = IVM::new(100_000);
    vm.load_program(&program).expect("load table ABI contract");
    let descriptor = metadata
        .contract_interface
        .as_ref()
        .unwrap()
        .entrypoints
        .iter()
        .find(|entry| entry.name == "invoke")
        .unwrap();
    let entry_pc = metadata.prefix_len() as u64 + descriptor.entry_pc;
    vm.set_program_counter(entry_pc).unwrap();
    // Arbitrary guest-facing descriptors cannot substitute for the signed host record.
    vm.set_register(10, u64::MAX);
    vm.set_register(11, 8193);
    vm.set_register(12, 1);
    vm.set_register(13, 0);
    prepared
        .precharge_vm(&mut vm)
        .expect("precharge prepared arguments");
    vm.run_with_host(&mut host)
        .expect("host prepares the authenticated root tables");
    assert_eq!(ivm::argument_record_decode_count(), 1);
    assert_eq!(vm.call_result_word_count().unwrap(), 1);
    assert_eq!(vm.public_call_result_word(0).unwrap(), 0);

    let mut unpaid = IVM::new(100_000);
    unpaid.load_program(&program).unwrap();
    unpaid.set_program_counter(entry_pc).unwrap();
    assert_eq!(unpaid.run_with_host(&mut host), Err(VMError::DecodeError));
    assert!(unpaid.call_result_word_count().is_err());
    assert_eq!(ivm::argument_record_decode_count(), 1);
}
