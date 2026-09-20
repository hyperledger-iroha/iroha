//! Native projection of a freshly generated Taira validator into its public reset layout.
//!
//! Generate on the validator host and retain the owner-private localnet directory at its exact
//! absolute path. The projected config still reads `torii.account_onboarding.private_key_file`,
//! `torii.faucet.private_key_file`, the public `streaming.codec.rans_tables_path`, and
//! `nexus.registry.manifest_directory` there. Signed genesis binds the native semantic manifest
//! digest; retain the generated public manifest receipt for exact byte custody. This command does
//! not produce a standalone config bundle or relocate those required inputs. The generated HTTP
//! binding is admitted against `runtime/operator-signer.key` before the explicit deployment key
//! replaces it; the ledger/faucet and onboarding identities remain unchanged. The required Torii
//! listener selects its final bind interface at the generated port; P2P bindings stay unchanged.

use super::*;
use iroha::data_model::NetworkId;
use zeroize::Zeroizing;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

/// Materialize one generated validator without exporting its private configuration.
#[derive(clap::Args, Debug)]
pub(super) struct MaterializeValidatorConfig {
    /// Inherited owner-private generated validator configuration descriptor.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Exact native generator output directory containing genesis identity and HTTP signer custody.
    #[arg(long, value_name = "PATH")]
    localnet_dir: PathBuf,
    /// Canonical target role; its ordinal must match the generated peer storage paths.
    #[arg(long, value_parser = VALIDATOR_SLUGS)]
    validator: String,
    /// Independently reviewed canonical checked identity of the generated signed genesis.
    #[arg(long, value_name = "NETWORK_ID", value_parser = checked_network_id)]
    network_id: NetworkId,
    /// Exact installed signed-genesis artifact path for this validator.
    #[arg(long, value_name = "PATH")]
    genesis_file: PathBuf,
    /// Dedicated canonical Ed25519 HTTP operator-authentication public key.
    #[arg(long, value_name = "PUBLIC_KEY", value_parser = checked_operator_key)]
    operator_public_key: PublicKey,
    /// Final Torii listener as canonical IP:PORT; its port must match the generated config.
    #[arg(long, value_name = "IP:PORT", value_parser = checked_torii_bind_address)]
    torii_bind_address: std::net::SocketAddr,
    /// Fresh owner-private output; an existing file is never replaced.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

fn checked_network_id(value: &str) -> Result<NetworkId, String> {
    let network: NetworkId = value
        .parse()
        .map_err(|_| "network identity must be a canonical checked NetworkId".to_owned())?;
    if network.to_string() != value {
        return Err("network identity must be a canonical checked NetworkId".to_owned());
    }
    Ok(network)
}

fn checked_operator_key(value: &str) -> Result<PublicKey, String> {
    validator_operator_public_key(value)
        .map_err(|_| "operator public key must be canonical Ed25519".to_owned())
}

fn checked_torii_bind_address(value: &str) -> Result<std::net::SocketAddr, String> {
    let address = value.parse::<std::net::SocketAddr>().map_err(|_| {
        "Torii bind address must be a canonical IP:PORT with a nonzero port".to_owned()
    })?;
    if address.port() == 0
        || address.to_string() != value
        || matches!(address, std::net::SocketAddr::V6(address) if address.scope_id() != 0)
    {
        return Err(
            "Torii bind address must be a canonical unscoped IP:PORT with a nonzero port"
                .to_owned(),
        );
    }
    Ok(address)
}

/// Consume private bytes only through native descriptor custody and create a new private file.
pub(super) fn materialize(args: &MaterializeValidatorConfig) -> Result<()> {
    validate_absolute_normal_path(&args.localnet_dir, "generated network directory")?;
    validate_owner_private_dir(&args.localnet_dir, "generated network directory")?;
    validate_absolute_normal_path(&args.genesis_file, "installed signed genesis")?;
    let identity_path = args.localnet_dir.join("genesis.expected_hash");
    let (identity_file, identity_snapshot) =
        open_pinned_regular(&identity_path, "generated public genesis identity")?;
    let identity = read_pinned_bytes(
        &identity_path,
        "generated public genesis identity",
        identity_file,
        &identity_snapshot,
        128,
    )?;
    validate_generated_identity(&identity, &args.network_id)?;
    let runtime = args.localnet_dir.join("runtime");
    validate_owner_private_dir(&runtime, "generated runtime directory")?;
    let source_operator =
        crate::operator_key::load_operator_key_pair(&runtime.join("operator-signer.key"))?;
    let source_operator_public_key = source_operator.public_key().clone();
    drop(source_operator);
    let source = crate::client_config::read_inherited_private_file(
        args.config_fd,
        MAX_CONFIG_BYTES,
        "generated validator config",
    )?;
    let output = project_config(
        &source,
        &args.localnet_dir,
        &args.validator,
        &args.network_id,
        &args.genesis_file,
        &source_operator_public_key,
        &args.operator_public_key,
        args.torii_bind_address,
    )?;
    inputs::write_new_private(&args.output, &output)
}

fn validate_generated_identity(bytes: &[u8], network: &NetworkId) -> Result<()> {
    if bytes != format!("{network}\n").as_bytes() {
        return Err(eyre!(
            "generated public genesis identity differs from the independently reviewed NetworkId"
        ));
    }
    Ok(())
}

fn field_mut<'a>(table: &'a mut toml::Table, path: &[&str]) -> Result<&'a mut toml::Value> {
    let (last, parents) = path
        .split_last()
        .ok_or_else(|| eyre!("empty native configuration field path"))?;
    let mut cursor = table;
    for part in parents {
        cursor = cursor
            .get_mut(*part)
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("generated config omits required table `{part}`"))?;
    }
    cursor
        .get_mut(*last)
        .ok_or_else(|| eyre!("generated config omits required field `{last}`"))
}

fn state_paths(peer: usize) -> Vec<(Vec<&'static str>, PathBuf, &'static str)> {
    let state = PathBuf::from(format!("state/peer{peer}"));
    vec![
        (
            vec!["kura", "store_dir"],
            PathBuf::from(format!("storage/peer{peer}")),
            "storage",
        ),
        (
            vec!["soracloud_runtime", "state_dir"],
            state.join("soracloud_runtime"),
            "inrou-data/runtime",
        ),
        (
            vec!["tiered_state", "cold_store_root"],
            state.join("tiered_state"),
            "storage/tiered_state",
        ),
        (
            vec!["tiered_state", "da_store_root"],
            state.join("da_wsv_snapshots"),
            "storage/da_wsv_snapshots",
        ),
        (
            vec!["streaming", "session_store_dir"],
            state.join("streaming"),
            "storage/streaming",
        ),
        (
            vec![
                "network",
                "soranet_handshake",
                "pow",
                "revocation_store_path",
            ],
            state.join("soranet/ticket_revocations.norito"),
            "privacy/soranet/ticket_revocations.norito",
        ),
        (vec!["torii", "data_dir"], state.join("torii"), "torii-data"),
        (
            vec!["torii", "da_ingest", "replay_cache_store_dir"],
            state.join("torii/da_replay"),
            "torii-data/da_replay",
        ),
        (
            vec!["torii", "da_ingest", "manifest_store_dir"],
            state.join("torii/da_manifests"),
            "torii-data/da_manifests",
        ),
        (
            vec!["sorafs", "storage", "data_dir"],
            state.join("sorafs"),
            "sorafs-data",
        ),
        (
            vec!["sorafs", "por", "state_dir"],
            state.join("sorafs/por"),
            "sorafs-data/por",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn project_config(
    bytes: &[u8],
    source_root: &Path,
    validator: &str,
    network: &NetworkId,
    genesis_file: &Path,
    source_operator_key: &PublicKey,
    operator_key: &PublicKey,
    torii_bind_address: std::net::SocketAddr,
) -> Result<Zeroizing<Vec<u8>>> {
    validate_absolute_normal_path(source_root, "generated network directory")?;
    validate_absolute_normal_path(genesis_file, "installed signed genesis")?;
    let peer = VALIDATOR_SLUGS
        .iter()
        .position(|role| *role == validator)
        .ok_or_else(|| eyre!("unknown canonical public validator role"))?;
    validator_operator_public_key(&source_operator_key.to_string())?;
    validator_operator_public_key(&operator_key.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(eyre!(
            "generated validator config is empty or exceeds its bound"
        ));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| eyre!("generated validator config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("generated validator config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends")
            || table.contains_key("snapshot")
            || table.get("chain").and_then(toml::Value::as_str) != Some(CHAIN_ID)
            || table
                .get("chain_discriminant")
                .and_then(toml::Value::as_integer)
                != Some(i64::from(CHAIN_DISCRIMINANT))
        {
            return Err(eyre!(
                "source is not the exact generated Taira configuration shape"
            ));
        }
        let registry = table
            .get("nexus")
            .and_then(toml::Value::as_table)
            .and_then(|nexus| nexus.get("registry"))
            .and_then(toml::Value::as_table)
            .ok_or_else(|| eyre!("generated Taira config omits its lane manifest registry"))?;
        if registry.contains_key("cache_directory")
            || registry
                .get("manifest_directory")
                .and_then(toml::Value::as_str)
                != source_root.join("lane-manifests").to_str()
        {
            return Err(eyre!(
                "generated Taira lane manifest registry has a foreign directory or cache overlay"
            ));
        }
        let genesis = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("generated validator config omits genesis"))?;
        if genesis.contains_key("expected_hash")
            || genesis.contains_key("manifest_json")
            || genesis
                .get("expected_hash_file")
                .and_then(toml::Value::as_str)
                != Some("genesis.expected_hash")
            || genesis.get("file").and_then(toml::Value::as_str)
                != source_root.join("genesis.signed.nrt").to_str()
        {
            return Err(eyre!(
                "generated validator genesis binding differs from its exact source"
            ));
        }
        genesis.remove("expected_hash_file");
        genesis.insert(
            "expected_hash".into(),
            toml::Value::String(network.to_string()),
        );
        genesis.insert(
            "file".into(),
            toml::Value::String(
                genesis_file
                    .to_str()
                    .ok_or_else(|| eyre!("installed genesis path must be UTF-8"))?
                    .to_owned(),
            ),
        );
        let state_root = Path::new("/var/lib/taira").join(validator);
        for (field, relative, destination) in state_paths(peer) {
            let value = field_mut(&mut table, &field)?;
            if value.as_str() != source_root.join(relative).to_str() {
                return Err(eyre!(
                    "generated validator state path differs at `{}`",
                    field.join(".")
                ));
            }
            *value = toml::Value::String(
                state_root
                    .join(destination)
                    .to_str()
                    .ok_or_else(|| eyre!("canonical state path must be UTF-8"))?
                    .to_owned(),
            );
        }
        let mut snapshot = toml::Table::new();
        snapshot.insert("mode".into(), toml::Value::String("read_write".into()));
        snapshot.insert(
            "store_dir".into(),
            toml::Value::String(
                state_root
                    .join("snapshots")
                    .to_str()
                    .expect("fixed UTF-8 state path")
                    .to_owned(),
            ),
        );
        table.insert("snapshot".into(), toml::Value::Table(snapshot));
        let torii = table
            .get_mut("torii")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("generated validator config omits Torii"))?;
        let generated_address = torii
            .get("address")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| eyre!("generated validator config omits its Torii listener"))?;
        let generated_address: iroha_primitives::addr::SocketAddr =
            json::from_value(Value::String(generated_address.to_owned()))
                .map_err(|_| eyre!("generated Torii listener is not a canonical address"))?;
        if torii_bind_address.port() == 0
            || torii_bind_address.port() != generated_address.port()
            || matches!(torii_bind_address, std::net::SocketAddr::V6(address) if address.scope_id() != 0)
        {
            return Err(eyre!(
                "final Torii listener must be unscoped and retain the exact nonzero generated port"
            ));
        }
        torii.insert(
            "address".into(),
            toml::Value::String(
                iroha_primitives::addr::SocketAddr::from(torii_bind_address).to_literal(),
            ),
        );
        let generated_signatures = torii
            .get("operator_signatures")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| eyre!("generated validator omits its HTTP operator binding"))?;
        let expected_source_keys = [toml::Value::String(source_operator_key.to_string())];
        if generated_signatures.len() != 2
            || generated_signatures
                .get("enabled")
                .and_then(toml::Value::as_bool)
                != Some(true)
            || generated_signatures
                .get("allowed_public_keys")
                .and_then(toml::Value::as_array)
                .map(Vec::as_slice)
                != Some(expected_source_keys.as_slice())
        {
            return Err(eyre!(
                "generated validator HTTP operator binding differs from its native signer custody"
            ));
        }
        // The explicit deployment key replaces only the admitted generated HTTP binding.
        // Faucet and onboarding custody continue to reference their generated signers.
        let mut signatures = toml::Table::new();
        signatures.insert("enabled".into(), toml::Value::Boolean(true));
        signatures.insert(
            "allowed_public_keys".into(),
            toml::Value::Array(vec![toml::Value::String(operator_key.to_string())]),
        );
        torii.insert("operator_signatures".into(), toml::Value::Table(signatures));
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize native public validator config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("materialized validator config exceeds its bound"));
        }
        validate_validator_genesis_config(
            rendered.as_bytes(),
            genesis_file,
            &hex::encode(network.as_bytes()),
        )?;
        validate_validator_operator_config(rendered.as_bytes(), &operator_key.to_string())?;
        Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> NetworkId {
        checked_network_id(
            "hash:97507E381726890C14F116C07577A26146286D6B2C2747F902FC08D8FBE4731D#DF02",
        )
        .expect("canonical public test identity")
    }

    fn operator() -> PublicKey {
        iroha_test_samples::ALICE_KEYPAIR.public_key().clone()
    }

    fn source_operator() -> PublicKey {
        iroha_test_samples::BOB_KEYPAIR.public_key().clone()
    }

    fn insert(table: &mut toml::Table, fields: &[&str], value: toml::Value) {
        let (last, parents) = fields.split_last().expect("test field path");
        let mut cursor = table;
        for parent in parents {
            cursor = cursor
                .entry((*parent).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .expect("test table");
        }
        cursor.insert((*last).to_owned(), value);
    }

    fn source(peer: usize) -> toml::Table {
        let mut table = toml::Table::new();
        insert(&mut table, &["chain"], CHAIN_ID.into());
        insert(
            &mut table,
            &["chain_discriminant"],
            i64::from(CHAIN_DISCRIMINANT).into(),
        );
        insert(
            &mut table,
            &["private_key"],
            "opaque-test-value-preserved".into(),
        );
        insert(
            &mut table,
            &["genesis", "file"],
            "/generated/genesis.signed.nrt".into(),
        );
        insert(
            &mut table,
            &["genesis", "expected_hash_file"],
            "genesis.expected_hash".into(),
        );
        insert(
            &mut table,
            &["nexus", "registry", "manifest_directory"],
            "/generated/lane-manifests".into(),
        );
        for field in ["address", "public_address"] {
            insert(
                &mut table,
                &["network", field],
                iroha_primitives::addr::SocketAddr::from((
                    [127, 0, 0, 1],
                    1337 + u16::try_from(peer).expect("four validator fixture ordinals fit u16"),
                ))
                .to_literal()
                .into(),
            );
        }
        insert(
            &mut table,
            &["torii", "address"],
            iroha_primitives::addr::SocketAddr::from((
                [127, 0, 0, 1],
                8080 + u16::try_from(peer).expect("four validator fixture ordinals fit u16"),
            ))
            .to_literal()
            .into(),
        );
        insert(
            &mut table,
            &["torii", "operator_signatures", "enabled"],
            true.into(),
        );
        insert(
            &mut table,
            &["torii", "operator_signatures", "allowed_public_keys"],
            toml::Value::Array(vec![source_operator().to_string().into()]),
        );
        insert(
            &mut table,
            &["torii", "faucet", "private_key_file"],
            "/generated/runtime/ledger-signer.key".into(),
        );
        insert(
            &mut table,
            &["torii", "account_onboarding", "private_key_file"],
            "/generated/runtime/onboarding-signer.key".into(),
        );
        for (fields, relative, _) in state_paths(peer) {
            insert(
                &mut table,
                &fields,
                Path::new("/generated")
                    .join(relative)
                    .to_str()
                    .expect("test path")
                    .into(),
            );
        }
        table
    }

    fn project(table: &toml::Table, role: &str) -> Result<toml::Table> {
        let peer = VALIDATOR_SLUGS
            .iter()
            .position(|candidate| *candidate == role)
            .unwrap_or(0);
        project_with_torii_bind(
            table,
            role,
            (
                [0, 0, 0, 0],
                8080 + u16::try_from(peer).expect("four validator fixture ordinals fit u16"),
            )
                .into(),
        )
    }

    fn project_with_torii_bind(
        table: &toml::Table,
        role: &str,
        torii_bind_address: std::net::SocketAddr,
    ) -> Result<toml::Table> {
        let output = project_config(
            toml::to_string(table)?.as_bytes(),
            Path::new("/generated"),
            role,
            &identity(),
            Path::new("/installed/genesis.json"),
            &source_operator(),
            &operator(),
            torii_bind_address,
        )?;
        Ok(toml::from_str(std::str::from_utf8(&output)?)?)
    }

    #[test]
    fn materialization_binds_every_validator_state_path_and_preserves_other_fields() {
        for (peer, role) in VALIDATOR_SLUGS.iter().enumerate() {
            let original = source(peer);
            let mut actual = project(&original, role).expect("exact generated input");
            for (fields, _, destination) in state_paths(peer) {
                assert_eq!(
                    field_mut(&mut actual, &fields).unwrap().as_str(),
                    Path::new("/var/lib/taira")
                        .join(role)
                        .join(destination)
                        .to_str()
                );
            }
            assert_ne!(source_operator(), operator());
            assert_eq!(
                actual["torii"]["operator_signatures"]["allowed_public_keys"],
                toml::Value::Array(vec![operator().to_string().into()])
            );
            assert_eq!(actual["torii"]["faucet"], original["torii"]["faucet"]);
            assert_eq!(
                actual["torii"]["account_onboarding"],
                original["torii"]["account_onboarding"]
            );
            assert_eq!(actual["private_key"], original["private_key"]);
            assert_eq!(actual["chain"], original["chain"]);
            assert_eq!(
                actual["genesis"]["expected_hash"].as_str(),
                Some(identity().to_string().as_str())
            );
            assert!(actual["genesis"].get("expected_hash_file").is_none());
            assert_eq!(actual["snapshot"]["mode"].as_str(), Some("read_write"));
            assert_eq!(
                actual["snapshot"]["store_dir"].as_str(),
                Path::new("/var/lib/taira")
                    .join(role)
                    .join("snapshots")
                    .to_str()
            );
        }
    }

    #[test]
    fn materialization_projects_split_torii_bind_without_changing_p2p_or_signer_custody() {
        for (peer, role) in VALIDATOR_SLUGS.iter().enumerate() {
            let original = source(peer);
            let projected = project(&original, role).expect("explicit public Torii binding");
            let expected = iroha_primitives::addr::SocketAddr::from((
                [0, 0, 0, 0],
                8080 + u16::try_from(peer).unwrap(),
            ))
            .to_literal();
            assert_eq!(
                projected["torii"]["address"].as_str(),
                Some(expected.as_str())
            );
            assert_eq!(projected["network"], original["network"]);
            assert_eq!(projected["private_key"], original["private_key"]);
            assert_eq!(projected["torii"]["faucet"], original["torii"]["faucet"]);
            assert_eq!(
                projected["torii"]["account_onboarding"],
                original["torii"]["account_onboarding"]
            );
        }
    }

    #[test]
    fn materialization_rejects_invalid_torii_listener_and_port_drift() {
        for port in [0, 8081] {
            let error = project_with_torii_bind(
                &source(0),
                VALIDATOR_SLUGS[0],
                ([0, 0, 0, 0], port).into(),
            )
            .unwrap_err();
            assert!(error.to_string().contains("exact nonzero generated port"));
        }
        for value in [
            toml::Value::String("127.0.0.1:8080".into()),
            toml::Value::String("addr:127.0.0.1:8080#0000".into()),
            toml::Value::String(
                iroha_primitives::addr::SocketAddr::from(([127, 0, 0, 1], 0)).to_literal(),
            ),
            toml::Value::Integer(8080),
        ] {
            let mut changed = source(0);
            insert(&mut changed, &["torii", "address"], value);
            assert!(project(&changed, VALIDATOR_SLUGS[0]).is_err());
        }
        let mut missing = source(0);
        missing["torii"].as_table_mut().unwrap().remove("address");
        assert!(project(&missing, VALIDATOR_SLUGS[0]).is_err());
    }

    #[test]
    fn materialization_torii_bind_argument_requires_canonical_ip_and_nonzero_port() {
        for good in ["0.0.0.0:8080", "127.0.0.1:8083", "[::]:8080"] {
            assert_eq!(checked_torii_bind_address(good).unwrap().to_string(), good);
        }
        for bad in [
            "",
            "0.0.0.0",
            "0.0.0.0:0",
            "0.0.0.0:65536",
            "0.0.0.0:08080",
            "localhost:8080",
            "http://0.0.0.0:8080/",
            " 0.0.0.0:8080",
            "0.0.0.0:8080 ",
            "[0:0:0:0:0:0:0:0]:8080",
            "[fe80::1%2]:8080",
        ] {
            assert!(checked_torii_bind_address(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn materialization_rejects_changed_missing_and_wrong_peer_state_paths() {
        for (fields, _, _) in state_paths(0) {
            let mut changed = source(0);
            *field_mut(&mut changed, &fields).unwrap() = "/other/private-state".into();
            assert!(
                project(&changed, VALIDATOR_SLUGS[0]).is_err(),
                "{}",
                fields.join(".")
            );
            *field_mut(&mut changed, &fields).unwrap() = toml::Value::Integer(1);
            assert!(project(&changed, VALIDATOR_SLUGS[0]).is_err());
            let (last, parents) = fields.split_last().unwrap();
            let mut cursor = &mut changed;
            for parent in parents {
                cursor = cursor.get_mut(*parent).unwrap().as_table_mut().unwrap();
            }
            cursor.remove(*last);
            assert!(project(&changed, VALIDATOR_SLUGS[0]).is_err());
        }
        assert!(project(&source(1), VALIDATOR_SLUGS[0]).is_err());
        assert!(project(&source(0), "taira-validator-5").is_err());
    }

    #[test]
    fn materialization_rejects_inheritance_identity_drift_and_source_bindings() {
        for (path, value) in [
            (
                vec!["extends"],
                toml::Value::String("elsewhere.toml".into()),
            ),
            (vec!["chain"], toml::Value::String("other-chain".into())),
            (vec!["chain_discriminant"], toml::Value::Integer(1)),
            (
                vec!["genesis", "file"],
                toml::Value::String("/other/genesis.signed.nrt".into()),
            ),
            (
                vec!["genesis", "expected_hash_file"],
                toml::Value::String("other.hash".into()),
            ),
            (
                vec!["genesis", "expected_hash"],
                toml::Value::String(identity().to_string()),
            ),
            (
                vec!["genesis", "manifest_json"],
                toml::Value::String("unbound.json".into()),
            ),
            (
                vec!["nexus", "registry"],
                toml::Value::Table(toml::Table::new()),
            ),
            (
                vec!["nexus", "registry", "manifest_directory"],
                toml::Value::String("/other/lane-manifests".into()),
            ),
            (
                vec!["nexus", "registry", "cache_directory"],
                toml::Value::String("/generated/cache".into()),
            ),
            (vec!["snapshot"], toml::Value::Table(toml::Table::new())),
            (
                vec!["torii", "operator_signatures"],
                toml::Value::Table(toml::Table::new()),
            ),
        ] {
            let mut changed = source(0);
            insert(&mut changed, &path, value);
            assert!(
                project(&changed, VALIDATOR_SLUGS[0]).is_err(),
                "{}",
                path.join(".")
            );
        }
        let signatures_path = ["torii", "operator_signatures"];
        for (field, value) in [
            ("enabled", toml::Value::Boolean(false)),
            ("enabled", toml::Value::String("private-test-marker".into())),
            ("allowed_public_keys", toml::Value::Array(vec![])),
            (
                "allowed_public_keys",
                toml::Value::String(source_operator().to_string()),
            ),
            (
                "allowed_public_keys",
                toml::Value::Array(vec![operator().to_string().into()]),
            ),
            (
                "allowed_public_keys",
                toml::Value::Array(vec!["invalid-key".into()]),
            ),
            (
                "allowed_public_keys",
                toml::Value::Array(vec![
                    source_operator().to_string().into(),
                    source_operator().to_string().into(),
                ]),
            ),
            ("allow_node_key", toml::Value::Boolean(false)),
            ("extra", toml::Value::Boolean(true)),
        ] {
            let mut changed = source(0);
            insert(
                &mut changed,
                &["torii", "operator_signatures", field],
                value,
            );
            let error = project(&changed, VALIDATOR_SLUGS[0]).unwrap_err();
            assert!(
                error.to_string().contains("native signer custody"),
                "{field}"
            );
            assert!(!error.to_string().contains("private-test-marker"));
        }
        for field in ["enabled", "allowed_public_keys"] {
            let mut changed = source(0);
            field_mut(&mut changed, &signatures_path)
                .unwrap()
                .as_table_mut()
                .unwrap()
                .remove(field);
            assert!(project(&changed, VALIDATOR_SLUGS[0]).is_err(), "{field}");
        }
        let mut missing = source(0);
        missing["torii"]
            .as_table_mut()
            .unwrap()
            .remove("operator_signatures");
        assert!(project(&missing, VALIDATOR_SLUGS[0]).is_err());
        let projected = project(&source(0), VALIDATOR_SLUGS[0]).unwrap();
        assert!(project(&projected, VALIDATOR_SLUGS[0]).is_err());
    }

    #[test]
    fn materialization_requires_exact_public_genesis_identity_bytes() {
        let network = identity();
        let canonical = format!("{network}\n");
        validate_generated_identity(canonical.as_bytes(), &network).unwrap();
        for changed in [
            network.to_string(),
            format!("{network}\r\n"),
            format!("{canonical}{canonical}"),
            format!("{}\n", hex::encode(network.as_bytes())),
            canonical.to_lowercase(),
            String::new(),
        ] {
            assert!(validate_generated_identity(changed.as_bytes(), &network).is_err());
        }
    }

    #[test]
    fn materialization_cli_requires_explicit_custody_and_canonical_identities() {
        #[derive(clap::Parser)]
        struct Command {
            #[command(flatten)]
            args: MaterializeValidatorConfig,
        }
        let arguments = vec![
            "materialize-validator-config".to_owned(),
            "--config-fd".to_owned(),
            "3".to_owned(),
            "--localnet-dir".to_owned(),
            "/generated".to_owned(),
            "--validator".to_owned(),
            VALIDATOR_SLUGS[0].to_owned(),
            "--network-id".to_owned(),
            identity().to_string(),
            "--genesis-file".to_owned(),
            "/installed/genesis.json".to_owned(),
            "--operator-public-key".to_owned(),
            operator().to_string(),
            "--torii-bind-address".to_owned(),
            "0.0.0.0:8080".to_owned(),
            "--output".to_owned(),
            "/private/output.toml".to_owned(),
        ];
        let parsed = <Command as clap::Parser>::try_parse_from(&arguments).unwrap();
        assert_eq!(parsed.args.validator, VALIDATOR_SLUGS[0]);
        assert_eq!(parsed.args.network_id, identity());
        assert_eq!(parsed.args.torii_bind_address, ([0, 0, 0, 0], 8080).into());
        let full_arguments = ["iroha", "taira", "public-reset"]
            .into_iter()
            .map(str::to_owned)
            .chain(arguments.iter().cloned());
        let full = <crate::Args as clap::Parser>::try_parse_from(full_arguments)
            .expect("native command graph admits local descriptor custody");
        assert!(full.config_fd.is_none());
        crate::reject_irrelevant_taira_public_reset_globals(&full)
            .expect("local config descriptor is not a global client signer");
        for flag in (1..arguments.len()).step_by(2) {
            let mut missing = arguments.clone();
            missing.drain(flag..flag + 2);
            assert!(<Command as clap::Parser>::try_parse_from(missing).is_err());
        }
        for (index, bad) in [
            (2, "2"),
            (2, "65536"),
            (6, "taira-validator-5"),
            (8, "unchecked-network"),
            (12, "invalid-public-key"),
            (14, "0.0.0.0:0"),
            (14, "localhost:8080"),
            (14, "http://0.0.0.0:8080/"),
            (14, "0.0.0.0:08080"),
        ] {
            let mut changed = arguments.clone();
            changed[index] = bad.to_owned();
            assert!(<Command as clap::Parser>::try_parse_from(changed).is_err());
        }
    }
}
