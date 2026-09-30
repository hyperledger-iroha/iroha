//! Durable mutable-call custody, substitution, and ambiguous-dispatch regressions.
use super::*;
use std::cell::Cell;

/// One in-place substitution applied to an otherwise valid prepared call.
type PlanMutation = Box<dyn Fn(&mut PreparedContractCall)>;

fn fixture() -> Result<(Config, PreparedContractCall, TransactionRecord)> {
    let (config, _) = crate::service_tests::fixture()?;
    let artifact = kotodama_lang::compiler::Compiler::new()
        .compile_source(
            "seiyaku Example { kotoage fn run() authorize(\"CanInvokeContractEntrypoint\") {} }",
        )
        .map_err(|error| eyre!(error))?;
    let verified = ivm_artifact_admission::verify_contract_artifact(&artifact)?;
    let address = ContractAddress::derive(
        &config.network_id,
        &config.account,
        7,
        DataSpaceId::UNIVERSAL,
    )?;
    let intent = ContractCallDraftIntent {
        invocation: iroha::data_model::transaction::executable::ContractInvocation {
            contract_address: address,
            expected_code_hash: verified.code_hash,
            entrypoint: "run".to_owned(),
            arguments: None,
        },
        metadata: Metadata::default(),
    };
    let fee = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1_500_000));
    let mut plan = CallPlan {
        version: 1,
        network_id: config.network_id,
        chain_id: config.chain.to_string(),
        authority: config.account.clone(),
        chain_discriminant: config.account_chain_discriminant,
        created_at_ns: 1,
        artifact_hex: hex::encode(artifact),
        alias: "example::universal".parse()?,
        payload: None,
        intent,
        requested_fee: fee,
        grant: None,
    };
    bind_operation_metadata(&mut plan)?;
    let signature = Signature::try_new(config.key_pair.private_key(), &plan_signing_bytes(&plan)?)?;
    let prepared = PreparedContractCall {
        plan,
        signature_hex: hex::encode(signature.payload()),
    };
    let signed = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        prepared.plan.requested_fee.clone(),
    )
    .with_metadata(prepared.plan.intent.metadata.clone())
    .with_executable(Executable::ContractCall(
        prepared.plan.intent.invocation.clone(),
    ))
    .try_sign(config.key_pair.private_key())?;
    Ok((
        config,
        prepared,
        transaction_record("contract-call", &signed),
    ))
}
struct Transport {
    submits: Cell<usize>,
    ambiguous: Cell<bool>,
}
impl DeploymentTransport for Transport {
    fn submit(&self, _: &SignedTransaction) -> Result<()> {
        self.submits.set(self.submits.get() + 1);
        if self.ambiguous.get() {
            Err(eyre!("connection lost after write"))
        } else {
            Ok(())
        }
    }
    fn wait(&self, hash: HashOf<SignedTransaction>) -> Result<AppliedEvidence> {
        if self.ambiguous.get() {
            return Err(eyre!("transaction outcome unavailable"));
        }
        Ok(AppliedEvidence {
            hash: hash.to_string(),
            terminal_kind: "Applied".into(),
            block_height: 42,
            scope: "global".into(),
            resolved_from: "state".into(),
        })
    }
}
#[test]
fn call_resume_never_resubmits_after_ambiguous_dispatch() -> Result<()> {
    let (config, prepared, step) = fixture()?;
    validate_plan(&prepared, &config)?;
    validate_call_transaction(&prepared.plan, &step)?;
    let temporary = tempfile::tempdir()?;
    let journal = Journal::open(&temporary.path().join("call"), true)?;
    journal.put_exact("plan.json", &prepared)?;
    journal.put_exact("call.json", &step)?;
    let transport = Transport {
        submits: Cell::new(0),
        ambiguous: Cell::new(true),
    };
    let error = execute_step(&journal, &step, 1, &transport)
        .expect_err("ambiguous submit must retain pending ownership");
    assert!(error.downcast_ref::<ContractCallPending>().is_some());
    assert_eq!(transport.submits.get(), 1);
    validate_attempt(&journal, &step, 1)?;
    transport.ambiguous.set(false);
    let applied = execute_step(&journal, &step, 1, &transport)?;
    assert_eq!(applied.hash, step.hash);
    assert_eq!(transport.submits.get(), 1);
    assert_eq!(execute_step(&journal, &step, 1, &transport)?, applied);
    assert_eq!(transport.submits.get(), 1);
    Ok(())
}
#[test]
fn call_signed_plan_and_transaction_reject_substitution() -> Result<()> {
    let (config, prepared, step) = fixture()?;
    validate_plan(&prepared, &config)?;
    let mutations: Vec<PlanMutation> = vec![
        Box::new(|p| p.plan.intent.invocation.entrypoint = "other".into()),
        Box::new(|p| p.plan.alias = "other::universal".parse().unwrap()),
        Box::new(|p| p.plan.created_at_ns += 1),
        Box::new(|p| p.plan.chain_id = "other".into()),
        Box::new(|p| p.plan.payload = Some(norito::json!({"amount": "1000"}))),
        Box::new(|p| p.plan.artifact_hex.push_str("00")),
        Box::new(|p| {
            let mut bytes = hex::decode(&p.signature_hex).unwrap();
            bytes[0] ^= 1;
            p.signature_hex = hex::encode(bytes);
        }),
    ];
    for mutate in mutations {
        let mut changed = prepared.clone();
        mutate(&mut changed);
        assert!(validate_plan(&changed, &config).is_err());
    }
    let mut later = prepared.clone();
    later.plan.created_at_ns += 1;
    bind_operation_metadata(&mut later.plan)?;
    later.signature_hex = hex::encode(
        Signature::try_new(
            config.key_pair.private_key(),
            &plan_signing_bytes(&later.plan)?,
        )?
        .payload(),
    );
    validate_plan(&later, &config)?;
    assert_ne!(later.operation_id()?, prepared.operation_id()?);
    assert!(
        validate_call_transaction(&later.plan, &step).is_err(),
        "an earlier identical call must not satisfy a new operation"
    );
    later.plan.intent.metadata.remove(&operation_metadata_key());
    later.signature_hex = hex::encode(
        Signature::try_new(
            config.key_pair.private_key(),
            &plan_signing_bytes(&later.plan)?,
        )?
        .payload(),
    );
    assert!(
        validate_plan(&later, &config).is_err(),
        "even a signed plan must retain its operation tag"
    );
    // Re-signing the exact executable without the plan-bound operation metadata is a substitution.
    let original = decode_transaction(&step)?;
    let changed = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        prepared.plan.requested_fee.clone(),
    )
    .with_executable(original.instructions().clone())
    .try_sign(config.key_pair.private_key())?;
    assert!(
        validate_call_transaction(
            &prepared.plan,
            &transaction_record("contract-call", &changed)
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn call_rejects_missing_payload_and_conflicting_retained_evidence() -> Result<()> {
    let (_, prepared, step) = fixture()?;
    let temporary = tempfile::tempdir()?;
    let journal = Journal::open(&temporary.path().join("call"), true)?;
    journal.put_exact(
        "attempt-0001.json",
        &TransactionAttempt {
            name: step.name.clone(),
            hash: step.hash.clone(),
        },
    )?;
    assert!(
        validate_stage_layout(&journal, &prepared.plan)
            .unwrap_err()
            .to_string()
            .contains("must never be prepared again")
    );
    journal.put_exact("call.json", &step)?;
    validate_stage_layout(&journal, &prepared.plan)?;
    let transport = Transport {
        submits: Cell::new(0),
        ambiguous: Cell::new(false),
    };
    let mut false_evidence = transport.wait(decode_transaction(&step)?.hash())?;
    false_evidence.block_height += 1;
    journal.put_exact("applied-0001.json", &false_evidence)?;
    assert!(execute_step(&journal, &step, 1, &transport).is_err());
    assert_eq!(transport.submits.get(), 0);
    Ok(())
}
#[test]
fn call_self_grant_is_exact_and_precedes_call_preparation() -> Result<()> {
    let (config, mut prepared, _) = fixture()?;
    let artifact = hex::decode(&prepared.plan.artifact_hex)?;
    let verified = ivm_artifact_admission::verify_contract_artifact(&artifact)?;
    let permission =
        required_permission(&verified, &prepared.plan.intent)?.expect("guarded entrypoint");
    let grant = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        prepared.plan.requested_fee.clone(),
    )
    .with_instructions([Grant::account_permission(
        permission,
        config.account.clone(),
    )])
    .try_sign(config.key_pair.private_key())?;
    prepared.plan.grant = Some(transaction_record("entrypoint-grant", &grant));
    bind_operation_metadata(&mut prepared.plan)?;
    prepared.signature_hex = hex::encode(
        Signature::try_new(
            config.key_pair.private_key(),
            &plan_signing_bytes(&prepared.plan)?,
        )?
        .payload(),
    );
    validate_plan(&prepared, &config)?;
    let signed_call = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        prepared.plan.requested_fee.clone(),
    )
    .with_metadata(prepared.plan.intent.metadata.clone())
    .with_executable(Executable::ContractCall(
        prepared.plan.intent.invocation.clone(),
    ))
    .try_sign(config.key_pair.private_key())?;
    let step = transaction_record("contract-call", &signed_call);
    let temporary = tempfile::tempdir()?;
    let journal = Journal::open(&temporary.path().join("call"), true)?;
    journal.put_exact("call.json", &step)?;
    assert!(validate_stage_layout(&journal, &prepared.plan).is_err());
    let transport = Transport {
        submits: Cell::new(0),
        ambiguous: Cell::new(false),
    };
    execute_step(
        &journal,
        prepared.plan.grant.as_ref().unwrap(),
        0,
        &transport,
    )?;
    validate_stage_layout(&journal, &prepared.plan)?;
    execute_step(&journal, &step, 1, &transport)?;
    assert_eq!(transport.submits.get(), 2);
    let other = KeyPair::random();
    let wrong_grant = TransactionBuilder::new(
        config.network_id,
        config.account.clone(),
        prepared.plan.requested_fee.clone(),
    )
    .with_instructions([Grant::account_permission(
        CanInvokeContractEntrypoint {
            contract: prepared.plan.intent.invocation.contract_address.clone(),
            entrypoint: "run".into(),
        },
        AccountId::new(other.public_key().clone()),
    )])
    .try_sign(config.key_pair.private_key())?;
    prepared.plan.grant = Some(transaction_record("entrypoint-grant", &wrong_grant));
    bind_operation_metadata(&mut prepared.plan)?;
    prepared.signature_hex = hex::encode(
        Signature::try_new(
            config.key_pair.private_key(),
            &plan_signing_bytes(&prepared.plan)?,
        )?
        .payload(),
    );
    assert!(validate_plan(&prepared, &config).is_err());
    Ok(())
}
#[test]
fn call_cancellation_rejects_any_attempt_or_foreign_operation() -> Result<()> {
    let (config, prepared, step) = fixture()?;
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("call");
    let service = ContractCallService::new(config)?;
    service.persist(&prepared, &path)?;
    assert_eq!(service.cancel(&path)?, prepared.operation_id()?);
    assert!(matches!(
        service.inspect(&path)?,
        ContractCallDisposition::Cancelled
    ));
    assert!(
        service
            .resume(&path)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    let journal = Journal::open(&path, false)?;
    journal.put_exact(
        "attempt-0001.json",
        &TransactionAttempt {
            name: step.name.clone(),
            hash: step.hash.clone(),
        },
    )?;
    assert!(is_cancelled(&journal, &prepared).is_err());
    let foreign = Journal::open(&temporary.path().join("foreign"), true)?;
    foreign.put_exact("cancelled.json", &"wrong-operation")?;
    assert!(is_cancelled(&foreign, &prepared).is_err());
    Ok(())
}

#[test]
fn call_failure_cannot_overwrite_applied_or_change_the_exact_hash() -> Result<()> {
    struct FailedTransport {
        different_hash: bool,
    }
    impl DeploymentTransport for FailedTransport {
        fn submit(&self, _: &SignedTransaction) -> Result<()> {
            panic!("retained attempts must never be submitted again");
        }
        fn wait(&self, hash: HashOf<SignedTransaction>) -> Result<AppliedEvidence> {
            let bound_hash = if self.different_hash {
                HashOf::from_untyped_unchecked(Hash::new(b"different transaction"))
            } else {
                hash
            };
            let response = norito::json::from_value(norito::json!({
                "hash": (bound_hash.to_string()), "scope": "global", "resolved_from": "state",
                "status": { "kind": "Rejected" }
            }))?;
            let proof = TransactionFinalityFailure::from_response(bound_hash, response)?
                .expect("canonical rejection fixture");
            Err(eyre::Report::new(proof))
        }
    }
    let (_, _, step) = fixture()?;
    for retained_applied in [false, true] {
        let temporary = tempfile::tempdir()?;
        let journal = Journal::open(&temporary.path().join("call"), true)?;
        journal.put_exact(
            "attempt-0001.json",
            &TransactionAttempt {
                name: step.name.clone(),
                hash: step.hash.clone(),
            },
        )?;
        if retained_applied {
            let transport = Transport {
                submits: Cell::new(0),
                ambiguous: Cell::new(false),
            };
            journal.put_exact(
                "applied-0001.json",
                &transport.wait(decode_transaction(&step)?.hash())?,
            )?;
        }
        assert!(
            execute_step(
                &journal,
                &step,
                1,
                &FailedTransport {
                    different_hash: true
                }
            )
            .is_err()
        );
        assert!(!journal.exists("failed-0001.json")?);
        let error = execute_step(
            &journal,
            &step,
            1,
            &FailedTransport {
                different_hash: false,
            },
        )
        .expect_err("rejection is terminal or contradicts retained Applied evidence");
        assert_eq!(journal.exists("failed-0001.json")?, !retained_applied);
        if retained_applied {
            assert!(error.to_string().contains("conflicts"));
        } else {
            assert!(error.downcast_ref::<TransactionFinalityFailure>().is_some());
        }
    }
    let hash = decode_transaction(&step)?.hash();
    let make_proof = |hash: HashOf<SignedTransaction>| -> Result<TransactionFinalityFailure> {
        let response = norito::json::from_value(norito::json!({
            "hash": (hash.to_string()), "scope": "global", "resolved_from": "state",
            "status": { "kind": "Rejected" }
        }))?;
        Ok(TransactionFinalityFailure::from_response(hash, response)?.expect("rejection"))
    };
    let proof = make_proof(hash)?;
    for foreign in [false, true] {
        let temporary = tempfile::tempdir()?;
        let journal = Journal::open(&temporary.path().join("call"), true)?;
        if foreign {
            journal.put_exact(
                "failed-0001.json",
                &make_proof(HashOf::from_untyped_unchecked(Hash::new(
                    b"another operation",
                )))?,
            )?;
        } else {
            journal.put_exact("failed-0001.json", &"malformed retained failure")?;
        }
        assert!(
            validate_retained_failure(&journal, 1, hash, &proof).is_err(),
            "bad retained evidence must not release the operation gate"
        );
    }
    Ok(())
}

/// Read one bounded HTTP/1.1 request head and its `Content-Length` body from a fixture socket.
fn read_fixture_request(stream: &mut std::net::TcpStream) -> Result<(String, Vec<u8>)> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        bytes.push(byte[0]);
        if bytes.len() > 64 * 1024 {
            return Err(eyre!("resolution fixture headers exceed bound"));
        }
    }
    let headers = String::from_utf8(bytes)?;
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>())
        })
        .transpose()?
        .unwrap_or(0);
    if length > 64 * 1024 {
        return Err(eyre!("resolution fixture body exceeds bound"));
    }
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    Ok((headers, body))
}

#[test]
fn call_standalone_resolution_uses_configured_discriminant_and_restores_caller() -> Result<()> {
    use std::{io::Write as _, net::TcpListener, thread, time::Instant};
    let (mut config, prepared, _) = fixture()?;
    config.account_chain_discriminant = 42;
    let expected_authority = config.account.to_i105_for_discriminant(42)?;
    let alias = prepared.plan.alias.clone();
    let expected_address = prepared.plan.intent.invocation.contract_address.clone();
    let capabilities = norito::json::to_vec(&norito::json!({
        "data_model_version": (iroha::data_model::DATA_MODEL_VERSION),
        "signed_transaction_schema_hash_hex": (hex::encode(norito::schema::identity::frame_hash::<SignedTransaction>()))
    }))?;
    let state = norito::json::to_vec(&norito::json!({
        "authority": (expected_authority.clone()), "contract_alias": (alias.to_string()),
        "deploy_nonce": "8", "dataspace_alias": "universal", "dataspace_id": "0",
        "previous_contract_address": (expected_address.to_string()),
        "observed_block_height": "9",
        "observed_block_hash": (HashOf::<iroha::data_model::block::BlockHeader>::from_untyped_unchecked(Hash::new(b"deployment state response fixture")).to_string()),
        "ledger_time_ms": "1000", "chain_discriminant": "42"
    }))?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    config.torii_api_url = format!("http://{}/", listener.local_addr()?).parse()?;
    let server = thread::spawn(move || -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        for (index, response) in [capabilities, state].into_iter().enumerate() {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(accepted) => break accepted,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Err(eyre!("resolution fixture timed out"));
                        }
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => return Err(error.into()),
                }
            };
            stream.set_nonblocking(false)?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            let (headers, body) = read_fixture_request(&mut stream)?;
            if index == 0 {
                assert!(headers.starts_with("GET /v1/node/capabilities "));
            } else {
                assert!(headers.starts_with("POST /v1/contracts/deployment-state "));
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("\r\nx-iroha-signature:")
                );
                let request: Value = norito::json::from_slice(&body)?;
                assert_eq!(
                    request.get("authority").and_then(Value::as_str),
                    Some(expected_authority.as_str())
                );
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            )?;
            stream.write_all(&response)?;
        }
        Ok(())
    });
    let service = ContractCallService::new(config)?;
    let _caller_profile = ChainDiscriminantGuard::enter(73);
    let resolved = service.resolve_address(&alias);
    assert_eq!(
        iroha::data_model::account::address::chain_discriminant(),
        73
    );
    server
        .join()
        .map_err(|_| eyre!("resolution fixture panicked"))??;
    assert_eq!(resolved?, expected_address);
    Ok(())
}
