"""Hostile no-I/O tests for the unprovisioned rollout-observation authority."""

from __future__ import annotations

import inspect
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import close_taira_publication_handoff as publication_closer
from scripts import publish_taira_rollout as publisher
from scripts import seal_taira_release_controllers as sealed_controller
from scripts import taira_privacy_rollout_contract as rollout


ROOT = Path(__file__).resolve().parents[2]
HOSTILE_OBSERVATIONS = (
    "synthetic-four-peer-result",
    "recomputed-self-hashes",
    "candidate-or-publication-signer-reuse",
    "stale-run-or-replay",
    "source-or-deploy-splice",
    "controller-or-host-splice",
    "operation-table-or-result-splice",
    "legacy-unsigned-observation",
)


def _forbidden(label: str, calls: list[str]):
    def fail(*_args, **_kwargs):
        calls.append(label)
        raise AssertionError(f"barrier ran after forbidden operation: {label}")

    return fail


def test_provisioning_contract_is_independent_canonical_and_unconditional() -> None:
    barrier = rollout.require_authenticated_rollout_observation_authority_provisioned
    assert not inspect.signature(barrier).parameters
    source = inspect.getsource(barrier)
    assert "os.environ" not in source
    assert "os.getenv" not in source
    assert "Path(" not in source
    for delegated in (
        publisher._require_authenticated_rollout_observation_authority,
        publication_closer._require_authenticated_rollout_observation_authority,
        sealed_controller._require_authenticated_rollout_observation_authority,
    ):
        assert not inspect.signature(delegated).parameters
        delegated_source = inspect.getsource(delegated)
        assert "os.environ" not in delegated_source
        assert "os.getenv" not in delegated_source

    with pytest.raises(rollout.RolloutContractError) as failure:
        barrier()
    message = str(failure.value)
    for required in (
        rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
        rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_REPLAY_NAMESPACE,
        "separately pinned trust root",
        "exact rollout-plan and observation bytes and digests",
        "admitted candidate",
        "source and DPN commits",
        "Cargo.lock and workspace-source",
        "candidate OCI digest",
        "qualification receipt",
        "deploy receipt",
        "deployed binary/config/capability identities",
        "all four peer and public-Torii identities",
        "supervisor",
        "authority host and installation identities",
        "installed controller digest",
        "run nonce",
        "issued time",
        "expiry",
        "replay identity",
        "governance, canary, resource, restart, convergence",
        "owner/path-only trust",
        "reused signing keys",
        "legacy unsigned observations",
    ):
        assert required in message


@pytest.mark.parametrize("hostile_case", HOSTILE_OBSERVATIONS)
def test_public_observation_validator_stops_before_structural_inspection(
    monkeypatch: pytest.MonkeyPatch,
    hostile_case: str,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        rollout,
        "_validate_unsigned_result_structure",
        _forbidden("unsigned structural validation", calls),
    )
    attacker_result = {
        "hostile_case": hostile_case,
        "publication_authorized": True,
        "rollout_id": "00" * 32,
    }
    with pytest.raises(
        rollout.RolloutContractError,
        match=rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        rollout.validate_result(attacker_result, plan={})
    assert calls == []


def test_observation_cli_stops_before_plan_or_result_path_io(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(rollout, "_load", _forbidden("path read", calls))
    plan = tmp_path / "attacker-plan.json"
    result = tmp_path / "attacker-result.json"
    assert (
        rollout.main(
            [
                "verify-result",
                "--plan",
                str(plan),
                "--result",
                str(result),
            ]
        )
        == 1
    )
    captured = capsys.readouterr()
    assert captured.out == ""
    assert rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA in captured.err
    assert calls == []
    assert not plan.exists()
    assert not result.exists()


@pytest.mark.parametrize("hostile_case", HOSTILE_OBSERVATIONS)
def test_publication_stops_before_request_path_signer_registry_or_replay_io(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    hostile_case: str,
) -> None:
    calls: list[str] = []
    for name in (
        "_publish_after_authenticated_rollout_observation",
        "_validate_request",
        "_capture_candidate",
        "_capture_pinned_executable",
        "_capture_registry_config",
        "_admission_bytes",
        "sign_release_manifest",
    ):
        monkeypatch.setattr(publisher, name, _forbidden(name, calls))

    signer = tmp_path / "reused-candidate-release-publication-signer"
    signer.write_bytes(b"must remain untouched\n")
    signer_before = signer.stat()
    replay = tmp_path / "publication-replay.json"
    replay.write_bytes(b"unchanged replay state\n")
    replay_before = replay.stat()
    output = tmp_path / "publication-output"
    request = SimpleNamespace(
        hostile_case=hostile_case,
        external_signer_path=signer,
        replay_ledger_path=replay,
        terminal_handoff=output,
    )

    with pytest.raises(
        publisher.TairaPublicationError,
        match=rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        publisher.publish(request)
    assert calls == []
    assert not output.exists()
    assert signer.read_bytes() == b"must remain untouched\n"
    assert replay.read_bytes() == b"unchanged replay state\n"
    signer_after = signer.stat()
    replay_after = replay.stat()
    assert (signer_after.st_ino, signer_after.st_size, signer_after.st_mtime_ns) == (
        signer_before.st_ino,
        signer_before.st_size,
        signer_before.st_mtime_ns,
    )
    assert (replay_after.st_ino, replay_after.st_size, replay_after.st_mtime_ns) == (
        replay_before.st_ino,
        replay_before.st_size,
        replay_before.st_mtime_ns,
    )


@pytest.mark.parametrize(
    "operation", ("verify-privacy-rollout", "publish-rollout")
)
def test_installed_controller_main_stops_before_attestation_or_path_io(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    operation: str,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        sealed_controller,
        "parse_args",
        lambda _argv: SimpleNamespace(command="run", operation=operation),
    )
    monkeypatch.setattr(
        sealed_controller, "_attest", _forbidden("controller attestation", calls)
    )
    assert sealed_controller.main([]) == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert (
        sealed_controller.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA
        in captured.err
    )
    assert calls == []


def test_direct_installed_controller_routes_are_also_barriered(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        sealed_controller,
        "_dispatch_installed_python",
        _forbidden("installed validator execution", calls),
    )
    monkeypatch.setattr(
        sealed_controller,
        "_operation_option_values",
        _forbidden("publication argument inspection", calls),
    )
    with pytest.raises(
        sealed_controller.ControllerSealError,
        match=sealed_controller.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        sealed_controller._dispatch(
            "verify-privacy-rollout", ["--result", "/attacker/result"]
        )
    with pytest.raises(
        sealed_controller.ControllerSealError,
        match=sealed_controller.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        sealed_controller._dispatch_publication_composite([], {})
    assert calls == []


def test_publication_close_helper_stops_before_terminal_or_output_io(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    tmp_path: Path,
) -> None:
    calls: list[str] = []
    monkeypatch.setattr(
        publication_closer,
        "_close_handoff",
        _forbidden("publication terminal inspection", calls),
    )
    source = tmp_path / "attacker-terminal"
    output = tmp_path / "public-output"
    with pytest.raises(
        publication_closer.PublicationHandoffError,
        match=rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA,
    ):
        publication_closer.close_handoff(
            source,
            output,
            expected_authority_uid=501,
            expected_authority_gid=20,
            expected_controller_uid=0,
            expected_controller_gid=0,
            expected_qualification_receipt_id="11" * 32,
            expected_signing_fingerprint="22" * 32,
            expected_source_commit="33" * 20,
            expected_dpn_validator_release_commit="44" * 20,
            expected_cargo_lock_sha256="55" * 32,
            expected_workspace_source_manifest_sha256="66" * 32,
            rollout_plan=tmp_path / "plan.json",
            rollout_result=tmp_path / "result.json",
            rollout_authority_envelope=tmp_path / "envelope.json",
            rollout_durable_receipt=tmp_path / "receipt.json",
        )
    assert calls == []
    assert not source.exists()
    assert not output.exists()

    monkeypatch.setattr(
        publication_closer, "parse_args", lambda _argv: SimpleNamespace()
    )
    assert publication_closer.main([]) == 1
    captured = capsys.readouterr()
    assert captured.out == ""
    assert rollout.AUTHENTICATED_ROLLOUT_OBSERVATION_AUTHORITY_SCHEMA in captured.err
    assert calls == []


def test_untrusted_lower_helpers_have_no_unbarriered_production_caller() -> None:
    rollout_source = (ROOT / "scripts/taira_privacy_rollout_contract.py").read_text(
        encoding="utf-8"
    )
    publisher_source = (ROOT / "scripts/publish_taira_rollout.py").read_text(
        encoding="utf-8"
    )
    public_validator = inspect.getsource(rollout.validate_result)
    assert public_validator.index(
        "require_authenticated_rollout_observation_authority_provisioned()"
    ) < public_validator.index("_validate_unsigned_result_structure(")
    assert public_validator.index(
        "_validate_unsigned_result_structure("
    ) < public_validator.index("taira_authority_client.authorize(")
    public_publisher = inspect.getsource(publisher.publish)
    assert public_publisher.index(
        "_require_authenticated_rollout_observation_authority()"
    ) < public_publisher.index(
        "rollout_observation.verify_authenticated_result_files("
    )
    assert public_publisher.index(
        "rollout_observation.verify_authenticated_result_files("
    ) < public_publisher.index(
        "return _publish_after_authenticated_rollout_observation"
    )
    assert rollout_source.count("_validate_unsigned_result_structure(") == 3
    assert publisher_source.count(
        "_publish_after_authenticated_rollout_observation("
    ) == 2
