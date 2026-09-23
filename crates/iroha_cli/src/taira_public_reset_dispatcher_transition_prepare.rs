//! Native plan producer: qualified import plus explicit current typed occupied bindings.
use super::super::super::{EdgeAdmittedReleaseV1, ValidatorAdmittedReleaseV1};
use super::*;

#[path = "taira_public_reset_dispatcher_runtime_capture.rs"]
pub(in super::super::super) mod capture;

/// Derive a transition plan without replacing controllers, services, or ledger state.
#[derive(clap::Args, Debug)]
pub(in super::super::super) struct PrepareDispatcherTransition {
    #[arg(long)]
    import_root: PathBuf,
    #[arg(long)]
    expected_result_sha256: String,
    #[arg(long)]
    retained_inventory: PathBuf,
    #[arg(long)]
    expected_retained_inventory_sha256: String,
    #[arg(long)]
    current_runtime: PathBuf,
    #[arg(long)]
    expected_current_runtime_sha256: String,
    #[arg(long)]
    trusted_public_key: PathBuf,
    #[arg(long)]
    operation_id: String,
    #[arg(long)]
    output: PathBuf,
}

/// Existing current-type occupied bindings; their executable source may differ from configuration.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(super) struct CurrentRuntime {
    schema: String,
    host_identity_sha256: String,
    validators: Vec<ValidatorAdmittedReleaseV1>,
    edge: EdgeAdmittedReleaseV1,
}
struct Observed(Vec<(Pin, File, super::super::super::FileSnapshot)>);
impl Observed {
    fn pin(&mut self, path: &Path, mode: Option<u32>, maximum: u64) -> Result<Pin> {
        validate_absolute_normal_path(path, "plan producer input")?;
        require_root_no_symlink_ancestors(path, "plan producer input")?;
        let (mut file, snapshot) = open_pinned_regular(path, "plan producer input")?;
        need(
            snapshot.uid == 0
                && snapshot.len > 0
                && snapshot.len <= maximum
                && snapshot.mode & 0o022 == 0
                && mode.is_none_or(|mode| snapshot.mode & 0o7777 == mode),
            "plan producer input custody differs",
        )?;
        let value = Pin {
            path: path.to_string_lossy().into_owned(),
            sha256: hash_reader(&mut file)?,
            size: snapshot.len,
            mode: snapshot.mode & 0o7777,
        };
        ensure_pinned_unchanged(path, "plan producer input", &file, &snapshot)?;
        self.0.push((value.clone(), file, snapshot));
        Ok(value)
    }
    fn revalidate(&self) -> Result<()> {
        for (pin, file, snapshot) in &self.0 {
            require_root_no_symlink_ancestors(Path::new(&pin.path), "plan producer input")?;
            ensure_pinned_unchanged(Path::new(&pin.path), "plan producer input", file, snapshot)?;
        }
        Ok(())
    }
}
fn text<'a>(record: &'a Value, field: &str) -> Result<&'a str> {
    record
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("missing producer {field}"))
}
fn candidate(import: &Path, expected: &str, observed: &mut Observed) -> Result<Candidate> {
    let proof = import.join("preparation");
    let preparation = observed.pin(&proof.join("result.json"), Some(0o400), 16 * 1024 * 1024)?;
    need(
        preparation.sha256 == expected,
        "qualified result digest differs",
    )?;
    let value: Value = json::from_slice(&admission::read(&preparation)?)?;
    let candidate = Candidate {
        commit: text(&value, "commit")?.into(),
        tree: text(&value, "tree")?.into(),
        signer_fingerprint: text(&value, "signer_fingerprint")?.into(),
        executable: observed.pin(&import.join("artifacts/bin/iroha"), Some(0o755), MAX_BINARY)?,
        preparation,
        request: observed.pin(&proof.join("request.json"), Some(0o400), 16 * 1024 * 1024)?,
        checks: observed.pin(&proof.join("checks.json"), Some(0o400), 16 * 1024 * 1024)?,
        capture: observed.pin(&proof.join("capture.json"), Some(0o400), 16 * 1024 * 1024)?,
        transfer_request: observed.pin(
            &import.join("request.json"),
            Some(0o400),
            16 * 1024 * 1024,
        )?,
        transfer_completed: observed.pin(
            &import.join("completed.json"),
            Some(0o400),
            16 * 1024 * 1024,
        )?,
        binary_transfer: observed.pin(
            &import.join("artifacts/verified-manifest.json"),
            Some(0o400),
            16 * 1024 * 1024,
        )?,
        source_transfer: observed.pin(
            &import.join("source/verified-manifest.json"),
            Some(0o400),
            16 * 1024 * 1024,
        )?,
    };
    need(
        import
            == Path::new(&format!(
                "{RUNTIME}/release-import-{}-{expected}",
                candidate.commit
            )),
        "exact qualified import root required",
    )?;
    Ok(candidate)
}
pub(super) fn validate_runtime(runtime: &CurrentRuntime) -> Result<()> {
    need(
        runtime.schema == "iroha.taira.dispatcher-current-runtime.v1"
            && runtime.validators.len() == 4,
        "current typed runtime with four ordered validators required",
    )?;
    require_lower_sha256(&runtime.host_identity_sha256, "host identity")?;
    for (index, prior) in runtime.validators.iter().enumerate() {
        let slug = SLUGS[index];
        let service = format!("/srv/taira/{slug}");
        validate_lower_hex("current configuration revision", &prior.commit, 40)?;
        need(
            prior.release_root == format!("{service}/releases/{}", prior.commit)
                && prior.service_state.stopped_state().is_some(),
            "exact stopped configuration binding required",
        )?;
        occupied::validate_prior_binding(prior, &service, &format!("iroha3d-{slug}.service"))?;
    }
    validate_lower_hex("edge configuration revision", &runtime.edge.commit, 40)?;
    need(
        runtime.edge.release_root == format!("/srv/taira/edge/releases/{}", runtime.edge.commit),
        "edge configuration binding differs",
    )?;
    require_lower_sha256(&runtime.edge.cli_sha256, "edge CLI")?;
    require_lower_sha256(&runtime.edge.config_sha256, "edge configuration")
}
fn selected_role(slug: &str, release: &str, files: Vec<Pin>) -> Result<OccupiedRole> {
    let directory = if slug == "taira-edge" { "edge" } else { slug };
    let state = format!("/var/lib/taira/{directory}");
    require_root_directory(Path::new(&state), slug != "taira-edge", "preserved state")?;
    let state_meta = fs::symlink_metadata(&state)?;
    let current = format!("/srv/taira/{directory}/current");
    require_root_no_symlink_ancestors(Path::new(&current), "preserved selector")?;
    let meta = fs::symlink_metadata(&current)?;
    need(
        meta.file_type().is_symlink()
            && meta.uid() == 0
            && fs::read_link(&current)? == Path::new(release),
        "selected runtime differs from current selector",
    )?;
    Ok(OccupiedRole {
        slug: slug.into(),
        state: DirectoryIdentity {
            path: state,
            device: state_meta.dev(),
            inode: state_meta.ino(),
        },
        selector: Selector {
            path: current,
            target: release.into(),
            device: meta.dev(),
            inode: meta.ino(),
        },
        files,
    })
}
fn runtime_roles(runtime: &CurrentRuntime, observed: &mut Observed) -> Result<Vec<OccupiedRole>> {
    validate_runtime(runtime)?;
    let mut roles = Vec::new();
    for (index, prior) in runtime.validators.iter().enumerate() {
        let mut files = Vec::new();
        for artifact in &prior.artifacts {
            let pin = observed.pin(
                Path::new(&artifact.path),
                Some(u32::from(artifact.mode)),
                MAX_BINARY,
            )?;
            need(
                pin.sha256 == artifact.sha256 && pin.size == artifact.size,
                "current artifact differs from selected binding",
            )?;
            files.push(pin);
        }
        let role = selected_role(SLUGS[index], &prior.release_root, files)?;
        prior
            .service_state
            .validate_state_identity(role.state.device, role.state.inode)?;
        roles.push(role);
    }
    let cli = observed.pin(
        &Path::new(&runtime.edge.release_root).join("bin/iroha"),
        Some(0o755),
        MAX_BINARY,
    )?;
    let config = observed.pin(
        &Path::new(&runtime.edge.release_root).join("taira.conf"),
        Some(0o640),
        MAX_BINARY,
    )?;
    let installed = observed.pin(
        Path::new("/etc/nginx/conf.d/taira.conf"),
        Some(0o640),
        MAX_BINARY,
    )?;
    let unit = observed.pin(
        Path::new("/etc/systemd/system/nginx.service"),
        Some(0o644),
        MAX_BINARY,
    )?;
    need(
        cli.sha256 == runtime.edge.cli_sha256
            && config.sha256 == runtime.edge.config_sha256
            && installed.sha256 == config.sha256,
        "current edge differs from selected binding",
    )?;
    roles.push(selected_role(
        "taira-edge",
        &runtime.edge.release_root,
        vec![cli, config, installed, unit],
    )?);
    Ok(roles)
}
impl PrepareDispatcherTransition {
    /// Produce private reviewable input from current typed records; never apply it.
    pub(in super::super::super) fn run<W: Write>(&self, output: &mut W) -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = output;
            return Err(eyre!("dispatcher preparation requires Linux"));
        }
        #[cfg(target_os = "linux")]
        {
            need(rustix::process::geteuid().as_raw() == 0, "root is required")?;
            validate_lower_hex("operation ID", &self.operation_id, 32)?;
            for hash in [
                &self.expected_result_sha256,
                &self.expected_retained_inventory_sha256,
                &self.expected_current_runtime_sha256,
            ] {
                require_lower_sha256(hash, "producer input digest")?;
            }
            let mut observed = Observed(Vec::new());
            let input = observed.pin(&self.current_runtime, Some(0o600), 16 * 1024 * 1024)?;
            need(
                input.sha256 == self.expected_current_runtime_sha256,
                "runtime input digest differs",
            )?;
            let runtime: CurrentRuntime = json::from_slice(&admission::read(&input)?)?;
            validate_runtime(&runtime)?;
            let coordination = Path::new(CONTROL)
                .join("hosts")
                .join(&runtime.host_identity_sha256);
            // Acquire exactly the same existing locks before observing mutable predecessor files.
            let locks = admission::locks_for_host(&runtime.host_identity_sha256)?;
            let inventory = observed.pin(&self.retained_inventory, None, 16 * 1024 * 1024)?;
            need(
                inventory.sha256 == self.expected_retained_inventory_sha256,
                "retained inventory digest differs",
            )?;
            let lease_pin = observed.pin(
                &coordination.join("lease.json"),
                Some(0o600),
                16 * 1024 * 1024,
            )?;
            let lease: HostLeaseV1 = json::from_slice(&admission::read(&lease_pin)?)?;
            need(
                lease.inventory_sha256 == inventory.sha256,
                "sealed lease does not bind selected retained inventory",
            )?;
            let progress_pin = observed.pin(
                &coordination.join("progress.json"),
                Some(0o600),
                16 * 1024 * 1024,
            )?;
            let progress: HostProgressV1 = json::from_slice(&admission::read(&progress_pin)?)?;
            require_lower_sha256(&lease.authorization_semantic_sha256, "sealed authorization")?;
            let terminal_pin = observed.pin(
                &Path::new(RUNTIME)
                    .join("journal-v1/completed")
                    .join(format!("{}.json", lease.authorization_semantic_sha256)),
                Some(0o600),
                16 * 1024 * 1024,
            )?;
            let terminal: Value = json::from_slice(&admission::read(&terminal_pin)?)?;
            let completed_next_step = u16::try_from(
                terminal
                    .get("next_step")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| eyre!("terminal next_step missing"))?,
            )?;
            let mut guards = Vec::new();
            for slug in SLUGS {
                guards.push(observed.pin(
                    &Path::new(CONTROL).join(slug).join("guard.json"),
                    Some(0o600),
                    16 * 1024 * 1024,
                )?);
            }
            let plan = Plan {
                schema: SCHEMA.into(),
                operation_id: self.operation_id.clone(),
                host_identity_sha256: runtime.host_identity_sha256.clone(),
                trusted_public_key: observed.pin(
                    &self.trusted_public_key,
                    None,
                    16 * 1024 * 1024,
                )?,
                candidate: candidate(
                    &self.import_root,
                    &self.expected_result_sha256,
                    &mut observed,
                )?,
                predecessor: Predecessor {
                    inventory_sha256: inventory.sha256,
                    authorization_sha256: lease.authorization_semantic_sha256,
                    authorization_nonce: lease.authorization_nonce,
                    completed_next_step,
                    sealed_forward_ordinal: progress.next_forward_ordinal,
                    completed: terminal_pin,
                    lease: lease_pin,
                    progress: progress_pin,
                    dispatcher: observed.pin(
                        Path::new(FIXED_DISPATCHER),
                        Some(0o755),
                        MAX_BINARY,
                    )?,
                    guards,
                    occupied: runtime_roles(&runtime, &mut observed)?,
                },
            };
            admission::validate_plan(&plan)?;
            let held = admission::admit(&plan)?;
            let new_guards = admission::new_guards(&plan, &operation_root(&plan))?;
            let bytes = json::to_vec(&plan)?;
            storage::check(&plan, &bytes, &operation_root(&plan), &new_guards)?;
            locks.revalidate()?;
            observed.revalidate()?;
            admission::revalidate(&plan, &held)?;
            require_root_no_symlink_ancestors(&self.output, "transition plan output")?;
            super::super::super::inputs::write_new_private(&self.output, &bytes)?;
            writeln!(
                output,
                "{}",
                json::to_json(&norito::json!({
                    "schema": "iroha.taira.dispatcher-transition-prepared.v1",
                    "plan_path": (self.output.to_string_lossy().as_ref()),
                    "plan_sha256": (sha256_hex(&bytes)),
                    "operation_id": (plan.operation_id),
                    "controller_mutated": false,
                    "ledger_mutated": false,
                }))?
            )?;
            Ok(())
        }
    }
}
