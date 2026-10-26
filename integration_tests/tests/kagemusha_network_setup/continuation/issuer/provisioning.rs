//! Explicit same-network engineering issuer composition; no eligibility or E1 admission.
//!
//! The independently selected runtime supplies its existing E1 store, reviewed platform roots,
//! exact programs, HTTPS credential and trusted endpoint. This owner initializes only a new
//! native issuer journal and fresh local signing custody. Its empty operator table approves no
//! subject. The serving owner remains responsible for startup, eligibility, evidence and signing.
use super::*;
use iroha_config::parameters::user::KagemushaEnrollmentWorker;
use iroha_core::kagemusha_wallet_v1::{
    enrollment_issuer::enrollment_issuer_journal_scope_v1, enrollment_journal::EnrollmentJournalV1,
};
use iroha_fs::{FileSnapshot, PublishMode, RetainedFile};
use norito::json::{JsonDeserialize, Value};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use rand::TryRng as _;
use zeroize::Zeroizing;

const INPUT_MAX: usize = 64 << 10;
const PUBLIC_MAX: usize = 16 << 10;
// Keep aligned with wallet_enrollment_store.py and revocation.py; serving validates
// the actual SQLite schema, generation and denylist contents independently.
const DENYLIST_CUSTODY_MAX: u64 = 12 << 20;
// Match the concrete Native worker's program bounds.
const EXECUTABLE_MAX: u64 = 256 << 20;
const ARCHIVE_MAX: u64 = 8 << 20;

#[path = "provisioning/configuration.rs"]
mod configuration;
#[path = "provisioning/inputs.rs"]
mod inputs;
#[path = "provisioning/scopes.rs"]
mod scopes;
use inputs::Inputs;

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Original {
    path: PathBuf,
    sha256: String,
}
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct AndroidRoot {
    family: String,
    spki: Original,
}
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Android {
    package_name: String,
    minimum_package_version: u64,
    signing_certificates_der: Vec<Original>,
    roots: Vec<AndroidRoot>,
    patch_floor_yyyymm: u32,
}
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Request {
    schema: String,
    issuer_request: Original,
    setup: Original,
    registered: Original,
    peer_index: u8,
    private_parent: PathBuf,
    output_name: String,
    scope_hex: String,
    application_scopes: scopes::Originals,
    observation_endpoint: String,
    observation_credential: PathBuf,
    observation_tls_root: Option<Original>,
    worker: KagemushaEnrollmentWorker,
    android: Android,
    maximum_response_ms: u64,
    challenge_lifetime_ms: u64,
}

fn pin(text: &str) -> Result<[u8; 32]> {
    digest(&Value::String(text.to_owned()))
}
fn exact_keys(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or_else(|| eyre!("object required"))?;
    ensure!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>()
            == keys.iter().copied().collect(),
        "closed object required"
    );
    Ok(())
}
fn bindings(request: &Request, handoff: &Value, setup: &Value, registered: &Value) -> Result<()> {
    exact_keys(
        handoff,
        &[
            "schema",
            "setup_sha256",
            "registered_sha256",
            "scheme_id",
            "manifest_digest",
            "verifier_pack",
            "selection_path",
            "scope",
        ],
    )?;
    ensure!(
        request.schema == "iroha.kagemusha.engineering-issuer-provisioning.v1"
            && handoff["schema"].as_str()
                == Some("iroha.kagemusha.network-controller-issuer-request.v1")
            && setup["schema"].as_str() == Some("iroha.kagemusha.executed-ledger-setup.v1")
            && registered["schema"].as_str() == Some("iroha.kagemusha.network-registered.v1")
            && request.peer_index < 4,
        "same-network issuer grammar required"
    );
    ensure!(
        handoff["setup_sha256"].as_str() == Some(request.setup.sha256.as_str())
            && handoff["registered_sha256"].as_str() == Some(request.registered.sha256.as_str())
            && registered["setup_sha256"] == handoff["setup_sha256"]
            && registered["scheme_id"] == handoff["scheme_id"]
            && registered["manifest_digest"] == handoff["manifest_digest"]
            && registered["verifier_pack_sha256"] == handoff["verifier_pack"]["sha256"],
        "cross-occurrence issuer selection refused"
    );
    let parent = request
        .issuer_request
        .path
        .parent()
        .ok_or_else(|| eyre!("handoff parent"))?;
    ensure!(
        handoff["selection_path"].as_str() == parent.join("issuer-selection.json").to_str(),
        "exact handoff destination required"
    );
    ensure!(
        request.maximum_response_ms > 0
            && request.maximum_response_ms <= 60_000
            && request.challenge_lifetime_ms > 0
            && (1..=300_000).contains(&request.worker.exchange_timeout_ms),
        "positive bounded issuer intervals required"
    );
    pin(&request.scope_hex)?;
    Ok(())
}

fn platform(
    request: &Request,
    scheme: [u8; 32],
    inputs: &mut Inputs,
) -> Result<(
    KagemushaWalletAppPolicyV1,
    KagemushaWalletEnrollmentPolicyTemplateV1,
)> {
    ensure!(
        (1..=4).contains(&request.android.signing_certificates_der.len())
            && (1..=8).contains(&request.android.roots.len())
            && request.worker.attestation_root.is_none(),
        "explicit Android original bounds required"
    );
    let mut signers = request
        .android
        .signing_certificates_der
        .iter()
        .map(|original| {
            inputs
                .read(original, PUBLIC_MAX)
                .map(|b| Sha256::digest(&b).into())
        })
        .collect::<Result<Vec<[u8; 32]>>>()?;
    signers.sort();
    let app = KagemushaWalletAppPolicyV1 {
        version: 1,
        scheme_id: scheme,
        identity: KagemushaWalletAppIdentityV1::Android {
            package_name: request.android.package_name.clone(),
            minimum_package_version: request.android.minimum_package_version,
            signer_set: signers,
        },
    };
    let mut roots = Vec::new();
    for original in &request.android.roots {
        let family = match original.family.as_str() {
            "google-rsa" => KagemushaWalletAndroidRootFamilyV1::GoogleRsa,
            "google-ca1" => KagemushaWalletAndroidRootFamilyV1::GoogleCa1,
            _ => return Err(eyre!("unqualified Android root family")),
        };
        roots.push(KagemushaWalletAndroidRootV1 {
            family,
            spki_sha256: Sha256::digest(inputs.read(&original.spki, PUBLIC_MAX)?).into(),
            revocation_source: KagemushaWalletRevocationSourceV1::GoogleAttestationStatus,
        });
    }
    roots.sort_by_key(|r| r.spki_sha256);
    let template = KagemushaWalletEnrollmentPolicyTemplateV1 {
        version: 1,
        scheme_id: scheme,
        app_policy: app.policy_digest()?,
        platform: KagemushaWalletEnrollmentPlatformV1::Android {
            root_set: roots,
            patch_floor_yyyymm: request.android.patch_floor_yyyymm,
        },
        regulatory_policy: KagemushaWalletRegulatoryPolicyV1::default(),
        challenge_lifetime_ms: request.challenge_lifetime_ms,
        attestation_lease_lifetime_ms: 0,
    };
    template.validate_for_app(&app)?;
    iroha_core_zk::kagemusha_wallet_enrollment_v1::issuer_worker::require_qualified_android_adapters_v1(
        &template.platform)?;
    Ok((app, template))
}

fn public(key: &SigningKey) -> Result<KagemushaDevicePublicKeyV1> {
    Ok(KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_sec1_point(false).as_bytes(),
    )?)
}
fn certificate(
    scheme: &KagemushaWalletSchemeV1,
    signer: &SigningKey,
) -> Result<KagemushaWalletSignerCertificateV1> {
    // Same PUBLIC engineering root as full_catalog::fixture_scope. Refuse every other
    // source root; this helper is not a production signer and accepts no production key.
    let root = SigningKey::from_slice(&[0x11; 32])?;
    ensure!(
        scheme.scheme_root_key == public(&root)?,
        "engineering catalog required"
    );
    let body = KagemushaWalletSignerCertificateBodyV1 {
        version: 1,
        scheme_id: scheme.scheme_id(),
        role: KagemushaWalletSignerRoleV1::Enrollment,
        key: public(signer)?,
        serial: 1,
    };
    let signature: Signature = root.sign(&body.signing_message());
    Ok(KagemushaWalletSignerCertificateV1::sign(
        body,
        scheme,
        KagemushaWalletSignerOutputV1::Der(signature.to_der().as_bytes()),
    )?)
}
fn fresh_signer() -> Result<SigningKey> {
    for _ in 0..16 {
        let mut bytes = Zeroizing::new([0; 32]);
        rand::rngs::SysRng
            .try_fill_bytes(bytes.as_mut())
            .map_err(|_| eyre!("entropy unavailable"))?;
        if let Ok(key) = SigningKey::from_slice(bytes.as_ref()) {
            return Ok(key);
        }
    }
    Err(eyre!("bounded key generation refused"))
}
fn text(table: &mut Table, key: &str, value: impl Into<String>) {
    table.insert(key.into(), toml::Value::String(value.into()));
}
fn number(table: &mut Table, key: &str, value: u64) -> Result<()> {
    table.insert(key.into(), toml::Value::Integer(i64::try_from(value)?));
    Ok(())
}
fn path_text(path: &Path) -> Result<String> {
    Ok(path
        .to_str()
        .ok_or_else(|| eyre!("UTF-8 private path required"))?
        .to_owned())
}

fn decode_request(raw: &[u8]) -> Result<Request> {
    let raw_value: Value = norito::json::from_slice(raw)?;
    exact_keys(
        &raw_value["worker"],
        &[
            "python_executable",
            "python_sha256_hex",
            "verifier_archive",
            "verifier_sha256_hex",
            "openssl_executable",
            "openssl_sha256_hex",
            "attestation_root",
            "store_directory",
            "exchange_timeout_ms",
        ],
    )?;
    Ok(norito::json::from_slice(raw)?)
}

fn selected_output_parent(request: &Request) -> Result<(PrivateDirectory, PathBuf)> {
    // Match the controller's source-derived ROOT/target/qualification namespace.
    // Do not create journal/signing custody for a handoff it cannot consume.
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| eyre!("integration checkout required"))?
        .canonicalize()?;
    let qualification = repository.join("target/qualification");
    ensure!(
        qualification.is_absolute()
            && qualification.canonicalize()?.as_os_str() == qualification.as_os_str(),
        "original qualification directory required"
    );
    ensure!(
        request.private_parent.is_absolute()
            && request.private_parent.starts_with(&qualification)
            && request.private_parent.canonicalize()?.as_os_str()
                == request.private_parent.as_os_str(),
        "private issuer output must remain inside the original qualification directory"
    );
    let name = Path::new(&request.output_name);
    ensure!(
        name.components().count() == 1
            && matches!(
                name.components().next(),
                Some(std::path::Component::Normal(_))
            )
            && name
                .file_name()
                .is_some_and(|leaf| leaf == name.as_os_str()),
        "one exact fresh issuer child name required"
    );
    let parent = PrivateDirectory::open_exact(&request.private_parent)?;
    let output_path = request.private_parent.join(name);
    match fs::symlink_metadata(&output_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => {
            return Err(eyre!(
                "fresh absent issuer output required; preserve existing custody"
            ));
        }
    }
    parent.revalidate()?;
    Ok((parent, output_path))
}

fn prepare(input: &Original) -> Result<()> {
    ensure!(cfg!(target_os = "linux"), "native issuer requires Linux");
    let mut inputs = Inputs(Vec::new());
    inputs.retain_private(&input.path, INPUT_MAX as u64)?;
    let raw = inputs.read(input, INPUT_MAX)?;
    let request = decode_request(&raw)?;
    let (parent, output_path) = selected_output_parent(&request)?;
    for selected in [&request.issuer_request, &request.setup, &request.registered] {
        inputs.retain_private(&selected.path, INPUT_MAX as u64)?;
    }
    let handoff: Value =
        norito::json::from_slice(&inputs.read(&request.issuer_request, INPUT_MAX)?)?;
    let setup: Value = norito::json::from_slice(&inputs.read(&request.setup, INPUT_MAX)?)?;
    let registered: Value =
        norito::json::from_slice(&inputs.read(&request.registered, INPUT_MAX)?)?;
    bindings(&request, &handoff, &setup, &registered)?;
    let scopes = scopes::read(&request.application_scopes, &mut inputs)?;
    let pack_selection: Original = norito::json::from_value(handoff["verifier_pack"].clone())?;
    let scheme_id = digest(&handoff["scheme_id"])?;
    let manifest = digest(&handoff["manifest_digest"])?;
    let pack = InstalledVerifierPackV1::load(
        &inputs.read(&pack_selection, VERIFIER_PACK_MAX_BYTES_V1)?,
        InstallationV1 {
            scheme_id,
            manifest_digest: manifest,
        },
    )?;
    let scheme = *pack.verifier().scheme();
    ensure!(
        hex::encode(scheme.network_id) == setup["network_hex"].as_str().unwrap_or(""),
        "pack belongs to another network"
    );
    let (app, enrollment) = platform(&request, scheme_id, &mut inputs)?;
    let signer = fresh_signer()?;
    let certificate = certificate(&scheme, &signer)?;
    let observer = KeyPair::try_random_with_algorithm(Algorithm::Ed25519)?;
    let eligibility = KagemushaEligibilityPolicyTemplateV1 {
        version: 1,
        network_id: scheme.network_id,
        scheme_id,
        revision: 1,
        authority: KagemushaEligibilityAuthorityV1::SchemeOperator {
            operator_digest: scopes.operator_digest,
        },
        public_key: observer
            .public_key()
            .to_bytes()
            .1
            .try_into()
            .map_err(|_| eyre!("Ed25519 extent"))?,
        maximum_response_ms: request.maximum_response_ms,
    };
    let template = eligibility.encode_canonical()?;
    inputs.retain_private(&request.observation_credential, 4096)?;
    if let Some(root) = &request.observation_tls_root {
        inputs.retain_private(&root.path, PUBLIC_MAX as u64)?;
        inputs.read(root, PUBLIC_MAX)?;
    }
    for (path, pin, maximum) in [
        (
            &request.worker.python_executable,
            &request.worker.python_sha256_hex,
            EXECUTABLE_MAX,
        ),
        (
            &request.worker.verifier_archive,
            &request.worker.verifier_sha256_hex,
            ARCHIVE_MAX,
        ),
        (
            &request.worker.openssl_executable,
            &request.worker.openssl_sha256_hex,
            EXECUTABLE_MAX,
        ),
    ] {
        inputs.program(path, pin, maximum)?;
    }
    let worker_store = PrivateDirectory::open_exact(&request.worker.store_directory)?;
    inputs.retain_private(
        &request.worker.store_directory.join("wallet-e1.generation"),
        128,
    )?;
    inputs.retain_private(
        &request
            .worker
            .store_directory
            .join("wallet-e1-denylist.custody"),
        DENYLIST_CUSTODY_MAX,
    )?;
    // Opening the database is an existence/custody check only; do not substitute
    // a synthetic SQLite original or claim that the E1 worker has admitted it.
    let database =
        RetainedFile::open_private(request.worker.store_directory.join("wallet-e1.sqlite3"))?;
    let output = parent.create_child(&request.output_name)?;
    let journal_scope = enrollment_issuer_journal_scope_v1(pin(&request.scope_hex)?)?;
    drop(EnrollmentJournalV1::initialize(
        &output,
        "journal",
        &journal_scope,
    )?);
    let operator = output.create_child("operator")?;
    let secret = Zeroizing::new(observer.private_key().try_to_bytes()?.1);
    ensure!(secret.len() == 32, "observer key extent");
    operator.write_atomic("signer.key", &secret, PublishMode::CreateNew)?;
    operator.write_atomic("template.norito", &template, PublishMode::CreateNew)?;
    // Initial empty state grants no eligibility. Only the separately authenticated operator
    // may later publish real current decisions; this composer never emits an approval row.
    let current = norito::json::to_vec(&norito::json!({"version":1,
        "templateHex":(hex::encode(&template)), "records":[]}))?;
    operator.write_atomic("current.json", &current, PublishMode::CreateNew)?;
    let secret = Zeroizing::new(signer.to_bytes());
    output.write_atomic(
        "enrollment-signer.key",
        secret.as_ref(),
        PublishMode::CreateNew,
    )?;
    for (name, bytes) in [
        ("audience.bin", scopes.audience.clone()),
        ("release.bin", scopes.release.clone()),
        ("service-origin.bin", scopes.service_origin.clone()),
        ("scheme.norito", scheme.to_canonical_bytes()?),
        (
            "enrollment-certificate.norito",
            certificate.to_canonical_bytes()?,
        ),
        ("app-policy.norito", app.encode_canonical()?),
        ("enrollment-template.norito", enrollment.encode_canonical()?),
    ] {
        output.write_atomic(name, &bytes, PublishMode::CreateNew)?;
    }
    let configuration = configuration::encode(
        &request,
        &output_path,
        configuration::Originals {
            scheme: &scheme,
            app: &app,
            enrollment: &enrollment,
            certificate: &certificate,
            template: &template,
            manifest,
            scopes: &scopes,
        },
    )?;
    inputs.revalidate()?;
    parent.revalidate()?;
    worker_store.revalidate()?;
    database.revalidate()?;
    output.write_atomic("issuer.toml", &configuration, PublishMode::CreateNew)?;
    publish_selection(&request, &output_path, &configuration, &mut inputs)
}

fn publish_selection(
    request: &Request,
    output_path: &Path,
    configuration: &[u8],
    inputs: &mut Inputs,
) -> Result<()> {
    let configuration_path = output_path.join("issuer.toml");
    inputs.retain_private(&configuration_path, CONFIG_MAX as u64)?;
    inputs.read(
        &Original {
            path: configuration_path,
            sha256: hash(configuration),
        },
        CONFIG_MAX,
    )?;
    let selection = norito::json::to_vec(&norito::json!({
        "schema":"iroha.kagemusha.network-controller-issuer-selection.v1",
        "setup_sha256":(request.setup.sha256), "registered_sha256":(request.registered.sha256),
        "peer_index":(request.peer_index), "configuration":{
            "path":(path_text(&output_path.join("issuer.toml"))?), "sha256":(hash(configuration))}}))?;
    // Published last. No readiness, E1 verdict, eligibility approval or credential is reported.
    let destination = request
        .issuer_request
        .path
        .parent()
        .ok_or_else(|| eyre!("handoff parent"))?;
    let handoff_owner = PrivateDirectory::open_exact(destination)?;
    inputs.revalidate()?;
    PrivateDirectory::open_exact(output_path)?.revalidate()?;
    handoff_owner.write_atomic("issuer-selection.json", &selection, PublishMode::CreateNew)?;
    Ok(())
}

#[test]
#[ignore = "explicit Linux-only current engineering issuer input; creates fresh runtime custody, never approves eligibility or fabricates E1"]
fn prepare_same_live_engineering_issuer_configuration() {
    let result = (|| -> Result<()> {
        let path = PathBuf::from(
            std::env::var_os("KAGEMUSHA_ENGINEERING_ISSUER_REQUEST")
                .ok_or_else(|| eyre!("explicit request required"))?,
        );
        let sha256 = std::env::var("KAGEMUSHA_ENGINEERING_ISSUER_REQUEST_SHA256")?;
        pin(&sha256)?;
        prepare(&Original { path, sha256 })
    })();
    assert!(
        result.is_ok(),
        "issuer composition refused; preserve private partial custody, never overwrite or reset"
    );
}

#[cfg(test)]
#[path = "provisioning/tests.rs"]
mod tests;
