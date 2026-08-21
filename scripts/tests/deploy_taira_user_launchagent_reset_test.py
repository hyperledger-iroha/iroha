"""Focused tests for the exact user/501 Taira reset controller."""

from __future__ import annotations

import dataclasses
import hashlib
import json
from pathlib import Path
import plistlib
from types import SimpleNamespace

import pytest

from scripts import deploy_taira_user_launchagent_reset as module


OLD_NETWORK_ID = "hash:" + "A0" * 31 + "A1#0D73"
NEW_HASH = "b0" * 31 + "b1"


def private_dir(path: Path) -> Path:
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    path.chmod(0o700)
    return path


def private_file(path: Path, body: bytes, *, executable: bool = False) -> Path:
    private_dir(path.parent)
    path.write_bytes(body)
    path.chmod(0o700 if executable else 0o600)
    return path


def fixture_layout(tmp_path: Path) -> module.Layout:
    home = private_dir(tmp_path / "Users/administrator")
    taira = private_dir(home / "apps/dpn-test/taira")
    launch_agents = private_dir(home / "Library/LaunchAgents")
    return module.Layout(
        home=home,
        taira_root=taira,
        launch_agents=launch_agents,
        reset_manifests=taira / "reset-manifests",
        reset_bundles=taira / "reset-bundles",
        releases=taira / "releases",
        rollback_root=taira / "rollback/user-launchagent",
        log_root=taira / "logs/user-launchagent",
        lock_path=taira / ".user-launchagent-reset.lock",
    )


def activation_payload(
    layout: module.Layout,
    generation: str = "nevo-reset-1",
) -> dict[str, object]:
    binary = layout.releases / "candidate-release/iroha3d"
    verifier = binary.with_name("kagami")
    operator_client = binary.with_name("taira_operator_status")
    return {
        "schema": module.SCHEMA,
        "generation": generation,
        "uid": module.UID,
        "launchctl_domain": module.DOMAIN,
        "labels": list(module.LABELS),
        "bundle": str(layout.reset_bundles / generation),
        "reset_manifest_sha256": "1" * 64,
        "binary": str(binary),
        "binary_sha256": hashlib.sha256(b"candidate-binary").hexdigest(),
        "genesis_native_verifier": str(verifier),
        "genesis_native_verifier_sha256": hashlib.sha256(
            b"candidate-native-verifier"
        ).hexdigest(),
        "operator_status_client": str(operator_client),
        "operator_status_client_sha256": hashlib.sha256(
            b"candidate-operator-status-client"
        ).hexdigest(),
        "genesis_external_signer_sha256": "4" * 64,
        "genesis_public_key": (
            "ed0120403BA31890B09C40B7108A0AC1319D27C10FE5442027CF8333C5C3A09CBB0343"
        ),
        "genesis_expected_hash": NEW_HASH,
        "genesis_artifact_linkage_sha256": "5" * 64,
        "nevo_review_sha256": "6" * 64,
        "reviewed_unsigned_genesis_sha256": "7" * 64,
        "pre_sign_rendered_genesis_sha256": "a" * 64,
        "native_verifier_peer_config_set_sha256": "b" * 64,
        "bound_genesis_manifest_sha256": "8" * 64,
        "signed_genesis_sha256": "9" * 64,
        "local_reviewed_inputs_identity_sha256": "c" * 64,
        "local_testnet_source_closure_sha256": "d" * 64,
        "local_testnet_python_sha256": module.stable_hash_path(
            module.LOCAL_TESTNET_PYTHON.resolve(strict=True)
        ).sha256,
        "source_commit": "2" * 40,
        "dpn_validator_release_commit": "3" * 40,
        "limits": {
            "minimum_free_bytes": module.MIN_FREE_BYTES,
            "maximum_fsync_latency_ms": 500,
            "startup_timeout_seconds": 60,
            "stability_timeout_seconds": 10,
            "poll_interval_seconds": 1,
        },
    }


def write_activation(
    layout: module.Layout,
    payload: dict[str, object] | None = None,
) -> module.Activation:
    payload = payload or activation_payload(layout)
    path = layout.reset_manifests / "nevo-reset-1.json"
    private_file(path, module.canonical_json(payload))
    return module.load_activation(path, layout)


def old_plist_body(
    label: str,
    binary: Path,
    workdir: Path,
) -> bytes:
    return plistlib.dumps(
        {
            "Label": label,
            "ProgramArguments": [
                str(binary),
                "--sora",
                "--config",
                str(workdir / "config.toml"),
            ],
            "WorkingDirectory": str(workdir),
            "KeepAlive": True,
            "ProcessType": "Standard",
            "ThrottleInterval": 30,
            "StandardOutPath": str(workdir / "logs/iroha3d.stdout.log"),
            "StandardErrorPath": str(workdir / "logs/iroha3d.stderr.log"),
        },
        fmt=plistlib.FMT_XML,
        sort_keys=True,
    )


def build_fixture(tmp_path: Path) -> tuple[module.Layout, module.Activation, SimpleNamespace]:
    layout = fixture_layout(tmp_path)
    payload = activation_payload(layout)
    candidate_binary = Path(str(payload["binary"]))
    private_file(candidate_binary, b"candidate-binary", executable=True)
    candidate_verifier = Path(str(payload["genesis_native_verifier"]))
    private_file(candidate_verifier, b"candidate-native-verifier", executable=True)
    candidate_operator_client = Path(str(payload["operator_status_client"]))
    private_file(
        candidate_operator_client,
        b"candidate-operator-status-client",
        executable=True,
    )
    bundle = Path(str(payload["bundle"]))
    private_dir(bundle)
    private_file(bundle / "genesis.json", b'{"domain":"nevo.dpn"}\n')
    private_file(bundle / "genesis.signed.nrt", b"signed-nevo-genesis")
    private_file(bundle / "base-config.toml", b"chain = 'taira'\n")
    private_file(bundle / "reset-manifest.json", b'{"schema":"fixture"}\n')
    private_file(bundle / "validator-roster.toml", b"[[validators]]\n")
    private_file(bundle / "rendered/genesis.json", b'{"domain":"nevo.dpn"}\n')

    candidate_peers: list[SimpleNamespace] = []
    for index, (slug, port) in enumerate(zip(module.SLUGS, module.TORII_PORTS), start=1):
        workdir = private_dir(bundle / "rendered" / slug)
        config = private_file(workdir / "config.toml", b"chain = 'taira'\n")
        storage = private_dir(workdir / "storage")
        candidate_peers.append(
            SimpleNamespace(
                slug=slug,
                torii_port=port,
                workdir=workdir,
                config=config,
                config_sha256=hashlib.sha256(config.read_bytes()).hexdigest(),
                storage=storage,
            )
        )

    old_release = private_dir(layout.releases / "old-release")
    old_binary = private_file(old_release / "iroha3d", b"old-binary", executable=True)
    old_genesis = private_file(old_release / "genesis.signed.nrt", b"old-genesis")
    for index, (label, slug, port) in enumerate(
        zip(module.LABELS, module.SLUGS, module.TORII_PORTS),
        start=1,
    ):
        workdir = private_dir(old_release / slug)
        private_dir(workdir / "logs")
        private_dir(workdir / "storage")
        config = f'''[torii]
address = "addr:127.0.0.1:{port}#ABCD"

[genesis]
file = "{old_genesis}"
expected_hash = "{OLD_NETWORK_ID}"
'''.encode()
        private_file(workdir / "config.toml", config)
        private_file(
            layout.launch_agents / f"{label}.plist",
            old_plist_body(label, old_binary, workdir),
        )

    activation = write_activation(layout, payload)
    bundle_plan = SimpleNamespace(
        root=bundle,
        owner_uid=module.UID,
        peers=tuple(candidate_peers),
        manifest={"genesis_expected_hash": NEW_HASH},
    )
    return layout, activation, bundle_plan


class FakeLaunchctl:
    def __init__(self) -> None:
        self.checked: list[str] = []

    def require_initial_cohort(self) -> None:
        self.checked.append("cohort")

    def require_loaded_definition(self, path: Path, body: bytes) -> None:
        del body
        self.checked.append(path.name)


def build_plan_fixture(
    tmp_path: Path,
) -> tuple[module.Layout, module.ResetPlan, dict[str, object], FakeLaunchctl]:
    layout, activation, bundle_plan = build_fixture(tmp_path)
    captured: dict[str, object] = {}

    def validate_bundle(path: Path, **kwargs: object) -> SimpleNamespace:
        captured["path"] = path
        captured.update(kwargs)
        return bundle_plan

    launchctl = FakeLaunchctl()
    plan = module.build_plan(
        activation,
        layout=layout,
        launchctl=launchctl,  # type: ignore[arg-type]
        validate_bundle_fn=validate_bundle,
        validate_genesis_integrity_fn=lambda _activation, _manifest: ({}, {}),
        run_native_genesis_verifier_fn=lambda _activation, _receipt: (
            module.metadata_identity(activation.genesis_native_verifier.lstat())
        ),
    )
    return layout, plan, captured, launchctl


def test_parser_is_dry_run_by_default() -> None:
    args = module.parser().parse_args(["--manifest", "/tmp/reset.json"])

    assert args.apply is False
    assert args.confirm_reset is None


def test_readme_local_lane_is_isolated_direct_four_file_and_per_peer() -> None:
    readme = (
        module.nevo_composer.REPO_ROOT / "configs/soranexus/taira/README.md"
    ).read_text(encoding="utf-8")
    section = readme.split("### Existing macOS user-LaunchAgent cohort", 1)[1]
    section = section.split("The live bundle is never composed", 1)[0]

    assert "/opt/homebrew/bin/python3" in section
    assert section.count("-I -B -S") >= 4
    assert "exactly four mode-0600" in section
    assert "--local-testnet-reviewed-inputs-sha256" in section
    assert "--local-testnet-source-closure-sha256" in section
    assert "--local-testnet-python-sha256" in section
    assert '"operator_status_client"' in section
    assert '"operator_status_client_sha256"' in section
    assert "--operator-status-client" in section
    assert "--trusted-operator-status-client-sha256" in section
    assert "--controller-manifest" not in section
    assert "sudo " not in section
    assert section.count("\n  --operator-private-key-file ") == module.PEER_COUNT
    assert (
        section.count("\n  --rollback-operator-private-key-file ")
        == module.PEER_COUNT
    )


def test_manifest_requires_exact_user_cohort_and_refuses_system_label(tmp_path: Path) -> None:
    layout = fixture_layout(tmp_path)
    payload = activation_payload(layout)
    payload["labels"] = [*module.SYSTEM_LABELS]
    path = layout.reset_manifests / "nevo-reset-1.json"
    private_file(path, module.canonical_json(payload))

    with pytest.raises(module.ResetError, match="exact user/501"):
        module.load_activation(path, layout)


def test_manifest_refuses_broad_candidate_path(tmp_path: Path) -> None:
    layout = fixture_layout(tmp_path)
    payload = activation_payload(layout)
    payload["bundle"] = str(layout.reset_bundles)
    path = layout.reset_manifests / "nevo-reset-1.json"
    private_file(path, module.canonical_json(payload))

    with pytest.raises(module.ResetError, match="too broad"):
        module.load_activation(path, layout)


def test_manifest_refuses_symlinked_candidate_ancestry(tmp_path: Path) -> None:
    layout = fixture_layout(tmp_path)
    private_dir(layout.reset_bundles)
    target = private_dir(layout.taira_root / "actual-bundle")
    (layout.reset_bundles / "nevo-reset-1").symlink_to(target, target_is_directory=True)
    payload = activation_payload(layout)
    path = layout.reset_manifests / "nevo-reset-1.json"
    private_file(path, module.canonical_json(payload))

    with pytest.raises(module.ResetError, match="contains a symlink"):
        module.load_activation(path, layout)


def test_build_plan_binds_existing_reset_manifest_and_exact_four_plists(tmp_path: Path) -> None:
    layout, plan, captured, launchctl = build_plan_fixture(tmp_path)

    assert captured["headroom_anchor"] == layout.taira_root
    assert captured["expected_reset_manifest_sha256"] == "1" * 64
    assert captured["expected_binary_sha256"] == hashlib.sha256(b"candidate-binary").hexdigest()
    assert tuple(peer.label for peer in plan.candidate) == module.LABELS
    assert tuple(peer.torii_port for peer in plan.candidate) == module.TORII_PORTS
    assert len({peer.plist_sha256 for peer in plan.candidate}) == module.PEER_COUNT
    for peer in plan.candidate:
        payload = plistlib.loads(peer.plist_body)
        assert payload["Label"] == peer.label
        assert payload["ProgramArguments"] == [
            str(plan.activation.binary),
            "--sora",
            "--config",
            str(peer.config),
        ]
        assert payload["WorkingDirectory"] == str(peer.workdir)
        assert payload["StandardOutPath"].startswith(str(plan.log_dir) + "/")
    assert launchctl.checked[0] == "cohort"
    assert set(launchctl.checked[1:]) == {
        f"{label}.plist" for label in module.LABELS
    }


def test_build_plan_rejects_retired_namespace_in_any_public_projection(tmp_path: Path) -> None:
    layout, activation, bundle_plan = build_fixture(tmp_path)
    retired = bytes.fromhex("776f6e6465726c616e64")
    private_file(bundle_plan.peers[2].config, b"domain = '" + retired + b"'\n")

    with pytest.raises(module.ResetError, match="retired namespace"):
        module.build_plan(
            activation,
            layout=layout,
            validate_bundle_fn=lambda *_args, **_kwargs: bundle_plan,
            validate_genesis_integrity_fn=lambda _activation, _manifest: ({}, {}),
            run_native_genesis_verifier_fn=lambda _activation, _receipt: (
                module.metadata_identity(activation.genesis_native_verifier.lstat())
            ),
        )


def test_controller_sources_contain_no_retired_namespace_literal() -> None:
    retired = bytes.fromhex("776f6e6465726c616e64")
    paths = (Path(module.__file__), Path(__file__))

    for path in paths:
        assert retired not in path.read_bytes().lower()


def test_build_plan_rejects_missing_nevo_genesis(tmp_path: Path) -> None:
    layout, activation, bundle_plan = build_fixture(tmp_path)
    private_file(bundle_plan.root / "genesis.json", b'{"domain":"example"}\n')

    with pytest.raises(module.ResetError, match="does not provision nevo.dpn"):
        module.build_plan(
            activation,
            layout=layout,
            validate_bundle_fn=lambda *_args, **_kwargs: bundle_plan,
            validate_genesis_integrity_fn=lambda _activation, _manifest: ({}, {}),
            run_native_genesis_verifier_fn=lambda _activation, _receipt: (
                module.metadata_identity(activation.genesis_native_verifier.lstat())
            ),
        )


def test_build_plan_requires_four_fresh_candidate_storage_directories(tmp_path: Path) -> None:
    layout, activation, bundle_plan = build_fixture(tmp_path)
    private_file(bundle_plan.peers[1].storage / "block", b"not fresh")

    with pytest.raises(module.ResetError, match="distinct fresh bundle"):
        module.build_plan(
            activation,
            layout=layout,
            validate_bundle_fn=lambda *_args, **_kwargs: bundle_plan,
            validate_genesis_integrity_fn=lambda _activation, _manifest: ({}, {}),
            run_native_genesis_verifier_fn=lambda _activation, _receipt: (
                module.metadata_identity(activation.genesis_native_verifier.lstat())
            ),
        )


def test_genesis_integrity_binds_review_signed_verifier_and_all_peer_artifacts(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _layout, activation, bundle_plan = build_fixture(tmp_path)
    bundle = bundle_plan.root
    reviewed = b'{"domain":"nevo.dpn","reviewed":true}\n'
    pre_sign = b'{"domain":"nevo.dpn","rendered":true}\n'
    bound = b'{"bound":true,"domain":"nevo.dpn"}\n'
    signed = b"exact-signed-nevo-genesis"
    review = module.canonical_json(
        {"unsigned_genesis_sha256": hashlib.sha256(reviewed).hexdigest()}
    )
    private_file(bundle / "genesis.reviewed-unsigned.json", reviewed)
    private_file(bundle / "genesis.pre-sign-rendered.json", pre_sign)
    private_file(bundle / "nevo-reset.review.json", review)
    private_file(bundle / "genesis.json", bound)
    private_file(bundle / "genesis.signed.nrt", signed)
    inventory = {
        peer.slug: {
            "directories": [],
            "files": {
                "config.toml": hashlib.sha256(peer.config.read_bytes()).hexdigest()
            },
        }
        for peer in bundle_plan.peers
    }
    peer_config_sha256 = [
        hashlib.sha256(peer.config.read_bytes()).hexdigest()
        for peer in bundle_plan.peers
    ]
    peer_config_set_sha256 = module._ordered_sha256_set(peer_config_sha256)
    validator_roster_sha256 = hashlib.sha256(
        (bundle / "validator-roster.toml").read_bytes()
    ).hexdigest()
    activation = dataclasses.replace(
        activation,
        nevo_review_sha256=hashlib.sha256(review).hexdigest(),
        reviewed_unsigned_genesis_sha256=hashlib.sha256(reviewed).hexdigest(),
        pre_sign_rendered_genesis_sha256=hashlib.sha256(pre_sign).hexdigest(),
        native_verifier_peer_config_set_sha256=peer_config_set_sha256,
        bound_genesis_manifest_sha256=hashlib.sha256(bound).hexdigest(),
        signed_genesis_sha256=hashlib.sha256(signed).hexdigest(),
        privacy_native_verifier_sha256="c" * 64,
        local_reviewed_inputs_identity_sha256=None,
        local_testnet_source_closure_sha256=None,
        local_testnet_python_sha256=None,
    )
    receipt = {
        "schema": "iroha.kagami.prepared-genesis-verification.v2",
        "status": "verified",
        "reviewed_manifest_sha256": activation.reviewed_unsigned_genesis_sha256,
        "validator_roster_sha256": validator_roster_sha256,
        "bound_manifest_sha256": activation.bound_genesis_manifest_sha256,
        "pre_sign_manifest_sha256": activation.pre_sign_rendered_genesis_sha256,
        "signed_genesis_sha256": activation.signed_genesis_sha256,
        "peer_config_sha256": peer_config_sha256,
        "peer_config_set_sha256": activation.native_verifier_peer_config_set_sha256,
        "genesis_public_key": activation.genesis_public_key,
        "expected_hash": activation.genesis_expected_hash,
        "validator_count": module.PEER_COUNT,
        "reviewed_transform_passed": True,
        "allowed_transform_passed": True,
        "staged_context_passed": True,
        "full_core_validation_passed": True,
    }
    receipt_sha = hashlib.sha256(module._artifact_canonical_json(receipt)).hexdigest()
    linkage = {
        "schema": "iroha.taira.nevo-genesis-artifact-linkage.v1",
        "review_sha256": activation.nevo_review_sha256,
        "reviewed_unsigned_genesis_sha256": (
            activation.reviewed_unsigned_genesis_sha256
        ),
        "validator_roster_sha256": validator_roster_sha256,
        "pre_sign_rendered_genesis_sha256": (
            activation.pre_sign_rendered_genesis_sha256
        ),
        "bound_genesis_manifest_sha256": (
            activation.bound_genesis_manifest_sha256
        ),
        "signed_genesis_sha256": activation.signed_genesis_sha256,
        "genesis_expected_hash": activation.genesis_expected_hash,
        "genesis_public_key": activation.genesis_public_key,
        "external_signer_sha256": activation.genesis_external_signer_sha256,
        "native_genesis_verifier_sha256": (
            activation.genesis_native_verifier_sha256
        ),
        "operator_status_client_sha256": activation.operator_status_client_sha256,
        "native_genesis_verifier_receipt_sha256": receipt_sha,
        "native_verifier_peer_config_set_sha256": (
            activation.native_verifier_peer_config_set_sha256
        ),
        "privacy_native_verifier_sha256": activation.privacy_native_verifier_sha256,
    }
    linkage_sha = hashlib.sha256(module._artifact_canonical_json(linkage)).hexdigest()
    activation = dataclasses.replace(
        activation,
        genesis_artifact_linkage_sha256=linkage_sha,
    )
    manifest = {
        **{
            field: linkage[field]
            for field in (
                "genesis_expected_hash",
                "genesis_public_key",
                "external_signer_sha256",
                "native_genesis_verifier_sha256",
            )
        },
        "genesis_artifact_linkage": linkage,
        "genesis_artifact_linkage_sha256": linkage_sha,
        "genesis_native_verifier_receipt": receipt,
        "genesis_native_verifier_receipt_sha256": receipt_sha,
        "native_verifier_peer_config_set_sha256": (
            activation.native_verifier_peer_config_set_sha256
        ),
        "validator_roster_sha256": validator_roster_sha256,
        "configs": {
            peer.slug: digest
            for peer, digest in zip(bundle_plan.peers, peer_config_sha256)
        },
        "validator_artifact_inventory": inventory,
        "privacy_bootstrap_release": {
            "schema": "iroha.taira.signed_privacy_reset.v1"
        },
        "privacy_native_verifier_sha256": activation.privacy_native_verifier_sha256,
        "operator_status_client_sha256": activation.operator_status_client_sha256,
        "release_controller": {
            "digest": "e" * 64,
            "manifest_sha256": "f" * 64,
            "platform": "macos",
        },
    }
    manifest["genesis_external_signer_sha256"] = manifest.pop(
        "external_signer_sha256"
    )
    manifest["genesis_native_verifier_sha256"] = manifest.pop(
        "native_genesis_verifier_sha256"
    )
    monkeypatch.setattr(
        module.nevo_composer,
        "verify_reviewed_payloads",
        lambda **_kwargs: {},
    )

    accepted_receipt, accepted_inventory = module._require_nevo_genesis_integrity(
        activation,
        manifest,
    )
    assert accepted_receipt == receipt
    assert accepted_inventory == inventory

    captured: list[str] = []

    def fake_run(command, **_kwargs):  # type: ignore[no-untyped-def]
        captured.extend(command)
        return SimpleNamespace(
            returncode=0,
            stdout=module.canonical_json(receipt),
            stderr=b"",
        )

    monkeypatch.setattr(module.subprocess, "run", fake_run)
    module._run_native_genesis_verifier(activation, receipt)
    assert captured.count("--peer-config") == module.PEER_COUNT
    assert captured[captured.index("--reviewed-manifest") + 1] == str(
        bundle / "genesis.reviewed-unsigned.json"
    )
    assert captured[captured.index("--validator-roster") + 1] == str(
        bundle / "validator-roster.toml"
    )
    assert [
        captured[index + 1]
        for index, value in enumerate(captured)
        if value == "--peer-config"
    ] == [str(peer.config) for peer in bundle_plan.peers]

    private_file(bundle_plan.peers[1].config, b"mutated-config")
    with pytest.raises(module.ResetError, match="peer config digest changed"):
        module._require_nevo_genesis_integrity(activation, manifest)


def _local_genesis_integrity_fixture(
    tmp_path: Path,
) -> tuple[module.Activation, dict[str, object]]:
    """Build one real, broker-free local review projection from checked-in bytes."""

    _layout, activation, bundle_plan = build_fixture(tmp_path)
    bundle = bundle_plan.root
    fixture = (
        module.nevo_composer.REPO_ROOT
        / "crates/iroha_kagami/tests/fixtures/taira_nevo_v2"
    )
    reviewed = (fixture / "unsigned-genesis.json").read_bytes()
    review_raw = (fixture / "review.json").read_bytes()
    config_raw = (
        module.nevo_composer.REPO_ROOT / "configs/soranexus/taira/config.toml"
    ).read_bytes()
    plan_raw = (
        module.nevo_composer.REPO_ROOT
        / "configs/soranexus/taira/privacy_bootstrap_plan.json"
    ).read_bytes()
    verified_review = module.nevo_composer.verify_reviewed_payloads(
        unsigned_genesis_bytes=reviewed,
        review_bytes=review_raw,
        base_genesis_bytes=module.nevo_composer.CHECKED_IN_TAIRA_GENESIS.read_bytes(),
        base_config_bytes=config_raw,
    )
    pre_sign = b'{"rendered":true}\n'
    bound = b'{"bound":true}\n'
    signed = b"signed-local-nevo-genesis"
    private_file(bundle / "genesis.reviewed-unsigned.json", reviewed)
    private_file(bundle / "genesis.pre-sign-rendered.json", pre_sign)
    private_file(bundle / "nevo-reset.review.json", review_raw)
    private_file(bundle / "genesis.json", bound)
    private_file(bundle / "genesis.signed.nrt", signed)
    private_file(bundle / "base-config.toml", config_raw)
    for peer in bundle_plan.peers:
        private_file(peer.config, config_raw)

    closure_manifest, closure_sha256 = module.local_testnet_source_closure()
    closure = {**closure_manifest, "sha256": closure_sha256}
    peer_config_sha256 = [
        hashlib.sha256(peer.config.read_bytes()).hexdigest()
        for peer in bundle_plan.peers
    ]
    config_manifest = {
        peer.slug: digest
        for peer, digest in zip(bundle_plan.peers, peer_config_sha256)
    }
    config_set_sha256 = module._ordered_sha256_set(peer_config_sha256)
    reviewed_inputs_raw = {
        "privacy_bootstrap_plan.json": plan_raw,
        "config.toml": config_raw,
        "genesis.json": reviewed,
        "nevo-reset.review.json": review_raw,
    }
    reviewed_inputs = {
        name: {"sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}
        for name, raw in reviewed_inputs_raw.items()
    }
    cargo_lock_sha256 = "a" * 64
    workspace_sha256 = "b" * 64
    source = {
        "commit": activation.source_commit,
        "dpn_validator_release_commit": activation.dpn_validator_release_commit,
        "cargo_lock_sha256": cargo_lock_sha256,
        "workspace_source_manifest_sha256": workspace_sha256,
    }
    identity_manifest = {
        "schema": "iroha.taira.local_testnet_reviewed_inputs.v1",
        "authority_claim": "none-user-authorized-same-host-testnet",
        "source": source,
        "privacy_inputs": reviewed_inputs,
    }
    reviewed_identity = hashlib.sha256(
        module._artifact_canonical_json(identity_manifest)
    ).hexdigest()
    activation = dataclasses.replace(
        activation,
        nevo_review_sha256=hashlib.sha256(review_raw).hexdigest(),
        reviewed_unsigned_genesis_sha256=hashlib.sha256(reviewed).hexdigest(),
        pre_sign_rendered_genesis_sha256=hashlib.sha256(pre_sign).hexdigest(),
        native_verifier_peer_config_set_sha256=config_set_sha256,
        bound_genesis_manifest_sha256=hashlib.sha256(bound).hexdigest(),
        signed_genesis_sha256=hashlib.sha256(signed).hexdigest(),
        privacy_native_verifier_sha256=None,
        local_reviewed_inputs_identity_sha256=reviewed_identity,
        local_testnet_source_closure_sha256=closure_sha256,
    )
    roster_sha256 = hashlib.sha256(
        (bundle / "validator-roster.toml").read_bytes()
    ).hexdigest()
    receipt = {
        "schema": "iroha.kagami.prepared-genesis-verification.v2",
        "status": "verified",
        "reviewed_manifest_sha256": activation.reviewed_unsigned_genesis_sha256,
        "validator_roster_sha256": roster_sha256,
        "bound_manifest_sha256": activation.bound_genesis_manifest_sha256,
        "pre_sign_manifest_sha256": activation.pre_sign_rendered_genesis_sha256,
        "signed_genesis_sha256": activation.signed_genesis_sha256,
        "peer_config_sha256": peer_config_sha256,
        "peer_config_set_sha256": config_set_sha256,
        "genesis_public_key": activation.genesis_public_key,
        "expected_hash": activation.genesis_expected_hash,
        "validator_count": module.PEER_COUNT,
        "reviewed_transform_passed": True,
        "allowed_transform_passed": True,
        "staged_context_passed": True,
        "full_core_validation_passed": True,
    }
    receipt_sha256 = hashlib.sha256(
        module._artifact_canonical_json(receipt)
    ).hexdigest()
    linkage = {
        "schema": "iroha.taira.nevo-genesis-artifact-linkage.v1",
        "review_sha256": activation.nevo_review_sha256,
        "reviewed_unsigned_genesis_sha256": activation.reviewed_unsigned_genesis_sha256,
        "validator_roster_sha256": roster_sha256,
        "pre_sign_rendered_genesis_sha256": activation.pre_sign_rendered_genesis_sha256,
        "bound_genesis_manifest_sha256": activation.bound_genesis_manifest_sha256,
        "signed_genesis_sha256": activation.signed_genesis_sha256,
        "genesis_expected_hash": activation.genesis_expected_hash,
        "genesis_public_key": activation.genesis_public_key,
        "external_signer_sha256": activation.genesis_external_signer_sha256,
        "native_genesis_verifier_sha256": activation.genesis_native_verifier_sha256,
        "operator_status_client_sha256": activation.operator_status_client_sha256,
        "native_genesis_verifier_receipt_sha256": receipt_sha256,
        "native_verifier_peer_config_set_sha256": config_set_sha256,
        "local_reviewed_inputs_identity_sha256": reviewed_identity,
    }
    linkage_sha256 = hashlib.sha256(
        module._artifact_canonical_json(linkage)
    ).hexdigest()
    activation = dataclasses.replace(
        activation,
        genesis_artifact_linkage_sha256=linkage_sha256,
    )
    inventory = {
        peer.slug: {
            "directories": [],
            "files": {"config.toml": digest},
        }
        for peer, digest in zip(bundle_plan.peers, peer_config_sha256)
    }
    review_projection = {
        "schema": verified_review["schema"],
        "sha256": reviewed_inputs["nevo-reset.review.json"]["sha256"],
        "public_inputs_sha256": verified_review["public_inputs_sha256"],
        "unsigned_genesis_sha256": verified_review["unsigned_genesis_sha256"],
        "public_identities": verified_review["public_identities"],
        "credential_hash_bindings": verified_review["credential_hash_bindings"],
    }
    privacy_release = {
        "schema": "iroha.taira.local_testnet_reviewed_reset.v1",
        "reviewed_inputs": reviewed_inputs,
        "bound_genesis_manifest_sha256": activation.bound_genesis_manifest_sha256,
        "signed_genesis_sha256": activation.signed_genesis_sha256,
        "validator_config_sha256": config_manifest,
        "nevo_reset_review": review_projection,
        "authority_claim": "none-user-authorized-same-host-testnet",
        "issuer_state": "disabled-no-broker",
        "post_genesis_issuer_enablement_required": True,
        "reviewed_inputs_identity_sha256": reviewed_identity,
        "source": source,
    }
    manifest: dict[str, object] = {
        "cargo_lock_sha256": cargo_lock_sha256,
        "workspace_source_manifest_sha256": workspace_sha256,
        "genesis_expected_hash": activation.genesis_expected_hash,
        "genesis_public_key": activation.genesis_public_key,
        "genesis_external_signer_sha256": activation.genesis_external_signer_sha256,
        "genesis_native_verifier_sha256": activation.genesis_native_verifier_sha256,
        "operator_status_client_sha256": activation.operator_status_client_sha256,
        "genesis_artifact_linkage": linkage,
        "genesis_artifact_linkage_sha256": linkage_sha256,
        "genesis_native_verifier_receipt": receipt,
        "genesis_native_verifier_receipt_sha256": receipt_sha256,
        "native_verifier_peer_config_set_sha256": config_set_sha256,
        "validator_roster_sha256": roster_sha256,
        "configs": config_manifest,
        "validator_artifact_inventory": inventory,
        "privacy_bootstrap_release": privacy_release,
        "local_reviewed_inputs_identity_sha256": reviewed_identity,
        "local_testnet_source_closure": closure,
        "local_testnet_python": {
            "path": str(module.LOCAL_TESTNET_PYTHON),
            "sha256": activation.local_testnet_python_sha256,
        },
    }
    return activation, manifest


def test_local_genesis_integrity_accepts_exact_broker_free_reviewed_release(
    tmp_path: Path,
) -> None:
    activation, manifest = _local_genesis_integrity_fixture(tmp_path)

    receipt, inventory = module._require_nevo_genesis_integrity(
        activation, manifest
    )

    assert receipt["status"] == "verified"
    assert set(inventory) == set(module.SLUGS)


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (lambda release: release.update(authority_claim="claimed"), "authority or issuer"),
        (lambda release: release.update(issuer_state="enabled"), "authority or issuer"),
        (
            lambda release: release["reviewed_inputs"].update(
                {"bootle_lantern_broker_public.json": {"sha256": "0" * 64, "size": 1}}
            ),
            "exactly four files",
        ),
        (
            lambda release: release["reviewed_inputs"]["config.toml"].update(
                sha256="0" * 64
            ),
            "reviewed input changed",
        ),
    ),
)
def test_local_genesis_integrity_rejects_authority_issuer_broker_and_row_drift(
    tmp_path: Path,
    mutation,
    message: str,
) -> None:
    activation, manifest = _local_genesis_integrity_fixture(tmp_path)
    release = manifest["privacy_bootstrap_release"]
    assert isinstance(release, dict)
    mutation(release)

    with pytest.raises(module.ResetError, match=message):
        module._require_nevo_genesis_integrity(activation, manifest)


def test_local_genesis_integrity_rejects_identity_controller_and_enabled_peer(
    tmp_path: Path,
) -> None:
    activation, manifest = _local_genesis_integrity_fixture(tmp_path / "identity")
    release = manifest["privacy_bootstrap_release"]
    assert isinstance(release, dict)
    release["reviewed_inputs_identity_sha256"] = "0" * 64
    with pytest.raises(module.ResetError, match="identity differs"):
        module._require_nevo_genesis_integrity(activation, manifest)

    activation, manifest = _local_genesis_integrity_fixture(tmp_path / "controller")
    manifest["release_controller"] = {
        "digest": "0" * 64,
        "manifest_sha256": "1" * 64,
        "platform": "macos",
    }
    with pytest.raises(module.ResetError, match="production verifier authority"):
        module._require_nevo_genesis_integrity(activation, manifest)

    activation, manifest = _local_genesis_integrity_fixture(tmp_path / "issuer")
    peer_config = activation.bundle / "rendered" / module.SLUGS[2] / "config.toml"
    private_file(
        peer_config,
        peer_config.read_bytes().replace(
            b"[torii.privacy_bootle_lantern_issuer]\nenabled = false",
            b"[torii.privacy_bootle_lantern_issuer]\nenabled = true",
            1,
        ),
    )
    peer_digest = hashlib.sha256(peer_config.read_bytes()).hexdigest()
    config_manifest = manifest["configs"]
    assert isinstance(config_manifest, dict)
    config_manifest[module.SLUGS[2]] = peer_digest
    release = manifest["privacy_bootstrap_release"]
    assert isinstance(release, dict)
    release["validator_config_sha256"] = dict(config_manifest)
    inventory = manifest["validator_artifact_inventory"]
    assert isinstance(inventory, dict)
    inventory[module.SLUGS[2]]["files"]["config.toml"] = peer_digest
    config_hashes = [config_manifest[slug] for slug in module.SLUGS]
    config_set = module._ordered_sha256_set(config_hashes)
    manifest["native_verifier_peer_config_set_sha256"] = config_set
    linkage = manifest["genesis_artifact_linkage"]
    assert isinstance(linkage, dict)
    linkage["native_verifier_peer_config_set_sha256"] = config_set
    receipt = manifest["genesis_native_verifier_receipt"]
    assert isinstance(receipt, dict)
    receipt["peer_config_sha256"] = config_hashes
    receipt["peer_config_set_sha256"] = config_set
    receipt_sha = hashlib.sha256(module._artifact_canonical_json(receipt)).hexdigest()
    manifest["genesis_native_verifier_receipt_sha256"] = receipt_sha
    linkage["native_genesis_verifier_receipt_sha256"] = receipt_sha
    linkage_sha = hashlib.sha256(module._artifact_canonical_json(linkage)).hexdigest()
    manifest["genesis_artifact_linkage_sha256"] = linkage_sha
    activation = dataclasses.replace(
        activation,
        native_verifier_peer_config_set_sha256=config_set,
        genesis_artifact_linkage_sha256=linkage_sha,
    )
    with pytest.raises(module.ResetError, match="issuance exactly disabled"):
        module._require_nevo_genesis_integrity(activation, manifest)


def test_candidate_artifact_walk_rejects_nested_storage_and_symlink_directories(
    tmp_path: Path,
) -> None:
    root = private_dir(tmp_path / "peer")
    private_dir(root / "storage")
    runtime = private_dir(root / "runtime")
    private_dir(runtime / "storage")
    with pytest.raises(module.ResetError, match="nested storage"):
        module._candidate_artifact_paths(root)

    (runtime / "storage").rmdir()
    target = private_dir(tmp_path / "target")
    (runtime / "linked").symlink_to(target, target_is_directory=True)
    with pytest.raises(module.ResetError, match="directory is unsafe"):
        module._candidate_artifact_paths(root)


def test_predecessor_refuses_symlinked_storage(tmp_path: Path) -> None:
    layout, _activation, _bundle_plan = build_fixture(tmp_path)
    workdir = layout.releases / "old-release/taira-validator-1"
    storage = workdir / "storage"
    storage.rmdir()
    storage.symlink_to(private_dir(layout.taira_root / "other-storage"), target_is_directory=True)

    with pytest.raises(module.ResetError, match="contains a symlink"):
        module.parse_old_plist(
            1,
            layout.launch_agents / f"{module.LABELS[0]}.plist",
            layout,
        )


def test_predecessor_cohort_rejects_mixed_genesis(tmp_path: Path) -> None:
    layout, _activation, _bundle_plan = build_fixture(tmp_path)
    peers = [
        module.parse_old_plist(
            number,
            layout.launch_agents / f"{label}.plist",
            layout,
        )
        for number, label in enumerate(module.LABELS, start=1)
    ]
    peers[3] = SimpleNamespace(
        **{
            **peers[3].__dict__,
            "network_id": "hash:" + "C0" * 31 + "C1#0000",
        }
    )

    with pytest.raises(module.ResetError, match="mixes binary or genesis"):
        module.require_coherent_predecessor(peers)  # type: ignore[arg-type]


def test_predecessor_storage_identity_is_retained_for_rollback(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)
    storage = plan.predecessor[2].storage
    moved = storage.with_name("storage.old")
    storage.rename(moved)
    private_dir(storage)

    with pytest.raises(module.ResetError, match="storage identity changed"):
        module.require_predecessor_artifacts_unchanged(plan.predecessor)


def launchctl_result(returncode: int, body: str = "") -> SimpleNamespace:
    return SimpleNamespace(returncode=returncode, stdout=body.encode(), stderr=b"")


def test_launchctl_refuses_loaded_dormant_system_label(monkeypatch: pytest.MonkeyPatch) -> None:
    ops = module.LaunchctlOps()
    domain_body = "\n".join(module.LABELS)

    def run(arguments: list[str], *, check: bool) -> SimpleNamespace:
        del check
        target = arguments[-1]
        if target == module.DOMAIN:
            return launchctl_result(0, domain_body)
        if target == f"system/{module.SYSTEM_LABELS[0]}":
            return launchctl_result(0, "loaded")
        if target.startswith("system/"):
            return launchctl_result(113)
        return launchctl_result(0, "loaded")

    monkeypatch.setattr(ops, "run", run)

    with pytest.raises(module.ResetError, match="dormant system validator label"):
        ops.require_initial_cohort()


def test_launchctl_refuses_loaded_argument_reordering(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    layout, _activation, _bundle_plan = build_fixture(tmp_path)
    label = module.LABELS[0]
    path = layout.launch_agents / f"{label}.plist"
    body = path.read_bytes()
    payload = plistlib.loads(body)
    arguments = payload["ProgramArguments"]
    output = "\n".join(
        [
            f"{module.DOMAIN}/{label} = {{",
            f"\tpath = {path}",
            "\ttype = LaunchAgent",
            "\tstate = running",
            f"\tprogram = {arguments[0]}",
            "\targuments = {",
            f"\t\t{arguments[0]}",
            f"\t\t{arguments[2]}",
            f"\t\t{arguments[1]}",
            f"\t\t{arguments[3]}",
            "\t}",
            f"\tworking directory = {payload['WorkingDirectory']}",
            f"\tstdout path = {payload['StandardOutPath']}",
            f"\tstderr path = {payload['StandardErrorPath']}",
            f"\tdomain = {module.DOMAIN}",
            "}",
        ]
    ).encode()
    ops = module.LaunchctlOps()
    monkeypatch.setattr(ops, "job_output", lambda *_args: output)

    with pytest.raises(module.ResetError, match="arguments differ"):
        ops.require_loaded_definition(path, body)


class FixtureHealth(module.HealthClient):
    def __init__(
        self,
        *,
        minimum_signers: int = 3,
        qc_signers: int = 3,
        disagree: bool = False,
        same_node: bool = False,
    ) -> None:
        self.minimum_signers = minimum_signers
        self.qc_signers = qc_signers
        self.disagree = disagree
        self.same_node = same_node

    def _request(
        self,
        url: str,
        *,
        headers: dict[str, str] | None = None,
        parse_json: bool,
    ) -> object:
        del headers
        if not parse_json:
            return None
        return {"blocks": 7}

    def _protected_status(
        self,
        port: int,
        network_id: str,
        private_key_file: Path,
    ) -> dict[str, object]:
        del network_id, private_key_file
        suffix = 2 if self.disagree and port == module.TORII_PORTS[-1] else 0
        subject = {"block_hash": "00" * 31 + f"0{suffix}"}
        return {
            "protocol_version": 4,
            "restart_required": False,
            "height": 7,
            "last_committed_height": 7,
            "height_context": {
                "validator_count": 4,
                "mode": {"mode": "permissioned", "details": None},
                "quorum": {
                    "min_signers": self.minimum_signers,
                    "total_power": 4,
                },
            },
            "last_committed_subject": subject,
            "last_commit_qc": {
                "certificate": {
                    "round": {"height": 7, "view": 0},
                    "phase": {"phase": "commit", "details": None},
                    "subject": subject,
                },
                "validator_count": 4,
                "signer_count": self.qc_signers,
                "min_signers": self.minimum_signers,
                "signed_power": self.qc_signers,
                "total_power": 4,
            },
            "height_context_id": {"epoch": 1},
            "node_fingerprint": {"port": 0 if self.same_node else port},
            "build_fingerprint": {"commit": "a" * 40},
            "config_fingerprint": {"network": "nevo"},
        }


def fixture_operator_keys() -> tuple[Path, ...]:
    return tuple(Path(f"/runtime-only/operator-{index}.key") for index in range(4))


def test_health_requires_exact_three_of_four_qc() -> None:
    with pytest.raises(module.ResetError, match="3-of-4 quorum"):
        FixtureHealth(minimum_signers=2).fleet_sample(
            module.TORII_PORTS, OLD_NETWORK_ID, fixture_operator_keys()
        )


def test_health_requires_one_common_committed_frontier() -> None:
    with pytest.raises(module.ResetError, match="disagree"):
        FixtureHealth(disagree=True).fleet_sample(
            module.TORII_PORTS, OLD_NETWORK_ID, fixture_operator_keys()
        )


def test_health_requires_a_durable_three_signer_commit_qc() -> None:
    with pytest.raises(module.ResetError, match="durable CommitQC"):
        FixtureHealth(qc_signers=2).fleet_sample(
            module.TORII_PORTS, OLD_NETWORK_ID, fixture_operator_keys()
        )


def test_health_requires_four_distinct_node_identities() -> None:
    with pytest.raises(module.ResetError, match="distinct node identities"):
        FixtureHealth(same_node=True).fleet_sample(
            module.TORII_PORTS, OLD_NETWORK_ID, fixture_operator_keys()
        )


def test_health_accepts_exact_four_peer_three_of_four_qc() -> None:
    sample = FixtureHealth().fleet_sample(
        module.TORII_PORTS, OLD_NETWORK_ID, fixture_operator_keys()
    )

    assert sample.height == 7
    assert sample.block_hash == "00" * 32
    assert len(sample.peers) == 4


def test_health_stability_does_not_require_idle_chain_advancement(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    health = FixtureHealth()
    monkeypatch.setattr(module.time, "sleep", lambda _seconds: None)
    limits = module.Limits(
        minimum_free_bytes=module.MIN_FREE_BYTES,
        maximum_fsync_latency_ms=500,
        startup_timeout_seconds=30,
        stability_timeout_seconds=5,
        poll_interval_seconds=0.25,
    )

    first, second = health.wait_fleet(
        module.TORII_PORTS,
        OLD_NETWORK_ID,
        fixture_operator_keys(),
        limits,
    )

    assert first == second


def test_health_requires_four_distinct_ordered_peer_authentication_keys() -> None:
    health = FixtureHealth()
    limits = module.Limits(
        minimum_free_bytes=module.MIN_FREE_BYTES,
        maximum_fsync_latency_ms=500,
        startup_timeout_seconds=30,
        stability_timeout_seconds=5,
        poll_interval_seconds=0.25,
    )
    with pytest.raises(module.ResetError, match="four distinct ordered peer key"):
        health.wait_fleet(
            module.TORII_PORTS,
            OLD_NETWORK_ID,
            (Path("/runtime-only/shared.key"),) * module.PEER_COUNT,
            limits,
        )
    with pytest.raises(module.ResetError, match="four ordered peer key files"):
        health.fleet_sample(
            module.TORII_PORTS,
            OLD_NETWORK_ID,
            (Path("/runtime-only/one.key"),),
        )


@pytest.mark.parametrize(
    ("port", "network_id", "key_path"),
    (
        (module.TORII_PORTS[0], OLD_NETWORK_ID, Path("/private/old-peer-1.key")),
        (
            module.TORII_PORTS[3],
            module.reset_bundle.validator_renderer._format_literal(
                "hash", NEW_HASH.upper()
            ),
            Path("/private/candidate-peer-4.key"),
        ),
    ),
)
def test_native_operator_status_client_is_digest_pinned_fixed_env_and_exact_argv(
    tmp_path: Path,
    port: int,
    network_id: str,
    key_path: Path,
) -> None:
    client = private_file(
        tmp_path / "taira_operator_status",
        (
            b"#!/opt/homebrew/bin/python3\n"
            b"import json, os, sys\n"
            + f"expected = {['--torii-url', f'http://127.0.0.1:{port}/', '--network-id', network_id, '--operator-private-key-file', str(key_path), '--timeout-ms', '2000']!r}\n".encode()
            + b"proxy = any(name.upper().endswith('_PROXY') for name in os.environ)\n"
            + b"if sys.argv[1:] != expected or os.environ.get('LANG') != 'C' or os.environ.get('LC_ALL') != 'C' or proxy:\n"
            + b"    raise SystemExit(23)\n"
            + b"print(json.dumps({'status': 'ok'}, sort_keys=True))\n"
        ),
        executable=True,
    )
    digest = hashlib.sha256(client.read_bytes()).hexdigest()
    health = module.HealthClient(client, digest)

    assert health._protected_status(
        port, network_id, key_path
    ) == {"status": "ok"}

    wrong_digest = module.HealthClient(client, "0" * 64)
    with pytest.raises(module.ResetError, match="differs before protected read"):
        wrong_digest._protected_status(
            port, network_id, key_path
        )


@pytest.mark.parametrize(
    ("body", "message"),
    (
        (b"#!/bin/sh\nexit 23\n", "refused the protected read"),
        (b"#!/bin/sh\nprintf 'not-json\\n'\n", "returned invalid JSON"),
    ),
)
def test_native_operator_status_client_rejects_exit_and_invalid_json(
    tmp_path: Path,
    body: bytes,
    message: str,
) -> None:
    client = private_file(tmp_path / "taira_operator_status", body, executable=True)
    health = module.HealthClient(client, hashlib.sha256(body).hexdigest())

    with pytest.raises(module.ResetError, match=message):
        health._protected_status(
            module.TORII_PORTS[0], OLD_NETWORK_ID, Path("/private/peer-1.key")
        )


def test_native_operator_status_client_rejects_timeout_oversize_and_mutation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    timeout_client = private_file(
        tmp_path / "timeout/taira_operator_status",
        b"#!/bin/sh\n/bin/sleep 10\n",
        executable=True,
    )
    timeout_health = module.HealthClient(
        timeout_client, hashlib.sha256(timeout_client.read_bytes()).hexdigest()
    )
    ticks = iter((0.0, 5.0, 5.0))
    monkeypatch.setattr(module.time, "monotonic", lambda: next(ticks, 5.0))
    monkeypatch.setattr(module.time, "sleep", lambda _seconds: None)
    with pytest.raises(module.ResetError, match="runtime bound"):
        timeout_health._protected_status(
            module.TORII_PORTS[0], OLD_NETWORK_ID, Path("/private/peer-1.key")
        )

    monkeypatch.undo()
    oversize_client = private_file(
        tmp_path / "oversize/taira_operator_status",
        (
            b"#!/opt/homebrew/bin/python3\n"
            b"import sys\n"
            + f"sys.stdout.write('x' * {module.MAX_HTTP_BYTES + 1})\n".encode()
        ),
        executable=True,
    )
    oversize_health = module.HealthClient(
        oversize_client,
        hashlib.sha256(oversize_client.read_bytes()).hexdigest(),
    )
    with pytest.raises(module.ResetError, match="bound"):
        oversize_health._protected_status(
            module.TORII_PORTS[0], OLD_NETWORK_ID, Path("/private/peer-1.key")
        )

    mutation_client = private_file(
        tmp_path / "mutation/taira_operator_status",
        (
            b"#!/opt/homebrew/bin/python3\n"
            b"import sys\n"
            b"with open(sys.argv[0], 'ab') as output:\n"
            b"    output.write(b' ')\n"
            b"print('{}')\n"
        ),
        executable=True,
    )
    mutation_health = module.HealthClient(
        mutation_client,
        hashlib.sha256(mutation_client.read_bytes()).hexdigest(),
    )
    with pytest.raises(module.ResetError, match="changed during protected read"):
        mutation_health._protected_status(
            module.TORII_PORTS[0], OLD_NETWORK_ID, Path("/private/peer-1.key")
        )


def test_apply_refuses_nonexact_or_misordered_peer_key_paths(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)
    candidate = tuple(
        peer.workdir / "runtime/validator-signer.key" for peer in plan.candidate
    )
    predecessor = tuple(
        peer.workdir / "runtime/validator-signer.key" for peer in plan.predecessor
    )

    with pytest.raises(module.ResetError, match="exact ordered peer runtime paths"):
        module.apply_reset(
            plan,
            confirmation=plan.activation.confirmation,
            operator_private_key_files=tuple(reversed(candidate)),
            rollback_operator_private_key_files=predecessor,
        )
    with pytest.raises(
        module.ResetError, match="exact ordered predecessor runtime paths"
    ):
        module.apply_reset(
            plan,
            confirmation=plan.activation.confirmation,
            operator_private_key_files=candidate,
            rollback_operator_private_key_files=tuple(reversed(predecessor)),
        )


def test_apply_refuses_wrong_confirmation_before_lock(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)

    with pytest.raises(module.ResetError, match="confirmation"):
        module.apply_reset(
            plan,
            confirmation="RESET-THE-WRONG-THING",
            operator_private_key_files=(Path("/private/key"),),
            rollback_operator_private_key_files=(Path("/private/key"),),
        )

    assert not plan.activation.path.parents[1].joinpath(".user-launchagent-reset.lock").exists()


class RollbackOps:
    def __init__(self) -> None:
        self.loaded = set(module.LABELS)

    def job_loaded(self, domain: str, label: str) -> bool:
        assert domain == module.DOMAIN
        return label in self.loaded

    def require_initial_cohort(self) -> None:
        assert self.loaded == set(module.LABELS)

    def bootout(self, label: str) -> None:
        self.loaded.remove(label)

    def wait_absent(self, labels: tuple[str, ...]) -> None:
        assert not (self.loaded & set(labels))

    def bootstrap(self, path: Path) -> None:
        self.loaded.add(path.stem)

    def require_loaded_definition(self, path: Path, body: bytes) -> None:
        assert path.read_bytes() == body
        assert path.stem in self.loaded


class RollbackHealth:
    def wait_fleet(self, *_args: object) -> tuple[module.FleetSample, module.FleetSample]:
        sample = module.FleetSample(
            height=7,
            block_hash="0" * 64,
            peers=tuple({"port": port} for port in module.TORII_PORTS),
        )
        return sample, sample


class CandidateFailureHealth(RollbackHealth):
    def __init__(self) -> None:
        self.calls = 0

    def wait_fleet(self, *_args: object) -> tuple[module.FleetSample, module.FleetSample]:
        self.calls += 1
        if self.calls == 2:
            raise module.ResetError("candidate QC failed")
        return super().wait_fleet(*_args)


def test_rollback_restores_all_plists_and_retains_storage_evidence(tmp_path: Path) -> None:
    layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)
    private_dir(plan.archive_dir)
    for peer in plan.candidate:
        module.atomic_write(
            peer.plist_path,
            peer.plist_body,
            mode=0o600,
            replace=True,
        )
    before_storage = {
        peer.label: (peer.storage.lstat().st_dev, peer.storage.lstat().st_ino)
        for peer in plan.predecessor
    }
    ops = RollbackOps()

    module.rollback(
        plan,
        ops,  # type: ignore[arg-type]
        RollbackHealth(),  # type: ignore[arg-type]
        Path("/runtime-only/old-operator.key"),
        module.ResetError("candidate readiness failed"),
    )

    assert ops.loaded == set(module.LABELS)
    for peer in plan.predecessor:
        assert peer.plist_path.read_bytes() == peer.plist_body
        assert (peer.storage.lstat().st_dev, peer.storage.lstat().st_ino) == (
            before_storage[peer.label]
        )
    receipt = json.loads((plan.archive_dir / "rollback.json").read_bytes())
    assert receipt["restored"] is True
    assert receipt["errors"] == []
    inventory = json.loads((plan.archive_dir / "inventory.json").read_bytes())
    assert "rollback.json" in inventory


def test_apply_automatically_rolls_back_a_candidate_qc_failure(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)
    monkeypatch.setattr(
        module.reset_bundle,
        "require_mutable_bundle_identities",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        module,
        "_require_nevo_genesis_integrity",
        lambda _activation, _manifest: ({}, {}),
    )
    monkeypatch.setattr(
        module,
        "_run_native_genesis_verifier",
        lambda _activation, _receipt: plan.genesis_native_verifier_identity,
    )
    ops = RollbackOps()
    health = CandidateFailureHealth()

    with pytest.raises(module.ResetError, match="candidate QC failed"):
        module.apply_reset(
            plan,
            confirmation=plan.activation.confirmation,
            operator_private_key_files=tuple(
                peer.workdir / "runtime/validator-signer.key"
                for peer in plan.candidate
            ),
            rollback_operator_private_key_files=tuple(
                peer.workdir / "runtime/validator-signer.key"
                for peer in plan.predecessor
            ),
            layout=layout,
            ops=ops,  # type: ignore[arg-type]
            health=health,  # type: ignore[arg-type]
        )

    assert health.calls == 3
    assert ops.loaded == set(module.LABELS)
    assert not layout.lock_path.exists()
    for peer in plan.predecessor:
        assert peer.plist_path.read_bytes() == peer.plist_body
    receipt = json.loads((plan.archive_dir / "rollback.json").read_bytes())
    assert receipt["restored"] is True


def test_plan_projection_is_explicitly_non_mutating(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)

    projection = module.plan_projection(plan)

    assert projection["mode"] == "dry-run"
    assert projection["mutated"] is False
    assert projection["confirmation_required"] == (
        f"RESET-TAIRA-USER-501:{plan.activation.sha256}"
    )
