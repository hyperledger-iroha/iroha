//! Regression checks for the checked-in default Docker Compose identities.
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
type ServiceEnvironments = BTreeMap<String, BTreeMap<String, String>>;
const DEFAULT_STREAMING_PUBLIC_KEY: &str =
    "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
const DEFAULT_STREAMING_PRIVATE_KEY: &str =
    "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83";
fn defaults_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../defaults")
}
fn parse_service_environments(path: &Path) -> ServiceEnvironments {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let mut environments = ServiceEnvironments::new();
    let mut current_service: Option<String> = None;
    let mut in_environment = false;
    for line in contents.lines() {
        if let Some(candidate) = line
            .strip_prefix("  ")
            .and_then(|candidate| candidate.strip_suffix(':'))
            && !candidate.starts_with(char::is_whitespace)
            && candidate.strip_prefix("irohad").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            environments.entry(candidate.to_owned()).or_default();
            current_service = Some(candidate.to_owned());
            in_environment = false;
            continue;
        }
        if current_service.is_some() && line == "    environment:" {
            in_environment = true;
            continue;
        }
        if !in_environment {
            continue;
        }
        let Some(field) = line.strip_prefix("      ") else {
            if !line.trim().is_empty() {
                in_environment = false;
            }
            continue;
        };
        if field.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((name, value)) = field.split_once(": ") else {
            continue;
        };
        environments
            .get_mut(current_service.as_ref().expect("service is present"))
            .expect("service environment was initialized")
            .insert(name.to_owned(), value.to_owned());
    }
    environments
}
fn yaml_scalar(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}
fn validate_transport_identities(
    path: &Path,
    environments: &ServiceEnvironments,
) -> BTreeMap<String, (String, String)> {
    let expected_services = (0_u8..4)
        .map(|index| format!("irohad{index}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        environments.keys().cloned().collect::<BTreeSet<_>>(),
        expected_services,
        "{} must describe the canonical four-validator committee",
        path.display()
    );
    let mut public_keys = BTreeSet::new();
    let mut private_keys = BTreeSet::new();
    let mut identities = BTreeMap::new();
    for (service, environment) in environments {
        let public_text = yaml_scalar(
            environment
                .get("P2P_SORANET_TRANSPORT_PUBLIC_KEY")
                .unwrap_or_else(|| panic!("{service} lacks its SoraNet transport public key")),
        );
        let private_text = yaml_scalar(
            environment
                .get("P2P_SORANET_TRANSPORT_PRIVATE_KEY")
                .unwrap_or_else(|| panic!("{service} lacks its SoraNet transport private key")),
        );
        let node_public_text = yaml_scalar(
            environment
                .get("PUBLIC_KEY")
                .unwrap_or_else(|| panic!("{service} lacks its validator public key")),
        );
        let node_private_text = yaml_scalar(
            environment
                .get("PRIVATE_KEY")
                .unwrap_or_else(|| panic!("{service} lacks its validator private key")),
        );
        let public = public_text
            .parse::<PublicKey>()
            .unwrap_or_else(|error| panic!("{service} transport public key is invalid: {error}"));
        let private = private_text
            .parse::<ExposedPrivateKey>()
            .unwrap_or_else(|error| panic!("{service} transport private key is invalid: {error}"));
        let node_public = node_public_text
            .parse::<PublicKey>()
            .unwrap_or_else(|error| panic!("{service} validator public key is invalid: {error}"));
        let transport = KeyPair::new(public.clone(), private.0)
            .unwrap_or_else(|error| panic!("{service} transport key pair does not match: {error}"));
        assert_eq!(transport.algorithm(), Algorithm::Ed25519);
        assert_eq!(node_public.algorithm(), Algorithm::BlsNormal);
        assert_ne!(public, node_public, "{service} reuses its signing identity");
        assert_ne!(
            public_text, DEFAULT_STREAMING_PUBLIC_KEY,
            "{service} reuses the checked-in streaming public identity"
        );
        assert_ne!(
            private_text, DEFAULT_STREAMING_PRIVATE_KEY,
            "{service} reuses the checked-in streaming private identity"
        );
        assert_ne!(
            private_text, node_private_text,
            "{service} reuses its validator signing secret"
        );
        assert!(
            public_keys.insert(public_text.to_owned()),
            "{} repeats transport public key {public_text}",
            path.display()
        );
        assert!(
            private_keys.insert(private_text.to_owned()),
            "{} repeats a transport private key",
            path.display()
        );
        identities.insert(
            service.clone(),
            (public_text.to_owned(), private_text.to_owned()),
        );
    }
    identities
}
#[test]
fn default_compose_snapshots_share_valid_dedicated_soranet_identities() {
    let paths = [
        defaults_dir().join("docker-compose.single.yml"),
        defaults_dir().join("docker-compose.local.yml"),
        defaults_dir().join("docker-compose.yml"),
    ];
    let baseline_environments = parse_service_environments(&paths[0]);
    let baseline_identities = validate_transport_identities(&paths[0], &baseline_environments);
    for path in &paths[1..] {
        let environments = parse_service_environments(path);
        let identities = validate_transport_identities(path, &environments);
        assert_eq!(
            identities,
            baseline_identities,
            "{} changed the deterministic SoraNet identity assignment",
            path.display()
        );
        assert_eq!(
            environments,
            baseline_environments,
            "{} changed validator runtime environment semantics",
            path.display()
        );
    }
}
