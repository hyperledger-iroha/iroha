#[test]
fn fee_quote_decision_schema_is_an_exact_closed_payer_union() {
    let expected = norito::json!({
        "oneOf": [
            {
                "additionalProperties": false,
                "properties": {
                    "debit_source": {
                        "additionalProperties": false,
                        "properties": {
                            "kind": {
                                "const": "account",
                                "type": "string"
                            },
                            "value": {
                                "$ref": "#/components/schemas/CanonicalAccountId"
                            }
                        },
                        "required": ["kind", "value"],
                        "type": "object"
                    },
                    "program_revision": {
                        "type": "null"
                    }
                },
                "required": ["debit_source", "program_revision"],
                "type": "object"
            },
            {
                "additionalProperties": false,
                "properties": {
                    "debit_source": {
                        "additionalProperties": false,
                        "properties": {
                            "kind": {
                                "const": "sponsor_program",
                                "type": "string"
                            },
                            "value": {
                                "$ref": "#/components/schemas/FeeSponsorProgramId"
                            }
                        },
                        "required": ["kind", "value"],
                        "type": "object"
                    },
                    "program_revision": {
                        "format": "uint64",
                        "minimum": 1,
                        "type": "integer"
                    }
                },
                "required": ["debit_source", "program_revision"],
                "type": "object"
            }
        ]
    });

    for (label, document) in [
        ("package authority", canonical_document()),
        ("generated spec", generate_spec()),
    ] {
        assert_eq!(
            component_schemas(&document)["FeeQuoteResponse"]["properties"]["decision"]
                ["properties"]["value"],
            expected,
            "{label} FeeQuoteResponse decision value schema"
        );
    }
}
