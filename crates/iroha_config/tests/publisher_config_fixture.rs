//! Exact file references for configuration parser tests, never daemon publisher custody.
//!
//! The noncanonical keyring bytes exercise bounded configuration loading only. The daemon
//! must still reject this data in its canonical keyring, role and network preflight. Tests
//! that successfully construct a publisher service belong to its authenticated fixture lane.

use std::{io, path::Path};

use iroha_config_base::{
    file_source::{ConfigFileAccess, ConfigFileRequest, ConfigFileSource},
    read::ConfigReader,
    toml::TomlSource,
};
use toml::{Table, Value};
use zeroize::Zeroizing;

// Absolute virtual paths keep the exact source independent of each TOML origin.
// These are supplied bytes only; the source never opens either native path.
const KEYRING_FILE: &str = "/__iroha_config_test__/unadmitted-publisher-keyring";
const SUBMITTER_FILE: &str = "/__iroha_config_test__/publisher-submitter";
const UNADMITTED_KEYRING_DATA: &[u8] = b"configuration-parser-only; not an authenticated keyring";

/// Bind only the two test-owned file references, preserving every other publisher field.
pub(crate) fn bind_fixture_refs(table: &mut Table) {
    let publisher = table
        .entry("kagemusha_load_authorizer")
        .or_insert_with(|| Value::Table(Table::new()))
        .as_table_mut()
        .expect("publisher configuration must remain a table");
    publisher.insert("keyring_file".into(), KEYRING_FILE.into());
    publisher.insert("submitter_key_file".into(), SUBMITTER_FILE.into());
}

/// Overlay file references after the base fixture without changing that checked-in fixture.
pub(crate) fn with_fixture_refs(reader: ConfigReader) -> ConfigReader {
    let mut table = Table::new();
    bind_fixture_refs(&mut table);
    reader.with_toml_source(TomlSource::inline(table))
}

/// Explicit bounded parser-only bytes. Missing paths never fall back to native files.
pub(crate) struct ParserOnlyPublisherFiles;

impl ConfigFileSource for ParserOnlyPublisherFiles {
    fn read(&self, path: &Path, request: ConfigFileRequest) -> io::Result<Zeroizing<Vec<u8>>> {
        if request.access != ConfigFileAccess::Private {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
        let bytes = if path == Path::new(KEYRING_FILE) {
            UNADMITTED_KEYRING_DATA.to_vec()
        } else if path == Path::new(SUBMITTER_FILE) {
            // Reuse the existing canonical test key; generate no operational credentials.
            let fixture: Table = include_str!("fixtures/base.toml")
                .parse()
                .expect("base fixture TOML should parse");
            fixture["private_key"]
                .as_str()
                .expect("base fixture declares its canonical private key")
                .as_bytes()
                .to_vec()
        } else {
            return Err(io::ErrorKind::NotFound.into());
        };
        if bytes.len() > request.maximum {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(Zeroizing::new(bytes))
    }
}

#[test]
fn parser_only_source_keeps_selected_path_private_and_bounded() {
    let files = ParserOnlyPublisherFiles;
    let private = ConfigFileRequest {
        access: ConfigFileAccess::Private,
        maximum: 65_536,
    };
    let keyring = files.read(Path::new(KEYRING_FILE), private).unwrap();
    assert_eq!(keyring.as_slice(), UNADMITTED_KEYRING_DATA);
    let submitter = files.read(Path::new(SUBMITTER_FILE), private).unwrap();
    let canonical = std::str::from_utf8(&submitter).unwrap();
    canonical.parse::<iroha_crypto::PrivateKey>().unwrap();
    assert_eq!(
        files
            .read(Path::new("another-keyring"), private)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    assert_eq!(
        files
            .read(
                Path::new(KEYRING_FILE),
                ConfigFileRequest {
                    access: ConfigFileAccess::Public,
                    ..private
                },
            )
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        files
            .read(
                Path::new(KEYRING_FILE),
                ConfigFileRequest {
                    maximum: UNADMITTED_KEYRING_DATA.len() - 1,
                    ..private
                },
            )
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn publisher_fixture_parses_through_inline_and_named_toml_origins() {
    for named_source in [false, true] {
        let mut table: Table = include_str!("fixtures/base.toml").parse().unwrap();
        bind_fixture_refs(&mut table);
        let source = if named_source {
            TomlSource::new("other/config/peer.toml".into(), table)
        } else {
            TomlSource::inline(table)
        };
        let actual = ConfigReader::new()
            .without_env()
            .with_toml_source(source)
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .unwrap()
            .parse_with_file_source(&ParserOnlyPublisherFiles)
            .unwrap();
        assert_eq!(
            actual
                .kagemusha_load_authorizer
                .as_ref()
                .unwrap()
                .custody
                .keyring
                .as_slice(),
            UNADMITTED_KEYRING_DATA
        );
    }
}

#[test]
fn binding_fixture_paths_preserves_retired_field_rejection() {
    let mut table: Table = "[kagemusha_load_authorizer]\nenabled = false\n"
        .parse()
        .unwrap();
    bind_fixture_refs(&mut table);
    let publisher = table
        .remove("kagemusha_load_authorizer")
        .unwrap()
        .as_table()
        .unwrap()
        .clone();
    let error = ConfigReader::new()
        .with_toml_source(TomlSource::inline(publisher))
        .read_and_complete::<iroha_config::parameters::user::KagemushaLoadAuthorizer>()
        .unwrap_err();
    assert!(format!("{error:?}").contains("enabled"));
}

#[test]
fn missing_publisher_refs_still_refuse_root_configuration() {
    let user = ConfigReader::new()
        .read_toml_with_extends(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/base.toml"),
        )
        .unwrap()
        .with_toml_source(TomlSource::inline(
            "[kagemusha_load_authorizer]\n".parse().unwrap(),
        ))
        .read_and_complete::<iroha_config::parameters::user::Root>()
        .unwrap();
    let error = user
        .parse_with_file_source(&ParserOnlyPublisherFiles)
        .unwrap_err();
    assert!(
        format!("{error:?}").contains(
            "kagemusha_load_authorizer requires both keyring_file and submitter_key_file"
        )
    );
}
