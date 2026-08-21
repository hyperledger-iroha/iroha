"""Adversarial tests for source-bound Sumeragi v2 Verus evidence."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "formal" / "sumeragi_v2_verus_evidence.py"
MANIFEST = "a" * 64
NONCE = "b" * 64


def load_module():
    """Load the evidence helper from its repository path."""

    spec = importlib.util.spec_from_file_location("sumeragi_v2_verus_evidence", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def sha256(path: Path) -> str:
    """Return one fixture file digest."""

    return hashlib.sha256(path.read_bytes()).hexdigest()


def configure_fixture(module, monkeypatch, tmp_path: Path):
    """Create one canonical evidence fixture with pinned synthetic tools."""

    source = tmp_path / "source.rs"
    source.write_text("verified source\n", encoding="utf-8")
    log = tmp_path / module.EXPECTED_LOG_PATH
    log.parent.mkdir(parents=True)
    log.write_text(
        "\n".join(
            (
                module.begin_marker(NONCE, MANIFEST),
                "verification results:: 1690 verified, 0 errors",
                f"verification results:: {module.EXPECTED_ROOT_VERIFIED} verified, 0 errors",
                module.success_marker(NONCE, MANIFEST),
            )
        )
        + "\n",
        encoding="utf-8",
    )
    host = "TestOS-test"
    tool = {
        "platform": "test-platform",
        "verus": "c" * 64,
        "cargo_verus": "d" * 64,
    }
    monkeypatch.setattr(module, "REQUIRED_SOURCE_PATHS", ("source.rs",))
    monkeypatch.setattr(module, "workspace_source_manifest", lambda _root: MANIFEST)
    monkeypatch.setattr(module, "_host_key", lambda: host)
    monkeypatch.setattr(module, "EXPECTED_TOOL_SHA256", {host: tool})
    monkeypatch.setattr(module, "_verify_invocation_contract", lambda _root: None)

    evidence = {
        "schema_version": module.SCHEMA_VERSION,
        "verification_contract_sha256": module.verification_contract_sha256(),
        "source_manifest_sha256": MANIFEST,
        "sources": [{"path": "source.rs", "sha256": sha256(source)}],
        "tool": {
            "version": module.EXPECTED_VERUS_VERSION,
            "platform": tool["platform"],
            "verus_sha256": tool["verus"],
            "cargo_verus_sha256": tool["cargo_verus"],
        },
        "invocation": list(module.EXPECTED_INVOCATION),
        "log": module.EXPECTED_LOG_PATH,
        "log_sha256": sha256(log),
        "nonce": NONCE,
        "results": {
            "dependency_verified": module.EXPECTED_DEPENDENCY_VERIFIED,
            "root_verified": module.EXPECTED_ROOT_VERIFIED,
            "errors": 0,
        },
        "backend_verification": True,
    }
    return evidence, source, log


def test_canonical_verus_evidence_is_accepted(monkeypatch, tmp_path: Path) -> None:
    """The exact source/tool/invocation/log binding validates."""

    module = load_module()
    evidence, _, _ = configure_fixture(module, monkeypatch, tmp_path)
    assert module.SCHEMA_VERSION == 2
    assert module.validate_evidence(evidence, root=tmp_path) == ()


@pytest.mark.parametrize(
    ("mutation", "expected"),
    (
        (
            lambda evidence: evidence.__setitem__("schema_version", 1),
            "schema_version must equal 2",
        ),
        (
            lambda evidence: evidence["invocation"].remove("--no-cheating"),
            "invocation does not match",
        ),
        (
            lambda evidence: evidence.__setitem__("source_manifest_sha256", "0" * 64),
            "not bound to the current workspace source manifest",
        ),
        (
            lambda evidence: evidence.__setitem__("backend_verification", "true"),
            "backend_verification=true",
        ),
        (
            lambda evidence: evidence.__setitem__(
                "verification_contract_sha256", "0" * 64
            ),
            "verification contract digest has drifted",
        ),
        (
            lambda evidence: evidence["results"].__setitem__("root_verified", 0),
            "independently pinned counts",
        ),
        (
            lambda evidence: evidence["tool"].__setitem__("verus_sha256", "0" * 64),
            "verifier digest is not pinned",
        ),
        (
            lambda evidence: evidence.__setitem__("nonce", "not-a-nonce"),
            "nonce must be 64 lowercase hexadecimal digits",
        ),
        (
            lambda evidence: evidence.__setitem__("unexpected", True),
            "fields must equal",
        ),
    ),
)
def test_evidence_metadata_mutations_are_rejected(
    monkeypatch, tmp_path: Path, mutation, expected: str
) -> None:
    """Self-reported flags, counts, hashes, and invocations are not trusted."""

    module = load_module()
    evidence, _, _ = configure_fixture(module, monkeypatch, tmp_path)
    drifted = copy.deepcopy(evidence)
    mutation(drifted)
    assert any(expected in error for error in module.validate_evidence(drifted, root=tmp_path))


def test_source_drift_invalidates_existing_evidence(monkeypatch, tmp_path: Path) -> None:
    """Changing an authoritative source invalidates its per-file binding."""

    module = load_module()
    evidence, source, _ = configure_fixture(module, monkeypatch, tmp_path)
    source.write_text("mutated source\n", encoding="utf-8")
    assert "Verus evidence source inventory or digest has drifted" in module.validate_evidence(
        evidence, root=tmp_path
    )


def test_log_digest_drift_is_rejected(monkeypatch, tmp_path: Path) -> None:
    """A transcript cannot be replaced after evidence generation."""

    module = load_module()
    evidence, _, log = configure_fixture(module, monkeypatch, tmp_path)
    log.write_text(log.read_text(encoding="utf-8") + "trailing output\n", encoding="utf-8")
    errors = module.validate_evidence(evidence, root=tmp_path)
    assert "Verus evidence log digest mismatch" in errors
    assert any("delimit the complete transcript" in error for error in errors)


def test_archived_log_override_retains_canonical_evidence_name(
    monkeypatch, tmp_path: Path
) -> None:
    """A copied release log validates without rewriting the evidence payload."""

    module = load_module()
    evidence, _, log = configure_fixture(module, monkeypatch, tmp_path)
    archived = tmp_path / "archive" / "verus.log"
    archived.parent.mkdir()
    archived.write_bytes(log.read_bytes())
    log.unlink()
    assert module.validate_evidence(
        evidence, root=tmp_path, log_path=archived
    ) == ()


@pytest.mark.parametrize(
    "lines",
    (
        (
            "verification results:: 1690 verified, 0 errors",
            "verification results:: 171 verified, 0 errors",
        ),
        (
            "BEGIN",
            "verification results:: 1690 verified, 0 errors",
            "verification results:: 171 verified, 0 errors",
            "SUCCESS",
            "SUCCESS",
        ),
        (
            "BEGIN",
            "verification results:: 1690 verified, 0 errors",
            "verification results:: 0 verified, 0 errors",
            "SUCCESS",
        ),
        (
            "BEGIN",
            "verification results:: 1690 verified, 1 errors",
            "verification results:: 171 verified, 0 errors",
            "SUCCESS",
        ),
        (
            "BEGIN",
            "verification results:: 1690 verified, 0 errors",
            "verification results:: 105 verified, 0 errors",
            "SUCCESS",
        ),
        (
            "BEGIN",
            "verification results:: 171 verified, 0 errors",
            "verification results:: 1690 verified, 0 errors",
            "SUCCESS",
        ),
    ),
)
def test_malformed_or_ambiguous_verus_transcripts_are_rejected(lines) -> None:
    """Zero, reordered, failed, missing, or repeated markers cannot pass."""

    module = load_module()
    expanded = [
        module.begin_marker(NONCE, MANIFEST)
        if line == "BEGIN"
        else module.success_marker(NONCE, MANIFEST)
        if line == "SUCCESS"
        else line
        for line in lines
    ]
    with pytest.raises(ValueError):
        module.parse_verus_log(
            "\n".join(expanded) + "\n",
            nonce=NONCE,
            source_manifest_sha256=MANIFEST,
        )


def test_symlinked_verus_log_is_rejected(monkeypatch, tmp_path: Path) -> None:
    """Release evidence cannot redirect the canonical log through a symlink."""

    module = load_module()
    evidence, _, log = configure_fixture(module, monkeypatch, tmp_path)
    target = tmp_path / "actual.log"
    log.replace(target)
    log.symlink_to(target)
    assert any(
        "log is not a regular file" in error
        for error in module.validate_evidence(evidence, root=tmp_path)
    )


def test_fake_verifier_digest_is_rejected_before_evidence_is_built(
    monkeypatch, tmp_path: Path
) -> None:
    """An echo-like executable cannot mint evidence under the pinned tool name."""

    module = load_module()
    _, _, log = configure_fixture(module, monkeypatch, tmp_path)
    verifier = tmp_path / "verus"
    cargo_verus = tmp_path / "cargo-verus"
    verifier.write_text("#!/bin/sh\nprintf 'Version: fake\\nPlatform: test-platform\\n'\n")
    cargo_verus.write_text("#!/bin/sh\nexit 0\n")
    verifier.chmod(0o755)
    cargo_verus.chmod(0o755)
    with pytest.raises(ValueError):
        module.build_evidence(
            root=tmp_path,
            log_path=log,
            nonce=NONCE,
            verus=verifier,
            cargo_verus=cargo_verus,
        )


def test_indented_pinned_verus_version_output_is_normalized(tmp_path: Path) -> None:
    """The release binary's indented version fields remain machine-readable."""

    module = load_module()
    verifier = tmp_path / "verus"
    verifier.write_text(
        "#!/bin/sh\n"
        "printf 'Verus\\n  Version: "
        f"{module.EXPECTED_VERUS_VERSION}"
        "\\n  Platform: macos_aarch64\\n'\n",
        encoding="utf-8",
    )
    verifier.chmod(0o755)
    assert module._verus_version(verifier) == (
        module.EXPECTED_VERUS_VERSION,
        "macos_aarch64",
    )


def test_repository_verus_invocation_matches_the_evidence_contract() -> None:
    """The recorded command is exactly the one the checked-in runner executes."""

    module = load_module()
    module._verify_invocation_contract(ROOT)


def test_repository_required_verus_sources_are_regular_files() -> None:
    """Every source named by the evidence contract exists on the real tree."""

    module = load_module()
    entries = module._source_entries(ROOT)
    assert [entry["path"] for entry in entries] == list(module.REQUIRED_SOURCE_PATHS)


def test_repository_verus_evidence_binds_the_sidecar_admission_corridor() -> None:
    """Every production owner in the chunk-admission handoff is source-bound."""

    module = load_module()
    assert {
        "crates/iroha_core/src/merge_sidecar.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "crates/iroha_core/src/sumeragi/v2_lane_work/canonical_executed_block_application_repair.rs",
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "crates/iroha_p2p/src/network.rs",
        "crates/iroha_p2p/src/peer.rs",
        "crates/irohad/src/main.rs",
    } <= set(module.REQUIRED_SOURCE_PATHS)


def test_repository_verus_evidence_binds_exact_timeout_proposal_corridor() -> None:
    """Shared predicate, WAL callers, regressions, and Verus proof are sealed."""

    module = load_module()
    assert {
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
        "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
        "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
        "crates/iroha_sumeragi_core/src/verus_proofs.rs",
    } <= set(module.REQUIRED_SOURCE_PATHS)


def test_repository_verus_evidence_binds_lexically_included_proof_tail() -> None:
    """Lexically included proof and checked-token providers are sealed."""

    module = load_module()
    root_source = "crates/iroha_sumeragi_core/src/verus_proofs.rs"
    included_tail = (
        "crates/iroha_sumeragi_core/src/verus_proofs/production_kernel_tail.rs"
    )
    checked_token_parent = (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    )
    checked_token_provider = (
        "crates/iroha_core/src/sumeragi/v2_core/refinement/first_release_witness.rs"
    )
    assert {
        root_source,
        included_tail,
        checked_token_parent,
        checked_token_provider,
    } <= set(module.REQUIRED_SOURCE_PATHS)
    assert (
        ROOT.joinpath(root_source)
        .read_text(encoding="utf-8")
        .count('include!("verus_proofs/production_kernel_tail.rs");')
        == 1
    )
    assert (
        ROOT.joinpath(checked_token_parent)
        .read_text(encoding="utf-8")
        .count('include!("refinement/first_release_witness.rs");')
        == 1
    )


def test_repository_verus_evidence_binds_extracted_runtime_providers() -> None:
    """Extracted adapter and runtime implementations remain independently sealed."""

    module = load_module()
    providers = {
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "crates/iroha_core/src/sumeragi/v2_wire_registry_and_authentication.rs",
        "crates/iroha_core/src/sumeragi/v2_ready_durable_validate_adapter_preview.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime_effect_ownership_core_impl.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime_effect_ownership_rebind_impl.rs",
    }
    assert providers <= set(module.REQUIRED_SOURCE_PATHS)
    adapter = ROOT.joinpath("crates/iroha_core/src/sumeragi/v2.rs").read_text(
        encoding="utf-8"
    )
    runtime = ROOT.joinpath("crates/iroha_core/src/sumeragi/v2_runtime.rs").read_text(
        encoding="utf-8"
    )
    assert adapter.count(
        'include!("v2_authenticated_recovered_adapter_startup_impl.rs");'
    ) == 1
    assert adapter.count('include!("v2_wire_registry_and_authentication.rs");') == 1
    assert adapter.count('include!("v2_ready_durable_validate_adapter_preview.rs");') == 1
    assert runtime.count('include!("v2_runtime_effect_ownership_core_impl.rs");') == 1
    assert runtime.count(
        'include!("v2_runtime_effect_ownership_rebind_impl.rs");'
    ) == 1


def test_verus_invocation_without_no_cheating_is_rejected(tmp_path: Path) -> None:
    """Deleting the root no-cheating flag invalidates the source contract."""

    module = load_module()
    runner = tmp_path / "scripts" / "verify_sumeragi_v2.sh"
    harness = tmp_path / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    runner.parent.mkdir(parents=True)
    harness.parent.mkdir(parents=True)
    runner.write_text(
        (ROOT / "scripts" / "verify_sumeragi_v2.sh").read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    harness_source = (
        ROOT / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    harness.write_text(
        harness_source.replace("      --no-cheating\n", "", 1),
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="harness command has drifted"):
        module._verify_invocation_contract(tmp_path)
