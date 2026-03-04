//! End-to-end test: Kotodama pointer constructors lower to Norito TLVs and
//! CoreHost validates TLVs for SetAccountDetail.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn kotodama_set_account_detail_with_constructors() {
    // Kotodama program uses pointer constructors for Name/Json and authority() for AccountId
    let src = r#"
        fn main() {
          // Use a valid AccountId multihash form for Iroha v2
          set_account_detail(account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"), name("cursor"), json("{\"x\":1}"));
        }
    "#;
    // Use default compiler options (no forced VECTOR bit)
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile kotodama");

    // Allow ample cycles so the program can mirror TLVs and perform the syscall.
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    vm.run()
        .expect("CoreHost should validate typed TLVs for name/json");
}
