//! Native-only materialization of secret-bearing retained validator configuration.

use super::*;
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
    /// Sole configuration field to replace with this absolute path.
    #[arg(long, value_name = "PATH")]
    genesis_file: PathBuf,
    /// Fresh 0600 config in an existing owner-only directory; never overwritten.
    #[arg(long, value_name = "PATH")]
    output: PathBuf,
}

pub(super) fn config_rebase(args: &ConfigRebase) -> Result<()> {
    validate_absolute_normal_path(&args.expected_genesis_file, "expected genesis path")?;
    validate_absolute_normal_path(&args.genesis_file, "new genesis path")?;
    let source = inherited_config(args.config_fd)?;
    let output = rebase_genesis_file(&source, &args.expected_genesis_file, &args.genesis_file)?;
    super::inputs::write_new_private(&args.output, &output)
}

fn inherited_config(fd: u32) -> Result<Zeroizing<Vec<u8>>> {
    crate::client_config::read_inherited_private_file(fd, MAX_CONFIG_BYTES, "validator config")
}

fn rebase_genesis_file(
    bytes: &[u8],
    expected: &Path,
    replacement: &Path,
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
        let replacement = replacement
            .to_str()
            .ok_or_else(|| eyre!("new genesis path is not UTF-8"))?;
        genesis.insert(
            "file".to_owned(),
            toml::Value::String(replacement.to_owned()),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = b"private_key = 'fixture-secret-not-runtime'\nsoranet_transport_private_key = 'transport-fixture'\n[genesis]\nfile = '/retained/genesis.nrt'\nexpected_hash = 'public-hash'\n[streaming]\nidentity_private_key = 'streaming-fixture'\n";

    #[test]
    fn config_rebase_changes_only_genesis_file_and_keeps_errors_secret_free() {
        let output = rebase_genesis_file(
            FIXTURE,
            Path::new("/retained/genesis.nrt"),
            Path::new("/installed/genesis.nrt"),
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
        let command = super::super::PublicReset {
            command: super::super::PublicResetCommand::ConfigRebase(ConfigRebase {
                config_fd: file.as_raw_fd() as u32,
                expected_genesis_file: PathBuf::from("/retained/genesis.nrt"),
                genesis_file: PathBuf::from("/installed/genesis.nrt"),
                output: destination.clone(),
            }),
        };
        let mut stdout = Vec::new();
        command.run_without_client_config(&mut stdout).unwrap();
        assert!(stdout.is_empty());
        assert_eq!(destination.metadata().unwrap().mode() & 0o7777, 0o600);
        assert_eq!(fs::read(&source).unwrap(), FIXTURE);
        let before = fs::read(&destination).unwrap();
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
