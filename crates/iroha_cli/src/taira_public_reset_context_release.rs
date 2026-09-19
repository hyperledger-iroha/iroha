//! Derive real source, artifact and validator identities before generated plans exist.
//! All credential/config decoding remains in the existing native typed parser.

use super::reset_context::*;
use super::*;

pub(super) struct DerivedRelease {
    pub(super) revision: RevisionV1,
    pub(super) validators: Vec<ValidatorV1>,
    pub(super) edge: EdgeV1,
    pub(super) pins: Vec<PinnedInput>,
}

fn pin_public(path: &Path, label: &str) -> Result<PinnedInput> {
    let (file, snapshot) = open_pinned_regular(path, label)?;
    Ok(PinnedInput {
        path: path.to_path_buf(),
        file,
        snapshot,
    })
}

fn derive_revision(intent: &ResetRevisionIntentV1) -> Result<(RevisionV1, PinnedInput)> {
    let input = pin_public(Path::new(&intent.source_manifest_path), "source manifest")?;
    let bytes = pinned_bytes(&input, MAX_JSON_BYTES)?;
    let source: SourceManifestV1 = json::from_slice(&bytes)
        .map_err(|_| eyre!("source manifest is not exact native V1 JSON"))?;
    let revision = RevisionV1 {
        branch: source.branch,
        commit: source.head_commit_sha1.clone(),
        tree: source.head_tree_sha1,
        cargo_lock_sha256: source.cargo_lock_sha256,
        source_root: intent.source_root.clone(),
        source_manifest_path: intent.source_manifest_path.clone(),
        source_manifest_sha256: sha256_hex(&bytes),
        source_closure_sha256: source.closure_sha256,
        target: BUILD_TARGET.into(),
        profile: BUILD_PROFILE.into(),
        build_id: source.head_commit_sha1,
    };
    validate_revision(&revision)?;
    validate_source_closure(&revision)?;
    revalidate_pinned(&input, "source manifest")?;
    Ok((revision, input))
}

fn derive_artifacts(
    intents: &[ResetArtifactIntentV1],
    revision: &RevisionV1,
) -> Result<(Vec<ArtifactV1>, Vec<PinnedInput>)> {
    let mut artifacts = Vec::new();
    let mut pins = Vec::new();
    for intent in intents {
        let path = Path::new(&intent.local_path);
        let (mode, maximum) = artifact_role_policy(&intent.role)?;
        let mut input = pin_public(path, "release artifact")?;
        if input.snapshot.len == 0 || input.snapshot.len > maximum {
            return Err(eyre!("release artifact is empty or exceeds its role bound"));
        }
        #[cfg(unix)]
        if input.snapshot.uid != rustix::process::geteuid().as_raw()
            || input.snapshot.mode & 0o7777 != u32::from(mode)
            || input.snapshot.nlink != 1
        {
            return Err(eyre!(
                "release artifact does not have its exact owner/mode/single-link custody"
            ));
        }
        let sha256 = sha256_reader(&mut input.file, path)?;
        revalidate_pinned(&input, "release artifact")?;
        artifacts.push(ArtifactV1 {
            role: intent.role.clone(),
            local_path: intent.local_path.clone(),
            remote_path: intent.remote_path.clone(),
            sha256,
            size: input.snapshot.len,
            mode,
            source_commit: revision.commit.clone(),
            target: BUILD_TARGET.into(),
        });
        pins.push(input);
    }
    Ok((artifacts, pins))
}

fn role_pin<'a>(
    artifacts: &[ArtifactV1],
    pins: &'a [PinnedInput],
    role: &str,
) -> Result<&'a PinnedInput> {
    if artifacts.len() != pins.len() {
        return Err(eyre!("release artifact pin closure differs"));
    }
    let index = artifacts
        .iter()
        .position(|a| a.role == role)
        .ok_or_else(|| eyre!("missing artifact role `{role}`"))?;
    let input = &pins[index];
    if input.path != Path::new(&artifacts[index].local_path) {
        return Err(eyre!("release artifact pin path differs"));
    }
    Ok(input)
}

fn validate_public_genesis(
    artifacts: &[ArtifactV1],
    pins: &[PinnedInput],
    public: &public_inputs::PublicInputsV1,
) -> Result<()> {
    let genesis = artifact(artifacts, "genesis")?;
    if genesis.sha256 != public.signed_genesis_sha256 {
        return Err(eyre!(
            "validator genesis artifact differs from native public inputs"
        ));
    }
    let expected = format!("{}\n", public.genesis_hash);
    let hash_file = role_pin(artifacts, pins, "genesis_hash")?;
    if pinned_bytes(hash_file, 65)?.as_slice() != expected.as_bytes() {
        return Err(eyre!(
            "validator genesis-hash artifact differs from native public inputs"
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn derive_fingerprints(
    intent: &ResetTopologyIntentV1,
    validator: &ResetValidatorIntentV1,
    client: &ValidatorClientV1,
    artifacts: &[ArtifactV1],
    pins: &[PinnedInput],
    public: &public_inputs::PublicInputsV1,
    genesis_bytes: &[u8],
    operator_public_key: &str,
    build_identity: &iroha_core::release_identity::BuildIdentity,
) -> Result<(String, String, String)> {
    use iroha_config::{
        base::toml::{MAX_TOML_SOURCE_BYTES, TomlSource},
        parameters::actual,
    };
    let input = role_pin(artifacts, pins, "config")?;
    let path = input.path.clone();
    let bytes = Zeroizing::new(pinned_bytes(input, MAX_TOML_SOURCE_BYTES as u64)?);
    validate_validator_genesis_config(
        &bytes,
        Path::new(&artifact(artifacts, "genesis")?.remote_path),
        &public.genesis_hash,
    )?;
    validate_validator_operator_config(&bytes, operator_public_key)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| eyre!("validator config is not UTF-8"))?;
    let table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("validator config is not TOML"))?;
    let config = actual::Root::from_toml_source(TomlSource::new_sensitive(
        path,
        table,
        crate::soracloud::zeroize_taira_toml_table,
    ))
    .map_err(|_| eyre!("validator config failed current typed admission"))?;
    validate_validator_pin_fee_asset(
        &config.gov.sorafs_pin_fee_asset_id,
        &intent.faucet_policy.asset_definition_id,
    )?;
    revalidate_pinned(input, "validator config")?;
    validate_candidate_inrou_scope(
        intent.qualification_scope,
        &validator.slug,
        &config.soracloud_runtime.inrou,
    )?;
    validate_candidate_probe_bind(&client.probe_origin, config.torii.address.value())?;
    if config.common.chain.to_string() != CHAIN_ID
        || config.common.peer.id.to_string() != client.peer_id
    {
        return Err(eyre!(
            "validator config differs from the explicit chain/peer identity"
        ));
    }
    let (hash, metadata) =
        iroha_core::release_identity::genesis_identity(genesis_bytes, &config.genesis.public_key)?;
    if hash.to_string() != public.genesis_hash || Hash::from(config.genesis.expected_hash) != hash {
        return Err(eyre!(
            "actual signed genesis differs from the explicit next genesis hash"
        ));
    }
    validate_taira_genesis_mode(metadata.mode)?;
    let shared = config
        .sumeragi
        .v2_config(
            std::time::Duration::from_millis(metadata.block_cadence_ms.get()),
            metadata.mode.into(),
        )
        .map_err(|_| eyre!("validator signed consensus configuration is invalid"))?;
    shared
        .validate_ingress_roster_capacity(4)
        .map_err(|_| eyre!("validator cannot admit the four-member roster"))?;
    Ok((
        Hash::new(config.common.peer.id.encode()).to_string(),
        build_identity.build_fingerprint().to_string(),
        shared.fingerprint().to_string(),
    ))
}

fn validate_shared_parts(validators: &[ValidatorV1], edge: &EdgeV1) -> Result<()> {
    let first = &validators
        .first()
        .ok_or_else(|| eyre!("validator cohort absent"))?
        .artifacts;
    for role in [
        "iroha3d",
        "iroha_cli",
        "kagami",
        "sorafs_node",
        "genesis",
        "genesis_hash",
    ] {
        let expected = artifact(first, role)?;
        for validator in validators.iter().skip(1) {
            let actual = artifact(&validator.artifacts, role)?;
            if actual.sha256 != expected.sha256
                || actual.size != expected.size
                || actual.mode != expected.mode
            {
                return Err(eyre!(
                    "validator role `{role}` must be byte-identical on all four hosts"
                ));
            }
        }
    }
    let config_hashes = validators
        .iter()
        .map(|validator| artifact(&validator.artifacts, "config").map(|value| value.sha256.clone()))
        .collect::<Result<BTreeSet<_>>>()?;
    if config_hashes.len() != VALIDATOR_SLUGS.len() {
        return Err(eyre!("each validator configuration must be distinct"));
    }
    let first_cli = artifact(first, "iroha_cli")?;
    let edge_cli = artifact(&edge.artifacts, "iroha_cli")?;
    if first_cli.sha256 != edge_cli.sha256
        || first_cli.size != edge_cli.size
        || first_cli.mode != edge_cli.mode
    {
        return Err(eyre!(
            "edge and validators must share one exact compiled CLI"
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn derive_release(
    intent: &ResetTopologyIntentV1,
    public: &public_inputs::PublicInputsV1,
    operator_public_key: &str,
    inputs: &ResetContextInputs,
) -> Result<DerivedRelease> {
    let _chain_guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
    if intent.validators.len() != 4
        || intent.validator_clients.len() != 4
        || inputs.validator_unit.len() != 4
    {
        return Err(eyre!(
            "assembly requires exactly four ordered validator inputs"
        ));
    }
    validator_operator_public_key(operator_public_key)?;
    let (revision, source_pin) = derive_revision(&intent.revision)?;
    let build_identity = crate::compiled_build_identity()?;
    if build_identity.release_source_commit()? != revision.commit {
        return Err(eyre!(
            "compiled executable source differs from the release revision"
        ));
    }
    let mut pins = vec![source_pin];
    let mut validators = Vec::new();
    for (((validator, client), unit_path), expected_slug) in intent
        .validators
        .iter()
        .zip(&intent.validator_clients)
        .zip(&inputs.validator_unit)
        .zip(VALIDATOR_SLUGS)
    {
        if validator.slug != expected_slug || client.slug != expected_slug {
            return Err(eyre!("validator order is canonical"));
        }
        let (artifacts, mut artifact_pins) = derive_artifacts(&validator.artifacts, &revision)?;
        validate_public_genesis(&artifacts, &artifact_pins, public)?;
        let unit = artifact(&artifacts, "validator_unit")?;
        if Path::new(&unit.local_path) != unit_path || unit.sha256 != unit_hash(unit_path)? {
            return Err(eyre!(
                "validator unit input is not its exact candidate artifact"
            ));
        }
        let unit_sha = unit.sha256.clone();
        let genesis = role_pin(&artifacts, &artifact_pins, "genesis")?;
        let genesis_bytes = pinned_bytes(genesis, 64 * 1024 * 1024)?;
        let (node_fingerprint, build_fingerprint, config_fingerprint) = derive_fingerprints(
            intent,
            validator,
            client,
            &artifacts,
            &artifact_pins,
            public,
            &genesis_bytes,
            operator_public_key,
            &build_identity,
        )?;
        revalidate_pinned(genesis, "signed genesis")?;
        let derived = ValidatorV1 {
            slug: validator.slug.clone(),
            node_fingerprint,
            build_fingerprint,
            config_fingerprint,
            endpoint: validator.endpoint.clone(),
            platform: validator.platform.clone(),
            service_root: validator.service_root.clone(),
            state_root: validator.state_root.clone(),
            reset_guard: validator.reset_guard.clone(),
            systemd_unit: validator.systemd_unit.clone(),
            systemd_unit_sha256: unit_sha,
            artifacts,
            initial_state: validator.initial_state.clone(),
        };
        validate_validator(
            &derived,
            expected_slug,
            &revision,
            intent.qualification_scope,
        )?;
        pins.append(&mut artifact_pins);
        validators.push(derived);
    }
    let (artifacts, mut artifact_pins) = derive_artifacts(&intent.edge.artifacts, &revision)?;
    let mut edge_unit_pin = pin_public(&inputs.edge_unit, "edge systemd unit")?;
    if edge_unit_pin.snapshot.len == 0 || edge_unit_pin.snapshot.len > 1024 * 1024 {
        return Err(eyre!("systemd unit is empty or oversized"));
    }
    let systemd_unit_sha256 = sha256_reader(&mut edge_unit_pin.file, &inputs.edge_unit)?;
    revalidate_pinned(&edge_unit_pin, "edge systemd unit")?;
    pins.push(edge_unit_pin);
    pins.append(&mut artifact_pins);
    let edge = EdgeV1 {
        slug: intent.edge.slug.clone(),
        endpoint: intent.edge.endpoint.clone(),
        platform: intent.edge.platform.clone(),
        service_root: intent.edge.service_root.clone(),
        state_root: intent.edge.state_root.clone(),
        reset_guard: intent.edge.reset_guard.clone(),
        nginx_config: intent.edge.nginx_config.clone(),
        artifacts,
        systemd_unit_sha256,
        initial_state: intent.edge.initial_state.clone(),
    };
    validate_edge(&edge, &revision)?;
    validate_shared_parts(&validators, &edge)?;
    validate_source_closure(&revision)?;
    for pin in &pins {
        revalidate_pinned(pin, "release context input")?;
    }
    Ok(DerivedRelease {
        revision,
        validators,
        edge,
        pins,
    })
}

#[cfg(not(unix))]
pub(super) fn derive_release(
    _: &ResetTopologyIntentV1,
    _: &public_inputs::PublicInputsV1,
    _: &str,
    _: &ResetContextInputs,
) -> Result<DerivedRelease> {
    Err(eyre!("public reset input assembly requires Unix"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn directory() -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix(".context-public-artifact-")
            .tempdir_in(std::env::var_os("HOME").unwrap())
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }
    #[test]
    fn reset_context_artifact_derives_real_bytes_and_retains_drift_custody() {
        let _chain_guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let dir = directory();
        let path = dir.path().join("genesis.nrt");
        fs::write(&path, b"public-fixture").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let intent = ResetArtifactIntentV1 {
            role: "genesis".into(),
            local_path: path.to_str().unwrap().into(),
            remote_path: "/srv/fixture/genesis.nrt".into(),
        };
        let revision = sample_inventory_fixture().revision;
        let (artifacts, pins) = derive_artifacts(&[intent], &revision).unwrap();
        assert_eq!(artifacts[0].sha256, sha256_hex(b"public-fixture"));
        assert_eq!(artifacts[0].size, 14);
        assert_eq!(artifacts[0].source_commit, revision.commit);
        assert_eq!(artifacts[0].mode, 0o644);
        fs::write(&path, b"changed").unwrap();
        assert!(revalidate_pinned(&pins[0], "public fixture").is_err());
    }
    #[test]
    fn reset_context_artifact_rejects_wrong_mode_and_symlink_before_projection() {
        let _chain_guard = ChainDiscriminantGuard::enter(CHAIN_DISCRIMINANT);
        let dir = directory();
        let path = dir.path().join("genesis.nrt");
        fs::write(&path, b"public-fixture").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut intent = ResetArtifactIntentV1 {
            role: "genesis".into(),
            local_path: path.to_str().unwrap().into(),
            remote_path: "/srv/fixture/genesis.nrt".into(),
        };
        let revision = sample_inventory_fixture().revision;
        assert!(derive_artifacts(&[intent.clone()], &revision).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let link = dir.path().join("alias.nrt");
        symlink(&path, &link).unwrap();
        intent.local_path = link.to_str().unwrap().into();
        assert!(derive_artifacts(&[intent], &revision).is_err());
    }
}
