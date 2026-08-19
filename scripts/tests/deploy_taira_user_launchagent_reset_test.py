"""Focused tests for the exact user/501 Taira reset controller."""

from __future__ import annotations

import hashlib
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
    bundle = Path(str(payload["bundle"]))
    private_dir(bundle)
    private_file(bundle / "genesis.json", b'{"domain":"nevo.dpn"}\n')
    private_file(bundle / "reset-manifest.json", b'{"schema":"fixture"}\n')

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
    )
    return layout, plan, captured, launchctl


def test_parser_is_dry_run_by_default() -> None:
    args = module.parser().parse_args(["--manifest", "/tmp/reset.json"])

    assert args.apply is False
    assert args.confirm_reset is None


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
        )


def test_build_plan_requires_four_fresh_candidate_storage_directories(tmp_path: Path) -> None:
    layout, activation, bundle_plan = build_fixture(tmp_path)
    private_file(bundle_plan.peers[1].storage / "block", b"not fresh")

    with pytest.raises(module.ResetError, match="distinct fresh bundle"):
        module.build_plan(
            activation,
            layout=layout,
            validate_bundle_fn=lambda *_args, **_kwargs: bundle_plan,
        )


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


class FixtureHealth(module.HealthClient):
    def __init__(self, *, minimum_signers: int = 3, disagree: bool = False) -> None:
        self.minimum_signers = minimum_signers
        self.disagree = disagree

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
        port = int(url.split(":")[2].split("/")[0])
        suffix = 2 if self.disagree and port == module.TORII_PORTS[-1] else 0
        if not url.endswith("/v1/sumeragi/status"):
            return {"blocks": 7}
        return {
            "protocol_version": 4,
            "restart_required": False,
            "height": 7,
            "last_committed_height": 7,
            "height_context": {
                "validator_count": 4,
                "quorum": {
                    "min_signers": self.minimum_signers,
                    "total_power": 4,
                },
            },
            "last_committed_subject": {"block_hash": "00" * 31 + f"0{suffix}"},
        }


class FixtureOperatorContext:
    def headers(self, method: str, target: str, body: bytes) -> dict[str, str]:
        del method, target, body
        return {}


def test_health_requires_exact_three_of_four_qc() -> None:
    with pytest.raises(module.ResetError, match="3-of-4 quorum"):
        FixtureHealth(minimum_signers=2).fleet_sample(
            module.TORII_PORTS, FixtureOperatorContext()
        )


def test_health_requires_one_common_committed_frontier() -> None:
    with pytest.raises(module.ResetError, match="disagree"):
        FixtureHealth(disagree=True).fleet_sample(
            module.TORII_PORTS, FixtureOperatorContext()
        )


def test_health_accepts_exact_four_peer_three_of_four_qc() -> None:
    sample = FixtureHealth().fleet_sample(
        module.TORII_PORTS, FixtureOperatorContext()
    )

    assert sample.height == 7
    assert sample.block_hash == "00" * 32
    assert len(sample.peers) == 4


def test_health_stability_does_not_require_idle_chain_advancement(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    health = FixtureHealth()
    monkeypatch.setattr(
        module,
        "load_operator_context_from_file",
        lambda *_args: FixtureOperatorContext(),
    )
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
        Path("/runtime-only/operator.key"),
        limits,
    )

    assert first == second


def test_apply_refuses_wrong_confirmation_before_lock(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)

    with pytest.raises(module.ResetError, match="confirmation"):
        module.apply_reset(
            plan,
            confirmation="RESET-THE-WRONG-THING",
            operator_private_key_file=Path("/private/key"),
            rollback_operator_private_key_file=Path("/private/key"),
        )

    assert not plan.activation.path.parents[1].joinpath(".user-launchagent-reset.lock").exists()


def test_plan_projection_is_explicitly_non_mutating(tmp_path: Path) -> None:
    _layout, plan, _captured, _launchctl = build_plan_fixture(tmp_path)

    projection = module.plan_projection(plan)

    assert projection["mode"] == "dry-run"
    assert projection["mutated"] is False
    assert projection["confirmation_required"] == (
        f"RESET-TAIRA-USER-501:{plan.activation.sha256}"
    )
