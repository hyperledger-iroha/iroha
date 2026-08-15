//! End-to-end coverage for bounded Kotodama List lowering and execution.
use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{IVM, KotodamaCompiler, ProgramMetadata, host::DefaultHost, pointer_abi::PointerType};
use ivm_abi::{list::ListLayoutV1, sum::SumLayoutV1};
use std::collections::BTreeMap;
mod common;
fn run(source: &str) -> IVM {
    run_with_gas(source).0
}
fn run_with_gas(source: &str) -> (IVM, u64) {
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
    let gas_used = u64::MAX.saturating_sub(vm.remaining_gas());
    (vm, gas_used)
}
fn run_main_body_with_gas(result_type: &str, body: &str) -> (IVM, u64) {
    run_with_gas(&format!(
        r#"
        seiyaku ListGasContract {{
            view fn main() -> {result_type} {{
                {body}
            }}
        }}
        "#
    ))
}
fn returned_int_list(vm: &IVM, capacity: u64) -> (u64, u64, Vec<i64>, Vec<u64>) {
    let layout = ListLayoutV1::try_new(capacity, 1).expect("List<int, N> layout");
    let base = vm.register(10);
    let words = (0..layout.allocation_bytes().expect("bounded allocation") / 8)
        .map(|word| vm.load_u64(base + word * 8).expect("returned List word"))
        .collect::<Vec<_>>();
    let length = words[0];
    let active_end = 2 + usize::try_from(length).expect("bounded List length");
    let elements = words[2..active_end]
        .iter()
        .map(|pointer| common::decode_i64_word(vm, *pointer))
        .collect();
    (length, words[1], elements, words[active_end..].to_vec())
}
fn positive_gas_delta(measured: u64, control: u64, operation: &str) -> u64 {
    assert!(
        measured > control,
        "{operation} must consume gas beyond its matched control: measured={measured}, control={control}"
    );
    measured - control
}
fn argument_tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload fits u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}
fn run_parameterized_int_entrypoint(program: &[u8], index: i64) -> (IVM, u64) {
    let metadata = ProgramMetadata::parse(program).expect("parse parameterized List metadata");
    let entrypoint = metadata
        .contract_interface
        .as_ref()
        .expect("parameterized List contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "main")
        .expect("main entrypoint");
    let schema = entrypoint
        .argument_schema
        .as_ref()
        .expect("parameterized List schema");
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc;
    let payload =
        Json::from_str_norito(&format!(r#"{{"index":"{index}"}}"#)).expect("valid List arguments");
    let record =
        ivm::encode_argument_record_from_json(schema, &payload).expect("encode List arguments");
    let key: Name = "trigger_event_json".parse().expect("public input key");
    let host = DefaultHost::new().with_public_inputs(BTreeMap::from([(
        key,
        argument_tlv(PointerType::NoritoBytes, &record),
    )]));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(program)
        .expect("load parameterized List program");
    vm.set_program_counter(entrypoint_pc)
        .expect("select parameterized List entrypoint");
    vm.run().expect("execute parameterized List program");
    let gas = u64::MAX.saturating_sub(vm.remaining_gas());
    (vm, gas)
}
fn run_multiword_mutation_failure_case(
    program: &[u8],
    operation: i64,
    index: &str,
) -> (Vec<u64>, u64) {
    let metadata = ProgramMetadata::parse(program).expect("parse multiword List metadata");
    let entrypoint = metadata
        .contract_interface
        .as_ref()
        .expect("multiword List contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "main")
        .expect("main entrypoint");
    let schema = entrypoint
        .argument_schema
        .as_ref()
        .expect("multiword List argument schema");
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("prefix fits u64") + entrypoint.entry_pc;
    let payload = Json::from_str_norito(&format!(
        r#"{{"operation":"{operation}","index":"{index}"}}"#
    ))
    .expect("valid multiword List arguments");
    let record =
        ivm::encode_argument_record_from_json(schema, &payload).expect("encode List arguments");
    let key: Name = "trigger_event_json".parse().expect("public input key");
    let host = DefaultHost::new().with_public_inputs(BTreeMap::from([(
        key,
        argument_tlv(PointerType::NoritoBytes, &record),
    )]));
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(program)
        .expect("load multiword List program");
    vm.set_program_counter(entrypoint_pc)
        .expect("select multiword List entrypoint");
    vm.run().expect("execute multiword List program");
    let layout = ListLayoutV1::try_new(2, 2).expect("List<Pair, 2> layout");
    let base = vm.register(10);
    let allocation = (0..layout.allocation_bytes().expect("bounded allocation") / 8)
        .map(|word| vm.load_u64(base + word * 8).expect("returned List word"))
        .collect();
    let heap_cursor = vm
        .alloc_heap(0)
        .expect("observe heap cursor without allocating");
    (allocation, heap_cursor)
}
#[test]
fn safe_mutations_execute_with_transactional_failures() {
    let vm = run(r#"
        seiyaku ListMutations {
            view fn main() -> int {
                var List<int, 3> values = [1, 2];
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
    assert_eq!(common::decode_i64_register(&vm, 10), 312);
}
#[test]
fn comprehension_and_take_execute_as_bounded_copies() {
    let vm = run(r#"
        seiyaku ListComprehension {
            fn build() -> List<int, 4> {
                let List<int, 4> source = [1, 2, 3];
                [value * 2 for value in source if value > 1]
            }

            view fn main() -> int {
                let doubled = build();
                let List<int, 2> head = doubled.take(2);
                head.get(1).unwrap_or(0)
            }
        }
        "#);
    assert_eq!(common::decode_i64_register(&vm, 10), 6);
}
#[test]
fn list_gas_grows_with_the_active_element_count_at_fixed_capacity() {
    let mut samples = Vec::new();
    for active_len in [1_u64, 4, 8] {
        let elements = (1..=active_len)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            r#"
            seiyaku ListGas {{
                view fn main() -> int {{
                    let List<int, 8> source = [{elements}];
                    let List<int, 8> copied = [value for value in source if value > 0];
                    if copied.contains(99) {{ return -1; }}
                    copied.len()
                }}
            }}
            "#
        );
        let (vm, gas_used) = run_with_gas(&source);
        assert_eq!(
            common::decode_i64_register(&vm, 10),
            i64::try_from(active_len).expect("bounded active length")
        );
        samples.push((active_len, gas_used));
    }
    for pair in samples.windows(2) {
        assert!(
            pair[0].1 < pair[1].1,
            "List gas must grow with active elements at one fixed capacity: {samples:?}"
        );
    }
}
#[test]
fn get_gas_is_deterministic_and_does_not_scan_preceding_elements() {
    let program = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ListGetGas {
                view fn main(int index) -> int {
                    let List<int, 8> values = [10, 20, 30, 40];
                    return values.get(index).unwrap_or(99);
                }
            }
            "#,
        )
        .expect("compile parameterized List get contract");
    let control = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ListGetGasControl {
                view fn main(int index) -> int {
                    return 0;
                }
            }
            "#,
        )
        .expect("compile matched argument-decoding control");
    let mut samples = Vec::new();
    for (index, expected) in [(0, 10), (1, 20), (3, 40), (8, 99)] {
        let (vm, gas) = run_parameterized_int_entrypoint(&program, index);
        let (_, control_gas) = run_parameterized_int_entrypoint(&control, index);
        let (_, repeated_gas) = run_parameterized_int_entrypoint(&program, index);
        let (_, repeated_control_gas) = run_parameterized_int_entrypoint(&control, index);
        assert_eq!(common::decode_i64_register(&vm, 10), expected);
        let operation_gas = gas
            .checked_sub(control_gas)
            .expect("List get exceeds matched decode control");
        let repeated_operation_gas = repeated_gas
            .checked_sub(repeated_control_gas)
            .expect("repeated List get exceeds matched decode control");
        assert_eq!(
            operation_gas, repeated_operation_gas,
            "identical List get input must consume identical gas"
        );
        samples.push((index, operation_gas));
    }
    assert!(
        samples[0].1 < samples[1].1,
        "canonical exact-int zero must retain its cheaper zero-magnitude path: {samples:?}"
    );
    assert_eq!(
        samples[1].1, samples[2].1,
        "get must not scan preceding elements with the same exact-int width: {samples:?}"
    );
    assert!(
        samples[3].1 < samples[0].1,
        "a missing get must skip payload materialization: {samples:?}"
    );
}
#[test]
fn try_set_gas_and_transactionality_cover_success_and_failure() {
    let (control, control_gas) = run_main_body_with_gas(
        "List<int, 4>",
        r#"
        var List<int, 4> values = [10, 20];
        return values;
        "#,
    );
    let control_values = returned_int_list(&control, 4);
    let (success, success_gas) = run_main_body_with_gas(
        "List<int, 4>",
        r#"
        var List<int, 4> values = [10, 20];
        values.try_set(index: 1, value: 99);
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&success, 4),
        (2, 4, vec![10, 99], vec![0, 0])
    );
    let (failure, failure_gas) = run_main_body_with_gas(
        "List<int, 4>",
        r#"
        var List<int, 4> values = [10, 20];
        values.try_set(index: 8, value: 99);
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&failure, 4),
        control_values,
        "failed try_set must leave the complete allocation unchanged"
    );
    let success_delta = positive_gas_delta(success_gas, control_gas, "successful try_set");
    let failure_delta = positive_gas_delta(failure_gas, control_gas, "failed try_set");
    assert!(
        failure_delta < success_delta,
        "failed try_set must skip element writes: success={success_delta}, failure={failure_delta}"
    );
}
#[test]
fn arbitrary_width_out_of_range_indices_are_total_and_transactional() {
    const SIGNED_512_MIN: &str = "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048";
    const SIGNED_512_MAX: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
    for index in [
        "9223372036854775808",
        "18446744073709551615",
        "18446744073709551616",
        SIGNED_512_MIN,
        SIGNED_512_MAX,
    ] {
        let (vm, _) = run_main_body_with_gas(
            "List<int, 4>",
            &format!(
                r#"
                var List<int, 4> values = [10, 20];
                let int index = {index};
                if values.get(index).unwrap_or(77) != 77 {{
                    values.try_set(index: 0, value: -1);
                }}
                if values.try_set(index: index, value: 99) {{
                    values.try_set(index: 0, value: -2);
                }}
                return values;
                "#
            ),
        );
        assert_eq!(
            returned_int_list(&vm, 4),
            (2, 4, vec![10, 20], vec![0, 0]),
            "index {index} must produce none/false without mutating the List"
        );
    }
}
#[test]
fn try_push_gas_and_transactionality_cover_space_and_full_capacity() {
    let (space_control, space_control_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20];
        return values;
        "#,
    );
    let (success, success_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20];
        values.try_push(30);
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&success, 3),
        (3, 3, vec![10, 20, 30], vec![])
    );
    let (full_control, full_control_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20, 30];
        return values;
        "#,
    );
    let full_control_values = returned_int_list(&full_control, 3);
    let (failure, failure_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20, 30];
        values.try_push(40);
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&failure, 3),
        full_control_values,
        "full-capacity try_push must leave the complete allocation unchanged"
    );
    assert_eq!(
        returned_int_list(&space_control, 3),
        (2, 3, vec![10, 20], vec![0])
    );
    let success_delta = positive_gas_delta(success_gas, space_control_gas, "successful try_push");
    let failure_delta = positive_gas_delta(failure_gas, full_control_gas, "full-capacity try_push");
    assert!(
        failure_delta < success_delta,
        "full try_push must skip element and length writes: success={success_delta}, failure={failure_delta}"
    );
}
#[test]
fn failed_multiword_mutations_preserve_every_word_and_allocate_nothing_after_preflight() {
    let program = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku MultiwordMutationFailures {
                struct Pair { int first, int second }

                view fn main(int operation, int index) -> List<Pair, 2> {
                    if operation < 2 {
                        var List<Pair, 2> values = [
                            Pair { first: 10, second: 20 },
                        ];
                        let Pair replacement = Pair { first: 90, second: 91 };
                        let Pair control_poison = Pair { first: -1, second: -1 };
                        let Pair failure_poison = Pair { first: -2, second: -2 };
                        if operation == 0 {
                            // Match the exact bounds-proof allocation performed by
                            // try_set before comparing its failure branch to control.
                            let int length = values.len();
                            let bool non_negative = index >= 0;
                            let bool below_length = index < length;
                            let bool present = non_negative && below_length;
                            if present {
                                values.try_set(
                                    index: 0,
                                    value: control_poison,
                                );
                            }
                        } else if values.try_set(
                            index: index,
                            value: replacement,
                        ) {
                            values.try_set(
                                index: 0,
                                value: failure_poison,
                            );
                        }
                        return values;
                    }

                    var List<Pair, 2> values = [
                        Pair { first: 10, second: 20 },
                        Pair { first: 30, second: 40 },
                    ];
                    let Pair replacement = Pair { first: 90, second: 91 };
                    let Pair failure_poison = Pair { first: -3, second: -3 };
                    if operation == 3 && values.try_push(replacement) {
                        values.try_set(
                            index: 0,
                            value: failure_poison,
                        );
                    }
                    return values;
                }
            }
            "#,
        )
        .expect("compile multiword mutation failure fixture");
    for index in ["-1", "1", "8", "18446744073709551616"] {
        let control = run_multiword_mutation_failure_case(&program, 0, index);
        let failure = run_multiword_mutation_failure_case(&program, 1, index);
        assert_eq!(
            failure.0, control.0,
            "failed try_set({index}) mutated a reserved Pair word"
        );
        assert_eq!(
            failure.1, control.1,
            "failed try_set({index}) allocated after its matched bounds proof"
        );
    }
    let full_control = run_multiword_mutation_failure_case(&program, 2, "0");
    let full_failure = run_multiword_mutation_failure_case(&program, 3, "0");
    assert_eq!(
        full_failure.0, full_control.0,
        "full-capacity try_push mutated a reserved Pair word"
    );
    assert_eq!(
        full_failure.1, full_control.1,
        "full-capacity try_push allocated before returning false"
    );
}
#[test]
fn pop_gas_and_transactionality_cover_nonempty_and_empty_lists() {
    let (nonempty_control, nonempty_control_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20];
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&nonempty_control, 3),
        (2, 3, vec![10, 20], vec![0])
    );
    let (nonempty, nonempty_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [10, 20];
        values.pop();
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&nonempty, 3),
        (1, 3, vec![10], vec![0, 0]),
        "pop must clear the vacated slot"
    );
    let (empty_control, empty_control_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [];
        return values;
        "#,
    );
    let empty_control_values = returned_int_list(&empty_control, 3);
    let (empty, empty_gas) = run_main_body_with_gas(
        "List<int, 3>",
        r#"
        var List<int, 3> values = [];
        values.pop();
        return values;
        "#,
    );
    assert_eq!(
        returned_int_list(&empty, 3),
        empty_control_values,
        "empty pop must leave the complete allocation unchanged"
    );
    let nonempty_delta = positive_gas_delta(nonempty_gas, nonempty_control_gas, "nonempty pop");
    let empty_delta = positive_gas_delta(empty_gas, empty_control_gas, "empty pop");
    assert!(
        empty_delta < nonempty_delta,
        "empty pop must skip payload reads and clearing writes: nonempty={nonempty_delta}, empty={empty_delta}"
    );
}
#[test]
fn contains_gas_increases_by_one_exact_scan_step_per_mismatch() {
    let mut samples = Vec::new();
    for (needle, expected) in [(10, 1), (20, 1), (30, 1), (40, 1), (99, 0)] {
        let (vm, gas) = run_main_body_with_gas(
            "int",
            &format!(
                r#"
            let List<int, 4> values = [10, 20, 30, 40];
            if values.contains({needle}) {{ return 1; }}
            return 0;
            "#
            ),
        );
        assert_eq!(common::decode_i64_register(&vm, 10), expected);
        samples.push((needle, gas));
    }
    let first_scan_step = samples[1].1 - samples[0].1;
    assert!(first_scan_step > 0, "each mismatch must consume gas");
    assert_eq!(samples[2].1 - samples[1].1, first_scan_step);
    assert_eq!(samples[3].1 - samples[2].1, first_scan_step);
    assert!(
        samples[4].1 > samples[3].1,
        "an absent value must examine all active elements: {samples:?}"
    );
}
#[test]
fn comprehension_gas_delta_is_exactly_linear_in_active_source_elements() {
    let mut deltas = Vec::new();
    for active_len in [0_u64, 1, 2, 4, 8] {
        let elements = (1..=active_len)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let (control, control_gas) = run_main_body_with_gas(
            "int",
            &format!(
                r#"
            let List<int, 8> source = [{elements}];
            return source.len();
            "#
            ),
        );
        assert_eq!(
            common::decode_i64_register(&control, 10),
            i64::try_from(active_len).expect("bounded active length")
        );
        let (copied, copied_gas) = run_main_body_with_gas(
            "int",
            &format!(
                r#"
            let List<int, 8> source = [{elements}];
            let List<int, 8> copied = [value for value in source];
            return copied.len();
            "#
            ),
        );
        assert_eq!(
            common::decode_i64_register(&copied, 10),
            i64::try_from(active_len).expect("bounded active length")
        );
        deltas.push((
            active_len,
            positive_gas_delta(copied_gas, control_gas, "List comprehension"),
        ));
    }
    let fixed_delta = deltas[0].1;
    let per_element = deltas[1].1 - fixed_delta;
    assert!(
        per_element > 0,
        "each active source element must consume gas"
    );
    for &(active_len, delta) in &deltas[1..] {
        assert_eq!(
            delta - fixed_delta,
            active_len * per_element,
            "comprehension work must be exactly linear after fixed allocation/loop overhead: {deltas:?}"
        );
    }
}
#[test]
fn enumerate_materializes_bounded_structured_elements() {
    let vm = run(r#"
        seiyaku ListEnumerate {
            view fn main() -> int {
                let List<int, 4> values = [7, 8];
                let indexed = values.enumerate();
                let pair = indexed.get(1).unwrap_or((0, 0));
                pair.0 * 10 + pair.1
            }
        }
        "#);
    assert_eq!(common::decode_i64_register(&vm, 10), 18);
}
#[test]
fn list_of_options_uses_one_word_per_element() {
    let vm = run(r#"
        seiyaku ListOfOptions {
            view fn main() -> List<Option<int>, 4> {
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
    let list_layout = ListLayoutV1::try_new(4, 1).expect("List<Option<int>, 4> layout");
    let elements = ivm::list::read_words(&vm, list, list_layout).expect("read returned List");
    assert_eq!(elements.len(), 2);
    let sum_layout = SumLayoutV1::option(1).expect("Option<int> layout");
    let (present, payload) =
        ivm::sum::read_words(&vm, elements[0][0], sum_layout).expect("read present option");
    assert!(present);
    assert_eq!(payload.len(), 1);
    assert_eq!(
        common::decode_i64_word(&vm, payload[0]),
        7,
        "Option::some payload"
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
                Option<List<int, 3>> labels,
                List<Option<int>, 3> markers,
                Result<(int, bool), int> outcome,
            }

            fn ok(int last, bool ready) -> Envelope {
                Envelope {
                    labels: Option::some([1, last]),
                    markers: [Option::none, Option::some(last)],
                    outcome: Result::ok((9, ready)),
                }
            }

            fn err(int code) -> Envelope {
                Envelope {
                    labels: Option::none,
                    markers: [Option::none, Option::some(7)],
                    outcome: Result::err(code),
                }
            }

            view fn main() -> int {
                let List<Envelope, 2> values = [ok(7, true), err(5)];

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
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn recursive_contains_support_does_not_admit_resource_elements() {
    let error = KotodamaCompiler::new()
        .compile_source(
            r#"
            seiyaku ResourceElements {
                fn reject(List<Option<StateMap<int, int>>, 2> values) {
                    let _values = values;
                }

                view fn main() -> int { 0 }
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

                view fn main() -> int {
                    let List<Empty, 1> values = [Empty {}];
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
                view fn main() -> int {
                    let List<int, 4> values = [];
                    values.len()
                }
            }
            "#,
        )
        .expect("ordinary contextual empty Lists remain valid");
}
