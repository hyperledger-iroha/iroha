//! Exact, locally authorized contract calls using the native deployment journal store.
use super::*;
use base64::Engine as _;
use iroha::client::ContractCallDraftIntent;
use iroha::data_model::{
    smart_contract::manifest::EntryPointKind,
    transaction::{Executable, TransactionAdmissionIntent},
};
use iroha_crypto::Signature;
use iroha_executor_data_model::permission::smart_contract::CanInvokeContractEntrypoint;
use norito::json::Value;

/// Local artifact and invocation selected by the package-aware caller.
#[derive(Clone, Debug)]
pub struct ContractCallRequest {
    /// Complete verified local compiled artifact.
    pub artifact: Vec<u8>,
    /// Human-readable configured alias, resolved to the exact invocation address before signing.
    pub alias: ContractAlias,
    /// Canonical named arguments; absent for a zero-parameter entrypoint.
    pub payload: Option<Value>,
    /// Exact executable and metadata derived locally from the artifact and arguments.
    pub intent: ContractCallDraftIntent,
    /// Explicit payer, sponsor revision and gas bound.
    pub fee_payment: FeePaymentIntent,
}
#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CallPlan {
    version: u8,
    network_id: NetworkId,
    chain_id: String,
    authority: AccountId,
    chain_discriminant: u16,
    created_at_ns: u64,
    artifact_hex: String,
    alias: ContractAlias,
    payload: Option<Value>,
    intent: ContractCallDraftIntent,
    requested_fee: FeePaymentIntent,
    grant: Option<TransactionRecord>,
}
/// Immutable signed local operation; contains no private key.
#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct PreparedContractCall {
    plan: CallPlan,
    signature_hex: String,
}
impl PreparedContractCall {
    /// Stable journal identifier derived from the complete locally signed operation.
    /// # Errors
    /// Returns the canonical JSON encoding error.
    pub fn operation_id(&self) -> Result<String> {
        let _profile = ChainDiscriminantGuard::enter(self.plan.chain_discriminant);
        Ok(hex::encode(
            Hash::new(plan_signing_bytes(&self.plan)?).as_ref(),
        ))
    }
    /// Exact contract address selected by the caller.
    pub fn contract_address(&self) -> &ContractAddress {
        &self.plan.intent.invocation.contract_address
    }
    /// Exact public entrypoint selected by the caller.
    pub fn entrypoint(&self) -> &str {
        &self.plan.intent.invocation.entrypoint
    }
    /// Whether this operation includes an exact permission grant to the calling account.
    pub fn grants_entrypoint_to_self(&self) -> bool {
        self.plan.grant.is_some()
    }
}
/// Receipt emitted only after the exact retained call reaches global state-resolved Applied.
#[derive(Clone, Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct ContractCallReceipt {
    /// Stable locally signed operation identifier.
    pub operation_id: String,
    /// Exact network identity.
    pub network_id: NetworkId,
    /// Transaction authority.
    pub authority: AccountId,
    /// Configured alias observed before preparation.
    pub contract_alias: ContractAlias,
    /// Exact signature-bound invocation address.
    pub contract_address: ContractAddress,
    /// Exact signature-bound selector.
    pub entrypoint: String,
    /// Optional preceding exact self-grant's Applied evidence.
    pub grant: Option<AppliedEvidence>,
    /// Exact mutable call's Applied evidence.
    pub call: AppliedEvidence,
}
/// Retained disposition used to prevent a fresh operation from replacing unresolved work.
pub enum ContractCallDisposition {
    /// An unattempted stage or an unresolved exact hash requires resume.
    Pending,
    /// The complete operation was cancelled before any transaction attempt.
    Cancelled,
    /// The exact operation reached Applied and was rechecked against the configured network.
    Applied(ContractCallReceipt),
    /// The exact attempted transaction is authoritatively rejected or expired.
    Failed(TransactionFinalityFailure),
}
/// An attempted transaction is unresolved; retry only the retained journal.
#[derive(Debug, thiserror::Error)]
#[error("contract call stage `{stage}` hash {hash} is unresolved; resume its journal: {source}")]
pub struct ContractCallPending {
    /// Exact stage name.
    pub stage: String,
    /// Exact signed hash recorded before dispatch.
    pub hash: String,
    /// Original transport/finality diagnostic.
    #[source]
    pub source: eyre::Report,
}
/// Package contract call preparation and exact-hash recovery using one immutable signer context.
pub struct ContractCallService {
    config: Config,
    client: Client,
}
impl ContractCallService {
    /// Construct the service without network or filesystem effects.
    /// # Errors
    /// Rejects invalid client context or a zero finality timeout.
    pub fn new(config: Config) -> Result<Self> {
        if config.transaction_status_timeout.is_zero() {
            return Err(eyre!("call finality timeout must be nonzero"));
        }
        let client = Client::new(config.clone())?;
        Ok(Self { config, client })
    }
    /// Resolve the configured alias through the authenticated native deployment-state read.
    /// # Errors
    /// Rejects missing aliases or inconsistent network/account response bindings.
    pub fn resolve_address(&self, alias: &ContractAlias) -> Result<ContractAddress> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        self.client.refresh_capabilities()?;
        read_contract_deployment_state(
            &self.client,
            &self.config.account,
            alias,
            self.config.account_chain_discriminant,
        )?
        .previous_contract_address
        .ok_or_else(|| eyre!("contract alias `{alias}` is not deployed"))
    }
    /// Prepare a locally signed immutable call intent and any required exact self-grant.
    /// No transaction is submitted or journal written.
    /// # Errors
    /// Rejects artifact, alias, argument, permission-read, fee, or signing failures.
    pub fn prepare(&self, request: ContractCallRequest) -> Result<PreparedContractCall> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        if request.artifact.is_empty() || request.artifact.len() > MAX_DEPLOYMENT_ARTIFACT_BYTES {
            return Err(eyre!("call artifact exceeds fixed bounds"));
        }
        if request.payload.as_ref().is_some_and(|value| {
            norito::json::to_vec(value).map_or(true, |bytes| bytes.len() > 64 * 1024)
        }) {
            return Err(eyre!("call arguments exceed the 64 KiB bound"));
        }
        let verified = ivm_artifact_admission::verify_contract_artifact(&request.artifact)?;
        let permission = required_permission(&verified, &request.intent)?;
        request.fee_payment.validate()?;
        if request.fee_payment.gas_limit().is_none() {
            return Err(eyre!("call requires an explicit gas bound"));
        }
        if self.resolve_address(&request.alias)? != request.intent.invocation.contract_address {
            return Err(eyre!("contract alias changed before call preparation"));
        }
        let mut grant = None;
        if let Some(permission) = permission {
            let held =
                authorization::read_effective_permissions(&self.client, &self.config.account)?;
            if !held.contains(&permission) {
                if permission.name() != "CanInvokeContractEntrypoint" {
                    return Err(eyre!(
                        "contract requires the exact permission `{}`; no broader permission is granted by call",
                        permission.name()
                    ));
                }
                let metadata = Metadata::default();
                let signing = TransactionSigningContext {
                    network_id: self.config.network_id,
                    authority: &self.config.account,
                    private_key: self.config.key_pair.private_key(),
                    transaction_ttl: Some(self.config.transaction_ttl),
                    fee_payment: &request.fee_payment,
                    metadata: &metadata,
                };
                let draft = signing.sign([InstructionBox::from(Grant::account_permission(
                    permission,
                    self.config.account.clone(),
                ))])?;
                let (signed, quote) =
                    quote_and_resign_transaction(&self.client, &draft, &request.fee_payment)?;
                self.client.check_funding(&Default::default(), &[quote])?;
                grant = Some(transaction_record("entrypoint-grant", &signed));
            }
        }
        let mut plan = CallPlan {
            version: 1,
            network_id: self.config.network_id,
            chain_id: self.config.chain.to_string(),
            authority: self.config.account.clone(),
            chain_discriminant: self.config.account_chain_discriminant,
            created_at_ns: u64::try_from(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos(),
            )?,
            artifact_hex: hex::encode(request.artifact),
            alias: request.alias,
            payload: request.payload,
            intent: request.intent,
            requested_fee: request.fee_payment,
            grant,
        };
        if plan
            .intent
            .metadata
            .get(&operation_metadata_key())
            .is_some()
        {
            return Err(eyre!(
                "caller may not override the durable call operation tag"
            ));
        }
        bind_operation_metadata(&mut plan)?;
        let signature = Signature::try_new(
            self.config.key_pair.private_key(),
            &plan_signing_bytes(&plan)?,
        )?;
        let prepared = PreparedContractCall {
            plan,
            signature_hex: hex::encode(signature.payload()),
        };
        validate_plan(&prepared, &self.config)?;
        Ok(prepared)
    }
    /// Persist the operation before either the grant or call can be dispatched.
    /// # Errors
    /// Rejects substituted plans or unsafe/busy journal storage.
    pub fn persist(&self, prepared: &PreparedContractCall, path: &Path) -> Result<()> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        validate_plan(prepared, &self.config)?;
        Journal::open(path, true)?.put_exact("plan.json", prepared)
    }
    /// Cancel a fully unattempted local operation without submitting any transaction.
    /// # Errors
    /// Rejects unsafe custody or any retained attempt, call payload, or unknown evidence.
    pub fn cancel(&self, path: &Path) -> Result<String> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(path, false)?;
        let prepared: PreparedContractCall = journal.read("plan.json")?;
        validate_plan(&prepared, &self.config)?;
        journal.require_unattempted()?;
        let operation_id = prepared.operation_id()?;
        journal.put_exact("cancelled.json", &operation_id)?;
        Ok(operation_id)
    }
    /// Execute an unattempted operation or recover its exact attempted hashes.
    /// # Errors
    /// Returns exact-hash pending, authoritative failure, or journal errors.
    pub fn resume(&self, path: &Path) -> Result<ContractCallReceipt> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(path, false)?;
        let prepared: PreparedContractCall = journal.read("plan.json")?;
        validate_plan(&prepared, &self.config)?;
        validate_stage_layout(&journal, &prepared.plan)?;
        if is_cancelled(&journal, &prepared)? {
            return Err(eyre!(
                "this call was cancelled before submission; create a new operation"
            ));
        }
        self.client.refresh_capabilities()?;
        let transport = CallTransport { service: self };
        let grant = prepared
            .plan
            .grant
            .as_ref()
            .map(|grant| execute_step(&journal, grant, 0, &transport))
            .transpose()?;
        let step = if journal.exists("call.json")? {
            journal.read::<TransactionRecord>("call.json")?
        } else {
            let plan = &prepared.plan;
            // Permission preparation occurs only after the exact grant has reached Applied.
            // The SDK verifies the complete Ordinary payload against the local trusted intent.
            let response = self.client.post_contract_call_json(
                &self.config.account,
                None,
                Some(&plan.intent.invocation.contract_address),
                None,
                &plan.intent.invocation.entrypoint,
                plan.payload.as_ref(),
                Some(&operation_metadata(plan)?),
                None,
                Some(u64::try_from(self.config.transaction_ttl.as_millis())?),
                &plan.requested_fee,
                &plan.intent,
            )?;
            let encoded = response
                .get("transaction_payload_b64")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("verified call draft omitted payload"))?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
            let builder = TransactionBuilder::decode_payload(&bytes)?;
            let signed = builder.try_sign(self.config.key_pair.private_key())?;
            let step = transaction_record("contract-call", &signed);
            validate_call_transaction(&prepared.plan, &step)?;
            journal.put_exact("call.json", &step)?;
            step
        };
        validate_call_transaction(&prepared.plan, &step)?;
        let call = execute_step(&journal, &step, 1, &transport)?;
        let receipt = ContractCallReceipt {
            operation_id: prepared.operation_id()?,
            network_id: prepared.plan.network_id,
            authority: prepared.plan.authority.clone(),
            contract_alias: prepared.plan.alias.clone(),
            contract_address: prepared.plan.intent.invocation.contract_address.clone(),
            entrypoint: prepared.plan.intent.invocation.entrypoint.clone(),
            grant,
            call,
        };
        journal.put_exact(RECEIPT_FILE_NAME, &receipt)?;
        Ok(receipt)
    }
    /// Inspect a retained operation without preparing, signing, or submitting any transaction.
    /// # Errors
    /// Returns journal/substitution errors; unavailable evidence remains pending.
    pub fn inspect(&self, path: &Path) -> Result<ContractCallDisposition> {
        let _profile = ChainDiscriminantGuard::enter(self.config.account_chain_discriminant);
        let journal = Journal::open(path, false)?;
        let prepared: PreparedContractCall = journal.read("plan.json")?;
        validate_plan(&prepared, &self.config)?;
        validate_stage_layout(&journal, &prepared.plan)?;
        if is_cancelled(&journal, &prepared)? {
            return Ok(ContractCallDisposition::Cancelled);
        }
        let mut grant = None;
        for (index, step) in [
            (0, prepared.plan.grant.clone()),
            (
                1,
                if journal.exists("call.json")? {
                    Some(journal.read("call.json")?)
                } else {
                    None
                },
            ),
        ] {
            let Some(step) = step else {
                if index == 1 {
                    return Ok(ContractCallDisposition::Pending);
                }
                continue;
            };
            if index == 1 {
                validate_call_transaction(&prepared.plan, &step)?;
            }
            if !journal.exists(&format!("attempt-{index:04}.json"))? {
                return Ok(ContractCallDisposition::Pending);
            }
            validate_attempt(&journal, &step, index)?;
            match (CallTransport { service: self }).wait(decode_transaction(&step)?.hash()) {
                Ok(applied) => {
                    validate_applied(decode_transaction(&step)?.hash(), &applied)?;
                    validate_retained_evidence(&journal, index, &applied)?;
                    if index == 0 {
                        grant = Some(applied);
                    } else {
                        let receipt = ContractCallReceipt {
                            operation_id: prepared.operation_id()?,
                            network_id: prepared.plan.network_id,
                            authority: prepared.plan.authority.clone(),
                            contract_alias: prepared.plan.alias.clone(),
                            contract_address: prepared
                                .plan
                                .intent
                                .invocation
                                .contract_address
                                .clone(),
                            entrypoint: prepared.plan.intent.invocation.entrypoint.clone(),
                            grant,
                            call: applied,
                        };
                        if journal.exists(RECEIPT_FILE_NAME)? {
                            let retained: ContractCallReceipt = journal.read(RECEIPT_FILE_NAME)?;
                            if norito::json::to_vec(&retained)? != norito::json::to_vec(&receipt)? {
                                return Err(eyre!(
                                    "retained call receipt disagrees with exact current Applied evidence"
                                ));
                            }
                        }
                        return Ok(ContractCallDisposition::Applied(receipt));
                    }
                }
                Err(error) => {
                    if let Some(proof) = finality_failure(&error) {
                        validate_retained_failure(
                            &journal,
                            index,
                            decode_transaction(&step)?.hash(),
                            &proof,
                        )?;
                        return Ok(ContractCallDisposition::Failed(proof));
                    }
                    return Ok(ContractCallDisposition::Pending);
                }
            }
        }
        Ok(ContractCallDisposition::Pending)
    }
}
fn plan_signing_bytes(plan: &CallPlan) -> Result<Vec<u8>> {
    let mut bytes = b"iroha.contract-call-operation.v1\0".to_vec();
    bytes.extend(norito::json::to_vec(plan)?);
    Ok(bytes)
}
fn operation_metadata_key() -> Name {
    "musubi_call_operation"
        .parse()
        .expect("static operation metadata key")
}
fn operation_tag(plan: &CallPlan) -> Result<String> {
    let mut unsigned = plan.clone();
    unsigned.intent.metadata.remove(&operation_metadata_key());
    Ok(hex::encode(
        Hash::new(plan_signing_bytes(&unsigned)?).as_ref(),
    ))
}
fn bind_operation_metadata(plan: &mut CallPlan) -> Result<()> {
    let tag = operation_tag(plan)?;
    plan.intent
        .metadata
        .insert(operation_metadata_key(), Json::new(tag));
    Ok(())
}
fn operation_metadata(plan: &CallPlan) -> Result<Metadata> {
    let tag = Json::new(operation_tag(plan)?);
    if plan.intent.metadata.get(&operation_metadata_key()) != Some(&tag) {
        return Err(eyre!(
            "call metadata does not bind its exact immutable operation"
        ));
    }
    let mut metadata = Metadata::default();
    metadata.insert(operation_metadata_key(), tag);
    Ok(metadata)
}
fn transaction_record(name: &str, signed: &SignedTransaction) -> TransactionRecord {
    TransactionRecord {
        name: name.to_owned(),
        hash: signed.hash().to_string(),
        norito_hex: hex::encode(signed.encode_versioned()),
    }
}
fn required_permission(
    artifact: &ivm_artifact_admission::VerifiedContractArtifact,
    intent: &ContractCallDraftIntent,
) -> Result<Option<Permission>> {
    if artifact.code_hash != intent.invocation.expected_code_hash {
        return Err(eyre!(
            "call code identity differs from the verified local artifact"
        ));
    }
    let descriptor = artifact
        .contract_interface
        .entrypoints
        .iter()
        .find(|entry| entry.name == intent.invocation.entrypoint)
        .ok_or_else(|| eyre!("entrypoint is absent from the verified local artifact"))?;
    let permission = match descriptor.kind {
        EntryPointKind::View => return Err(eyre!("read-only entrypoints use `musubi view`")),
        EntryPointKind::Kotoage => descriptor.permission.as_deref(),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => Some("CanInvokeContractEntrypoint"),
    };
    Ok(permission.map(|permission| {
        if permission == "CanInvokeContractEntrypoint" {
            CanInvokeContractEntrypoint {
                contract: intent.invocation.contract_address.clone(),
                entrypoint: intent.invocation.entrypoint.clone(),
            }
            .into()
        } else {
            Permission::new(permission.to_owned(), Json::new(()))
        }
    }))
}
fn validate_plan(prepared: &PreparedContractCall, config: &Config) -> Result<()> {
    let plan = &prepared.plan;
    if plan.version != 1
        || plan.network_id != config.network_id
        || plan.chain_id != config.chain.to_string()
        || plan.chain_discriminant != config.account_chain_discriminant
        || plan.authority != config.account
        || plan.authority.try_signatory() != Some(config.key_pair.public_key())
        || plan.created_at_ns == 0
    {
        return Err(eyre!(
            "call journal belongs to a different network, authority, or address profile"
        ));
    }
    if plan.artifact_hex.is_empty()
        || plan.artifact_hex.len() > 2 * MAX_DEPLOYMENT_ARTIFACT_BYTES
        || prepared.signature_hex.len() > 64 * 1024
    {
        return Err(eyre!(
            "call journal exceeds fixed artifact/signature bounds"
        ));
    }
    let signature = hex::decode(&prepared.signature_hex)?;
    if hex::encode(&signature) != prepared.signature_hex {
        return Err(eyre!("call plan signature is not canonical hex"));
    }
    Signature::from_bytes(&signature)
        .verify(config.key_pair.public_key(), &plan_signing_bytes(plan)?)?;
    let artifact = hex::decode(&plan.artifact_hex)?;
    if hex::encode(&artifact) != plan.artifact_hex {
        return Err(eyre!("call artifact is not canonical hex"));
    }
    let verified = ivm_artifact_admission::verify_contract_artifact(&artifact)?;
    let permission = required_permission(&verified, &plan.intent)?;
    operation_metadata(plan)?;
    plan.requested_fee.validate()?;
    if plan.requested_fee.gas_limit().is_none()
        || plan.payload.as_ref().is_some_and(|value| {
            norito::json::to_vec(value).map_or(true, |bytes| bytes.len() > 64 * 1024)
        })
        || plan.payload.is_some() != plan.intent.invocation.arguments.is_some()
    {
        return Err(eyre!(
            "call journal requires bounded arguments and an explicit gas limit"
        ));
    }
    if let Some(grant) = &plan.grant {
        if grant.norito_hex.len() > 2 * 1024 * 1024 {
            return Err(eyre!("retained self-grant exceeds fixed byte bound"));
        }
        let signed = decode_transaction(grant)?;
        let permission =
            permission.ok_or_else(|| eyre!("unguarded call must not create a grant"))?;
        if permission.name() != "CanInvokeContractEntrypoint" {
            return Err(eyre!(
                "call may grant only its exact entrypoint invocation permission"
            ));
        }
        let expected = Executable::Instructions(
            vec![InstructionBox::from(Grant::account_permission(
                permission,
                plan.authority.clone(),
            ))]
            .into(),
        );
        if grant.name != "entrypoint-grant"
            || signed.network_id() != Some(&plan.network_id)
            || signed.authority() != &plan.authority
            || signed.payload().admission_intent() != TransactionAdmissionIntent::Ordinary
            || signed.instructions() != &expected
            || !signed.metadata().is_empty()
            || !plan
                .requested_fee
                .has_same_payer_and_gas_bound(&signed.payload().fee_payment)
        {
            return Err(eyre!(
                "call self-grant differs from the exact owned entrypoint and authority"
            ));
        }
    }
    Ok(())
}
fn validate_call_transaction(plan: &CallPlan, step: &TransactionRecord) -> Result<()> {
    if step.norito_hex.len() > 2 * 1024 * 1024 {
        return Err(eyre!("retained call transaction exceeds fixed bound"));
    }
    let signed = decode_transaction(step)?;
    if step.name != "contract-call"
        || signed.network_id() != Some(&plan.network_id)
        || signed.authority() != &plan.authority
        || signed.payload().admission_intent() != TransactionAdmissionIntent::Ordinary
        || signed.instructions() != &Executable::ContractCall(plan.intent.invocation.clone())
        || signed.metadata() != &plan.intent.metadata
        || !plan
            .requested_fee
            .has_same_payer_and_gas_bound(&signed.payload().fee_payment)
    {
        return Err(eyre!(
            "retained call transaction differs from the locally signed operation"
        ));
    }
    Ok(())
}
struct CallTransport<'a> {
    service: &'a ContractCallService,
}
impl DeploymentTransport for CallTransport<'_> {
    fn submit(&self, signed: &SignedTransaction) -> Result<()> {
        self.service
            .client
            .submit_transaction_and_wait(signed)
            .map(|_| ())
    }
    fn wait(&self, hash: HashOf<SignedTransaction>) -> Result<AppliedEvidence> {
        applied_evidence(
            hash,
            self.service.client.wait_for_transaction_applied(
                hash,
                TransactionWaitOptions {
                    timeout: self.service.config.transaction_status_timeout,
                    ..TransactionWaitOptions::default()
                },
            )?,
        )
    }
}
fn is_cancelled(journal: &Journal, prepared: &PreparedContractCall) -> Result<bool> {
    if !journal.exists("cancelled.json")? {
        return Ok(false);
    }
    journal.require_unattempted()?;
    if journal.read::<String>("cancelled.json")? != prepared.operation_id()? {
        return Err(eyre!(
            "cancellation names a different immutable call operation"
        ));
    }
    Ok(true)
}
fn validate_stage_layout(journal: &Journal, plan: &CallPlan) -> Result<()> {
    let call_exists = journal.exists("call.json")?;
    for index in 0..2 {
        let attempted = journal.exists(&format!("attempt-{index:04}.json"))?;
        let applied = journal.exists(&format!("applied-{index:04}.json"))?;
        let failed = journal.exists(&format!("failed-{index:04}.json"))?;
        if (applied || failed) && !attempted || applied && failed {
            return Err(eyre!(
                "call stage has missing or conflicting durable attempt evidence"
            ));
        }
        if index == 0 && plan.grant.is_none() && attempted {
            return Err(eyre!("call has an unexpected permission-grant attempt"));
        }
        if index == 1 && attempted && !call_exists {
            return Err(eyre!(
                "attempted call has no retained signed payload; it must never be prepared again"
            ));
        }
    }
    if plan.grant.is_some() && call_exists && !journal.exists("applied-0000.json")? {
        return Err(eyre!(
            "call was prepared before its permission grant reached Applied"
        ));
    }
    if journal.exists(RECEIPT_FILE_NAME)?
        && (!call_exists || !journal.exists("applied-0001.json")?)
    {
        return Err(eyre!(
            "call receipt lacks its durable signed and Applied stage"
        ));
    }
    Ok(())
}
fn validate_retained_evidence(
    journal: &Journal,
    index: usize,
    evidence: &AppliedEvidence,
) -> Result<()> {
    if journal.exists(&format!("failed-{index:04}.json"))? {
        return Err(eyre!(
            "retained failure conflicts with current Applied evidence"
        ));
    }
    let path = format!("applied-{index:04}.json");
    if journal.exists(&path)? && journal.read::<AppliedEvidence>(&path)? != *evidence {
        return Err(eyre!(
            "retained Applied evidence differs from current exact-hash evidence"
        ));
    }
    Ok(())
}
fn validate_retained_failure(
    journal: &Journal,
    index: usize,
    hash: HashOf<SignedTransaction>,
    proof: &TransactionFinalityFailure,
) -> Result<()> {
    proof.validate_for_hash(hash)?;
    if journal.exists(&format!("applied-{index:04}.json"))? {
        return Err(eyre!(
            "retained Applied evidence conflicts with current failure"
        ));
    }
    let path = format!("failed-{index:04}.json");
    if journal.exists(&path)? {
        let retained: TransactionFinalityFailure = journal.read(&path)?;
        retained.validate_for_hash(hash)?;
        if norito::json::to_vec(&retained)? != norito::json::to_vec(proof)? {
            return Err(eyre!(
                "retained failure differs from current exact-hash evidence"
            ));
        }
    }
    Ok(())
}
fn validate_attempt(journal: &Journal, step: &TransactionRecord, index: usize) -> Result<()> {
    let marker: TransactionAttempt = journal.read(&format!("attempt-{index:04}.json"))?;
    if marker.name != step.name || marker.hash != step.hash {
        return Err(eyre!(
            "call attempt differs from its exact signed transaction"
        ));
    }
    Ok(())
}
fn finality_failure(error: &eyre::Report) -> Option<TransactionFinalityFailure> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<TransactionFinalityFailure>())
        .cloned()
}
fn execute_step<T: DeploymentTransport>(
    journal: &Journal,
    step: &TransactionRecord,
    index: usize,
    transport: &T,
) -> Result<AppliedEvidence> {
    let signed = decode_transaction(step)?;
    let attempt = format!("attempt-{index:04}.json");
    let result = (|| {
        if journal.exists(&attempt)? {
            validate_attempt(journal, step, index)?;
        } else {
            journal.put_exact(
                &attempt,
                &TransactionAttempt {
                    name: step.name.clone(),
                    hash: step.hash.clone(),
                },
            )?;
            transport.submit(&signed)?;
        }
        let evidence = transport.wait(signed.hash())?;
        validate_applied(signed.hash(), &evidence)?;
        validate_retained_evidence(journal, index, &evidence)?;
        journal.put_exact(&format!("applied-{index:04}.json"), &evidence)?;
        Ok(evidence)
    })();
    result.map_err(|error: eyre::Report| {
        if let Some(proof) = finality_failure(&error) {
            let recorded = (|| {
                validate_retained_failure(journal, index, signed.hash(), &proof)?;
                journal.put_exact(&format!("failed-{index:04}.json"), &proof)
            })();
            if let Err(journal_error) = recorded {
                return journal_error;
            }
            return error;
        }
        eyre::Report::new(ContractCallPending {
            stage: step.name.clone(),
            hash: step.hash.clone(),
            source: error,
        })
    })
}
#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
