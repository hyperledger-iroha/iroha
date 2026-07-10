//! End-to-end acceptance coverage for Kotodama V1 runtime semantics.

use std::{collections::BTreeMap, fmt::Write as _};

use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{
    CoreHost, IVM, ProgramMetadata, host::DefaultHost, kotodama::compiler::Compiler,
    pointer_abi::PointerType,
};

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
    left: i64,
    right: i64,
) -> DefaultHost {
    let payload = Json::from_str_norito(&format!(r#"{{"left":{left},"right":{right}}}"#))
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

  state Values: StateMap<i64, i64>;

  kotoage fn run() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 1);
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

  struct Pair { count: i64, ready: bool }
  state Values: StateMap<i64, Pair>;

  kotoage fn run() -> i64 authorize("WriteState") {
    Values[7] = Pair(9, true);
    let found = Values.get(7);
    require(found.is_some(), AggregateError::MissingValue);
    let pair = found.unwrap_or(Pair(0, false));
    require(pair.count == 9, AggregateError::WrongCount);
    require(pair.ready, AggregateError::WrongFlag);

    let removed = Values.remove(7);
    let old = removed.unwrap_or(Pair(0, false));
    require(old.count == 9, AggregateError::RemovalLostValue);
    require(Values.get(7).is_none(), AggregateError::RemovalDidNotDelete);
    return pair.count;
  }
}
"#;

    let vm = compile_and_run(source);
    assert_eq!(vm.register(10), 9);
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

  struct Pair { count: i64, ready: bool }

  view fn run() -> i64 {
    let some = option::some(Pair(7, true));
    let from_some = some.unwrap_or(Pair(90, false));
    require(from_some.count == 7 && from_some.ready, AggregateSumError::SomeLost);

    let none = option::none(Pair(0, false));
    let from_none = none.unwrap_or(Pair(11, true));
    require(from_none.count == 11 && from_none.ready, AggregateSumError::NoneFallbackLost);

    let ok = result::ok(Pair(13, true), Pair(0, false));
    let from_ok = ok.unwrap_or(Pair(91, false));
    require(from_ok.count == 13 && from_ok.ready, AggregateSumError::OkLost);

    let err = result::err(Pair(0, false), Pair(17, true));
    let from_err = err.unwrap_or(Pair(19, true));
    require(from_err.count == 19 && from_err.ready, AggregateSumError::ErrFallbackLost);
    let error_value = err.unwrap_err_or(Pair(92, false));
    require(error_value.count == 17 && error_value.ready, AggregateSumError::ResultErrorLost);
    return from_some.count + from_none.count + from_ok.count + from_err.count + error_value.count;
  }
}
"#;

    let vm = compile_and_run(source);
    assert_eq!(vm.register(10), 67);
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

  struct Pair { count: i64, ready: bool }
  state Counter: i64;
  state Current: Pair;
  state Maybe: Option<Pair>;
  state Outcome: Result<Pair, Pair>;

  hajimari() {
    Counter = 3;
    Current = Pair(5, true);
    Maybe = option::some(Pair(7, true));
    Outcome = result::ok(Pair(11, true), Pair(0, false));
  }

  kotoage fn run() -> i64 authorize("WriteState") {
    require(Counter == 3, StateRootError::ScalarLost);
    require(Current.count == 5 && Current.ready, StateRootError::StructLost);
    let maybe = Maybe.unwrap_or(Pair(0, false));
    require(maybe.count == 7 && maybe.ready, StateRootError::OptionLost);
    let outcome = Outcome.unwrap_or(Pair(0, false));
    require(outcome.count == 11 && outcome.ready, StateRootError::ResultLost);
    Counter = Counter + 1;
    return Counter + Current.count + maybe.count + outcome.count;
  }
}
"#;

    let vm = compile_init_and_run(source);
    assert_eq!(vm.register(10), 27);
}

#[test]
fn pointer_literal_state_is_materialized_before_record_encoding() {
    let source = r#"
seiyaku PointerStateAcceptance {
  error enum PointerStateError { LiteralLost = 1 }
  state Message: string;

  hajimari() { Message = "言霊"; }

  view fn run() -> i64 {
    require(Message == "言霊", PointerStateError::LiteralLost);
    return 1;
  }
}
"#;

    let vm = compile_init_and_run(source);
    assert_eq!(vm.register(10), 1);
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

  state Hits: StateMap<i64, i64>;

  fn bump(result: bool) -> bool {
    let previous = Hits.get_or(0, 0);
    Hits[0] = previous + 1;
    return result;
  }

  kotoage fn run() -> i64 authorize("WriteState") {
    let false_and = false && bump(true);
    let true_or = true || bump(false);
    let true_and = true && bump(true);
    let false_or = false || bump(true);

    require(!false_and, LogicError::FalseAndChangedResult);
    require(true_or, LogicError::TrueOrChangedResult);
    require(true_and, LogicError::TrueAndChangedResult);
    require(false_or, LogicError::FalseOrChangedResult);
    let count = Hits.get_or(0, 0);
    require(count == 2, LogicError::WrongSideEffectCount);
    return count;
  }
}
"#;

    let vm = compile_and_run(source);
    assert_eq!(vm.register(10), 2);
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

  state Values: StateMap<i64, i64>;

  kotoage fn run() -> i64 authorize("WriteState") {
"#,
    );
    let inserted = (-32_i64..32).rev().collect::<Vec<_>>();
    for key in &inserted {
        writeln!(source, "    Values[{key}] = {};", key * 2).expect("write generated assignment");
    }
    source.push_str(
        r#"
    var seen: i64 = 0;
    for (key, value) in Values.take(64) {
"#,
    );
    let mut expected = inserted;
    expected.sort_by_key(|key| norito::to_bytes(key).expect("encode canonical i64 key"));
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
    assert_eq!(vm.register(10), 64);
}

#[test]
fn signed_comparisons_match_all_boundary_pairs_in_values_and_branches() {
    let source = r#"
seiyaku SignedComparisonAcceptance {
  view fn compare(left: i64, right: i64) -> (bool, bool, bool, bool, bool, bool, i64, i64, i64, i64, i64, i64) {
    var branch_eq: i64 = 0;
    var branch_ne: i64 = 0;
    var branch_lt: i64 = 0;
    var branch_le: i64 = 0;
    var branch_gt: i64 = 0;
    var branch_ge: i64 = 0;
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

    let values = [i64::MIN, i64::MIN + 1, -1, 0, 1, i64::MAX - 1, i64::MAX];
    for left in values {
        for right in values {
            vm.reset_from_runtime_template(&template);
            vm.set_host(argument_host(argument_schema, left, right));
            vm.run().expect("execute signed comparison pair");

            let expected = [
                left == right,
                left != right,
                left < right,
                left <= right,
                left > right,
                left >= right,
            ];
            for (index, expected) in expected.into_iter().enumerate() {
                assert_eq!(
                    vm.register(10 + index),
                    u64::from(expected),
                    "value comparison {index} failed for {left} and {right}"
                );
                assert_eq!(
                    vm.register(16 + index),
                    u64::from(expected),
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
