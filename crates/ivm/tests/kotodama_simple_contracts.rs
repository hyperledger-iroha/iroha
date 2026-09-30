//! Execute concise contract syntax through the real compiler and IVM ABI.
use ivm::{CoreHost, IVM, ProgramMetadata, VMError};
use kotodama_lang::compiler::Compiler;

mod common;

fn load(source: &str) -> (IVM, Vec<u8>) {
    let code = Compiler::new()
        .compile_source(source)
        .expect("compile concise contract");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load concise contract");
    common::select_kotodama_entrypoint(&mut vm, &code, "run");
    (vm, code)
}

#[test]
fn expect_preserves_zero_empty_bytes_names_and_aggregate_state() {
    let (mut vm, _) = load(
        r#"seiyaku SimpleState {
            error enum StateError { Missing = 1101 }
            const Name KEY = Name::parse("note");
            struct Note { int nonce, bytes commitment, Name source, bool active }
            state StateMap<Name, Note> Notes;
            state StateMap<Name, int> Nonces;
            state StateMap<Name, bytes> Evidence;

            kotoage fn run() -> bool authorize("WriteState") {
                Notes[KEY] = Note {
                    nonce: 0, commitment: b"", source: KEY, active: false
                };
                Nonces[KEY] = 0;
                Evidence[KEY] = b"";
                let note = Notes.get(KEY).expect(StateError::Missing);
                let Option<Name> source = Option::some(note.source);
                return note.nonce == 0 && note.commitment == b"" && !note.active
                    && source.expect(StateError::Missing) == KEY
                    && Nonces.get(KEY).expect(StateError::Missing) == 0
                    && Evidence.get(KEY).expect(StateError::Missing) == b"";
            }
        }"#,
    );
    vm.run().expect("extract present state values");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn expect_absence_aborts_with_the_authenticated_nominal_error() {
    for ty in ["int", "bytes", "Name", "Note", "()"] {
        let source = format!(
            r#"seiyaku MissingState {{
                error enum StateError {{ Missing = 1101 }}
                struct Note {{ int nonce, bytes commitment }}
                view fn run() -> {ty} {{
                    let Option<{ty}> absent = Option::none;
                    return absent.expect(StateError::Missing);
                }}
            }}"#
        );
        let (mut vm, code) = load(&source);
        let metadata = ProgramMetadata::parse(&code).expect("parse signed interface");
        let descriptor = metadata
            .contract_interface
            .as_ref()
            .expect("contract interface")
            .error_types
            .iter()
            .find(|error| error.identity == "MissingState::StateError")
            .expect("declared nominal error descriptor");
        let error = vm
            .run()
            .expect_err("missing state must abort")
            .split_metered()
            .1;
        assert_eq!(
            error,
            VMError::ContractAbort {
                contract: "MissingState".to_owned(),
                name: "Missing".to_owned(),
                error_type: descriptor.identity.clone(),
                schema_hash: descriptor.schema_hash(),
                code: 1101,
            },
            "missing {ty} must preserve the nominal error identity"
        );
    }
}

#[test]
fn expect_evaluates_a_stateful_receiver_once() {
    let (mut vm, _) = load(
        r#"seiyaku Once {
            error enum StateError { Missing = 1101 }
            state StateMap<int, int> Calls;
            fn next() -> Option<int> {
                let count = Calls.get(0).unwrap_or(0) + 1;
                Calls[0] = count;
                return Option::some(count);
            }
            kotoage fn run() -> bool authorize("WriteState") {
                let value = next().expect(StateError::Missing);
                return value == 1 && Calls.get(0).expect(StateError::Missing) == 1;
            }
        }"#,
    );
    vm.run().expect("execute receiver once");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn positional_and_named_calls_preserve_source_evaluation_order() {
    let (mut vm, _) = load(
        r#"seiyaku SimpleCalls {
            state StateMap<int, int> Calls;
            fn next() -> int {
                let count = Calls.get(0).unwrap_or(0) + 1;
                Calls[0] = count;
                return count;
            }
            fn pair(int left, int right) -> int { return left * 10 + right; }
            kotoage fn run() -> bool authorize("WriteState") {
                let positional = pair(next(), next());
                let named = pair(right: next(), left: next());
                return positional == 12 && named == 43
                    && Calls.get(0).unwrap_or(0) == 4;
            }
        }"#,
    );
    vm.run().expect("execute positional and named calls");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn mutable_record_fields_roundtrip_without_changing_other_fields() {
    let (mut vm, _) = load(
        r#"seiyaku RecordUpdates {
            error enum StateError { Missing = 1101 }
            struct Consent { int nonce, bool approved }
            struct Note { Name source, bytes commitment, Consent consent }
            state StateMap<int, Note> Notes;

            kotoage fn run() -> bool authorize("WriteState") {
                Notes[1] = Note {
                    source: Name::parse("invoice"), commitment: b"original",
                    consent: Consent { nonce: 0, approved: false }
                };
                var note = Notes.get(1).expect(StateError::Missing);
                let original = note;
                note.consent.nonce = 1;
                note.consent.approved = true;
                note.commitment = b"approved";
                Notes[1] = note;
                let saved = Notes.get(1).expect(StateError::Missing);
                return saved.consent.nonce == 1 && saved.consent.approved
                    && saved.commitment == b"approved"
                    && saved.source == Name::parse("invoice")
                    && original.consent.nonce == 0 && !original.consent.approved
                    && original.commitment == b"original";
            }
        }"#,
    );
    vm.run().expect("mutate and persist selected record fields");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn record_fields_preserve_mutability_and_type_checks() {
    for body in [
        "let note = Note { nonce: 0 }; note.nonce = 1;",
        "var note = Note { nonce: 0 }; note.nonce = true;",
    ] {
        let source = format!(
            "seiyaku Immutable {{ struct Note {{ int nonce }} view fn run() {{ {body} }} }}"
        );
        assert!(
            Compiler::new().compile_source(&source).is_err(),
            "invalid record mutation must be rejected: {body}"
        );
    }
}

#[test]
fn wide_records_cross_stack_table_windows_without_corrupting_fields() {
    let fields = (0..40)
        .map(|index| format!("int f{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let initial = (0..40)
        .map(|index| format!("f{index}: {index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let checks = (0..40)
        .map(|index| {
            let expected = match index {
                0 => 100,
                39 => 139,
                other => other,
            };
            format!("saved.f{index} == {expected}")
        })
        .collect::<Vec<_>>()
        .join(" && ");
    let source = format!(
        r#"seiyaku WideRecord {{
            error enum StateError {{ Missing = 1101 }}
            struct Record {{ {fields} }}
            state StateMap<int, Record> Records;
            fn adjust(Record original) -> Record {{
                var updated = original;
                updated.f0 += 100;
                updated.f39 += 100;
                return updated;
            }}
            kotoage fn run() -> bool authorize("WriteState") {{
                Records[0] = Record {{ {initial} }};
                let original = Records.get(0).expect(StateError::Missing);
                Records[0] = adjust(original);
                let saved = Records.get(0).expect(StateError::Missing);
                return {checks} && original.f0 == 0 && original.f39 == 39;
            }}
        }}"#
    );
    let (mut vm, _) = load(&source);
    vm.run()
        .expect("transfer all fields across stack table windows");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn record_updates_survive_control_flow_merges_and_evaluate_rhs_once() {
    let (mut vm, _) = load(
        r#"seiyaku RecordFlow {
            struct Consent { int nonce, bool approved }
            struct Note { bytes evidence, Consent consent }
            state StateMap<int, int> Calls;
            fn next() -> int {
                let count = Calls.get(0).unwrap_or(0) + 1;
                Calls[0] = count;
                return count;
            }
            kotoage fn run() -> bool authorize("WriteState") {
                var note = Note {
                    evidence: b"retained", consent: Consent { nonce: 0, approved: false }
                };
                for i in range(3) {
                    if i == 1 {
                        note.consent.approved = true;
                    }
                    note.consent.nonce += next();
                }
                return note.consent.nonce == 6 && note.consent.approved
                    && note.evidence == b"retained" && Calls.get(0).unwrap_or(0) == 3;
            }
        }"#,
    );
    vm.run()
        .expect("merge record field updates across branches and loops");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn durable_record_fields_observe_updates_from_effectful_helpers() {
    let (mut vm, _) = load(
        r#"seiyaku DurableRecord {
            struct Counter { int count, bytes evidence }
            state Counter Current;
            hajimari() { Current = Counter { count: 0, evidence: b"original" }; }
            fn bump() { Current.count += 1; }
            kotoage fn run() -> bool authorize("WriteState") {
                Current = Counter { count: 0, evidence: b"original" };
                Current.count = 2;
                bump();
                return Current.count == 3 && Current.evidence == b"original";
            }
        }"#,
    );
    vm.run()
        .expect("read durable fields after an effectful helper");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}

#[test]
fn nested_tuple_record_swaps_survive_continue_and_break() {
    let (mut vm, _) = load(
        r#"seiyaku RecordControl {
            struct Note { int value, bytes evidence, bool approved }
            view fn run() -> bool {
                var pair = (
                    Note { value: 1, evidence: b"first", approved: false },
                    Note { value: 2, evidence: b"second", approved: false }
                );
                let original = pair;
                var iterations = 0;
                for i in range(4) {
                    let left = pair.0;
                    let right = pair.1;
                    pair.0 = right;
                    pair.1 = left;
                    iterations += 1;
                    if i == 0 { continue; }
                    if i == 2 { break; }
                    pair.0.approved = true;
                }
                return iterations == 3
                    && pair.0.value == 2 && pair.0.evidence == b"second" && !pair.0.approved
                    && pair.1.value == 1 && pair.1.evidence == b"first" && pair.1.approved
                    && original.0.value == 1 && !original.0.approved
                    && original.1.value == 2 && !original.1.approved;
            }
        }"#,
    );
    vm.run()
        .expect("copy overlapping product fields through continue and break edges");
    assert_eq!(vm.public_call_result_word(0), Ok(1));
}
