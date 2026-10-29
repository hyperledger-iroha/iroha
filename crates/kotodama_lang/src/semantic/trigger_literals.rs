//! Shared trigger-id decoding for literal registration access metadata.
//!
//! Production semantic analysis rejects an undecodable direct `Json::parse` argument.
//! Code generation uses the same decoder after local/private-call specialization; dynamic
//! inputs retain conservative access metadata, and test mode retains diagnostic fallbacks.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::trigger::{Trigger, TriggerId};
use kotodama_surface::builtins::{Builtin, PointerConstructor};
use norito::json;

use super::{CallTarget, ExprKind, SemanticError, TypedExpr, sem_err};

/// Access-hint reason used by both frontend validation and code generation.
pub(crate) const DECODE_FAILURE_REASON: &str =
    "literal create_trigger spec could not be decoded for access metadata";

/// Read the trigger identifier from the JSON forms accepted by access analysis.
pub(crate) fn id_from_json(raw: &str) -> Option<TriggerId> {
    let value: json::Value = json::from_slice(raw.as_bytes()).ok()?;
    match value {
        json::Value::String(encoded) => {
            let bytes = STANDARD.decode(encoded.as_bytes()).ok()?;
            let trigger: Trigger = ivm_abi::codec::decode_canonical_norito(&bytes).ok()?;
            Some(trigger.id().clone())
        }
        json::Value::Object(map) => {
            let id = map.get("id")?.as_str()?;
            id.parse().ok()
        }
        _ => None,
    }
}

/// Validate direct JSON constructor input without guessing local or private-call values.
pub(crate) fn validate_registration_literal(argument: &TypedExpr) -> Result<(), SemanticError> {
    let (ExprKind::Call {
        target: CallTarget::Builtin(Builtin::PointerConstructor(PointerConstructor::Json)),
        args,
    }
    | ExprKind::NamedCall {
        target: CallTarget::Builtin(Builtin::PointerConstructor(PointerConstructor::Json)),
        args,
        ..
    }) = argument.kind()
    else {
        return Ok(());
    };
    if let Some(raw) = args.first().and_then(|argument| match argument.kind() {
        ExprKind::String(raw) => Some(raw),
        _ => None,
    }) && id_from_json(raw).is_none()
    {
        return Err(sem_err("E_ACCESS_INCOMPLETE", DECODE_FAILURE_REASON.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compiler::{CompilerMode, CompilerOptions},
        diagnostic::DiagnosticPhase,
        session::{CompileRequest, CompilerSession},
    };

    fn source(argument: &str) -> String {
        format!(
            "seiyaku Triggers {{ permission Admin; \
             kotoage fn register() authorize(Admin) {{ \
                 ledger::trigger::register(trigger_spec: {argument}); \
             }} }}"
        )
    }

    #[test]
    fn invalid_trigger_literal_shapes_fail_check_and_build_at_the_argument() {
        let session = CompilerSession::default();
        for (raw, label) in [
            "{}",
            r#"{"id":null}"#,
            r#"{"id":1}"#,
            r#"{"id":"bad name"}"#,
            "[]",
            "null",
            r#""not base64""#,
            r#""AA==""#,
        ]
        .into_iter()
        .flat_map(|raw| ["", "value: "].map(|label| (raw, label)))
        {
            let literal = json::to_string(raw).expect("quote JSON for source");
            let argument = format!("Json::parse({label}{literal})");
            let source = source(&argument);
            let request = CompileRequest {
                source: &source,
                source_name: Some("trigger.ko"),
            };
            let checked = session.check(request).expect_err("invalid trigger literal");
            let built = session.build(request).expect_err("invalid trigger literal");
            assert_eq!(checked, built, "{raw}");
            assert_eq!(checked.diagnostics.len(), 1);
            let diagnostic = &checked.diagnostics[0];
            assert_eq!(diagnostic.code, "E_ACCESS_INCOMPLETE");
            assert_eq!(diagnostic.phase, DiagnosticPhase::Semantic);
            let span = diagnostic.primary_span.as_ref().expect("argument span");
            assert_eq!(span.source.as_deref(), Some("trigger.ko"));
            let range = span.byte_range.expect("argument byte range");
            assert_eq!(&source[range.start as usize..range.end as usize], argument);
        }
    }

    #[test]
    fn trigger_literal_checks_preserve_type_and_json_diagnostics() {
        let session = CompilerSession::default();
        for (argument, code) in [
            (r#"Name::parse("wake")"#, "K2003"),
            (r#"Json::parse("{")"#, "E_JSON_LITERAL_INVALID"),
        ] {
            let source = source(argument);
            let request = CompileRequest {
                source: &source,
                source_name: Some("trigger.ko"),
            };
            let checked = session.check(request).expect_err("invalid argument");
            assert_eq!(checked, session.build(request).unwrap_err());
            assert_eq!(checked.diagnostics.len(), 1);
            assert_eq!(checked.diagnostics[0].code, code);
        }
    }

    #[test]
    fn trigger_literal_checks_preserve_dynamic_and_specialized_inputs() {
        let session = CompilerSession::default();
        let literal = r#"Json::parse("{\"id\":\"wake\"}")"#;
        let direct = source(literal);
        let local = source("spec").replace(
            "ledger::trigger::register",
            &format!("let spec = {literal}; ledger::trigger::register"),
        );
        let private = format!(
            "seiyaku Triggers {{ permission Admin; \
             fn stage(Json spec) {{ ledger::trigger::register(trigger_spec: spec); }} \
             kotoage fn register() authorize(Admin) {{ stage(spec: {literal}); }} }}"
        );
        for source in [&direct, &local, &private] {
            let request = CompileRequest {
                source,
                source_name: Some("trigger.ko"),
            };
            session.check(request).expect("valid trigger input checks");
            let built = session.build(request).expect("valid trigger input builds");
            let entrypoints = built.manifest.entrypoints.expect("entrypoints");
            let register = entrypoints
                .iter()
                .find(|entry| entry.name == "register")
                .unwrap();
            assert_eq!(register.access_hints_complete, Some(true));
            assert!(register.read_keys.iter().any(|key| key == "trigger:wake"));
        }
        let dynamic = source("spec").replace("fn register()", "fn register(Json spec)");
        let request = CompileRequest {
            source: &dynamic,
            source_name: Some("trigger.ko"),
        };
        session
            .check(request)
            .expect("runtime JSON remains supported");
        session
            .build(request)
            .expect("runtime JSON retains conservative access");
    }

    #[test]
    fn test_mode_preserves_undecodable_trigger_hint_fallback() {
        let session = CompilerSession::new(CompilerOptions {
            mode: CompilerMode::Test,
            ..CompilerOptions::default()
        });
        let source = source(r#"Json::parse("{}")"#);
        let request = CompileRequest {
            source: &source,
            source_name: Some("trigger.ko"),
        };
        session
            .check(request)
            .expect("test mode checks malformed trigger fixtures");
        let built = session
            .build(request)
            .expect("test mode emits diagnostic fallback");
        let entrypoints = built.manifest.entrypoints.expect("entrypoints");
        let register = entrypoints
            .iter()
            .find(|entry| entry.name == "register")
            .unwrap();
        assert_eq!(register.access_hints_complete, Some(false));
        assert_eq!(register.access_hints_skipped, [DECODE_FAILURE_REASON]);
    }

    #[test]
    fn encoded_trigger_ids_require_a_canonical_norito_frame() {
        use iroha_data_model::{
            account::AccountId,
            events::{EventFilterBox, execute_trigger::ExecuteTriggerEventFilter},
            transaction::{Executable, IvmBytecode},
            trigger::action::{Action, Repeats},
        };
        let id: TriggerId = "wake".parse().unwrap();
        let authority = AccountId::new(
            "ed0120A98BAFB0663CE08D75EBD506FEC38A84E576A7C9B0897693ED4B04FD9EF2D18D"
                .parse()
                .unwrap(),
        );
        let action = Action::new(
            Executable::Ivm(IvmBytecode::from_compiled(Vec::new())),
            Repeats::Indefinitely,
            authority,
            EventFilterBox::ExecuteTrigger(ExecuteTriggerEventFilter::new()),
        )
        .unwrap();
        let trigger = Trigger::new(id.clone(), action);
        let mut bytes = ivm_abi::codec::encode_canonical_norito(&trigger).unwrap();
        let encoded = json::to_string(&STANDARD.encode(&bytes)).unwrap();
        assert_eq!(id_from_json(&encoded), Some(id));
        let session = CompilerSession::default();
        let source = source(&format!(
            "Json::parse({})",
            json::to_string(&encoded).unwrap()
        ));
        let request = CompileRequest {
            source: &source,
            source_name: Some("encoded_trigger.ko"),
        };
        session
            .check(request)
            .expect("canonical encoded trigger checks");
        session
            .build(request)
            .expect("canonical encoded trigger builds");
        bytes.push(0);
        let trailing = json::to_string(&STANDARD.encode(bytes)).unwrap();
        assert!(
            id_from_json(&trailing).is_none(),
            "trailing bytes must fail closed"
        );
    }
}
