use super::*;
use crate::{
    FilesystemGovernancePublisher, GovernanceDagCanonicalRequestHeaderV1,
    GovernanceDagRuntimeSigner, GovernanceDagSigningPurposeV1, GovernancePublisher, NodeHandle,
    NodeRuntimeDeps,
    config::StorageConfig,
    governance::{
        qualify_governance_dag_runtime_checkpoint_store,
        qualify_governance_dag_runtime_signer_provider,
        write_runtime_dag_committed_snapshot_fixture_v1,
    },
};
use axum::{
    body::Bytes,
    extract::State,
    http::{self, HeaderName, Request},
    response::Redirect,
    routing::{any, post},
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature as IrohaSignature};
use norito::codec::Encode as _;
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_LOG_VERSION_V1,
    GovernanceDagSubmissionOriginV1, GovernanceDagSubmissionProvenanceV1, GovernanceLogNodeV1,
    GovernanceLogSignatureV1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
    SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
    SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
        DealSettlementStatusV1, DealSettlementV1, XorQuantity,
    },
    governance_dag_block_cid_v1, governance_dag_submission_account_digest_v1,
};
use std::{
    fmt,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Barrier, Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
    },
};
use tempfile::TempDir;
use tokio::{sync::Mutex, task::JoinHandle};
use tower::ServiceExt as _;
#[test]
fn service_default_request_bound_covers_single_entry_archive_ceiling() {
    let service = SorafsGovernanceDagService::default();
    let max_request_bytes =
        usize::try_from(service.max_request_bytes.0).expect("default request ceiling fits usize");
    assert_eq!(max_request_bytes, BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1);
    assert!(
        max_request_bytes > GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1,
        "archive publication has an explicit wrapper allowance"
    );
    let archive_decode_limits =
        block_prefix_archive_decode_limits(BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1);
    assert!(archive_decode_limits.max_total_elements() > MAX_REPUTATION_TRUST_EDGES);
}
#[test]
fn maximum_admitted_block_has_exact_archive_and_request_boundary() {
    let request_ceiling = u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1)
        .expect("archive request ceiling fits u64");
    assert_eq!(
        BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1 - GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1,
        BLOCK_PREFIX_ARCHIVE_CANONICAL_OVERHEAD_BYTES_V1,
    );
    assert_eq!(
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1 - BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1,
        BLOCK_PREFIX_ARCHIVE_MULTIPART_OVERHEAD_BYTES_V1,
    );
    assert!(block_prefix_archive_lengths_fit(
        BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1,
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1,
        request_ceiling,
    ));
    assert!(!block_prefix_archive_lengths_fit(
        BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1 + 1,
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1,
        request_ceiling,
    ));
    assert!(!block_prefix_archive_lengths_fit(
        BLOCK_PREFIX_ARCHIVE_MAX_CANONICAL_BYTES_V1,
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1 + 1,
        request_ceiling,
    ));
}
// Keep one target-gated assertion for every ABI branch. Overlapping branches
// fail with duplicate definitions; missing branches fail to resolve the flag.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
#[test]
fn linux_no_follow_flag_matches_low_flag_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
#[test]
fn linux_no_follow_flag_matches_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
#[test]
fn android_arm_no_follow_flag_matches_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
#[test]
fn android_x86_no_follow_flag_matches_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
}
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
#[test]
fn android_riscv64_no_follow_flag_matches_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x400000);
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "riscv32", target_arch = "riscv64")
))]
#[test]
fn linux_riscv_no_follow_flag_remains_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
}
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
#[test]
fn apple_and_bsd_no_follow_flag_matches_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
}
const TEST_CID_PAYLOAD: &str = "bafkreibdt5m62vphg7dxcr6pkwwqygydbnwx5z2iu5bgsuxzxbjnlkjv4u";
const TEST_CID_BLOCK: &str = "bafkreicjnlfibzgy6kp3r2gnqfwdv62i2pyqhfylhixocyambdfgomtn5y";
const TEST_CID_HEAD: &str = "bafkreie7fzwthi3rp3ucmnj2ibf2iymndlxlnb4226jwxtuo2x2gqfesju";
fn xor(value: &str) -> XorQuantity {
    value.parse().expect("canonical XOR quantity")
}
const TEST_CID_OLD: &str = "bafkreiglubvvonx26z7fjmd3kypk5fbzlz3uyul2pwiquvbwtyjghth32q";
const TEST_CID_NEW: &str = "bafkreiarkb5a4l26nhk57jakmkq3263o4v7gxtmfyz6jxbbrwnx76ioeg4";
const TEST_CID_ATTACKER: &str = "bafkreihgjoryus4vrrzlydkccfilursggzbcjbpnol5locdmo2i44qaizq";
const KUBO_INTEGRATION_ENV: &str = "SORAFS_RUN_KUBO_INTEGRATION";
const KUBO_BIN_ENV: &str = "SORAFS_KUBO_BIN";
const KUBO_CONFORMANCE_VERSION_V1: &str = "0.42.0";
const TEST_IPFS_AUTH_HANDLE: &str = "vault:governance/ipfs:primary";
const TEST_HEAD_AUTH_HANDLE: &str = "vault:governance/head:primary";
const TEST_CHECKPOINT_STORE_HANDLE: &str = "kms:governance/checkpoint:primary";
const TEST_PRODUCER_SIGNER_HANDLE: &str = "hsm:governance/source-signer:primary";
const TEST_PRODUCER_PEER_ID: &str = "12D3KooWGovernanceServiceTest";
const TEST_AUTH_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
    GovernanceDagRuntimeProviderQualificationV1::new(1, [0x81; 32]);
const TEST_STORE_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
    GovernanceDagRuntimeProviderQualificationV1::new(1, [0x82; 32]);
const TEST_PRODUCER_SIGNER_QUALIFICATION: GovernanceDagRuntimeProviderQualificationV1 =
    GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]);
const TEST_RECEIVER_POLICY_DIGEST: [u8; 32] = [0x91; 32];
const TEST_REPLAY_NAMESPACE_DIGEST: [u8; 32] = [0x92; 32];
const TEST_INGRESS_REPLICA_SET_DIGEST: [u8; 32] = [0x93; 32];
struct TestAuthenticator {
    handle: String,
    private_key: PrivateKey,
    public_key: [u8; 32],
    ingress_binding: StdMutex<GovernanceDagRequestIngressBindingV1>,
    provider_secret: StdMutex<String>,
    nonce_counter: AtomicU64,
    qualification_revision: AtomicU64,
    qualification_refuse: AtomicBool,
    drift_during_authentication: AtomicBool,
    refuse: AtomicBool,
}
impl fmt::Debug for TestAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestAuthenticator")
            .field("handle", &self.handle)
            .field("hsm", &"[REDACTED]")
            .finish()
    }
}
impl TestAuthenticator {
    fn new(handle: &str, provider_secret: &str) -> Self {
        let (private_key, public_key) = test_request_auth_keypair(handle);
        Self {
            handle: handle.to_owned(),
            private_key,
            public_key,
            ingress_binding: StdMutex::new(default_test_request_ingress_binding(handle)),
            provider_secret: StdMutex::new(provider_secret.to_owned()),
            nonce_counter: AtomicU64::new(1),
            qualification_revision: AtomicU64::new(1),
            qualification_refuse: AtomicBool::new(false),
            drift_during_authentication: AtomicBool::new(false),
            refuse: AtomicBool::new(false),
        }
    }
    fn with_ingress_binding(
        mut self,
        ingress_binding: GovernanceDagRequestIngressBindingV1,
    ) -> Self {
        *self
            .ingress_binding
            .get_mut()
            .expect("access test ingress binding") = ingress_binding;
        self
    }
    fn ingress_binding(&self) -> GovernanceDagRequestIngressBindingV1 {
        *self
            .ingress_binding
            .lock()
            .expect("lock test ingress binding")
    }
    fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
        *self
            .ingress_binding
            .lock()
            .expect("lock test ingress binding") = ingress_binding;
    }
    fn rotate(&self, provider_secret: &str) {
        *self
            .provider_secret
            .lock()
            .expect("lock test provider diagnostic") = provider_secret.to_owned();
    }
    fn signed_envelope(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
        let now = current_unix_timestamp_seconds();
        let counter = self.nonce_counter.fetch_add(1, AtomicOrdering::SeqCst);
        let mut nonce = blake3_array(self.handle.as_bytes());
        nonce[..8].copy_from_slice(&counter.to_be_bytes());
        let payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
            request,
            now,
            now.saturating_add(15),
            nonce,
            self.public_key,
        );
        let signature =
            IrohaSignature::try_new(&self.private_key, &payload).map_err(|_| "signing")?;
        let signature: [u8; 64] = signature
            .payload()
            .try_into()
            .map_err(|_| "signature length")?;
        GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
            request,
            now,
            now.saturating_add(15),
            nonce,
            self.public_key,
            signature,
        )
        .map_err(str::to_owned)
    }
}
fn test_request_auth_keypair(handle: &str) -> (PrivateKey, [u8; 32]) {
    let seed = blake3_array(handle.as_bytes());
    let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &seed)
        .expect("test request-auth Ed25519 seed is valid");
    let keypair =
        KeyPair::from_private_key(private_key.clone()).expect("derive test request-auth keypair");
    let (algorithm, bytes) = keypair
        .public_key()
        .try_to_bytes()
        .expect("encode test request-auth public key");
    assert_eq!(algorithm, Algorithm::Ed25519);
    let public_key = bytes
        .try_into()
        .expect("test Ed25519 public key has 32 bytes");
    (private_key, public_key)
}
fn test_request_auth_public_key(handle: &str) -> [u8; 32] {
    test_request_auth_keypair(handle).1
}
fn default_test_request_ingress_binding(handle: &str) -> GovernanceDagRequestIngressBindingV1 {
    let config = SorafsGovernanceDagService::default();
    let (scope, endpoint, max_body_bytes) = if handle == TEST_HEAD_AUTH_HANDLE {
        (
            GovernanceDagAuthenticationScope::SignedHead,
            "http://127.0.0.1:9099/head",
            config.max_request_bytes.0,
        )
    } else {
        (
            GovernanceDagAuthenticationScope::Ipfs,
            "http://127.0.0.1:5001",
            authenticated_ipfs_wire_body_max_bytes(config.max_request_bytes.0)
                .expect("derive test IPFS ingress body bound"),
        )
    };
    configured_request_ingress_binding(
        scope,
        endpoint,
        test_request_auth_public_key(handle),
        max_body_bytes,
        config.request_auth_max_envelope_lifetime_secs,
        config.request_auth_max_future_skew_secs,
        "test authenticator",
    )
    .expect("construct test request-ingress binding")
}
fn test_ingress_qualification(
    provider: GovernanceDagRuntimeProviderQualificationV1,
    binding: GovernanceDagRequestIngressBindingV1,
) -> GovernanceDagRequestIngressQualificationV1 {
    GovernanceDagRequestIngressQualificationV1::try_new(
        provider,
        binding,
        TEST_RECEIVER_POLICY_DIGEST,
        TEST_REPLAY_NAMESPACE_DIGEST,
        TEST_INGRESS_REPLICA_SET_DIGEST,
    )
    .expect("construct live test request-ingress qualification")
}
fn signed_test_request_auth_envelope(
    handle: &str,
    request: &GovernanceDagCanonicalRequestV1,
    issued_at: u64,
    expires_at: u64,
    nonce: [u8; 32],
) -> GovernanceDagRequestAuthenticationEnvelopeV1 {
    let (private_key, public_key) = test_request_auth_keypair(handle);
    let payload = GovernanceDagRequestAuthenticationEnvelopeV1::signing_payload(
        request, issued_at, expires_at, nonce, public_key,
    );
    let signature =
        IrohaSignature::try_new(&private_key, &payload).expect("sign test request-auth payload");
    let signature = signature
        .payload()
        .try_into()
        .expect("test Ed25519 signature has 64 bytes");
    GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
        request, issued_at, expires_at, nonce, public_key, signature,
    )
    .expect("construct test request-auth envelope")
}
fn request_auth_header_fields(
    envelope: &GovernanceDagRequestAuthenticationEnvelopeV1,
) -> Vec<(String, Vec<u8>)> {
    governance_dag_request_authentication_headers_v1(envelope)
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value.into_bytes()))
        .collect()
}
fn verify_request_before_test_backend(
    request: &GovernanceDagCanonicalRequestV1,
    headers: &[(String, Vec<u8>)],
    body: &[u8],
    expected_scope: GovernanceDagAuthenticationScope,
    policy: &GovernanceDagRequestAuthenticationPolicyV1,
    now: u64,
    replay_cache: &mut dyn crate::GovernanceDagRequestAuthenticationReplayStoreV1,
    backend_calls: &AtomicU64,
) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
    if request.scope() != expected_scope {
        return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
    }
    let request_url = Url::parse(request.canonical_url())
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    let mut endpoint = request_url.clone();
    endpoint.set_query(None);
    if expected_scope == GovernanceDagAuthenticationScope::Ipfs {
        endpoint.set_path("/");
    }
    let endpoint_binding =
        governance_dag_request_ingress_endpoint_binding_v1(expected_scope, endpoint.as_str())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    let binding = GovernanceDagRequestIngressBindingV1::try_new(
        expected_scope,
        endpoint_binding,
        policy.public_key(),
        1024 * 1024,
        policy.max_envelope_lifetime_secs(),
        policy.max_future_skew_secs(),
    )
    .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    let origin = request_url.origin().ascii_serialization();
    let authority = origin
        .split_once("://")
        .map(|(_, authority)| authority)
        .ok_or(GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    let target = match request_url.query() {
        Some(query) => format!("{}?{query}", request_url.path()),
        None => request_url.path().to_owned(),
    };
    let mut http_request = Request::builder()
        .method(request.method())
        .uri(target.as_str())
        .version(http::Version::HTTP_11)
        .header(header::HOST, authority)
        .body(body.to_vec())
        .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalRequest)?;
    for selected in request.selected_headers() {
        let name = HeaderName::from_bytes(selected.name().as_bytes())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
        let value = HeaderValue::from_bytes(selected.value().as_bytes())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
        http_request.headers_mut().append(name, value);
    }
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
        let value = HeaderValue::from_bytes(value)
            .map_err(|_| GovernanceDagRequestAuthenticationErrorV1::NoncanonicalHeader)?;
        http_request.headers_mut().append(name, value);
    }
    let mut receiver =
        GovernanceDagHttpRequestReceiverV1::try_new(endpoint.as_str(), binding, replay_cache)?;
    let verified = receiver.verify_http_request(http_request, now)?;
    if verified.descriptor() != request
        || !verified.request().headers().contains_key(header::HOST)
        || verified.request().uri().scheme().is_some()
        || verified.request().uri().authority().is_some()
        || verified
            .request()
            .uri()
            .path_and_query()
            .map(http::uri::PathAndQuery::as_str)
            != Some(target.as_str())
        || GOVERNANCE_DAG_REQUEST_AUTH_HEADER_NAMES_V1
            .iter()
            .any(|name| verified.request().headers().contains_key(*name))
    {
        return Err(GovernanceDagRequestAuthenticationErrorV1::RequestMismatch);
    }
    backend_calls.fetch_add(1, AtomicOrdering::SeqCst);
    Ok(())
}
#[derive(Debug)]
struct UnavailableTestReplayStore;
impl crate::GovernanceDagRequestAuthenticationReplayStoreV1 for UnavailableTestReplayStore {
    fn consume_nonce(
        &mut self,
        _nonce: [u8; 32],
        _expires_at_unix_secs: u64,
        _now_unix_secs: u64,
    ) -> Result<(), GovernanceDagRequestAuthenticationErrorV1> {
        Err(GovernanceDagRequestAuthenticationErrorV1::ReplayStoreUnavailable)
    }
}
fn canonical_test_request(
    scope: GovernanceDagAuthenticationScope,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> GovernanceDagCanonicalRequestV1 {
    let mut headers = headers
        .iter()
        .map(|(name, value)| {
            GovernanceDagCanonicalRequestHeaderV1::try_new(name, value)
                .expect("canonical test request header")
        })
        .collect::<Vec<_>>();
    headers.sort_unstable();
    GovernanceDagCanonicalRequestV1::try_new(
        scope,
        method,
        url,
        headers,
        body.len() as u64,
        blake3_array(body),
        1024 * 1024,
    )
    .expect("canonical test request")
}
impl GovernanceDagRequestAuthenticator for TestAuthenticator {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn ingress_qualification(&self) -> Result<GovernanceDagRequestIngressQualificationV1, String> {
        if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
            return Err("auth_token=must-never-escape".to_owned());
        }
        Ok(test_ingress_qualification(
            GovernanceDagRuntimeProviderQualificationV1::new(
                self.qualification_revision.load(AtomicOrdering::SeqCst),
                [0x81; 32],
            ),
            self.ingress_binding(),
        ))
    }
    fn authenticate(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
        if self
            .drift_during_authentication
            .swap(false, AtomicOrdering::SeqCst)
        {
            self.qualification_revision
                .fetch_add(1, AtomicOrdering::SeqCst);
        }
        if self.refuse.load(AtomicOrdering::SeqCst) {
            return Err(format!(
                "hsm_diagnostic={}",
                self.provider_secret.lock().map_err(|_| "poisoned")?
            ));
        }
        self.signed_envelope(request)
    }
}
trait TestRebindableRequestAuthenticator: GovernanceDagRequestAuthenticator {
    fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1);
}
impl TestRebindableRequestAuthenticator for TestAuthenticator {
    fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
        TestAuthenticator::rebind_ingress(self, ingress_binding);
    }
}
struct FinalRequestAuthenticator {
    signer: TestAuthenticator,
    expected_body_length: u64,
    expected_body_blake3: [u8; 32],
    expected_condition: HeaderName,
    expected_condition_value: HeaderValue,
    observed_put: AtomicBool,
}
impl fmt::Debug for FinalRequestAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FinalRequestAuthenticator")
            .field("expected_body", &"[REDACTED]")
            .field("expected_condition", &self.expected_condition)
            .finish_non_exhaustive()
    }
}
impl FinalRequestAuthenticator {
    fn new(
        expected_body: &[u8],
        expected_condition: HeaderName,
        expected_condition_value: HeaderValue,
    ) -> Self {
        Self {
            signer: TestAuthenticator::new(TEST_HEAD_AUTH_HANDLE, "final-request-hsm"),
            expected_body_length: expected_body.len() as u64,
            expected_body_blake3: blake3_array(expected_body),
            expected_condition,
            expected_condition_value,
            observed_put: AtomicBool::new(false),
        }
    }
}
impl GovernanceDagRequestAuthenticator for FinalRequestAuthenticator {
    fn handle(&self) -> &str {
        TEST_HEAD_AUTH_HANDLE
    }
    fn ingress_qualification(&self) -> Result<GovernanceDagRequestIngressQualificationV1, String> {
        self.signer.ingress_qualification()
    }
    fn authenticate(
        &self,
        request: &GovernanceDagCanonicalRequestV1,
    ) -> Result<GovernanceDagRequestAuthenticationEnvelopeV1, String> {
        if request.method() == Method::PUT.as_str() {
            let expected_condition = self.expected_condition.as_str();
            let expected_condition_value = self
                .expected_condition_value
                .to_str()
                .map_err(|_| "noncanonical expected condition")?;
            let observed = request
                .selected_headers()
                .iter()
                .map(|header| (header.name(), header.value()))
                .collect::<BTreeMap<_, _>>();
            if request.scope() != GovernanceDagAuthenticationScope::SignedHead
                || observed.get(header::CONTENT_TYPE.as_str()).copied()
                    != Some("application/vnd.iroha.norito")
                || observed.get(expected_condition).copied() != Some(expected_condition_value)
                || request.body_length() != self.expected_body_length
                || request.body_blake3() != self.expected_body_blake3
            {
                return Err(
                    "signed-head authenticator received an incomplete PUT request".to_owned(),
                );
            }
            self.observed_put.store(true, AtomicOrdering::SeqCst);
        }
        self.signer.signed_envelope(request)
    }
}
impl TestRebindableRequestAuthenticator for FinalRequestAuthenticator {
    fn rebind_ingress(&self, ingress_binding: GovernanceDagRequestIngressBindingV1) {
        self.signer.rebind_ingress(ingress_binding);
    }
}
#[derive(Default)]
struct TestSealedStoreInner {
    checkpoint: Option<GovernanceDagSealedStateRecord>,
    publish_intent: Option<GovernanceDagSealedStateRecord>,
    producer_checkpoint: Option<GovernanceDagSealedStateRecord>,
    producer_publish_intent: Option<GovernanceDagSealedStateRecord>,
    ipfs_request_replay: Option<GovernanceDagSealedStateRecord>,
    signed_head_request_replay: Option<GovernanceDagSealedStateRecord>,
    checkpoint_generation_floor: u64,
    intent_generation_floor: u64,
    producer_checkpoint_generation_floor: u64,
    producer_intent_generation_floor: u64,
    ipfs_request_replay_generation_floor: u64,
    signed_head_request_replay_generation_floor: u64,
}
struct TestSealedStore {
    handle: String,
    inner: StdMutex<TestSealedStoreInner>,
    checkpoint_load_count: AtomicU64,
    checkpoint_second_load: StdMutex<Option<GovernanceDagSealedStateRecord>>,
    intent_load_count: AtomicU64,
    intent_second_load: StdMutex<Option<GovernanceDagSealedStateRecord>>,
    qualification_revision: AtomicU64,
    qualification_refuse: AtomicBool,
    drift_during_operation: AtomicBool,
    drift_during_replay_cas: AtomicBool,
    refuse: AtomicBool,
    replay_load_barrier: Option<Arc<Barrier>>,
    replay_initial_loads_remaining: AtomicU64,
    replay_cas_calls: AtomicU64,
    replay_cas_completed: AtomicBool,
    diverge_replay_readback: AtomicBool,
}
impl fmt::Debug for TestSealedStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TestSealedStore")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}
impl TestSealedStore {
    fn new(handle: &str) -> Self {
        Self {
            handle: handle.to_owned(),
            inner: StdMutex::new(TestSealedStoreInner::default()),
            checkpoint_load_count: AtomicU64::new(0),
            checkpoint_second_load: StdMutex::new(None),
            intent_load_count: AtomicU64::new(0),
            intent_second_load: StdMutex::new(None),
            qualification_revision: AtomicU64::new(1),
            qualification_refuse: AtomicBool::new(false),
            drift_during_operation: AtomicBool::new(false),
            drift_during_replay_cas: AtomicBool::new(false),
            refuse: AtomicBool::new(false),
            replay_load_barrier: None,
            replay_initial_loads_remaining: AtomicU64::new(0),
            replay_cas_calls: AtomicU64::new(0),
            replay_cas_completed: AtomicBool::new(false),
            diverge_replay_readback: AtomicBool::new(false),
        }
    }
    fn with_replay_load_barrier(mut self, barrier: Arc<Barrier>) -> Self {
        self.replay_load_barrier = Some(barrier);
        self.replay_initial_loads_remaining = AtomicU64::new(2);
        self
    }
    fn maybe_drift(&self) {
        if self
            .drift_during_operation
            .swap(false, AtomicOrdering::SeqCst)
        {
            self.qualification_revision
                .fetch_add(1, AtomicOrdering::SeqCst);
        }
    }
    fn return_checkpoint_on_second_load(&self, record: GovernanceDagSealedStateRecord) {
        *self
            .checkpoint_second_load
            .lock()
            .expect("lock checkpoint race fixture") = Some(record);
        self.checkpoint_load_count.store(0, AtomicOrdering::SeqCst);
    }
    fn return_intent_on_second_load(&self, record: GovernanceDagSealedStateRecord) {
        *self
            .intent_second_load
            .lock()
            .expect("lock intent race fixture") = Some(record);
        self.intent_load_count.store(0, AtomicOrdering::SeqCst);
    }
    fn slot(
        inner: &TestSealedStoreInner,
        slot: GovernanceDagSealedStateSlot,
    ) -> &Option<GovernanceDagSealedStateRecord> {
        match slot {
            GovernanceDagSealedStateSlot::Checkpoint => &inner.checkpoint,
            GovernanceDagSealedStateSlot::PublishIntent => &inner.publish_intent,
            GovernanceDagSealedStateSlot::ProducerCheckpoint => &inner.producer_checkpoint,
            GovernanceDagSealedStateSlot::ProducerPublishIntent => &inner.producer_publish_intent,
            GovernanceDagSealedStateSlot::IpfsRequestReplay => &inner.ipfs_request_replay,
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => {
                &inner.signed_head_request_replay
            }
        }
    }
    fn slot_mut(
        inner: &mut TestSealedStoreInner,
        slot: GovernanceDagSealedStateSlot,
    ) -> &mut Option<GovernanceDagSealedStateRecord> {
        match slot {
            GovernanceDagSealedStateSlot::Checkpoint => &mut inner.checkpoint,
            GovernanceDagSealedStateSlot::PublishIntent => &mut inner.publish_intent,
            GovernanceDagSealedStateSlot::ProducerCheckpoint => &mut inner.producer_checkpoint,
            GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                &mut inner.producer_publish_intent
            }
            GovernanceDagSealedStateSlot::IpfsRequestReplay => &mut inner.ipfs_request_replay,
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => {
                &mut inner.signed_head_request_replay
            }
        }
    }
}
impl GovernanceDagSealedCheckpointStore for TestSealedStore {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if self.qualification_refuse.load(AtomicOrdering::SeqCst) {
            return Err("kms_access_token=must-never-escape".to_owned());
        }
        Ok(GovernanceDagRuntimeProviderQualificationV1::new(
            self.qualification_revision.load(AtomicOrdering::SeqCst),
            [0x82; 32],
        ))
    }
    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
        self.maybe_drift();
        if self.refuse.load(AtomicOrdering::SeqCst) {
            return Err("kms_access_token=must-never-escape".to_owned());
        }
        let raced = match slot {
            GovernanceDagSealedStateSlot::Checkpoint
                if self
                    .checkpoint_load_count
                    .fetch_add(1, AtomicOrdering::SeqCst)
                    == 1 =>
            {
                self.checkpoint_second_load
                    .lock()
                    .map_err(|_| "poisoned".to_owned())?
                    .clone()
            }
            GovernanceDagSealedStateSlot::PublishIntent
                if self.intent_load_count.fetch_add(1, AtomicOrdering::SeqCst) == 1 =>
            {
                self.intent_second_load
                    .lock()
                    .map_err(|_| "poisoned".to_owned())?
                    .clone()
            }
            _ => None,
        };
        if raced.is_some() {
            return Ok(raced);
        }
        let inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
        let mut record = Self::slot(&inner, slot).clone();
        drop(inner);
        if matches!(
            slot,
            GovernanceDagSealedStateSlot::IpfsRequestReplay
                | GovernanceDagSealedStateSlot::SignedHeadRequestReplay
        ) && self.replay_cas_completed.load(AtomicOrdering::SeqCst)
            && self
                .diverge_replay_readback
                .swap(false, AtomicOrdering::SeqCst)
            && let Some(observed) = &record
        {
            record = Some(GovernanceDagSealedStateRecord::new(
                slot,
                observed.generation,
                norito::to_bytes(&RequestAuthReplayStateV1 {
                    version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
                    entries: Vec::new(),
                })
                .expect("encode divergent replay readback"),
            ));
        }
        if matches!(
            slot,
            GovernanceDagSealedStateSlot::IpfsRequestReplay
                | GovernanceDagSealedStateSlot::SignedHeadRequestReplay
        ) && self
            .replay_initial_loads_remaining
            .fetch_update(
                AtomicOrdering::SeqCst,
                AtomicOrdering::SeqCst,
                |remaining| remaining.checked_sub(1),
            )
            .is_ok()
            && let Some(barrier) = &self.replay_load_barrier
        {
            barrier.wait();
        }
        Ok(record)
    }
    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), String> {
        let is_replay_slot = matches!(
            slot,
            GovernanceDagSealedStateSlot::IpfsRequestReplay
                | GovernanceDagSealedStateSlot::SignedHeadRequestReplay
        );
        if is_replay_slot {
            self.replay_cas_calls.fetch_add(1, AtomicOrdering::SeqCst);
            if self
                .drift_during_replay_cas
                .swap(false, AtomicOrdering::SeqCst)
            {
                self.qualification_revision
                    .fetch_add(1, AtomicOrdering::SeqCst);
            }
        }
        self.maybe_drift();
        if self.refuse.load(AtomicOrdering::SeqCst) {
            return Err("kms_access_token=must-never-escape".to_owned());
        }
        if !next.has_valid_revision(slot) || next.generation == 0 {
            return Err("invalid sealed record".to_owned());
        }
        let mut inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
        let current_revision = Self::slot(&inner, slot)
            .as_ref()
            .map(|record| record.revision);
        if current_revision != expected_revision {
            return Err("compare-and-swap conflict".to_owned());
        }
        let floor = match slot {
            GovernanceDagSealedStateSlot::Checkpoint => inner.checkpoint_generation_floor,
            GovernanceDagSealedStateSlot::PublishIntent => inner.intent_generation_floor,
            GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                inner.producer_checkpoint_generation_floor
            }
            GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                inner.producer_intent_generation_floor
            }
            GovernanceDagSealedStateSlot::IpfsRequestReplay => {
                inner.ipfs_request_replay_generation_floor
            }
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => {
                inner.signed_head_request_replay_generation_floor
            }
        };
        let generation_valid = match slot {
            GovernanceDagSealedStateSlot::Checkpoint => next.generation > floor,
            GovernanceDagSealedStateSlot::PublishIntent if Self::slot(&inner, slot).is_some() => {
                next.generation >= floor
            }
            GovernanceDagSealedStateSlot::PublishIntent => next.generation > floor,
            GovernanceDagSealedStateSlot::ProducerCheckpoint
            | GovernanceDagSealedStateSlot::ProducerPublishIntent
            | GovernanceDagSealedStateSlot::IpfsRequestReplay
            | GovernanceDagSealedStateSlot::SignedHeadRequestReplay => next.generation > floor,
        };
        if !generation_valid {
            return Err("monotonic generation rollback".to_owned());
        }
        match slot {
            GovernanceDagSealedStateSlot::Checkpoint => {
                inner.checkpoint_generation_floor = next.generation;
            }
            GovernanceDagSealedStateSlot::PublishIntent => {
                inner.intent_generation_floor = next.generation;
            }
            GovernanceDagSealedStateSlot::ProducerCheckpoint => {
                inner.producer_checkpoint_generation_floor = next.generation;
            }
            GovernanceDagSealedStateSlot::ProducerPublishIntent => {
                inner.producer_intent_generation_floor = next.generation;
            }
            GovernanceDagSealedStateSlot::IpfsRequestReplay => {
                inner.ipfs_request_replay_generation_floor = next.generation;
            }
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => {
                inner.signed_head_request_replay_generation_floor = next.generation;
            }
        }
        *Self::slot_mut(&mut inner, slot) = Some(next);
        if is_replay_slot {
            self.replay_cas_completed
                .store(true, AtomicOrdering::SeqCst);
        }
        Ok(())
    }
    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String> {
        self.maybe_drift();
        if self.refuse.load(AtomicOrdering::SeqCst) {
            return Err("kms_access_token=must-never-escape".to_owned());
        }
        let mut inner = self.inner.lock().map_err(|_| "poisoned".to_owned())?;
        let current_revision = Self::slot(&inner, slot)
            .as_ref()
            .map(|record| record.revision);
        if current_revision != Some(expected_revision) {
            return Err("compare-and-swap conflict".to_owned());
        }
        *Self::slot_mut(&mut inner, slot) = None;
        Ok(())
    }
}
fn test_request_auth_policy(public_key: [u8; 32]) -> GovernanceDagRequestAuthenticationPolicyV1 {
    validate_request_auth_policy(public_key, 30, 5, "test authenticator")
        .expect("construct test request-auth policy")
}
fn test_authenticator(
    handle: &str,
    authentication_scope: GovernanceDagAuthenticationScope,
) -> OpaqueAuthenticator {
    let provider = Arc::new(TestAuthenticator::new(handle, "test-only-hsm"));
    OpaqueAuthenticator::try_new(
        handle,
        TEST_AUTH_QUALIFICATION,
        provider.ingress_binding(),
        provider,
        authentication_scope,
        "test authenticator",
    )
    .expect("bind test authenticator")
}
fn bind_test_authenticator_to_endpoint<T>(
    provider: Arc<T>,
    scope: GovernanceDagAuthenticationScope,
    endpoint: &str,
    max_body_bytes: u64,
) -> OpaqueAuthenticator
where
    T: TestRebindableRequestAuthenticator + 'static,
{
    let current = provider
        .ingress_qualification()
        .expect("read test request-ingress qualification");
    let current_binding = current.binding();
    let endpoint_binding = governance_dag_request_ingress_endpoint_binding_v1(scope, endpoint)
        .expect("bind canonical test endpoint");
    let ingress_binding = GovernanceDagRequestIngressBindingV1::try_new(
        scope,
        endpoint_binding,
        current_binding.public_key(),
        max_body_bytes,
        current_binding.max_envelope_lifetime_secs(),
        current_binding.max_future_skew_secs(),
    )
    .expect("bind exact test request ingress");
    provider.rebind_ingress(ingress_binding);
    let handle = provider.handle().to_owned();
    let provider: Arc<dyn GovernanceDagRequestAuthenticator> = provider;
    OpaqueAuthenticator::try_new(
        &handle,
        current.provider(),
        ingress_binding,
        provider,
        scope,
        "test authenticator",
    )
    .expect("qualify exact test endpoint authenticator")
}
fn test_runtime_providers(
    view: &SorafsGovernanceDagServiceView,
    checkpoint_store: Arc<TestSealedStore>,
) -> GovernanceDagServiceRuntimeProviders {
    let bindings = runtime_provider_bindings(view).expect("derive test runtime bindings");
    let head_authenticator = bindings.head_request_ingress_binding().map(|binding| {
        let provider: Arc<dyn GovernanceDagRequestAuthenticator> = Arc::new(
            TestAuthenticator::new(TEST_HEAD_AUTH_HANDLE, "test-only-head-bearer")
                .with_ingress_binding(binding),
        );
        provider
    });
    GovernanceDagServiceRuntimeProviders {
        ipfs_authenticator: Some(Arc::new(
            TestAuthenticator::new(TEST_IPFS_AUTH_HANDLE, "test-only-ipfs-bearer")
                .with_ingress_binding(bindings.ipfs_request_ingress_binding()),
        )),
        head_authenticator,
        checkpoint_store: Some(checkpoint_store),
    }
}
struct TestRuntimeProviderRegistry {
    providers: GovernanceDagServiceRuntimeProviders,
    failure: Option<GovernanceDagServiceRuntimeProviderRegistryErrorV1>,
    observed_bindings: StdMutex<Option<GovernanceDagServiceRuntimeProviderBindingsV1>>,
}
impl TestRuntimeProviderRegistry {
    fn returning(providers: GovernanceDagServiceRuntimeProviders) -> Self {
        Self {
            providers,
            failure: None,
            observed_bindings: StdMutex::new(None),
        }
    }
    fn failing(failure: GovernanceDagServiceRuntimeProviderRegistryErrorV1) -> Self {
        Self {
            providers: GovernanceDagServiceRuntimeProviders::default(),
            failure: Some(failure),
            observed_bindings: StdMutex::new(None),
        }
    }
}
impl GovernanceDagServiceRuntimeProviderRegistryV1 for TestRuntimeProviderRegistry {
    fn resolve(
        &self,
        bindings: &GovernanceDagServiceRuntimeProviderBindingsV1,
    ) -> Result<
        GovernanceDagServiceRuntimeProviders,
        GovernanceDagServiceRuntimeProviderRegistryErrorV1,
    > {
        *self
            .observed_bindings
            .lock()
            .expect("lock observed registry bindings") = Some(bindings.clone());
        if let Some(failure) = self.failure {
            return Err(failure);
        }
        Ok(self.providers.clone())
    }
}
fn test_checkpoint_store(provider: Arc<TestSealedStore>) -> OpaqueCheckpointStore {
    OpaqueCheckpointStore::try_new(
        TEST_CHECKPOINT_STORE_HANDLE,
        TEST_STORE_QUALIFICATION,
        provider,
    )
    .expect("bind test sealed checkpoint store")
}
fn test_sealed_http_receiver(
    scope: GovernanceDagAuthenticationScope,
    provider: Arc<TestSealedStore>,
) -> GovernanceDagSealedHttpRequestReceiverV1 {
    let authenticator_handle = match scope {
        GovernanceDagAuthenticationScope::Ipfs => TEST_IPFS_AUTH_HANDLE,
        GovernanceDagAuthenticationScope::SignedHead => TEST_HEAD_AUTH_HANDLE,
    };
    GovernanceDagSealedHttpRequestReceiverV1::try_new(
        scope,
        1024 * 1024,
        test_request_auth_policy(test_request_auth_public_key(authenticator_handle)),
        TEST_CHECKPOINT_STORE_HANDLE,
        TEST_STORE_QUALIFICATION,
        Some(provider),
    )
    .expect("bind sealed HTTP ingress receiver")
}
fn sealed_receiver_request_parts(
    scope: GovernanceDagAuthenticationScope,
    method: &str,
    url: &str,
    selected_headers: &[(&str, &str)],
    body: &[u8],
    now: u64,
    nonce: [u8; 32],
) -> (GovernanceDagCanonicalRequestV1, Vec<(String, Vec<u8>)>) {
    let request = canonical_test_request(scope, method, url, selected_headers, body);
    let authenticator_handle = match scope {
        GovernanceDagAuthenticationScope::Ipfs => TEST_IPFS_AUTH_HANDLE,
        GovernanceDagAuthenticationScope::SignedHead => TEST_HEAD_AUTH_HANDLE,
    };
    let envelope =
        signed_test_request_auth_envelope(authenticator_handle, &request, now, now + 15, nonce);
    let mut headers = request_auth_header_fields(&envelope);
    headers.extend(
        selected_headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.as_bytes().to_vec())),
    );
    headers.push((
        "content-length".to_owned(),
        body.len().to_string().into_bytes(),
    ));
    (request, headers)
}
fn runtime_boundary_view(root: &Path) -> SorafsGovernanceDagServiceView {
    let source_dir = root.join("source");
    let state_dir = root.join("state");
    fs::create_dir_all(&source_dir).expect("create test source directory");
    let publisher_public_key_hex = hex::encode(
        ed25519_dalek::SigningKey::from_bytes(&[0x42; 32])
            .verifying_key()
            .to_bytes(),
    );
    SorafsGovernanceDagServiceView {
        source_dir: Some(source_dir),
        producer_publisher_peer_id: Some(TEST_PRODUCER_PEER_ID.to_owned()),
        producer_signer_handle: Some(TEST_PRODUCER_SIGNER_HANDLE.to_owned()),
        producer_signer_revision: Some(TEST_PRODUCER_SIGNER_QUALIFICATION.revision),
        producer_signer_policy_digest: Some(TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest),
        producer_publisher_public_key_hex: Some(publisher_public_key_hex.clone()),
        service: SorafsGovernanceDagService {
            enabled: true,
            state_dir: Some(state_dir),
            ipfs_api_url: Some("http://127.0.0.1:5001".to_owned()),
            signed_head_url: Some("http://127.0.0.1:9099/head".to_owned()),
            ipfs_authenticator_handle: Some(TEST_IPFS_AUTH_HANDLE.to_owned()),
            ipfs_authenticator_revision: Some(TEST_AUTH_QUALIFICATION.revision),
            ipfs_authenticator_policy_digest: Some(TEST_AUTH_QUALIFICATION.policy_digest),
            ipfs_request_auth_public_key: Some(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE)),
            head_authenticator_handle: Some(TEST_HEAD_AUTH_HANDLE.to_owned()),
            head_authenticator_revision: Some(TEST_AUTH_QUALIFICATION.revision),
            head_authenticator_policy_digest: Some(TEST_AUTH_QUALIFICATION.policy_digest),
            head_request_auth_public_key: Some(test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE)),
            checkpoint_store_handle: Some(TEST_CHECKPOINT_STORE_HANDLE.to_owned()),
            checkpoint_store_revision: Some(TEST_STORE_QUALIFICATION.revision),
            checkpoint_store_policy_digest: Some(TEST_STORE_QUALIFICATION.policy_digest),
            publisher_public_key_hex: Some(publisher_public_key_hex),
            allow_insecure_http: true,
            allow_private_ipfs_endpoint: true,
            allow_private_head_endpoint: true,
            max_request_bytes: iroha_config::base::util::Bytes(
                u64::try_from(BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1)
                    .expect("archive request ceiling fits u64"),
            ),
            listen_addr: "127.0.0.1:0".to_owned(),
            ..SorafsGovernanceDagService::default()
        },
    }
}
#[test]
fn configured_publisher_key_requires_one_canonical_strong_ed25519_point() {
    let valid = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32])
        .verifying_key()
        .to_bytes();
    assert_eq!(
        decode_strong_ed25519_public_key_hex(&hex::encode(valid), "publisher key")
            .expect("strong canonical key"),
        valid
    );
    let identity = {
        let mut encoded = [0_u8; 32];
        encoded[0] = 1;
        encoded
    };
    assert!(matches!(
        decode_strong_ed25519_public_key_hex(&hex::encode(identity), "publisher key"),
        Err(GovernanceDagServiceError::Config(message))
            if message.contains("canonical strong Ed25519")
    ));
    let mut noncanonical = [0xff_u8; 32];
    noncanonical[0] = 0xed;
    noncanonical[31] = 0x7f;
    assert!(
        decode_strong_ed25519_public_key_hex(&hex::encode(noncanonical), "publisher key").is_err()
    );
    assert!(
        decode_strong_ed25519_public_key_hex(&"11".repeat(32), "publisher key").is_err(),
        "mixed-torsion Ed25519 encodings must fail the production subgroup check"
    );
}
struct KuboHarness {
    _root: TempDir,
    repo: PathBuf,
    binary: PathBuf,
    api_url: String,
    daemon_log: PathBuf,
    child: Option<Child>,
}
impl KuboHarness {
    async fn start() -> Self {
        assert_eq!(
            std::env::var(KUBO_INTEGRATION_ENV).as_deref(),
            Ok("1"),
            "set {KUBO_INTEGRATION_ENV}=1 to run the isolated Kubo integration lane"
        );
        let binary = std::env::var_os(KUBO_BIN_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("ipfs"));
        let root = secure_temp_dir();
        let repo = root.path().join("ipfs-repo");
        fs::create_dir(&repo).expect("create isolated Kubo repository");
        #[cfg(unix)]
        fs::set_permissions(&repo, fs::Permissions::from_mode(0o700))
            .expect("secure isolated Kubo repository");
        let version_bytes = Self::run_command(&binary, &repo, &["version", "--number"]);
        let version = std::str::from_utf8(&version_bytes)
            .expect("Kubo version must be UTF-8")
            .trim();
        assert_eq!(
            version, KUBO_CONFORMANCE_VERSION_V1,
            "the fixed UnixFS profile must be checked against the release-pinned Kubo version"
        );
        Self::run_command(
            &binary,
            &repo,
            &[
                "init",
                "--empty-repo",
                "--profile=test,autoconf-off,announce-off",
            ],
        );
        Self::run_command(
            &binary,
            &repo,
            &["config", "Addresses.API", "/ip4/127.0.0.1/tcp/0"],
        );
        Self::run_command(
            &binary,
            &repo,
            &["config", "Addresses.Gateway", "/ip4/127.0.0.1/tcp/0"],
        );
        Self::run_command(
            &binary,
            &repo,
            &[
                "config",
                "--json",
                "Addresses.Swarm",
                r#"["/ip4/127.0.0.1/tcp/0"]"#,
            ],
        );
        Self::run_command(
            &binary,
            &repo,
            &["config", "--bool", "Discovery.MDNS.Enabled", "false"],
        );
        Self::assert_network_isolation(&binary, &repo);
        let daemon_log = root.path().join("kubo-daemon.log");
        let stdout = File::create(&daemon_log).expect("create Kubo daemon log");
        let stderr = stdout.try_clone().expect("clone Kubo daemon log handle");
        let child = Command::new(&binary)
            .arg("daemon")
            .env("IPFS_PATH", &repo)
            .env("IPFS_TELEMETRY", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .unwrap_or_else(|err| panic!("start isolated Kubo daemon: {err}"));
        let mut harness = Self {
            _root: root,
            repo,
            binary,
            api_url: String::new(),
            daemon_log,
            child: Some(child),
        };
        harness.api_url = harness.wait_for_api().await;
        harness.wait_until_ready().await;
        harness
    }
    fn run_command(binary: &Path, repo: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(binary)
            .args(args)
            .env("IPFS_PATH", repo)
            .env("IPFS_TELEMETRY", "off")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|err| panic!("run isolated Kubo command `{args:?}`: {err}"));
        assert!(
            output.status.success(),
            "isolated Kubo command `{args:?}` failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        output.stdout
    }
    fn assert_network_isolation(binary: &Path, repo: &Path) {
        let bytes = Self::run_command(binary, repo, &["config", "show"]);
        let config: JsonValue =
            json::from_slice(&bytes).expect("isolated Kubo config must be JSON");
        let null_or_empty = |value: Option<&JsonValue>| {
            value.is_none_or(|value| value.is_null() || value.as_array().is_some_and(Vec::is_empty))
        };
        assert_eq!(
            config
                .get("AutoConf")
                .and_then(|value| value.get("Enabled"))
                .and_then(JsonValue::as_bool),
            Some(false),
            "isolated Kubo must disable remote AutoConf"
        );
        assert!(
            null_or_empty(config.get("Bootstrap")),
            "isolated Kubo must have no bootstrap peers"
        );
        assert!(null_or_empty(
            config.get("DNS").and_then(|value| value.get("Resolvers"))
        ));
        assert!(null_or_empty(
            config
                .get("Ipns")
                .and_then(|value| value.get("DelegatedPublishers"))
        ));
        assert!(null_or_empty(
            config
                .get("Routing")
                .and_then(|value| value.get("DelegatedRouters"))
        ));
        assert_eq!(
            config
                .get("Provide")
                .and_then(|value| value.get("Enabled"))
                .and_then(JsonValue::as_bool),
            Some(false),
            "isolated Kubo must disable content announcements"
        );
        let addresses = config
            .get("Addresses")
            .expect("isolated Kubo config has Addresses");
        for field in ["API", "Gateway"] {
            assert_eq!(
                addresses.get(field).and_then(JsonValue::as_str),
                Some("/ip4/127.0.0.1/tcp/0"),
                "isolated Kubo {field} listener must be loopback-only"
            );
        }
        assert_eq!(
            addresses
                .get("Swarm")
                .and_then(JsonValue::as_array)
                .and_then(|values| values.first())
                .and_then(JsonValue::as_str),
            Some("/ip4/127.0.0.1/tcp/0"),
            "isolated Kubo swarm listener must be loopback-only"
        );
        assert_eq!(
            addresses
                .get("Swarm")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1),
            "isolated Kubo must expose only one loopback swarm listener"
        );
    }
    async fn wait_for_api(&mut self) -> String {
        let api_path = self.repo.join("api");
        let deadline = time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(raw) = fs::read_to_string(&api_path) {
                let raw = raw.trim();
                let components = raw.split('/').collect::<Vec<_>>();
                if components.len() == 5
                    && components[1] == "ip4"
                    && components[2] == "127.0.0.1"
                    && components[3] == "tcp"
                    && components[4].parse::<u16>().is_ok_and(|port| port != 0)
                {
                    return format!("http://127.0.0.1:{}/", components[4]);
                }
                panic!("Kubo published a non-loopback or malformed API address: {raw}");
            }
            if let Some(status) = self
                .child
                .as_mut()
                .expect("Kubo child exists while starting")
                .try_wait()
                .expect("inspect Kubo daemon status")
            {
                panic!(
                    "isolated Kubo daemon exited early with {status}\n{}",
                    self.log_text()
                );
            }
            assert!(
                time::Instant::now() < deadline,
                "timed out waiting for isolated Kubo API\n{}",
                self.log_text()
            );
            time::sleep(Duration::from_millis(25)).await;
        }
    }
    async fn wait_until_ready(&self) {
        let endpoint = self.endpoint();
        let url = endpoint
            .ipfs_url("api/v0/version", &[])
            .expect("construct Kubo version URL");
        let deadline = time::Instant::now() + Duration::from_secs(20);
        loop {
            let request = endpoint
                .request(Method::POST, url.clone())
                .expect("construct Kubo readiness request");
            if let Ok(response) = endpoint
                .execute(request, "Kubo readiness request failed")
                .await
                && response.status().is_success()
            {
                let body = read_bounded_response(response, 64 * 1024)
                    .await
                    .expect("read Kubo version response");
                let value: JsonValue =
                    json::from_slice(&body).expect("Kubo version response must be JSON");
                let version = value
                    .get("Version")
                    .and_then(JsonValue::as_str)
                    .expect("Kubo version response has Version");
                eprintln!("isolated Kubo {version} ready at {}", self.api_url);
                return;
            }
            assert!(
                time::Instant::now() < deadline,
                "timed out waiting for isolated Kubo readiness\n{}",
                self.log_text()
            );
            time::sleep(Duration::from_millis(25)).await;
        }
    }
    fn endpoint(&self) -> PinnedEndpoint {
        let authenticated_wire_body_max_bytes = authenticated_ipfs_wire_body_max_bytes(
            GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
        )
        .expect("derive Kubo authenticated wire-body bound");
        let provider = Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "isolated-kubo-authenticator",
        ));
        PinnedEndpoint {
            url: Url::parse(&self.api_url).expect("parse isolated Kubo API URL"),
            client: Client::builder()
                .no_proxy()
                .redirect(Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(20))
                .build()
                .expect("construct isolated Kubo HTTP client"),
            authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
            authenticator: bind_test_authenticator_to_endpoint(
                provider,
                GovernanceDagAuthenticationScope::Ipfs,
                &self.api_url,
                authenticated_wire_body_max_bytes,
            ),
            authenticated_wire_body_max_bytes,
        }
    }
    fn log_text(&self) -> String {
        fs::read_to_string(&self.daemon_log)
            .unwrap_or_else(|err| format!("cannot read Kubo daemon log: {err}"))
    }
    fn stop_child(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        let _ = Command::new(&self.binary)
            .arg("shutdown")
            .env("IPFS_PATH", &self.repo)
            .env("IPFS_TELEMETRY", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                }
                Ok(None) | Err(_) => {
                    // This fallback can only target the exact child spawned above.
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
            }
        }
    }
    fn shutdown(mut self) {
        self.stop_child();
    }
}
impl Drop for KuboHarness {
    fn drop(&mut self) {
        self.stop_child();
    }
}
struct TestSigner {
    private_key: PrivateKey,
    public_key: [u8; 32],
}
impl TestSigner {
    fn new(seed: u8) -> Self {
        let private_key = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("test Ed25519 seed is valid");
        let keypair =
            KeyPair::from_private_key(private_key.clone()).expect("derive test Ed25519 keypair");
        let (algorithm, bytes) = keypair
            .public_key()
            .try_to_bytes()
            .expect("encode test public key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let mut public_key = [0_u8; 32];
        public_key.copy_from_slice(bytes);
        Self {
            private_key,
            public_key,
        }
    }
    fn sign(&self, payload: &[u8]) -> GovernanceLogSignatureV1 {
        let signature = IrohaSignature::try_new(&self.private_key, payload)
            .expect("sign test governance payload");
        GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: self.public_key.to_vec(),
            signature: signature.payload().to_vec(),
        }
    }
}
struct PublisherTestSigner {
    handle: String,
    peer_id: Vec<u8>,
    signer: TestSigner,
}
impl fmt::Debug for PublisherTestSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PublisherTestSigner")
            .field("handle", &self.handle)
            .field("peer_id", &self.peer_id)
            .finish_non_exhaustive()
    }
}
impl GovernanceDagRuntimeSigner for PublisherTestSigner {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        Ok(GovernanceDagRuntimeProviderQualificationV1::new(
            1, [0x83; 32],
        ))
    }
    fn publisher_peer_id(&self) -> &[u8] {
        &self.peer_id
    }
    fn public_key(&self) -> [u8; 32] {
        self.signer.public_key
    }
    fn sign(
        &self,
        _purpose: GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        self.signer
            .sign(payload)
            .signature
            .try_into()
            .map_err(|_| "test signature length".to_owned())
    }
}
fn empty_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}
fn settlement(sequence: u64, timestamp: u64) -> DealSettlementV1 {
    let mut deal_id = [0x11; 32];
    deal_id[..8].copy_from_slice(&sequence.saturating_add(1).to_le_bytes());
    let settled_at = timestamp.saturating_sub(1);
    let mut ledger = DealLedgerSnapshotV1 {
        version: DEAL_LEDGER_VERSION_V1,
        snapshot_id: [0; 32],
        sequence: 1,
        previous_snapshot_id: None,
        deal_id,
        terms_digest: [0x44; 32],
        provider_id: [0x22; 32],
        client_id: [0x33; 32],
        deal_start_epoch: settled_at.saturating_sub(2),
        deal_end_epoch: settled_at.saturating_sub(1),
        settlement_window_epochs: 2,
        window_start_epoch: settled_at.saturating_sub(2),
        window_end_epoch: settled_at,
        provider_accrual: xor("0.00000001"),
        client_liability: xor("0.00000001"),
        micropayment_credit_generated: XorQuantity::zero(),
        micropayment_credit_applied: XorQuantity::zero(),
        micropayment_credit_carry: XorQuantity::zero(),
        client_debit: xor("0.00000001"),
        outstanding_liability: XorQuantity::zero(),
        bond_total: xor("0.00000002"),
        bond_locked: XorQuantity::zero(),
        bond_slashed: XorQuantity::zero(),
        bond_released: xor("0.00000002"),
        window_expected_charge: xor("0.00000001"),
        window_micropayment_generated: XorQuantity::zero(),
        window_micropayment_applied: XorQuantity::zero(),
        window_client_debit: xor("0.00000001"),
        window_bond_slashed: XorQuantity::zero(),
        window_bond_released: xor("0.00000002"),
        captured_at: settled_at,
    };
    ledger.snapshot_id = ledger.derive_snapshot_id().expect("ledger id");
    let mut settlement = DealSettlementV1 {
        version: DEAL_SETTLEMENT_VERSION_V1,
        settlement_id: [0; 32],
        deal_id,
        ledger,
        status: DealSettlementStatusV1::Completed,
        settled_at,
        audit_notes: None,
    };
    settlement.settlement_id = settlement.derive_settlement_id().expect("settlement id");
    settlement
}
fn signed_source(count: usize, seed: u8, first_timestamp: u64) -> SourceSnapshot {
    let signer = TestSigner::new(seed);
    let peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
    let mut previous_node_cid = None;
    let mut previous_block_cid = None;
    let mut source_blocks = Vec::new();
    let mut decoded_blocks = Vec::new();
    for sequence in 0..count as u64 {
        let timestamp = first_timestamp.saturating_add(sequence);
        let mut node = GovernanceLogNodeV1 {
            version: GOVERNANCE_LOG_VERSION_V1,
            node_cid: Vec::new(),
            prev_cid: previous_node_cid.clone(),
            timestamp,
            publisher_peer_id: peer_id.clone(),
            submission_provenance: None,
            payload: GovernanceLogPayloadV1::DealSettlement(Box::new(settlement(
                sequence, timestamp,
            ))),
            publisher_signature: empty_signature(),
        };
        node.node_cid = node.recompute_node_cid().expect("derive test node CID");
        node.publisher_signature = signer.sign(
            &node
                .signature_payload_bytes()
                .expect("encode test node signing payload"),
        );
        let block_cid = governance_dag_block_cid_v1(
            previous_block_cid.as_deref(),
            sequence,
            timestamp,
            &peer_id,
            &node,
        )
        .expect("derive test block CID");
        let mut block = GovernanceDagBlockV1 {
            version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
            block_cid,
            prev_block_cid: previous_block_cid.clone(),
            sequence,
            timestamp,
            publisher_peer_id: peer_id.clone(),
            node,
            block_signature: empty_signature(),
        };
        block.block_signature = signer.sign(
            &block
                .signature_payload_bytes()
                .expect("encode test block signing payload"),
        );
        block.validate().expect("test block is valid");
        let bytes = norito::to_bytes(&block).expect("encode test block");
        previous_node_cid = Some(block.node.node_cid.clone());
        previous_block_cid = Some(block.block_cid.clone());
        decoded_blocks.push(block.clone());
        source_blocks.push(SourceBlock {
            encoded_blake3: blake3_array(&bytes),
            payload_kind: "deal_settlement".to_owned(),
            block,
            bytes,
        });
    }
    let last = source_blocks.last().expect("test source is non-empty");
    let checkpoint_cid = (count > GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1).then(|| {
        source_blocks[count - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1]
            .block
            .block_cid
            .clone()
    });
    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: last.block.block_cid.clone(),
        block_count: count as u64,
        generated_at: last.block.timestamp,
        publisher_peer_id: peer_id,
        checkpoint_cid,
        head_signature: empty_signature(),
    };
    head.head_signature = signer.sign(
        &head
            .signature_payload_bytes()
            .expect("encode test head signing payload"),
    );
    validate_governance_dag_head_against_chain_v1(&head, &decoded_blocks)
        .expect("test source chain is valid");
    let head_bytes = norito::to_bytes(&head).expect("encode test head");
    let chain_blake3 = source_chain_blake3_v1(&head_bytes, &source_blocks);
    SourceSnapshot {
        index_blake3: [0x44; 32],
        chain_blake3,
        head,
        head_bytes,
        blocks: source_blocks,
    }
}
fn appeal_finance_report(timestamp: u64) -> SoraFsAppealFinanceReportV1 {
    SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id: [0x42; 16],
        case_id: "case-42".to_owned(),
        round_id: Some("round-1".to_owned()),
        generated_at_unix_ms: timestamp.saturating_mul(1_000),
        appeal_finance_config_version: "baseline-v1".to_owned(),
        evidence_bundle_digest: Some([0xA7; 32]),
        outcome: SoraFsAppealFinanceOutcomeV1::Overturn,
        deposit_xor: xor("420"),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "refund-account".to_owned(),
            amount_xor: xor("420"),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "treasury-account".to_owned(),
            amount_xor: xor("50"),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: "escrow-account".to_owned(),
            amount_xor: XorQuantity::zero(),
        },
        panel_size: 3,
        panel_reward_total_xor: xor("85"),
        rewards_paid_total_xor: xor("60"),
        rewards_forfeited_treasury_xor: xor("25"),
        juror_payouts: vec![
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-a".to_owned(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
            SoraFsAppealFinanceJurorPayoutV1 {
                juror_id: "juror-b".to_owned(),
                stipend_xor: xor("25"),
                bonus_xor: xor("5"),
                total_xor: xor("30"),
            },
        ],
        no_show_juror_ids: vec!["juror-c".to_owned()],
    }
}
fn signed_finance_source(seed: u8, timestamp: u64) -> SourceSnapshot {
    let signer = TestSigner::new(seed);
    let account_key = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
        .expect("derive canonical submission account");
    let account = iroha_data_model::account::AccountId::new(account_key.public_key().clone());
    let mut source = signed_source(1, seed, timestamp);
    let source_block = source.blocks.first_mut().expect("single source block");
    source_block.block.node.payload =
        GovernanceLogPayloadV1::AppealFinanceReport(appeal_finance_report(timestamp));
    source_block.block.node.submission_provenance = Some(GovernanceDagSubmissionProvenanceV1 {
        publisher_account_digest: governance_dag_submission_account_digest_v1(&account.encode()),
        origin: GovernanceDagSubmissionOriginV1::AppealFinanceReport,
    });
    source_block.block.node.node_cid = source_block
        .block
        .node
        .recompute_node_cid()
        .expect("derive attributed node CID");
    source_block.block.node.publisher_signature = signer.sign(
        &source_block
            .block
            .node
            .signature_payload_bytes()
            .expect("encode attributed node signing payload"),
    );
    source_block.block.block_cid = source_block
        .block
        .recompute_block_cid()
        .expect("derive attributed block CID");
    source_block.block.block_signature = signer.sign(
        &source_block
            .block
            .signature_payload_bytes()
            .expect("encode attributed block signing payload"),
    );
    source_block
        .block
        .validate()
        .expect("attributed source block validates");
    source_block.bytes =
        norito::to_bytes(&source_block.block).expect("encode attributed source block");
    source_block.encoded_blake3 = blake3_array(&source_block.bytes);
    source_block.payload_kind = "appeal_finance_report".to_owned();
    source.head.head_block_cid = source_block.block.block_cid.clone();
    source.head.head_signature = signer.sign(
        &source
            .head
            .signature_payload_bytes()
            .expect("encode attributed head signing payload"),
    );
    validate_governance_dag_head_against_chain_v1(&source.head, &[source_block.block.clone()])
        .expect("attributed source head validates");
    source.head_bytes = norito::to_bytes(&source.head).expect("encode attributed source head");
    source.chain_blake3 = source_chain_blake3_v1(&source.head_bytes, &source.blocks);
    source
}
fn test_runtime_config(source: &SourceSnapshot, root: &Path) -> RuntimeConfig {
    let mut expected_public_key = [0_u8; 32];
    expected_public_key.copy_from_slice(&source.head.head_signature.public_key);
    let source_dir = root.join("source");
    let state_dir = root.join("state");
    fs::create_dir_all(&source_dir).expect("create test source root");
    fs::create_dir_all(&state_dir).expect("create test state root");
    RuntimeConfig {
        source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
            .expect("fence test source root"),
        source_dir,
        state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
            .expect("fence test state root"),
        listen_addr: "127.0.0.1:0".parse().expect("test address"),
        poll_interval: Duration::from_millis(10),
        max_response_bytes: 1024 * 1024,
        max_request_bytes: 1024 * 1024,
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
        expected_publisher_peer_id: source.head.publisher_peer_id.clone(),
        expected_public_key,
    }
}
fn checkpoint_from_source(source: &SourceSnapshot) -> CheckpointBodyV1 {
    let mirror_blocks = source
        .blocks
        .iter()
        .map(|block| PublishedBlockV1 {
            sequence: block.block.sequence,
            governance_block_cid: block.block.block_cid.clone(),
            governance_node_cid: block.block.node.node_cid.clone(),
            payload_kind: block.payload_kind.clone(),
            timestamp: block.block.timestamp,
            encoded_blake3: block.encoded_blake3,
            encoded_len: block.bytes.len() as u64,
            ipfs_cid: canonical_ipfs_file_cid(&block.bytes)
                .expect("test block fits fixed UnixFS profile"),
        })
        .collect();
    CheckpointBodyV1 {
        version: CHECKPOINT_VERSION_V1,
        generation: 1,
        head_block_cid: source.head.head_block_cid.clone(),
        block_count: source.head.block_count,
        head_bytes: source.head_bytes.clone(),
        head_bytes_blake3: blake3_array(&source.head_bytes),
        head_ipfs_cid: canonical_ipfs_file_cid(&source.head_bytes)
            .expect("test head fits fixed UnixFS profile"),
        source_chain_blake3: source.chain_blake3,
        mirror_blake3: [0x55; 32],
        published_at_unix: source.head.generated_at,
        archive_head: BlockPrefixArchiveHeadV1::empty(),
        mirror_blocks,
    }
}
fn checkpoint_with_canonical_mirror(source: &SourceSnapshot) -> CheckpointBodyV1 {
    let mut checkpoint = checkpoint_from_source(source);
    let mirror = mirror_index_value(
        source,
        &checkpoint.mirror_blocks,
        &checkpoint.archive_head,
        checkpoint.generation,
        &checkpoint.head_ipfs_cid,
        checkpoint.published_at_unix,
    )
    .expect("build canonical checkpoint mirror");
    checkpoint.mirror_blake3 = blake3_array(
        json::to_json_pretty(&mirror)
            .expect("encode canonical checkpoint mirror")
            .as_bytes(),
    );
    checkpoint
}
fn intent_from_source(source: &SourceSnapshot) -> PublishIntentBodyV1 {
    PublishIntentBodyV1 {
        version: PUBLISH_INTENT_VERSION_V1,
        generation: 1,
        target_head_block_cid: source.head.head_block_cid.clone(),
        target_block_count: source.head.block_count,
        target_head_bytes: source.head_bytes.clone(),
        target_head_blake3: blake3_array(&source.head_bytes),
        target_source_chain_blake3: source.chain_blake3,
        previous_public_head_blake3: None,
        created_at_unix: source.head.generated_at,
        archive_head: BlockPrefixArchiveHeadV1::empty(),
        blocks: source
            .blocks
            .iter()
            .map(|block| IntentBlockV1 {
                sequence: block.block.sequence,
                governance_block_cid: block.block.block_cid.clone(),
                governance_node_cid: block.block.node.node_cid.clone(),
                payload_kind: block.payload_kind.clone(),
                timestamp: block.block.timestamp,
                encoded_blake3: block.encoded_blake3,
                encoded_len: block.bytes.len() as u64,
                ipfs_cid: Some(
                    canonical_ipfs_file_cid(&block.bytes)
                        .expect("test block fits fixed UnixFS profile"),
                ),
            })
            .collect(),
        head_ipfs_cid: Some(
            canonical_ipfs_file_cid(&source.head_bytes)
                .expect("test head fits fixed UnixFS profile"),
        ),
    }
}
fn block_prefix_archive_test_endpoint() -> PinnedEndpoint {
    PinnedEndpoint {
        url: Url::parse("http://127.0.0.1:1/").expect("parse test archive endpoint"),
        client: Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .expect("build test archive client"),
        authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
        authenticator: test_authenticator(
            TEST_IPFS_AUTH_HANDLE,
            GovernanceDagAuthenticationScope::Ipfs,
        ),
        authenticated_wire_body_max_bytes: 1024 * 1024,
    }
}
fn signed_block_prefix_archive_fixture(
    source: &SourceSnapshot,
    start: u64,
    end: u64,
    predecessor: BlockPrefixArchiveHeadV1,
    endpoint: &PinnedEndpoint,
) -> (
    SignedBlockPrefixArchiveV1,
    Vec<u8>,
    BlockPrefixArchiveHeadV1,
) {
    signed_block_prefix_archive_fixture_with_checkpoint(
        source,
        start,
        end,
        predecessor,
        CheckpointCommitmentV1::empty(),
        1,
        endpoint,
    )
}
fn signed_block_prefix_archive_fixture_with_checkpoint(
    source: &SourceSnapshot,
    start: u64,
    end: u64,
    predecessor: BlockPrefixArchiveHeadV1,
    checkpoint: CheckpointCommitmentV1,
    target_generation: u64,
    endpoint: &PinnedEndpoint,
) -> (
    SignedBlockPrefixArchiveV1,
    Vec<u8>,
    BlockPrefixArchiveHeadV1,
) {
    let mut intent = intent_from_source(source);
    intent.generation = target_generation;
    for (intent_block, source_block) in intent.blocks.iter_mut().zip(&source.blocks) {
        intent_block.ipfs_cid = Some(canonical_raw_sha256_cid(&source_block.bytes));
    }
    let by_sequence =
        published_blocks_by_sequence(None, &intent).expect("construct published-block fixture map");
    let archive = SignedBlockPrefixArchiveV1 {
        version: BLOCK_PREFIX_ARCHIVE_VERSION_V1,
        archive_generation: predecessor.generation + 1,
        predecessor,
        predecessor_checkpoint_revision: checkpoint.revision,
        predecessor_checkpoint_digest: checkpoint.digest,
        predecessor_block_count: checkpoint.block_count,
        predecessor_head_block_cid: checkpoint.head_block_cid,
        target_checkpoint_generation: intent.generation,
        target_head_block_cid: intent.target_head_block_cid,
        target_block_count: intent.target_block_count,
        target_source_chain_blake3: intent.target_source_chain_blake3,
        ipfs_authenticator_handle: endpoint.authenticator.handle.clone(),
        ipfs_authenticator_revision: endpoint
            .authenticator
            .ingress_qualification
            .provider()
            .revision,
        ipfs_authenticator_policy_digest: endpoint
            .authenticator
            .ingress_qualification
            .provider()
            .policy_digest,
        ipfs_authenticator_public_key: endpoint.authenticator.verification_policy.public_key(),
        checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        checkpoint_store_revision: TEST_STORE_QUALIFICATION.revision,
        checkpoint_store_policy_digest: TEST_STORE_QUALIFICATION.policy_digest,
        archived_block_count: end,
        blocks: block_prefix_archive_entries(source, &by_sequence, start, end)
            .expect("construct exact archive entries"),
    };
    validate_signed_block_prefix_archive(&archive).expect("validate archive fixture");
    validate_block_prefix_archive_against_source(&archive, source)
        .expect("bind archive fixture to source");
    let bytes = norito::to_bytes(&archive).expect("encode archive fixture");
    let descriptor = block_prefix_archive_add_descriptor(endpoint, &archive, &bytes)
        .expect("build archive add descriptor");
    let envelope = endpoint
        .authenticator
        .authenticate(&descriptor)
        .expect("authenticate archive add descriptor");
    let head = BlockPrefixArchiveHeadV1 {
        generation: archive.archive_generation,
        digest: blake3_array(&bytes),
        ipfs_cid: canonical_raw_sha256_cid(&bytes),
        archived_block_count: archive.archived_block_count,
        last_block_cid: archive
            .blocks
            .last()
            .expect("archive fixture has a last block")
            .published
            .governance_block_cid
            .clone(),
        last_node_cid: archive
            .blocks
            .last()
            .expect("archive fixture has a last block")
            .published
            .governance_node_cid
            .clone(),
        predecessor_checkpoint_revision: archive.predecessor_checkpoint_revision,
        predecessor_checkpoint_digest: archive.predecessor_checkpoint_digest,
        predecessor_block_count: archive.predecessor_block_count,
        predecessor_head_block_cid: archive.predecessor_head_block_cid.clone(),
        publication: Some(
            BlockPrefixArchivePublicationV1::from_envelope(&envelope, &descriptor)
                .expect("convert archive publication"),
        ),
    };
    verify_block_prefix_archive_publication(&archive, &bytes, &head)
        .expect("verify archive publication fixture");
    (archive, bytes, head)
}
fn secure_temp_dir() -> TempDir {
    let temp_root = std::env::temp_dir()
        .canonicalize()
        .expect("resolve the physical temporary-directory root");
    let dir = tempfile::Builder::new()
        .prefix("sorafs-governance-service-")
        .tempdir_in(temp_root)
        .expect("create test directory");
    #[cfg(unix)]
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
        .expect("secure test directory");
    dir
}
fn write_test_sidecar_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create source sidecar parent");
    }
    fs::write(path, bytes).expect("write source sidecar payload");
    fs::write(
        digest_sidecar_path(path),
        format!("{}\n", hex::encode(blake3_array(bytes))),
    )
    .expect("write source sidecar digest");
}
fn materialize_source_snapshot(root: &Path, source: &mut SourceSnapshot) {
    fs::create_dir_all(root).expect("create Governance DAG source root");
    let mut entries = Vec::with_capacity(source.blocks.len());
    let mut by_digest = JsonMap::new();
    let mut by_source_payload_digest = BTreeMap::<String, Vec<JsonValue>>::new();
    let mut by_kind = BTreeMap::<String, Vec<JsonValue>>::new();
    for (position, block) in source.blocks.iter().enumerate() {
        let block_cid_hex = hex::encode(&block.block.block_cid);
        let block_path_label = format!(
            "runtime-dag/blocks/{:020}_{block_cid_hex}.to",
            block.block.sequence
        );
        write_test_sidecar_file(&root.join(&block_path_label), &block.bytes);
        let source_payload_bytes = canonical_source_payload_bytes(&block.block.node.payload)
            .expect("encode test source payload");
        let source_payload_len =
            u64::try_from(source_payload_bytes.len()).expect("test source payload length fits u64");
        let source_payload_digest_hex = hex::encode(blake3_array(&source_payload_bytes));
        let mut source_json = JsonMap::new();
        source_json.insert(
            "payload_kind".into(),
            JsonValue::from(block.payload_kind.clone()),
        );
        source_json.insert("sequence".into(), JsonValue::from(block.block.sequence));
        source_json.insert(
            "source_payload_blake3".into(),
            JsonValue::from(source_payload_digest_hex.clone()),
        );
        source_json.insert(
            "source_payload_len".into(),
            JsonValue::from(source_payload_len),
        );
        let source_json_bytes = json::to_json_pretty(&JsonValue::Object(source_json))
            .expect("encode test JSON source")
            .into_bytes();
        validate_governance_car_source_lengths(source_payload_bytes.len(), source_json_bytes.len())
            .expect("test Governance DAG source pair satisfies size limits");
        let source_json_len =
            u64::try_from(source_json_bytes.len()).expect("test JSON source length fits u64");
        let source_json_digest_hex = hex::encode(blake3_array(&source_json_bytes));
        let (source_payload_path_label, source_json_path_label) =
            governance_source_pair_relative_paths(
                &block.payload_kind,
                source_payload_len,
                &source_payload_digest_hex,
                source_json_len,
                &source_json_digest_hex,
            )
            .expect("derive test Governance DAG source-pair paths");
        write_test_sidecar_file(
            &root.join(&source_payload_path_label),
            &source_payload_bytes,
        );
        write_test_sidecar_file(&root.join(&source_json_path_label), &source_json_bytes);
        let digest_hex = hex::encode(block.encoded_blake3);
        let mut entry = JsonMap::new();
        entry.insert("position".into(), JsonValue::from(position as u64));
        entry.insert("sequence".into(), JsonValue::from(block.block.sequence));
        entry.insert("block_path".into(), JsonValue::from(block_path_label));
        entry.insert(
            "encoded_path".into(),
            JsonValue::from(source_payload_path_label),
        );
        entry.insert("json_path".into(), JsonValue::from(source_json_path_label));
        entry.insert(
            "encoded_len".into(),
            JsonValue::from(block.bytes.len() as u64),
        );
        entry.insert(
            "source_payload_len".into(),
            JsonValue::from(source_payload_len),
        );
        entry.insert(
            "source_payload_blake3".into(),
            JsonValue::from(source_payload_digest_hex.clone()),
        );
        entry.insert("block_cid_hex".into(), JsonValue::from(block_cid_hex));
        entry.insert(
            "node_cid_hex".into(),
            JsonValue::from(hex::encode(&block.block.node.node_cid)),
        );
        entry.insert(
            "prev_block_cid_hex".into(),
            block
                .block
                .prev_block_cid
                .as_ref()
                .map(hex::encode)
                .map(JsonValue::from)
                .unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "prev_node_cid_hex".into(),
            block
                .block
                .node
                .prev_cid
                .as_ref()
                .map(hex::encode)
                .map(JsonValue::from)
                .unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "payload_kind".into(),
            JsonValue::from(block.payload_kind.clone()),
        );
        entry.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        entry.insert(
            "submission_publisher_account_digest_hex".into(),
            block
                .block
                .node
                .submission_provenance
                .as_ref()
                .map(|provenance| JsonValue::from(hex::encode(provenance.publisher_account_digest)))
                .unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "submission_origin".into(),
            block
                .block
                .node
                .submission_provenance
                .as_ref()
                .map(|provenance| JsonValue::from(provenance.origin.label()))
                .unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "published_at_unix".into(),
            JsonValue::from(block.block.timestamp),
        );
        entries.push(JsonValue::Object(entry));
        by_digest.insert(
            digest_hex,
            JsonValue::Array(vec![JsonValue::from(position as u64)]),
        );
        by_source_payload_digest
            .entry(source_payload_digest_hex)
            .or_default()
            .push(JsonValue::from(position as u64));
        by_kind
            .entry(block.payload_kind.clone())
            .or_default()
            .push(JsonValue::from(position as u64));
    }
    let mut index = JsonMap::new();
    index.insert("schema".into(), JsonValue::from(RUNTIME_INDEX_SCHEMA));
    index.insert("source".into(), JsonValue::from(RUNTIME_INDEX_SOURCE));
    index.insert("root".into(), JsonValue::from(RUNTIME_INDEX_LOGICAL_ROOT));
    index.insert(
        "generated_at".into(),
        JsonValue::from(source.head.generated_at),
    );
    index.insert(
        "signer_handle".into(),
        JsonValue::from(TEST_PRODUCER_SIGNER_HANDLE),
    );
    index.insert(
        "signer_revision".into(),
        JsonValue::from(TEST_PRODUCER_SIGNER_QUALIFICATION.revision),
    );
    index.insert(
        "signer_policy_digest_hex".into(),
        JsonValue::from(hex::encode(
            TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest,
        )),
    );
    index.insert(
        "checkpoint_store_handle".into(),
        JsonValue::from(TEST_CHECKPOINT_STORE_HANDLE),
    );
    index.insert(
        "checkpoint_store_revision".into(),
        JsonValue::from(TEST_STORE_QUALIFICATION.revision),
    );
    index.insert(
        "checkpoint_store_policy_digest_hex".into(),
        JsonValue::from(hex::encode(TEST_STORE_QUALIFICATION.policy_digest)),
    );
    index.insert(
        "publisher_public_key_hex".into(),
        JsonValue::from(hex::encode(&source.head.head_signature.public_key)),
    );
    index.insert(
        "publisher_peer_id_hex".into(),
        JsonValue::from(hex::encode(&source.head.publisher_peer_id)),
    );
    index.insert(
        "publisher_peer_id".into(),
        JsonValue::from(
            std::str::from_utf8(&source.head.publisher_peer_id)
                .expect("test publisher peer id is UTF-8"),
        ),
    );
    index.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&source.head.head_block_cid)),
    );
    index.insert(
        "head_generated_at".into(),
        JsonValue::from(source.head.generated_at),
    );
    index.insert(
        "block_count".into(),
        JsonValue::from(source.head.block_count),
    );
    index.insert("by_encoded_blake3".into(), JsonValue::Object(by_digest));
    index.insert(
        "by_source_payload_blake3".into(),
        JsonValue::Object(
            by_source_payload_digest
                .into_iter()
                .map(|(digest, positions)| (digest, JsonValue::Array(positions)))
                .collect(),
        ),
    );
    index.insert(
        "by_payload_kind".into(),
        JsonValue::Object(
            by_kind
                .into_iter()
                .map(|(kind, positions)| (kind, JsonValue::Array(positions)))
                .collect(),
        ),
    );
    index.insert("blocks".into(), JsonValue::Array(entries));
    let index_bytes = json::to_json_pretty(&JsonValue::Object(index))
        .expect("encode Governance DAG runtime index")
        .into_bytes();
    source.index_blake3 = blake3_array(&index_bytes);
    source.chain_blake3 = source_chain_blake3_v1(&source.head_bytes, &source.blocks);
    write_runtime_dag_committed_snapshot_fixture_v1(root, source.head_bytes.clone(), index_bytes)
        .expect("commit typed Governance DAG runtime head/index fixture");
}
fn producer_checkpoint_from_source(
    root: &Path,
    source: &SourceSnapshot,
) -> RuntimeDagProducerCheckpointV1 {
    RuntimeDagProducerCheckpointV1 {
        version: GOVERNANCE_RUNTIME_DAG_PRODUCER_CHECKPOINT_VERSION_V1,
        root_digest: runtime_dag_producer_root_digest(root)
            .expect("derive canonical test producer root digest"),
        signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        signer_revision: TEST_PRODUCER_SIGNER_QUALIFICATION.revision,
        signer_policy_digest: TEST_PRODUCER_SIGNER_QUALIFICATION.policy_digest,
        checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        checkpoint_store_revision: TEST_STORE_QUALIFICATION.revision,
        checkpoint_store_policy_digest: TEST_STORE_QUALIFICATION.policy_digest,
        publisher_peer_id: source.head.publisher_peer_id.clone(),
        publisher_public_key: source
            .head
            .head_signature
            .public_key
            .as_slice()
            .try_into()
            .expect("test source public key is 32 bytes"),
        block_count: source.head.block_count,
        head_block_cid: source
            .head
            .head_block_cid
            .as_slice()
            .try_into()
            .expect("test source head CID is 32 bytes"),
        head_bytes_digest: blake3_array(&source.head_bytes),
        index_bytes_digest: source.index_blake3,
        qualification_transition_generation: 0,
        qualification_transition_digest: [0; 32],
        qualification_archive_generation: 0,
        qualification_archive_digest: [0; 32],
    }
}
fn seed_producer_checkpoint(
    provider: &TestSealedStore,
    root: &Path,
    source: &SourceSnapshot,
) -> GovernanceDagSealedStateRecord {
    let checkpoint = producer_checkpoint_from_source(root, source);
    let generation = checkpoint
        .block_count
        .checked_add(checkpoint.qualification_transition_generation)
        .and_then(|generation| generation.checked_add(checkpoint.qualification_archive_generation))
        .and_then(|generation| generation.checked_add(1))
        .expect("test producer generation");
    let record = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerCheckpoint,
        generation,
        norito::to_bytes(&checkpoint).expect("encode test producer checkpoint"),
    );
    provider
        .compare_and_swap(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
            None,
            record.clone(),
        )
        .expect("seed test producer checkpoint");
    record
}
async fn kubo_unpin(endpoint: &PinnedEndpoint, cid: &str) {
    let url = endpoint
        .ipfs_url("api/v0/pin/rm", &[("arg", cid), ("recursive", "true")])
        .expect("construct Kubo unpin URL");
    let request = endpoint
        .request(Method::POST, url)
        .expect("construct Kubo unpin request");
    let response = endpoint
        .execute(request, "Kubo unpin request failed")
        .await
        .expect("send Kubo unpin request");
    assert!(response.status().is_success(), "Kubo unpin failed");
    let _ = read_bounded_response(response, 64 * 1024)
        .await
        .expect("read Kubo unpin response");
}
async fn assert_kubo_has_no_swarm_peers(endpoint: &PinnedEndpoint) {
    let url = endpoint
        .ipfs_url("api/v0/swarm/peers", &[])
        .expect("construct Kubo swarm peers URL");
    let request = endpoint
        .request(Method::POST, url)
        .expect("construct Kubo swarm peers request");
    let response = endpoint
        .execute(request, "Kubo swarm-peers request failed")
        .await
        .expect("send Kubo swarm peers request");
    assert!(response.status().is_success());
    let body = read_bounded_response(response, 64 * 1024)
        .await
        .expect("read Kubo swarm peers response");
    let value: JsonValue = json::from_slice(&body).expect("Kubo swarm response must be JSON");
    assert!(
        value
            .get("Peers")
            .is_none_or(|peers| peers.is_null() || peers.as_array().is_some_and(Vec::is_empty)),
        "isolated Kubo must have no swarm peers: {value:?}"
    );
}
fn real_kubo_service_view(
    source: &SourceSnapshot,
    source_dir: &Path,
    state_dir: &Path,
    api_url: &str,
    signed_head_url: &str,
) -> SorafsGovernanceDagServiceView {
    let paths = [source_dir, state_dir];
    assert!(paths.iter().all(|path| {
        let path = path.to_string_lossy();
        !path.contains(['"', '\\', '\n', '\r'])
    }));
    let config = format!(
        r#"[sorafs.storage]
governance_dag_dir = "{}"
governance_dag_publisher_peer_id = "{TEST_PRODUCER_PEER_ID}"
governance_dag_signer_handle = "{TEST_PRODUCER_SIGNER_HANDLE}"
governance_dag_signer_revision = 1
governance_dag_signer_policy_digest_hex = "{}"
governance_dag_publisher_public_key_hex = "{}"

[sorafs.storage.governance_dag_service]
enabled = true
state_dir = "{}"
ipfs_api_url = "{}"
signed_head_url = "{}"
ipfs_authenticator_handle = "{TEST_IPFS_AUTH_HANDLE}"
ipfs_authenticator_revision = 1
ipfs_authenticator_policy_digest_hex = "{}"
ipfs_request_auth_public_key_hex = "{}"
head_authenticator_handle = "{TEST_HEAD_AUTH_HANDLE}"
head_authenticator_revision = 1
head_authenticator_policy_digest_hex = "{}"
head_request_auth_public_key_hex = "{}"
checkpoint_store_handle = "{TEST_CHECKPOINT_STORE_HANDLE}"
checkpoint_store_revision = 1
checkpoint_store_policy_digest_hex = "{}"
publisher_public_key_hex = "{}"
poll_interval_secs = 1
connect_timeout_ms = 5000
request_timeout_ms = 20000
dns_timeout_ms = 5000
max_future_skew_secs = 60
max_request_bytes = {}
allow_insecure_http = true
allow_private_ipfs_endpoint = true
allow_private_head_endpoint = true
allow_head_bootstrap = true
listen_addr = "127.0.0.1:0"
"#,
        source_dir.display(),
        "83".repeat(32),
        hex::encode(&source.head.head_signature.public_key),
        state_dir.display(),
        api_url,
        signed_head_url,
        "81".repeat(32),
        hex::encode(test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE)),
        "81".repeat(32),
        hex::encode(test_request_auth_public_key(TEST_HEAD_AUTH_HANDLE)),
        "82".repeat(32),
        hex::encode(&source.head.head_signature.public_key),
        BLOCK_PREFIX_ARCHIVE_MAX_REQUEST_BYTES_V1,
    );
    let config_path = state_dir
        .parent()
        .expect("integration state directory has parent")
        .join("governance-dag-service.toml");
    fs::write(&config_path, config).expect("write standalone G-DAG service config");
    load_service_config(&config_path).expect("parse standalone G-DAG service config")
}
async fn spawn_router_with_authenticator<T>(
    router: Router,
    path: &str,
    authentication_scope: GovernanceDagAuthenticationScope,
    provider: Arc<T>,
) -> (PinnedEndpoint, JoinHandle<()>)
where
    T: TestRebindableRequestAuthenticator + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock service");
    let address = listener.local_addr().expect("mock listener address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router.into_make_service()).await;
    });
    let url = Url::parse(&format!("http://{address}{path}")).expect("mock URL");
    let client = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .build()
        .expect("mock HTTP client");
    let config = SorafsGovernanceDagService::default();
    let authenticated_wire_body_max_bytes = match authentication_scope {
        GovernanceDagAuthenticationScope::Ipfs => {
            authenticated_ipfs_wire_body_max_bytes(config.max_request_bytes.0)
                .expect("derive mock authenticated wire-body bound")
        }
        GovernanceDagAuthenticationScope::SignedHead => config.max_request_bytes.0,
    };
    let authenticator = bind_test_authenticator_to_endpoint(
        provider,
        authentication_scope,
        url.as_str(),
        authenticated_wire_body_max_bytes,
    );
    (
        PinnedEndpoint {
            url,
            client,
            authentication_scope,
            authenticator,
            authenticated_wire_body_max_bytes,
        },
        handle,
    )
}
async fn spawn_router(router: Router, path: &str) -> (PinnedEndpoint, JoinHandle<()>) {
    spawn_router_with_authenticator(
        router,
        path,
        GovernanceDagAuthenticationScope::Ipfs,
        Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "mock-router-authenticator",
        )),
    )
    .await
}
fn test_response(status: StatusCode, body: impl Into<Body>) -> Response {
    let mut response = Response::new(body.into());
    *response.status_mut() = status;
    response
}
#[derive(Clone)]
struct MockIpfsState {
    add_body: Arc<Vec<u8>>,
    cat_body: Arc<Vec<u8>>,
    pin_present: bool,
}
async fn mock_ipfs_add(State(state): State<MockIpfsState>, _body: Bytes) -> Response {
    test_response(StatusCode::OK, state.add_body.as_ref().clone())
}
async fn mock_ipfs_pin_add() -> Response {
    test_response(StatusCode::OK, "{}")
}
async fn mock_ipfs_pin_ls(State(state): State<MockIpfsState>) -> Response {
    let body = if state.pin_present {
        let cid = json::from_slice::<JsonValue>(&state.add_body)
            .ok()
            .and_then(|value| {
                value
                    .get("Hash")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| TEST_CID_PAYLOAD.to_owned());
        format!(r#"{{"Keys":{{"{cid}":{{}}}}}}"#)
    } else {
        r#"{"Keys":{}}"#.to_owned()
    };
    test_response(StatusCode::OK, body)
}
async fn mock_ipfs_cat(State(state): State<MockIpfsState>) -> Response {
    test_response(StatusCode::OK, state.cat_body.as_ref().clone())
}
fn mock_ipfs_router(state: MockIpfsState) -> Router {
    Router::new()
        .route("/api/v0/add", post(mock_ipfs_add))
        .route("/api/v0/pin/add", post(mock_ipfs_pin_add))
        .route("/api/v0/pin/ls", post(mock_ipfs_pin_ls))
        .route("/api/v0/cat", post(mock_ipfs_cat))
        .layer(axum::extract::DefaultBodyLimit::disable())
        .with_state(state)
}
async fn count_unexpected_publication_io(State(request_count): State<Arc<AtomicU64>>) -> Response {
    request_count.fetch_add(1, AtomicOrdering::SeqCst);
    test_response(StatusCode::INTERNAL_SERVER_ERROR, "unexpected request")
}
#[derive(Default)]
struct SignedHeadInner {
    bytes: Option<Vec<u8>>,
    etag: String,
    duplicate_etag: bool,
    put_status: Option<StatusCode>,
    readback_override: Option<Vec<u8>>,
    put_count: u64,
}
#[derive(Clone)]
struct SignedHeadState(Arc<Mutex<SignedHeadInner>>);
async fn mock_signed_head_get(State(state): State<SignedHeadState>) -> Response {
    let state = state.0.lock().await;
    let Some(bytes) = &state.bytes else {
        return test_response(StatusCode::NOT_FOUND, Body::empty());
    };
    let mut response = test_response(StatusCode::OK, bytes.clone());
    response.headers_mut().append(
        header::ETAG,
        HeaderValue::from_str(&state.etag).expect("mock ETag"),
    );
    if state.duplicate_etag {
        response
            .headers_mut()
            .append(header::ETAG, HeaderValue::from_static("\"duplicate\""));
    }
    response
}
async fn mock_signed_head_put(
    State(state): State<SignedHeadState>,
    _headers: HeaderMap,
    body: Bytes,
) -> Response {
    let mut state = state.0.lock().await;
    state.put_count = state.put_count.saturating_add(1);
    if let Some(status) = state.put_status {
        return test_response(status, Body::empty());
    }
    state.bytes = Some(
        state
            .readback_override
            .clone()
            .unwrap_or_else(|| body.to_vec()),
    );
    state.etag = "\"v2\"".to_owned();
    test_response(StatusCode::NO_CONTENT, Body::empty())
}
async fn spawn_signed_head(
    inner: SignedHeadInner,
) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>) {
    spawn_signed_head_with_authenticator(
        inner,
        Arc::new(TestAuthenticator::new(
            TEST_HEAD_AUTH_HANDLE,
            "mock-signed-head-authenticator",
        )),
    )
    .await
}
async fn spawn_signed_head_with_authenticator<T>(
    inner: SignedHeadInner,
    provider: Arc<T>,
) -> (PinnedEndpoint, SignedHeadState, JoinHandle<()>)
where
    T: TestRebindableRequestAuthenticator + 'static,
{
    let state = SignedHeadState(Arc::new(Mutex::new(inner)));
    let router = Router::new()
        .route("/head", get(mock_signed_head_get).put(mock_signed_head_put))
        .with_state(state.clone());
    let (endpoint, handle) = spawn_router_with_authenticator(
        router,
        "/head",
        GovernanceDagAuthenticationScope::SignedHead,
        provider,
    )
    .await;
    (endpoint, state, handle)
}
async fn response_header_bomb() -> Response {
    let mut response = test_response(StatusCode::OK, "ok");
    for index in 0..=MAX_RESPONSE_HEADERS {
        let name =
            HeaderName::from_bytes(format!("x-test-{index}").as_bytes()).expect("mock header name");
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static("value"));
    }
    response
}
async fn response_body_bomb() -> Response {
    test_response(StatusCode::OK, vec![0_u8; 17])
}
async fn response_gzip() -> Response {
    let mut response = test_response(StatusCode::OK, "abc");
    response
        .headers_mut()
        .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    response
}
async fn mock_authenticator_drift(State(provider): State<Arc<TestAuthenticator>>) -> Response {
    provider
        .qualification_revision
        .fetch_add(1, AtomicOrdering::SeqCst);
    test_response(StatusCode::OK, "qualified-before-response")
}
