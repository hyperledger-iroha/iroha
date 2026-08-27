//! Headless Mochi sandbox lifecycle commands.
use super::{
    CliOverrides, configured_readiness_options_for, configured_readiness_smoke_for,
    prepare_supervisor_with_overrides, resolve_workspace_root_for_cli,
};
use mochi_core::{
    BootstrapBundle, BootstrapInputs, BootstrapWriteError, ExposedPrivateKey, LocalMcpProbeResult,
    PeerState, ReadinessOptions, Supervisor, SupervisorSessionInfo, ToriiClient, ToriiError,
    wait_for_all_managed_peers_genesis, write_bootstrap_bundle,
};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
#[cfg(unix)]
use std::fs::File;
use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    future::Future,
    io::{ErrorKind, Write as _},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::runtime::Runtime;
const LOCAL_MCP_STARTUP_INITIAL_BACKOFF: Duration = Duration::from_millis(250);
const LOCAL_MCP_STARTUP_MAX_BACKOFF: Duration = Duration::from_secs(1);
const REHEARSAL_PEER_COUNT: usize = 4;
const REHEARSAL_EVIDENCE_SCHEMA: u64 = 1;
const REHEARSAL_EVIDENCE_MAX_BYTES: usize = 2_048;
const INTERNAL_GENESIS_TEST_OVERRIDE: &str = "MOCHI_TEST_USE_INTERNAL_GENESIS";
#[derive(Clone, Copy)]
struct ReadinessRequirements {
    all_peers_genesis: bool,
    smoke: bool,
}
impl ReadinessRequirements {
    const SERVE_WITH_SMOKE: Self = Self {
        all_peers_genesis: true,
        smoke: true,
    };
    const SERVE_WITHOUT_SMOKE: Self = Self {
        all_peers_genesis: false,
        smoke: false,
    };
    const REHEARSAL: Self = Self::SERVE_WITH_SMOKE;
}
struct ReadinessProof {
    session: SupervisorSessionInfo,
    mcp_probe: LocalMcpProbeResult,
}
fn write_session_metadata_file(
    session_path: &Path,
    workspace_root: &Path,
    session: &SupervisorSessionInfo,
    readiness_smoke: bool,
    mcp_probe: &LocalMcpProbeResult,
) -> Result<(), String> {
    let payload = norito::json!({
        "pid": (process::id()),
        "ready": true,
        "mcp_ready": true,
        "readiness_smoke": readiness_smoke,
        "profile": (session.profile_slug.clone()),
        "chain_id": (session.chain_id.clone()),
        "generation_id": (session.generation_id.clone()),
        "workspace_root": (workspace_root.display().to_string()),
        "sandbox_root": (session.sandbox_root.display().to_string()),
        "peer_alias": (session.peer_alias.clone()),
        "api_base": (session.api_base.clone()),
        "torii_url": (session.torii_url.clone()),
        "mcp_url": (session.mcp_url.clone()),
        "account_id": (session.account_id.clone()),
        "onboarding_credential_id": (session.onboarding_credential_id.clone()),
        "onboarding_signer_file": (session.onboarding_signer_file.display().to_string()),
        "onboarding_token_file": (session.onboarding_token_file.display().to_string()),
        "mcp_protocol_version": (mcp_probe.protocol_version.clone()),
        "mcp_toolset_version": (mcp_probe.toolset_version.clone()),
        "mcp_tool_count": (mcp_probe.tool_count),
        "mcp_tools": (mcp_probe.tool_names.clone()),
    });
    let bytes = norito::json::to_vec_pretty(&payload)
        .map_err(|err| format!("failed to serialize session metadata: {err}"))?;
    if let Some(parent) = session_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create session metadata directory {}: {err}",
                parent.display()
            )
        })?;
    }
    static NEXT_SESSION_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let parent = session_path
        .parent()
        .ok_or_else(|| "session metadata path has no parent".to_owned())?;
    let mut staged = None;
    for _ in 0..32 {
        let id = NEXT_SESSION_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".session.json.tmp.{}.{id}", process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        match options.open(&path) {
            Ok(file) => {
                staged = Some((path, file));
                break;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to stage session metadata {}: {error}",
                    session_path.display()
                ));
            }
        }
    }
    let (staged_path, mut file) = staged
        .ok_or_else(|| "failed to allocate a unique session metadata file".to_owned())?;
    let write_result = file.write_all(&bytes).and_then(|()| file.sync_all());
    if let Err(error) = write_result {
        let _ = fs::remove_file(&staged_path);
        return Err(format!(
            "failed to write session metadata {}: {error}",
            session_path.display()
        ));
    }
    if let Err(error) = fs::rename(&staged_path, session_path) {
        let _ = fs::remove_file(&staged_path);
        return Err(format!(
            "failed to publish session metadata {}: {error}",
            session_path.display()
        ));
    }
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync session metadata directory {}: {error}",
                parent.display()
            )
        })?;
    Ok(())
}
fn bootstrap_inputs_from_session(
    session: &SupervisorSessionInfo,
    private_key: Option<String>,
) -> BootstrapInputs {
    BootstrapInputs {
        api_base: session.api_base.clone(),
        torii_url: session.torii_url.clone(),
        mcp_url: Some(session.mcp_url.clone()),
        chain_id: session.chain_id.clone(),
        account_id: session.account_id.clone(),
        private_key,
    }
}
fn write_bootstrap_files_for_session(
    workspace_root: &Path,
    session: &SupervisorSessionInfo,
    private_key: Option<String>,
) -> Result<Vec<PathBuf>, BootstrapWriteError> {
    let bundle = BootstrapBundle::render(&bootstrap_inputs_from_session(session, private_key));
    write_bootstrap_bundle(workspace_root, &bundle, true)
}
/// Run the long-lived headless sandbox server.
pub(super) fn run_serve(overrides: CliOverrides) -> Result<(), String> {
    let (supervisor, supervisor_error, bundle_config) =
        prepare_supervisor_with_overrides(&overrides);
    let mut supervisor = supervisor.ok_or_else(|| {
        supervisor_error
            .map(|err| format!("failed during sandbox preparation: {err}"))
            .unwrap_or_else(|| "failed during sandbox preparation".to_owned())
    })?;
    let workspace_root =
        resolve_workspace_root_for_cli(&overrides, bundle_config.as_ref(), Some(&supervisor));
    let readiness_smoke = configured_readiness_smoke_for(bundle_config.as_ref(), &overrides);
    let readiness_options = configured_readiness_options_for(&overrides);
    supervisor
        .start_all()
        .map_err(|err| format!("failed while starting peers: {err}"))?;
    let requirements = if readiness_smoke {
        ReadinessRequirements::SERVE_WITH_SMOKE
    } else {
        ReadinessRequirements::SERVE_WITHOUT_SMOKE
    };
    let runtime = Runtime::new().map_err(|err| format!("failed to create runtime: {err}"))?;
    let proof = runtime.block_on(prove_readiness(
        &supervisor,
        readiness_options,
        requirements,
        "sandbox startup",
    ))?;
    let private_key = supervisor.signers().first().map(|signer| {
        ExposedPrivateKey(signer.key_pair().private_key().clone()).to_string()
    });
    write_bootstrap_files_for_session(&workspace_root, &proof.session, private_key)
        .map_err(|err| format!("failed while writing workspace bootstrap files: {err}"))?;
    let session_path = proof.session.sandbox_root.join("session.json");
    write_session_metadata_file(
        &session_path,
        &workspace_root,
        &proof.session,
        readiness_smoke,
        &proof.mcp_probe,
    )?;
    println!("MOCHI sandbox ready");
    println!("  workspace: {}", workspace_root.display());
    println!("  sandbox: {}", proof.session.sandbox_root.display());
    println!("  torii: {}", proof.session.torii_url);
    println!("  mcp: {}", proof.session.mcp_url);
    println!("  session: {}", session_path.display());
    runtime.block_on(wait_for_shutdown_signal());
    Ok(())
}
/// Run a bounded, one-shot rehearsal of live four-peer wipe and re-genesis.
pub(super) fn run_wipe_rehearsal(overrides: CliOverrides) -> Result<(), String> {
    require_disposable_data_root(&overrides)?;
    reject_test_genesis_override()?;
    if overrides.readiness_smoke == Some(false) {
        return Err("wipe rehearsal requires readiness smoke; remove `--disable-smoke`".to_owned());
    }
    let (supervisor, supervisor_error, bundle_config) =
        prepare_supervisor_with_overrides(&overrides);
    let mut supervisor = supervisor.ok_or_else(|| {
        supervisor_error
            .map(|err| format!("failed during wipe-rehearsal preparation: {err}"))
            .unwrap_or_else(|| "failed during wipe-rehearsal preparation".to_owned())
    })?;
    if !configured_readiness_smoke_for(bundle_config.as_ref(), &overrides) {
        return Err(
            "wipe rehearsal requires readiness smoke; enable it in the selected config".to_owned(),
        );
    }
    let expected_aliases = peer_aliases(&supervisor);
    validate_exact_four_peer_topology(&expected_aliases)?;
    let readiness_options = configured_readiness_options_for(&overrides);
    let runtime = Runtime::new().map_err(|err| format!("failed to create runtime: {err}"))?;
    supervisor
        .start_all()
        .map_err(|err| format!("failed while starting rehearsal peers: {err}"))?;
    let initial_proof = runtime.block_on(prove_readiness(
        &supervisor,
        readiness_options,
        ReadinessRequirements::REHEARSAL,
        "initial generation",
    ))?;
    supervisor.refresh_peer_states();
    let running_before = running_aliases(&supervisor);
    validate_running_aliases("initial generation", &expected_aliases, &running_before)?;
    let old_generation_id = supervisor.generation_id().to_owned();
    supervisor
        .wipe_and_regenerate()
        .map_err(|err| format!("wipe and re-genesis failed: {err}"))?;
    let new_generation_id = supervisor.generation_id().to_owned();
    supervisor.refresh_peer_states();
    let running_after_restart = running_aliases(&supervisor);
    validate_transition(
        &old_generation_id,
        &new_generation_id,
        &expected_aliases,
        &running_before,
        &running_after_restart,
    )?;
    let regenerated_proof = runtime.block_on(prove_readiness(
        &supervisor,
        readiness_options,
        ReadinessRequirements::REHEARSAL,
        "regenerated generation",
    ))?;
    supervisor.refresh_peer_states();
    let running_after_readiness = running_aliases(&supervisor);
    validate_transition(
        &old_generation_id,
        &new_generation_id,
        &expected_aliases,
        &running_before,
        &running_after_readiness,
    )?;
    let evidence = encode_rehearsal_evidence(
        &old_generation_id,
        &new_generation_id,
        &expected_aliases,
        initial_proof.mcp_probe.tool_count,
        regenerated_proof.mcp_probe.tool_count,
    )?;
    supervisor
        .stop_all()
        .map_err(|err| format!("wipe rehearsal passed but peer cleanup failed: {err}"))?;
    println!("{evidence}");
    Ok(())
}
async fn prove_readiness(
    supervisor: &Supervisor,
    readiness_options: ReadinessOptions,
    requirements: ReadinessRequirements,
    stage: &str,
) -> Result<ReadinessProof, String> {
    let session = supervisor
        .session_info()
        .map_err(|err| format!("failed while collecting {stage} sandbox connection info: {err}"))?;
    let client = supervisor
        .torii_client(&session.peer_alias)
        .ok_or_else(|| format!("failed to create a Torii client for {stage}"))?;
    if requirements.all_peers_genesis {
        let managed_clients = managed_peer_clients(supervisor, stage)?;
        wait_for_all_managed_peers_genesis(managed_clients, readiness_options)
            .await
            .map_err(|err| {
                format!(
                    "failed while waiting for committed genesis on every peer in {stage}: {err}"
                )
            })?;
    }
    if requirements.smoke {
        let mut plan = supervisor
            .default_readiness_smoke_plan()
            .map_err(|err| format!("failed while preparing readiness smoke for {stage}: {err}"))?;
        plan.status_options = readiness_options;
        client.wait_for_readiness_smoke(plan).await.map_err(|err| {
            format!(
                "failed while waiting for readiness smoke in {stage}: {err} ({:?})",
                err.summarize()
            )
        })?;
    } else {
        client
            .wait_for_ready(readiness_options)
            .await
            .map_err(|err| {
                format!(
                    "failed while waiting for /status readiness in {stage}: {err} ({:?})",
                    err.summarize()
                )
            })?;
    }
    let mcp_probe = validate_local_mcp_for_startup(&client, readiness_options.timeout)
        .await
        .map_err(|err| format!("failed while validating local MCP in {stage}: {err}"))?;
    Ok(ReadinessProof { session, mcp_probe })
}
fn managed_peer_clients(
    supervisor: &Supervisor,
    stage: &str,
) -> Result<Vec<(String, ToriiClient)>, String> {
    supervisor
        .peers()
        .iter()
        .map(|peer| {
            peer.torii_client()
                .map(|client| (peer.alias().to_owned(), client))
                .map_err(|err| {
                    format!(
                        "failed to create a Torii client for managed peer {} at {} in {stage}: {err}",
                        peer.alias(),
                        peer.torii_address()
                    )
                })
        })
        .collect()
}
fn peer_aliases(supervisor: &Supervisor) -> Vec<String> {
    supervisor
        .peers()
        .iter()
        .map(|peer| peer.alias().to_owned())
        .collect()
}
fn running_aliases(supervisor: &Supervisor) -> Vec<String> {
    supervisor
        .peers()
        .iter()
        .filter(|peer| peer.state() == PeerState::Running)
        .map(|peer| peer.alias().to_owned())
        .collect()
}
fn validate_exact_four_peer_topology(expected_aliases: &[String]) -> Result<(), String> {
    let aliases = alias_set("rehearsal topology", expected_aliases)?;
    if aliases.len() != REHEARSAL_PEER_COUNT {
        return Err(format!(
            "wipe rehearsal requires exactly {REHEARSAL_PEER_COUNT} peers, found {}",
            aliases.len()
        ));
    }
    Ok(())
}
fn validate_running_aliases(
    stage: &str,
    expected_aliases: &[String],
    running_aliases: &[String],
) -> Result<(), String> {
    let expected = alias_set("expected peer aliases", expected_aliases)?;
    let running = alias_set(stage, running_aliases)?;
    if running != expected {
        return Err(format!(
            "{stage} running aliases differ from the exact topology: expected {expected:?}, found {running:?}"
        ));
    }
    Ok(())
}
fn validate_transition(
    old_generation_id: &str,
    new_generation_id: &str,
    expected_aliases: &[String],
    running_before: &[String],
    running_after: &[String],
) -> Result<(), String> {
    validate_exact_four_peer_topology(expected_aliases)?;
    if old_generation_id == new_generation_id {
        return Err(format!(
            "wipe rehearsal did not select a new generation: `{old_generation_id}`"
        ));
    }
    validate_running_aliases("initial generation", expected_aliases, running_before)?;
    validate_running_aliases("regenerated generation", expected_aliases, running_after)
}
fn alias_set(label: &str, aliases: &[String]) -> Result<BTreeSet<String>, String> {
    let set = aliases.iter().cloned().collect::<BTreeSet<_>>();
    if set.len() != aliases.len() {
        return Err(format!("{label} contains duplicate peer aliases"));
    }
    Ok(set)
}
fn encode_rehearsal_evidence(
    old_generation_id: &str,
    new_generation_id: &str,
    aliases: &[String],
    initial_mcp_tool_count: usize,
    regenerated_mcp_tool_count: usize,
) -> Result<String, String> {
    let sorted_aliases = alias_set("rehearsal evidence", aliases)?
        .into_iter()
        .collect::<Vec<_>>();
    let value = norito::json!({
        "schema": REHEARSAL_EVIDENCE_SCHEMA,
        "command": "sandbox.rehearse-wipe-and-regenerate",
        "result": "passed",
        "peer_count": REHEARSAL_PEER_COUNT,
        "old_generation_id": (old_generation_id),
        "new_generation_id": (new_generation_id),
        "initial": {
            "all_peers_genesis": true,
            "readiness_smoke": true,
            "mcp_ready": true,
            "mcp_tool_count": initial_mcp_tool_count,
            "running_aliases": (sorted_aliases.clone()),
        },
        "regenerated": {
            "all_peers_genesis": true,
            "readiness_smoke": true,
            "mcp_ready": true,
            "mcp_tool_count": regenerated_mcp_tool_count,
            "running_aliases": (sorted_aliases),
        },
        "cleanup": "stopped",
    });
    let encoded = norito::json::to_string(&value)
        .map_err(|err| format!("failed to encode wipe-rehearsal evidence: {err}"))?;
    if encoded.len() > REHEARSAL_EVIDENCE_MAX_BYTES {
        return Err(format!(
            "wipe-rehearsal evidence exceeded its {REHEARSAL_EVIDENCE_MAX_BYTES}-byte bound"
        ));
    }
    Ok(encoded)
}
fn require_disposable_data_root(overrides: &CliOverrides) -> Result<PathBuf, String> {
    let root = overrides.data_root.clone().ok_or_else(|| {
        "wipe rehearsal requires an explicit fresh `--data-root <path>`".to_owned()
    })?;
    match fs::symlink_metadata(&root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "wipe-rehearsal data root `{}` must be a non-symlink directory",
                    root.display()
                ));
            }
            let mut entries = fs::read_dir(&root).map_err(|err| {
                format!(
                    "failed to inspect wipe-rehearsal data root `{}`: {err}",
                    root.display()
                )
            })?;
            if entries
                .next()
                .transpose()
                .map_err(|err| {
                    format!(
                        "failed to inspect wipe-rehearsal data root `{}`: {err}",
                        root.display()
                    )
                })?
                .is_some()
            {
                return Err(format!(
                    "wipe-rehearsal data root `{}` is not empty; use a fresh disposable path",
                    root.display()
                ));
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect wipe-rehearsal data root `{}`: {error}",
                root.display()
            ));
        }
    }
    Ok(root)
}
fn reject_test_genesis_override() -> Result<(), String> {
    if env::var_os(INTERNAL_GENESIS_TEST_OVERRIDE).is_some() {
        return Err(format!(
            "wipe rehearsal requires real Kagami; unset `{INTERNAL_GENESIS_TEST_OVERRIDE}`"
        ));
    }
    Ok(())
}
async fn validate_local_mcp_for_startup(
    client: &ToriiClient,
    readiness_timeout: Duration,
) -> Result<LocalMcpProbeResult, ToriiError> {
    retry_local_mcp_rate_limit(
        || client.validate_local_mcp(),
        readiness_timeout,
        LOCAL_MCP_STARTUP_INITIAL_BACKOFF,
        LOCAL_MCP_STARTUP_MAX_BACKOFF,
    )
    .await
}
async fn retry_local_mcp_rate_limit<F, Fut>(
    mut probe: F,
    readiness_timeout: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
) -> Result<LocalMcpProbeResult, ToriiError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<LocalMcpProbeResult, ToriiError>>,
{
    let started = tokio::time::Instant::now();
    let Some(deadline) = started.checked_add(readiness_timeout) else {
        return Err(local_mcp_readiness_timeout(readiness_timeout));
    };
    let mut backoff = initial_backoff.min(max_backoff);
    loop {
        match tokio::time::timeout_at(deadline, probe()).await {
            Ok(Ok(result)) => return Ok(result),
            Ok(Err(error)) if local_mcp_error_is_rate_limited(&error) => {
                let delay = local_mcp_retry_delay(&error, backoff)
                    .expect("rate-limited MCP errors always produce a retry delay");
                if error.retry_after().is_none() {
                    backoff = backoff.saturating_mul(2).min(max_backoff);
                }
                let now = tokio::time::Instant::now();
                let Some(remaining) = deadline.checked_duration_since(now) else {
                    return Err(local_mcp_readiness_timeout(readiness_timeout));
                };
                if delay >= remaining {
                    tokio::time::sleep(remaining).await;
                    return Err(local_mcp_readiness_timeout(readiness_timeout));
                }
                if delay.is_zero() {
                    tokio::task::yield_now().await;
                } else {
                    tokio::time::sleep(delay).await;
                }
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err(local_mcp_readiness_timeout(readiness_timeout)),
        }
    }
}
fn local_mcp_retry_delay(error: &ToriiError, fallback: Duration) -> Option<Duration> {
    if !local_mcp_error_is_rate_limited(error) {
        return None;
    }
    Some(error.retry_after().unwrap_or(fallback))
}
fn local_mcp_readiness_timeout(readiness_timeout: Duration) -> ToriiError {
    ToriiError::Timeout {
        context: format!("local MCP readiness after {readiness_timeout:?}"),
    }
}
fn local_mcp_error_is_rate_limited(error: &ToriiError) -> bool {
    matches!(error, ToriiError::RateLimited { .. })
        || matches!(
            error,
            ToriiError::UnexpectedStatus { status, .. } if status.as_u16() == 429
        )
}
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        if env::var_os("MOCHI_DETACHED").is_some() {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).ok();
            let mut hangup =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).ok();
            loop {
                tokio::select! {
                    received = async {
                        match &mut terminate {
                            Some(signal) => signal.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        if received.is_some() {
                            break;
                        }
                    }
                    _ = async {
                        match &mut interrupt {
                            Some(signal) => signal.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {}
                    _ = async {
                        match &mut hangup {
                            Some(signal) => signal.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {}
                }
            }
            return;
        }
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = terminate.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::Value;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    fn aliases() -> Vec<String> {
        (0..REHEARSAL_PEER_COUNT)
            .map(|index| format!("peer{index}"))
            .collect()
    }
    fn local_mcp_probe_fixture() -> LocalMcpProbeResult {
        LocalMcpProbeResult {
            protocol_version: "2025-06-18".to_owned(),
            toolset_version: Some("test-v1".to_owned()),
            tool_count: 1,
            tool_names: vec!["iroha.health".to_owned()],
        }
    }
    fn local_mcp_rate_limit_error(retry_after: Option<Duration>) -> ToriiError {
        ToriiError::RateLimited { retry_after }
    }
    fn session_fixture(root: &Path) -> SupervisorSessionInfo {
        let sandbox_root = root.join(".mochi/sandbox/four-peer-bft");
        SupervisorSessionInfo {
            profile_slug: "four-peer-bft".to_owned(),
            chain_id: "mochi-local".to_owned(),
            generation_id: "0123456789abcdef0123456789abcdef".to_owned(),
            sandbox_root: sandbox_root.clone(),
            workspace_root: Some(root.to_path_buf()),
            peer_alias: "peer0".to_owned(),
            api_base: "http://127.0.0.1:8080".to_owned(),
            torii_url: "http://127.0.0.1:8080".to_owned(),
            mcp_url: "http://127.0.0.1:8080/v1/mcp".to_owned(),
            account_id: Some("local-admin".to_owned()),
            onboarding_credential_id: "local-dev".to_owned(),
            onboarding_signer_file: sandbox_root.join("runtime/onboarding-signer.key"),
            onboarding_token_file: sandbox_root.join("runtime/onboarding.token"),
        }
    }
    #[test]
    fn bootstrap_inputs_preserve_the_session_contract() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session = session_fixture(temp.path());
        let private_key = Some("existing-local-client-key".to_owned());
        let inputs = bootstrap_inputs_from_session(&session, private_key.clone());
        assert_eq!(inputs.api_base, session.api_base);
        assert_eq!(inputs.torii_url, session.torii_url);
        assert_eq!(inputs.mcp_url.as_deref(), Some(session.mcp_url.as_str()));
        assert_eq!(inputs.chain_id, session.chain_id);
        assert_eq!(inputs.account_id.as_deref(), session.account_id.as_deref());
        assert_eq!(inputs.private_key, private_key);
    }
    #[test]
    fn bootstrap_writer_emits_all_local_connection_artifacts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session = session_fixture(temp.path());
        let written = write_bootstrap_files_for_session(
            temp.path(),
            &session,
            Some("existing-local-client-key".to_owned()),
        )
            .expect("write bootstrap files");
        assert_eq!(written.len(), 4);
        let env_local =
            fs::read_to_string(temp.path().join(".env.local")).expect("read generated env file");
        assert!(env_local.contains("IROHA_MCP_URL=http://127.0.0.1:8080/v1/mcp"));
        for relative in [
            ".mochi/generated/typescript/connect.ts",
            ".mochi/generated/rust/connect.rs",
            ".mochi/generated/kotlin/MochiConnect.kt",
        ] {
            assert!(temp.path().join(relative).is_file(), "missing {relative}");
        }
    }
    #[test]
    fn session_metadata_binds_generation_without_exposing_onboarding_secrets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session = session_fixture(temp.path());
        let session_path = temp.path().join("session.json");
        write_session_metadata_file(
            &session_path,
            temp.path(),
            &session,
            true,
            &local_mcp_probe_fixture(),
        )
        .expect("write session metadata");
        let payload: Value = norito::json::from_slice(
            &fs::read(&session_path).expect("read generated session metadata"),
        )
        .expect("parse session metadata");
        let payload = payload.as_object().expect("session metadata object");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            let mode = fs::metadata(&session_path)
                .expect("session metadata permissions")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode & 0o077, 0, "session metadata must be owner-only");
        }
        assert_eq!(
            payload.get("generation_id").and_then(Value::as_str),
            Some(session.generation_id.as_str())
        );
        assert_eq!(
            payload
                .get("onboarding_credential_id")
                .and_then(Value::as_str),
            Some("local-dev")
        );
        assert_eq!(
            payload
                .get("onboarding_signer_file")
                .and_then(Value::as_str),
            Some(session.onboarding_signer_file.to_string_lossy().as_ref())
        );
        assert_eq!(
            payload.get("onboarding_token_file").and_then(Value::as_str),
            Some(session.onboarding_token_file.to_string_lossy().as_ref())
        );
        for forbidden in [
            "private_key",
            "onboarding_token",
            "onboarding_token_hash",
            "onboarding_token_digest",
            "onboarding_signer",
            "onboarding_private_key",
        ] {
            assert!(!payload.contains_key(forbidden));
        }
    }
    #[test]
    fn transition_requires_new_generation_and_exact_running_set() {
        let peers = aliases();
        validate_transition("generation-a", "generation-b", &peers, &peers, &peers)
            .expect("exact transition");
        let same_generation =
            validate_transition("generation-a", "generation-a", &peers, &peers, &peers)
                .expect_err("same generation must fail");
        assert!(same_generation.contains("did not select a new generation"));
    }
    #[test]
    fn transition_rejects_missing_or_duplicate_running_aliases() {
        let peers = aliases();
        let missing = peers[..3].to_vec();
        let error = validate_transition("generation-a", "generation-b", &peers, &peers, &missing)
            .expect_err("missing peer must fail");
        assert!(error.contains("running aliases differ"));
        let mut duplicate = peers.clone();
        duplicate[3] = duplicate[2].clone();
        let error = validate_transition("generation-a", "generation-b", &peers, &peers, &duplicate)
            .expect_err("duplicate peer must fail");
        assert!(error.contains("duplicate peer aliases"));
    }
    #[test]
    fn transition_rejects_non_four_peer_topology() {
        let peers = aliases()[..3].to_vec();
        let error = validate_transition("generation-a", "generation-b", &peers, &peers, &peers)
            .expect_err("three peers must fail");
        assert!(error.contains("requires exactly 4 peers"));
    }
    #[test]
    fn evidence_is_bounded_and_contains_both_proofs() {
        let encoded = encode_rehearsal_evidence(
            "0123456789abcdef0123456789abcdef",
            "fedcba9876543210fedcba9876543210",
            &aliases(),
            17,
            19,
        )
        .expect("encode evidence");
        assert!(encoded.len() <= REHEARSAL_EVIDENCE_MAX_BYTES);
        let value: Value = norito::json::from_str(&encoded).expect("parse evidence");
        assert_eq!(value["result"].as_str(), Some("passed"));
        assert_eq!(value["initial"]["mcp_tool_count"].as_u64(), Some(17));
        assert_eq!(value["regenerated"]["mcp_tool_count"].as_u64(), Some(19));
        assert_eq!(
            value["initial"]["running_aliases"],
            value["regenerated"]["running_aliases"]
        );
        assert_ne!(value["old_generation_id"], value["new_generation_id"]);
    }
    #[test]
    fn disposable_root_must_be_explicit_and_empty() {
        let missing = CliOverrides::default();
        assert!(
            require_disposable_data_root(&missing)
                .expect_err("implicit root must fail")
                .contains("explicit fresh")
        );
        let temp = tempfile::tempdir().expect("tempdir");
        let empty = temp.path().join("empty");
        fs::create_dir(&empty).expect("create empty root");
        let mut overrides = CliOverrides::default();
        overrides.data_root = Some(empty.clone());
        assert_eq!(
            require_disposable_data_root(&overrides).expect("empty root accepted"),
            empty
        );
        fs::write(empty.join("existing"), b"state").expect("write existing state");
        assert!(
            require_disposable_data_root(&overrides)
                .expect_err("non-empty root must fail")
                .contains("is not empty")
        );
    }
    #[cfg(unix)]
    #[test]
    fn disposable_root_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let target = temp.path().join("target");
        let link = temp.path().join("link");
        fs::create_dir(&target).expect("create target");
        symlink(&target, &link).expect("create symlink");
        let mut overrides = CliOverrides::default();
        overrides.data_root = Some(link);
        assert!(
            require_disposable_data_root(&overrides)
                .expect_err("symlink root must fail")
                .contains("non-symlink")
        );
    }
    #[test]
    fn local_mcp_startup_retry_recovers_from_transient_429() {
        let runtime = Runtime::new().expect("runtime");
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed_attempts = Arc::clone(&attempts);
        let result = runtime.block_on(retry_local_mcp_rate_limit(
            move || {
                let attempt = observed_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt < 2 {
                        Err(local_mcp_rate_limit_error(None))
                    } else {
                        Ok(local_mcp_probe_fixture())
                    }
                }
            },
            Duration::from_secs(1),
            Duration::ZERO,
            Duration::ZERO,
        ));
        assert_eq!(
            result.expect("third attempt succeeds"),
            local_mcp_probe_fixture()
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }
    #[test]
    fn local_mcp_startup_retry_never_retries_protocol_failure() {
        let runtime = Runtime::new().expect("runtime");
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed_attempts = Arc::clone(&attempts);
        let error = runtime
            .block_on(retry_local_mcp_rate_limit(
                move || {
                    observed_attempts.fetch_add(1, Ordering::SeqCst);
                    async { Err(ToriiError::Decode("invalid MCP tool catalog".to_owned())) }
                },
                Duration::from_secs(1),
                Duration::ZERO,
                Duration::ZERO,
            ))
            .expect_err("protocol failure must be returned immediately");
        assert!(matches!(error, ToriiError::Decode(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn local_mcp_startup_retry_is_bounded() {
        let runtime = Runtime::new().expect("runtime");
        let attempts = Arc::new(AtomicUsize::new(0));
        let observed_attempts = Arc::clone(&attempts);
        let error = runtime
            .block_on(retry_local_mcp_rate_limit(
                move || {
                    observed_attempts.fetch_add(1, Ordering::SeqCst);
                    async { Err(local_mcp_rate_limit_error(Some(Duration::from_secs(1)))) }
                },
                Duration::from_millis(10),
                Duration::from_millis(1),
                Duration::from_millis(2),
            ))
            .expect_err("persistent throttling must reach the readiness deadline");
        assert!(matches!(
            error,
            ToriiError::Timeout { context } if context.contains("local MCP readiness")
        ));
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "a Retry-After beyond the remaining deadline cannot trigger another probe"
        );
    }
    #[test]
    fn local_mcp_startup_retry_honors_server_retry_after() {
        let retry_after = Duration::from_secs(7);
        assert_eq!(
            local_mcp_retry_delay(
                &local_mcp_rate_limit_error(Some(retry_after)),
                Duration::from_millis(250),
            ),
            Some(retry_after)
        );
        assert_eq!(
            local_mcp_retry_delay(
                &local_mcp_rate_limit_error(None),
                Duration::from_millis(250),
            ),
            Some(Duration::from_millis(250))
        );
    }
}
