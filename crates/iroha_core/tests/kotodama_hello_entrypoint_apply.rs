//! Verify explicit Kotodama CNTR selection enters `main` and reaches `write_detail`.

use iroha_core::{
    smartcontracts::ivm::{cache::IvmCache, host::CoreHost},
    state::{World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::prelude::*;
use iroha_model_base::name::Name;
use iroha_primitives::nonzero;
use ivm::{IVM, ProgramMetadata};
use kotodama_lang::compiler::Compiler as KotodamaCompiler;
use mv::storage::StorageReadOnly;
fn seeded_authority(seed: u8) -> (AccountId, KeyPair) {
    let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("seeded Kotodama authority keypair should be valid");
    (AccountId::new(keypair.public_key().clone()), keypair)
}
fn select_kotodama_entrypoint(vm: &mut IVM, program: &[u8], name: &str) {
    let metadata = ProgramMetadata::parse(program).expect("parse Kotodama V1 artifact");
    let entrypoint = metadata
        .contract_interface
        .as_ref()
        .expect("Kotodama V1 artifact must embed CNTR")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == name)
        .unwrap_or_else(|| panic!("missing Kotodama V1 entrypoint `{name}`"));
    let entrypoint_pc = u64::try_from(metadata.prefix_len()).expect("program prefix fits u64")
        + entrypoint.entry_pc;
    vm.set_program_counter(entrypoint_pc)
        .unwrap_or_else(|error| panic!("select Kotodama V1 entrypoint `{name}`: {error:?}"));
}
/// Writes a detail on the authority directly. The authority's own signed transaction applies the
/// queued write here; a real contract call runs under a scoped authority and is refused, so
/// `examples/hello` does not do it.
const DETAIL_WRITER: &str = r#"
seiyaku Hello {
    permission Admin;
    hajimari() {
        debug::info("Hello from Kotodama");
    }
    kotoage fn main() authorize(Admin) {
        debug::info("Hello from Kotodama");
        set_example_detail();
    }
    fn set_example_detail() {
        ledger::account::set_metadata(
            account: context::authority(),
            key: Name::parse("example"),
            value: Json::parse("{\"hello\":\"world\"}"),
        );
    }
}
"#;
/// The README deploys this example and calls it with no grants, so it must stay callable.
#[test]
fn published_hello_example_needs_no_permission_grant() {
    let program = KotodamaCompiler::new()
        .compile_source(include_str!("../../../examples/hello/hello.ko"))
        .expect("compile hello contract");
    let metadata = ProgramMetadata::parse(&program).expect("parse hello artifact");
    let interface = metadata
        .contract_interface
        .as_ref()
        .expect("hello artifact embeds CNTR");
    assert!(interface.permissions.is_empty());
    for name in ["greet", "count"] {
        let entrypoint = interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == name)
            .unwrap_or_else(|| panic!("hello is missing `{name}`"));
        assert_eq!(
            entrypoint.authorization,
            iroha_data_model::smart_contract::manifest::EntrypointAuthorizationV1::Anyone,
            "`{name}` must be callable without a grant"
        );
    }
}
#[test]
fn selected_kotodama_hello_main_entrypoint_writes_expected_detail() {
    let program = KotodamaCompiler::new()
        .compile_source(DETAIL_WRITER)
        .expect("compile hello contract");
    let (authority, authority_key) = seeded_authority(7);
    let mut vm = IVM::new(5_000_000);
    vm.set_host(CoreHost::new(authority.clone()));
    vm.load_program(&program).expect("load hello contract");
    select_kotodama_entrypoint(&mut vm, &program, "main");
    vm.run().expect("run hello contract");
    let queued = CoreHost::with_host(&mut vm, CoreHost::drain_instructions)
        .expect("ordinary instruction inspection retains no attachment owner");
    assert_eq!(
        queued.len(),
        1,
        "expected selected `main` to queue exactly one detail write",
    );
    // The authority account exists over the authenticated signed genesis.
    let world = World::with(
        [],
        [Account::new(authority.clone()).build(&authority)],
        std::iter::empty::<AssetDefinition>(),
    );
    let state = crate::native_genesis::certified_state(world);
    {
        // Host effects apply only under an exact signed root: the authority's own signed
        // transaction carries the queued write.
        let transaction = TransactionBuilder::new(
            *state.network_id_ref(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(queued)
        .sign(authority_key.private_key());
        let parameters = state.transaction_admission_parameters();
        let accepted = AcceptedTransaction::accept(
            transaction,
            state.network_id_ref(),
            parameters.sumeragi().max_clock_drift(),
            parameters.transaction(),
            parameters
                .require_crypto_policy()
                .expect("signed genesis crypto policy"),
        )
        .expect("stateless transaction admission");
        let header = BlockHeader::new(
            nonzero!(2_u64),
            state.view().latest_block_hash(),
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        iroha_core::tx::execute_component_transaction_for_testing(
            &mut block,
            accepted,
            &mut IvmCache::new(),
            None,
        )
        .expect("apply queued hello instructions");
        block
            .commit_world_overlay_for_testing()
            .expect("commit hello block");
    }
    let key: Name = "example".parse().expect("metadata key");
    let view = state.view();
    let account = view
        .world
        .accounts()
        .get(&authority)
        .expect("authority account should exist");
    assert_eq!(
        account.metadata().get(&key).map(AsRef::as_ref),
        Some("{\"hello\":\"world\"}"),
    );
}
