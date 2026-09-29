//! Four-validator admission of canonical replay-binding proofs and mandatory IVM replay.
//!
//! A real proved call increments contract state. Cryptographically valid binding
//! of forged claims does not authorize that state change. This opt-in gate keeps
//! the production verifier caps, deadline, gas policy and mandatory DA/RBC.
use super::{
    contract_probe_call_intent, contract_state_json_value, deploy_contract_artifact,
    wait_for_tx_applied,
};
use eyre::{Report, Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    blocking::Client,
    client::{AccountTransactionDraft, FeeQuoteRequest, TransactionFinalityFailure},
    query::QueryError,
};
use iroha_core::zk::{self, confidential_v2};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    Level, ValidationFail,
    isi::{Grant, InstructionBox, Log, verifying_keys},
    permission::Permission,
    proof::{ProofAttachment, ProofAttachmentList, ProofBox, VerifyingKeyBox, VerifyingKeyId},
    query::error::QueryExecutionFail,
    smart_contract::{ContractAddress, ContractAlias},
    transaction::{
        Executable, FeePaymentIntent, IvmBytecode, IvmProved, SignedTransaction,
        TransactionEntrypoint, error::TransactionRejectionReason,
    },
    zk::OpenVerifyEnvelope,
};
use iroha_executor_data_model::permission::{
    account::{AccountAliasPermissionScope, CanManageAccountAlias},
    governance::{CanEnactGovernance, CanManageVerifyingKeys},
    smart_contract::CanManageSmartContractCode,
};
use iroha_model_base::{metadata::Metadata, topology::DataSpaceId};
use iroha_test_network::{Network, NetworkBuilder, read_on_dedicated_thread};
use iroha_test_samples::{ALICE_ID, SAMPLE_GENESIS_ACCOUNT_ID};
use iroha_torii::{ZkIvmDeriveRequestDto, ZkIvmDeriveResponseDto};
use std::{num::NonZeroU32, time::Duration};

#[path = "../proof_fixtures.rs"]
mod proof_fixtures;

const TEST_NAME: &str =
    "four_validator_ivm_proved_rejects_alias_role_corruption_and_forged_binding";
const OBSERVATION_TIMEOUT: Duration = Duration::from_secs(600);
const EXACT_CIRCUIT_REJECTION: &str = "zk_proof: verifying key and proof must use the exact canonical ivm-replay-binding-v1 circuit id";
const REPLAY_REJECTION: &str = "zk_proof: events commitment mismatch";

fn counter_artifact() -> Vec<u8> {
    ivm::KotodamaCompiler::new_with_options(ivm::kotodama::compiler::CompilerOptions {
        force_zk: true,
        max_cycles: 4_096,
        ..Default::default()
    })
    .compile_source(
        r#"seiyaku ProvedCounter {
  state StateMap<int, int> Counters;
  kotoage fn bump() authorize("CanEnactGovernance") {
    let current = Counters.get(7).unwrap_or(0);
    Counters[7] = current + 1;
  }
}"#,
    )
    .expect("compile the real zero-argument counter with the ZK mode bit")
}

fn bounded_client(client: Client) -> Client {
    integration_tests::sync::rebind_blocking_client(&client, |configured| {
        configured.transaction_status_timeout = OBSERVATION_TIMEOUT;
        configured.torii_request_timeout = Duration::from_secs(90);
        configured.transaction_ttl = Some(Duration::from_secs(660));
    })
}

async fn derive(
    client: &Client,
    bytecode: &IvmBytecode,
    metadata: &Metadata,
    fee_payment: &FeePaymentIntent,
    key: &VerifyingKeyId,
) -> Result<IvmProved> {
    let request = norito::json::to_value(&ZkIvmDeriveRequestDto {
        vk_ref: key.clone(),
        authority: client.client().account().clone(),
        fee_payment: fee_payment.clone(),
        metadata: metadata.clone(),
        bytecode: bytecode.clone(),
    })?;
    let client = client.clone();
    let response = read_on_dedicated_thread(move || {
        // The normal SDK signs this exact-network, exact-authority request.
        client.client().post_zk_ivm_derive_json(&request)
    })
    .await?;
    let response: ZkIvmDeriveResponseDto = norito::json::from_value(response)?;
    ensure!(
        response.proved.bytecode == *bytecode,
        "derive changed bytecode"
    );
    Ok(response.proved)
}

async fn prove(code_hash: Hash, proved: &IvmProved, key: &VerifyingKeyBox) -> Result<ProofBox> {
    let overlay_hash = Hash::new(norito::encode_canonical(&proved.overlay)?);
    let events = proved.events_commitment;
    let gas = proved.gas_policy_commitment;
    let key = key.clone();
    read_on_dedicated_thread(move || {
        let proof = zk::prove_halo2_ipa_ivm_replay_binding_envelope(
            zk::IVM_REPLAY_BINDING_V1_CANONICAL_CIRCUIT_ID,
            &key,
            code_hash,
            overlay_hash,
            events,
            gas,
            None,
        )
        .map_err(Report::msg)?;
        ensure!(
            zk::verify_backend(zk::ZK_BACKEND_HALO2_IPA, &proof, Some(&key)),
            "locally generated binding proof must be cryptographically valid"
        );
        Ok(proof)
    })
    .await
}

async fn transaction(
    client: &Client,
    proved: IvmProved,
    attachment: ProofAttachment,
    metadata: &Metadata,
    fee_payment: &FeePaymentIntent,
    nonce: u32,
) -> Result<SignedTransaction> {
    let account = client.account_client();
    let mut payload = account.prepare_transaction(
        AccountTransactionDraft::new(
            Executable::IvmProved(proved),
            fee_payment.clone(),
            metadata.clone(),
        )
        .with_attachments(ProofAttachmentList::try_from(vec![attachment])?)
        .with_time_to_live(Duration::from_secs(660)),
    )?;
    payload.nonce = Some(NonZeroU32::new(nonce).expect("nonzero test nonce"));
    let quote = tokio::time::timeout(
        OBSERVATION_TIMEOUT,
        account.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }),
    )
    .await
    .map_err(|_| eyre!("exact IvmProved fee quote timed out"))??;
    ensure!(
        payload
            .fee_payment
            .has_same_payer_and_gas_bound(&quote.intent),
        "fee quote changed replay-bound payer or gas limit"
    );
    payload.fee_payment = quote.intent;
    Ok(account.sign_transaction(payload)?)
}

async fn applied_everywhere(
    network: &Network,
    http: &reqwest::Client,
    hash: HashOf<SignedTransaction>,
    label: &str,
) -> Result<()> {
    tokio::time::timeout(OBSERVATION_TIMEOUT, async {
        let hash = hex::encode(hash.as_ref());
        let mut common_height = None;
        for peer in network.peers() {
            let height = wait_for_tx_applied(
                http,
                peer.client().client().endpoint(),
                &hash,
                OBSERVATION_TIMEOUT,
                label,
            )
            .await?;
            if let Some(expected) = common_height {
                ensure!(height == expected, "validators disagree on Applied height");
            }
            common_height = Some(height);
        }
        Ok(())
    })
    .await
    .map_err(|_| eyre!("{label}: all-peer Applied observations exceeded the shared deadline"))?
}

async fn counter_everywhere(
    network: &Network,
    http: &reqwest::Client,
    address: &ContractAddress,
    expected: &str,
) -> Result<()> {
    tokio::time::timeout(OBSERVATION_TIMEOUT, async {
        for (index, peer) in network.peers().iter().enumerate() {
            let value = contract_state_json_value(
                http,
                peer.client().client().endpoint(),
                address,
                "Counters/7",
            )
            .await?;
            ensure!(
                value == norito::json::Value::from(expected),
                "validator {index} counter changed: {value:?}"
            );
        }
        Ok(())
    })
    .await
    .map_err(|_| eyre!("all-peer counter observations exceeded the shared deadline"))?
}

fn require_exact_rejection(
    result: Result<HashOf<SignedTransaction>>,
    expected: &str,
) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| eyre!("invalid proved transaction was Applied"))?;
    let Some(TransactionRejectionReason::Validation(ValidationFail::NotPermitted(message))) =
        error.downcast_ref::<TransactionRejectionReason>()
    else {
        return Err(eyre!(
            "transport, timeout, fee or unrelated rejection cannot establish proof rejection: {error:?}"
        ));
    };
    ensure!(
        message == expected,
        "wrong native rejection: {message:?}, expected {expected:?}"
    );
    Ok(())
}

async fn require_authenticated_rejection(
    client: &Client,
    transaction: &SignedTransaction,
    result: Result<HashOf<SignedTransaction>>,
    expected: &str,
) -> Result<()> {
    let error = result
        .err()
        .ok_or_else(|| eyre!("invalid proved transaction was Applied"))?;
    if error.downcast_ref::<TransactionRejectionReason>().is_some() {
        return require_exact_rejection(Err(error), expected);
    }
    let failure = error
        .downcast_ref::<TransactionFinalityFailure>()
        .ok_or_else(|| {
            eyre!(
                "proof rejection requires a typed native result, not transport failure: {error:?}"
            )
        })?;
    failure.validate_for_hash(transaction.hash())?;
    ensure!(
        failure.response().status.kind == "Rejected",
        "expiry is not proof rejection"
    );
    let entrypoint_hash = transaction.hash_as_entrypoint();
    tokio::time::timeout(OBSERVATION_TIMEOUT, async {
        loop {
            let details = read_on_dedicated_thread({
                let client = client.clone();
                move || {
                    client
                        .client()
                        .get_transaction_details(entrypoint_hash)
                        .map_err(Report::new)
                }
            })
            .await;
            let details = match details {
                Ok(details) => details,
                Err(error)
                    if matches!(
                        error.downcast_ref::<QueryError>(),
                        Some(QueryError::Validation(ValidationFail::QueryFailed(
                            QueryExecutionFail::NotFound | QueryExecutionFail::CapacityLimit
                        )))
                    ) =>
                {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                Err(error) => return Err(error),
            };
            // The normal signed details API already validates the canonical
            // source/output binding. Also require the exact original signed
            // payload, not merely another rejection at the requested height.
            ensure!(
                details.transaction.entrypoint_hash() == &entrypoint_hash,
                "wrong details entrypoint"
            );
            let TransactionEntrypoint::External(committed) = details.transaction.entrypoint()
            else {
                return Err(eyre!(
                    "rejection details must contain an external signed transaction"
                ));
            };
            ensure!(
                committed == transaction,
                "rejection details changed the signed transaction"
            );
            let reason = details
                .transaction
                .result()
                .0
                .as_ref()
                .err()
                .ok_or_else(|| eyre!("rejection details contain a successful result"))?;
            return require_exact_rejection(Err(eyre!(reason.clone())), expected);
        }
    })
    .await
    .map_err(|_| eyre!("authenticated rejection details exceeded the shared deadline"))?
}

async fn rejected_unchanged_everywhere(
    network: &Network,
    http: &reqwest::Client,
    address: &ContractAddress,
    metadata: &Metadata,
    fee_payment: &FeePaymentIntent,
    proved: &IvmProved,
    attachment: &ProofAttachment,
    case: u32,
    label: &str,
    expected: &str,
) -> Result<()> {
    // Fresh signed identities exercise each peer's ingress without reusing a
    // prior request. Routing may forward admission to an authoritative peer;
    // this does not establish which validator performed the local execution.
    for (index, peer) in network.peers().iter().enumerate() {
        let client = bounded_client(peer.client());
        let transaction = transaction(
            &client,
            proved.clone(),
            attachment.clone(),
            metadata,
            fee_payment,
            100 + 4 * case + u32::try_from(index)?,
        )
        .await?;
        let result = read_on_dedicated_thread({
            let client = client.clone();
            let transaction = transaction.clone();
            move || client.submit_transaction_and_wait(&transaction)
        })
        .await;
        require_authenticated_rejection(&client, &transaction, result, expected).await?;
    }
    // An independently Applied barrier rules out stalled validators. Its own
    // ordinary log effect is unrelated to the counter under test.
    let client = bounded_client(network.client());
    let label_owned = label.to_owned();
    let hash = read_on_dedicated_thread(move || {
        client.submit(
            Log::new(
                Level::INFO,
                format!("IvmProved rejection barrier {label_owned}"),
            ),
            FeePaymentIntent::authority(Vec::new(), None),
        )
    })
    .await?;
    applied_everywhere(network, http, hash, label).await?;
    counter_everywhere(network, http, address, "1").await
}

fn relabel(proof: &ProofBox, circuit_id: &str) -> Result<ProofBox> {
    let mut envelope: OpenVerifyEnvelope = norito::decode_canonical(&proof.bytes)?;
    envelope.circuit_id = circuit_id.into();
    Ok(ProofBox::new(
        proof.backend.clone(),
        norito::encode_canonical(&envelope)?,
    ))
}

#[test]
#[ignore = "requires same-candidate native daemon, four validators and real Halo2 proving"]
fn four_validator_ivm_proved_rejects_alias_role_corruption_and_forged_binding() -> Result<()> {
    ensure!(
        std::env::var("IROHA_TEST_REQUIRE_NETWORK").as_deref() == Ok("1"),
        "this qualification requires IROHA_TEST_REQUIRE_NETWORK=1"
    );
    let worker = std::thread::Builder::new()
        .name(TEST_NAME.into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()?
                .block_on(run())
        })?;
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

async fn run() -> Result<()> {
    let replay =
        zk::halo2_ipa_ivm_replay_binding_vk_record("integration", 1).map_err(Report::msg)?;
    let key = replay.key.clone().expect("canonical replay key");
    let key_id = VerifyingKeyId::new(zk::ZK_BACKEND_HALO2_IPA, "ivm_proved_replay_vk");
    let full = confidential_v2::confidential_unshield_v2_vk_record("integration", 1)
        .map_err(Report::msg)?;
    let wrong_key_id = VerifyingKeyId::new(zk::ZK_BACKEND_HALO2_IPA, "ivm_proved_wrong_role_vk");
    let alias = ContractAlias::from_components("proved_counter", None, "universal")?;
    let alias_permission = CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Alias(
            iroha_data_model::alias_setup::ResolvedAccountAliasV1::new(
                alias.canonical_text().parse()?,
                DataSpaceId::UNIVERSAL,
            ),
        ),
    };
    let mut builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers();
    for permission in [
        Permission::from(CanManageSmartContractCode),
        Permission::from(CanEnactGovernance),
        Permission::from(alias_permission),
    ] {
        builder = builder
            .with_genesis_instruction(Grant::account_permission(permission, ALICE_ID.clone()));
    }
    builder = builder
        .with_genesis_instruction(Grant::account_permission(
            Permission::from(CanManageVerifyingKeys),
            SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        ))
        .with_genesis_instruction(verifying_keys::RegisterVerifyingKey {
            id: key_id.clone(),
            record: replay,
        })
        .with_genesis_instruction(verifying_keys::RegisterVerifyingKey {
            id: wrong_key_id.clone(),
            record: full,
        });
    let network = sandbox::start_network_async_or_skip(builder, TEST_NAME)
        .await?
        .ok_or_else(|| eyre!("required four-validator native network was skipped"))?;
    ensure!(
        network.peers().len() == 4,
        "expected exactly four validators"
    );
    network.ensure_blocks(1).await?;
    let client = bounded_client(network.client());
    let http = integration_tests::http::client();
    let artifact = counter_artifact();
    let verified = ivm::verify_contract_artifact(&artifact)
        .map_err(|error| eyre!("counter artifact: {error}"))?;
    let parsed = ivm::ProgramMetadata::parse(&artifact)?;
    let gas_limit = iroha_core::smartcontracts::ivm::gas_limit_for_meta(&parsed.metadata)?;
    let fee_payment = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(gas_limit));
    let (address, _, height) = deploy_contract_artifact(
        &client,
        &http,
        &artifact,
        "proved_counter",
        "deploy replay counter",
    )
    .await?;
    network.ensure_blocks(height).await?;
    let metadata = contract_probe_call_intent(&artifact, &address, &alias, "bump")?.metadata;
    let bytecode = IvmBytecode::from_compiled(artifact);
    let initial = derive(&client, &bytecode, &metadata, &fee_payment, &key_id).await?;
    let initial_proof = prove(verified.code_hash, &initial, &key).await?;
    let initial_attachment = ProofAttachment::new_ref(
        zk::ZK_BACKEND_HALO2_IPA.into(),
        initial_proof,
        key_id.clone(),
    );
    let valid = transaction(
        &client,
        initial.clone(),
        initial_attachment.clone(),
        &metadata,
        &fee_payment,
        1,
    )
    .await?;
    let hash = read_on_dedicated_thread({
        let client = client.clone();
        move || client.submit_transaction_and_wait(&valid)
    })
    .await?;
    applied_everywhere(&network, &http, hash, "valid IvmProved call").await?;
    counter_everywhere(&network, &http, &address, "1").await?;

    let current = derive(&client, &bytecode, &metadata, &fee_payment, &key_id).await?;
    let current_proof = prove(verified.code_hash, &current, &key).await?;
    let honest = ProofAttachment::new_ref(
        zk::ZK_BACKEND_HALO2_IPA.into(),
        current_proof.clone(),
        key_id.clone(),
    );
    let mut backend_alias = honest.clone();
    backend_alias.backend = "halo2/ipa/pasta".into();
    backend_alias.proof.backend = backend_alias.backend.clone();
    backend_alias.vk_ref.backend = backend_alias.backend.clone();
    let mut wrong_role = honest.clone();
    wrong_role.vk_ref = wrong_key_id;
    wrong_role.proof = relabel(
        &current_proof,
        confidential_v2::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID,
    )?;
    let cases = [
        (
            "retired-circuit",
            ProofAttachment::new_ref(
                zk::ZK_BACKEND_HALO2_IPA.into(),
                relabel(&current_proof, "halo2/pasta/ipa/ivm-execution-v1")?,
                key_id.clone(),
            ),
            EXACT_CIRCUIT_REJECTION,
        ),
        (
            "bare-circuit-alias",
            ProofAttachment::new_ref(
                zk::ZK_BACKEND_HALO2_IPA.into(),
                relabel(&current_proof, "ivm-replay-binding-v1")?,
                key_id.clone(),
            ),
            EXACT_CIRCUIT_REJECTION,
        ),
        (
            "backend-alias",
            backend_alias,
            "zk_proof: unsupported backend for Executable::IvmProved (expected halo2/ipa or stark/fri)",
        ),
        ("wrong-relation", wrong_role, EXACT_CIRCUIT_REJECTION),
        (
            "native-corruption",
            ProofAttachment::new_ref(
                zk::ZK_BACKEND_HALO2_IPA.into(),
                proof_fixtures::corrupt_native_halo2_proof(&current_proof),
                key_id.clone(),
            ),
            "zk_proof: proof or verifying key failed cryptographic verification",
        ),
    ];
    for (case, (label, attachment, reason)) in cases.iter().enumerate() {
        rejected_unchanged_everywhere(
            &network,
            &http,
            &address,
            &metadata,
            &fee_payment,
            &current,
            attachment,
            u32::try_from(case)?,
            label,
            reason,
        )
        .await?;
    }
    // The old payload/proof is still cryptographically valid, but replay now
    // reads counter=1 instead of counter=0. Re-signing must not spend it twice.
    rejected_unchanged_everywhere(
        &network,
        &http,
        &address,
        &metadata,
        &fee_payment,
        &initial,
        &initial_attachment,
        5,
        "stale-state-replay",
        REPLAY_REJECTION,
    )
    .await?;
    let mut forged = current;
    let mut overlay: Vec<InstructionBox> = forged.overlay.iter().cloned().collect();
    overlay.push(Log::new(Level::INFO, "forged overlay effect".to_owned()).into());
    forged.overlay = overlay.into();
    // The native prover really accepts these four commitments. Only execution
    // replay detects the forgery; no private commitment implementation is copied.
    let forged_proof = prove(verified.code_hash, &forged, &key).await?;
    let forged_attachment =
        ProofAttachment::new_ref(zk::ZK_BACKEND_HALO2_IPA.into(), forged_proof, key_id);
    rejected_unchanged_everywhere(
        &network,
        &http,
        &address,
        &metadata,
        &fee_payment,
        &forged,
        &forged_attachment,
        6,
        "valid-binding-forged-overlay",
        REPLAY_REJECTION,
    )
    .await?;
    Ok(())
}

#[test]
fn replay_counter_fixture_uses_real_v1_interface_and_zk_mode() -> Result<()> {
    let artifact = counter_artifact();
    let parsed = ivm::ProgramMetadata::parse(&artifact)?;
    ensure!(
        parsed.metadata.mode & ivm::ivm_mode::ZK != 0,
        "ZK mode missing"
    );
    let verified = ivm::verify_contract_artifact(&artifact).map_err(|error| eyre!("{error}"))?;
    ensure!(
        verified
            .contract_interface
            .entrypoints
            .iter()
            .any(|entry| entry.name == "bump" && entry.params.is_empty()),
        "exact no-argument entrypoint missing"
    );
    Ok(())
}

#[test]
fn replay_rejection_control_rejects_transport_and_unrelated_native_errors() {
    let rejection = |message: &str| {
        eyre!(TransactionRejectionReason::Validation(
            ValidationFail::NotPermitted(message.to_owned())
        ))
    };
    require_exact_rejection(Err(rejection(REPLAY_REJECTION)), REPLAY_REJECTION).unwrap();
    assert!(
        require_exact_rejection(
            Err(eyre!("transport: {REPLAY_REJECTION}")),
            REPLAY_REJECTION
        )
        .is_err()
    );
    assert!(require_exact_rejection(Err(rejection("fee refused")), REPLAY_REJECTION).is_err());
}
