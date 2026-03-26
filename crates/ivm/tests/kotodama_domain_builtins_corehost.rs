//! End-to-end tests for Kotodama domain builtins: unregister_domain and transfer_domain.

use std::collections::HashMap;

use ivm::{
    IVM, KotodamaCompiler,
    mock_wsv::{DomainId, MockWorldStateView, PermissionToken, ScopedAccountId, WsvHost},
};

#[test]
fn kotodama_unregister_domain() {
    // Program unregisters a domain using a constructor
    let src = r#"
        fn main() { unregister_domain(domain("wonderland")); }
    "#;
    unsafe { std::env::set_var("IVM_COMPILER_DEBUG", "1") };
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile kotodama");
    // Prepare WSV with the domain present and caller permitted to register domains
    let mut wsv = MockWorldStateView::new();
    // Use a caller in a different domain to allow unregistering `wonderland` (no accounts in that domain)
    let alice: ScopedAccountId = ScopedAccountId::new(
        "admin".parse().expect("domain id"),
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let dom: DomainId = "wonderland".parse().unwrap();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, dom));
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&alice.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(100_000);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("unregister_domain should validate TLV and queue ISI");
}

#[test]
fn kotodama_transfer_domain() {
    // Program transfers a domain from `authority()` to bob
    let src = r#"
        fn main() {
          transfer_domain(authority(), domain("wonderland"), account_id("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ"));
        }
    "#;
    unsafe { std::env::set_var("IVM_COMPILER_DEBUG", "1") };
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile kotodama");
    let mut wsv = MockWorldStateView::new();
    let alice: ScopedAccountId = ScopedAccountId::new(
        "wonderland".parse().expect("domain id"),
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    wsv.add_account_unchecked(alice.clone());
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&alice.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(100_000);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("transfer_domain should validate TLVs and queue ISI");
}
