//! Regression checks for the checked-in default Docker Compose manifests: their dedicated
//! validator identities and their byte-exact reproduction from the development seed.
use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair, PublicKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU16,
    path::{Path, PathBuf},
};
type ServiceEnvironments = BTreeMap<String, BTreeMap<String, String>>;
const DEFAULT_STREAMING_PUBLIC_KEY: &str =
    "ed01201C61FAF8FE94E253B93114240394F79A607B7FA55F9E5A41EBEC74B88055768B";
const DEFAULT_STREAMING_PRIVATE_KEY: &str =
    "802620282ED9F3CF92811C3818DBC4AE594ED59DC1A2F78E4241E31924E101D6B1FB83";
/// Development seed `scripts/tests/consistency.sh docker-compose` renders the snapshots with.
const SNAPSHOT_SEED: &[u8] = b"Iroha";
/// Checked-in snapshots with the image and build context that the consistency check passes.
const SNAPSHOTS: [(&str, &str, bool); 3] = [
    ("docker-compose.single.yml", "hyperledger/iroha:local", true),
    ("docker-compose.local.yml", "hyperledger/iroha:local", true),
    ("docker-compose.yml", "hyperledger/iroha:dev", false),
];
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve the workspace root")
}
fn defaults_dir() -> PathBuf {
    workspace_root().join("defaults")
}
fn snapshot_paths() -> Vec<PathBuf> {
    SNAPSHOTS
        .iter()
        .map(|(file, _, _)| defaults_dir().join(file))
        .collect()
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
fn environment_value<'a>(
    service: &str,
    environment: &'a BTreeMap<String, String>,
    name: &str,
) -> &'a str {
    yaml_scalar(
        environment
            .get(name)
            .unwrap_or_else(|| panic!("{service} lacks {name}")),
    )
}
fn assert_canonical_committee(path: &Path, environments: &ServiceEnvironments) {
    let expected_services = (0_u8..4)
        .map(|index| format!("irohad{index}"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        environments.keys().cloned().collect::<BTreeSet<_>>(),
        expected_services,
        "{} must describe the canonical four-validator committee",
        path.display()
    );
}
fn validate_transport_identities(
    path: &Path,
    environments: &ServiceEnvironments,
) -> BTreeMap<String, (String, String)> {
    assert_canonical_committee(path, environments);
    let mut public_keys = BTreeSet::new();
    let mut private_keys = BTreeSet::new();
    let mut identities = BTreeMap::new();
    for (service, environment) in environments {
        let public_text =
            environment_value(service, environment, "P2P_SORANET_TRANSPORT_PUBLIC_KEY");
        let private_text =
            environment_value(service, environment, "P2P_SORANET_TRANSPORT_PRIVATE_KEY");
        let node_public_text = environment_value(service, environment, "PUBLIC_KEY");
        let node_private_text = environment_value(service, environment, "PRIVATE_KEY");
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
fn validate_streaming_identities(
    path: &Path,
    environments: &ServiceEnvironments,
) -> BTreeMap<String, (String, String)> {
    assert_canonical_committee(path, environments);
    let mut public_keys = BTreeSet::new();
    let mut private_keys = BTreeSet::new();
    let mut identities = BTreeMap::new();
    for (service, environment) in environments {
        let public_text = environment_value(service, environment, "STREAMING_IDENTITY_PUBLIC_KEY");
        let private_text =
            environment_value(service, environment, "STREAMING_IDENTITY_PRIVATE_KEY");
        let public = public_text
            .parse::<PublicKey>()
            .unwrap_or_else(|error| panic!("{service} streaming public key is invalid: {error}"));
        let private = private_text
            .parse::<ExposedPrivateKey>()
            .unwrap_or_else(|error| panic!("{service} streaming private key is invalid: {error}"));
        let streaming = KeyPair::new(public, private.0)
            .unwrap_or_else(|error| panic!("{service} streaming key pair does not match: {error}"));
        assert_eq!(
            streaming.algorithm(),
            Algorithm::Ed25519,
            "{service} streaming admission requires an Ed25519 identity"
        );
        for (role, other) in [
            ("validator", "PUBLIC_KEY"),
            ("SoraNet transport", "P2P_SORANET_TRANSPORT_PUBLIC_KEY"),
        ] {
            assert_ne!(
                public_text,
                environment_value(service, environment, other),
                "{service} streaming identity reuses its {role} public key"
            );
        }
        for (role, other) in [
            ("validator", "PRIVATE_KEY"),
            ("SoraNet transport", "P2P_SORANET_TRANSPORT_PRIVATE_KEY"),
        ] {
            assert_ne!(
                private_text,
                environment_value(service, environment, other),
                "{service} streaming identity reuses its {role} secret"
            );
        }
        assert_ne!(
            public_text, DEFAULT_STREAMING_PUBLIC_KEY,
            "{service} reuses the checked-in streaming public identity"
        );
        assert_ne!(
            private_text, DEFAULT_STREAMING_PRIVATE_KEY,
            "{service} reuses the checked-in streaming private identity"
        );
        assert!(
            public_keys.insert(public_text.to_owned()),
            "{} repeats streaming public key {public_text}",
            path.display()
        );
        assert!(
            private_keys.insert(private_text.to_owned()),
            "{} repeats a streaming private key",
            path.display()
        );
        identities.insert(
            service.clone(),
            (public_text.to_owned(), private_text.to_owned()),
        );
    }
    identities
}
/// Returns the manifest after the `kagami docker` banner: leading `# ` comment lines and the
/// single blank separator line.
fn manifest_body<'a>(path: &Path, contents: &'a str) -> &'a str {
    let mut rest = contents;
    while rest.starts_with("# ") {
        let line_end = rest
            .find('\n')
            .unwrap_or_else(|| panic!("{} has an unterminated banner line", path.display()));
        rest = &rest[line_end + 1..];
    }
    rest.strip_prefix('\n').unwrap_or_else(|| {
        panic!(
            "{} must separate its banner from the manifest with one blank line",
            path.display()
        )
    })
}
#[test]
fn default_compose_snapshots_share_valid_dedicated_soranet_identities() {
    let paths = snapshot_paths();
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
#[test]
fn default_compose_snapshots_share_valid_dedicated_streaming_identities() {
    let paths = snapshot_paths();
    let baseline = validate_streaming_identities(&paths[0], &parse_service_environments(&paths[0]));
    for path in &paths[1..] {
        assert_eq!(
            validate_streaming_identities(path, &parse_service_environments(path)),
            baseline,
            "{} changed the deterministic streaming identity assignment",
            path.display()
        );
    }
}
#[test]
fn default_compose_snapshots_reproduce_from_the_development_seed() {
    let root = workspace_root();
    let committee = NonZeroU16::new(4).expect("the canonical committee is non-empty");
    for (file, image, build) in SNAPSHOTS {
        let target = defaults_dir().join(file);
        let mut rendered = Vec::new();
        iroha_swarm::Swarm::deterministic_dev(
            committee,
            SNAPSHOT_SEED,
            true,
            image,
            build.then_some(root.as_path()),
            false,
            &target,
            None,
        )
        .unwrap_or_else(|error| panic!("configure the {file} swarm: {error}"))
        .build()
        .write(&mut rendered, None)
        .unwrap_or_else(|error| panic!("render the {file} swarm: {error}"));
        let rendered = String::from_utf8(rendered).expect("Compose output is UTF-8");
        let checked_in = std::fs::read_to_string(&target)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", target.display()));
        assert!(
            manifest_body(&target, &checked_in) == rendered,
            "{} is stale; regenerate it with `bash scripts/tests/consistency.sh --update docker-compose`",
            target.display()
        );
    }
}
