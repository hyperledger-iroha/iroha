//! Once-per-epoch operator maintenance of independently provisioned public mint keys.
//!
//! This command never accepts validator seeds. The native Kagami provisioner emits
//! the public schedule; the configured ledger owner authorizes one real parameter
//! transaction per epoch. A retained dispatch is only observed, never replaced.

use super::*;
use iroha_data_model::{
    block::consensus_v2::HeightContext,
    parameter::system::KagemushaMintFinalityNextEpochParameterV1 as NextRoster,
    prelude::{FindParameters, FindPeers, Pagination},
    query::builder::QueryBuilderExt as _,
    transaction::signed::TransactionAdmissionIntent,
};
use std::{collections::BTreeSet, num::NonZeroU64};

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Read authenticated epoch state and report missing or mismatched provisioning early.
    Preflight(TargetArgs),
    /// Retain the exact next-epoch transaction without submitting it.
    Prepare(TargetArgs),
    /// Dispatch the retained transaction once, then verify its exact committed carrier.
    Apply(TargetArgs),
    /// Reverify a retained transaction without signing or submitting.
    Status(TargetArgs),
    /// Stage each next epoch only after observing its real predecessor epoch.
    Maintain(MaintainArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct CommonArgs {
    /// Independently selected genesis and four validator identities.
    #[arg(long)]
    trust: PathBuf,
    /// Native public provisioning output; no seed or private key fields are accepted.
    #[arg(long)]
    schedule: PathBuf,
    /// Existing owner-private parent of deterministic network/epoch journals.
    #[arg(long)]
    journal_dir: PathBuf,
    /// Finite whole invocation budget, including expected read-side restart intervals.
    #[arg(long, default_value_t = 180_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
    /// Original per-epoch preparation/dispatch budget; retained on disk and never renewed.
    #[arg(long, default_value_t = 180_000, value_parser = clap::value_parser!(u64).range(1..))]
    operation_timeout_ms: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct TargetArgs {
    #[command(flatten)]
    common: CommonArgs,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    target_epoch: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MaintainArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Exit only after this scheduled epoch's transaction has verified committed finality.
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    stop_after_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ScheduleV1 {
    schema_version: u8,
    network_id: NetworkId,
    parameters: Vec<Parameter>,
    payment_asset: AssetDefinitionId,
    transaction_fee_maximum: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct EpochPlanV1 {
    schema_version: u8,
    network_id: NetworkId,
    target_epoch: u64,
    owner: AccountId,
    schedule_sha256: String,
    payment_asset: AssetDefinitionId,
    transaction_fee_maximum: Quantity,
    trust_sha256: String,
    parameter: Parameter,
    expires_at_ms: u64,
    epoch_end_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CompletionV1 {
    schema_version: u8,
    network_id: NetworkId,
    target_epoch: u64,
    transaction_hash: String,
    applied_height: u64,
    parameter_sha256: String,
    carrier_sha256: String,
}

#[derive(JsonSerialize)]
struct ProgressV1 {
    schema_version: u8,
    network_id: NetworkId,
    current_epoch: u64,
    epoch_end_height: u64,
    observed_height: u64,
    target_epoch: u64,
    state: String,
    #[norito(required)]
    transaction_hash: Option<String>,
}

fn collect_optional_reads<T>(reads: Vec<Result<T>>) -> Result<Vec<Option<T>>> {
    reads
        .into_iter()
        .map(|read| match read {
            Ok(value) => Ok(Some(value)),
            Err(error) if crate::taira::observation_transport_unavailable(&error) => Ok(None),
            Err(error) => Err(error),
        })
        .collect()
}

fn typed_parameter(parameter: &Parameter) -> Result<NextRoster> {
    let Parameter::Custom(custom) = parameter else {
        eyre::bail!("epoch schedule accepts only native next-mint-roster parameters");
    };
    NextRoster::from_custom_parameter(custom)
        .ok_or_else(|| eyre!("epoch schedule contains an invalid next-mint-roster parameter"))
}

impl ScheduleV1 {
    fn validate(&self, trust: &DeploymentTrustV1) -> Result<()> {
        require(
            self.schema_version == 1 && (1..=256).contains(&self.parameters.len()),
            "epoch schedule must have version1 and one to256 public epoch parameters",
        )?;
        trust.validate(self.network_id)?;
        require(
            self.transaction_fee_maximum > Quantity::zero(),
            "epoch maintenance requires a positive explicit transaction fee cap",
        )?;
        let mut expected = trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect::<Vec<_>>();
        expected.sort();
        let mut previous: Option<u64> = None;
        for parameter in &self.parameters {
            let roster = typed_parameter(parameter)?.roster;
            require(
                roster.network_id == self.network_id && roster.epoch > 0,
                "scheduled roster has another network or reserved epoch0",
            )?;
            if let Some(previous) = previous {
                require(
                    Some(roster.epoch) == previous.checked_add(1),
                    "schedule epochs must be contiguous and strictly increasing",
                )?;
            }
            require(
                roster
                    .validators
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect::<Vec<_>>()
                    == expected,
                "epoch maintenance supports only the exact selected fixed four validators; reprovision membership changes explicitly",
            )?;
            iroha_core::zk::kagemusha_v1_recursion::validate_kagemusha_mint_finality_roster_keys_v1(&roster)
                .map_err(|error| eyre!("invalid native scheduled mint public keys: {error:?}"))?;
            previous = Some(roster.epoch);
        }
        Ok(())
    }

    fn parameter(&self, target: u64) -> Result<&Parameter> {
        self.parameters.iter().find(|parameter| typed_parameter(parameter).is_ok_and(|value| value.roster.epoch == target))
            .ok_or_else(|| eyre!("public epoch schedule exhausted: provision target epoch {target} before its boundary"))
    }
}

fn epoch_window(
    current_epoch: u64,
    current_height: u64,
    epoch_end: u64,
    target: u64,
) -> Result<()> {
    require(
        current_epoch.checked_add(1) == Some(target),
        "target epoch must succeed the actual authenticated current epoch; a next_epoch_snapshot is not an epoch transition",
    )?;
    require(
        current_height
            .checked_add(3)
            .is_some_and(|carrier| carrier < epoch_end),
        "insufficient epoch height budget: QueuePlan admission, anchor and merge must commit the next roster before the boundary; stage immediately after the epoch transition",
    )
}

fn require_next_epoch(height: &HeightContext, target: u64) -> Result<()> {
    epoch_window(height.epoch, height.height, height.epoch_end_height, target)
}

fn require_carrier_epoch(context: &HeightContext, plan: &EpochPlanV1, applied: u64) -> Result<()> {
    require(
        context.epoch.checked_add(1) == Some(plan.target_epoch)
            && context.epoch_end_height == plan.epoch_end_height
            && context.height == applied
            && applied < plan.epoch_end_height,
        "maintenance transaction did not commit in its original predecessor epoch before the required boundary",
    )
}

fn validate_staking_observation(
    records: &json::Value,
    expected: &BTreeSet<iroha_model_base::peer::PeerId>,
    next_start: u64,
    minimum: &Quantity,
) -> Result<()> {
    let items = records
        .get("items")
        .and_then(json::Value::as_array)
        .ok_or_else(|| eyre!("invalid public staking census"))?;
    require(
        records.get("lane_id").and_then(json::Value::as_u64) == Some(0) && items.len() == 4,
        "maintenance requires exactly four core-lane staking records",
    )?;
    let mut observed = BTreeSet::new();
    for item in items {
        require(
            item.get("authority_source").and_then(json::Value::as_str) == Some("staking"),
            "manifest fallback is not native staking eligibility",
        )?;
        let peer = item
            .get("peer_id")
            .and_then(json::Value::as_str)
            .ok_or_else(|| eyre!("staking peer identity missing"))?
            .parse::<iroha_model_base::peer::PeerId>()?;
        let activation = item
            .get("activation_height")
            .and_then(json::Value::as_u64)
            .ok_or_else(|| eyre!("staking activation height missing"))?;
        let deactivation = match item.get("deactivation_height") {
            Some(json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .ok_or_else(|| eyre!("invalid staking deactivation height"))?,
            ),
            None => eyre::bail!("staking deactivation bound missing"),
        };
        let stake = item
            .get("self_stake")
            .and_then(json::Value::as_str)
            .ok_or_else(|| eyre!("staking self bond missing"))?
            .parse::<Quantity>()?;
        require(
            expected.contains(&peer)
                && observed.insert(peer)
                && activation <= next_start
                && deactivation.is_none_or(|end| next_start < end)
                && &stake >= minimum,
            "observed fixed-four validator lacks retained next-epoch tenure or minimum self bond",
        )?;
    }
    require(
        &observed == expected,
        "staking census changed the fixed-four policy",
    )
}

fn operation_name(network: NetworkId, target: u64) -> String {
    format!("epoch-{network}-{target}")
}

fn now_ms() -> Result<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

fn require_authority_fee_selection(intent: &FeePaymentIntent) -> Result<()> {
    require(
        intent == &FeePaymentIntent::authority(Vec::new(), None),
        "epoch maintenance requires explicit --fee-payer authority; sponsor or other global fee selectors conflict with the public schedule",
    )
}

// Exhaustion owned by this controller is never a retryable socket timeout.
fn require_epoch_budget(deadline: Instant, stage: &str) -> Result<()> {
    require(
        Instant::now() < deadline,
        &format!(
            "epoch maintenance owned deadline elapsed during {stage}; reconcile any retained dispatch with status"
        ),
    )
}

fn retained_write_read_deadline(
    common: &CommonArgs,
    network: NetworkId,
    target: u64,
    invocation: Instant,
) -> Result<Instant> {
    let path = common.journal_dir.join(operation_name(network, target));
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(invocation),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let journal = Journal::open(&path, false)?;
    let plan: EpochPlanV1 = journal.read_json("plan.json")?;
    require(
        plan.network_id == network && plan.target_epoch == target,
        "retained epoch deadline belongs to another operation",
    )?;
    original_completion_deadline(plan.expires_at_ms, now_ms()?, Instant::now(), invocation)
}

fn original_completion_deadline(
    expires_at_ms: u64,
    observed_now_ms: u64,
    now: Instant,
    invocation: Instant,
) -> Result<Instant> {
    let remaining = expires_at_ms.checked_sub(observed_now_ms).filter(|value| *value > 0)
        .ok_or_else(|| eyre!("epoch operation exhausted its original deadline after dispatch; reconcile the retained hash with status"))?;
    let remaining = Duration::from_millis(remaining);
    if remaining >= invocation.saturating_duration_since(now) {
        return Ok(invocation);
    }
    now.checked_add(remaining)
        .ok_or_else(|| eyre!("epoch deadline overflow"))
}

fn fee_matches(schedule: &ScheduleV1, quote: &FeeQuoteResponse) -> Result<()> {
    let FeePaymentIntent::Authority(intent) = &quote.intent else {
        eyre::bail!("epoch maintenance must pay from its authorized ledger owner");
    };
    require(
        intent.gas_limit.is_none() && !quote.components.is_empty(),
        "invalid epoch instruction fee quote",
    )?;
    let mut total = Quantity::zero();
    for component in &quote.components {
        require(
            component.kind == FeeChargeKind::Nexus
                && component.asset_definition_id == schedule.payment_asset,
            "epoch fee quote changed its allowed fee kind or asset",
        )?;
        total = total.checked_add(&component.max_amount)?;
    }
    require(
        total <= schedule.transaction_fee_maximum,
        "epoch transaction fee exceeds the explicit public schedule cap",
    )
}

fn verify_prepared(
    plan: &EpochPlanV1,
    schedule: &ScheduleV1,
    prepared: &PreparedV1,
) -> Result<SignedTransaction> {
    let wire = hex::decode(&prepared.signed_transaction_wire_hex)?;
    require(
        !wire.is_empty()
            && wire.len() <= MAX_BYTES
            && hex::encode(&wire) == prepared.signed_transaction_wire_hex,
        "invalid retained epoch transaction wire",
    )?;
    let transaction = SignedTransaction::decode_all_versioned(&wire)?;
    transaction.verify_signature()?;
    let instructions: Vec<InstructionBox> = vec![SetParameter::new(plan.parameter.clone()).into()];
    require(
        prepared.schema_version == 1
            && prepared.operation_id == operation_name(plan.network_id, plan.target_epoch)
            && prepared.intent_sha256 == digest(&json::to_vec(plan)?)
            && prepared.phase == "mint-finality-next-epoch"
            && prepared.instructions == instructions
            && prepared.alias_plan.is_none()
            && prepared.transaction_hash == hex::encode(transaction.hash().as_ref())
            && transaction.encode_wire_v1()? == wire
            && transaction.network_id() == Some(&plan.network_id)
            && transaction.authority() == &plan.owner
            && transaction.instructions() == &Executable::from(instructions)
            && transaction.metadata().is_empty()
            && transaction.attachments().is_none()
            && transaction.multisig_signatures().is_none()
            && transaction.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced
            && transaction.fee_payment_intent() == &prepared.fee_quote.intent,
        "retained epoch transaction differs from its sole exact authorized parameter",
    )?;
    let expiry = transaction
        .creation_time()
        .checked_add(
            transaction
                .time_to_live()
                .ok_or_else(|| eyre!("epoch transaction omitted finite TTL"))?,
        )
        .ok_or_else(|| eyre!("epoch transaction lifetime overflow"))?;
    require(
        expiry.as_millis() <= u128::from(plan.expires_at_ms),
        "epoch transaction exceeds its original operation deadline",
    )?;
    prepared
        .fee_quote
        .validate_for_signed_payload(transaction.payload())
        .map_err(|error| eyre!(error))?;
    fee_matches(schedule, &prepared.fee_quote)?;
    Ok(transaction)
}

fn validate_plan(
    plan: &EpochPlanV1,
    schedule: &ScheduleV1,
    trust_sha256: &str,
    owner: &AccountId,
    target: u64,
    parameter: &Parameter,
) -> Result<()> {
    require(
        plan.schema_version == 1
            && plan.network_id == schedule.network_id
            && plan.target_epoch == target
            && plan.owner == *owner
            && plan.payment_asset == schedule.payment_asset
            && plan.transaction_fee_maximum == schedule.transaction_fee_maximum
            && plan.trust_sha256 == trust_sha256
            && &plan.parameter == parameter,
        "retained epoch plan differs from the explicit owner, network, schedule or trust",
    )
}

struct Runtime {
    schedule: ScheduleV1,
    trust: DeploymentTrustV1,
    clients: [Client; 4],
    observer: AuthenticatedHeightObserverV1,
    deadline: Instant,
}

impl Runtime {
    fn new<C: RunContext>(context: &C, args: &CommonArgs) -> Result<Self> {
        require(
            !context.input_instructions()
                && !context.output_instructions()
                && context.transaction_metadata().is_none(),
            "epoch maintenance cannot combine arbitrary instructions or metadata",
        )?;
        require(
            context.config().chain.to_string() == "fc56984b-2be7-431d-840e-21514d1883f0"
                && context.config().account_chain_discriminant == 369,
            "epoch maintenance requires the canonical Taira account profile",
        )?;
        require_authority_fee_selection(&context.transaction_fee_payment()?)?;
        let deadline = operation_deadline(args.timeout_ms)?;
        let schedule: ScheduleV1 = json::from_slice(&read_public_input(&args.schedule)?)?;
        let trust: DeploymentTrustV1 = json::from_slice(&read_public_input(&args.trust)?)?;
        schedule.validate(&trust)?;
        require(
            schedule.network_id == context.config().network_id,
            "configured owner belongs to another scheduled network",
        )?;
        let clients = trust
            .peers
            .iter()
            .map(|peer| {
                let mut config = context.config().clone();
                config.torii_api_url = peer.torii_origin.parse()?;
                let mut builder = Client::builder(config);
                builder.operator_key_pair = context.operator_key_pair().cloned();
                builder.build().map_err(Into::into)
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| eyre!("maintenance requires exactly four peer clients"))?;
        let observer = AuthenticatedHeightObserverV1::from_trust(&trust, schedule.network_id)?;
        Ok(Self {
            schedule,
            trust,
            clients,
            observer,
            deadline,
        })
    }

    fn checkpoint_until(&mut self, deadline: Instant) -> Result<VerifiedCommittedHeightV1> {
        loop {
            require_epoch_budget(deadline, "waiting for authenticated epoch state")?;
            let observation = self.observer.observe_current(&self.clients, 369, deadline);
            require_epoch_budget(deadline, "authenticated epoch observation")?;
            match observation? {
                HeightObservationV1::Verified(value) => return Ok(value),
                HeightObservationV1::Pending => {
                    std::thread::sleep(operation_poll_delay(deadline, Instant::now()))
                }
            }
        }
    }

    fn pause(&self) {
        std::thread::sleep(operation_poll_delay(self.deadline, Instant::now()));
    }

    fn write_client<C: RunContext>(
        &self,
        context: &C,
        deadline: Instant,
    ) -> Result<BlockingClient> {
        let client = BlockingClient::from_client(
            context
                .client_from_config()?
                .with_request_deadline(deadline),
        )?;
        client.refresh_capabilities()?;
        let owner = &context.config().account;
        require(
            client.client().get_account_read(owner)?.account_id == *owner,
            "maintenance owner identity changed",
        )?;
        let permissions =
            crate::account::list_effective_permissions(client.client(), owner, None, 0, None)?;
        require(
            permissions.iter().any(|permission| {
                permission.name() == "CanSetParameters" && permission.payload().get() == "null"
            }),
            "epoch maintenance requires a separately authorized CanSetParameters owner; HTTP operator identity alone is insufficient",
        )?;
        Ok(client)
    }

    fn fixed_four_observation(&self, current: &HeightContext, deadline: Instant) -> Result<()> {
        use iroha_data_model::parameter::system::SumeragiNposParameters;
        use iroha_model_base::topology::LaneId;
        let expected = self
            .trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect::<BTreeSet<_>>();
        let next_start = current
            .epoch_end_height
            .checked_add(1)
            .ok_or_else(|| eyre!("next epoch height overflow"))?;
        let mut reads = Vec::new();
        for client in &self.clients {
            let client = client.with_request_deadline(deadline);
            reads.push((|| -> Result<()> {
                let present = client.query(FindPeers).with_pagination(Pagination::new(NonZeroU64::new(5), 0)).execute_all()?;
                require(present.len() == 4 && present.into_iter().collect::<BTreeSet<_>>() == expected,
                    "fixed-four maintenance observed extra, missing or substituted registered peers")?;
                let parameters: Parameters = client.query_single(FindParameters)?;
                let npos = parameters.custom().get(&SumeragiNposParameters::parameter_id())
                    .and_then(SumeragiNposParameters::from_custom_parameter)
                    .ok_or_else(|| eyre!("maintenance requires valid committed NPoS parameters"))?;
                require(npos.max_validators == 4, "maintenance supports only the explicit fixed-four committee policy")?;
                let records = client.get_public_lane_validators(LaneId::SINGLE)?;
                validate_staking_observation(&records, &expected, next_start, &npos.min_self_bond)
            })());
        }
        let mut transport = None;
        for result in reads {
            if let Err(error) = result {
                if crate::taira::observation_transport_unavailable(&error) {
                    transport = Some(error);
                } else {
                    return Err(error);
                }
            }
        }
        if let Some(error) = transport {
            return Err(error);
        }
        // These are conservative fresh ledger observations, not a future-election
        // proof. Native boundary construction still authenticates the elected set.
        Ok(())
    }

    fn existing_parameter(&self, target: u64, deadline: Instant) -> Result<String> {
        let expected = self.schedule.parameter(target)?;
        let mut states = Vec::new();
        // Read all peers before deciding progress: a fixed error cannot be hidden by another peer's restart.
        for client in &self.clients {
            let result = (|| {
                let parameters: Parameters = client
                    .with_request_deadline(deadline)
                    .query_single(FindParameters)?;
                Ok(match parameters.custom().get(&NextRoster::parameter_id()) {
                    None => "missing",
                    Some(custom) if &Parameter::Custom(custom.clone()) == expected => "staged",
                    Some(custom) => {
                        let value = NextRoster::from_custom_parameter(custom)
                            .ok_or_else(|| eyre!("committed next-roster parameter is malformed"))?;
                        require(
                            value.roster.network_id == self.schedule.network_id,
                            "committed next-roster parameter has another network",
                        )?;
                        if value.roster.epoch < target {
                            "previous_epoch"
                        } else {
                            "conflict"
                        }
                    }
                })
            })();
            states.push(result);
        }
        let mut transport = None;
        let mut values = Vec::new();
        for result in states {
            match result {
                Ok(value) => values.push(value),
                Err(error) if crate::taira::observation_transport_unavailable(&error) => {
                    transport = Some(error)
                }
                Err(error) => return Err(error),
            }
        }
        if values.contains(&"conflict") {
            return Ok("conflict".into());
        }
        if let Some(error) = transport {
            return Err(error);
        }
        Ok(if values.iter().all(|value| *value == "staged") {
            "staged"
        } else {
            "missing"
        }
        .into())
    }

    fn finish(
        &self,
        journal: &Journal,
        plan: &EpochPlanV1,
        prepared: &PreparedV1,
        transaction: &SignedTransaction,
        height: &VerifiedCommittedHeightV1,
        persist: bool,
        deadline: Instant,
    ) -> Result<Option<CompletionV1>> {
        let mut reads = Vec::new();
        for client in &self.clients {
            reads.push(observe(
                &client.with_request_deadline(deadline),
                prepared,
                transaction,
            ));
        }
        let observations = collect_optional_reads(reads)?;
        let mut pending = observations.iter().any(Option::is_none);
        for observed in observations.iter().flatten() {
            require(
                observed.state != "failed",
                &format!(
                    "epoch {} transaction {} failed with its retained native status; inspect status, never replace it",
                    plan.target_epoch, prepared.transaction_hash
                ),
            )?;
        }
        let mut carrier_reads = Vec::new();
        for (client, observed) in self.clients.iter().zip(&observations) {
            let Some(observed) = observed else {
                continue;
            };
            if observed.state != "applied_verification_pending" {
                pending = true;
                continue;
            }
            carrier_reads.push((|| -> Result<Option<(CompletionV1, Vec<u8>)>> {
                let applied = matching_applied_height(
                    &prepared.transaction_hash,
                    &observed.global_status,
                    &observed.peer_status,
                )?
                .ok_or_else(|| eyre!("missing exact Applied epoch transaction"))?;
                if applied > height.committed_height().get() {
                    return Ok(None);
                }
                let proof = height
                    .proof_at(
                        NonZeroU64::new(applied).ok_or_else(|| eyre!("invalid zero carrier"))?,
                    )
                    .ok_or_else(|| eyre!("epoch carrier is outside the authenticated prefix"))?;
                require_carrier_epoch(&proof.finality_artifact.height_context, plan, applied)?;
                let details = observed
                    .committed
                    .as_ref()
                    .ok_or_else(|| eyre!("epoch Applied lacks native committed details"))?;
                require(
                    details.transaction.block_hash() == &proof.block_header.hash(),
                    "epoch transaction carrier differs from authenticated finality",
                )?;
                let wire = match client
                    .with_request_deadline(deadline)
                    .get_canonical_executed_block_wire(
                        NonZeroU64::new(applied).unwrap(),
                        &details.transaction,
                        &proof.finality_artifact.commit_qc.execution_commitment,
                    ) {
                    Ok(wire) => wire,
                    Err(error) => return Err(error),
                };
                let value = CompletionV1 {
                    schema_version: 1,
                    network_id: plan.network_id,
                    target_epoch: plan.target_epoch,
                    transaction_hash: prepared.transaction_hash.clone(),
                    applied_height: applied,
                    parameter_sha256: digest(&json::to_vec(&plan.parameter)?),
                    carrier_sha256: digest(&wire),
                };
                Ok(Some((value, wire)))
            })());
        }
        let mut receipt: Option<CompletionV1> = None;
        let mut carrier: Option<Vec<u8>> = None;
        for read in carrier_reads {
            match read {
                Ok(Some((value, wire))) => {
                    if let Some(prior) = &receipt {
                        require(
                            prior == &value && carrier.as_ref() == Some(&wire),
                            "validators disagree on exact epoch transaction carrier",
                        )?;
                    } else {
                        receipt = Some(value);
                        carrier = Some(wire);
                    }
                }
                Ok(None) => pending = true,
                Err(error) if crate::taira::observation_transport_unavailable(&error) => {
                    pending = true
                }
                Err(error) => return Err(error),
            }
        }
        if pending {
            return Ok(None);
        }
        require_epoch_budget(deadline, "complete exact epoch maintenance")?;
        let receipt = receipt.ok_or_else(|| eyre!("missing four-peer epoch completion"))?;
        let wire = carrier.unwrap();
        if let Some(existing) = journal.read_optional_bounded("carrier.nrt", 64 * 1024 * 1024)? {
            require(existing == wire, "retained epoch carrier changed")?;
        } else if persist {
            journal.install_bounded("carrier.nrt", &wire, 64 * 1024 * 1024)?;
        }
        // This file is a retained result, never an authority for skipping the fresh checks above.
        if let Some(existing) = journal.optional_json::<CompletionV1>("completion.json")? {
            require(
                existing == receipt,
                "retained epoch completion differs from fresh native verification",
            )?;
        } else if persist {
            journal.install_json("completion.json", &receipt)?;
        }
        require_epoch_budget(deadline, "return verified epoch maintenance")?;
        Ok(Some(receipt))
    }
}

fn context_at(height: &VerifiedCommittedHeightV1) -> Result<&HeightContext> {
    Ok(&height
        .proof_at(height.committed_height())
        .ok_or_else(|| eyre!("missing authenticated epoch context"))?
        .finality_artifact
        .height_context)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Preflight,
    Prepare,
    Apply,
    Status,
}

fn execute<C: RunContext>(
    runtime: &mut Runtime,
    context: &mut C,
    common: &CommonArgs,
    target: u64,
    action: Action,
    height: &VerifiedCommittedHeightV1,
) -> Result<Option<CompletionV1>> {
    let current = context_at(height)?;
    let parameter = runtime.schedule.parameter(target)?.clone();
    if action == Action::Preflight {
        require(
            current.epoch.checked_add(1) == Some(target),
            "preflight target differs from the actual current epoch successor",
        )?;
        runtime.fixed_four_observation(current, runtime.deadline)?;
        let state = runtime.existing_parameter(target, runtime.deadline)?;
        if state != "staged" {
            require_next_epoch(current, target)?;
        }
        context.print_data(&ProgressV1 {
            schema_version: 1,
            network_id: runtime.schedule.network_id,
            current_epoch: current.epoch,
            epoch_end_height: current.epoch_end_height,
            observed_height: height.committed_height().get(),
            target_epoch: target,
            state,
            transaction_hash: None,
        })?;
        return Ok(None);
    }
    let path = common
        .journal_dir
        .join(operation_name(runtime.schedule.network_id, target));
    let create = match fs::symlink_metadata(&path) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    };
    if create {
        require(
            action != Action::Status,
            "epoch has no retained preparation",
        )?;
        require_next_epoch(current, target)?;
    }
    let journal = Journal::open(&path, create)?;
    let expected_schedule = digest(&json::to_vec(&runtime.schedule)?);
    let expected_trust = digest(&json::to_vec(&runtime.trust)?);
    let plan = if create {
        let plan = EpochPlanV1 {
            schema_version: 1,
            network_id: runtime.schedule.network_id,
            target_epoch: target,
            owner: context.config().account.clone(),
            schedule_sha256: expected_schedule.clone(),
            payment_asset: runtime.schedule.payment_asset.clone(),
            transaction_fee_maximum: runtime.schedule.transaction_fee_maximum.clone(),
            trust_sha256: expected_trust.clone(),
            parameter: parameter.clone(),
            epoch_end_height: current.epoch_end_height,
            expires_at_ms: now_ms()?
                .checked_add(common.operation_timeout_ms)
                .ok_or_else(|| eyre!("epoch operation deadline overflow"))?,
        };
        journal.install_json("plan.json", &plan)?;
        plan
    } else {
        journal.read_json::<EpochPlanV1>("plan.json")?
    };
    validate_plan(
        &plan,
        &runtime.schedule,
        &expected_trust,
        &context.config().account,
        target,
        &parameter,
    )?;
    let mut prepared: Option<PreparedV1> = journal.optional_json("prepared.json")?;
    if prepared.is_none() && action != Action::Status {
        require_next_epoch(current, target)?;
        require(
            now_ms()? < plan.expires_at_ms,
            "epoch operation exhausted its original preparation deadline",
        )?;
        let deadline = original_completion_deadline(
            plan.expires_at_ms,
            now_ms()?,
            Instant::now(),
            runtime.deadline,
        )?;
        // Persist the original operation expiry before the first eligibility or parameter read.
        // A transient preflight failure resumes this same plan and cannot renew its budget.
        runtime.fixed_four_observation(current, deadline)?;
        let state = runtime.existing_parameter(target, deadline)?;
        require(
            state == "missing",
            "new epoch preparation requires missing previous-or-absent roster, not a conflict or unowned staged value",
        )?;
        require_epoch_budget(deadline, "original epoch preparation preflight")?;
        let client = runtime.write_client(context, deadline)?;
        let instructions: Vec<InstructionBox> = vec![SetParameter::new(parameter).into()];
        let (transaction, fee_quote) = crate::quote_and_sign_transaction_with_expiry(
            &client,
            Executable::from(instructions.clone()),
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
            plan.expires_at_ms,
        )?;
        require(
            fee_quote
                .observation
                .next_block_height
                .checked_add(2)
                .is_some_and(|carrier| carrier < plan.epoch_end_height),
            "native fee observation leaves insufficient epoch carrier budget",
        )?;
        let value = PreparedV1 {
            schema_version: 1,
            operation_id: operation_name(plan.network_id, target),
            intent_sha256: digest(&json::to_vec(&plan)?),
            phase: "mint-finality-next-epoch".into(),
            signed_transaction_wire_hex: hex::encode(transaction.encode_wire_v1()?),
            transaction_hash: hex::encode(transaction.hash().as_ref()),
            instructions,
            fee_quote,
            alias_plan: None,
        };
        verify_prepared(&plan, &runtime.schedule, &value)?;
        require(
            now_ms()? < plan.expires_at_ms,
            "epoch preparation exceeded its original deadline before retention",
        )?;
        journal.install_json("prepared.json", &value)?;
        prepared = Some(value);
    }
    let Some(prepared) = prepared else {
        return Ok(None);
    };
    let transaction = verify_prepared(&plan, &runtime.schedule, &prepared)?;
    let expected_claim = digest(&json::to_vec(&prepared)?);
    let claim: Option<String> = journal.optional_json("submitted.json")?;
    if let Some(claim) = &claim {
        require(
            claim == &expected_claim,
            "epoch dispatch claim changed its exact signed payload",
        )?;
    }
    if action == Action::Apply && claim.is_none() {
        // The fresh proof's actual epoch, not a height quotient or a future snapshot,
        // controls when replacing the single next-roster parameter is legitimate.
        let dispatch_deadline = original_completion_deadline(
            plan.expires_at_ms,
            now_ms()?,
            Instant::now(),
            runtime.deadline,
        )?;
        let fresh = runtime.checkpoint_until(dispatch_deadline)?;
        let fresh_context = context_at(&fresh)?;
        require_next_epoch(fresh_context, target)?;
        require(
            fresh_context.epoch_end_height == plan.epoch_end_height,
            "epoch boundary changed since preparation",
        )?;
        require(
            now_ms()? < plan.expires_at_ms,
            "retained epoch operation expired before dispatch; it will not be replaced",
        )?;
        runtime.fixed_four_observation(fresh_context, dispatch_deadline)?;
        let state = runtime.existing_parameter(target, dispatch_deadline)?;
        require(
            state == "missing",
            "next-roster changed or is unavailable before sole dispatch",
        )?;
        let client = runtime.write_client(context, dispatch_deadline)?;
        require_epoch_budget(dispatch_deadline, "epoch dispatch claim")?;
        require(
            now_ms()? < plan.expires_at_ms,
            "epoch original dispatch deadline elapsed before claim",
        )?;
        require(
            record_dispatch_claim(&journal, "submitted.json", &prepared)?,
            "epoch was already dispatched",
        )?;
        let outcome = client.submit_transaction(&transaction);
        if let Ok(hash) = &outcome {
            require(
                hash == &transaction.hash(),
                "epoch submit response changed the retained hash",
            )?;
        }
        journal.install_json(
            "submission-result.json",
            &SubmissionResultV1 {
                transaction_hash: prepared.transaction_hash.clone(),
                accepted: outcome.is_ok(),
                error: outcome.err().map(|error| format!("{error:#}")),
            },
        )?;
    }
    if action == Action::Prepare {
        context.print_data(&ProgressV1 {
            schema_version: 1,
            network_id: plan.network_id,
            current_epoch: current.epoch,
            epoch_end_height: plan.epoch_end_height,
            observed_height: height.committed_height().get(),
            target_epoch: target,
            state: "prepared".into(),
            transaction_hash: Some(prepared.transaction_hash.clone()),
        })?;
        return Ok(None);
    }
    let completion_deadline = if action == Action::Apply {
        original_completion_deadline(
            plan.expires_at_ms,
            now_ms()?,
            Instant::now(),
            runtime.deadline,
        )?
    } else {
        runtime.deadline
    };
    loop {
        let fresh;
        let checkpoint = if action == Action::Apply {
            fresh = runtime.checkpoint_until(completion_deadline)?;
            &fresh
        } else {
            height
        };
        let result = runtime.finish(
            &journal,
            &plan,
            &prepared,
            &transaction,
            checkpoint,
            action == Action::Apply,
            completion_deadline,
        );
        require_epoch_budget(completion_deadline, "original epoch completion budget")?;
        let result = result?;
        if let Some(receipt) = &result {
            context.print_data(receipt)?;
            return Ok(result);
        }
        if action == Action::Status {
            context.print_data(&ProgressV1 {
                schema_version: 1,
                network_id: plan.network_id,
                current_epoch: current.epoch,
                epoch_end_height: plan.epoch_end_height,
                observed_height: height.committed_height().get(),
                target_epoch: target,
                state: "pending".into(),
                transaction_hash: Some(prepared.transaction_hash.clone()),
            })?;
            return Ok(None);
        }
        require_epoch_budget(
            completion_deadline,
            "epoch operation original finality deadline; use status to reconcile the retained hash",
        )?;
        std::thread::sleep(operation_poll_delay(completion_deadline, Instant::now()));
    }
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
        match self {
            Self::Maintain(args) => maintain(context, args),
            command => {
                let (args, action) = match command {
                    Self::Preflight(args) => (args, Action::Preflight),
                    Self::Prepare(args) => (args, Action::Prepare),
                    Self::Apply(args) => (args, Action::Apply),
                    Self::Status(args) => (args, Action::Status),
                    Self::Maintain(_) => unreachable!(),
                };
                let mut runtime = Runtime::new(context, &args.common)?;
                let mut read_deadline = if action == Action::Apply || action == Action::Prepare {
                    retained_write_read_deadline(
                        &args.common,
                        runtime.schedule.network_id,
                        args.target_epoch,
                        runtime.deadline,
                    )?
                } else {
                    runtime.deadline
                };
                loop {
                    let height = runtime.checkpoint_until(read_deadline)?;
                    let result = execute(
                        &mut runtime,
                        context,
                        &args.common,
                        args.target_epoch,
                        action,
                        &height,
                    );
                    match result {
                        Ok(value) if value.is_some() || action != Action::Apply => return Ok(()),
                        Ok(_) => runtime.pause(),
                        Err(error)
                            if action == Action::Apply
                                && crate::taira::observation_transport_unavailable(&error) =>
                        {
                            read_deadline = retained_write_read_deadline(
                                &args.common,
                                runtime.schedule.network_id,
                                args.target_epoch,
                                runtime.deadline,
                            )?;
                            runtime.pause()
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
        }
    }
}

fn maintenance_action(claim_at_invocation_start: bool, historical_or_completed: bool) -> Action {
    if claim_at_invocation_start || historical_or_completed {
        Action::Status
    } else {
        Action::Apply
    }
}

fn maintain<C: RunContext>(context: &mut C, args: MaintainArgs) -> Result<()> {
    let mut runtime = Runtime::new(context, &args.common)?;
    runtime.schedule.parameter(args.stop_after_epoch)?;
    let resumed_claims = runtime
        .schedule
        .parameters
        .iter()
        .map(typed_parameter)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|value| {
            let epoch = value.roster.epoch;
            fs::symlink_metadata(
                args.common
                    .journal_dir
                    .join(operation_name(runtime.schedule.network_id, epoch))
                    .join("submitted.json"),
            )
            .is_ok()
            .then_some(epoch)
        })
        .collect::<BTreeSet<_>>();
    let mut verified = BTreeSet::new();
    let mut read_deadline = runtime.deadline;
    for parameter in &runtime.schedule.parameters {
        let epoch = typed_parameter(parameter)?.roster.epoch;
        if !resumed_claims.contains(&epoch) {
            read_deadline = read_deadline.min(retained_write_read_deadline(
                &args.common,
                runtime.schedule.network_id,
                epoch,
                runtime.deadline,
            )?);
        }
    }
    loop {
        let height = runtime.checkpoint_until(read_deadline)?;
        let current = context_at(&height)?;
        let target = current
            .epoch
            .checked_add(1)
            .ok_or_else(|| eyre!("epoch overflow"))?;
        // Resolve any older retained in-flight operation before scheduling another.
        let mut next = target;
        for parameter in &runtime.schedule.parameters {
            let epoch = typed_parameter(parameter)?.roster.epoch;
            if epoch > target || epoch > args.stop_after_epoch {
                break;
            }
            if !verified.contains(&epoch)
                && args
                    .common
                    .journal_dir
                    .join(operation_name(runtime.schedule.network_id, epoch))
                    .exists()
            {
                next = epoch;
                break;
            }
        }
        require(
            next <= args.stop_after_epoch,
            "requested maintenance stop epoch passed without its retained verified completion",
        )?;
        if verified.contains(&next) {
            runtime.pause();
            continue;
        }
        let completed_path = args
            .common
            .journal_dir
            .join(operation_name(runtime.schedule.network_id, next))
            .join("completion.json");
        let historical_claim = next < target
            && fs::symlink_metadata(completed_path.with_file_name("submitted.json")).is_ok();
        let action = maintenance_action(
            resumed_claims.contains(&next),
            historical_claim || fs::symlink_metadata(&completed_path).is_ok(),
        );
        match execute(&mut runtime, context, &args.common, next, action, &height) {
            Ok(Some(_)) => {
                read_deadline = runtime.deadline;
                verified.insert(next);
                if next == args.stop_after_epoch {
                    return Ok(());
                }
            }
            Ok(None) => {}
            Err(error) if crate::taira::observation_transport_unavailable(&error) => {
                read_deadline = if action == Action::Apply {
                    retained_write_read_deadline(
                        &args.common,
                        runtime.schedule.network_id,
                        next,
                        runtime.deadline,
                    )?
                } else {
                    runtime.deadline
                };
            }
            Err(error) => return Err(error),
        }
        runtime.pause();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        isi::kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityEpochRosterV1},
        nexus::FeeDebitSource,
        transaction::{TransactionBuilder, signed::FeeChargeLimit},
    };
    use iroha_model_base::{peer::PeerId, topology::DataSpaceId};
    use iroha_torii_shared::{FeeQuoteComponent, FeeQuoteDecision, FeeQuoteObservation};

    fn schedule() -> (ScheduleV1, DeploymentTrustV1, KeyPair) {
        let trust = finality::test_trust();
        let network = finality::test_network_id();
        let mut peers = trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect::<Vec<_>>();
        peers.sort();
        let parameters = (1..=2).map(|epoch| {
            let validators = peers.iter().enumerate().map(|(index, peer)| {
                iroha_core::zk::kagemusha_v1_recursion::derive_kagemusha_mint_finality_validator_keys_v1(
                    &[u8::try_from(index + 20).unwrap(); 32], epoch, peer.clone()).unwrap()
            }).collect();
            Parameter::Custom(NextRoster { roster: KagemushaMintFinalityEpochRosterV1 {
                version: KAGEMUSHA_CHAIN_VERSION_V1, network_id: network, epoch, validators,
            }}.into_custom_parameter())
        }).collect();
        (
            ScheduleV1 {
                schema_version: 1,
                network_id: network,
                parameters,
                payment_asset: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
                transaction_fee_maximum: Quantity::from(2_u32),
            },
            trust,
            KeyPair::try_from_seed(vec![121; 32], Algorithm::Ed25519).unwrap(),
        )
    }

    fn prepared(
        schedule: &ScheduleV1,
        trust: &DeploymentTrustV1,
        key: &KeyPair,
    ) -> (EpochPlanV1, PreparedV1) {
        let plan = EpochPlanV1 {
            schema_version: 1,
            network_id: schedule.network_id,
            target_epoch: 1,
            owner: AccountId::new(key.public_key().clone()),
            schedule_sha256: digest(&json::to_vec(schedule).unwrap()),
            payment_asset: schedule.payment_asset.clone(),
            transaction_fee_maximum: schedule.transaction_fee_maximum.clone(),
            trust_sha256: digest(&json::to_vec(trust).unwrap()),
            parameter: schedule.parameter(1).unwrap().clone(),
            expires_at_ms: now_ms().unwrap() + 180_000,
            epoch_end_height: 11,
        };
        let instructions: Vec<InstructionBox> =
            vec![SetParameter::new(plan.parameter.clone()).into()];
        let intent = FeePaymentIntent::authority(
            vec![FeeChargeLimit::new(
                FeeChargeKind::Nexus,
                schedule.payment_asset.clone(),
                Quantity::from(1_u32),
            )],
            None,
        );
        let mut builder =
            TransactionBuilder::new(plan.network_id, plan.owner.clone(), intent.clone())
                .with_instructions(instructions.clone())
                .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced);
        builder.set_ttl(Duration::from_secs(60));
        let transaction = builder.try_sign(key.private_key()).unwrap();
        let fee_quote = FeeQuoteResponse {
            intent,
            observation: FeeQuoteObservation {
                ledger_time_ms: 1,
                next_block_height: 8,
                route_dataspace_id: DataSpaceId::UNIVERSAL,
            },
            components: vec![FeeQuoteComponent {
                kind: FeeChargeKind::Nexus,
                asset_definition_id: schedule.payment_asset.clone(),
                max_amount: Quantity::from(1_u32),
            }],
            capacities: Vec::new(),
            decision: FeeQuoteDecision::Accepted {
                debit_source: FeeDebitSource::Account(plan.owner.clone()),
                program_revision: None,
            },
        };
        let prepared = PreparedV1 {
            schema_version: 1,
            operation_id: operation_name(plan.network_id, 1),
            intent_sha256: digest(&json::to_vec(&plan).unwrap()),
            phase: "mint-finality-next-epoch".into(),
            signed_transaction_wire_hex: hex::encode(transaction.encode_wire_v1().unwrap()),
            transaction_hash: hex::encode(transaction.hash().as_ref()),
            instructions,
            fee_quote,
            alias_plan: None,
        };
        (plan, prepared)
    }

    #[test]
    fn epoch_maintenance_schedule_rejects_wrong_epoch_network_and_membership() {
        let (schedule, trust, _) = schedule();
        schedule.validate(&trust).unwrap();
        let mut wrong = schedule.clone();
        wrong.parameters[1] = wrong.parameters[0].clone();
        assert!(wrong.validate(&trust).is_err());
        let mut wrong = schedule.clone();
        let mut value = typed_parameter(&wrong.parameters[0]).unwrap();
        value.roster.epoch = 0;
        wrong.parameters[0] = Parameter::Custom(value.into_custom_parameter());
        assert!(wrong.validate(&trust).is_err());
        let mut wrong = schedule.clone();
        let mut value = typed_parameter(&wrong.parameters[0]).unwrap();
        value.roster.network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"different network")),
        );
        wrong.parameters[0] = Parameter::Custom(value.into_custom_parameter());
        assert!(wrong.validate(&trust).is_err());
        let mut wrong = schedule.clone();
        let mut value = typed_parameter(&wrong.parameters[0]).unwrap();
        value.roster.validators.pop();
        wrong.parameters[0] = Parameter::Custom(value.into_custom_parameter());
        assert!(wrong.validate(&trust).is_err());
        assert!(
            schedule
                .parameter(3)
                .unwrap_err()
                .to_string()
                .contains("exhausted")
        );
        let mut value = json::to_value(&schedule).unwrap();
        value.as_object_mut().unwrap().insert(
            "private_seed".into(),
            json::Value::String("forbidden".into()),
        );
        assert!(json::from_value::<ScheduleV1>(value).is_err());
    }

    #[test]
    fn epoch_maintenance_waits_for_actual_epoch_and_preserves_carrier_deadline() {
        assert!(epoch_window(0, 7, 11, 1).is_ok());
        assert!(epoch_window(0, 8, 11, 1).is_err());
        assert!(epoch_window(0, 10, 11, 1).is_err());
        assert!(
            epoch_window(0, 11, 11, 2).is_err(),
            "a boundary snapshot still belongs to epoch0"
        );
        assert!(epoch_window(1, 12, 22, 2).is_ok());
        assert!(epoch_window(1, 12, 22, 3).is_err());
        assert!(epoch_window(u64::MAX, u64::MAX, u64::MAX, 1).is_err());
    }

    #[test]
    fn epoch_maintenance_preparation_binds_single_parameter_fee_and_original_lifetime() {
        let (schedule, trust, key) = schedule();
        let (plan, value) = prepared(&schedule, &trust, &key);
        verify_prepared(&plan, &schedule, &value).unwrap();
        require_authority_fee_selection(&FeePaymentIntent::authority(Vec::new(), None)).unwrap();
        assert!(
            require_authority_fee_selection(&value.fee_quote.intent).is_err(),
            "global caps cannot silently replace the schedule cap"
        );
        let sponsor = iroha_data_model::nexus::FeeSponsorProgramId::new(
            plan.owner.clone(),
            "maintenance".parse().unwrap(),
        );
        assert!(
            require_authority_fee_selection(&FeePaymentIntent::sponsor(
                sponsor,
                1,
                Vec::new(),
                None
            ))
            .is_err()
        );
        for mutate in [0, 1, 2, 3, 4] {
            let mut wrong = value.clone();
            match mutate {
                0 => wrong
                    .instructions
                    .push(SetParameter::new(schedule.parameters[1].clone()).into()),
                1 => wrong.transaction_hash.push('0'),
                2 => wrong.intent_sha256 = "0".repeat(64),
                3 => wrong.phase = "catalog".into(),
                _ => wrong.fee_quote.components[0].max_amount = Quantity::from(3_u32),
            }
            assert!(verify_prepared(&plan, &schedule, &wrong).is_err());
        }
        let mut early = plan.clone();
        early.expires_at_ms = 1;
        let mut changed = value.clone();
        changed.intent_sha256 = digest(&json::to_vec(&early).unwrap());
        assert!(verify_prepared(&early, &schedule, &changed).is_err());
        let wire = hex::decode(&value.signed_transaction_wire_hex).unwrap();
        let transaction = SignedTransaction::decode_all_versioned(&wire).unwrap();
        let ordinary = TransactionBuilder::new(
            plan.network_id,
            plan.owner.clone(),
            value.fee_quote.intent.clone(),
        )
        .with_instructions(value.instructions.clone())
        .with_admission_intent(TransactionAdmissionIntent::Ordinary)
        .try_sign(key.private_key())
        .unwrap();
        changed = value.clone();
        changed.signed_transaction_wire_hex = hex::encode(ordinary.encode_wire_v1().unwrap());
        changed.transaction_hash = hex::encode(ordinary.hash().as_ref());
        assert!(verify_prepared(&plan, &schedule, &changed).is_err());
        assert_eq!(
            transaction.instructions(),
            &Executable::from(value.instructions.clone())
        );
    }

    #[test]
    fn epoch_maintenance_journal_preserves_one_dispatch_across_schedule_renewal() {
        let (full, trust, key) = schedule();
        let mut first = full.clone();
        first.parameters.truncate(1);
        let (plan, value) = prepared(&first, &trust, &key);
        assert_ne!(plan.schedule_sha256, digest(&json::to_vec(&full).unwrap()));
        validate_plan(
            &plan,
            &full,
            &plan.trust_sha256,
            &plan.owner,
            1,
            full.parameter(1).unwrap(),
        )
        .unwrap();
        let mut changed = full.clone();
        changed.transaction_fee_maximum = Quantity::from(3_u32);
        assert!(
            validate_plan(
                &plan,
                &changed,
                &plan.trust_sha256,
                &plan.owner,
                1,
                changed.parameter(1).unwrap()
            )
            .is_err()
        );
        assert!(
            validate_plan(
                &plan,
                &full,
                "changed trust",
                &plan.owner,
                1,
                full.parameter(1).unwrap()
            )
            .is_err()
        );
        let root = tempfile::tempdir().unwrap();
        fs::set_permissions(
            root.path(),
            std::os::unix::fs::PermissionsExt::from_mode(0o700),
        )
        .unwrap();
        let path = root.path().join(operation_name(plan.network_id, 1));
        let mut dispatches = 0;
        {
            let journal = Journal::open(&path, true).unwrap();
            journal.install_json("plan.json", &plan).unwrap();
            journal.install_json("prepared.json", &value).unwrap();
            if record_dispatch_claim(&journal, "submitted.json", &value).unwrap() {
                dispatches += 1;
            }
        }
        let journal = Journal::open(&path, false).unwrap();
        let retained: PreparedV1 = journal.read_json("prepared.json").unwrap();
        verify_prepared(&plan, &full, &retained).unwrap();
        if record_dispatch_claim(&journal, "submitted.json", &retained).unwrap() {
            dispatches += 1;
        }
        assert_eq!(
            dispatches, 1,
            "renewal or restart cannot cause another POST"
        );
        assert!(
            matches!(maintenance_action(true, false), Action::Status),
            "even same-epoch claims pre-existing at invocation start reconcile read-only"
        );
        assert!(
            matches!(maintenance_action(false, false), Action::Apply),
            "a fresh claim retains original write/finality deadline"
        );
        let instant = Instant::now();
        let owned = require_epoch_budget(instant, "native test exact boundary").unwrap_err();
        assert!(
            !crate::taira::observation_transport_unavailable(&owned),
            "owned operation deadline must never be retried as a network disconnect"
        );
        let socket: eyre::Report = std::io::Error::from(std::io::ErrorKind::TimedOut).into();
        assert!(
            crate::taira::observation_transport_unavailable(&socket),
            "real read-side timeout remains bounded progress"
        );
        let invocation = instant + Duration::from_secs(300);
        assert!(original_completion_deadline(100, 100, instant, invocation).is_err());
        assert!(original_completion_deadline(100, 101, instant, invocation).is_err());
        assert_eq!(
            original_completion_deadline(100, 99, instant, invocation).unwrap(),
            instant + Duration::from_millis(1)
        );
        assert_eq!(
            original_completion_deadline(u64::MAX, 1, instant, instant).unwrap(),
            instant
        );

        assert!(
            journal.read_optional("completion.json").unwrap().is_none(),
            "a dispatch is not completion"
        );
    }

    #[test]
    fn epoch_maintenance_staking_preflight_rejects_fallback_and_changed_tenure() {
        let connection: eyre::Report =
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into();
        let ordered = collect_optional_reads(vec![Ok(1), Err(connection), Ok(3)]).unwrap();
        assert_eq!(
            ordered,
            vec![Some(1), None, Some(3)],
            "peer positions survive disconnects"
        );
        let connection: eyre::Report =
            std::io::Error::from(std::io::ErrorKind::ConnectionRefused).into();
        let error = collect_optional_reads::<u8>(vec![
            Err(connection),
            Err(eyre!("fixed bound carrier mismatch")),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("fixed bound carrier mismatch"));
        let (_, trust, _) = schedule();
        let expected: BTreeSet<PeerId> = trust
            .peers
            .iter()
            .map(|peer| peer.peer_id.clone())
            .collect();
        let items = expected.iter().map(|peer| {
            let peer_id = peer.to_string();
            norito::json!({"authority_source":"staking", "peer_id":peer_id, "activation_height":1,
                "deactivation_height":null, "self_stake":"5"})
        }).collect::<Vec<_>>();
        let records = norito::json!({"lane_id":0,"items":items});
        validate_staking_observation(&records, &expected, 12, &Quantity::from(1_u32)).unwrap();
        for (field, replacement) in [
            ("authority_source", json::Value::String("manifest".into())),
            ("activation_height", json::Value::from(13_u64)),
            ("deactivation_height", json::Value::from(12_u64)),
            ("self_stake", json::Value::String("0".into())),
        ] {
            let mut wrong = records.clone();
            wrong
                .as_object_mut()
                .unwrap()
                .get_mut("items")
                .unwrap()
                .as_array_mut()
                .unwrap()[0]
                .as_object_mut()
                .unwrap()
                .insert(field.into(), replacement);
            assert!(
                validate_staking_observation(&wrong, &expected, 12, &Quantity::from(1_u32))
                    .is_err()
            );
        }
    }
}
