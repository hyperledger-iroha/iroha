// Same-scope regression coverage extracted to keep the parent source budget bounded.
#[test]
fn host_codec_bytes_hash_gas_and_lengths_ignore_ambient_layout() {
    let value = vec!["first".to_owned(), "second".to_owned()];
    let canonical = CoreHost::encode_norito_payload(&value).expect("encode canonical host payload");
    let canonical_hash = Hash::new(&canonical);
    let canonical_gas = CoreHost::state_query_gas(canonical.len());
    assert_eq!(
        CoreHost::norito_encoded_len_exact(&value),
        Some(u64::try_from(canonical.len()).expect("fixture length fits u64"))
    );
    let canonical_retained_bytes = {
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.restrict_output_limits(HostOutputLimits::new(1, u64::MAX));
        assert!(host.try_reserve_serialized_output(&value, 1));
        host.retained_output_bytes()
    };
    let name: Name = "state_input".parse().expect("valid public-input name");
    let canonical_name = CoreHost::encode_norito_payload(&name).expect("encode canonical Name");
    let path: StatePath = "state/first".parse().expect("valid state path");
    let canonical_path =
        CoreHost::encode_norito_payload(&path).expect("encode canonical StatePath");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
    let ambient_before = norito::to_bytes(&value).expect("encode payload under ambient layout");
    assert_ne!(ambient_before, canonical);
    let alternate_name = norito::to_bytes(&name).expect("encode Name under alternate layout");
    assert_ne!(alternate_name, canonical_name);
    let alternate_path = norito::to_bytes(&path).expect("encode StatePath under alternate layout");
    assert_ne!(alternate_path, canonical_path);
    let encoded_under_ambient =
        CoreHost::encode_norito_payload(&value).expect("encode host payload canonically");
    assert_eq!(encoded_under_ambient, canonical);
    assert_eq!(Hash::new(&encoded_under_ambient), canonical_hash);
    assert_eq!(
        CoreHost::state_query_gas(encoded_under_ambient.len()),
        canonical_gas
    );
    assert_eq!(
        CoreHost::norito_encoded_len_exact(&value),
        Some(u64::try_from(canonical.len()).expect("fixture length fits u64"))
    );
    let retained_under_ambient = {
        let mut host = CoreHost::new(ALICE_ID.clone());
        host.restrict_output_limits(HostOutputLimits::new(1, u64::MAX));
        assert!(host.try_reserve_serialized_output(&value, 1));
        host.retained_output_bytes()
    };
    assert_eq!(retained_under_ambient, canonical_retained_bytes);
    assert_eq!(
        CoreHost::decode_name_payload(&canonical_name)
            .expect("decode canonical Name under ambient layout"),
        name
    );
    assert_eq!(
        CoreHost::decode_name_payload(&alternate_name),
        Err(ivm::VMError::DecodeError)
    );
    assert_eq!(
        decode_canonical_norito::<StatePath>(&canonical_path)
            .expect("decode canonical StatePath under ambient layout"),
        path
    );
    assert!(
        decode_canonical_norito::<StatePath>(&alternate_path).is_err(),
        "ambient StatePath framing must not pass canonical decoding"
    );
    assert_eq!(
        norito::to_bytes(&value).expect("re-encode payload under ambient layout"),
        ambient_before,
        "canonical helpers must restore the caller's ambient layout"
    );
    drop(ambient);
    assert_eq!(
        norito::to_bytes(&value).expect("encode payload after ambient guard"),
        canonical
    );
}
fn exact_return_type(
    kind: iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1,
) -> iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
    iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
        nodes: vec![
            iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::Leaf(kind),
        ],
    }
}
fn decode_nested_return(
    payload: &[u8],
    kind: iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1,
) -> norito::json::Value {
    let schema = exact_return_type(kind);
    let record =
        crate::smartcontracts::ivm::return_value::decode_entrypoint_return_record(&schema, payload)
            .expect("decode canonical schema-bound nested return record");
    crate::smartcontracts::ivm::return_value::render_entrypoint_return_record(&schema, &record)
        .expect("render typed nested return record")
}
fn decode_nested_int(payload: &[u8]) -> i64 {
    decode_nested_return(
        payload,
        iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
    )
    .as_str()
    .expect("nested int renders as a canonical string")
    .parse()
    .expect("fixture nested int fits i64")
}
pub(super) fn store_tlv(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
    let tlv = make_tlv(ty as u16, payload);
    vm.alloc_host_tlv(&tlv)
        .expect("allocate VM-owned TLV input")
}
pub(super) fn quantity_frame(value: &Quantity) -> Vec<u8> {
    QuantityValueV1::new(value.clone())
        .encode_frame()
        .expect("encode quantity frame")
}
pub(super) fn store_quantity(vm: &mut IVM, value: &Quantity) -> u64 {
    store_tlv(vm, PointerType::Quantity, &quantity_frame(value))
}
fn read_option_words(vm: &IVM, handle: u64, some_words: u64) -> (bool, Vec<u64>) {
    let layout = ivm::sum::SumLayoutV1::option(some_words).expect("option layout");
    ivm::sum::read_words(vm, handle, layout).expect("read option handle")
}
fn read_option_int(vm: &IVM, handle: u64) -> Option<i64> {
    let (present, words) = read_option_words(vm, handle, 1);
    if !present {
        assert!(words.is_empty(), "Option::none cannot carry inactive words");
        return None;
    }
    let [pointer] = words.as_slice() else {
        panic!("Option<int>::some must carry exactly one pointer")
    };
    let tlv = vm
        .memory
        .validate_tlv(*pointer)
        .expect("next-offset int TLV");
    assert_eq!(tlv.type_id, PointerType::Int);
    Some(
        IntValueV1::decode_frame(tlv.payload)
            .expect("decode next-offset int")
            .into_int()
            .try_to_i64()
            .expect("query next offset fits i64"),
    )
}
fn decode_typed_leaf<T>(vm: &IVM, pointer: u64, expected: PointerType) -> T
where
    for<'de> T: NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    let tlv = vm.memory.validate_tlv(pointer).expect("typed leaf TLV");
    assert_eq!(tlv.type_id, expected);
    norito::decode_from_bytes(tlv.payload).expect("decode typed leaf")
}
fn decode_quantity_leaf(vm: &IVM, pointer: u64) -> Quantity {
    let tlv = vm.memory.validate_tlv(pointer).expect("quantity leaf TLV");
    assert_eq!(tlv.type_id, PointerType::Quantity);
    QuantityValueV1::decode_frame(tlv.payload)
        .expect("decode quantity leaf")
        .into_quantity()
}
fn seed_test_call_hash(tx: &mut StateTransaction<'_, '_>, byte: u8) {
    tx.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
}
fn fixture_domain_id() -> DomainId {
    DomainId::try_new("wonderland", "universal").expect("fixture domain id")
}
fn fixture_account(label: &str) -> AccountId {
    match label {
        "alice" => ALICE_ID.clone(),
        "bob" => BOB_ID.clone(),
        "carol" | "charlie" => {
            let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
            AccountId::new(fixture_public_key_from_seed(seed))
        }
        other => panic!("unsupported fixture account label: {other}"),
    }
}
fn local_contract_host(authority: AccountId) -> CoreHost {
    let mut host = CoreHost::new(authority);
    host.set_local_contract_debug_execution();
    host
}
fn build_authenticated_test_contract_program(
    code: &[u8],
    vector_length: u8,
    zk_mode: bool,
) -> Vec<u8> {
    build_authenticated_test_contract_program_with_states(code, vector_length, zk_mode, Vec::new())
}
fn build_authenticated_test_contract_program_with_states(
    code: &[u8],
    vector_length: u8,
    zk_mode: bool,
    states: Vec<ivm::EmbeddedStateDescriptor>,
) -> Vec<u8> {
    let contract_interface = ivm::EmbeddedContractInterfaceV1 {
        seiyaku_name: "CoreHostHarness".to_owned(),
        compiler_fingerprint: "iroha-core-host-tests".to_owned(),
        abi_hash: ivm_sys::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        features_bitmap: if zk_mode {
            ivm::CONTRACT_FEATURE_BIT_ZK
        } else {
            0
        },
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("CanRunCoreHostHarness".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: None,
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
        states,
        error_codes: Vec::new(),
    };
    let mut program = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: if zk_mode { ivm::ivm_mode::ZK } else { 0 },
        vector_length,
        max_cycles: 1_000_000,
        abi_version: 1,
    }
    .encode();
    program.extend_from_slice(&contract_interface.encode_section());
    program.extend_from_slice(code);
    program
}
fn fixture_account_in_domain(label: &str, domain_label: &str) -> AccountId {
    let seed: Vec<u8> = format!("{label}@{domain_label}")
        .as_bytes()
        .iter()
        .copied()
        .cycle()
        .take(32)
        .collect();
    AccountId::new(fixture_public_key_from_seed(seed))
}
pub(super) fn fixture_public_key_from_seed(seed: Vec<u8>) -> iroha_crypto::PublicKey {
    let (public_key, _) = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
        .expect("fixture seed must derive a valid keypair")
        .into_parts();
    public_key
}
#[test]
fn fixture_public_key_from_seed_uses_checked_seed_derivation() {
    assert_eq!(
        fixture_public_key_from_seed(vec![0x61; 32])
            .try_algorithm()
            .expect("fixture public key algorithm"),
        Algorithm::Ed25519
    );
    assert!(
        KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
        "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
    );
}
#[test]
fn contract_subject_sysvar_returns_bound_subject_and_fails_outside_contract_scope() {
    let authority = fixture_account("alice");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        &authority,
        7,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    let subject = contract_address.subject_id();
    let mut host = CoreHost::new(authority.clone());
    host.set_contract_runtime_context(Some(ContractRuntimeExecutionContext {
        contract_address,
        contract_subject: subject.clone(),
        contract_alias: None,
        entrypoint: "main".to_owned(),
    }));
    let mut vm = IVM::new(100_000);
    host.syscall(ivm_sys::SYSCALL_SYSVAR_CONTRACT_SUBJECT, &mut vm)
        .expect("read contract subject");
    let observed: AccountId =
        CoreHost::decode_tlv_typed(&vm, vm.register(10), PointerType::AccountId)
            .expect("decode contract subject");
    assert_eq!(observed, subject);
    let mut host = local_contract_host(authority);
    assert_eq!(
        host.syscall(ivm_sys::SYSCALL_SYSVAR_CONTRACT_SUBJECT, &mut vm),
        Err(ivm::VMError::PermissionDenied)
    );
}
fn build_fixture_account(id: &AccountId, authority: &AccountId) -> Account {
    Account::new(id.clone()).build(authority)
}
fn retail_dataspace_catalog() -> (
    iroha_data_model::nexus::DataSpaceId,
    iroha_data_model::nexus::DataSpaceCatalog,
) {
    let paynet = iroha_data_model::nexus::DataSpaceId::new(12);
    let catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
        iroha_data_model::nexus::DataSpaceMetadata::default(),
        iroha_data_model::nexus::DataSpaceMetadata {
            id: paynet,
            alias: "paynet".to_owned(),
            description: None,
            fault_tolerance: 1,
        },
    ])
    .expect("retail dataspace catalog");
    (paynet, catalog)
}
fn resolved_test_account_alias(
    tx: &StateTransaction<'_, '_>,
    alias: &AccountAlias,
) -> iroha_data_model::alias_setup::ResolvedAccountAliasV1 {
    iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
        alias
            .to_literal(&tx.nexus.dataspace_catalog)
            .expect("fixture alias must resolve through the live catalog")
            .parse()
            .expect("fixture alias literal must be canonical"),
        alias.dataspace,
    )
}
fn seed_test_account_alias_lease_record(
    tx: &mut StateTransaction<'_, '_>,
    alias: &AccountAlias,
    owner: &AccountId,
) {
    let dataspace_name = tx
        .nexus
        .dataspace_catalog
        .by_id(alias.dataspace)
        .expect("fixture alias dataspace must be catalogued")
        .alias
        .clone();
    let dataspace_selector =
        crate::sns::selector_for_dataspace_alias(&dataspace_name).expect("dataspace selector");
    let dataspace_key = crate::sns::record_storage_key(&dataspace_selector);
    if tx.world.smart_contract_state.get(&dataspace_key).is_none() {
        let address = AccountAddress::from_account_id(owner).expect("fixture owner address");
        let mut metadata = Metadata::default();
        metadata.insert(
            crate::sns::SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace metadata key"),
            iroha_primitives::json::Json::new(alias.dataspace.as_u64()),
        );
        let record = iroha_data_model::sns::NameRecordV1::new(
            dataspace_selector,
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            metadata,
        );
        tx.world
            .smart_contract_state
            .insert(dataspace_key, norito::codec::Encode::encode(&record));
    }
    if let Some(domain_id) = alias
        .domain_id(&tx.nexus.dataspace_catalog)
        .expect("fixture alias domain")
    {
        let domain_owner = tx
            .world
            .domains
            .get(&domain_id)
            .map(|domain| domain.owned_by().clone())
            .unwrap_or_else(|| owner.clone());
        if tx.world.domains.get(&domain_id).is_none() {
            let domain = Domain::new(domain_id.clone()).build(&domain_owner);
            tx.world.insert_domain_entry(domain_id.clone(), domain);
            tx.world.track_domain_owner(&domain_id, &domain_owner);
        }
        let selector = crate::sns::selector_for_domain(&domain_id).expect("SNS domain selector");
        let storage_key = crate::sns::record_storage_key(&selector);
        if tx.world.smart_contract_state.get(&storage_key).is_none() {
            let address =
                AccountAddress::from_account_id(&domain_owner).expect("domain owner address");
            let record = iroha_data_model::sns::NameRecordV1::new(
                selector,
                domain_owner,
                vec![iroha_data_model::sns::NameControllerV1::account(&address)],
                0,
                0,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                Metadata::default(),
            );
            tx.world
                .smart_contract_state
                .insert(storage_key, norito::codec::Encode::encode(&record));
        }
    }
    let selector = crate::sns::selector_for_account_alias(alias, &tx.nexus.dataspace_catalog)
        .expect("fixture alias selector");
    let storage_key = crate::sns::record_storage_key(&selector);
    if tx.world.smart_contract_state.get(&storage_key).is_some() {
        return;
    }
    let address = AccountAddress::from_account_id(owner).expect("fixture owner address");
    let record = iroha_data_model::sns::NameRecordV1::new(
        selector,
        owner.clone(),
        vec![iroha_data_model::sns::NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        Metadata::default(),
    );
    tx.world
        .smart_contract_state
        .insert(storage_key, norito::codec::Encode::encode(&record));
}
/// Lease-only state fixture for alias-resolution tests.
struct SeedTestAccountAliasLease {
    alias: AccountAlias,
    owner: AccountId,
}
impl SeedTestAccountAliasLease {
    fn new(
        alias: AccountAlias,
        owner: AccountId,
        _payer: AccountId,
        _term_years: u8,
        _pricing_class_hint: Option<u8>,
    ) -> Self {
        Self { alias, owner }
    }
}
impl Execute for SeedTestAccountAliasLease {
    fn execute(
        self,
        _authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
        seed_test_account_alias_lease_record(tx, &self.alias, &self.owner);
        Ok(())
    }
}
/// Declarative repair/CAS adapter used to build alias-resolution fixtures.
struct EnsureTestAccountAliasBinding {
    account: AccountId,
    alias: AccountAlias,
}
impl EnsureTestAccountAliasBinding {
    fn bind(account: AccountId, alias: AccountAlias, _lease_expiry_ms: Option<u64>) -> Self {
        Self { account, alias }
    }
}
impl Execute for EnsureTestAccountAliasBinding {
    fn execute(
        self,
        authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), iroha_data_model::isi::error::InstructionExecutionError> {
        let resolved = resolved_test_account_alias(tx, &self.alias);
        if let Some(expected_target) = tx.world.account_aliases.get(&self.alias).cloned()
            && expected_target != self.account
        {
            // Alias resolution deliberately fails closed unless the SNS lease,
            // forward binding, and rekey record agree. Model the lease transfer
            // that accompanies this test-only cross-account rebind before
            // applying the binding CAS.
            let selector =
                crate::sns::selector_for_account_alias(&self.alias, &tx.nexus.dataspace_catalog)
                    .expect("fixture alias selector");
            tx.world
                .smart_contract_state
                .remove(crate::sns::record_storage_key(&selector));
            seed_test_account_alias_lease_record(tx, &self.alias, &self.account);
            return iroha_data_model::isi::alias_setup::RebindAccountAlias::new(
                resolved,
                expected_target,
                self.account,
            )
            .execute(authority, tx);
        }
        seed_test_account_alias_lease_record(tx, &self.alias, &self.account);
        iroha_data_model::isi::alias_setup::EnsureAlias::new(
            iroha_data_model::alias_setup::AliasIntentV1::AccountAlias(
                iroha_data_model::alias_setup::AliasAccountIntentV1 {
                    alias: resolved,
                    target_account: self.account,
                    provision: iroha_data_model::alias_setup::AccountProvisionV1::Existing,
                    role: iroha_data_model::alias_setup::AccountAliasRoleV1::Additional,
                },
            ),
            iroha_data_model::alias_setup::AliasLeaseAcquisitionV1::new(1, None),
            iroha_data_model::alias_setup::AliasQuoteGuardV1 {
                expected_policy_version: 0,
                expected_payment_asset: AssetDefinitionId::derive_from_components(
                    DomainId::try_new("assets", "universal").expect("fixture asset domain"),
                    "xor".parse().expect("fixture asset name"),
                ),
                max_amount: Quantity::zero(),
                valid_until_ms: 0,
            },
        )
        .execute(authority, tx)
    }
}
fn fixture_signing_keypair(authority: &AccountId) -> KeyPair {
    if authority == &*ALICE_ID {
        return (*ALICE_KEYPAIR).clone();
    }
    if authority == &*BOB_ID {
        return (*BOB_KEYPAIR).clone();
    }
    panic!("unsupported fixture signing authority: {authority}");
}
pub(super) fn contract_test_state(authority: &AccountId) -> State {
    let domain = Domain::new(fixture_domain_id()).build(authority);
    let account = build_fixture_account(authority, authority);
    let world = World::with([domain], [account], []);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    grant_named_permission_to_account(
        &state,
        authority,
        authority.clone(),
        "CanRegisterSmartContractCode",
    );
    grant_named_permission_to_account(
        &state,
        authority,
        authority.clone(),
        iroha_data_model::smart_contract::CONTRACT_HAJIMARI_PERMISSION_NAME,
    );
    state
}
pub(super) fn install_contract(
    state: &State,
    authority: &AccountId,
    source: &str,
    nonce: u64,
) -> ContractAddress {
    install_contract_with_interface_and_lifecycle(state, authority, source, nonce, false, |_| {})
}
fn install_contract_with_pending_lifecycle(
    state: &State,
    authority: &AccountId,
    source: &str,
    nonce: u64,
) -> ContractAddress {
    install_contract_with_interface_and_lifecycle(state, authority, source, nonce, true, |_| {})
}
fn install_contract_with_interface(
    state: &State,
    authority: &AccountId,
    source: &str,
    nonce: u64,
    customize_interface: impl FnOnce(&mut ivm::EmbeddedContractInterfaceV1),
) -> ContractAddress {
    install_contract_with_interface_and_lifecycle(
        state,
        authority,
        source,
        nonce,
        false,
        customize_interface,
    )
}
fn install_contract_with_interface_and_lifecycle(
    state: &State,
    authority: &AccountId,
    source: &str,
    nonce: u64,
    leave_lifecycle_pending: bool,
    customize_interface: impl FnOnce(&mut ivm::EmbeddedContractInterfaceV1),
) -> ContractAddress {
    let compiler =
        ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
            mode: ivm::kotodama::compiler::CompilerMode::Production,
            ..ivm::kotodama::compiler::CompilerOptions::default()
        });
    let (mut code, _manifest) = compiler
        .compile_source_with_manifest(source)
        .expect("compile contract with manifest");
    sanitize_test_contract_artifact_wildcards(&mut code);
    rewrite_test_contract_interface(&mut code, customize_interface);
    let mut manifest = ivm::verify_contract_artifact(&code)
        .expect("test contract artifact must verify after wildcard sanitization")
        .manifest;
    let next_height = u64::try_from(state.view().height() + 1)
        .ok()
        .and_then(core::num::NonZeroU64::new)
        .expect("next block height must fit in u64 and be non-zero");
    let mut block = state.block(BlockHeader::new(next_height, None, None, None, 0, 0));
    let mut tx = block.transaction();
    tx.world.add_account_permission(
        authority,
        iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode.into(),
    );
    let code_hash =
        register_code_bytes(authority, code, &mut tx).expect("register contract bytecode");
    manifest.code_hash = Some(code_hash);
    manifest = manifest.signed(&fixture_signing_keypair(authority));
    register_manifest(authority, manifest, &mut tx).expect("register contract manifest");
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical test network id"),
        authority,
        nonce,
        DataSpaceId::UNIVERSAL,
    )
    .expect("derive contract address");
    activate_instance(authority, contract_address.clone(), code_hash, &mut tx)
        .expect("activate contract");
    // Most host tests exercise an isolated syscall and use a fixture that represents a
    // contract after its lifecycle transition. Lifecycle-specific tests opt into retaining
    // the real pending marker through `install_contract_with_pending_lifecycle`.
    if !leave_lifecycle_pending {
        crate::smartcontracts::code::set_pending_contract_lifecycle(
            &mut tx,
            &contract_address,
            None,
        );
    }
    tx.apply();
    block.commit().expect("commit contract registration block");
    contract_address
}
