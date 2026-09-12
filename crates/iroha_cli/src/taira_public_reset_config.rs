//! Native custody for validator configuration and dedicated operator signing keys.

use super::*;
use iroha::data_model::NetworkId;
use zeroize::Zeroizing;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;

#[derive(clap::Args, Debug)]
pub(super) struct ConfigRebase {
    /// Inherited owner-controlled regular config descriptor; contents never enter argv/stdout.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Exact current genesis.file value; drift fails before creating output.
    #[arg(long, value_name = "PATH")]
    expected_genesis_file: PathBuf,
    /// Replace genesis.file with this absolute path.
    #[arg(long, value_name = "PATH")]
    genesis_file: PathBuf,
    /// Exact current inline checked genesis identity; paired with --network-id.
    #[arg(long, value_name = "NETWORK_ID", requires = "network_id", value_parser = canonical_network_id)]
    expected_network_id: Option<NetworkId>,
    /// Explicit checked identity of the new genesis; paired with --expected-network-id.
    #[arg(long, value_name = "NETWORK_ID", requires = "expected_network_id", value_parser = canonical_network_id)]
    network_id: Option<NetworkId>,
    /// Enable operator signatures with exactly this dedicated canonical Ed25519 public key.
    #[arg(long, value_name = "PUBLIC_KEY", value_parser = canonical_operator_public_key)]
    operator_public_key: Option<PublicKey>,
    /// Fresh 0600 config in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct ClientConfigRebase {
    /// Inherited owner-controlled regular config descriptor; contents never enter argv/stdout.
    #[arg(long, value_name = "FD", value_parser = clap::value_parser!(u32).range(3..=65535))]
    config_fd: u32,
    /// Exact current inline checked network identity; drift fails before creating output.
    #[arg(long, value_name = "NETWORK_ID", value_parser = canonical_network_id)]
    expected_network_id: NetworkId,
    /// Explicit checked identity of the new genesis.
    #[arg(long, value_name = "NETWORK_ID", value_parser = canonical_network_id)]
    network_id: NetworkId,
    /// Fresh 0600 config in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

#[derive(clap::Args, Debug)]
pub(super) struct OperatorKeygen {
    /// Fresh absolute runtime key file in a direct owner-only directory outside repositories.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
}

fn canonical_operator_public_key(value: &str) -> Result<PublicKey, String> {
    let key = value
        .parse::<PublicKey>()
        .map_err(|_| "operator public key must be canonical Ed25519".to_owned())?;
    if key.try_algorithm().ok() != Some(Algorithm::Ed25519) || key.to_string() != value {
        return Err("operator public key must be canonical Ed25519".to_owned());
    }
    Ok(key)
}

fn canonical_network_id(value: &str) -> Result<NetworkId, String> {
    let network = value
        .parse::<NetworkId>()
        .map_err(|_| "network identity must be canonical checked NetworkId".to_owned())?;
    if network.to_string() != value {
        return Err("network identity must be canonical checked NetworkId".to_owned());
    }
    Ok(network)
}

/// Generate one runtime credential and print only its public identity and path.
pub(super) fn operator_keygen(args: &OperatorKeygen, writer: &mut impl Write) -> Result<()> {
    #[cfg(unix)]
    {
        operator_keygen_unix(args, writer)
    }
    #[cfg(not(unix))]
    {
        let _ = (args, writer);
        Err(eyre!(
            "operator key generation requires Unix private-file custody"
        ))
    }
}

#[cfg(unix)]
fn operator_keygen_unix(args: &OperatorKeygen, writer: &mut impl Write) -> Result<()> {
    use iroha_crypto::{ExposedPrivateKey, KeyPair};
    use rustix::fs::{Mode, OFlags};

    let path = &args.private_key_file;
    validate_absolute_normal_path(path, "operator private-key path")?;
    let path_text = path
        .to_str()
        .ok_or_else(|| eyre!("operator private-key path must be UTF-8"))?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("operator private-key path has no parent"))?;
    validate_owner_private_dir(parent, "operator private-key directory")?;
    for ancestor in parent.ancestors() {
        match fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => {
                return Err(eyre!(
                    "operator private-key path must be outside repositories"
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(eyre!("cannot establish operator key repository exclusion")),
        }
    }
    let directory = File::from(rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let held_parent = directory.metadata()?;
    let named_parent = fs::symlink_metadata(parent)?;
    if held_parent.dev() != named_parent.dev()
        || held_parent.ino() != named_parent.ino()
        || held_parent.uid() != rustix::process::geteuid().as_raw()
        || held_parent.mode() & 0o7777 != 0o700
    {
        return Err(eyre!("operator private-key directory changed during open"));
    }
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("operator private-key path has no file name"))?;
    let mut file = File::from(
        rustix::fs::openat(
            &directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| eyre!("cannot create fresh operator private-key file"))?,
    );
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    let key = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
        .map_err(|_| eyre!("native operator key generation failed"))?;
    let encoded = Zeroizing::new(
        ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .map_err(|_| eyre!("cannot encode operator private key"))?,
    );
    file.write_all(encoded.as_bytes())
        .wrap_err("cannot write operator private-key file")?;
    file.write_all(b"\n")
        .wrap_err("cannot terminate operator private-key file")?;
    file.sync_all()
        .wrap_err("cannot synchronize operator private-key file")?;
    directory.sync_all()?;
    validate_owner_private_dir(parent, "operator private-key directory")?;
    let named_parent = fs::symlink_metadata(parent)?;
    let named = fs::symlink_metadata(path)?;
    let held = file.metadata()?;
    if held_parent.dev() != named_parent.dev()
        || held_parent.ino() != named_parent.ino()
        || named.dev() != held.dev()
        || named.ino() != held.ino()
        || !named.is_file()
        || named.file_type().is_symlink()
        || named.uid() != rustix::process::geteuid().as_raw()
        || named.mode() & 0o7777 != 0o600
        || named.nlink() != 1
        || named.len() != encoded.len() as u64 + 1
    {
        return Err(eyre!(
            "operator private-key custody changed during publication"
        ));
    }
    let mut report = Map::new();
    report.insert(
        "schema".into(),
        Value::String("iroha.taira.public-reset.operator-key.v1".into()),
    );
    report.insert(
        "public_key".into(),
        Value::String(key.public_key().to_string()),
    );
    report.insert("path".into(), Value::String(path_text.into()));
    writeln!(writer, "{}", json::to_json(&Value::Object(report))?)?;
    Ok(())
}

pub(super) fn config_rebase(args: &ConfigRebase) -> Result<()> {
    validate_absolute_normal_path(&args.expected_genesis_file, "expected genesis path")?;
    validate_absolute_normal_path(&args.genesis_file, "new genesis path")?;
    let network_rebind = match (&args.expected_network_id, &args.network_id) {
        (None, None) => None,
        (Some(expected), Some(replacement)) => Some((expected, replacement)),
        _ => {
            return Err(eyre!(
                "validator identity rebind requires paired expected and new NetworkId"
            ));
        }
    };
    let source = inherited_config(args.config_fd)?;
    let output = rebase_genesis_file(
        &source,
        &args.expected_genesis_file,
        &args.genesis_file,
        network_rebind,
        args.operator_public_key.as_ref(),
    )?;
    super::inputs::write_new_private(&args.output, &output)
}

pub(super) fn client_config_rebase(args: &ClientConfigRebase) -> Result<()> {
    let source = crate::client_config::read_inherited_private_file(
        args.config_fd,
        MAX_CONFIG_BYTES,
        "client config",
    )?;
    let output = rebase_client_network_id(&source, &args.expected_network_id, &args.network_id)?;
    super::inputs::write_new_private(&args.output, &output)
}

fn inherited_config(fd: u32) -> Result<Zeroizing<Vec<u8>>> {
    crate::client_config::read_inherited_private_file(fd, MAX_CONFIG_BYTES, "validator config")
}

fn rebase_genesis_file(
    bytes: &[u8],
    expected: &Path,
    replacement: &Path,
    network_rebind: Option<(&NetworkId, &NetworkId)>,
    operator_public_key: Option<&PublicKey>,
) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(eyre!("validator config exceeds its materialization bound"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("validator config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("validator config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends") {
            return Err(eyre!(
                "validator config cannot inherit an unbound TOML source"
            ));
        }
        let genesis = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("validator config omits its genesis table"))?;
        let expected = expected
            .to_str()
            .ok_or_else(|| eyre!("expected genesis path is not UTF-8"))?;
        if genesis.get("file").and_then(toml::Value::as_str) != Some(expected) {
            return Err(eyre!(
                "validator genesis.file differs from the explicitly retained path"
            ));
        }
        if let Some((expected, replacement)) = network_rebind {
            if genesis.contains_key("expected_hash_file") {
                return Err(eyre!(
                    "validator identity rebind cannot use genesis.expected_hash_file"
                ));
            }
            rebind_inline_network_id(genesis, "expected_hash", expected, replacement)?;
        }
        let replacement = replacement
            .to_str()
            .ok_or_else(|| eyre!("new genesis path is not UTF-8"))?;
        genesis.insert(
            "file".to_owned(),
            toml::Value::String(replacement.to_owned()),
        );
        if let Some(key) = operator_public_key {
            canonical_operator_public_key(&key.to_string()).map_err(|error| eyre!(error))?;
            let torii = table
                .entry("torii")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| eyre!("validator torii configuration must be a table"))?;
            let signatures = torii
                .entry("operator_signatures")
                .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                .as_table_mut()
                .ok_or_else(|| eyre!("validator operator_signatures must be a table"))?;
            signatures.insert("enabled".to_owned(), toml::Value::Boolean(true));
            signatures.insert(
                "allowed_public_keys".to_owned(),
                toml::Value::Array(vec![toml::Value::String(key.to_string())]),
            );
        }
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize validator config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("materialized validator config exceeds its bound"));
        }
        Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

fn rebind_inline_network_id(
    table: &mut toml::Table,
    key: &str,
    expected: &NetworkId,
    replacement: &NetworkId,
) -> Result<()> {
    let literal = table
        .get(key)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("config requires its current inline checked network identity"))?;
    let actual = canonical_network_id(literal)
        .map_err(|_| eyre!("config inline network identity is not canonical checked NetworkId"))?;
    if &actual != expected {
        return Err(eyre!(
            "config network identity differs from the explicitly retained identity"
        ));
    }
    table.insert(key.to_owned(), toml::Value::String(replacement.to_string()));
    Ok(())
}

fn rebase_client_network_id(
    bytes: &[u8],
    expected: &NetworkId,
    replacement: &NetworkId,
) -> Result<Zeroizing<Vec<u8>>> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(eyre!("client config exceeds its materialization bound"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| eyre!("client config is not UTF-8"))?;
    let mut table: toml::Table =
        toml::from_str(text).map_err(|_| eyre!("client config is not valid TOML"))?;
    let result = (|| {
        if table.contains_key("extends") || table.contains_key("network_id_file") {
            return Err(eyre!(
                "client identity rebind cannot use extends or network_id_file"
            ));
        }
        rebind_inline_network_id(&mut table, "network_id", expected, replacement)?;
        let rendered = Zeroizing::new(
            toml::to_string_pretty(&table)
                .map_err(|_| eyre!("cannot materialize client config"))?,
        );
        if rendered.len() as u64 > MAX_CONFIG_BYTES {
            return Err(eyre!("materialized client config exceeds its bound"));
        }
        Ok(Zeroizing::new(rendered.as_bytes().to_vec()))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = b"private_key = 'fixture-secret-not-runtime'\nsoranet_transport_private_key = 'transport-fixture'\n[genesis]\nfile = '/retained/genesis.nrt'\nexpected_hash = 'public-hash'\n[streaming]\nidentity_private_key = 'streaming-fixture'\n";

    fn network_fixture(seed: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
            seed,
        )))
    }

    fn validator_network_fixture(network: &NetworkId) -> Vec<u8> {
        String::from_utf8(FIXTURE.to_vec())
            .unwrap()
            .replace("public-hash", &network.to_string())
            .into_bytes()
    }

    fn client_network_fixture(network: &NetworkId) -> Vec<u8> {
        format!("network_id = '{network}'\ntorii_url = 'https://taira.sora.org'\n[account]\nprivate_key = 'fixture-secret-not-runtime'\npublic_key = 'retained-public-fixture'\n").into_bytes()
    }

    #[test]
    fn config_rebase_network_identity_uses_exact_cas_and_preserves_other_config() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = validator_network_fixture(&old);
        let output = rebase_genesis_file(
            &source,
            Path::new("/retained/genesis.nrt"),
            Path::new("/installed/genesis.nrt"),
            Some((&old, &next)),
            None,
        )
        .unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(&source).unwrap()).unwrap();
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "file".to_owned(),
                toml::Value::String("/installed/genesis.nrt".to_owned()),
            );
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "expected_hash".to_owned(),
                toml::Value::String(next.to_string()),
            );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(
            rebase_genesis_file(
                &source,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                Some((&next, &old)),
                None
            )
            .is_err()
        );
        assert!(
            rebase_genesis_file(
                &output,
                Path::new("/installed/genesis.nrt"),
                Path::new("/other/genesis.nrt"),
                Some((&old, &next)),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn config_rebase_network_identity_rejects_competing_and_noncanonical_sources() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = String::from_utf8(validator_network_fixture(&old)).unwrap();
        for invalid in [
            format!("extends = '/unbound/fixture.toml'\n{source}"),
            source.replace(
                "[genesis]",
                "[genesis]\nexpected_hash_file = '/unbound/network-id'",
            ),
            source.replace(&format!("expected_hash = '{old}'"), ""),
            source.replace(&old.to_string(), "fixture-secret-not-runtime"),
            source.replace(&old.to_string(), &format!(" {old}")),
        ] {
            let error = rebase_genesis_file(
                invalid.as_bytes(),
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                Some((&old, &next)),
                None,
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[test]
    fn config_rebase_network_identity_cli_requires_paired_checked_values() {
        use clap::Parser as _;
        let old = network_fixture(b"retained test genesis").to_string();
        let next = network_fixture(b"replacement test genesis").to_string();
        let base = [
            "iroha",
            "taira",
            "public-reset",
            "config-rebase",
            "--config-fd",
            "3",
            "--expected-genesis-file",
            "/retained/genesis.nrt",
            "--genesis-file",
            "/installed/genesis.nrt",
            "--output",
            "/private/runtime/validator.toml",
        ];
        assert!(crate::Args::try_parse_from(base).is_ok());
        let mut paired = base.to_vec();
        paired.extend(["--expected-network-id", &old, "--network-id", &next]);
        assert!(crate::Args::try_parse_from(paired).is_ok());
        for tail in [
            vec!["--expected-network-id", old.as_str()],
            vec!["--network-id", next.as_str()],
            vec![
                "--expected-network-id",
                old.as_str(),
                "--network-id",
                "raw-unchecked-hash",
            ],
        ] {
            let mut args = base.to_vec();
            args.extend(tail);
            assert!(crate::Args::try_parse_from(args).is_err());
        }
        for invalid in [
            format!(" {old}"),
            format!("{old}\n"),
            "raw-unchecked-hash".to_owned(),
        ] {
            assert!(canonical_network_id(&invalid).is_err());
        }
        assert!(
            crate::Args::try_parse_from([
                "iroha",
                "taira",
                "public-reset",
                "client-config-rebase",
                "--config-fd",
                "3",
                "--expected-network-id",
                &old,
                "--network-id",
                &next,
                "--output",
                "/private/runtime/client.toml"
            ])
            .is_ok()
        );
    }

    #[test]
    fn client_config_rebase_network_identity_uses_exact_cas_and_preserves_other_config() {
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = client_network_fixture(&old);
        let output = rebase_client_network_id(&source, &old, &next).unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(&source).unwrap()).unwrap();
        expected.insert(
            "network_id".to_owned(),
            toml::Value::String(next.to_string()),
        );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        assert!(rebase_client_network_id(&source, &next, &old).is_err());
        assert!(rebase_client_network_id(&output, &old, &next).is_err());
        for invalid in [
            [
                b"extends = '/unbound/config.toml'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            [
                b"network_id_file = '/unbound/network-id'\n".as_slice(),
                source.as_slice(),
            ]
            .concat(),
            b"network_id = 'fixture-secret-not-runtime'\n".to_vec(),
            b"private_key = 'fixture-secret-not-runtime'\n".to_vec(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
        ] {
            let error = rebase_client_network_id(&invalid, &old, &next).unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn client_config_rebase_inherited_fd_preserves_custody_and_has_no_stdout() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("client-config-rebase-");
        let root = directory.path().canonicalize().unwrap();
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = root.join("source.toml");
        let bytes = client_network_fixture(&old);
        fs::write(&source, &bytes).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let mut file = File::open(&source).unwrap();
        file.seek(std::io::SeekFrom::Start(7)).unwrap();
        let output = root.join("rebound.toml");
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ClientConfigRebase(ClientConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_network_id: old,
                network_id: next,
                output: output.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(file.stream_position().unwrap(), 7);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        let published = fs::read(&output).unwrap();
        let table: toml::Table = toml::from_str(std::str::from_utf8(&published).unwrap()).unwrap();
        assert_eq!(
            table["network_id"].as_str(),
            Some(next.to_string().as_str())
        );
        let metadata = fs::symlink_metadata(&output).unwrap();
        assert_eq!(metadata.mode() & 0o7777, 0o600);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
        assert!(command.run_without_client_config(&mut stdout).is_err());
        assert_eq!(fs::read(&output).unwrap(), published);
        assert!(stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn client_config_rebase_rejects_drift_and_unsafe_custody_before_output() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("client-config-rebase-refusal-");
        let root = directory.path().canonicalize().unwrap();
        let old = network_fixture(b"retained test genesis");
        let next = network_fixture(b"replacement test genesis");
        let source = root.join("source.toml");
        fs::write(&source, client_network_fixture(&old)).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&source).unwrap();
        let output = root.join("rebound.toml");
        let mut args = ClientConfigRebase {
            config_fd: file.as_raw_fd() as u32,
            expected_network_id: next,
            network_id: old,
            output: output.clone(),
        };
        assert!(client_config_rebase(&args).is_err());
        assert!(!output.exists());
        args.expected_network_id = old;
        args.network_id = next;
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(client_config_rebase(&args).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&source, root.join("linked.toml")).unwrap();
        assert!(client_config_rebase(&args).is_err());
        assert!(!output.exists());
        fs::remove_file(root.join("linked.toml")).unwrap();
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o755)).unwrap();
        args.output = unsafe_parent.join("output.toml");
        assert!(client_config_rebase(&args).is_err());
        assert!(!args.output.exists());
        std::os::unix::fs::symlink(&source, &output).unwrap();
        args.output = output;
        assert!(client_config_rebase(&args).is_err());
        assert_eq!(fs::read(&source).unwrap(), client_network_fixture(&old));
    }

    #[test]
    fn config_rebase_operator_key_is_canonical_and_changes_only_explicit_operator_fields() {
        let key = iroha_crypto::KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519).unwrap();
        let canonical = key.public_key().to_string();
        assert_eq!(
            canonical_operator_public_key(&canonical).unwrap(),
            *key.public_key()
        );
        for invalid in [
            format!(" {canonical}"),
            format!("{canonical}\n"),
            "private-fixture".into(),
        ] {
            let error = canonical_operator_public_key(&invalid).unwrap_err();
            assert!(!error.contains(&invalid));
        }
        let source = [
            FIXTURE,
            b"[torii]\naddress = '127.0.0.1:8080'\n[torii.operator_signatures]\nenabled = false\nallowed_public_keys = ['retired-public-fixture']\nallow_node_key = false\nnonce_ttl_secs = 120\n",
        ]
        .concat();
        for source in [FIXTURE, source.as_slice()] {
            let output = rebase_genesis_file(
                source,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                None,
                Some(key.public_key()),
            )
            .unwrap();
            let mut actual: toml::Table =
                toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
            let mut original: toml::Table =
                toml::from_str(std::str::from_utf8(source).unwrap()).unwrap();
            let operators = actual
                .get_mut("torii")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .get_mut("operator_signatures")
                .unwrap()
                .as_table_mut()
                .unwrap();
            assert_eq!(
                operators.remove("enabled"),
                Some(toml::Value::Boolean(true))
            );
            assert_eq!(
                operators.remove("allowed_public_keys"),
                Some(toml::Value::Array(vec![toml::Value::String(
                    canonical.clone()
                )]))
            );
            if let Some(torii) = original.get_mut("torii") {
                let operators = torii
                    .as_table_mut()
                    .unwrap()
                    .get_mut("operator_signatures")
                    .unwrap()
                    .as_table_mut()
                    .unwrap();
                operators.remove("enabled");
                operators.remove("allowed_public_keys");
            } else {
                actual.remove("torii");
            }
            actual
                .get_mut("genesis")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .insert(
                    "file".into(),
                    toml::Value::String("/retained/genesis.nrt".into()),
                );
            assert_eq!(actual, original);
        }
        for malformed in [
            [
                b"torii = 'fixture-secret-not-runtime'\n".as_slice(),
                FIXTURE,
            ]
            .concat(),
            [
                FIXTURE,
                b"[torii]\noperator_signatures = 'fixture-secret-not-runtime'\n",
            ]
            .concat(),
        ] {
            let error = rebase_genesis_file(
                &malformed,
                Path::new("/retained/genesis.nrt"),
                Path::new("/installed/genesis.nrt"),
                None,
                Some(key.public_key()),
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
        }
    }

    #[cfg(unix)]
    fn operator_runtime_fixture() -> tempfile::TempDir {
        // Real key generation must pass the production repository exclusion, including in tests.
        let home = std::env::var_os("HOME").expect("native operator test home");
        let root = Path::new(&home).canonicalize().unwrap();
        let directory = tempfile::Builder::new()
            .prefix("iroha-operator-key-test-")
            .tempdir_in(root)
            .unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        directory
    }

    #[cfg(unix)]
    #[test]
    fn operator_keygen_publishes_canonical_private_key_and_only_public_report() {
        let directory = operator_runtime_fixture();
        let mut public_keys = Vec::new();
        for name in ["first.key", "second.key"] {
            let path = directory.path().join(name);
            let mut stdout = Vec::new();
            operator_keygen(
                &OperatorKeygen {
                    private_key_file: path.clone(),
                },
                &mut stdout,
            )
            .unwrap();
            let report: Value = json::from_slice(&stdout).unwrap();
            let object = report.as_object().unwrap();
            assert_eq!(object.len(), 3);
            assert_eq!(
                report.get("schema").and_then(Value::as_str),
                Some("iroha.taira.public-reset.operator-key.v1")
            );
            assert_eq!(report.get("path").and_then(Value::as_str), path.to_str());
            let loaded = crate::operator_key::load_operator_key_pair(&path).unwrap();
            assert_eq!(
                loaded.public_key().try_algorithm().unwrap(),
                Algorithm::Ed25519
            );
            assert_eq!(
                report.get("public_key").and_then(Value::as_str),
                Some(loaded.public_key().to_string().as_str())
            );
            let encoded = Zeroizing::new(fs::read(&path).unwrap());
            assert_eq!(encoded.last(), Some(&b'\n'));
            assert!(
                !stdout
                    .windows(encoded.len() - 1)
                    .any(|window| window == &encoded[..encoded.len() - 1])
            );
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file());
            assert_eq!(metadata.uid(), rustix::process::geteuid().as_raw());
            assert_eq!(metadata.mode() & 0o7777, 0o600);
            assert_eq!(metadata.nlink(), 1);
            public_keys.push(loaded.public_key().clone());
            let mut refused_stdout = Vec::new();
            assert!(
                operator_keygen(
                    &OperatorKeygen {
                        private_key_file: path.clone()
                    },
                    &mut refused_stdout
                )
                .is_err()
            );
            assert!(refused_stdout.is_empty());
            assert!(Zeroizing::new(fs::read(path).unwrap()).as_slice() == encoded.as_slice());
        }
        assert_ne!(public_keys[0], public_keys[1]);
    }

    #[cfg(unix)]
    #[test]
    fn operator_keygen_rejects_repository_existing_symlink_and_unsafe_parent_paths() {
        use std::os::unix::fs::symlink;
        let directory = operator_runtime_fixture();
        let root = directory.path();
        let existing = root.join("existing.key");
        fs::write(&existing, b"fixture-must-remain").unwrap();
        fs::set_permissions(&existing, fs::Permissions::from_mode(0o600)).unwrap();
        let link = root.join("link.key");
        symlink(&existing, &link).unwrap();
        let unsafe_parent = root.join("unsafe");
        fs::create_dir(&unsafe_parent).unwrap();
        fs::set_permissions(&unsafe_parent, fs::Permissions::from_mode(0o755)).unwrap();
        let indirect_parent = root.join("indirect");
        symlink(root, &indirect_parent).unwrap();
        let repository = root.join("repository");
        fs::create_dir(&repository).unwrap();
        fs::set_permissions(&repository, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(repository.join(".git")).unwrap();
        for path in [
            PathBuf::from("relative.key"),
            existing.clone(),
            link,
            unsafe_parent.join("operator.key"),
            indirect_parent.join("operator.key"),
            repository.join("operator.key"),
        ] {
            let mut stdout = Vec::new();
            assert!(
                operator_keygen(
                    &OperatorKeygen {
                        private_key_file: path.clone()
                    },
                    &mut stdout
                )
                .is_err()
            );
            assert!(stdout.is_empty());
            if path != existing && !path.is_symlink() {
                assert!(!path.exists());
            }
        }
        assert_eq!(fs::read(existing).unwrap(), b"fixture-must-remain");
        // A worktree .git file is a repository boundary too, not only a .git directory.
        fs::write(root.join(".git"), b"gitdir: fixture-only").unwrap();
        let path = root.join("worktree.key");
        assert!(
            operator_keygen(
                &OperatorKeygen {
                    private_key_file: path.clone()
                },
                &mut Vec::new()
            )
            .is_err()
        );
        assert!(!path.exists());
    }

    #[test]
    fn config_rebase_changes_only_genesis_file_and_keeps_errors_secret_free() {
        let output = rebase_genesis_file(
            FIXTURE,
            Path::new("/retained/genesis.nrt"),
            Path::new("/installed/genesis.nrt"),
            None,
            None,
        )
        .unwrap();
        let mut expected: toml::Table =
            toml::from_str(std::str::from_utf8(FIXTURE).unwrap()).unwrap();
        expected
            .get_mut("genesis")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert(
                "file".to_owned(),
                toml::Value::String("/installed/genesis.nrt".to_owned()),
            );
        let actual: toml::Table = toml::from_str(std::str::from_utf8(&output).unwrap()).unwrap();
        assert_eq!(actual, expected);
        for bytes in [
            FIXTURE.to_vec(),
            [b"extends='/unbound'\n".as_slice(), FIXTURE].concat(),
            b"private_key = 'fixture-secret-not-runtime\n".to_vec(),
        ] {
            let error = rebase_genesis_file(
                &bytes,
                Path::new("/wrong/path"),
                Path::new("/installed/genesis.nrt"),
                None,
                None,
            )
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
            assert!(!format!("{error:#}").contains("transport-fixture"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_inherited_fd_publishes_private_file_without_stdout_or_overwrite() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-");
        let root = directory.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let source = root.join("source.toml");
        fs::write(&source, FIXTURE).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&source).unwrap();
        let destination = root.join("materialized.toml");
        let key = iroha_crypto::KeyPair::try_from_seed(vec![0x54; 32], Algorithm::Ed25519).unwrap();
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ConfigRebase(ConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_genesis_file: PathBuf::from("/retained/genesis.nrt"),
                genesis_file: PathBuf::from("/installed/genesis.nrt"),
                expected_network_id: None,
                network_id: None,
                operator_public_key: Some(key.public_key().clone()),
                output: destination.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(destination.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(fs::read(&source).unwrap(), FIXTURE);
        let before = fs::read(&destination).unwrap();
        let materialized: toml::Table =
            toml::from_str(std::str::from_utf8(&before).unwrap()).unwrap();
        let signatures = &materialized["torii"]["operator_signatures"];
        assert_eq!(signatures["enabled"].as_bool(), Some(true));
        assert_eq!(
            signatures["allowed_public_keys"].as_array().unwrap(),
            &vec![toml::Value::String(key.public_key().to_string())]
        );
        assert!(command.run_without_client_config(&mut stdout).is_err());
        assert_eq!(fs::read(destination).unwrap(), before);
        assert!(stdout.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_rejects_drift_malformed_input_and_unsafe_descriptors_before_output() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-refusal-");
        let source = directory.path().join("source.toml");
        let output = directory.path().join("output.toml");
        for bytes in [FIXTURE, b"private_key = 'fixture-secret-not-runtime\n"] {
            fs::write(&source, bytes).unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
            let file = File::open(&source).unwrap();
            let error = config_rebase(&ConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_genesis_file: PathBuf::from("/wrong/path"),
                genesis_file: PathBuf::from("/installed/genesis.nrt"),
                expected_network_id: None,
                network_id: None,
                operator_public_key: None,
                output: output.clone(),
            })
            .unwrap_err();
            assert!(!format!("{error:#}").contains("fixture-secret"));
            assert!(!output.exists());
        }
        fs::write(&source, FIXTURE).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        let file = File::open(&source).unwrap();
        assert!(inherited_config(file.as_raw_fd() as u32).is_err());
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&source, directory.path().join("linked.toml")).unwrap();
        assert!(inherited_config(file.as_raw_fd() as u32).is_err());
        let directory_fd = File::open(directory.path()).unwrap();
        assert!(inherited_config(directory_fd.as_raw_fd() as u32).is_err());
        let (reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        assert!(inherited_config(reader.as_raw_fd() as u32).is_err());
        assert!(inherited_config(2).is_err());
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn config_rebase_preserves_caller_offset_and_rejects_writable_or_unlinked_descriptors() {
        use std::os::fd::AsRawFd as _;
        let directory = private_custody_test_dir("config-rebase-fd-");
        let path = directory.path().join("source.toml");
        fs::write(&path, FIXTURE).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut source = File::open(&path).unwrap();
        source.seek(std::io::SeekFrom::Start(9)).unwrap();
        rustix::io::fcntl_setfd(&source, rustix::io::FdFlags::empty()).unwrap();
        let fd = source.as_raw_fd() as u32;
        assert_eq!(inherited_config(fd).unwrap().as_slice(), FIXTURE);
        assert_eq!(source.stream_position().unwrap(), 9);
        assert!(
            rustix::io::fcntl_getfd(&source)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        for read in [true, false] {
            let writable = fs::OpenOptions::new()
                .read(read)
                .write(true)
                .open(&path)
                .unwrap();
            let error = inherited_config(writable.as_raw_fd() as u32).unwrap_err();
            assert!(error.to_string().contains("read-only"));
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        assert!(inherited_config(fd).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(inherited_config(fd).is_err());
    }
}
