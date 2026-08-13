// Adversarial response-binding coverage shared by the in-module CLI tests.

fn exact_checkpoint_page_response(
    checkpoint: Option<&str>,
    content_type: &str,
) -> Response<Vec<u8>> {
    let mut page = Map::new();
    if let Some(checkpoint) = checkpoint {
        page.insert(
            "anchor".into(),
            norito::json!({"checkpoint_fingerprint": checkpoint}),
        );
    }
    page.insert("items".into(), Value::Array(Vec::new()));
    page.insert("payload_marker".into(), Value::from("DO_NOT_ECHO"));
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .body(norito::json::to_vec(&Value::Object(page)).expect("encode exact-checkpoint page"))
        .expect("exact-checkpoint response")
}

fn assert_payload_free_binding_error(error: eyre::Report, expected: &str) {
    let message = error.to_string();
    assert_eq!(message, expected);
    assert!(!message.contains("DO_NOT_ECHO"));
}

#[test]
fn billing_statements_cli_rejects_missing_response_anchor_without_output() {
    let checkpoint = "11".repeat(32);
    let args = BillingStatementsArgs {
        expected_checkpoint_fingerprint: checkpoint,
        after_statement_id: None,
        limit: 1,
    };
    let mut context = TestContext::new();

    let error = args
        .run_with(&mut context, |_client, _filter| {
            Ok(exact_checkpoint_page_response(None, "application/json"))
        })
        .expect_err("missing response anchor must fail closed");

    assert_payload_free_binding_error(
        error,
        "SoraFS hedging/billing exact-checkpoint response is missing anchor.checkpoint_fingerprint",
    );
    assert!(context.printed.is_empty());
}

#[test]
fn hedging_projection_cli_rejects_mismatched_and_wrong_case_anchors_without_output() {
    let checkpoint = "33".repeat(32);
    let args = HedgingProjectionArgs {
        expected_checkpoint_fingerprint: checkpoint.clone(),
        after: None,
        limit: 1,
    };
    for returned in ["44".repeat(32).to_ascii_uppercase(), checkpoint.clone()] {
        let mut context = TestContext::new();
        let error = args
            .run_with(&mut context, |_client, _filter| {
                Ok(exact_checkpoint_page_response(
                    Some(&returned),
                    "application/json",
                ))
            })
            .expect_err("mismatched or lowercase response anchor must fail closed");

        assert_payload_free_binding_error(
            error,
            "SoraFS hedging/billing exact-checkpoint response anchor does not match the request",
        );
        assert!(context.printed.is_empty());
    }
}

#[test]
fn exact_checkpoint_cli_rejects_ambiguous_json_media_type_without_output() {
    let checkpoint = "55".repeat(32);
    let args = BillingStatementsArgs {
        expected_checkpoint_fingerprint: checkpoint.clone(),
        after_statement_id: None,
        limit: 1,
    };
    let returned = checkpoint.to_ascii_uppercase();
    let mut context = TestContext::new();

    let error = args
        .run_with(&mut context, |_client, _filter| {
            Ok(exact_checkpoint_page_response(
                Some(&returned),
                "application/json; charset=utf-8",
            ))
        })
        .expect_err("ambiguous JSON media type must fail closed");

    assert_payload_free_binding_error(
        error,
        "SoraFS hedging/billing exact-checkpoint response must use application/json",
    );
    assert!(context.printed.is_empty());
}

#[test]
fn exact_checkpoint_cli_rejects_non_ok_without_echoing_response() {
    let checkpoint = "66".repeat(32);
    let args = HedgingProjectionArgs {
        expected_checkpoint_fingerprint: checkpoint,
        after: None,
        limit: 1,
    };
    let mut context = TestContext::new();

    let error = args
        .run_with(&mut context, |_client, _filter| {
            Ok(Response::builder()
                .status(StatusCode::CONFLICT)
                .header("Content-Type", "application/json")
                .body(br#"{"payload_marker":"DO_NOT_ECHO"}"#.to_vec())
                .expect("projection conflict response"))
        })
        .expect_err("non-OK response must fail closed");

    assert_payload_free_binding_error(
        error,
        "SoraFS hedging/billing exact-checkpoint response returned status 409 Conflict",
    );
    assert!(context.printed.is_empty());
}
