//! End-to-end tests for Kotodama domain builtins: unregister_domain and transfer_domain.
use crate::common;
use iroha_model_base::domain::DomainId;
use ivm::{
    IVM,
    mock_wsv::{AccountId, MockWorldStateView, PermissionToken, WsvHost},
};
use kotodama_lang::compiler::Compiler as KotodamaCompiler;
#[test]
fn kotodama_unregister_domain() {
    // Program unregisters a domain using a constructor
    let src = r#"
        seiyaku UnregisterDomain { permission FixtureAccess;
            kotoage fn main() authorize(FixtureAccess) {
                ledger::domain::unregister(domain: DomainId::parse("wonderland.universal"));
            }
        }
    "#;
    unsafe { std::env::set_var("IVM_COMPILER_DEBUG", "1") };
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile kotodama");
    // Prepare WSV with the domain present and caller permitted to register domains
    let mut wsv = MockWorldStateView::new();
    // Use a caller in a different domain to allow unregistering `wonderland.universal`
    // (no accounts in that domain).
    let _admin_domain: DomainId =
        iroha_model_base::domain::DomainId::try_new("admin", "universal").expect("domain id");
    let alice: AccountId = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let dom: DomainId =
        iroha_model_base::domain::DomainId::try_new("wonderland", "universal").unwrap();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, dom));
    let host = WsvHost::new_with_subject(wsv, alice.clone());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    common::select_kotodama_entrypoint(&mut vm, &prog, "main");
    vm.run()
        .expect("unregister_domain should validate TLV and queue ISI");
}
#[test]
fn kotodama_transfer_domain() {
    let bob = AccountId::new(
        "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
            .parse()
            .expect("public key"),
    );
    // Program transfers a domain from the execution authority to bob.
    let src = format!(
        r#"
        seiyaku TransferDomain {{ permission FixtureAccess;
        kotoage fn main() authorize(FixtureAccess) {{
          ledger::domain::transfer(source: context::authority(), domain: DomainId::parse("wonderland.universal"), destination: AccountId::parse("{bob}"));
        }}
        }}
    "#
    );
    unsafe { std::env::set_var("IVM_COMPILER_DEBUG", "1") };
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(&src).expect("compile kotodama");
    let mut wsv = MockWorldStateView::new();
    let wonderland: DomainId =
        iroha_model_base::domain::DomainId::try_new("wonderland", "universal").expect("domain id");
    let alice: AccountId = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    wsv.add_account_unchecked(alice.clone());
    // Only the current owner may transfer a domain, so the authority owns it first.
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    assert!(wsv.register_domain(&alice, wonderland.clone()));
    let host = WsvHost::new_with_subject(wsv, alice.clone());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    common::select_kotodama_entrypoint(&mut vm, &prog, "main");
    vm.run()
        .expect("transfer_domain should validate TLVs and move ownership");
    let host = vm
        .host_mut_any()
        .expect("host")
        .downcast_mut::<WsvHost>()
        .expect("downcast WsvHost");
    // Ownership moved: the former owner can no longer transfer it, the destination can.
    assert!(!host.wsv.transfer_domain(&alice, &wonderland, alice.clone()));
    assert!(host.wsv.transfer_domain(&bob, &wonderland, alice));
}
