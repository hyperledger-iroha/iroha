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
