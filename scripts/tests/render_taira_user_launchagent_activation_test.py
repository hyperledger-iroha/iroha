"""Focused tests for the bounded Taira user-activation renderer."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

import pytest

from scripts import render_taira_user_launchagent_activation as module


GENERATION = "20260820-nevo-faucet-reset-1"
NETWORK_ID = "hash:" + "AB" * 32 + "#1234"
PUBLIC_KEY = "ed0120" + "AB" * 32
BASE_CONFIG = b"chain = 'taira'\n"
PRIVACY_PLAN = b'{"network_id":null,"schema":"fixture"}\n'


def source_body(relative: str) -> bytes:
    if relative == "configs/soranexus/taira/config.toml":
        return BASE_CONFIG
    if relative == "configs/soranexus/taira/privacy_bootstrap_plan.json":
        return PRIVACY_PLAN
    return f"source:{relative}\n".encode()


def private_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)
    return path


def private_file(
    path: Path,
    body: bytes,
    *,
    executable: bool = False,
) -> Path:
    private_dir(path.parent)
    path.write_bytes(body)
    path.chmod(0o700 if executable else 0o600)
    return path


def sha256(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def fixture_layout(tmp_path: Path) -> module.Layout:
    taira = private_dir(tmp_path / "taira")
    return module.Layout(
        taira_root=taira,
        reset_bundles=private_dir(taira / "reset-bundles"),
        reset_manifests=private_dir(taira / "reset-manifests"),
        releases=private_dir(taira / "releases"),
        local_controllers=private_dir(taira / "local-reset-controller"),
        local_python=Path(sys.executable).resolve(),
        uid=os.getuid(),
    )


def fixture(
    tmp_path: Path,
) -> tuple[module.Layout, argparse.Namespace, dict[str, object], dict[str, bytes]]:
    layout = fixture_layout(tmp_path)
    bundle = private_dir(layout.reset_bundles / GENERATION)
    release = private_dir(layout.releases / "20260820-4ac0f546-nevo-faucet")
    output = layout.reset_manifests / f"{GENERATION}.json"

    binaries = {
        "iroha3d": b"candidate-iroha3d\n",
        "kagami": b"candidate-kagami\n",
        "taira_operator_status": b"candidate-operator-status\n",
    }
    for name, body in binaries.items():
        private_file(release / name, body, executable=True)
    reviewed_unsigned = b'{"genesis":"reviewed"}\n'
    review_payload = {
        "schema": "iroha.taira.nevo-reset-review.v1",
        "public_inputs_sha256": "34" * 32,
        "unsigned_genesis_sha256": sha256(reviewed_unsigned),
        "public_identities": {},
        "credential_hash_bindings": {},
    }
    artifacts = {
        "nevo-reset.review.json": module.canonical_artifact_json(review_payload),
        "genesis.reviewed-unsigned.json": reviewed_unsigned,
        "genesis.pre-sign-rendered.json": b'{"genesis":"rendered"}\n',
        "genesis.json": b'{"genesis":"bound"}\n',
        "genesis.signed.nrt": b"signed-genesis-wire\n",
    }
    for name, body in artifacts.items():
        private_file(bundle / name, body)
    private_file(bundle / "base-config.toml", BASE_CONFIG)

    source_commit = "ab" * 20
    dpn_commit = "cd" * 20
    cargo_lock_sha256 = "19" * 32
    workspace_sha256 = "28" * 32
    source = {
        "commit": source_commit,
        "dpn_validator_release_commit": dpn_commit,
        "cargo_lock_sha256": cargo_lock_sha256,
        "workspace_source_manifest_sha256": workspace_sha256,
    }
    source_rows = [
        {
            "path": relative,
            "sha256": sha256(source_body(relative)),
            "size": len(source_body(relative)),
        }
        for relative in module.LOCAL_SOURCE_CLOSURE_FILES
    ]
    closure_payload = {
        "schema": "iroha.taira.local-testnet-reset-source-closure.v1",
        "files": source_rows,
    }
    closure_sha256 = sha256(module.canonical_artifact_json(closure_payload))
    reviewed_inputs = {
        "config.toml": {"sha256": sha256(BASE_CONFIG), "size": len(BASE_CONFIG)},
        "genesis.json": {
            "sha256": sha256(artifacts["genesis.reviewed-unsigned.json"]),
            "size": len(artifacts["genesis.reviewed-unsigned.json"]),
        },
        "nevo-reset.review.json": {
            "sha256": sha256(artifacts["nevo-reset.review.json"]),
            "size": len(artifacts["nevo-reset.review.json"]),
        },
        "privacy_bootstrap_plan.json": {
            "sha256": sha256(PRIVACY_PLAN),
            "size": len(PRIVACY_PLAN),
        },
    }
    reviewed_identity_manifest = {
        "schema": "iroha.taira.local_testnet_reviewed_inputs.v1",
        "authority_claim": "none-user-authorized-same-host-testnet",
        "source": source,
        "privacy_inputs": reviewed_inputs,
    }
    local_identity = sha256(
        module.canonical_artifact_json(reviewed_identity_manifest)
    )
    linkage = {
        "schema": "iroha.taira.nevo-genesis-artifact-linkage.v1",
        "review_sha256": sha256(artifacts["nevo-reset.review.json"]),
        "reviewed_unsigned_genesis_sha256": sha256(
            artifacts["genesis.reviewed-unsigned.json"]
        ),
        "validator_roster_sha256": "82" * 32,
        "pre_sign_rendered_genesis_sha256": sha256(
            artifacts["genesis.pre-sign-rendered.json"]
        ),
        "bound_genesis_manifest_sha256": sha256(artifacts["genesis.json"]),
        "signed_genesis_sha256": sha256(artifacts["genesis.signed.nrt"]),
        "genesis_expected_hash": "73" * 32,
        "genesis_public_key": PUBLIC_KEY,
        "external_signer_sha256": "64" * 32,
        "native_genesis_verifier_sha256": sha256(binaries["kagami"]),
        "operator_status_client_sha256": sha256(
            binaries["taira_operator_status"]
        ),
        "native_genesis_verifier_receipt_sha256": "55" * 32,
        "native_verifier_peer_config_set_sha256": "46" * 32,
        "local_reviewed_inputs_identity_sha256": local_identity,
    }
    linkage_sha256 = sha256(module.canonical_artifact_json(linkage))
    local_python_body = layout.local_python.read_bytes()
    configs = {
        f"taira-validator-{number}": f"{number:02x}" * 32
        for number in range(1, 5)
    }
    manifest: dict[str, object] = {
        "schema": module.RESET_SCHEMA,
        "peer_count": module.PEER_COUNT,
        "irohad_sha256": sha256(binaries["iroha3d"]),
        "genesis_native_verifier_sha256": sha256(binaries["kagami"]),
        "operator_status_client_sha256": sha256(
            binaries["taira_operator_status"]
        ),
        "genesis_external_signer_sha256": linkage["external_signer_sha256"],
        "genesis_public_key": PUBLIC_KEY,
        "genesis_expected_hash": linkage["genesis_expected_hash"],
        "pre_sign_rendered_genesis_sha256": linkage[
            "pre_sign_rendered_genesis_sha256"
        ],
        "bound_genesis_manifest_sha256": linkage[
            "bound_genesis_manifest_sha256"
        ],
        "signed_genesis_sha256": linkage["signed_genesis_sha256"],
        "native_verifier_peer_config_set_sha256": linkage[
            "native_verifier_peer_config_set_sha256"
        ],
        "genesis_artifact_linkage": linkage,
        "genesis_artifact_linkage_sha256": linkage_sha256,
        "local_reviewed_inputs_identity_sha256": local_identity,
        "local_testnet_source_closure": {
            **closure_payload,
            "sha256": closure_sha256,
        },
        "local_testnet_python": {
            "path": str(layout.local_python),
            "sha256": sha256(local_python_body),
        },
        "source_commit": source_commit,
        "dpn_validator_release_commit": dpn_commit,
        "cargo_lock_sha256": cargo_lock_sha256,
        "workspace_source_manifest_sha256": workspace_sha256,
        "configs": configs,
        "privacy_bootstrap_release": {
            "schema": module.LOCAL_RELEASE_SCHEMA,
            "reviewed_inputs": reviewed_inputs,
            "reviewed_inputs_identity_sha256": local_identity,
            "bound_genesis_manifest_sha256": linkage[
                "bound_genesis_manifest_sha256"
            ],
            "signed_genesis_sha256": linkage["signed_genesis_sha256"],
            "nevo_reset_review": {
                "schema": review_payload["schema"],
                "sha256": linkage["review_sha256"],
                "public_inputs_sha256": review_payload["public_inputs_sha256"],
                "unsigned_genesis_sha256": review_payload[
                    "unsigned_genesis_sha256"
                ],
                "public_identities": review_payload["public_identities"],
                "credential_hash_bindings": review_payload[
                    "credential_hash_bindings"
                ],
            },
            "validator_config_sha256": configs,
            "authority_claim": "none-user-authorized-same-host-testnet",
            "issuer_state": "disabled-no-broker",
            "post_genesis_issuer_enablement_required": True,
            "source": source,
        },
    }
    private_file(
        bundle / "reset-manifest.json",
        module.canonical_artifact_json(manifest),
    )
    args = module.parser().parse_args(
        [
            "--generation",
            GENERATION,
            "--bundle",
            str(bundle),
            "--release-root",
            str(release),
            "--output",
            str(output),
        ]
    )
    return layout, args, manifest, artifacts


def rewrite_manifest(args: argparse.Namespace, manifest: dict[str, object]) -> None:
    private_file(
        args.bundle / "reset-manifest.json",
        module.canonical_artifact_json(manifest),
    )


def fake_controller(path: Path, *, fail: bool = False) -> Path:
    if fail:
        body = b"import sys\nprint('refused', file=sys.stderr)\nraise SystemExit(70)\n"
    else:
        body = f'''\
import hashlib
import json
from pathlib import Path
import sys

activation_path = Path(sys.argv[sys.argv.index("--manifest") + 1])
raw = activation_path.read_bytes()
activation = json.loads(raw)
final = activation_path.parent / (activation["generation"] + ".json")
if final.exists():
    raise SystemExit("final activation was published before dry-run")
digest = hashlib.sha256(raw).hexdigest()
projection = {{
    "schema": "{module.DRY_RUN_SCHEMA}",
    "mode": "dry-run",
    "activation_manifest": str(activation_path),
    "activation_manifest_sha256": digest,
    "confirmation_required": "RESET-TAIRA-USER-501:" + digest,
    "launchctl_domain": activation["launchctl_domain"],
    "labels": activation["labels"],
    "network_id": "{NETWORK_ID}",
    "bundle": activation["bundle"],
    "binary": activation["binary"],
    "binary_sha256": activation["binary_sha256"],
    "genesis_native_verifier": activation["genesis_native_verifier"],
    "genesis_native_verifier_sha256": activation["genesis_native_verifier_sha256"],
    "genesis_external_signer_sha256": activation["genesis_external_signer_sha256"],
    "operator_status_client": activation["operator_status_client"],
    "operator_status_client_sha256": activation["operator_status_client_sha256"],
    "genesis_public_key": activation["genesis_public_key"],
    "genesis_expected_hash": activation["genesis_expected_hash"],
    "genesis_artifact_linkage_sha256": activation["genesis_artifact_linkage_sha256"],
    "archive": "/private/archive",
    "candidate_logs": "/private/logs",
    "candidate_plists": {{}},
    "predecessor": [],
    "mutated": False,
}}
print(json.dumps(projection, sort_keys=True))
'''.encode()
    return private_file(path, body)


def stage_bound_controller_closure(
    layout: module.Layout,
    args: argparse.Namespace,
    manifest: dict[str, object],
    *,
    fail: bool = False,
) -> Path:
    root = private_dir(layout.local_controllers / "20260819-nevo-reset-2")
    controller_relative = "scripts/deploy_taira_user_launchagent_reset.py"
    controller = fake_controller(root / controller_relative, fail=fail)
    rows = manifest["local_testnet_source_closure"]["files"]
    for row in rows:
        relative = row["path"]
        body = controller.read_bytes() if relative == controller_relative else source_body(relative)
        private_file(root / relative, body)
        row["sha256"] = sha256(body)
        row["size"] = len(body)
    for current, directories, _ in os.walk(root):
        Path(current).chmod(0o700)
        for directory in directories:
            (Path(current) / directory).chmod(0o700)
    closure = manifest["local_testnet_source_closure"]
    closure["sha256"] = sha256(
        module.canonical_artifact_json(
            {"schema": closure["schema"], "files": closure["files"]}
        )
    )
    rewrite_manifest(args, manifest)
    return controller


def test_renders_exact_local_schema_and_raw_confirmation(tmp_path: Path) -> None:
    layout, args, manifest, _ = fixture(tmp_path)
    receipt = module.render(args, layout=layout)

    output = args.output
    raw = output.read_bytes()
    activation = json.loads(raw)
    assert set(activation) == module.MANIFEST_KEYS
    assert len(activation) == 29
    assert raw == module.canonical_activation_json(activation)
    assert output.stat().st_mode & 0o777 == 0o600
    assert output.stat().st_nlink == 1
    assert activation["generation"] == GENERATION
    assert activation["bundle"] == str(args.bundle)
    assert activation["nevo_review_sha256"] == manifest[
        "genesis_artifact_linkage"
    ]["review_sha256"]
    assert activation["binary_sha256"] == manifest["irohad_sha256"]
    assert receipt["activation_manifest_sha256"] == sha256(raw)
    assert receipt["confirmation_required"] == (
        "RESET-TAIRA-USER-501:" + sha256(raw)
    )
    assert receipt["controller_dry_run_validated"] is False
    assert list(output.parent.glob(f".{output.name}.next-*")) == []


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        ("top_linkage", "binding is inconsistent"),
        ("production", "not the same-host local-testnet release"),
        ("closure", "source closure digest is inconsistent"),
        ("review_projection", "NEVO review projection is inconsistent"),
    ],
)
def test_rejects_inconsistent_or_production_manifest(
    tmp_path: Path,
    mutation: str,
    message: str,
) -> None:
    layout, args, manifest, _ = fixture(tmp_path)
    if mutation == "top_linkage":
        manifest["pre_sign_rendered_genesis_sha256"] = "00" * 32
    elif mutation == "production":
        manifest["privacy_bootstrap_release"]["schema"] = (
            "iroha.taira.signed_privacy_reset.v1"
        )
    elif mutation == "closure":
        manifest["local_testnet_source_closure"]["sha256"] = "00" * 32
    else:
        manifest["privacy_bootstrap_release"]["nevo_reset_review"][
            "public_inputs_sha256"
        ] = "00" * 32
    rewrite_manifest(args, manifest)

    with pytest.raises(module.ActivationRenderError, match=message):
        module.render(args, layout=layout)
    assert not args.output.exists()


def test_rejects_mutated_binary_or_bound_bundle_artifact(tmp_path: Path) -> None:
    layout, args, _, _ = fixture(tmp_path)
    (args.release_root / "kagami").write_bytes(b"replacement-kagami\n")
    (args.release_root / "kagami").chmod(0o700)
    with pytest.raises(module.ActivationRenderError, match="Kagami differs"):
        module.render(args, layout=layout)
    assert not args.output.exists()

    layout, args, _, _ = fixture(tmp_path / "artifact")
    (args.bundle / "genesis.signed.nrt").write_bytes(b"replacement-wire\n")
    (args.bundle / "genesis.signed.nrt").chmod(0o600)
    with pytest.raises(module.ActivationRenderError, match="signed genesis differs"):
        module.render(args, layout=layout)
    assert not args.output.exists()


def test_refuses_existing_output_and_concurrent_creator_without_overwrite(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    layout, args, _, _ = fixture(tmp_path / "existing")
    private_file(args.output, b"existing-activation\n")
    with pytest.raises(module.ActivationRenderError, match="already exists"):
        module.render(args, layout=layout)
    assert args.output.read_bytes() == b"existing-activation\n"

    layout, args, _, _ = fixture(tmp_path / "race")
    original_link = module.os.link

    def race_link(source: Path, destination: Path, **kwargs: object) -> None:
        private_file(Path(destination), b"concurrent-activation\n")
        raise FileExistsError(destination)

    monkeypatch.setattr(module.os, "link", race_link)
    with pytest.raises(module.ActivationRenderError, match="appeared concurrently"):
        module.render(args, layout=layout)
    monkeypatch.setattr(module.os, "link", original_link)
    assert args.output.read_bytes() == b"concurrent-activation\n"
    assert list(args.output.parent.glob(f".{args.output.name}.next-*")) == []


def test_optional_controller_dry_run_precedes_atomic_publish(tmp_path: Path) -> None:
    layout, args, manifest, _ = fixture(tmp_path)
    controller = stage_bound_controller_closure(
        layout,
        args,
        manifest,
    )
    args.validate_with_controller = controller

    receipt = module.render(args, layout=layout)

    assert receipt["controller_dry_run_validated"] is True
    assert receipt["network_id"] == NETWORK_ID
    assert args.output.exists()
    assert args.output.stat().st_mode & 0o777 == 0o600


def test_controller_refusal_publishes_no_final_or_temporary_file(
    tmp_path: Path,
) -> None:
    layout, args, manifest, _ = fixture(tmp_path)
    controller = stage_bound_controller_closure(
        layout,
        args,
        manifest,
        fail=True,
    )
    args.validate_with_controller = controller

    with pytest.raises(module.ActivationRenderError, match="dry-run failed"):
        module.render(args, layout=layout)

    assert not args.output.exists()
    assert list(args.output.parent.glob(f".{args.output.name}.next-*")) == []


def test_unbound_or_extra_controller_source_never_executes(tmp_path: Path) -> None:
    layout, args, manifest, _ = fixture(tmp_path / "changed")
    controller = stage_bound_controller_closure(layout, args, manifest)
    controller.write_bytes(controller.read_bytes() + b"\n# unreviewed replacement\n")
    controller.chmod(0o600)
    args.validate_with_controller = controller
    with pytest.raises(module.ActivationRenderError, match="source closure file differs"):
        module.render(args, layout=layout)
    assert not args.output.exists()

    layout, args, manifest, _ = fixture(tmp_path / "extra")
    controller = stage_bound_controller_closure(layout, args, manifest)
    private_file(controller.parent / "__pycache__/injected.pyc", b"unreviewed\n")
    (controller.parent / "__pycache__").chmod(0o700)
    args.validate_with_controller = controller
    with pytest.raises(module.ActivationRenderError, match="inventory is not exact"):
        module.render(args, layout=layout)
    assert not args.output.exists()


def test_reset_manifest_uses_controller_one_mebibyte_bound(tmp_path: Path) -> None:
    layout, args, _, _ = fixture(tmp_path)
    oversized = b" " * (module.MAX_RESET_MANIFEST_BYTES + 1)
    private_file(args.bundle / "reset-manifest.json", oversized)
    with pytest.raises(module.ActivationRenderError, match="bounded owner-controlled"):
        module.render(args, layout=layout)
    assert not args.output.exists()


def test_rejects_duplicate_noncanonical_reset_manifest_and_nested_output(
    tmp_path: Path,
) -> None:
    layout, args, _, _ = fixture(tmp_path / "duplicate")
    private_file(
        args.bundle / "reset-manifest.json",
        b'{"schema":"a","schema":"b"}\n',
    )
    with pytest.raises(module.ActivationRenderError, match="duplicate JSON member"):
        module.render(args, layout=layout)

    layout, args, _, _ = fixture(tmp_path / "nested")
    args.output = layout.reset_manifests / "nested" / f"{GENERATION}.json"
    with pytest.raises(module.ActivationRenderError, match="path shape is not exact"):
        module.render(args, layout=layout)


def test_isolated_help_exposes_no_digest_or_key_override() -> None:
    script = Path(module.__file__).resolve()
    completed = subprocess.run(
        [sys.executable, "-I", "-B", "-S", str(script), "--help"],
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    assert "--generation" in completed.stdout
    assert "--bundle" in completed.stdout
    assert "--release-root" in completed.stdout
    assert "--validate-with-controller" in completed.stdout
    assert "sha256" not in completed.stdout.lower()
    assert "private-key" not in completed.stdout
    assert "--apply" not in completed.stdout
