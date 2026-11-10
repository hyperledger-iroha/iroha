//! SDK-native exact-request account authentication with no transport side effects.

use super::{InputArgs, MAX_PUBLIC_BYTES, read_pinned, read_private_key, write_new};
use eyre::{Result, ensure, eyre};
use iroha::{
    client::{
        canonical_network_request_signature_message, canonical_request_account_header_value,
        canonical_request_signature_header_value, canonical_request_timestamp_header_value,
    },
    http::Method,
};
use iroha_crypto::{PrivateKey, PublicKey, Signature};
use iroha_data_model::{NetworkId, account::AccountId};
use iroha_torii_shared::{
    route_catalog::contracts_and_verification_keys as routes,
    sorafs_gateway_compliance_api::{
        GatewayCompliancePromoteExpectationV1, request_idempotency_binding,
    },
};
use rand::{rand_core::TryRng as _, rngs::SysRng};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

/// Exact public target/body and explicit account credential for a bounded request receipt.
#[derive(Debug, clap::Args)]
pub struct RequestArgs {
    /// Pinned public RequestSpec JSON and fresh receipt output.
    #[command(flatten)]
    input: InputArgs,
    /// Exact body file for stage or acknowledge; absent for status and promote.
    #[arg(long, value_name = "PATH")]
    body_file: Option<PathBuf>,
    /// Fresh exact request body copy; use this file when sending the signed request.
    #[arg(long, value_name = "PATH")]
    body_output: PathBuf,
    /// Fresh owner-only HTTP header file, usable with curl --header @PATH.
    #[arg(long, value_name = "PATH")]
    headers_output: PathBuf,
    /// Owner-private native account key, matched to the explicit requested authority.
    #[arg(long, value_name = "PATH")]
    private_key_file: PathBuf,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct RequestSpec {
    network_id: NetworkId,
    authority: AccountId,
    action: String,
    url: String,
    body_blake3: [u8; 32],
    /// Independently verified target app_api.request_signature_max_clock_skew, in milliseconds.
    max_clock_skew_ms: u64,
}

#[derive(Debug, norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
struct RequestReceipt {
    schema: String,
    network_id: NetworkId,
    authority: AccountId,
    method: String,
    url: String,
    body_blake3_hex: String,
    /// Validity ceiling under the independently selected target clock-skew policy.
    submit_before_unix_ms: u64,
    headers: BTreeMap<String, String>,
}

pub(super) fn run(args: RequestArgs) -> Result<()> {
    let request: RequestSpec = args.input.read()?;
    let body = match &args.body_file {
        Some(path) => read_pinned(path, MAX_PUBLIC_BYTES, request.body_blake3)?,
        None => {
            ensure!(
                request.body_blake3 == *blake3::hash(&[]).as_bytes(),
                "empty body pin differs from the request"
            );
            Vec::new()
        }
    };
    validate(&request, &body)?;
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;
    let mut nonce = [0; 16];
    SysRng
        .try_fill_bytes(&mut nonce)
        .map_err(|_| eyre!("request nonce OS randomness failed"))?;
    let key = read_private_key(&args.private_key_file)?;
    let receipt = sign(&request, &body, &key, timestamp_ms, &hex::encode(nonce))?;
    write_new(&args.body_output, &body)?;
    let mut headers = String::new();
    for (name, value) in &receipt.headers {
        headers.push_str(name);
        headers.push_str(": ");
        headers.push_str(value);
        headers.push('\n');
    }
    write_new(&args.headers_output, headers.as_bytes())?;
    write_new(&args.input.output, &norito::json::to_vec(&receipt)?)
}

fn validate(request: &RequestSpec, body: &[u8]) -> Result<(url::Url, Method, &'static str)> {
    ensure!(
        blake3::hash(body).as_bytes() == &request.body_blake3,
        "request body differs from reviewed digest"
    );
    ensure!(
        (1..=300_000).contains(&request.max_clock_skew_ms),
        "target clock-skew policy must be explicitly selected within 1 to 300000 milliseconds"
    );
    let url = url::Url::parse(&request.url)?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(host)) => host.is_loopback(),
        Some(url::Host::Ipv6(host)) => host.is_loopback(),
        Some(url::Host::Domain("localhost")) => true,
        _ => false,
    };
    ensure!(
        (url.scheme() == "https" || (url.scheme() == "http" && loopback))
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "request requires an HTTPS origin or explicit loopback HTTP without credentials or fragment"
    );
    ensure!(
        request.url == url.as_str(),
        "request URL must use canonical spelling"
    );
    let (path, method, action) = match request.action.as_str() {
        "status" => (
            routes::SORAFS_GATEWAY_COMPLIANCE_STATUS_GET.path(),
            Method::GET,
            "status",
        ),
        "stage" => (
            routes::SORAFS_GATEWAY_COMPLIANCE_STAGE_POST.path(),
            Method::POST,
            "stage",
        ),
        "acknowledge" => (
            routes::SORAFS_GATEWAY_COMPLIANCE_ACKNOWLEDGE_POST.path(),
            Method::POST,
            "acknowledge",
        ),
        "promote" => (
            routes::SORAFS_GATEWAY_COMPLIANCE_PROMOTE_POST.path(),
            Method::POST,
            "promote",
        ),
        _ => return Err(eyre!("unknown compliance request action")),
    };
    ensure!(
        url.path() == path,
        "request URL is not the selected compliance route"
    );
    match request.action.as_str() {
        "status" => ensure!(
            body.is_empty() && url.query().is_none(),
            "status requires an empty body and no query"
        ),
        "promote" => {
            ensure!(body.is_empty(), "promote requires an empty body");
            GatewayCompliancePromoteExpectationV1::parse_query(
                url.query()
                    .ok_or_else(|| eyre!("promotion expectation is required"))?,
            )?;
        }
        "stage" | "acknowledge" => ensure!(
            !body.is_empty() && url.query().is_none(),
            "stage/acknowledge require an explicit body without query"
        ),
        _ => return Err(eyre!("unknown compliance request action")),
    }
    Ok((url, method, action))
}

fn sign(
    request: &RequestSpec,
    body: &[u8],
    key: &PrivateKey,
    timestamp_ms: u64,
    nonce: &str,
) -> Result<RequestReceipt> {
    let (url, method, action) = validate(request, body)?;
    let public = PublicKey::from_private_key(key)
        .map_err(|_| eyre!("invalid account signing credential"))?;
    ensure!(
        AccountId::new(public.clone()) == request.authority,
        "credential does not match explicit request authority"
    );
    let message = canonical_network_request_signature_message(
        &request.network_id,
        &method,
        &url,
        body,
        timestamp_ms,
        nonce,
    )?;
    let signature =
        Signature::try_new(key, &message).map_err(|_| eyre!("request account signature failed"))?;
    signature
        .verify(&public, &message)
        .map_err(|_| eyre!("request signature self-check failed"))?;
    let mut headers = BTreeMap::from([
        ("Accept".into(), "application/json".into()),
        ("Accept-Encoding".into(), "identity".into()),
        (
            "X-Iroha-Account".into(),
            canonical_request_account_header_value(&request.authority)?,
        ),
        (
            "X-Iroha-Signature".into(),
            canonical_request_signature_header_value(&signature)?,
        ),
        (
            "X-Iroha-Timestamp-Ms".into(),
            canonical_request_timestamp_header_value(timestamp_ms)?,
        ),
        ("X-Iroha-Nonce".into(), nonce.to_owned()),
    ]);
    if !matches!(request.action.as_str(), "status") {
        headers.insert("Content-Type".into(), "application/json".into());
        let target = &url[url::Position::BeforePath..url::Position::AfterQuery];
        headers.insert(
            "Idempotency-Key".into(),
            hex::encode(request_idempotency_binding(action, target, body)),
        );
    }
    Ok(RequestReceipt {
        schema: "iroha.sorafs.compliance.request.v1".into(),
        network_id: request.network_id,
        authority: request.authority.clone(),
        method: method.as_str().into(),
        url: request.url.clone(),
        body_blake3_hex: hex::encode(request.body_blake3),
        submit_before_unix_ms: timestamp_ms
            .checked_add(request.max_clock_skew_ms)
            .ok_or_else(|| eyre!("request freshness ceiling overflows"))?,
        headers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    #[test]
    fn compliance_receipt_matches_native_sdk_auth_and_rejects_target_or_authority_changes() {
        let key = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
        let body = b"{}";
        let mut request = RequestSpec {
            network_id: network,
            authority: AccountId::new(key.public_key().clone()),
            action: "stage".into(),
            url: "https://gateway.example.com/v1/sorafs/gateway/compliance/stage".into(),
            body_blake3: *blake3::hash(body).as_bytes(),
            max_clock_skew_ms: 30_000,
        };
        let receipt = sign(
            &request,
            body,
            key.private_key(),
            1_800_000_000_000,
            "reviewed-public-nonce",
        )
        .expect("receipt");
        let (url, method, _) = validate(&request, body).expect("target");
        let message = canonical_network_request_signature_message(
            &network,
            &method,
            &url,
            body,
            1_800_000_000_000,
            "reviewed-public-nonce",
        )
        .expect("SDK message");
        let signature = Signature::from_bytes(
            &STANDARD
                .decode(&receipt.headers["X-Iroha-Signature"])
                .expect("base64"),
        );
        signature
            .verify(key.public_key(), &message)
            .expect("SDK verification");
        assert_eq!(
            receipt.headers["Idempotency-Key"],
            hex::encode(request_idempotency_binding("stage", url.path(), body))
        );
        assert_eq!(receipt.submit_before_unix_ms, 1_800_000_030_000);
        let wrong_key = KeyPair::from_seed(vec![10; 32], Algorithm::Ed25519);
        assert!(sign(&request, body, wrong_key.private_key(), 1, "nonce").is_err());
        assert!(sign(&request, b"changed", key.private_key(), 1, "nonce").is_err());
        request.url = "https://gateway.example.com/v1/sorafs/gateway/compliance/promote".into();
        assert!(sign(&request, body, key.private_key(), 1, "nonce").is_err());
        request.url = "http://gateway.example.com/v1/sorafs/gateway/compliance/stage".into();
        assert!(sign(&request, body, key.private_key(), 1, "nonce").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn request_command_writes_private_exact_body_headers_and_receipt_without_key_material() {
        use iroha_crypto::ExposedPrivateKey;
        use iroha_data_model::account::address::{ChainDiscriminantGuard, chain_discriminant};
        use std::os::unix::fs::PermissionsExt as _;
        let _default = ChainDiscriminantGuard::enter(753);
        let key = KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519);
        let network =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([11; 32])));
        let directory = tempfile::tempdir().expect("temporary runtime custody");
        let request_path = directory.path().join("request.json");
        let key_path = directory.path().join("account.key");
        let body_output = directory.path().join("body");
        let headers_output = directory.path().join("headers");
        let output = directory.path().join("receipt.json");
        let json = {
            let _taira = ChainDiscriminantGuard::enter(369);
            norito::json!({ "network_id": network, "authority": (AccountId::new(key.public_key().clone())), "action": "status", "url": "https://gateway.example.com/v1/sorafs/gateway/compliance/status", "body_blake3": (*blake3::hash(&[]).as_bytes()), "max_clock_skew_ms": 30000 })
        };
        let bytes = norito::json::to_vec(&json).expect("public request");
        write_new(&request_path, &bytes).expect("request file");
        let secret = ExposedPrivateKey(key.private_key().clone())
            .try_to_multihash_string()
            .expect("synthetic key text");
        write_new(&key_path, secret.as_bytes()).expect("private synthetic key");
        let command = |account_chain_discriminant| super::super::super::Command::Provision {
            account_chain_discriminant,
            command: super::super::Command::Request(RequestArgs {
                input: InputArgs {
                    request_json: request_path.clone(),
                    request_blake3_hex: *blake3::hash(&bytes).as_bytes(),
                    output: output.clone(),
                },
                body_file: None,
                body_output: body_output.clone(),
                headers_output: headers_output.clone(),
                private_key_file: key_path.clone(),
            }),
        };
        assert!(command(753).run_artifact().is_err());
        assert_eq!(chain_discriminant(), 753);
        assert!(!output.exists());
        command(369).run_artifact().expect("offline Taira request");
        assert_eq!(chain_discriminant(), 753);
        assert!(std::fs::read(&body_output).expect("body").is_empty());
        let receipt: RequestReceipt = {
            let _taira = ChainDiscriminantGuard::enter(369);
            norito::json::from_slice(&std::fs::read(&output).expect("receipt"))
                .expect("public Taira receipt")
        };
        assert_eq!(receipt.authority, AccountId::new(key.public_key().clone()));
        assert_eq!(receipt.method, "GET");
        assert!(!receipt.headers.contains_key("Idempotency-Key"));
        for path in [&body_output, &headers_output, &output] {
            assert_eq!(
                std::fs::metadata(path)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert!(
                !String::from_utf8_lossy(&std::fs::read(path).expect("public output"))
                    .contains(&secret)
            );
        }
    }
}
