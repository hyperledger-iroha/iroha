//! Kotodama structs: basic named-field access lowers to pointer-ABI and
//! passes CoreHost TLV validation for domain transfer.

use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

#[test]
fn struct_fields_lower_to_corehost_syscall_args() {
    // Define a struct with pointer-ABI fields and use it to call a builtin.
    // Field access uses named fields and should lower to numeric field indices
    // and ultimately to correct TLVs in r10/r11 for the syscall.
    let src = r#"
        seiyaku C {
            struct TransferArgs { domain: DomainId; to: AccountId; }
            fn main() {
                let args = TransferArgs(
                    domain("wonderland"),
                    account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
                );
                transfer_domain(authority(), args.domain, args.to);
            }
        }
    "#;

    let compiler = KotodamaCompiler::new();
    let prog = compiler
        .compile_source(src)
        .expect("compile Kotodama with struct fields");

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&prog).expect("load program");
    vm.run()
        .expect("CoreHost should validate &DomainId in r10 and &AccountId in r11");
}
