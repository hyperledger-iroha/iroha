//! End-to-end coverage for bounded Kotodama List lowering and execution.

use ivm::{IVM, KotodamaCompiler, ProgramMetadata};
use ivm_abi::{list::ListLayoutV1, sum::SumLayoutV1};

fn run(source: &str) -> IVM {
    let program = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile bounded List program");
    let metadata = ProgramMetadata::parse(&program).expect("parse contract metadata");
    let entrypoint = metadata
        .contract_interface
        .as_ref()
        .expect("List test contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "main")
        .expect("main entrypoint");
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc;
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).expect("load List program");
    vm.set_program_counter(entrypoint_pc)
        .expect("select main entrypoint");
    vm.run().expect("execute List program");
    vm
}

#[test]
fn safe_mutations_execute_with_transactional_failures() {
    let vm = run(r#"
        seiyaku ListMutations {
            view fn main() -> i64 {
                var values: List<i64, 3> = [1, 2];
                if values.try_set(index: 8, value: 99) { return -1; }
                if !values.try_push(3) { return -2; }
                if values.try_push(4) { return -3; }
                let popped = values.pop().unwrap_or(0);
                let first = values.get(0).unwrap_or(0);
                if !values.contains(2) { return -4; }
                popped * 100 + first * 10 + values.len()
            }
        }
        "#);
    assert_eq!(vm.register(10), 312);
}

#[test]
fn comprehension_and_take_execute_as_bounded_copies() {
    let vm = run(r#"
        seiyaku ListComprehension {
            fn build() -> List<i64, 4> {
                let source: List<i64, 4> = [1, 2, 3];
                [value * 2 for value in source if value > 1]
            }

            view fn main() -> i64 {
                let doubled = build();
                let head: List<i64, 2> = doubled.take(2);
                head.get(1).unwrap_or(0)
            }
        }
        "#);
    assert_eq!(vm.register(10), 6);
}

#[test]
fn enumerate_materializes_bounded_structured_elements() {
    let vm = run(r#"
        seiyaku ListEnumerate {
            view fn main() -> i64 {
                let values: List<i64, 4> = [7, 8];
                let indexed = values.enumerate();
                let pair = indexed.get(1).unwrap_or((0, 0));
                pair.0 * 10 + pair.1
            }
        }
        "#);
    assert_eq!(vm.register(10), 18);
}

#[test]
fn list_of_options_uses_one_word_per_element() {
    let vm = run(r#"
        seiyaku ListOfOptions {
            view fn main() -> List<Option<i64>, 4> {
                [Option::some(7), Option::none]
            }
        }
        "#);
    let list = vm.register(10);
    assert_eq!(vm.load_u64(list), Ok(2), "returned List length header");
    assert_eq!(
        vm.load_u64(list + 8),
        Ok(4),
        "returned List capacity header"
    );
    let list_layout = ListLayoutV1::try_new(4, 1).expect("List<Option<i64>, 4> layout");
    let elements = ivm::list::read_words(&vm, list, list_layout).expect("read returned List");
    assert_eq!(elements.len(), 2);
    let sum_layout = SumLayoutV1::option(1).expect("Option<i64> layout");
    assert_eq!(
        ivm::sum::read_words(&vm, elements[0][0], sum_layout),
        Ok((true, vec![7]))
    );
    assert_eq!(
        ivm::sum::read_words(&vm, elements[1][0], sum_layout),
        Ok((false, vec![]))
    );
}

#[test]
fn contains_compares_nested_lists_sums_and_structs_by_value() {
    let vm = run(r#"
        seiyaku RecursiveContains {
            struct Envelope {
                labels: Option<List<i64, 3>>,
                markers: List<Option<i64>, 3>,
                outcome: Result<(i64, bool), i64>,
            }

            fn ok(last: i64, ready: bool) -> Envelope {
                Envelope {
                    labels: Option::some([1, last]),
                    markers: [Option::none, Option::some(last)],
                    outcome: Result::ok((9, ready)),
                }
            }

            fn err(code: i64) -> Envelope {
                Envelope {
                    labels: Option::none,
                    markers: [Option::none, Option::some(7)],
                    outcome: Result::err(code),
                }
            }

            view fn main() -> i64 {
                let values: List<Envelope, 2> = [ok(7, true), err(5)];

                // Every needle below is freshly allocated. Equality must use
                // the declared aggregate schema rather than handle identity.
                if !values.contains(ok(7, true)) { return -1; }
                if !values.contains(err(5)) { return -2; }

                // Same shape with a different nested Option payload.
                if values.contains(ok(8, true)) { return -3; }

                // Equal capacity does not make different active lengths equal.
                let short = Envelope {
                    labels: Option::some([7]),
                    markers: [Option::none, Option::some(7)],
                    outcome: Result::ok((9, true)),
                };
                if values.contains(short) { return -4; }

                // Sum tags and only their active payloads participate.
                let different_option_branch = Envelope {
                    labels: Option::some([1, 7]),
                    markers: [Option::some(0), Option::some(7)],
                    outcome: Result::ok((9, true)),
                };
                if values.contains(different_option_branch) { return -5; }
                let different_outer_option_branch = Envelope {
                    labels: Option::none,
                    markers: [Option::none, Option::some(7)],
                    outcome: Result::ok((9, true)),
                };
                if values.contains(different_outer_option_branch) { return -6; }
                if values.contains(ok(7, false)) { return -7; }
                if values.contains(err(6)) { return -8; }

                1
            }
        }
        "#);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn recursive_contains_support_does_not_admit_resource_elements() {
    let error = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ResourceElements {
                fn reject(values: List<Option<StateMap<i64, i64>>, 2>) {
                    let _values = values;
                }

                view fn main() -> i64 { 0 }
            }
            "#,
        )
        .expect_err("resource-bearing List elements must remain rejected");
    assert!(
        error.contains("E_LIST_RESOURCE_ELEMENT"),
        "unexpected compiler diagnostic: {error}"
    );
}

#[test]
fn zero_sized_elements_have_a_stable_public_compiler_diagnostic() {
    let error = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ZeroSizedElements {
                struct Empty {}

                view fn main() -> i64 {
                    let values: List<Empty, 1> = [Empty {}];
                    values.len()
                }
            }
            "#,
        )
        .expect_err("zero-sized List elements must fail semantic analysis");
    assert!(
        error.contains("E_LIST_ZERO_SIZED_ELEMENT"),
        "unexpected compiler diagnostic: {error}"
    );
    assert!(
        error.contains("List elements must encode at least one word"),
        "diagnostic must explain the representation requirement: {error}"
    );

    KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ContextualEmptyList {
                view fn main() -> i64 {
                    let values: List<i64, 4> = [];
                    values.len()
                }
            }
            "#,
        )
        .expect("ordinary contextual empty Lists remain valid");
}
