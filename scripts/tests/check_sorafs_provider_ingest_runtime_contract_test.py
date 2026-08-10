"""Static contracts for the production SoraFS provider-ingest runtime."""

from pathlib import Path


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
    assert "never copied into pool metadata or durable state" in source


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
        "governance-aware HSM/KMS signer backend",
        "provider-indexed immutable archive",
        "deployment-owned sealed-CAS backend",
        "retention-authority protocol",
    ):
        assert contract in combined


def test_provider_ingest_clippy_shapes_preserve_the_durable_codec() -> None:
    outbox = _read(NODE_OUTBOX)
    node_lib = _read(NODE_LIB)

    for contract in (
        "struct ProviderIngestExposedCompletionExpiryV1<'a>",
        "request: ProviderIngestExposedCompletionExpiryV1<'_>",
        "struct BoxedStoredCompletionDeliveryV1(Box<StoredCompletionDeliveryV1>);",
        "NoritoSerialize for BoxedStoredCompletionDeliveryV1",
        "NoritoDeserialize<'a> for BoxedStoredCompletionDeliveryV1",
        "fn boxed_completion_codec_preserves_prior_bytes()",
        "assert_eq!(actual, expected);",
    ):
        assert contract in outbox

    assert "completion: Box<StoredCompletionDeliveryV1>" not in outbox
    assert "pub type FinalizedProviderIngestRuntimeResultV1<" in node_lib
    assert ") -> FinalizedProviderIngestRuntimeResultV1<" in node_lib
