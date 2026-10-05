//! Typed, atomic public inputs for explicit native owner adoption.
//!
//! This is nested below the sole adoption codec. Opaque journals are streamed
//! only to obtain their public digest; their contents never select a phase.

use super::super::super::super::validate_absolute_normal_path;
use super::*;

#[derive(clap::Args, Debug)]
pub(crate) struct PrepareNativeEdgeOwner {
    /// Independently admitted MacStadium host pair.
    #[arg(long, value_name = "PATH")]
    host_pair: PathBuf,
    /// Independent SHA-256 pin of the exact host-pair bytes.
    #[arg(long, value_name = "SHA256")]
    expected_host_pair_sha256: String,
    /// Maintained native nginx capture plan for the unchanged public include.
    #[arg(long, value_name = "PATH")]
    nginx_plan: PathBuf,
    /// Independent SHA-256 pin of the exact capture-plan bytes.
    #[arg(long, value_name = "SHA256")]
    expected_nginx_plan_sha256: String,
    /// Ordered opaque incumbent journal basename; repeat for each admitted journal.
    #[arg(long, value_name = "BASENAME", required = true, num_args = 1)]
    opaque_journal: Vec<String>,
    /// Fresh owner-private directory beneath the independently admitted native custody.
    #[arg(long, value_name = "DIR")]
    output: PathBuf,
}

#[derive(Debug, JsonSerialize)]
struct PreparedNativeEdgeOwnerV1 {
    schema: String,
    qualified: bool,
    host_pair_sha256: String,
    nginx_plan_sha256: String,
    request: NativePublicFileV1,
    plan: NativePublicFileV1,
}

fn pinned_private(path: &Path, expected: &str, maximum: u64) -> Result<PublicPin> {
    validate_lower_hex("native owner input pin", expected, 64)?;
    let pin = PublicPin::open(path, maximum, true)?;
    if pin.reference.sha256 != expected {
        return Err(eyre!(
            "native owner preparation input changed from its independent pin"
        ));
    }
    Ok(pin)
}

fn plan_destination(nginx: &json::Value) -> Result<PathBuf> {
    let destination = nginx
        .get("destination")
        .ok_or_else(|| eyre!("native owner capture plan has no destination"))?;
    let parent = destination
        .get("directory")
        .and_then(|value| value.get("path"))
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native owner capture plan has no include directory"))?;
    let name = destination
        .get("basename")
        .and_then(json::Value::as_str)
        .ok_or_else(|| eyre!("native owner capture plan has no include basename"))?;
    if name.is_empty() || Path::new(name).file_name() != Some(OsStr::new(name)) {
        return Err(eyre!("native owner include must be one direct basename"));
    }
    Ok(Path::new(parent).join(name))
}

fn opaque_names(names: &[String], operation: &str) -> Result<()> {
    if names.is_empty() || names.len() > MAX_JOURNALS {
        return Err(eyre!(
            "native owner preparation requires 1..32 opaque journals"
        ));
    }
    let mut seen = BTreeSet::new();
    for name in names {
        let old_operation = name
            .strip_prefix(".taira-native-nginx-apply-")
            .and_then(|value| value.strip_suffix(".receipt.ndjson"))
            .ok_or_else(|| eyre!("native owner incident is not a canonical journal basename"))?;
        validate_lower_hex("native opaque incident operation", old_operation, 32)?;
        if old_operation == operation || !seen.insert(name) {
            return Err(eyre!(
                "native owner incidents must be distinct incumbent operations"
            ));
        }
    }
    Ok(())
}

fn projected_plan_reference(pin: &PublicPin, output: &Path) -> NativePublicFileV1 {
    let mut reference = pin.reference.clone();
    reference.file.path = output
        .join("adoption-plan.json")
        .to_string_lossy()
        .into_owned();
    reference
}

pub(crate) fn prepare<W: Write>(args: &PrepareNativeEdgeOwner, output: &mut W) -> Result<()> {
    // Resolve the actual OS account before any selected input supplies custody.
    let (uid, gid, user, home) = native_account()?;
    let mut pair_pin = pinned_private(
        &args.host_pair,
        &args.expected_host_pair_sha256,
        MAX_REQUEST,
    )?;
    let hosts: host_pair::ResetHostPairV1 =
        json::from_slice(&pair_pin.bytes(MAX_REQUEST as usize)?)?;
    hosts.validate()?;
    let native = &hosts.native_edge;
    let fixed_custody = Path::new(&home).join(".local/share/iroha/taira/public-reset-v1");
    if uid != native.owner_uid
        || gid != native.owner_gid
        || user != native.endpoint.user
        || home != native.owner_home
        || fixed_custody != Path::new(&native.custody_root)
    {
        return Err(eyre!(
            "native owner preparation selects another physical OS custodian"
        ));
    }
    validate_absolute_normal_path(&args.output, "native owner output directory")?;
    if !args.output.starts_with(&fixed_custody) || args.output == fixed_custody {
        return Err(eyre!(
            "native owner output must remain in admitted private custody"
        ));
    }
    let parent = PrivateDirectory::open(
        args.output
            .parent()
            .ok_or_else(|| eyre!("native owner output has no parent"))?,
    )?;
    require_path_absent(&args.output, "native owner output directory")?;
    let mut guard_pin = PublicPin::open(
        &fixed_custody.join("taira-edge/guard.json"),
        16 * 1024,
        true,
    )?;
    let guard: HostGuardV1 = json::from_slice(&guard_pin.bytes(16 * 1024)?)?;
    let dispatcher_pin =
        PublicPin::open(Path::new(&native.dispatcher_path), 512 * 1024 * 1024, false)?;
    let dispatcher_identity = &dispatcher_pin.reference.file.identity;
    if guard_pin.reference.sha256 != native.guard_sha256
        || guard.schema != HOST_GUARD_SCHEMA_V1
        || guard.host_slug != "taira-edge"
        || guard.dispatcher_path != native.dispatcher_path
        || guard.dispatcher_sha256 != native.dispatcher_sha256
        || dispatcher_pin.reference.sha256 != native.dispatcher_sha256
        || dispatcher_identity.uid != uid
        || dispatcher_identity.gid != gid
        || dispatcher_identity.links != 1
        || dispatcher_identity.mode & 0o111 == 0
        || dispatcher_identity.mode & 0o022 != 0
        || Path::new(&native.dispatcher_path) != fixed_custody.join("dispatcher/iroha")
        || guard.service_root != format!("{home}/.local/share/iroha/taira/edge")
        || guard.state_root != format!("{home}/.local/share/iroha/taira/edge/state")
        || guard.upload_parent != upload_parent(&guard.service_root)
    {
        return Err(eyre!(
            "native owner preparation differs from its independent guard"
        ));
    }
    let mut nginx_pin = pinned_private(
        &args.nginx_plan,
        &args.expected_nginx_plan_sha256,
        MAX_PLAN as u64,
    )?;
    let nginx_raw = nginx_pin.bytes(MAX_PLAN)?;
    let nginx: json::Value = json::from_slice(&nginx_raw)?;
    let operation = plan_operation(&nginx)?.to_owned();
    opaque_names(&args.opaque_journal, &operation)?;
    let publication_path = plan_destination(&nginx)?;
    let publication_pin = PublicPin::open(&publication_path, MAX_PLAN as u64, true)?;
    let mut plan = AdoptionPlanV1 {
        schema: PLAN_SCHEMA.into(),
        nginx,
        publication: publication_pin.reference.clone(),
        opaque_journals: Vec::new(),
    };
    let incident_directory = ReaderDirectory::open(publisher_directory(&plan)?)?;
    let mut incidents = Vec::with_capacity(args.opaque_journal.len());
    for name in &args.opaque_journal {
        let pin =
            PublicPin::reader_child(&incident_directory, OsStr::new(name), MAX_PLAN as u64, true)?;
        plan.opaque_journals.push(pin.reference.clone());
        incidents.push(pin);
    }
    let now = now_unix_ms()?;
    let expires = now
        .checked_add(super::super::super::super::MAX_AUTHORIZATION_LIFETIME_MS)
        .ok_or_else(|| eyre!("native owner admission window overflows"))?;
    let mut request = AdoptionRequestV1 {
        schema: REQUEST_SCHEMA.into(),
        hosts,
        operation_id: operation,
        authorization_nonce: hex::encode(rand::random::<[u8; 16]>()),
        helper_source_closure_sha256: host_pair::helper_source_closure_sha256(),
        adoption_plan: plan.publication.clone(),
        not_before_unix_ms: now,
        expires_at_unix_ms: expires,
    };
    validate_plan(&plan, &request)?;
    let capsule = NativeCapsule::admit_host(&request.hosts.native_edge)?;
    // This action invokes the sole maintained owner plan decoder. It observes
    // the actual native master and cannot publish an include or send a signal.
    let response = capsule.run(
        &parent,
        "validate-owner-inputs",
        &nginx_raw,
        &[],
        Instant::now() + Duration::from_secs(45),
    )?;
    let checked: json::Value = json::from_slice(&response)?;
    if checked != norito::json!({}) {
        return Err(eyre!(
            "native owner capture plan validation returned unexpected fields"
        ));
    }
    for pin in [
        &pair_pin,
        &guard_pin,
        &dispatcher_pin,
        &nginx_pin,
        &publication_pin,
    ] {
        pin.revalidate()?;
        if Path::new(&pin.reference.file.path).starts_with(&args.output) {
            return Err(eyre!("native owner output overlaps retained input"));
        }
    }
    for pin in &incidents {
        pin.revalidate()?;
    }
    let staging = parent.create_child(format!(
        ".native-owner-inputs-{}",
        hex::encode(rand::random::<[u8; 16]>())
    ))?;
    let plan_wire = json::to_json(&plan)?;
    if plan_wire.len() > MAX_PLAN {
        return Err(eyre!("native owner plan exceeds its finite bound"));
    }
    staging.write_atomic(
        "adoption-plan.json",
        plan_wire.as_bytes(),
        PublishMode::CreateNew,
    )?;
    let plan_pin = PublicPin::private_child(&staging, "adoption-plan.json", MAX_PLAN as u64)?;
    request.adoption_plan = projected_plan_reference(&plan_pin, &args.output);
    let request_wire = json::to_json(&request)?;
    if request_wire.len() > MAX_REQUEST as usize {
        return Err(eyre!("native owner request exceeds its finite bound"));
    }
    claims(&request, &sha256_hex(request_wire.as_bytes()))?;
    staging.write_atomic(
        "request.json",
        request_wire.as_bytes(),
        PublishMode::CreateNew,
    )?;
    staging.sync()?;
    plan_pin.revalidate()?;
    let expected_plan = request.adoption_plan.clone();
    drop(plan_pin); // Directory publication requires no retained descendants.
    for pin in [
        &pair_pin,
        &guard_pin,
        &dispatcher_pin,
        &nginx_pin,
        &publication_pin,
    ] {
        pin.revalidate()?;
    }
    for pin in &incidents {
        pin.revalidate()?;
    }
    let published = staging.rename_to_sibling(
        args.output
            .file_name()
            .ok_or_else(|| eyre!("native owner output basename missing"))?,
        PublishMode::CreateNew,
    )?;
    let plan_pin = PublicPin::private_child(&published, "adoption-plan.json", MAX_PLAN as u64)?;
    let request_pin = PublicPin::private_child(&published, "request.json", MAX_REQUEST)?;
    if plan_pin.reference != expected_plan
        || request_pin.reference.sha256 != sha256_hex(request_wire.as_bytes())
    {
        return Err(eyre!(
            "native owner input publication changed its exact child custody"
        ));
    }
    let receipt = PreparedNativeEdgeOwnerV1 {
        schema: "iroha.taira.public-reset.native-owner-adoption-prepared.v1".into(),
        qualified: false,
        host_pair_sha256: pair_pin.reference.sha256,
        nginx_plan_sha256: nginx_pin.reference.sha256,
        request: request_pin.reference,
        plan: plan_pin.reference,
    };
    output.write_all(json::to_json(&receipt)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_owner_preparation_cli_requires_independent_pins_and_incident_selection() {
        use clap::Parser as _;
        let command = vec![
            "iroha",
            "taira",
            "public-reset",
            "prepare-native-edge-owner",
            "--host-pair",
            "hosts.json",
            "--expected-host-pair-sha256",
            "host-pin",
            "--nginx-plan",
            "capture.json",
            "--expected-nginx-plan-sha256",
            "plan-pin",
            "--opaque-journal",
            ".taira-native-nginx-apply-00000000000000000000000000000000.receipt.ndjson",
            "--output",
            "fresh-owner-inputs",
        ];
        assert!(crate::Args::try_parse_from(command.clone()).is_ok());
        for flag in [
            "--host-pair",
            "--expected-host-pair-sha256",
            "--nginx-plan",
            "--expected-nginx-plan-sha256",
            "--opaque-journal",
            "--output",
        ] {
            let mut missing = command.clone();
            let index = missing
                .iter()
                .position(|argument| *argument == flag)
                .unwrap();
            missing.drain(index..index + 2);
            assert!(
                crate::Args::try_parse_from(missing).is_err(),
                "accepted missing {flag}"
            );
        }
        let mut selected_secret = command;
        selected_secret.extend(["--signing-key-fd", "3"]);
        assert!(crate::Args::try_parse_from(selected_secret).is_err());
    }

    #[test]
    fn owner_input_publication_retains_actual_plan_identity_at_final_name() {
        let directory = tempfile::Builder::new()
            .prefix(".native-owner-input-publication-")
            .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        let parent = PrivateDirectory::open(directory.path()).unwrap();
        let stage = parent.create_child("stage").unwrap();
        stage
            .write_atomic(
                "adoption-plan.json",
                b"opaque-public-plan",
                PublishMode::CreateNew,
            )
            .unwrap();
        let pin = PublicPin::private_child(&stage, "adoption-plan.json", 1024).unwrap();
        let expected = projected_plan_reference(&pin, &directory.path().join("complete"));
        pin.revalidate().unwrap();
        drop(pin);
        stage.sync().unwrap();
        let complete = stage
            .rename_to_sibling("complete", PublishMode::CreateNew)
            .unwrap();
        let actual = PublicPin::private_child(&complete, "adoption-plan.json", 1024).unwrap();
        assert_eq!(actual.reference, expected);
        let foreign = parent.create_child("foreign").unwrap();
        foreign
            .write_atomic("adoption-plan.json", b"different", PublishMode::CreateNew)
            .unwrap();
        foreign.sync().unwrap();
        assert!(
            foreign
                .rename_to_sibling("complete", PublishMode::CreateNew)
                .is_err()
        );
        actual.revalidate().unwrap();
    }

    #[test]
    fn opaque_selection_is_bounded_ordered_distinct_and_never_a_path() {
        let operation = "ff".repeat(16);
        let names: Vec<_> = (0..32)
            .map(|index| format!(".taira-native-nginx-apply-{index:032x}.receipt.ndjson"))
            .collect();
        opaque_names(&names, &operation).unwrap();
        assert!(opaque_names(&[], &operation).is_err());
        let mut oversized = names.clone();
        oversized.push(format!(
            ".taira-native-nginx-apply-{}.receipt.ndjson",
            "aa".repeat(16)
        ));
        assert!(opaque_names(&oversized, &operation).is_err());
        assert!(opaque_names(&[names[0].clone(), names[0].clone()], &operation).is_err());
        assert!(opaque_names(&[format!("../{}", names[0])], &operation).is_err());
        assert!(
            opaque_names(
                &[format!(
                    ".taira-native-nginx-apply-{operation}.receipt.ndjson"
                )],
                &operation
            )
            .is_err()
        );
    }
}
