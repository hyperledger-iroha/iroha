//! Small owner and source-review helpers kept outside the coordinator reducer.

use super::ProductionLifecycleOwnerV1;

/// Opaque proof that owner-open installed one exact recovered Decision Apply carrier.
///
/// Only the complete registry/coordinator census below can mint this linear
/// permit. Pending-tip replay consumes it when promoting authenticated
/// Decision recovery from its primitive Fetch witness directly to Apply.
#[must_use = "the recovered Apply carrier permit must enter pending-tip installation"]
pub(in crate::sumeragi) struct RecoveredPendingKuraApplyCarrierPermitV1 {
    _linearity: RecoveredPendingKuraApplyCarrierPermitLinearityV1,
}
struct RecoveredPendingKuraApplyCarrierPermitLinearityV1;
impl Drop for RecoveredPendingKuraApplyCarrierPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredPendingKuraApplyCarrierPermitV1 {
    /// Consume the opaque owner proof into the formal startup projection.
    pub(in crate::sumeragi) fn consume_for_executor(self) -> bool {
        true
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Attach interrupted-tip provenance to the recovered Decision Apply startup.
    ///
    /// Only the pending-Kura authenticated factory calls this after owner-open
    /// reconstructed the exact Ready Apply carrier from the move-only Decision
    /// Fetch. The inert replay remains nested in adapter startup and cannot
    /// become independently scheduled lifecycle work.
    pub(in crate::sumeragi) fn with_pending_kura_apply_replay(
        mut self,
        replay: crate::sumeragi::v2::RecoveredPendingKuraApplyReplayV1,
    ) -> Result<Self, &'static str> {
        if !self
            .registry
            .exactly_covers_recovered_decision_apply_ready_work(&self.coordinator)
        {
            return Err(
                "pending Kura startup did not reconstruct the exact recovered Decision Apply carrier",
            );
        }
        let apply_carrier = RecoveredPendingKuraApplyCarrierPermitV1 {
            _linearity: RecoveredPendingKuraApplyCarrierPermitLinearityV1,
        };
        let replay = replay.bind_recovered_apply_carrier(apply_carrier);
        let startup = self
            .adapter_startup
            .take()
            .expect("recovered Apply pending Kura owner retains adapter startup");
        self.adapter_startup = Some(startup.with_pending_kura_apply_replay(replay));
        Ok(self)
    }
}

#[cfg(test)]
/// Reconstruct the ledger source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_lifecycle_ledger_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let tests = include_str!("v2_lifecycle_ledger_tests.rs")
                .replacen(
                    "        include!(\"v2_lifecycle_ledger_tests_durable_recovery_01.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_durable_recovery_01.rs"),
                    1,
                )
                .replacen(
                    "        include!(\"v2_lifecycle_ledger_tests_durable_recovery_02.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_durable_recovery_02.rs"),
                    1,
                )
                .replacen(
                    "    include!(\"v2_lifecycle_ledger_tests_frame_and_store.rs\");\n",
                    include_str!("v2_lifecycle_ledger_tests_frame_and_store.rs"),
                    1,
                );
            include_str!("v2_lifecycle_ledger.rs")
                .replacen(
                    "include!(\"v2_lifecycle_ledger_operations.rs\");\n",
                    include_str!("v2_lifecycle_ledger_operations.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_ledger_store.rs\");\n",
                    include_str!("v2_lifecycle_ledger_store.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_ledger_tests.rs\");\n",
                    tests.as_str(),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the registry source exactly as Rust expands its reviewed provider.
pub(crate) fn reviewed_lifecycle_work_registry_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            let recovered_wal =
                include_str!("v2_lifecycle_work_registry_recovered_wal.rs").replacen(
                    "include!(\"v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs\");\n",
                    include_str!(
                        "v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs"
                    ),
                    1,
                );
            let recovery_registry_impl =
                include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs")
                    .replacen(
                        "include!(\"v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs\");\n",
                        include_str!(
                            "v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs"
                        ),
                        1,
                    )
                    .replacen(
                        "include!(\"v2_lifecycle_work_registry_validate_completion_impl.rs\");\n",
                        include_str!("v2_lifecycle_work_registry_validate_completion_impl.rs"),
                        1,
                    )
                    .replacen(
                        "include!(\"v2_lifecycle_work_registry_access_impl.rs\");\n",
                        include_str!("v2_lifecycle_work_registry_access_impl.rs"),
                        1,
                    )
                    .replacen(
                        "include!(\"v2_lifecycle_work_registry_validate_recovery_execution_impl.rs\");\n",
                        include_str!(
                            "v2_lifecycle_work_registry_validate_recovery_execution_impl.rs"
                        ),
                        1,
                    );
            let recovery = include_str!("v2_lifecycle_work_registry_validate_recovery.rs")
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery_registry_impl.rs\");\n",
                    recovery_registry_impl.as_str(),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery_parent.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_validate_recovery_parent.rs"),
                    1,
                );
            include_str!("v2_lifecycle_work_registry.rs")
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_pre_admission.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_pre_admission.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_live_wal_sign.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_live_wal_sign.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_output.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_output.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_live_validate_children.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_live_validate_children.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_recovered_wal.rs\");\n",
                    recovered_wal.as_str(),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_recovery.rs\");\n",
                    recovery.as_str(),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_execution.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_validate_execution.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_lifecycle_work_registry_validate_sidecar.rs\");\n",
                    include_str!("v2_lifecycle_work_registry_validate_sidecar.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the adapter source exactly as Rust expands its reviewed provider.
pub(crate) fn reviewed_v2_adapter_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2.rs")
                .replacen(
                    "include!(\"v2_authenticated_recovered_adapter_startup_impl.rs\");\n",
                    include_str!("v2_authenticated_recovered_adapter_startup_impl.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_verified_height_context_recovered_output_auth.rs\");\n",
                    include_str!("v2_verified_height_context_recovered_output_auth.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_ready_durable_validate_adapter_preview.rs\");\n",
                    include_str!("v2_ready_durable_validate_adapter_preview.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_recovered_lifecycle_sign_completion.rs\");\n",
                    include_str!("v2_recovered_lifecycle_sign_completion.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_adapter_equivocation_evidence.rs\");\n",
                    include_str!("v2_adapter_equivocation_evidence.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the effect executor source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_v2_effects_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2_effects.rs")
                .replacen(
                    "include!(\"v2_effects_recovered_lifecycle_output_service.rs\");\n",
                    include_str!("v2_effects_recovered_lifecycle_output_service.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_effects_lifecycle_admission_settlement.rs\");\n",
                    include_str!("v2_effects_lifecycle_admission_settlement.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the runtime source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_v2_runtime_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2_runtime.rs")
                .replacen(
                    "include!(\"v2_runtime_durable_recovery_pending.rs\");\n",
                    include_str!("v2_runtime_durable_recovery_pending.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_runtime_effect_ownership_core_impl.rs\");\n",
                    include_str!("v2_runtime_effect_ownership_core_impl.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_runtime_effect_ownership_rebind_impl.rs\");\n",
                    include_str!("v2_runtime_effect_ownership_rebind_impl.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_runtime_ready_validate_publication.rs\");\n",
                    include_str!("v2_runtime_ready_validate_publication.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
/// Reconstruct the worker source exactly as Rust expands its reviewed providers.
pub(crate) fn reviewed_v2_worker_source_for_test() -> &'static str {
    static SOURCE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SOURCE
        .get_or_init(|| {
            include_str!("v2_worker.rs")
                .replacen(
                    "include!(\"v2_worker_completion.rs\");\n",
                    include_str!("v2_worker_completion.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_worker_io_execution.rs\");\n",
                    include_str!("v2_worker_io_execution.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_worker_exact_output.rs\");\n",
                    include_str!("v2_worker_exact_output.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_worker_services.rs\");\n",
                    include_str!("v2_worker_services.rs"),
                    1,
                )
                .replacen(
                    "include!(\"v2_worker_services_impl.rs\");\n",
                    include_str!("v2_worker_services_impl.rs"),
                    1,
                )
        })
        .as_str()
}

#[cfg(test)]
const SOURCE_CONTRACT_ASSET: &str = include_str!("source_contracts_v1.txt");

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SourceId {
    Adapter,
    Authority,
    BodyPipeline,
    BodyPipelineTests,
    BodyStore,
    CanonicalRecoveryIngress,
    CertifiedServeStore,
    ConcreteAdmission,
    Coordinator,
    CoordinatorSupport,
    Effects,
    KuraTerminalOutcomes,
    LaneWork,
    Launch,
    Ledger,
    LifecycleOpen,
    LifecycleRecovery,
    PendingKuraRecovery,
    PendingLifecycle,
    Preactivation,
    Projection,
    Registry,
    RegistryRecovery,
    RegistryRecoveryImpl,
    ReplayAuthority,
    ReplayAuthorityBase,
    ReplayAuthorityCertifiedBody,
    Runner,
    RunnerHeightDriver,
    RunnerOuterCursor,
    Runtime,
    SchedulerInputs,
    Schema,
    Selector,
    Settlement,
    TurnDriver,
    WalRecovery,
    Worker,
}

#[cfg(test)]
impl SourceId {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "adapter" => Self::Adapter,
            "authority" => Self::Authority,
            "body_pipeline" => Self::BodyPipeline,
            "body_pipeline_tests" => Self::BodyPipelineTests,
            "body_store" => Self::BodyStore,
            "canonical_recovery_ingress" => Self::CanonicalRecoveryIngress,
            "certified_serve_store" => Self::CertifiedServeStore,
            "concrete_admission" => Self::ConcreteAdmission,
            "coordinator" => Self::Coordinator,
            "coordinator_support" => Self::CoordinatorSupport,
            "effects" => Self::Effects,
            "kura_terminal_outcomes" => Self::KuraTerminalOutcomes,
            "lane_work" => Self::LaneWork,
            "launch" => Self::Launch,
            "ledger" => Self::Ledger,
            "lifecycle_open" => Self::LifecycleOpen,
            "lifecycle_recovery" => Self::LifecycleRecovery,
            "pending_kura_recovery" => Self::PendingKuraRecovery,
            "pending_lifecycle" => Self::PendingLifecycle,
            "preactivation" => Self::Preactivation,
            "projection" => Self::Projection,
            "registry" => Self::Registry,
            "registry_recovery" => Self::RegistryRecovery,
            "registry_recovery_impl" => Self::RegistryRecoveryImpl,
            "replay_authority" => Self::ReplayAuthority,
            "replay_authority_base" => Self::ReplayAuthorityBase,
            "replay_authority_certified_body" => Self::ReplayAuthorityCertifiedBody,
            "runner" => Self::Runner,
            "runner_height_driver" => Self::RunnerHeightDriver,
            "runner_outer_cursor" => Self::RunnerOuterCursor,
            "runtime" => Self::Runtime,
            "scheduler_inputs" => Self::SchedulerInputs,
            "schema" => Self::Schema,
            "selector" => Self::Selector,
            "settlement" => Self::Settlement,
            "turn_driver" => Self::TurnDriver,
            "wal_recovery" => Self::WalRecovery,
            "worker" => Self::Worker,
            _ => return None,
        })
    }
}

#[cfg(test)]
fn source(id: SourceId) -> String {
    match id {
        SourceId::Adapter => reviewed_v2_adapter_source_for_test().to_owned(),
        SourceId::Authority => include_str!("v2_lifecycle_authority.rs").to_owned(),
        SourceId::BodyPipeline => {
            include_str!("v2_lifecycle_body_pipeline_transition.rs").to_owned()
        }
        SourceId::BodyPipelineTests => {
            include_str!("v2_lifecycle_body_pipeline_transition_tests.rs").to_owned()
        }
        SourceId::BodyStore => include_str!("v2_body_store.rs").to_owned(),
        SourceId::CanonicalRecoveryIngress => {
            include_str!("v2_runner/canonical_recovery_ingress.rs").to_owned()
        }
        SourceId::CertifiedServeStore => {
            include_str!("v2_certified_serve_payload_store.rs").to_owned()
        }
        SourceId::ConcreteAdmission => {
            include_str!("v2_lifecycle_concrete_admission.rs").to_owned()
        }
        SourceId::Coordinator => include_str!("v2_lifecycle_coordinator.rs").to_owned(),
        SourceId::CoordinatorSupport => {
            include_str!("v2_lifecycle_coordinator_support.rs").to_owned()
        }
        SourceId::Effects => reviewed_v2_effects_source_for_test().to_owned(),
        SourceId::KuraTerminalOutcomes => {
            include_str!("../kura/autonomous_lifecycle_terminal_outcomes.rs").to_owned()
        }
        SourceId::LaneWork => include_str!("v2_lane_work.rs").to_owned(),
        SourceId::Launch => include_str!("v2_lifecycle_launch.rs").to_owned(),
        SourceId::Ledger => reviewed_lifecycle_ledger_source_for_test().to_owned(),
        SourceId::LifecycleOpen => include_str!("v2_lifecycle_open.rs").replacen(
            "include!(\"v2_lifecycle_open_output_recovery.rs\");\n",
            include_str!("v2_lifecycle_open_output_recovery.rs"),
            1,
        ),
        SourceId::LifecycleRecovery => include_str!("v2_lifecycle_recovery.rs").to_owned(),
        SourceId::PendingKuraRecovery => include_str!("v2_pending_kura_recovery.rs").to_owned(),
        SourceId::PendingLifecycle => include_str!("v2_lifecycle_pending_kura.rs").to_owned(),
        SourceId::Preactivation => include_str!("v2_lifecycle_preactivation.rs").to_owned(),
        SourceId::Projection => include_str!("v2_lifecycle_projection.rs").to_owned(),
        SourceId::Registry => reviewed_lifecycle_work_registry_source_for_test().to_owned(),
        SourceId::RegistryRecovery => {
            include_str!("v2_lifecycle_work_registry_validate_recovery.rs").to_owned()
        }
        SourceId::RegistryRecoveryImpl => include_str!(
            "v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"
        )
        .replacen(
            "include!(\"v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs\");\n",
            include_str!("v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs"),
            1,
        )
        .replacen(
            "include!(\"v2_lifecycle_work_registry_validate_completion_impl.rs\");\n",
            include_str!("v2_lifecycle_work_registry_validate_completion_impl.rs"),
            1,
        )
        .replacen(
            "include!(\"v2_lifecycle_work_registry_access_impl.rs\");\n",
            include_str!("v2_lifecycle_work_registry_access_impl.rs"),
            1,
        )
        .replacen(
            "include!(\"v2_lifecycle_work_registry_validate_recovery_execution_impl.rs\");\n",
            include_str!("v2_lifecycle_work_registry_validate_recovery_execution_impl.rs"),
            1,
        ),
        SourceId::ReplayAuthority => include_str!("v2_lifecycle_replay_authority.rs")
            .replacen(
                "include!(\"v2_lifecycle_replay_authority_live_wal.rs\");\n",
                include_str!("v2_lifecycle_replay_authority_live_wal.rs"),
                1,
            )
            .replacen(
                "include!(\"v2_lifecycle_replay_authority_certified_serve.rs\");\n",
                include_str!("v2_lifecycle_replay_authority_certified_serve.rs"),
                1,
            )
            .replacen(
                "include!(\"v2_lifecycle_replay_authority_certified_body.rs\");\n",
                include_str!("v2_lifecycle_replay_authority_certified_body.rs"),
                1,
            )
            .replacen(
                "include!(\"v2_lifecycle_replay_authority_payload_projection.rs\");\n",
                include_str!("v2_lifecycle_replay_authority_payload_projection.rs"),
                1,
            )
            .replacen(
                "include!(\"v2_lifecycle_replay_authority_output_recovery.rs\");\n",
                include_str!("v2_lifecycle_replay_authority_output_recovery.rs"),
                1,
            ),
        SourceId::ReplayAuthorityBase => {
            include_str!("v2_lifecycle_replay_authority.rs").to_owned()
        }
        SourceId::ReplayAuthorityCertifiedBody => {
            include_str!("v2_lifecycle_replay_authority_certified_body.rs").to_owned()
        }
        SourceId::Runner => include_str!("v2_runner.rs").to_owned(),
        SourceId::RunnerHeightDriver => {
            include_str!("v2_runner/lifecycle_height_driver.rs").to_owned()
        }
        SourceId::RunnerOuterCursor => include_str!("v2_runner/outer_ingress_cursor.rs").to_owned(),
        SourceId::Runtime => reviewed_v2_runtime_source_for_test().to_owned(),
        SourceId::SchedulerInputs => include_str!("v2_lifecycle_scheduler_inputs.rs").to_owned(),
        SourceId::Schema => include_str!("v2_lifecycle_schema.rs").to_owned(),
        SourceId::Selector => include_str!("v2_lifecycle_selector.rs").to_owned(),
        SourceId::Settlement => include_str!("v2_lifecycle_settlement.rs").to_owned(),
        SourceId::TurnDriver => include_str!("v2_lifecycle_turn_driver.rs").to_owned(),
        SourceId::WalRecovery => include_str!("v2_lifecycle_wal_recovery.rs").to_owned(),
        SourceId::Worker => reviewed_v2_worker_source_for_test().to_owned(),
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
enum Edge {
    Begin,
    After(String),
    At(String),
    Last(String),
    End,
    Before(String),
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct Span {
    source: SourceId,
    start: Edge,
    end: Edge,
}

#[cfg(test)]
#[derive(Clone, Debug)]
enum Contract {
    Required(String, String, String),
    Forbidden(String, String, String),
    Count(String, String, usize, String),
    Order(String, Vec<String>, String),
}

#[cfg(test)]
#[derive(Clone, Debug)]
struct Case {
    id: String,
    regions: std::collections::BTreeMap<String, Vec<Span>>,
    contracts: Vec<Contract>,
}

#[cfg(test)]
fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z' | b'_'))
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
fn unescape(value: &str) -> Result<String, String> {
    let mut output = String::with_capacity(value.len());
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte != b'\\' {
            output.push(char::from(byte));
            continue;
        }
        output.push(match bytes.next() {
            Some(b'\\') => '\\',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'p') => '|',
            Some(other) => return Err(format!("unsupported asset escape \\{}", char::from(other))),
            None => return Err("trailing asset escape".to_owned()),
        });
    }
    Ok(output)
}

#[cfg(test)]
fn edge(kind: &str, token: String, start: bool) -> Result<Edge, String> {
    match (start, kind, token.as_str()) {
        (true, "begin", "-") => Ok(Edge::Begin),
        (true, "after", _) if !token.is_empty() && token != "-" => Ok(Edge::After(token)),
        (true, "at", _) if !token.is_empty() && token != "-" => Ok(Edge::At(token)),
        (true, "last", _) if !token.is_empty() && token != "-" => Ok(Edge::Last(token)),
        (false, "end", "-") => Ok(Edge::End),
        (false, "before", _) if !token.is_empty() && token != "-" => Ok(Edge::Before(token)),
        _ => Err(format!(
            "invalid {} edge {kind}:{token}",
            if start { "start" } else { "end" }
        )),
    }
}

#[cfg(test)]
fn parse_contracts() -> Result<Vec<Case>, String> {
    let mut lines = SOURCE_CONTRACT_ASSET.lines().enumerate();
    if lines.next().map(|(_, line)| line) != Some("sumeragi-source-contracts-v1") {
        return Err("source contract asset has no exact v1 header".to_owned());
    }
    let mut cases = Vec::new();
    let mut current: Option<Case> = None;
    let mut ids = std::collections::BTreeSet::new();
    for (line_index, line) in lines {
        let line_number = line_index + 1;
        if line.is_empty() {
            return Err(format!("source contract asset line {line_number} is blank"));
        }
        let fields = line
            .split('|')
            .map(unescape)
            .collect::<Result<Vec<_>, _>>()?;
        match fields.as_slice() {
            [tag, id] if tag == "case" && identifier(id) && current.is_none() => {
                if !ids.insert(id.clone()) {
                    return Err(format!("duplicate source contract case {id}"));
                }
                current = Some(Case {
                    id: id.clone(),
                    regions: std::collections::BTreeMap::new(),
                    contracts: Vec::new(),
                });
            }
            [
                tag,
                id,
                source_id,
                start_kind,
                start_token,
                end_kind,
                end_token,
            ] if tag == "region" && identifier(id) => {
                let source = SourceId::parse(source_id).ok_or_else(|| {
                    format!("unknown source id {source_id} at line {line_number}")
                })?;
                let span = Span {
                    source,
                    start: edge(start_kind, start_token.clone(), true)?,
                    end: edge(end_kind, end_token.clone(), false)?,
                };
                current
                    .as_mut()
                    .ok_or_else(|| format!("region outside case at line {line_number}"))?
                    .regions
                    .entry(id.clone())
                    .or_default()
                    .push(span);
            }
            [tag, region, needle, diagnostic]
                if matches!(tag.as_str(), "required" | "forbidden")
                    && identifier(region)
                    && !needle.is_empty()
                    && !diagnostic.is_empty() =>
            {
                let contract = if tag == "required" {
                    Contract::Required(region.clone(), needle.clone(), diagnostic.clone())
                } else {
                    Contract::Forbidden(region.clone(), needle.clone(), diagnostic.clone())
                };
                current
                    .as_mut()
                    .ok_or_else(|| format!("contract outside case at line {line_number}"))?
                    .contracts
                    .push(contract);
            }
            [tag, region, needle, expected, diagnostic]
                if tag == "count"
                    && identifier(region)
                    && !needle.is_empty()
                    && !diagnostic.is_empty() =>
            {
                let expected = expected
                    .parse()
                    .map_err(|_| format!("invalid count at line {line_number}"))?;
                current
                    .as_mut()
                    .ok_or_else(|| format!("contract outside case at line {line_number}"))?
                    .contracts
                    .push(Contract::Count(
                        region.clone(),
                        needle.clone(),
                        expected,
                        diagnostic.clone(),
                    ));
            }
            [tag, region, anchor_count, rest @ ..]
                if tag == "order" && identifier(region) && rest.len() >= 3 =>
            {
                let anchor_count: usize = anchor_count
                    .parse()
                    .map_err(|_| format!("invalid order count at line {line_number}"))?;
                if anchor_count < 2 || rest.len() != anchor_count + 1 {
                    return Err(format!("invalid order width at line {line_number}"));
                }
                let (diagnostic, anchors) = rest
                    .split_last()
                    .ok_or_else(|| format!("missing order diagnostic at line {line_number}"))?;
                if diagnostic.is_empty() || anchors.iter().any(String::is_empty) {
                    return Err(format!("empty order field at line {line_number}"));
                }
                current
                    .as_mut()
                    .ok_or_else(|| format!("contract outside case at line {line_number}"))?
                    .contracts
                    .push(Contract::Order(
                        region.clone(),
                        anchors.to_vec(),
                        diagnostic.clone(),
                    ));
            }
            [tag] if tag == "end" => {
                let case = current
                    .take()
                    .ok_or_else(|| format!("end outside case at line {line_number}"))?;
                if case.regions.is_empty() || case.contracts.is_empty() {
                    return Err(format!("empty source contract case {}", case.id));
                }
                if case.contracts.iter().any(|contract| match contract {
                    Contract::Required(region, ..)
                    | Contract::Forbidden(region, ..)
                    | Contract::Count(region, ..)
                    | Contract::Order(region, ..) => !case.regions.contains_key(region),
                }) {
                    return Err(format!(
                        "source contract case {} names an unknown region",
                        case.id
                    ));
                }
                cases.push(case);
            }
            _ => return Err(format!("invalid source contract asset line {line_number}")),
        }
    }
    if current.is_some() || cases.len() != 50 {
        return Err(format!(
            "source contract asset must contain exactly 50 closed cases"
        ));
    }
    Ok(cases)
}

#[cfg(test)]
fn resolve(span: &Span) -> Result<String, String> {
    let source = source(span.source);
    if let Edge::Last(token) = &span.start {
        let end = match &span.end {
            Edge::End => source.len(),
            Edge::Before(end_token) => source
                .find(end_token)
                .ok_or_else(|| format!("missing end delimiter {end_token:?}"))?,
            Edge::Begin | Edge::After(_) | Edge::At(_) | Edge::Last(_) => {
                return Err("invalid resolved end edge".to_owned());
            }
        };
        let start = source[..end]
            .rfind(token)
            .ok_or_else(|| format!("missing bounded last start delimiter {token:?}"))?;
        return Ok(source[start..end].to_owned());
    }
    let start = match &span.start {
        Edge::Begin => 0,
        Edge::After(token) => source
            .find(token)
            .map(|offset| offset + token.len())
            .ok_or_else(|| format!("missing start delimiter {token:?}"))?,
        Edge::At(token) => source
            .find(token)
            .ok_or_else(|| format!("missing start delimiter {token:?}"))?,
        Edge::Last(_) => return Err("invalid resolved last edge".to_owned()),
        Edge::End | Edge::Before(_) => return Err("invalid resolved start edge".to_owned()),
    };
    let end = match &span.end {
        Edge::End => source.len(),
        Edge::Before(token) => source[start..]
            .find(token)
            .map(|offset| start + offset)
            .ok_or_else(|| format!("missing end delimiter {token:?}"))?,
        Edge::Begin | Edge::After(_) | Edge::At(_) | Edge::Last(_) => {
            return Err("invalid resolved end edge".to_owned());
        }
    };
    Ok(source[start..end].to_owned())
}

#[cfg(test)]
fn region(case: &Case, id: &str) -> Result<Vec<String>, String> {
    case.regions[id].iter().map(resolve).collect()
}

/// Run one declarative source contract from the strict versioned asset.
#[cfg(test)]
pub(crate) fn run_source_contract(id: &str) {
    let cases = parse_contracts().unwrap_or_else(|error| panic!("{error}"));
    let case = cases
        .iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("missing source contract case {id}"));
    for contract in &case.contracts {
        match contract {
            Contract::Required(region_id, needle, diagnostic) => {
                let parts = region(case, region_id).unwrap_or_else(|error| panic!("{error}"));
                assert!(
                    parts.iter().any(|part| part.contains(needle)),
                    "{diagnostic}"
                );
            }
            Contract::Forbidden(region_id, needle, diagnostic) => {
                let parts = region(case, region_id).unwrap_or_else(|error| panic!("{error}"));
                assert!(
                    parts.iter().all(|part| !part.contains(needle)),
                    "{diagnostic}"
                );
            }
            Contract::Count(region_id, needle, expected, diagnostic) => {
                let parts = region(case, region_id).unwrap_or_else(|error| panic!("{error}"));
                let actual = parts
                    .iter()
                    .map(|part| part.matches(needle).count())
                    .sum::<usize>();
                assert_eq!(actual, *expected, "{diagnostic}");
            }
            Contract::Order(region_id, anchors, diagnostic) => {
                let text = region(case, region_id)
                    .unwrap_or_else(|error| panic!("{error}"))
                    .join("\n");
                let mut remainder = text.as_str();
                for anchor in anchors {
                    let offset = remainder
                        .find(anchor)
                        .unwrap_or_else(|| panic!("{diagnostic}"));
                    remainder = &remainder[offset + anchor.len()..];
                }
            }
        }
    }
}

#[cfg(test)]
macro_rules! source_contract_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            $crate::sumeragi::v2_lifecycle_coordinator::run_source_contract(stringify!($name));
        }
    };
    (#[allow(clippy::too_many_lines)] $name:ident) => {
        #[test]
        #[allow(clippy::too_many_lines)]
        fn $name() {
            $crate::sumeragi::v2_lifecycle_coordinator::run_source_contract(stringify!($name));
        }
    };
}

#[cfg(test)]
pub(crate) use source_contract_test;

#[cfg(test)]
#[test]
fn source_contract_case_ids_are_unique() {
    let cases = parse_contracts().expect("strict source contract asset parses");
    let mut ids = std::collections::BTreeSet::new();
    assert!(
        cases.iter().all(|case| ids.insert(case.id.as_str())),
        "source contract case IDs must be unique"
    );
    assert_eq!(ids.len(), 50, "source contract inventory drifted");
    for id in ids {
        run_source_contract(id);
    }
}
