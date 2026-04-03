use std::collections::HashMap;

use iroha_crypto::PublicKey;
use ivm::{
    IVM, KotodamaCompiler,
    mock_wsv::{AccountId, DomainId, MockWorldStateView, PermissionToken, WsvHost},
};

fn account(domain: &str, public_key: &str) -> AccountId {
    let _domain = DomainId::try_new(domain, "universal").unwrap();
    let public_key: PublicKey = public_key.parse().unwrap();
    AccountId::new(public_key)
}

#[test]
fn kotodama_register_domain_e2e() {
    // Compile a tiny Kotodama program that registers a domain via typed constructor.
    let src = r#"
        fn main() {
            register_domain(domain("e2e_domain"));
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let program = compiler.compile_source(src).expect("compile kotodama");

    // Set up a mock WSV with an authority having RegisterDomain permission.
    let alice = account(
        "domain",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);

    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Load and run
    vm.load_program(&program).expect("load program");
    // Place a small INPUT TLV buffer to satisfy any pointer-ABI publish needs.
    vm.memory.preload_input(0, &[]).expect("preload input");
    vm.run().expect("register_domain should succeed");
}
