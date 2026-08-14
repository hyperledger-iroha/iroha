//! Runtime regressions for pointer-backed literals projected from aggregate values.
use ivm::{CoreHost, IVM, kotodama::compiler::Compiler as KotodamaCompiler};
mod common;
fn run_program(source: &str) -> IVM {
    let code = KotodamaCompiler::new()
        .compile_source(source)
        .expect("compile Kotodama aggregate-state fixture");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(CoreHost::new());
    vm.load_program(&code)
        .expect("load aggregate-state fixture");
    common::select_kotodama_entrypoint(&mut vm, &code, "main");
    vm.run().expect("run aggregate-state fixture");
    vm
}
#[test]
fn mint_request_shape_roundtrips_all_eleven_fields() {
    let source = r#"
        seiyaku MintRequestShape {
            const int STATUS_PENDING = 1;
            const int REQUEST_TTL_MS = 86400000;

            struct Request {
                Name proposal_id,
                AccountId requester,
                Name requesting_fi_scope,
                AccountId destination,
                AssetDefinitionId asset_definition,
                quantity amount,
                int status,
                int created_at_ms,
                int expires_at_ms,
                int finalized_at_ms,
                int canceled_at_ms,
            }

            state StateMap<Name, Request> Requests;

            kotoage fn main() -> bool authorize("WriteState") {
                let Name proposal_id = Name::parse("request1");
                let AccountId requester = context::authority();
                let int created_at_ms = context::current_time_ms();
                Requests[proposal_id] = Request {
                    proposal_id,
                    requester,
                    requesting_fi_scope: Name::parse("hbl.sbp"),
                    destination: requester,
                    asset_definition: AssetDefinitionId::parse(
                        "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                    ),
                    amount: 100,
                    status: STATUS_PENDING,
                    created_at_ms,
                    expires_at_ms: created_at_ms + REQUEST_TTL_MS,
                    finalized_at_ms: 0,
                    canceled_at_ms: 0,
                };

                let Request request = Requests.get(proposal_id).unwrap_or(Request {
                    proposal_id: Name::parse("fallback"),
                    requester,
                    requesting_fi_scope: Name::parse("fallback.sbp"),
                    destination: requester,
                    asset_definition: AssetDefinitionId::parse(
                        "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                    ),
                    amount: 0,
                    status: 0,
                    created_at_ms: 0,
                    expires_at_ms: 0,
                    finalized_at_ms: 0,
                    canceled_at_ms: 0,
                });
                return request.proposal_id == proposal_id
                    && request.requester == requester
                    && request.requesting_fi_scope == Name::parse("hbl.sbp")
                    && request.destination == requester
                    && request.asset_definition == AssetDefinitionId::parse(
                        "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                    )
                    && request.amount == 100
                    && request.status == STATUS_PENDING
                    && request.created_at_ms == created_at_ms
                    && request.expires_at_ms == created_at_ms + REQUEST_TTL_MS
                    && request.finalized_at_ms == 0
                    && request.canceled_at_ms == 0;
            }
        }
    "#;
    let vm = run_program(source);
    assert_eq!(vm.register(10), 1);
}
#[test]
fn mixed_pointer_and_scalar_literal_fields_keep_their_exact_types() {
    let source = r#"
        seiyaku MixedAggregate {
            struct Value {
                Name label,
                bool enabled,
                int code,
                bytes payload,
                decimal ratio,
                quantity amount,
                bool terminal,
            }

            state StateMap<Name, Value> Values;

            kotoage fn main() -> bool authorize("WriteState") {
                let Name key = Name::parse("mixed");
                Values[key] = Value {
                    label: Name::parse("literal-label"),
                    enabled: true,
                    code: 7,
                    payload: b"payload",
                    ratio: 1.25,
                    amount: 9,
                    terminal: false,
                };
                let Value value = Values.get(key).unwrap_or(Value {
                    label: Name::parse("fallback"),
                    enabled: false,
                    code: 0,
                    payload: b"",
                    ratio: 0,
                    amount: 0,
                    terminal: true,
                });
                return value.label == Name::parse("literal-label")
                    && value.enabled
                    && value.code == 7
                    && value.payload == b"payload"
                    && value.ratio == 1.25
                    && value.amount == 9
                    && !value.terminal;
            }
        }
    "#;
    let vm = run_program(source);
    assert_eq!(vm.register(10), 1);
}
