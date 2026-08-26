#[test]
#[cfg(debug_assertions)]
fn prepared_public_arguments_decode_once_and_reject_pointer_substitution() {
    let compiler =
        ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
            mode: ivm::kotodama::compiler::CompilerMode::Production,
            ..ivm::kotodama::compiler::CompilerOptions::default()
        });
    let (program, _) = compiler
        .compile_source_with_manifest(
            r#"
seiyaku PreparedArguments {
  kotoage fn invoke(int count, Name label) authorize("Invoke") {
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
    let canonical = ivm::encode_argument_record_from_json(
        schema,
        &Json::from(norito::json!({"count": "7", "label": "ready"})),
    )
    .expect("encode arguments");
    ivm::reset_argument_record_decode_count();
    let prepared =
        ivm::prepare_argument_record_with_gas_limit(schema, Arc::from(canonical), u64::MAX)
            .expect("prepare arguments");
    let authority: AccountId = fixture_account("alice");
    let mut adversarial_host = CoreHost::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority.clone()]),
        Some(prepared.clone()),
    );
    let name: Name = TRIGGER_EVENT_PUBLIC_INPUT_KEY.parse().expect("input name");
    let mut adversarial_vm = IVM::new(100_000);
    prepared
        .precharge_vm(&mut adversarial_vm)
        .expect("precharge prepared arguments");
    let name_ptr = store_tlv(&mut adversarial_vm, PointerType::Name, &norito_blob(&name));
    adversarial_vm.set_register(10, name_ptr);
    adversarial_host
        .syscall(ivm_sys::SYSCALL_GET_PUBLIC_INPUT, &mut adversarial_vm)
        .expect("get host-bound argument capability");
    let issued_record_pointer = adversarial_vm.register(10);
    assert_eq!(
        adversarial_vm
            .memory
            .validate_tlv(issued_record_pointer)
            .expect("argument binding TLV")
            .payload,
        prepared.binding_bytes(),
        "the signed record stays host-owned instead of consuming the VM input arena"
    );
    let substituted_record_pointer = store_tlv(
        &mut adversarial_vm,
        PointerType::NoritoBytes,
        prepared.canonical_bytes(),
    );
    let schema_pointer = store_tlv(
        &mut adversarial_vm,
        PointerType::NoritoBytes,
        prepared.schema_bytes(),
    );
    adversarial_vm.set_register(10, substituted_record_pointer);
    adversarial_vm.set_register(11, schema_pointer);
    assert!(matches!(
        adversarial_host.prepare_syscall(ivm_sys::SYSCALL_DECODE_ARGUMENT_RECORD, &adversarial_vm),
        Err(VMError::DecodeError)
    ));
    assert_ne!(issued_record_pointer, substituted_record_pointer);
    let mut host = CoreHost::with_accounts_and_argument_record(
        authority.clone(),
        Arc::new(vec![authority]),
        Some(prepared.clone()),
    );
    let mut vm = IVM::new(100_000);
    let name_ptr = store_tlv(&mut vm, PointerType::Name, &norito_blob(&name));
    let schema_pointer = store_tlv(&mut vm, PointerType::NoritoBytes, prepared.schema_bytes());
    let code = [
        encoding::wide::encode_sys(
            instruction::wide::system::SCALL,
            u8::try_from(ivm_sys::SYSCALL_GET_PUBLIC_INPUT).expect("syscall id fits in u8"),
        )
        .to_le_bytes(),
        encoding::wide::encode_syscallx(ivm_sys::SYSCALL_DECODE_ARGUMENT_RECORD).to_le_bytes(),
        encoding::wide::encode_halt().to_le_bytes(),
    ]
    .concat();
    vm.load_program(&build_program(&code, 0))
        .expect("load argument wrapper program");
    vm.set_register(10, name_ptr);
    vm.set_register(11, schema_pointer);
    prepared
        .precharge_vm(&mut vm)
        .expect("precharge prepared arguments");
    vm.run_with_host(&mut host)
        .expect("guest wrapper must use the prepared decode path");
    assert_eq!(ivm::argument_record_decode_count(), 1);
    let table = vm
        .memory
        .validate_tlv(vm.register(10))
        .expect("ABI word table");
    assert_eq!(table.type_id, PointerType::Blob);
    assert_eq!(table.payload.len(), 1 + 2 * core::mem::size_of::<u64>());
}
