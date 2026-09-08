"""Static contracts for the production SoraFS provider-ingest runtime."""

from pathlib import Path
import re

import pytest

from scripts.tests.executor_visitor_delegation_source_test import (
    mask_rust,
    matching_delimiter,
    preceding_attributes,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
NODE_RUNTIME = (
    REPO_ROOT / "crates" / "sorafs_node" / "src" / "provider_ingest_runtime.rs"
)
NODE_OUTBOX = (
    REPO_ROOT / "crates" / "sorafs_node" / "src" / "provider_ingest_outbox.rs"
)
NODE_LIB = REPO_ROOT / "crates" / "sorafs_node" / "src" / "lib.rs"
DAEMON_RUNTIME = (
    REPO_ROOT / "crates" / "irohad" / "src" / "sorafs_provider_ingest_runtime.rs"
)
DAEMON_RUNTIME_TESTS = (
    REPO_ROOT
    / "crates"
    / "irohad"
    / "src"
    / "sorafs_provider_ingest_runtime"
    / "tests.rs"
)
QUARANTINE_RESTART_TEST = (
    REPO_ROOT
    / "crates"
    / "irohad"
    / "src"
    / "sorafs_provider_ingest_runtime"
    / "tests"
    / "quarantine_restart.rs"
)
CONFIG_USER = (
    REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "user.rs"
)
CONFIG_ACTUAL = (
    REPO_ROOT / "crates" / "iroha_config" / "src" / "parameters" / "actual.rs"
)
COMMIT_EXECUTION = (
    REPO_ROOT
    / "crates"
    / "iroha_core"
    / "src"
    / "smartcontracts"
    / "isi"
    / "sorafs.rs"
)
PROVIDER_ARCHIVE = (
    REPO_ROOT
    / "crates"
    / "iroha_core"
    / "src"
    / "query"
    / "provider_ingest_finalized.rs"
)
STORAGE_DOC = REPO_ROOT / "specs" / "sorafs" / "sorafs_node_storage.md"
CLOSURE_LEDGER = REPO_ROOT / "specs" / "sorafs" / "v1_closure_ledger.md"


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _code(source: str) -> str:
    """Compare executable Rust structure independently of comments and formatting."""

    return re.sub(r"\s+", "", mask_rust(source))


def _body(source: str, declaration: str) -> str:
    """Extract one required item body without matching comments or literals."""

    masked = mask_rust(source)
    matches = list(re.finditer(declaration + r"[^{};]*\{", masked))
    assert len(matches) == 1, f"expected one canonical item: {declaration}"
    opening = matches[0].end() - 1
    closing = matching_delimiter(masked, opening)
    return source[opening + 1 : closing]


def _assert_quarantine_restart_contract(source: str, parent: str) -> None:
    """Keep the actual restart phases and every safety assertion connected.

    The former file hash pinned import order, explicit lifetimes, Copy-value
    clones and a lint attribute. These checks instead describe the reviewed
    sealed-outbox, no-refetch, dead-letter, second-reopen and shared-byte proof.
    The native test must still execute to establish runtime behavior.
    """

    name = "post_admission_quarantine_survives_restart_with_shared_chunks"
    assert _code(parent).count("modquarantine_restart;") == 1
    module = re.search(r"\bmod\s+quarantine_restart\s*;", mask_rust(parent))
    assert module is not None
    assert not mask_rust(parent)[: module.start()].rstrip().endswith("]")
    assert _code(source).count("#[tokio::test]") == 1
    assert not re.search(r"#\s*\[\s*(?:ignore|should_panic|cfg|cfg_attr)\b", mask_rust(source))
    declaration = re.search(rf"\basync\s+fn\s+{name}\s*\(\s*\)", mask_rust(source))
    assert declaration is not None
    attributes = "\n".join(preceding_attributes(source, declaration.start()))
    assert "#[tokio::test]" in _code(attributes)
    body = _body(source, rf"\basync\s+fn\s+{name}\s*\(\s*\)")
    masked = mask_rust(body)
    assert not re.search(r"\b(?:return|for|while|loop|fn|macro_rules)\b", masked)
    # The only conditional is the exact final matches! guard inventoried below.
    assert len(re.findall(r"\bif\b", masked)) == 1
    expected_assertions = (
        'assert_ne!(manifest.digest().expect("primary manifest digest"), '
        'shared_manifest.digest().expect("shared manifest digest"));',
        '''assert!(matches!(outbox.status(authorization.job_id())
            .expect("pre-crash status").state,
            ProviderIngestDeliveryStateV1::SourceClaimed { attempts: 0, .. }));''',
        "assert_ne!(manifest_id, shared_manifest_id);",
        'assert_eq!(node.stored_manifests().expect("stored manifests").len(), 2);',
        "assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);",
        "assert_eq!(outcome.source_jobs_claimed, 1);",
        "assert_eq!(outcome.manifests_stored, 0);",
        '''assert_eq!(terminal.state, ProviderIngestDeliveryStateV1::DeadLetter {
            attempts: 2,
            reason: ProviderIngestDeadLetterReasonV1::StorageRejected,
            last_failure_class: ProviderIngestFailureClassV1::StorageRejected,
            observed_finalized_cursor: cursor,
        });''',
        '''assert_eq!(reopened.finalized_provider_ingest_status_page(None, 1)
            .expect("reopened status page").rows[0].state, terminal.state);''',
        '''assert_eq!(reopened.read_payload_range(&manifest_id, 0, payload.len())
            .expect("read quarantined manifest"), payload);''',
        '''assert_eq!(reopened.read_payload_range(&shared_manifest_id, 0, payload.len())
            .expect("read shared manifest"), payload);''',
        '''assert!(matches!(reopened.ingest_manifest(&manifest, &plan, &mut replay_reader),
            Err(NodeStorageError::Storage(StorageError::ManifestExists {
                manifest_id: existing,
            })) if existing == manifest_id));''',
    )
    assertions = []
    for match in re.finditer(r"\bassert(?:_eq|_ne)?!\s*\(", masked):
        opening = match.end() - 1
        closing = matching_delimiter(masked, opening, "(", ")")
        assertions.append(body[match.start() : closing + 1] + ";")
    # Rust permits a trailing macro-argument comma; it has no assertion semantics.
    canonical_assertion = lambda value: re.sub(r",(?=[)}])", "", _code(value))
    assert [canonical_assertion(value) for value in assertions] == [
        canonical_assertion(value) for value in expected_assertions
    ]
    phases = (
        "ProviderIngestOutbox::open_with_checkpoint_authority(",
        ".enqueue(authorization.clone())",
        ".claim_source(authorization.job_id(),",
        "let (manifest_id, shared_manifest_id) = {",
        "let node = NodeHandle::try_new(plain_config)",
        ".ingest_manifest(&shared_manifest, &plan, &mut shared_reader)",
        ".ingest_manifest(&manifest, &plan, &mut primary_reader)",
        ".provider_ingest_outbox_policy(Some(outbox_policy))",
        ".provider_ingest_checkpoint_provider(Some(CrashRestartCheckpointRuntimeV1::binding()))",
        "NodeRuntimeDeps::default().with_provider_ingest_checkpoint_runtime(checkpoint.clone())",
        "let node = NodeHandle::try_new_with_runtime_deps(configured.clone(), runtime_deps())",
        ".build_provider_ingest_runtime(",
        "Arc::clone(&fetch), storage, Arc::new(NeverBuildCompletionV1), "
        "Arc::new(NeverResolveSignerV1), Arc::new(NeverIngressV1),",
        ".checked_add(outbox_policy.source_lease_ttl_ms).and_then(|now| now.checked_add(1))",
        "let outcome = runtime.tick().await",
        "let terminal = node.finalized_provider_ingest_status_page(None, 1)",
        "drop(runtime); drop(node);",
        "let reopened = NodeHandle::try_new_with_runtime_deps(configured, runtime_deps())",
    )
    code = _code(body)
    offset = 0
    for phase in phases:
        expected = _code(phase)
        start = code.find(expected, offset)
        assert start >= 0, f"missing or reordered restart phase: {phase}"
        offset = start + len(expected)


def test_quarantine_restart_proof_is_connected_and_preserves_recovery_invariants() -> None:
    _assert_quarantine_restart_contract(
        _read(QUARANTINE_RESTART_TEST), _read(DAEMON_RUNTIME_TESTS)
    )


@pytest.mark.parametrize(
    "original,replacement",
    (
        ("#[tokio::test]", "#[tokio::test]\n#[ignore]"),
        ("#[tokio::test]", "#[tokio::test]\n#[cfg(any())]"),
        ("    let temp =", "    return;\n    let temp ="),
        ("assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);", ""),
        ("assert_eq!(outcome.source_jobs_claimed, 1);", "assert_eq!(outcome.source_jobs_claimed, 0);"),
        ("assert_eq!(outcome.manifests_stored, 0);", "assert_eq!(outcome.manifests_stored, 1);"),
        ("reason: ProviderIngestDeadLetterReasonV1::StorageRejected,", "reason: changed_reason,"),
        ("drop(runtime);\n    drop(node);", ""),
        (".read_payload_range(&shared_manifest_id, 0, payload.len())", ".read_payload_range(&manifest_id, 0, payload.len())"),
        ("if existing == manifest_id", "if existing != manifest_id"),
        ("let outcome = runtime.tick().await", "let outcome = unrelated_runtime.tick().await"),
        (".checked_add(outbox_policy.source_lease_ttl_ms)", ".checked_add(0)"),
        (".provider_ingest_checkpoint_provider(Some(CrashRestartCheckpointRuntimeV1::binding()))", ""),
    ),
)
def test_quarantine_contract_rejects_weakened_or_disconnected_proof(
    original: str, replacement: str
) -> None:
    source = _read(QUARANTINE_RESTART_TEST)
    assert original in source
    with pytest.raises(AssertionError):
        _assert_quarantine_restart_contract(
            source.replace(original, replacement, 1), _read(DAEMON_RUNTIME_TESTS)
        )


def test_quarantine_contract_ignores_layout_but_rejects_comment_and_module_substitutes() -> None:
    source = _read(QUARANTINE_RESTART_TEST)
    parent = _read(DAEMON_RUNTIME_TESTS)
    _assert_quarantine_restart_contract(source.replace("    ", "  "), parent)
    assertion = "assert_eq!(fetch.calls.load(Ordering::SeqCst), 0);"
    for replacement in (f"/* {assertion} */", f'let decoy = r#"{assertion}"#;'):
        with pytest.raises(AssertionError):
            _assert_quarantine_restart_contract(source.replace(assertion, replacement), parent)
    for replacement in ("// mod quarantine_restart;", "#[cfg(any())]\nmod quarantine_restart;"):
        with pytest.raises(AssertionError):
            _assert_quarantine_restart_contract(
                source, parent.replace("mod quarantine_restart;", replacement)
            )


def test_authenticated_source_pool_is_bounded_canonical_and_rechecked() -> None:
    source = _read(NODE_RUNTIME)

    for contract in (
        "pub trait ProviderIngestAuthenticatedProviderSourceV1",
        "pub struct ProviderIngestAuthenticatedSourcePoolV1",
        "sources.len() < 2",
        "sources.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS",
        "DuplicateProvider",
        "DuplicateSourceHandle",
        ".windows(2)",
        ".any(|pair| pair[0] >= pair[1])",
        "let before_ready = match self.validate_source(source)",
        "let readiness = source.source.check_readiness();",
        "let after = self.validate_source(source);",
        "Ok(before_ready && readiness.is_ok() && after.is_ok())",
    ):
        assert contract in source

    assert source.count("self.validate_source(source)") == 2
    assert source.count("source.source.check_readiness()") == 1
    _assert_public_source_pool_metadata(source, _read(NODE_OUTBOX))


def _assert_public_source_pool_metadata(source: str, outbox: str) -> None:
    """Keep credential owners opaque and out of pool metadata and checkpoints."""

    for name, fields in (
        ("ProviderIngestAuthenticatedSourceBindingV1", '''
            pub provider_id: [u8; 32], pub runtime_handle: String,
            pub revision: u64, pub policy_digest: [u8; 32],
        '''),
        ("ProviderIngestAuthenticatedSourceRegistrationV1", '''
            binding: ProviderIngestAuthenticatedSourceBindingV1,
            source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
        '''),
        ("PinnedProviderIngestSourceV1", '''
            binding: ProviderIngestAuthenticatedSourceBindingV1,
            source: Arc<dyn ProviderIngestAuthenticatedProviderSourceV1<Fetched = Fetched>>,
        '''),
        ("ProviderIngestAuthenticatedSourcePoolV1", '''
            runtime_handle: String,
            qualification: ProviderIngestRuntimeProviderQualificationV1,
            max_sources_per_fetch: usize, provider_ids: Vec<[u8; 32]>,
            sources: BTreeMap<[u8; 32], PinnedProviderIngestSourceV1<Fetched>>,
        '''),
    ):
        assert _code(_body(source, rf"\bstruct\s+{name}\b")) == _code(fields)
    for name, fields in (
        ("ProviderIngestAuthenticatedSourceRegistrationV1", ("binding",)),
        ("ProviderIngestAuthenticatedSourcePoolV1", (
            "runtime_handle", "qualification", "max_sources_per_fetch", "provider_ids"
        )),
    ):
        debug = _body(source, rf"\bimpl<[^{{}}]*fmt::Debug\s+for\s+{name}<Fetched>")
        expected = 'fn fmt(&self, formatter: &mut fmt::Formatter<\'_>) -> fmt::Result {'
        expected += 'formatter.debug_struct("public type")'
        expected += ''.join(f'.field("public field", &self.{field})' for field in fields)
        expected += '.finish_non_exhaustive() }'
        assert _code(debug) == _code(expected)
    # The durable owner never holds or serializes the runtime credential adapters.
    assert "ProviderIngestAuthenticatedSource" not in mask_rust(outbox)
    assert "PinnedProviderIngestSourceV1" not in mask_rust(outbox)


@pytest.mark.parametrize("mutation", ("binding_secret", "pool_secret", "debug_source", "durable_source"))
def test_source_pool_metadata_contract_rejects_secret_or_adapter_leakage(mutation: str) -> None:
    source = _read(NODE_RUNTIME)
    outbox = _read(NODE_OUTBOX)
    if mutation == "binding_secret":
        source = source.replace(
            "pub struct ProviderIngestAuthenticatedSourceBindingV1 {",
            "pub struct ProviderIngestAuthenticatedSourceBindingV1 { pub credential: String,",
        )
    elif mutation == "pool_secret":
        source = source.replace(
            "pub struct ProviderIngestAuthenticatedSourcePoolV1<Fetched: Send + 'static> {",
            "pub struct ProviderIngestAuthenticatedSourcePoolV1<Fetched: Send + 'static> { private_key: Vec<u8>,",
        )
    elif mutation == "debug_source":
        source = source.replace(
            '.field("provider_ids", &self.provider_ids)',
            '.field("provider_ids", &self.provider_ids).field("source", &self.sources)',
        )
    else:
        outbox = outbox.replace(
            "struct ProviderIngestOutboxCheckpointV1 {",
            "struct ProviderIngestOutboxCheckpointV1 { source: ProviderIngestAuthenticatedSourceBindingV1,",
        )
    with pytest.raises(AssertionError):
        _assert_public_source_pool_metadata(source, outbox)


def test_standard_daemon_pins_multi_provider_inventory_across_startup_and_ticks() -> None:
    source = _read(DAEMON_RUNTIME)

    assert "fn source_provider_ids(&self) -> &[[u8; 32]];" in source
    assert (
        "for ProviderIngestAuthenticatedSourcePoolV1<VerifiedProviderIngestPayloadV1>"
        in source
    )
    assert source.count("validate_authenticated_source_inventory(") >= 4
    assert "provider_ids.len() < 2" in source
    assert "provider_ids.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS" in source
    assert "*provider_id == local_provider_id" in source
    assert "Some(source_provider_ids)" in source
    assert "Some(&self.source_provider_ids)" in source
    assert (
        "authenticated provider-ingest source inventory is missing, substituted, "
        "noncanonical, or out of bounds"
        in source
    )


def test_completion_signer_binding_is_public_exact_and_rechecked() -> None:
    node = _read(NODE_RUNTIME)
    daemon = _read(DAEMON_RUNTIME)
    daemon_tests = _read(DAEMON_RUNTIME_TESTS)
    config = f"{_read(CONFIG_USER)}\n{_read(CONFIG_ACTUAL)}"

    for contract in (
        "pub struct ProviderIngestCompletionSignerQualificationV1",
        "pub struct ProviderIngestCompletionSignerBindingV1",
        "pub adapter_revision: u64",
        "pub signer_policy: ProviderIngestCompletionSignerPolicyV1",
        "pub algorithm: Algorithm",
        "pub public_key: PublicKey",
        "Algorithm::Ed25519 | Algorithm::MlDsa",
        "fn runtime_handle(&self) -> &str;",
        "fn qualification(",
        "qualification.matches_authority(expected_owner)",
    ):
        assert contract in node

    for contract in (
        "fn signer_binding(",
        "validate_resolver_signer_binding(",
        "configured_completion_signer_binding(",
        "expected_signer_binding: ProviderIngestCompletionSignerBindingV1",
        "self.current_eligibility()?;",
        "let transaction = self.signer.sign(payload).await?;",
    ):
        assert contract in daemon
    assert f"{daemon}\n{daemon_tests}".count("validate_resolver_signer_binding(") >= 7
    assert daemon.count("self.current_eligibility()?;") >= 2

    for field in (
        "completion_signer_resolver_handle",
        "completion_signer_handle",
        "completion_signer_adapter_revision",
        "completion_signer_policy_id_hex",
        "completion_signer_policy_revision",
        "completion_signer_policy_predecessor_digest_hex",
        "completion_signer_policy_digest_hex",
        "completion_signer_algorithm",
        "completion_signer_public_key_hex",
    ):
        assert field in config
    provider_ingest_config = config[
        config.index("pub struct SorafsProviderIngestRuntimeConfig")
        : config.index("impl SorafsProviderIngestRuntimeConfig")
    ]
    assert "private_key" not in provider_ingest_config
    assert "credential" not in provider_ingest_config


def test_completion_commit_still_revalidates_the_full_finalized_context() -> None:
    source = _read(COMMIT_EXECUTION)

    for contract in (
        "self.expected_assignment_revision == 0",
        "!self.expected_authority.is_valid()",
        "provider_ingest_anchor_matches_committed_prefix(",
        "record.assignment_revision != self.expected_assignment_revision",
        "provider_owner != &self.expected_authority.provider_owner",
        "completion_authority != &self.expected_authority",
        "assignment_revision: self.expected_assignment_revision",
        "completion_authority: self.expected_authority.clone()",
        "finalized_anchor: self.finalized_anchor",
    ):
        assert contract in source


def test_provider_indexed_committed_archive_is_the_only_daemon_reader() -> None:
    daemon = _read(DAEMON_RUNTIME)
    archive = _read(PROVIDER_ARCHIVE)

    for contract in (
        "Authoritative assignments come only from the daemon-owned immutable archive",
        "ArchivedProviderIngestFinalizedLedgerV1",
        ".read_assignment_page(",
        "qualify daemon-owned finalized provider-ingest archive activation gate",
    ):
        assert contract in daemon
    assert "view.world().replication_orders().iter()" not in daemon

    for contract in (
        "pub struct ProviderIngestFinalizedArchiveV1",
        "pub fn capture_kura_authenticated_view(",
        "pub fn qualify_against_kura_tip(",
        "pub fn read_provider_page(",
        "pub fn prepare_kura_authenticated_compaction(",
        "pub fn approve_and_install_kura_authenticated_compaction(",
        "RetentionAuthorityRequired",
        "require_exact_retention_readback(",
    ):
        assert contract in archive


def test_provider_ingest_docs_separate_pool_closure_from_external_blockers() -> None:
    combined = f"{_read(STORAGE_DOC)}\n{_read(CLOSURE_LEDGER)}"

    for contract in (
        "at least two non-local",
        "before and after fetch",
        "not copied into pool metadata",
        "governance-advert/stream-grant/pinned-HTTPS child transports",
        "own configured production",
        "completion-signer",
        "governance-aware external software signer backend",
        "provider-indexed immutable archive",
        "deployment-owned sealed-CAS backend",
        "retention-authority protocol",
    ):
        assert contract in combined


def test_provider_ingest_clippy_shapes_preserve_the_durable_codec() -> None:
    outbox = _read(NODE_OUTBOX)
    node_lib = _read(NODE_LIB)
    completion_codec = _read(
        NODE_OUTBOX.parent / "provider_ingest_outbox" / "completion_codec.rs"
    )
    assert "mod completion_codec;" in outbox, "boxed-completion codec module is disconnected"

    for contract in (
        "struct ProviderIngestExposedCompletionExpiryV1<'a>",
        "request: ProviderIngestExposedCompletionExpiryV1<'_>",
        "struct BoxedStoredCompletionDeliveryV1(Box<StoredCompletionDeliveryV1>);",
        "fn boxed_completion_codec_preserves_prior_bytes()",
        "assert_eq!(actual, expected);",
    ):
        assert contract in outbox

    for contract in (
        "use super::{BoxedStoredCompletionDeliveryV1, StoredCompletionDeliveryV1};",
        "NoritoSerialize for BoxedStoredCompletionDeliveryV1",
        "NoritoDeserialize<'a> for BoxedStoredCompletionDeliveryV1",
        "SerializePayload for BoxedStoredCompletionDeliveryV1",
        "<StoredCompletionDeliveryV1 as norito::core::NoritoSerialize>::schema_hash()",
        "<StoredCompletionDeliveryV1 as norito::core::NoritoDeserialize<'a>>::schema_hash()",
        "norito::core::SerializePayload::serialize(self.0.as_ref(), writer)",
        "archived.cast::<StoredCompletionDeliveryV1>()",
    ):
        assert contract in completion_codec, f"missing boxed-completion codec contract: {contract}"

    assert "completion: Box<StoredCompletionDeliveryV1>" not in outbox
    assert "pub type FinalizedProviderIngestRuntimeResultV1<" in node_lib
    assert ") -> FinalizedProviderIngestRuntimeResultV1<" in node_lib
