//! Wire, token and deadline contracts of the shared managed control envelope.

use super::*;

#[test]
fn control_requests_keep_their_exact_closed_wire_form() {
    let token = "c".repeat(64);
    for (action, kind) in [
        (ControlAction::ListenerAdoption, "listener_adoption"),
        (ControlAction::Status, "status"),
        (ControlAction::Down, "down"),
        (ControlAction::StartupExpired, "startup_expired"),
        (ControlAction::AttachmentStart, "attachment_start"),
        (ControlAction::AttachmentStatus, "attachment_status"),
    ] {
        let bytes = encode(&ControlRequest {
            token: token.clone(),
            action,
        })
        .unwrap();
        assert_eq!(
            bytes,
            format!(r#"{{"token":"{token}","action":{{"kind":"{kind}","value":null}}}}"#)
                .into_bytes()
        );
        let decoded: ControlRequest = decode(&bytes).unwrap();
        assert_eq!(decoded.token, token);
        assert_eq!(decoded.action, action);
    }
    // Retired string actions, unknown kinds and smuggled fields never decode.
    for rejected in [
        r#"{"token":"t","action":"listener_adoption"}"#,
        r#"{"token":"t","action":{"kind":"stop_validator"}}"#,
        r#"{"token":"t","action":{"kind":"listener_adoption","value":1}}"#,
        r#"{"token":"t","action":{"kind":"listener_adoption"},"extra":true}"#,
    ] {
        assert!(
            decode::<ControlRequest>(rejected.as_bytes()).is_err(),
            "{rejected}"
        );
    }
}

#[test]
fn control_authentication_rejects_short_and_different_tokens() {
    assert!(same_token(&"a".repeat(64), &"a".repeat(64)));
    assert!(!same_token("", ""));
    assert!(!same_token(&"a".repeat(64), &"b".repeat(64)));
    assert!(!same_token(&"a".repeat(63), &"a".repeat(63)));
}

#[test]
fn session_tokens_are_fresh_exact_lowercase_hexadecimal() {
    let token = random_token();
    assert_eq!(token.len(), 64);
    assert!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert!(same_token(&token, &token));
    assert!(!same_token(&token, &random_token()));
}

#[test]
fn continuous_remainder_closes_at_its_deadline_and_refuses_overflow() {
    assert_eq!(continuous_remaining(0).unwrap(), None);
    let now = iroha_primitives::time::native_continuous_clock_nanos().unwrap();
    assert_eq!(continuous_remaining(now).unwrap(), None);
    let budget = Duration::from_secs(90);
    let remaining = continuous_remaining(now + budget.as_nanos())
        .unwrap()
        .unwrap();
    assert!(remaining <= budget);
    assert!(remaining > Duration::from_secs(30));
    assert!(continuous_remaining(u128::MAX).is_err());
}

#[test]
fn errors_keep_the_messages_the_daemon_reports() {
    assert_eq!(
        Error::NativeDeadline.to_string(),
        "native operation I/O deadline elapsed; retain original journals"
    );
    assert_eq!(Error::Invalid("refused".into()).to_string(), "refused");
    let native = std::io::Error::new(std::io::ErrorKind::TimedOut, "native detail");
    assert_eq!(Error::from(native).to_string(), "native detail");
}
