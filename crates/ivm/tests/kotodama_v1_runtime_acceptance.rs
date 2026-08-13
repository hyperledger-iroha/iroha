//! End-to-end acceptance coverage for Kotodama V1 runtime semantics.
use std::{collections::BTreeMap, fmt::Write as _};
use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{
    CoreHost, IVM, ProgramMetadata, host::DefaultHost, kotodama::compiler::Compiler,
    pointer_abi::PointerType,
};
use norito::json as njson;
mod common;
const MAX_INT: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
const MAX_INT_MINUS_ONE: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042046";
const MIN_INT: &str = "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048";
const MIN_INT_PLUS_ONE: &str = "-6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
fn compile_and_run(source: &str) -> IVM {
    let code = Compiler::new()
        .compile_source(source)
        .expect("compile V1 contract");
    let parsed = ProgramMetadata::parse(&code).expect("parse V1 contract metadata");
    let run = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .expect("run entrypoint descriptor");
    let entry_pc = u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + run.entry_pc;
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load V1 contract");
    vm.set_program_counter(entry_pc)
        .expect("select run entrypoint");
    vm.run().expect("run V1 contract");
    vm
}
fn compile_and_run_with_default_host(source: &str) -> IVM {
    let code = Compiler::new()
        .compile_source(source)
        .expect("compile V1 contract for the standalone host");
    let parsed = ProgramMetadata::parse(&code).expect("parse V1 contract metadata");
    let run = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .expect("run entrypoint descriptor");
    let entry_pc = u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + run.entry_pc;
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).expect("load V1 contract");
    vm.set_program_counter(entry_pc)
        .expect("select run entrypoint");
    vm.run().expect("run V1 contract with DefaultHost");
    vm
}
fn compile_init_and_run(source: &str) -> IVM {
    let code = Compiler::new()
        .compile_source(source)
        .expect("compile initialized V1 contract");
    let parsed = ProgramMetadata::parse(&code).expect("parse initialized V1 contract metadata");
    let interface = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface");
    let entry_pc = |name: &str| {
        let entry = interface
            .entrypoints
            .iter()
            .find(|entrypoint| entrypoint.name == name)
            .unwrap_or_else(|| panic!("missing {name} entrypoint"));
        u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + entry.entry_pc
    };
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load initialized V1 contract");
    vm.set_program_counter(entry_pc("hajimari"))
        .expect("select hajimari entrypoint");
    vm.run().expect("initialize V1 contract");
    vm.reset();
    vm.set_gas_limit(u64::MAX);
    vm.set_program_counter(entry_pc("run"))
        .expect("select run entrypoint");
    vm.run().expect("run initialized V1 contract");
    vm
}
fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
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
fn argument_host(
    schema: &ivm_abi::entrypoint::EntrypointArgumentSchemaV1,
    left: &str,
    right: &str,
) -> DefaultHost {
    let payload = Json::from_str_norito(&format!(r#"{{"left":"{left}","right":"{right}"}}"#))
        .expect("valid comparison arguments");
    let payload = ivm::encode_argument_record_from_json(schema, &payload)
        .expect("encode comparison argument record");
    let key: Name = "trigger_event_json".parse().expect("public input key");
    DefaultHost::new().with_public_inputs(BTreeMap::from([(
        key,
        tlv(PointerType::NoritoBytes, &payload),
    )]))
}
#[test]
fn call_aware_allocation_preserves_internal_call_and_tuple_results() {
    let source = r#"
seiyaku CallAwareRuntime {
  fn swap(bool left, bool right) -> (bool, bool) {
    return (right, left);
  }

  fn relay(bool value) -> bool {
    return value;
  }

  view fn run() -> bool {
    let pair = swap(left: false, right: true);
    let checked = pair.0 && !pair.1;
    return relay(checked);
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn mixed_value_and_divergent_tails_execute_both_paths_without_unit_fallthrough() {
    let source = r#"
seiyaku MixedTailRuntime {
  fn via_if(bool flag) -> int {
    if flag { 7 } else { return 9; }
  }

  fn via_if_let(Option<int> value) -> int {
    if let Option::some(item) = value { item } else { return 0; }
  }

  fn via_match(Option<int> value) -> int {
    match value {
      Option::some(item) => item,
      Option::none => { return 0; },
    }
  }

  view fn run() -> int {
    via_if(true)
      + via_if(false)
      + via_if_let(Option::some(4))
      + via_if_let(Option::none)
      + via_match(Option::some(5))
      + via_match(Option::none)
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 25);
}
#[test]
fn result_if_let_executes_both_tags_and_binds_only_the_selected_payload() {
    let source = r#"
seiyaku ResultIfLetRuntime {
  fn via_ok(Result<int, bool> value) -> int {
    if let Result::ok(item) = value { item } else { 9 }
  }

  fn via_err(Result<int, bool> value) -> int {
    if let Result::err(failure) = value {
      if failure { 7 } else { 6 }
    } else {
      8
    }
  }

  view fn run() -> int {
    via_ok(Result::ok(4)) * 1000
      + via_ok(Result::err(true)) * 100
      + via_err(Result::err(true)) * 10
      + via_err(Result::ok(5))
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 4978);
}
#[test]
fn state_map_get_distinguishes_absent_present_zero_and_removal() {
    let source = r#"
seiyaku StateMapOptionAcceptance {
  error enum StateMapError {
    MissingReportedPresent = 1,
    MissingFallbackIgnored = 2,
    PresentReportedMissing = 3,
    PresentZeroLost = 4,
    RemovalReportedPresent = 5,
    RemovalLostValue = 6
  }

  state StateMap<int, int> Values;

  kotoage fn run() -> int authorize("WriteState") {
    let missing = Values.get(7);
    require(missing.is_none(), StateMapError::MissingReportedPresent);
    require(missing.unwrap_or(91) == 91, StateMapError::MissingFallbackIgnored);

    Values[7] = 0;
    let present = Values.get(7);
    require(present.is_some(), StateMapError::PresentReportedMissing);
    require(present.unwrap_or(91) == 0, StateMapError::PresentZeroLost);

    let deleted = Values.remove(7);
    require(deleted.is_some(), StateMapError::RemovalReportedPresent);
    require(deleted.unwrap_or(91) == 0, StateMapError::RemovalLostValue);
    let removed = Values.get(7);
    require(removed.is_none(), StateMapError::RemovalReportedPresent);
    return 1;
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn aggregate_state_map_value_roundtrips_as_one_record() {
    let source = r#"
seiyaku AggregateStateAcceptance {
  error enum AggregateError {
    MissingValue = 1,
    WrongCount = 2,
    WrongFlag = 3,
    RemovalLostValue = 4,
    RemovalDidNotDelete = 5
  }

  struct Pair { int count, bool ready }
  state StateMap<int, Pair> Values;

  kotoage fn run() -> int authorize("WriteState") {
    Values[7] = Pair { count: 9, ready: true };
    let found = Values.get(7);
    require(found.is_some(), AggregateError::MissingValue);
    let pair = found.unwrap_or(Pair { count: 0, ready: false });
    require(pair.count == 9, AggregateError::WrongCount);
    require(pair.ready, AggregateError::WrongFlag);

    let removed = Values.remove(7);
    let old = removed.unwrap_or(Pair { count: 0, ready: false });
    require(old.count == 9, AggregateError::RemovalLostValue);
    require(Values.get(7).is_none(), AggregateError::RemovalDidNotDelete);
    return pair.count;
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 9);
}
#[test]
fn aggregate_option_and_result_unwrap_merge_each_payload_word() {
    let source = r#"
seiyaku AggregateSumAcceptance {
  error enum AggregateSumError {
    SomeLost = 1,
    NoneFallbackLost = 2,
    OkLost = 3,
    ErrFallbackLost = 4,
    ResultErrorLost = 5
  }

  struct Pair { int count, bool ready }

  view fn run() -> int {
    let Option<Pair> some = Option::some(Pair { count: 7, ready: true });
    let from_some = some.unwrap_or(Pair { count: 90, ready: false });
    require(from_some.count == 7 && from_some.ready, AggregateSumError::SomeLost);

    let Option<Pair> none = Option::none;
    let from_none = none.unwrap_or(Pair { count: 11, ready: true });
    require(from_none.count == 11 && from_none.ready, AggregateSumError::NoneFallbackLost);

    let Result<Pair, Pair> ok = Result::ok(Pair { count: 13, ready: true });
    let from_ok = ok.unwrap_or(Pair { count: 91, ready: false });
    require(from_ok.count == 13 && from_ok.ready, AggregateSumError::OkLost);

    let Result<Pair, Pair> err = Result::err(Pair { count: 17, ready: true });
    let from_err = err.unwrap_or(Pair { count: 19, ready: true });
    require(from_err.count == 19 && from_err.ready, AggregateSumError::ErrFallbackLost);
    let error_value = err.unwrap_err_or(Pair { count: 92, ready: false });
    require(error_value.count == 17 && error_value.ready, AggregateSumError::ResultErrorLost);
    return from_some.count + from_none.count + from_ok.count + from_err.count + error_value.count;
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 67);
}
#[test]
fn propagation_materializes_the_enclosing_sum_layout_on_failure() {
    let result_source = r#"
seiyaku ResultPropagationLayoutAcceptance {
  fn source() -> Result<int, bool> {
    Result::err(true)
  }

  view fn run() -> Result<(int, int), bool> {
    let value = source()?;
    Result::ok((value, value))
  }
}
"#;
    let result_vm = compile_and_run(result_source);
    let result_layout = ivm::sum::SumLayoutV1::try_new(1, 2).expect("Result layout");
    assert_eq!(
        ivm::sum::read_words(&result_vm, result_vm.register(10), result_layout),
        Ok((false, vec![1])),
        "the returned error must occupy the wider enclosing Result allocation"
    );
    let option_source = r#"
seiyaku OptionPropagationLayoutAcceptance {
  fn source() -> Option<int> {
    Option::none
  }

  view fn run() -> Option<(int, int)> {
    let value = source()?;
    Option::some((value, value))
  }
}
"#;
    let option_vm = compile_and_run(option_source);
    let option_layout = ivm::sum::SumLayoutV1::option(2).expect("Option layout");
    assert_eq!(
        ivm::sum::read_words(&option_vm, option_vm.register(10), option_layout),
        Ok((false, vec![])),
        "the returned none must occupy the wider enclosing Option allocation"
    );
}
#[test]
fn native_json_executes_once_and_returns_canonical_recursive_values() {
    let source = r#"
seiyaku NativeJsonRuntimeAcceptance {
  view fn run() -> Json {
    let List<string, 4> labels = ["primary", "secondary"];
    var List<bytes, 2> blobs = [];
    blobs.try_push(b"\xaa");
    blobs.try_push(b"\xbb");
    let Option<quantity> maybe = Option::some(1.25);
    json {
      z_bytes: b"\xab\x01",
      maybe: maybe,
      labels: labels,
      blobs: blobs,
      amount: 1.25,
    }
  }
}
"#;
    let vm = compile_and_run(source);
    let output = vm
        .validate_tlv(vm.register(10))
        .expect("native JSON result TLV");
    assert_eq!(output.type_id, PointerType::Json);
    let json: Json = norito::decode_from_bytes(output.payload).expect("decode native JSON result");
    let value: njson::Value = json
        .clone()
        .try_into_any_norito()
        .expect("convert native JSON result");
    assert_eq!(
        value,
        norito::json!({
            "amount": "1.25",
            "blobs": ["0xaa", "0xbb"],
            "labels": ["primary", "secondary"],
            "maybe": "1.25",
            "z_bytes": "0xab01",
        })
    );
    let rendered = njson::to_string(&json).expect("render native JSON result");
    let key_positions = ["amount", "blobs", "labels", "maybe", "z_bytes"].map(|key| {
        rendered
            .find(&format!("\"{key}\""))
            .unwrap_or_else(|| panic!("missing canonical key `{key}` in {rendered}"))
    });
    assert!(
        key_positions.windows(2).all(|pair| pair[0] < pair[1]),
        "object keys must be encoded in canonical lexical order: {rendered}"
    );
}
#[test]
fn native_json_and_typed_getters_execute_with_default_host() {
    let source = r#"
seiyaku DefaultHostNativeJsonAcceptance {
  view fn run() -> int {
    let Json payload = json { value: 7 };
    match payload.get_int(Name::parse("value")) {
      Option::some(value) => value,
      Option::none => 0,
    }
  }
}
"#;
    let vm = compile_and_run_with_default_host(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 7);
}
#[test]
fn scalar_and_aggregate_state_roots_roundtrip_as_schema_bound_records() {
    let source = r#"
seiyaku StateRootAcceptance {
  error enum StateRootError {
    ScalarLost = 1,
    StructLost = 2,
    OptionLost = 3,
    ResultLost = 4
  }

  struct Pair { int count, bool ready }
  state int Counter;
  state Pair Current;
  state Option<Pair> Maybe;
  state Result<Pair, Pair> Outcome;

  hajimari() {
    Counter = 3;
    Current = Pair { count: 5, ready: true };
    Maybe = Option::some(Pair { count: 7, ready: true });
    Outcome = Result::ok(Pair { count: 11, ready: true });
  }

  kotoage fn run() -> int authorize("WriteState") {
    require(Counter == 3, StateRootError::ScalarLost);
    require(Current.count == 5 && Current.ready, StateRootError::StructLost);
    let maybe = Maybe.unwrap_or(Pair { count: 0, ready: false });
    require(maybe.count == 7 && maybe.ready, StateRootError::OptionLost);
    let outcome = Outcome.unwrap_or(Pair { count: 0, ready: false });
    require(outcome.count == 11 && outcome.ready, StateRootError::ResultLost);
    Counter = Counter + 1;
    return Counter + Current.count + maybe.count + outcome.count;
  }
}
"#;
    let vm = compile_init_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 27);
}
#[test]
fn pointer_literal_state_is_materialized_before_record_encoding() {
    let source = r#"
seiyaku PointerStateAcceptance {
  error enum PointerStateError { LiteralLost = 1 }
  state string Message;

  hajimari() { Message = "言霊"; }

  view fn run() -> int {
    require(Message == "言霊", PointerStateError::LiteralLost);
    return 1;
  }
}
"#;
    let vm = compile_init_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn logical_operators_short_circuit_state_side_effects() {
    let source = r#"
seiyaku ShortCircuitAcceptance {
  error enum LogicError {
    FalseAndChangedResult = 1,
    TrueOrChangedResult = 2,
    TrueAndChangedResult = 3,
    FalseOrChangedResult = 4,
    WrongSideEffectCount = 5
  }

  state StateMap<int, int> Hits;

  fn bump(bool result) -> bool {
    let previous = Hits.get_or(key: 0, default: 0);
    Hits[0] = previous + 1;
    return result;
  }

  kotoage fn run() -> int authorize("WriteState") {
    let false_and = false && bump(true);
    let true_or = true || bump(false);
    let true_and = true && bump(true);
    let false_or = false || bump(true);

    require(!false_and, LogicError::FalseAndChangedResult);
    require(true_or, LogicError::TrueOrChangedResult);
    require(true_and, LogicError::TrueAndChangedResult);
    require(false_or, LogicError::FalseOrChangedResult);
    let count = Hits.get_or(key: 0, default: 0);
    require(count == 2, LogicError::WrongSideEffectCount);
    return count;
  }
}
"#;
    let vm = compile_and_run(source);
    assert_eq!(common::decode_i64_register(&vm, 10), 2);
}
#[test]
fn state_map_iteration_uses_canonical_norito_byte_order_for_sixty_four_items() {
    let mut source = String::from(
        r#"
seiyaku StateMapIterationAcceptance {
  error enum IterationError {
    OutOfOrder = 1,
    WrongValue = 2,
    WrongItemCount = 3
  }

  state StateMap<int, int> Values;

  kotoage fn run() -> int authorize("WriteState") {
"#,
    );
    let inserted = (-32_i64..32).rev().collect::<Vec<_>>();
    for key in &inserted {
        writeln!(source, "    Values[{key}] = {};", key * 2).expect("write generated assignment");
    }
    source.push_str(
        r#"
    var int seen = 0;
    for (key, value) in Values.take(64) {
"#,
    );
    let mut expected = inserted;
    expected.sort_by_key(|key| {
        ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(i128::from(
            *key,
        )))
        .expect("encode canonical pointer-backed int key")
    });
    for (index, key) in expected.into_iter().enumerate() {
        writeln!(
            source,
            "      if (seen == {index}) {{ require(key == {key}, IterationError::OutOfOrder); }}"
        )
        .expect("write canonical-order assertion");
    }
    source.push_str(
        r#"
      require(value == key * 2, IterationError::WrongValue);
      seen = seen + 1;
    }
    require(seen == 64, IterationError::WrongItemCount);
    return seen;
  }
}
"#,
    );
    let vm = compile_and_run(&source);
    assert_eq!(common::decode_i64_register(&vm, 10), 64);
}
#[test]
fn signed_comparisons_match_all_boundary_pairs_in_values_and_branches() {
    let source = r#"
seiyaku SignedComparisonAcceptance {
  view fn compare(int left, int right) -> (bool, bool, bool, bool, bool, bool, int, int, int, int, int, int) {
    var int branch_eq = 0;
    var int branch_ne = 0;
    var int branch_lt = 0;
    var int branch_le = 0;
    var int branch_gt = 0;
    var int branch_ge = 0;
    if (left == right) { branch_eq = 1; }
    if (left != right) { branch_ne = 1; }
    if (left < right) { branch_lt = 1; }
    if (left <= right) { branch_le = 1; }
    if (left > right) { branch_gt = 1; }
    if (left >= right) { branch_ge = 1; }
    return (
      left == right,
      left != right,
      left < right,
      left <= right,
      left > right,
      left >= right,
      branch_eq,
      branch_ne,
      branch_lt,
      branch_le,
      branch_gt,
      branch_ge
    );
  }
}
"#;
    let code = Compiler::new()
        .compile_source(source)
        .expect("compile signed comparison contract");
    let parsed = ProgramMetadata::parse(&code).expect("parse comparison metadata");
    let interface = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface");
    let compare = interface
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "compare")
        .expect("compare entrypoint descriptor");
    let entry_pc = u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + compare.entry_pc;
    let argument_schema = compare
        .argument_schema
        .as_ref()
        .expect("comparison argument schema");
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).expect("load comparison contract");
    vm.set_program_counter(entry_pc)
        .expect("select comparison wrapper");
    let template = vm.runtime_template();
    let code_len = vm.memory.code_len();
    let loaded_code = vm
        .memory
        .load_region(0, code_len)
        .expect("loaded code region")
        .to_vec();
    let values = [
        MIN_INT,
        MIN_INT_PLUS_ONE,
        "-1",
        "0",
        "1",
        MAX_INT_MINUS_ONE,
        MAX_INT,
    ];
    for (left_index, left) in values.into_iter().enumerate() {
        for (right_index, right) in values.into_iter().enumerate() {
            vm.reset_from_runtime_template(&template)
                .expect("acceptance VM retains its template geometry");
            vm.set_host(argument_host(argument_schema, left, right));
            vm.run().expect("execute signed comparison pair");
            let expected = [
                left_index == right_index,
                left_index != right_index,
                left_index < right_index,
                left_index <= right_index,
                left_index > right_index,
                left_index >= right_index,
            ];
            for (index, expected) in expected.into_iter().enumerate() {
                assert_eq!(
                    vm.register(10 + index),
                    u64::from(expected),
                    "value comparison {index} failed for {left} and {right}"
                );
                assert_eq!(
                    common::decode_i64_register(&vm, 16 + index),
                    i64::from(expected),
                    "branch comparison {index} failed for {left} and {right}"
                );
            }
            assert_eq!(vm.memory.code_len(), code_len);
            assert_eq!(
                vm.memory
                    .load_region(0, code_len)
                    .expect("warm code region"),
                loaded_code.as_slice(),
                "warm invocation must retain the loaded bytecode"
            );
        }
    }
}
