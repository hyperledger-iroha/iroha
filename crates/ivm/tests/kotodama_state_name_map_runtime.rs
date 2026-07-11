//! Regressions for durable `StateMap<Name, i64>` runtime behavior.

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
            state Foo: StateMap<Name, i64>;
            kotoage fn main() -> i64 authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_read_modify_write_roundtrip() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            kotoage fn main() -> i64 authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                let prior = Foo.get(Name::parse("alice")).unwrap_or(0);
                Foo[Name::parse("alice")] = prior + 1;
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 2);
}

#[test]
fn durable_name_map_if_branch_reassignment_roundtrip() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            kotoage fn main() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_roundtrip_through_name_parameter() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;

            fn read_value(key: Name) -> i64 {
                return Foo.get(key).unwrap_or(0);
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_roundtrip_through_helper() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;

            fn read_value(key: Name) -> i64 {
                return Foo.ensure(key, 0);
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_struct_value_roundtrip_through_helper() {
    let src = r#"
        seiyaku C {
            struct Entry { amount: i64; active: bool; }
            state Entries: StateMap<Name, Entry>;

            fn read_score(key: Name) -> i64 {
                let entry = Entries.get(key).unwrap_or(Entry { amount: 0, active: false });
                var value = entry.amount;
                if (entry.active) {
                    value = value + 1;
                }
                return value;
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                Entries[Name::parse("alice")] = Entry { amount: 41, active: true };
                return read_score(Name::parse("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 42);
}

#[test]
fn durable_name_map_if_branch_roundtrip_through_name_parameter() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;

            fn read_value(key: Name) -> i64 {
                var value = 0;
                if (Foo.contains(key)) {
                    value = Foo.get(key).unwrap_or(0);
                }
                return value;
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
                return read_value(Name::parse("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_roundtrip_across_wsv_invocations() {
    let write_src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            view fn main() -> i64 {
                return Foo.get(Name::parse("alice")).unwrap_or(0);
            }
        }
    "#;

    let (_, wsv) = run_program_with_wsv(write_src, MockWorldStateView::new());
    let (vm, _) = run_program_with_wsv(read_src, wsv);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_if_branch_roundtrip_across_wsv_invocations() {
    let write_src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            view fn main() -> i64 {
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
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_to_account_id_map_roundtrip() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, AccountId>;

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
            state Foo: StateMap<Name, AccountId>;

            kotoage fn main() authorize("WriteState") {
                Foo[Name::parse("alice")] = context::authority();
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state Foo: StateMap<Name, AccountId>;

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
            state Foo: StateMap<Name, bytes>;

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
            state Foo: StateMap<Name, i64>;

            fn touch() -> i64 {
                return 7;
            }

            kotoage fn main() -> i64 authorize("WriteState") {
                let key = Name::parse("alice");
                Foo[key] = 1;
                let ignored = touch();
                return Foo.get(key).unwrap_or(0);
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_branch_value_drives_following_state_set() {
    let src = r#"
        seiyaku C {
            state Counter: StateMap<i64, i64>;
            state Foo: StateMap<Name, i64>;

            kotoage fn main() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 2);
}

#[test]
fn durable_name_map_branch_value_survives_following_addition() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;

            kotoage fn main() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 2);
}

#[test]
fn durable_name_map_branch_value_survives_path_work() {
    let src = r#"
        seiyaku C {
            state Foo: StateMap<Name, i64>;
            state EntryByPosition: StateMap<i64, i64>;

            kotoage fn main() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_branch_value_survives_following_state_work() {
    let src = r#"
        seiyaku C {
            state Counter: StateMap<i64, i64>;
            state CountByKey: StateMap<Name, i64>;
            state IndexById: StateMap<Name, i64>;
            state EntryByPosition: StateMap<i64, i64>;

            fn next_index() -> i64 {
                let value = Counter.ensure(1, 0);
                Counter[1] = value + 1;
                return value;
            }

            kotoage fn main() -> i64 authorize("WriteState") {
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
    assert_eq!(vm.register(10), 2);
}
