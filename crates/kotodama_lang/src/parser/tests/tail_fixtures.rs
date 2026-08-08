#[test]
fn fixture_actions_accept_formatter_trailing_commas() {
    let src = r#"
            module FixtureTrailingComma {
                koto_test { target: "target.ko" }
                fixture actors {
                    actor(
                        "issuer",
                        AccountId::parse("issuer"),
                        "0x00",
                    );
                }
            }
        "#;
    let program = parse(src).expect("fixture action with a trailing comma must parse");
    assert_eq!(program.fixtures.len(), 1);
    assert_eq!(program.fixtures[0].actions.len(), 1);
    assert_eq!(program.fixtures[0].actions[0].args.len(), 3);
}

#[test]
fn rejects_unregistered_unicode_attributes() {
    let src = r#"
        module ContractTests {
            #[テスト]
            fn smoke() {}
        }
        "#;
    let error = parse(src).expect_err("unregistered Unicode attributes are invalid");
    assert!(error.contains("non-ASCII"), "{error}");
}

#[test]
fn parse_koto_test_target_fixture_and_test_binding() {
    let src = r#"
        module ContractTests {
            koto_test { target: "contracts/demo.ko" }

            fixture seeded {
                caller(AccountId::parse("alice@wonderland"));
                grant_permission("register_domain");
            }

            #[test(fixture="seeded")]
            fn smoke() {}
        }
        "#;
    let prog = parse(src).expect("parse koto_test program");
    assert_eq!(
        prog.test_target
            .as_ref()
            .map(|target| target.target.as_str()),
        Some("contracts/demo.ko")
    );
    assert_eq!(prog.fixtures.len(), 1);
    assert_eq!(prog.fixtures[0].name, "seeded");
    assert_eq!(prog.fixtures[0].actions.len(), 2);

    let func = prog
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(f) => Some(f),
            _ => None,
        })
        .expect("function present");
    assert!(func.modifiers.is_test);
    assert_eq!(func.modifiers.test_fixture.as_deref(), Some("seeded"));
}
