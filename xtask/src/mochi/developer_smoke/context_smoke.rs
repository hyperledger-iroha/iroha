//! Two genuine installed contexts, explicit targeting, and original deployment recovery.
//!
//! CLI documents below are presentation observations. Only ordinary generated environments,
//! deployment owners, and the existing all-peer native reads establish successful execution.

use super::{
    Harness, require_deployment, require_phase, require_same_deployment,
    require_zero_owned_resources,
};
use iroha_deploy::managed::ManagedContext;
use norito::json::{self, Value};
use std::{
    error::Error,
    fs,
    path::Path,
    time::{Duration, Instant},
};

const SECONDARY: &str = "secondary";
const CONTEXT_BUDGET: Duration = Duration::from_secs(600);

pub(super) fn run(
    harness: &mut Harness,
    initial: &Value,
    original_deployment: &Value,
) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + CONTEXT_BUDGET;
    let mut secondary_attempted = false;
    let result = exercise(
        harness,
        initial,
        original_deployment,
        deadline,
        &mut secondary_attempted,
    );
    // Set before its up call: a failed command can still have created an owned worker. Always
    // attempt authenticated cleanup, independently of the flow's elapsed observation deadline.
    // The outer Harness also stops local, including when this flow or secondary cleanup fails.
    let cleanup = if secondary_attempted {
        harness
            .command(&["localnet", "down", SECONDARY])
            .and_then(|value| require_zero_owned_resources(&value))
    } else {
        Ok(())
    };
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (result, cleanup) => Err(format!(
            "installed context smoke failed: flow={result:?}; secondary cleanup={cleanup:?}"
        )
        .into()),
    }
}

fn exercise(
    harness: &mut Harness,
    initial: &Value,
    original_deployment: &Value,
    deadline: Instant,
    secondary_attempted: &mut bool,
) -> Result<(), Box<dyn Error>> {
    let local = status_context(initial)?;
    if local.name != "local" {
        return Err("installed context smoke expected the original default local context".into());
    }
    require_deployment_context(original_deployment, &local)?;
    require_selected(&command(harness, &["context", "show"], deadline)?, &local)?;
    require_list(
        &command(harness, &["context", "list"], deadline)?,
        &[&local],
    )?;
    stop(harness, &local.name, deadline)?;

    *secondary_attempted = true;
    let started = command(harness, &["localnet", "up", SECONDARY], deadline)?;
    require_phase(&started, "ready", 4)?;
    let secondary = status_context(&started)?;
    if secondary.name != SECONDARY
        || secondary.network_id == local.network_id
        || secondary.account_id == local.account_id
        || secondary.client_config == local.client_config
    {
        return Err("second generated context reused original network or signer custody".into());
    }
    require_selected(
        &command(harness, &["context", "show"], deadline)?,
        &secondary,
    )?;
    require_list(
        &command(harness, &["context", "list"], deadline)?,
        &[&local, &secondary],
    )?;
    // Named show must not select the stopped original.
    require_selected(
        &command(harness, &["context", "show", &local.name], deadline)?,
        &local,
    )?;
    require_selected(
        &command(harness, &["context", "show"], deadline)?,
        &secondary,
    )?;
    let second_deployment = command(harness, &["contract", "deploy", "hello.ko"], deadline)?;
    require_deployment_context(&second_deployment, &secondary)?;
    if second_deployment.get("journal") == original_deployment.get("journal") {
        return Err("different contexts shared a deployment journal".into());
    }
    harness.execute_on_every_peer_in_context(
        &second_deployment,
        "30",
        Some(&secondary.name),
        Some(deadline),
    )?;

    require_selected(
        &command(harness, &["context", "use", &local.name], deadline)?,
        &local,
    )?;
    require_selected(&command(harness, &["context", "show"], deadline)?, &local)?;
    // Explicit show in the other direction also leaves selection untouched.
    require_selected(
        &command(harness, &["context", "show", SECONDARY], deadline)?,
        &secondary,
    )?;
    require_selected(&command(harness, &["context", "show"], deadline)?, &local)?;
    require_selected(
        &command(harness, &["context", "use", SECONDARY], deadline)?,
        &secondary,
    )?;
    stop(harness, SECONDARY, deadline)?;

    // The selected workspace remains secondary while an exact explicit target restarts local.
    let explicit = command(
        harness,
        &["contract", "deploy", "hello.ko", "--context", &local.name],
        deadline,
    )?;
    require_deployment_context(&explicit, &local)?;
    require_same_deployment(original_deployment, &explicit)?;
    require_selected(
        &command(harness, &["context", "show"], deadline)?,
        &secondary,
    )?;
    require_status(harness, &local, "ready", 4, deadline)?;
    require_status(harness, &secondary, "stopped", 0, deadline)?;
    harness.execute_on_every_peer_in_context(&explicit, "30", Some(&local.name), Some(deadline))?;
    stop(harness, &local.name, deadline)?;

    let journal = original_deployment
        .get("journal")
        .and_then(Value::as_str)
        .filter(|journal| Path::new(journal).is_absolute())
        .ok_or("original deployment has no absolute retained journal")?;
    // Remove only test-owned source after its real deployment. Resume must use the original
    // signed plan and artifact; it must neither reconstruct source nor create another context.
    fs::remove_file(harness.workspace.join("hello.ko"))?;
    let recovered = command(
        harness,
        &[
            "contract",
            "deploy",
            "--resume",
            journal,
            "--context",
            &local.name,
        ],
        deadline,
    )?;
    require_deployment_context(&recovered, &local)?;
    require_same_deployment(original_deployment, &recovered)?;
    require_selected(
        &command(harness, &["context", "show"], deadline)?,
        &secondary,
    )?;
    require_status(harness, &local, "ready", 4, deadline)?;
    require_status(harness, &secondary, "stopped", 0, deadline)?;
    harness.execute_on_every_peer_in_context(
        &recovered,
        "30",
        Some(&local.name),
        Some(deadline),
    )?;
    require_list(
        &command(harness, &["context", "list"], deadline)?,
        &[&local, &secondary],
    )?;
    require_selected(
        &command(harness, &["context", "use", &local.name], deadline)?,
        &local,
    )?;
    require_selected(&command(harness, &["context", "show"], deadline)?, &local)?;
    eprintln!(
        "[developer-smoke] two genuine contexts, explicit target isolation, original recovery"
    );
    Ok(())
}

fn command(
    harness: &mut Harness,
    args: &[&str],
    deadline: Instant,
) -> Result<Value, Box<dyn Error>> {
    harness
        .command_document(args, deadline, false)
        .map(|document| document.value)
        .map_err(|error| format!("context CLI observation failed: {}", error.as_str()).into())
}

fn stop(harness: &mut Harness, name: &str, deadline: Instant) -> Result<(), Box<dyn Error>> {
    let stopped = command(harness, &["localnet", "down", name], deadline)?;
    require_phase(&stopped, "stopped", 0)?;
    require_zero_owned_resources(&stopped)
}

fn require_status(
    harness: &mut Harness,
    original: &ManagedContext,
    phase: &str,
    peers: u64,
    deadline: Instant,
) -> Result<(), Box<dyn Error>> {
    let status = command(harness, &["localnet", "status", &original.name], deadline)?;
    require_phase(&status, phase, peers)?;
    if status_context(&status)? != *original {
        return Err("context lifecycle changed original generated identity".into());
    }
    Ok(())
}

fn status_context(value: &Value) -> Result<ManagedContext, Box<dyn Error>> {
    Ok(json::from_value(
        value
            .get("context")
            .cloned()
            .ok_or("status has no context")?,
    )?)
}

fn require_selected(value: &Value, expected: &ManagedContext) -> Result<(), Box<dyn Error>> {
    let observed: ManagedContext = json::from_value(value.clone())?;
    if &observed != expected {
        return Err("workspace context selection changed unexpectedly".into());
    }
    Ok(())
}

fn require_list(value: &Value, expected: &[&ManagedContext]) -> Result<(), Box<dyn Error>> {
    let observed: Vec<ManagedContext> = json::from_value(value.clone())?;
    if expected.is_empty()
        || observed.len() != expected.len()
        || observed
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual != *expected)
    {
        return Err("context list differs from exact retained generated contexts".into());
    }
    Ok(())
}

fn require_deployment_context(
    value: &Value,
    expected: &ManagedContext,
) -> Result<(), Box<dyn Error>> {
    require_deployment(value)?;
    if value.get("context").and_then(Value::as_str) != Some(expected.name.as_str()) {
        return Err("deployment reported another context".into());
    }
    super::receipt_artifact(value, &expected.network_id, expected.dataspace_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_cleanup_explicitly_names_its_original_local_environment() {
        let harness = Harness::new(&std::env::current_exe().unwrap()).unwrap();
        let command = harness.command_builder(&super::super::LOCAL_CLEANUP, true);
        let args = command.get_args().collect::<Vec<_>>();
        assert_eq!(args[..3], ["localnet", "down", "local"]);
        assert_eq!(args[3], "--state");
        assert_eq!(args[4], harness.state.as_os_str());
        assert_eq!(args[5], "--json");
        assert!(!harness.state.exists());
    }

    #[test]
    fn elapsed_context_turn_refuses_before_subprocess_or_managed_state_creation() {
        let mut harness = Harness::new(&std::env::current_exe().unwrap()).unwrap();
        let before = fs::read_dir(harness.root.path()).unwrap().count();
        assert!(
            command(
                &mut harness,
                &["context", "list"],
                Instant::now() - Duration::from_millis(1),
            )
            .is_err()
        );
        assert_eq!(harness.sequence, 0);
        assert_eq!(fs::read_dir(harness.root.path()).unwrap().count(), before);
        assert!(!harness.state.exists());
    }

    #[test]
    fn malformed_observations_cannot_establish_a_context_or_context_set() {
        // Refusers only: these are deliberately not generated contexts or successful native data.
        for bad in [
            Value::Null,
            norito::json!({}),
            norito::json!([]),
            norito::json!([null]),
        ] {
            assert!(status_context(&bad).is_err());
            assert!(require_list(&bad, &[]).is_err());
        }
        for context in [
            Value::Null,
            norito::json!("local"),
            norito::json!({"name": "local"}),
        ] {
            assert!(status_context(&norito::json!({"context": context})).is_err());
        }
    }
}
