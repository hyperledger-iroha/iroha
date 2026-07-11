//! End-to-end coverage for nominal Kotodama `Amount` lowering and execution.

use iroha_primitives::numeric::Numeric;
use ivm::{IVM, KotodamaCompiler, PointerType, ProgramMetadata};

fn execute_rounded(mode: &str) -> Numeric {
    let source = format!(
        r#"
        seiyaku RoundedAmount {{
            fn rounded(value: Amount, divisor: Amount, scale: i64) -> Amount {{
                value.div_round(
                    divisor: divisor,
                    scale: scale,
                    mode: Rounding::{mode},
                )
            }}

            view fn main() -> Amount {{
                rounded(value: 1amt, divisor: 8amt, scale: 2)
            }}
        }}
        "#,
    );
    let program = KotodamaCompiler::new()
        .compile_source(&source)
        .expect("compile rounded Amount program");
    let metadata = ProgramMetadata::parse(&program).expect("parse rounded Amount metadata");
    let entrypoint = metadata
        .contract_interface
        .as_ref()
        .expect("rounded Amount contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "main")
        .expect("main entrypoint");
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc;
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program)
        .expect("load rounded Amount program");
    vm.set_program_counter(entrypoint_pc)
        .expect("select main entrypoint");
    vm.run().expect("execute rounded Amount program");

    let tlv = vm
        .validate_tlv(vm.register(10))
        .expect("returned Amount pointer");
    assert_eq!(tlv.type_id, PointerType::Amount);
    norito::decode_from_bytes(tlv.payload).expect("decode returned canonical Amount")
}

#[test]
fn rounded_amount_modes_execute_through_the_extended_syscall() {
    for (mode, expected) in [
        ("floor", "0.12"),
        ("ceil", "0.13"),
        ("nearest_even", "0.12"),
    ] {
        let value = execute_rounded(mode);
        assert_eq!(value.to_string(), expected, "Rounding::{mode}");
        value.validate_amount().expect("canonical Amount result");
    }
}
