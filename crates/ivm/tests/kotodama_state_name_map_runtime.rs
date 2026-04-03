//! Regressions for durable `Map<Name, int>` runtime behavior.

use std::{collections::HashMap, str::FromStr};

use iroha_crypto::PublicKey;
use iroha_data_model::prelude::AccountId;
use ivm::mock_wsv::{MockWorldStateView, WsvHost};
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};

fn run_program(src: &str) -> IVM {
    let code = KotodamaCompiler::new()
        .compile_source(src)
        .expect("compile kotodama");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code).expect("load program");
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
            state Foo: Map<Name, int>;
            fn main() -> int {
                Foo[name("alice")] = 1;
                return Foo[name("alice")];
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
            state Foo: Map<Name, int>;
            fn main() -> int {
                Foo[name("alice")] = 1;
                let prior = Foo[name("alice")];
                Foo[name("alice")] = prior + 1;
                return Foo[name("alice")];
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
            state Foo: Map<Name, int>;
            fn main() -> int {
                Foo[name("alice")] = 1;
                let value = 0;
                if (Foo.contains(name("alice"))) {
                    value = Foo[name("alice")];
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
            state Foo: Map<Name, int>;

            fn read_value(key: Name) -> int {
                return Foo[key];
            }

            fn main() -> int {
                Foo[name("alice")] = 1;
                return read_value(name("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_roundtrip_through_state_parameter() {
    let src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;

            fn read_value(state Map<Name, int> entries, key: Name) -> int {
                return entries.ensure(key, 0);
            }

            fn main() -> int {
                Foo[name("alice")] = 1;
                return read_value(Foo, name("alice"));
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 1);
}

#[test]
fn durable_name_map_if_branch_roundtrip_through_name_parameter() {
    let src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;

            fn read_value(key: Name) -> int {
                let value = 0;
                if (Foo.contains(key)) {
                    value = Foo[key];
                }
                return value;
            }

            fn main() -> int {
                Foo[name("alice")] = 1;
                return read_value(name("alice"));
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
            state Foo: Map<Name, int>;
            fn main() {
                Foo[name("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;
            fn main() -> int {
                return Foo[name("alice")];
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
            state Foo: Map<Name, int>;
            fn main() {
                Foo[name("alice")] = 1;
            }
        }
    "#;
    let read_src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;
            fn main() -> int {
                let value = 0;
                if (Foo.contains(name("alice"))) {
                    value = Foo[name("alice")];
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
fn durable_name_map_key_survives_function_call() {
    let src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;

            fn touch() -> int {
                return 7;
            }

            fn main() -> int {
                let key = name("alice");
                Foo[key] = 1;
                let ignored = touch();
                return Foo[key];
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
            state Counter: Map<int, int>;
            state Foo: Map<Name, int>;

            fn main() -> int {
                let key = name("alice");
                Foo[key] = 1;

                let value = 0;
                if (Foo.contains(key)) {
                    value = Foo[key];
                }

                Counter[1] = value + 1;
                return Counter[1];
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
            state Foo: Map<Name, int>;

            fn main() -> int {
                let key = name("alice");
                Foo[key] = 1;

                let value = 0;
                if (Foo.contains(key)) {
                    value = Foo[key];
                }

                return value + 1;
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 2);
}

#[test]
fn durable_name_map_branch_value_survives_path_map_key_work() {
    let src = r#"
        seiyaku C {
            state Foo: Map<Name, int>;
            state EntryByKey: Map<Name, int>;

            fn main() -> int {
                let key = name("alice");
                Foo[key] = 1;

                let value = 0;
                if (Foo.contains(key)) {
                    value = Foo[key];
                }

                EntryByKey[key.path(value)] = 7;
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
            state Counter: Map<int, int>;
            state CountByKey: Map<Name, int>;
            state IndexById: Map<Name, int>;
            state EntryByKey: Map<Name, int>;

            fn next_index() -> int {
                let value = Counter.ensure(1, 0);
                Counter[1] = value + 1;
                return value;
            }

            fn main() -> int {
                let tranche_id = name("t2");
                let beneficiary_lookup_key = name("alice");
                CountByKey[beneficiary_lookup_key] = 1;

                let index = next_index();
                let beneficiary_lookup_position = 0;
                if (CountByKey.contains(beneficiary_lookup_key)) {
                    beneficiary_lookup_position = CountByKey[beneficiary_lookup_key];
                }

                IndexById[tranche_id] = index;
                CountByKey[beneficiary_lookup_key] = beneficiary_lookup_position + 1;
                EntryByKey[beneficiary_lookup_key.path(beneficiary_lookup_position)] = index;
                return CountByKey[beneficiary_lookup_key];
            }
        }
    "#;

    let vm = run_program(src);
    assert_eq!(vm.register(10), 2);
}
