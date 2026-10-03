//! Runtime coverage for the single bounded table ABI across wide products and loop calls.

use ivm::{IVM, ProgramMetadata};
use kotodama_lang::compiler::Compiler as KotodamaCompiler;

fn run_table_function(source: &str) -> IVM {
    let artifact = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile table program");
    let metadata = ProgramMetadata::parse(&artifact).expect("metadata");
    let main = metadata
        .contract_interface
        .as_ref()
        .expect("CNTR")
        .entrypoints
        .iter()
        .find(|entry| entry.name == "main")
        .expect("main entrypoint");
    let mut vm = IVM::new(5_000_000);
    vm.load_program(&artifact).expect("admit table contract");
    vm.set_program_counter(metadata.prefix_len() as u64 + main.entry_pc)
        .unwrap();
    vm.run().expect("execute authenticated table calls");
    assert_eq!(vm.call_result_word_count().unwrap(), 1);
    vm
}

#[test]
fn wide_arguments_results_and_nested_loop_calls_reuse_caller_tables() {
    let fields = (0..32)
        .map(|index| format!("bool f{index};"))
        .collect::<Vec<_>>()
        .join(" ");
    let values = (0..32)
        .map(|index| format!("f{index}: {}", index % 2 == 0))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "seiyaku Wide {{
        struct Record {{ {fields} }}
        fn echo(Record value) -> Record {{ value }}
        fn relay(Record value) -> Record {{ echo(value: value) }}
        view fn main() -> bool {{
            let expected = Record {{ {values} }};
            var value = expected;
            for index in range(4) {{ value = relay(value: value); }}
            value == expected && value.f0 && !value.f31
        }}
    }}"
    );
    let vm = run_table_function(&source);
    assert_eq!(vm.public_call_result_word(0).unwrap(), 1);
}

#[test]
fn unit_calls_initialize_exactly_one_result_slot() {
    let source = "seiyaku UnitCalls { fn unit() { () } fn pair(bool value) -> ((), bool, ()) { (unit(), value, unit()) } view fn main() -> bool { pair(value: true) == ((), true, ()) } }";
    let vm = run_table_function(source);
    assert_eq!(vm.public_call_result_word(0).unwrap(), 1);
}

#[test]
fn empty_products_use_unit_slots_in_calls_sums_and_lists() {
    let vm = run_table_function(
        r#"
seiyaku EmptyProducts {
    struct Empty {}
    struct Pair { Empty first; Empty second; }
    fn echo(Empty value) -> Empty { value }
    fn pair(Pair value) -> Pair { value }
    view fn main() -> bool {
        let value = echo(value: Empty {});
        let expected = Pair { first: Empty {}, second: Empty {} };
        let List<Empty, 2> items = [value, Empty {}];
        let Option<Empty> wrapped = Option::some(value);
        value == Empty {} && pair(value: expected) == expected
            && items == [Empty {}, Empty {}] && wrapped == Option::some(Empty {})
    }
}
"#,
    );
    assert_eq!(vm.public_call_result_word(0), Ok(1));
    let vm = run_table_function(
        "seiyaku EmptyRoot { struct Empty {} view fn main() -> Empty { Empty {} } }",
    );
    assert_eq!(vm.public_call_result_word(0), Ok(0));
}

#[test]
fn wide_sum_payloads_cross_the_signed_immediate_boundary() {
    // The last payload word sits at byte 32,768, beyond the signed IR immediate.
    // Keep the public boundary Boolean: the existing public schema node limit is
    // independent of the wider private function table and its active sum payload.
    let width = 4096;
    let fields = (0..width)
        .map(|index| format!("bool f{index};"))
        .collect::<Vec<_>>()
        .join(" ");
    let values = (0..width)
        .map(|index| {
            if index == width - 1 {
                format!("f{index}: last")
            } else {
                format!("f{index}: {}", index % 2 == 0)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    // Keep separate artifacts beneath the unchanged 1 MiB image limit. The
    // constructors return sum handles, avoiding repeated wide table marshals.
    let option_source = format!(
        r#"seiyaku WideOption {{
            struct Wide {{ {fields} }}
            fn option(bool last, List<int, 1> trace, int digit) -> Option<Wide> {{
                var log = trace;
                log.set(index: 0, value: log.get(0).unwrap_or(0) * 10 + digit);
                Option::some(Wide {{ {values} }})
            }}
            fn equal(bool last, List<int, 1> trace) -> bool {{
                option(last: true, trace: trace, digit: 1) ==
                    option(last: last, trace: trace, digit: 2)
            }}
            view fn main() -> bool {{
                let List<int, 1> trace = [0];
                let same = equal(last: true, trace: trace);
                let different = !equal(last: false, trace: trace);
                let ordered_once = trace.get(0).unwrap_or(0) == 1212;
                let present = option(last: true, trace: trace, digit: 0);
                let selected = match present {{
                    Option::some(value) => value.f0 && !value.f4093 && value.f4094 && value.f4095,
                    Option::none => false,
                }};
                same && different && ordered_once && selected
            }}
        }}"#,
    );
    let result_source = format!(
        r#"seiyaku WideResult {{
            struct Wide {{ {fields} }}
            fn result(bool last, bool success) -> Result<Wide, Wide> {{
                let value = Wide {{ {values} }};
                if success {{ Result::ok(value) }} else {{ Result::err(value) }}
            }}
            view fn main() -> bool {{
                let success = result(last: true, success: true);
                let failure = result(last: false, success: false);
                let success_selected = match success {{
                    Result::ok(value) => value.f4095,
                    Result::err(value) => false,
                }};
                let failure_selected = match failure {{
                    Result::ok(value) => false,
                    Result::err(value) => !value.f4095,
                }};
                success_selected && failure_selected
            }}
        }}"#,
    );
    for source in [option_source, result_source] {
        let vm = run_table_function(&source);
        assert_eq!(vm.public_call_result_word(0), Ok(1));
    }
}
