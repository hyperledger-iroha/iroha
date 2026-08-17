from __future__ import annotations

import argparse
import hashlib
import json
import os
import tarfile
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import build_taira_rollout_candidate as candidate
from scripts import deploy_taira_v21_reset as deploy
from scripts import release_artifact_contract as contract
from scripts import render_taira_validator_bundle as receipt_renderer
from scripts import taira_rollout_admission as admission


COMMIT = "1" * 40
DPN_COMMIT = "d" * 40
WORKSPACE_SHA = "2" * 64
CARGO_SHA = "3" * 64
SOURCE = admission.SourceIdentity(COMMIT, DPN_COMMIT, CARGO_SHA, WORKSPACE_SHA)


def _receipt_signers() -> dict[str, dict[str, object]]:
    result: dict[str, dict[str, object]] = {}
    for number, slug in enumerate(admission.SLUGS, start=1):
        private_payload = number.to_bytes(32, "big")
        public_payload = receipt_renderer._secp256k1_public_payload(private_payload)
        public_key = receipt_renderer.RECEIPT_PUBLIC_KEY_PREFIX + (
            public_payload.hex().upper()
        )
        result[slug] = {
            "node_id": receipt_renderer.receipt_node_id(public_key),
            "public_key": {
                "algorithm": "secp256k1",
                "payload_hex": public_payload.hex(),
            },
        }
    return result


def _write(path: Path, payload: bytes) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return hashlib.sha256(payload).hexdigest()


def _fixture(tmp_path: Path) -> tuple[argparse.Namespace, dict[str, object]]:
    bundle = tmp_path / "bundle"
    binary = tmp_path / "iroha3d"
    supervisor = tmp_path / "taira_peer_supervisor.py"
    binary_sha = _write(binary, b"native-macos-arm64-validator\n")
    supervisor_sha = _write(supervisor, b"native-supervisor\n")
    reset_sha = _write(bundle / "reset-manifest.json", b"reset-manifest\n")
    config_digests = {
        f"taira-validator-{number}": _write(
            bundle / "rendered" / f"taira-validator-{number}" / "config.toml",
            f"validator-config-{number}\n".encode(),
        )
        for number in range(1, 5)
    }
    _write(bundle / "genesis.signed.nrt", b"signed-genesis\n")
    now = int(time.time())
    start_hash = "4" * 64
    end_hash = "5" * 64
    receipt_signers = _receipt_signers()
    body: dict[str, object] = {
        "artifact_handoff_sha256": "7" * 64,
        "end": {"block_hash": end_hash, "height": 102},
        "expires_at_unix": now + 900,
        "issued_at_unix": now - 10,
        "peer_count": 4,
        "peers": [
            {
                "final_block_hash": end_hash,
                "final_height": 102,
                "label": f"taira-validator-{number}",
                "number": number,
                "receipt_signer_node_id": receipt_signers[
                    f"taira-validator-{number}"
                ]["node_id"],
                "restart_proof": "passed",
                "source_commit": COMMIT,
                "validator_binary_sha256": binary_sha,
                "validator_config_sha256": config_digests[f"taira-validator-{number}"],
            }
            for number in range(1, 5)
        ],
        "platform": {"arch": "arm64", "os": "macos"},
        "receipt_signers": receipt_signers,
        "reset_manifest_sha256": reset_sha,
        "restart_generation": "6" * 64,
        "schema": admission.MACOS_RECEIPT_SCHEMA,
        "schema_version": admission.MACOS_RECEIPT_SCHEMA_VERSION,
        "source": SOURCE.as_dict(),
        "start": {"block_hash": start_hash, "height": 101},
        "supervisor_sha256": supervisor_sha,
        "validator_binary_sha256": binary_sha,
        "validator_config_sha256": config_digests,
    }
    receipt = {**body, "receipt_id": admission.compute_macos_receipt_id(body)}
    receipt_path = tmp_path / "receipt.json"
    receipt_path.write_bytes(contract.canonical_json_bytes(receipt))
    args = argparse.Namespace(
        cargo_lock_sha256=CARGO_SHA,
        dpn_validator_release_commit=DPN_COMMIT,
        macos_receipt=receipt_path,
        now_unix=now,
        output=tmp_path / "one" / "taira-macos-deploy.tar.gz",
        reset_bundle=bundle,
        source_commit=COMMIT,
        source_date_epoch=1_700_000_000,
        supervisor=supervisor,
        validator_binary=binary,
        workspace_source_manifest_sha256=WORKSPACE_SHA,
    )
    return args, receipt


def _stub_production_validation(
    monkeypatch: pytest.MonkeyPatch,
    receipt: dict[str, object],
) -> dict[str, object]:
    calls: dict[str, object] = {}
    configs = receipt["validator_config_sha256"]
    assert isinstance(configs, dict)

    def validate(bundle: Path, **kwargs):
        plan = SimpleNamespace(
            peers=tuple(
                SimpleNamespace(
                    slug=slug,
                    config_sha256=hashlib.sha256(
                        (bundle / "rendered" / slug / "config.toml").read_bytes()
                    ).hexdigest(),
                )
                for slug in sorted(configs)
            )
        )
        calls["bundle"] = bundle
        calls["kwargs"] = kwargs
        calls["plan"] = plan
        return plan

    def unchanged(actual_plan) -> None:
        assert actual_plan is calls["plan"]
        calls["runtime_rechecked"] = True

    monkeypatch.setattr(candidate.deploy, "validate_bundle", validate)
    monkeypatch.setattr(candidate.deploy, "require_bundle_runtime_unchanged", unchanged)
    return calls


def test_pack_deploy_payload_is_closed_deterministic_and_receipt_bound(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, receipt = _fixture(tmp_path)
    calls = _stub_production_validation(monkeypatch, receipt)
    first = candidate.pack_deploy_payload(args)
    second_args = argparse.Namespace(
        **{**vars(args), "output": tmp_path / "two" / args.output.name}
    )
    second = candidate.pack_deploy_payload(second_args)

    assert Path(first["archive"]).read_bytes() == Path(second["archive"]).read_bytes()
    assert calls["bundle"] == args.reset_bundle
    assert calls["runtime_rechecked"] is True
    validation_args = calls["kwargs"]
    assert isinstance(validation_args, dict)
    assert validation_args["expected_source_commit"] == COMMIT
    assert (
        validation_args["expected_dpn_validator_release_commit"] == DPN_COMMIT
    )
    assert (
        validation_args["expected_binary_sha256"] == receipt["validator_binary_sha256"]
    )
    assert first["receipt_id"] == receipt["receipt_id"]
    prefix = args.output.name.removesuffix(".tar.gz")
    with tarfile.open(args.output, mode="r:gz") as archive:
        members = archive.getmembers()
        assert all(
            (member.isfile() or member.isdir()) and not member.issparse()
            for member in members
        )
        names = [member.name for member in members]
        assert names == sorted(names)
        assert len(names) == len(set(names))
        assert f"{prefix}/bundle/rendered/taira-validator-1" in names
        assert f"{prefix}/bin" in names
        assert f"{prefix}/supervisor" in names
        manifest_member = archive.extractfile(
            f"{prefix}/{candidate.DEPLOY_PAYLOAD_MANIFEST}"
        )
        assert manifest_member is not None
        manifest = json.load(manifest_member)
    assert manifest["schema"] == candidate.DEPLOY_PAYLOAD_SCHEMA
    assert manifest["receipt_id"] == receipt["receipt_id"]
    assert manifest["source"] == SOURCE.as_dict()
    assert (
        manifest["components"]["validator_config_sha256"]
        == receipt["validator_config_sha256"]
    )
    assert [row["path"] for row in manifest["inventory"]] == sorted(
        row["path"] for row in manifest["inventory"]
    )
    assert any(
        row
        == {
            "kind": "directory",
            "mode": "0700",
            "path": "bundle/rendered/taira-validator-1",
        }
        for row in manifest["inventory"]
    )
    assert any(
        row["kind"] == "file" and row["mode"] == "0500" and row["path"] == "bin/iroha3d"
        for row in manifest["inventory"]
    )


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (
            lambda args: args.validator_binary.write_bytes(b"substituted\n"),
            "validator bytes differ",
        ),
        (
            lambda args: (
                args.reset_bundle / "rendered/taira-validator-3/config.toml"
            ).write_bytes(b"substituted-config\n"),
            "config bytes differ",
        ),
        (
            lambda args: args.macos_receipt.write_bytes(
                args.macos_receipt.read_bytes() + b"\n"
            ),
            "not canonical",
        ),
        (
            lambda args: args.supervisor.write_bytes(b"substituted-supervisor\n"),
            "supervisor bytes differ",
        ),
        (
            lambda args: (args.reset_bundle / "reset-manifest.json").write_bytes(
                b"substituted-reset-manifest\n"
            ),
            "reset manifest bytes differ",
        ),
    ),
)
def test_pack_deploy_payload_rejects_adversarial_component_substitution(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation,
    message: str,
) -> None:
    args, receipt = _fixture(tmp_path)
    _stub_production_validation(monkeypatch, receipt)
    mutation(args)
    with pytest.raises(
        (candidate.TairaCandidateBuildError, admission.TairaRolloutAdmissionError),
        match=message,
    ):
        candidate.pack_deploy_payload(args)


def test_pack_deploy_payload_rejects_symlink_in_closed_bundle(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args, receipt = _fixture(tmp_path)
    _stub_production_validation(monkeypatch, receipt)
    (args.reset_bundle / "unsafe").symlink_to(args.validator_binary)

    with pytest.raises(candidate.TairaCandidateBuildError, match="unsafe regular file"):
        candidate.pack_deploy_payload(args)


@pytest.mark.parametrize("attack", ("empty", "hardlink"))
def test_pack_deploy_payload_rejects_unsafe_regular_file(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, attack: str
) -> None:
    args, receipt = _fixture(tmp_path)
    _stub_production_validation(monkeypatch, receipt)
    unsafe = args.reset_bundle / "unsafe"
    if attack == "empty":
        unsafe.touch()
    else:
        os.link(args.reset_bundle / "genesis.signed.nrt", unsafe)

    with pytest.raises(candidate.TairaCandidateBuildError, match="unsafe regular file"):
        candidate.pack_deploy_payload(args)


def test_pack_deploy_payload_rejects_bundle_that_production_validator_refuses(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args, _ = _fixture(tmp_path)

    def reject(*_args, **_kwargs):
        raise deploy.DeploymentError("unexpected reset inventory")

    monkeypatch.setattr(candidate.deploy, "validate_bundle", reject)

    with pytest.raises(
        candidate.TairaCandidateBuildError,
        match="production revalidation: unexpected reset inventory",
    ):
        candidate.pack_deploy_payload(args)


def test_pack_deploy_payload_rejects_dpn_only_receipt_mismatch(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args, receipt = _fixture(tmp_path)
    _stub_production_validation(monkeypatch, receipt)
    args.dpn_validator_release_commit = "e" * 40

    with pytest.raises(
        admission.TairaRolloutAdmissionError,
        match="source identity differs",
    ):
        candidate.pack_deploy_payload(args)
