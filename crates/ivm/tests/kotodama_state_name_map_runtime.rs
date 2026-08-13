//! Regressions for durable `StateMap<Name, int>` runtime behavior.
use std::{collections::HashMap, str::FromStr};
use iroha_crypto::PublicKey;
use iroha_data_model::prelude::AccountId;
use ivm::mock_wsv::{MockWorldStateView, WsvHost};
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
fn run_program(src: &str) -> IVM {
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile kotodama");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run program");
    vm
}
fn test_subject() -> AccountId {
    AccountId::new(
        PublicKey::from_str(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
        )
        .expect("valid public key"),
    )
}
fn encoded_int_state_path(name: &str, key: i64) -> String {
    let key = ivm::numeric_tlv::encode_int(&iroha_primitives::bigint::BigInt::from_i128(
        i128::from(key),
    ))
    .expect("encode canonical StateMap int key");
    format!("{name}/{}", hex::encode(key))
}
fn run_program_with_wsv(src: &str, wsv: MockWorldStateView) -> (IVM, MockWorldStateView) {
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile kotodama");
    let subject = test_subject();
    let host = WsvHost::new_with_subject(wsv, subject, HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&code).expect("load program");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run program");
    let wsv = {
        let host_any = vm.host_mut_any().expect("host available");
        let host = host_any.downcast_mut::<WsvHost>().expect("wsv host");
        host.wsv.clone()
    };
    (vm, wsv)
}
#[test]
fn durable_name_map_roundtrip_read_after_write() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_read_modify_write_roundtrip() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                let prior = Foo.get(Name::parse("alice")).unwrap_or(0);
                Foo[Name::parse("alice")] = prior + 1;
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 2);
}
#[test]
fn durable_name_map_if_branch_reassignment_roundtrip() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                var value = 0;
                if (Foo.contains(Name::parse("alice"))) {
                    value = Foo.get(Name::parse("alice")).unwrap_or(0);
                }
                return value;
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_roundtrip_through_name_parameter() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;

            fn read_value(Name key) -> int {
                return Foo.get(key).unwrap_or(0);
            }

            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_roundtrip_through_helper() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;

            fn read_value(Name key) -> int {
                return Foo.ensure(key, 0);
            }

            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_struct_value_roundtrip_through_helper() {
    let src = r#"
        seiyaku C {
            struct Entry { int amount, bool active }
            state StateMap<Name, Entry> Entries;

            fn read_score(Name key) -> int {
                let entry = Entries.get(key).unwrap_or(Entry { amount: 0, active: false });
                var value = entry.amount;
                if (entry.active) {
                    value = value + 1;
                }
                return value;
            }

            kotoage fn main() -> int authorize("WriteState") {
                Entries[Name::parse("alice")] = Entry { amount: 41, active: true };
                return read_score(Name::parse("alice"));
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 42);
}
#[test]
fn missing_mixed_aggregate_option_uses_complete_helper_fallback() {
    let src = r#"
        seiyaku C {
            struct PolicyState {
                int version,
                bytes document,
                bytes document_hash,
                AccountId approved_by,
                int applied_at_ms,
                Name change_id,
            }
            state StateMap<Name, PolicyState> Policies;

            fn empty_policy_state() -> PolicyState {
                return PolicyState {
                    version: 0,
                    document: b"",
                    document_hash: b"",
                    approved_by: context::authority(),
                    applied_at_ms: 0,
                    change_id: Name::parse("initial"),
                };
            }

            view fn main() -> bool {
                let policy = Policies.get(Name::parse("spend")).unwrap_or(
                    empty_policy_state(),
                );
                return policy.version > -1
                    && policy.document == b""
                    && policy.document_hash == b""
                    && policy.approved_by == context::authority()
                    && policy.applied_at_ms == 0
                    && policy.change_id == Name::parse("initial");
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn present_mixed_aggregate_option_ignores_fallback_value_without_field_corruption() {
    let src = r#"
        seiyaku C {
            struct PolicyState {
                int version,
                bytes document,
                bytes document_hash,
                AccountId approved_by,
                int applied_at_ms,
                Name change_id,
            }
            state StateMap<Name, PolicyState> Policies;

            fn empty_policy_state() -> PolicyState {
                return PolicyState {
                    version: 0,
                    document: b"fallback",
                    document_hash: b"fallback-hash",
                    approved_by: context::authority(),
                    applied_at_ms: 0,
                    change_id: Name::parse("fallback"),
                };
            }

            kotoage fn main() -> bool authorize("WriteState") {
                let key = Name::parse("spend");
                Policies[key] = PolicyState {
                    version: 7,
                    document: b"policy",
                    document_hash: b"hash",
                    approved_by: context::authority(),
                    applied_at_ms: 99,
                    change_id: Name::parse("change-7"),
                };
                let policy = Policies.get(key).unwrap_or(empty_policy_state());
                return policy.version == 7
                    && policy.document == b"policy"
                    && policy.document_hash == b"hash"
                    && policy.approved_by == context::authority()
                    && policy.applied_at_ms == 99
                    && policy.change_id == Name::parse("change-7");
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn mixed_aggregate_unwrap_or_remains_eager_on_the_present_arm() {
    let src = r#"
        seiyaku C {
            struct PolicyState {
                int version,
                bytes document,
                bytes document_hash,
                AccountId approved_by,
                int applied_at_ms,
                Name change_id,
            }
            state StateMap<Name, PolicyState> Policies;
            state StateMap<int, int> FallbackCalls;

            fn observed_fallback() -> PolicyState {
                let count = FallbackCalls.get(1).unwrap_or(0);
                FallbackCalls[1] = count + 1;
                return PolicyState {
                    version: 0,
                    document: b"fallback",
                    document_hash: b"fallback-hash",
                    approved_by: context::authority(),
                    applied_at_ms: 0,
                    change_id: Name::parse("fallback"),
                };
            }

            kotoage fn main() -> int authorize("WriteState") {
                let key = Name::parse("spend");
                Policies[key] = PolicyState {
                    version: 7,
                    document: b"policy",
                    document_hash: b"hash",
                    approved_by: context::authority(),
                    applied_at_ms: 99,
                    change_id: Name::parse("change-7"),
                };
                let policy = Policies.get(key).unwrap_or(observed_fallback());
                return policy.version;
            }
        }
    "#;
    let (vm, wsv) = run_program_with_wsv(src, MockWorldStateView::new());
    assert_eq!(common::decode_i64_register(&vm, 10), 7);
    let fallback_calls = wsv
        .sc_get(&encoded_int_state_path("FallbackCalls", 1))
        .expect("eager fallback must persist its observable state mutation");
    assert_eq!(common::decode_int_state_value(&fallback_calls), 1);
}
#[test]
fn mixed_aggregate_unwrap_or_evaluates_fallback_once_on_the_absent_arm() {
    let src = r#"
        seiyaku C {
            struct PolicyState {
                int version,
                bytes document,
                bytes document_hash,
                AccountId approved_by,
                int applied_at_ms,
                Name change_id,
            }
            state StateMap<Name, PolicyState> Policies;
            state StateMap<int, int> FallbackCalls;

            fn observed_fallback() -> PolicyState {
                let count = FallbackCalls.get(1).unwrap_or(0);
                FallbackCalls[1] = count + 1;
                return PolicyState {
                    version: 11,
                    document: b"fallback",
                    document_hash: b"fallback-hash",
                    approved_by: context::authority(),
                    applied_at_ms: 77,
                    change_id: Name::parse("fallback"),
                };
            }

            kotoage fn main() -> int authorize("WriteState") {
                let policy = Policies.get(Name::parse("missing")).unwrap_or(
                    observed_fallback(),
                );
                return policy.version;
            }
        }
    "#;
    let (vm, wsv) = run_program_with_wsv(src, MockWorldStateView::new());
    assert_eq!(common::decode_i64_register(&vm, 10), 11);
    let fallback_calls = wsv
        .sc_get(&encoded_int_state_path("FallbackCalls", 1))
        .expect("selected fallback must persist its observable state mutation");
    assert_eq!(common::decode_int_state_value(&fallback_calls), 1);
}
#[test]
fn missing_nested_mixed_aggregate_option_preserves_every_fallback_word() {
    let src = r#"
        seiyaku C {
            struct PolicyState {
                int version,
                bytes document,
                bytes document_hash,
                AccountId approved_by,
                int applied_at_ms,
                Name change_id,
            }
            struct Envelope { PolicyState policy, bool enabled }
            state StateMap<Name, Envelope> Policies;

            fn default_envelope() -> Envelope {
                return Envelope {
                    policy: PolicyState {
                        version: 3,
                        document: b"nested",
                        document_hash: b"nested-hash",
                        approved_by: context::authority(),
                        applied_at_ms: 55,
                        change_id: Name::parse("nested-change"),
                    },
                    enabled: true,
                };
            }

            view fn main() -> bool {
                let envelope = Policies.get(Name::parse("offline")).unwrap_or(
                    default_envelope(),
                );
                return envelope.enabled
                    && envelope.policy.version == 3
                    && envelope.policy.document == b"nested"
                    && envelope.policy.document_hash == b"nested-hash"
                    && envelope.policy.approved_by == context::authority()
                    && envelope.policy.applied_at_ms == 55
                    && envelope.policy.change_id == Name::parse("nested-change");
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn aggregate_option_rejects_a_different_fallback_shape() {
    let src = r#"
        seiyaku C {
            struct PolicyState { int version, bytes document }
            struct WrongState { int version }
            state StateMap<Name, PolicyState> Policies;

            view fn main() -> int {
                let policy = Policies.get(Name::parse("spend")).unwrap_or(
                    WrongState { version: 0 },
                );
                return policy.version;
            }
        }
    "#;
    let error = KotodamaCompiler::new()
        .compile_source(src)
        .expect_err("unwrap_or must reject a fallback with a different aggregate shape");
    assert!(
        error.contains("unwrap_or")
            || error.contains("type mismatch")
            || error.contains("E_TYPE_ANNOTATION_MISMATCH"),
        "unexpected diagnostic: {error}"
    );
}
#[test]
fn durable_name_map_if_branch_roundtrip_through_name_parameter() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;

            fn read_value(Name key) -> int {
                var value = 0;
                if (Foo.contains(key)) {
                    value = Foo.get(key).unwrap_or(0);
                }
                return value;
            }

            kotoage fn main() -> int authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_roundtrip_across_wsv_invocations() {
    let write_src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            view fn main() -> int {
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;
    let (_, wsv) = run_program_with_wsv(write_src, MockWorldStateView::new());
    let (vm, _) = run_program_with_wsv(read_src, wsv);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_aggregate_get_or_preserves_persisted_bounded_lists() {
    let write_src = r#"
        seiyaku C {
            struct Tranche {
                int status,
                quantity remaining,
                List<string, 4> merchant_aliases,
                List<AccountId, 4> merchant_accounts,
            }
            state StateMap<Name, Tranche> Tranches;

            kotoage fn main() authorize("WriteState") {
                let List<string, 4> aliases = ["merchant@sbp"];
                let List<AccountId, 4> accounts = [context::authority()];
                Tranches[Name::parse("t1")] = Tranche {
                    status: 7,
                    remaining: 42,
                    merchant_aliases: aliases,
                    merchant_accounts: accounts,
                };
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            struct Tranche {
                int status,
                quantity remaining,
                List<string, 4> merchant_aliases,
                List<AccountId, 4> merchant_accounts,
            }
            state StateMap<Name, Tranche> Tranches;

            fn empty_tranche() -> Tranche {
                let List<string, 4> aliases = [];
                let List<AccountId, 4> accounts = [];
                return Tranche {
                    status: 0,
                    remaining: 0,
                    merchant_aliases: aliases,
                    merchant_accounts: accounts,
                };
            }

            view fn main() -> bool {
                let current = Tranches.get_or(
                    key: Name::parse("t1"),
                    default: empty_tranche(),
                );
                let missing = Tranches.get_or_default(
                    key: Name::parse("missing"),
                    default: empty_tranche(),
                );
                return current.status == 7
                    && current.remaining == 42
                    && current.merchant_aliases.len() == 1
                    && current.merchant_aliases.contains("merchant@sbp")
                    && current.merchant_accounts.len() == 1
                    && current.merchant_accounts.contains(context::authority())
                    && missing.status == 0
                    && missing.remaining == 0
                    && missing.merchant_aliases.len() == 0
                    && missing.merchant_accounts.len() == 0;
            }
        }
    "#;
    let (_, wsv) = run_program_with_wsv(write_src, MockWorldStateView::new());
    let (vm, _) = run_program_with_wsv(read_src, wsv);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn durable_aggregate_ensure_preserves_existing_record_words() {
    let src = r#"
        seiyaku C {
            struct Entry { int amount, bool active }
            state StateMap<Name, Entry> Entries;

            kotoage fn main() -> bool authorize("WriteState") {
                let first = Entries.ensure(
                    Name::parse("alice"),
                    Entry { amount: 41, active: true },
                );
                let second = Entries.ensure(
                    Name::parse("alice"),
                    Entry { amount: 99, active: false },
                );
                return first.amount == 41
                    && first.active
                    && second.amount == 41
                    && second.active;
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn durable_name_map_if_branch_roundtrip_across_wsv_invocations() {
    let write_src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            view fn main() -> int {
                var value = 0;
                if (Foo.contains(Name::parse("alice"))) {
                    value = Foo.get(Name::parse("alice")).unwrap_or(0);
                }
                return value;
            }
        }
    "#;
    let (_, wsv) = run_program_with_wsv(write_src, MockWorldStateView::new());
    let (vm, _) = run_program_with_wsv(read_src, wsv);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_to_account_id_map_roundtrip() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, AccountId> Foo;

            kotoage fn main() -> bool authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = context::authority();
                return Foo.contains(key)
                    && Foo.get(key).unwrap_or(context::authority()) == context::authority();
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn durable_name_to_account_id_map_roundtrip_across_wsv_invocations() {
    let write_src = r#"
        seiyaku C {
            state StateMap<Name, AccountId> Foo;

            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = context::authority();
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state StateMap<Name, AccountId> Foo;

            view fn main() -> bool {
                let key = Name::parse("alice");
                return Foo.contains(key)
                    && Foo.get(key).unwrap_or(context::authority()) == context::authority();
            }
        }
    "#;
    let (_, wsv) = run_program_with_wsv(write_src, MockWorldStateView::new());
    let (vm, _) = run_program_with_wsv(read_src, wsv);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn durable_name_to_blob_map_write_from_json_hex_roundtrip() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, bytes> Foo;

            kotoage fn main() -> bool authorize("WriteState") {
                let ev = Json::parse("{\"value_hex\":\"0x68656c6c6f\"}");
                let key = Name::parse("alice");
                if let Option::some(value) = ev.get_blob_hex(Name::parse("value_hex")) {
                    Foo[key] = value;
                } else {
                    return false;
                }
                return Foo.contains(key) && Foo.get(key).unwrap_or(b"") == b"hello";
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn durable_name_map_key_survives_function_call() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;

            fn touch() -> int {
                return 7;
            }

            kotoage fn main() -> int authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = 1;
                let ignored = touch();
                return Foo.get(key).unwrap_or(0);
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_branch_value_drives_following_state_set() {
    let src = r#"
        seiyaku C {
            state StateMap<int, int> Counter;
            state StateMap<Name, int> Foo;

            kotoage fn main() -> int authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = 1;

                var value = 0;
                if (Foo.contains(key)) {
                    value = Foo.get(key).unwrap_or(0);
                }

                Counter[1] = value + 1;
                return Counter.get(1).unwrap_or(0);
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 2);
}
#[test]
fn durable_name_map_branch_value_survives_following_addition() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;

            kotoage fn main() -> int authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = 1;

                var value = 0;
                if (Foo.contains(key)) {
                    value = Foo.get(key).unwrap_or(0);
                }

                return value + 1;
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 2);
}
#[test]
fn durable_name_map_branch_value_survives_path_work() {
    let src = r#"
        seiyaku C {
            state StateMap<Name, int> Foo;
            state StateMap<int, int> EntryByPosition;

            kotoage fn main() -> int authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = 1;

                var value = 0;
                if (Foo.contains(key)) {
                    value = Foo.get(key).unwrap_or(0);
                }

                EntryByPosition[value] = 7;
                return value;
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 1);
}
#[test]
fn durable_name_map_branch_value_survives_following_state_work() {
    let src = r#"
        seiyaku C {
            state StateMap<int, int> Counter;
            state StateMap<Name, int> CountByKey;
            state StateMap<Name, int> IndexById;
            state StateMap<int, int> EntryByPosition;

            fn next_index() -> int {
                let value = Counter.ensure(key: 1, default: 0);
                Counter[1] = value + 1;
                return value;
            }

            kotoage fn main() -> int authorize("WriteState") {
                let tranche_id = Name::parse("t2");
                let beneficiary_lookup_key = Name::parse("alice");
                CountByKey[beneficiary_lookup_key] = 1;

                let index = next_index();
                var beneficiary_lookup_position = 0;
                if (CountByKey.contains(beneficiary_lookup_key)) {
                    beneficiary_lookup_position = CountByKey.get(beneficiary_lookup_key).unwrap_or(0);
                }

                IndexById[tranche_id] = index;
                CountByKey[beneficiary_lookup_key] = beneficiary_lookup_position + 1;
                EntryByPosition[beneficiary_lookup_position] = index;
                return CountByKey.get(beneficiary_lookup_key).unwrap_or(0);
            }
        }
    "#;
    let vm = run_program(src);
    assert_eq!(common::decode_i64_register(&vm, 10), 2);
}
