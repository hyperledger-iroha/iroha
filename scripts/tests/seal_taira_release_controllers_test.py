from __future__ import annotations

import hashlib
import json
import os
import fcntl
from pathlib import Path
import stat

import pytest

from scripts import seal_taira_release_controllers as controller

ROOT = Path(__file__).resolve().parents[2]


def _write_handoff(root: Path, files: dict[str, bytes], kind: str = "test-handoff") -> None:
    root.mkdir(mode=0o700)
    rows: list[dict[str, object]] = []
    for relative, payload in sorted(files.items()):
        path = root / relative
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        path.write_bytes(payload)
        rows.append(
            {
                "path": relative,
                "sha256": hashlib.sha256(payload).hexdigest(),
                "size": len(payload),
            }
        )
    manifest = {
        "files": rows,
        "kind": kind,
        "schema": "iroha.taira.release_handoff",
        "schema_version": 1,
    }
    (root / controller.HANDOFF_MANIFEST).write_bytes(
        controller.canonical_json_bytes(manifest)
    )


def _attestation(
    handoff_root: Path,
    trusted: Path,
    role: str = "public-input-authority",
) -> dict[str, object]:
    identity_root = trusted.parent / "identity-roots"
    roots = {
        name: identity_root / name for name in ("staging", "runtime", "authority")
    }
    for root in roots.values():
        root.mkdir(mode=0o700, parents=True, exist_ok=True)
    return {
        "authority_gid": os.getgid(),
        "authority_root": str(roots["authority"]),
        "authority_uid": os.getuid(),
        "controller_gid": os.getegid(),
        "controller_digest": "d" * 64,
        "controller_root": str(controller.CONTROLLER_ROOT),
        "handoff_root": str(handoff_root),
        "invoking_gid": os.getgid(),
        "invoking_uid": os.getuid(),
        "host_id": "test-host-v1",
        "installation_id": "test-installation-v1",
        "role": role,
        "source_commit": "a" * 40,
        "runtime_gid": os.getgid(),
        "runtime_root": str(roots["runtime"]),
        "runtime_uid": os.getuid(),
        "staging_gid": os.getgid(),
        "staging_root": str(roots["staging"]),
        "staging_uid": os.getuid(),
        "trusted_executables": [],
        "trusted_inputs": [
            {
                "flag": "--source",
                "operation": "snapshot-public-privacy",
                "path": str(trusted / "public"),
            }
        ],
        "trusted_values": [],
        "uid": os.geteuid(),
    }


def _snapshot_args(source: Path, output: Path, forbidden: Path) -> list[str]:
    return [
        "--source",
        str(source),
        "--output",
        str(output),
        "--forbidden-root",
        str(forbidden),
    ]


def test_controller_closures_are_exact_installed_operation_dependencies() -> None:
    assert "configs/soranexus/taira/prepare_taira_release_source.sh" not in (
        controller.LINUX_FILES + controller.MACOS_FILES
    )
    assert "scripts/snapshot_taira_public_privacy_inputs.py" in controller.LINUX_FILES
    assert "scripts/build_privacy_v1_boi_handoff.py" in controller.LINUX_FILES
    assert "scripts/check_native_sdk_abi22_artifact.py" in controller.LINUX_FILES
    assert "scripts/build_taira_public_v2_prerequisite_handoff.py" in (
        controller.MACOS_FILES
    )
    assert "scripts/build_taira_public_v2_prerequisite_handoff.py" not in (
        controller.LINUX_FILES
    )
    for relative in (
        "scripts/deploy_taira_v21_reset_authority.py",
        "scripts/deploy_taira_v21_reset_health.py",
    ):
        assert relative in controller.MACOS_FILES
        assert relative not in controller.LINUX_FILES
    for relative in set(controller.LINUX_FILES + controller.MACOS_FILES):
        assert (ROOT / relative).is_file(), relative
        assert controller._validate_relative(relative) == relative
    assert controller.CONTROLLER_COMMAND == Path(
        "/usr/local/libexec/iroha-taira-release-controller-v1"
    )
    assert controller.CONTROLLER_ROOT == Path(
        "/usr/local/libexec/iroha-taira-release-controller-v1.d"
    )
    assert controller.KAGEMUSHA_PREPARE_RESET_FLAGS <= controller.OPERATION_FLAGS[
        "prepare-reset"
    ]
    assert not (
        controller.KAGEMUSHA_PREPARE_RESET_FLAGS
        & controller.REQUIRED_FLAGS[("prepare-reset", None)]
    )
    assert "--kagemusha-release-root" in controller.INPUT_PATH_FLAGS
    authenticated_controller_flag = "--authenticated-tool-controller"
    authenticated_controller_digest_flag = (
        "--trusted-authenticated-tool-controller-sha256"
    )
    assert authenticated_controller_flag in controller.OPERATION_FLAGS[
        "prepare-reset"
    ]
    assert authenticated_controller_digest_flag in controller.OPERATION_FLAGS[
        "prepare-reset"
    ]
    assert authenticated_controller_flag in controller.REQUIRED_FLAGS[
        ("prepare-reset", None)
    ]
    assert authenticated_controller_digest_flag in controller.REQUIRED_FLAGS[
        ("prepare-reset", None)
    ]
    assert authenticated_controller_flag in controller.INPUT_PATH_FLAGS
    assert authenticated_controller_flag in controller.TRUSTED_EXECUTABLE_FLAGS
    assert controller.EXECUTABLE_DIGEST_FLAGS[authenticated_controller_flag] == (
        authenticated_controller_digest_flag
    )
    for role in ("macos-qualification", "macos-deploy"):
        assert controller._expected_executable_identity(
            role, "prepare-reset", authenticated_controller_flag
        ) == "runtime"
    assert "seal" not in {action for contract in controller.ROLE_OPERATIONS.values() for action in contract[1]}
    assert "cleanup" not in {action for contract in controller.ROLE_OPERATIONS.values() for action in contract[1]}
    assert controller.ROLE_OPERATIONS["linux-boi-qualification"] == (
        "linux",
        {"admit", "assemble-boi"},
    )
    assert "--external-signer" not in controller.OPERATION_FLAGS["assemble-boi"]
    assert "--signing-public-key" not in controller.OPERATION_FLAGS["assemble-boi"]
    assert "--qualification-external-signer" in controller.OPERATION_FLAGS[
        "assemble-boi"
    ]
    assert "--trusted-qualification-external-signer-sha256" in (
        controller.REQUIRED_FLAGS[("assemble-boi", None)]
    )
    assert controller.EXECUTABLE_DIGEST_FLAGS[
        "--qualification-external-signer"
    ] == "--trusted-qualification-external-signer-sha256"
    assert "--qualification-signing-public-key" in controller.OPERATION_FLAGS[
        "assemble-boi"
    ]
    assert "--trusted-qualification-signing-fingerprint" in (
        controller.REQUIRED_FLAGS[("assemble-boi", None)]
    )
    assert controller.TRUSTED_LITERAL_FLAGS["assemble-boi"] == {
        "--trusted-signing-fingerprint",
        "--trusted-qualification-signing-fingerprint",
    }
    assert "--candidate-replay-ledger" not in controller.OPERATION_FLAGS["assemble-boi"]
    assert controller.IMMUTABLE_HANDOFF_OUTPUT_PREFIXES["assemble-boi"] == (
        "boi-qualified-"
    )
    assert "--boi-qualified-handoff-root" in controller.OPERATION_FLAGS[
        "deploy-reset"
    ]
    assert "--boi-qualified-handoff-root" in controller.REQUIRED_FLAGS[
        ("deploy-reset", None)
    ]
    assert "--trusted-boi-qualification-public-key" in controller.REQUIRED_FLAGS[
        ("deploy-reset", None)
    ]
    assert "--expected-boi-qualification-controller-digest" in (
        controller.REQUIRED_FLAGS[("deploy-reset", None)]
    )
    assert "--trusted-signing-fingerprint" in controller.TRUSTED_LITERAL_FLAGS[
        "deploy-reset"
    ]
    assert controller.REQUIRED_FLAGS[("assemble-boi", None)] == (
        controller.OPERATION_FLAGS["assemble-boi"]
    )
    assert controller.BOI_QUALIFICATION_ISOLATION_CONTRACT in (
        controller.BOI_QUALIFICATION_ISSUANCE_BARRIER
    )
    assert controller.BOI_QUALIFICATION_RUN_BINDING_CONTRACT in (
        controller.BOI_QUALIFICATION_ISSUANCE_BARRIER
    )
    assert controller.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT in (
        controller.BOI_QUALIFICATION_ISSUANCE_BARRIER
    )
    assert controller.DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT in (
        controller.DEPLOY_ISSUANCE_BARRIER
    )
    assert controller.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT in (
        controller.DEPLOY_ISSUANCE_BARRIER
    )


def test_boi_signing_roles_and_source_commit_are_attested_not_caller_selected() -> None:
    release = "1" * 64
    qualification = "2" * 64
    controller._require_distinct_release_and_qualification_signers(
        [
            {
                "operation": "assemble-boi",
                "flag": "--trusted-signing-fingerprint",
                "value": release,
            },
            {
                "operation": "assemble-boi",
                "flag": "--trusted-qualification-signing-fingerprint",
                "value": qualification,
            },
        ]
    )
    with pytest.raises(controller.ControllerSealError, match="must be distinct"):
        controller._require_distinct_release_and_qualification_signers(
            [
                {
                    "operation": "deploy-reset",
                    "flag": "--trusted-signing-fingerprint",
                    "value": release,
                },
                {
                    "operation": "deploy-reset",
                    "flag": "--trusted-boi-qualification-signing-fingerprint",
                    "value": release,
                },
            ]
        )

    attestation = {"source_commit": "a" * 40}
    controller._require_attested_source_commit(
        "assemble-boi",
        {"--expected-source-commit": ["a" * 40]},
        attestation,
    )
    controller._require_attested_source_commit(
        "admit", {"--output": ["/fresh"]}, attestation
    )
    with pytest.raises(controller.ControllerSealError, match="installed attestation"):
        controller._require_attested_source_commit(
            "assemble-boi",
            {"--expected-source-commit": ["b" * 40]},
            attestation,
        )


def test_controller_digest_is_domain_separated() -> None:
    payload = controller.canonical_json_bytes(
        {
            "files": [],
            "platform": "linux",
            "schema": controller.SCHEMA,
            "schema_version": controller.SCHEMA_VERSION,
            "source_commit": "1" * 40,
        }
    )
    assert controller.controller_digest(payload) == hashlib.sha256(
        b"iroha.taira.release-controller-closure.v1\0" + payload
    ).hexdigest()
    assert controller.controller_digest(payload) != hashlib.sha256(payload).hexdigest()


@pytest.mark.parametrize(
    "value",
    ("", ".", "..", "/absolute", "../escape", "scripts/../escape", "a//b", "a/./b"),
)
def test_manifest_path_rejects_noncanonical_values(value: str) -> None:
    with pytest.raises(controller.ControllerSealError):
        controller._validate_relative(value)


def test_handoff_is_copied_to_fresh_stage_before_source_can_change(tmp_path: Path) -> None:
    source = tmp_path / "source"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(source, {"bin/iroha3d": b"reviewed-validator"})

    result = controller.inspect_handoff(
        source, "test-handoff", staging, os.geteuid(), "run-one"
    )
    staged = Path(str(result["staged_root"]))
    original = staged / "bin/iroha3d"
    assert original.read_bytes() == b"reviewed-validator"

    replacement = source / "replacement"
    replacement.write_bytes(b"post-inspection replacement")
    os.replace(replacement, source / "bin/iroha3d")
    assert original.read_bytes() == b"reviewed-validator"
    assert stat_mode(original) == 0o444


def stat_mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


def test_handoff_stage_name_reuse_is_fail_closed(tmp_path: Path) -> None:
    source = tmp_path / "source"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(source, {"one": b"1"})
    controller.inspect_handoff(source, "test-handoff", staging, os.geteuid(), "same")
    with pytest.raises(controller.ControllerSealError, match="must be fresh"):
        controller.inspect_handoff(source, "test-handoff", staging, os.geteuid(), "same")


def test_handoff_rejects_intermediate_symlink_before_opening_payload(tmp_path: Path) -> None:
    source = tmp_path / "source"
    outside = tmp_path / "outside"
    staging = tmp_path / "staging"
    source.mkdir(mode=0o700)
    outside.mkdir(mode=0o700)
    staging.mkdir(mode=0o711)
    (outside / "iroha3d").write_bytes(b"outside")
    (source / "bin").symlink_to(outside, target_is_directory=True)
    manifest = {
        "files": [
            {
                "path": "bin/iroha3d",
                "sha256": hashlib.sha256(b"outside").hexdigest(),
                "size": 7,
            }
        ],
        "kind": "test-handoff",
        "schema": "iroha.taira.release_handoff",
        "schema_version": 1,
    }
    (source / controller.HANDOFF_MANIFEST).write_bytes(
        controller.canonical_json_bytes(manifest)
    )
    with pytest.raises(controller.ControllerSealError, match="symlink|intermediate"):
        controller.inspect_handoff(
            source, "test-handoff", staging, os.geteuid(), "symlink"
        )


def test_handoff_rejects_hardlinks_and_prebuffer_size_overflow(tmp_path: Path) -> None:
    source = tmp_path / "source"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(source, {"one": b"payload"})
    os.link(source / "one", source / "two")
    with pytest.raises(controller.ControllerSealError, match="single-link|hard-linked|unexpected"):
        controller.inspect_handoff(source, "test-handoff", staging, os.geteuid(), "hardlink")

    bounded = tmp_path / "bounded"
    bounded.write_bytes(b"12")
    with pytest.raises(controller.ControllerSealError, match="bounded"):
        controller._read_relative_stable(tmp_path, "bounded", 1)


@pytest.mark.parametrize("mutation", ("missing", "extra", "reordered"))
def test_public_input_handoff_rejects_missing_extra_or_reordered_inventory(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = tmp_path / "public-input-handoff"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(
        source,
        {"inputs/config.toml": b"config", "inputs/genesis.json": b"genesis"},
        kind="public-privacy-input",
    )
    if mutation == "missing":
        (source / "inputs/config.toml").unlink()
    elif mutation == "extra":
        (source / "inputs/unexpected").write_bytes(b"unexpected")
    else:
        manifest_path = source / controller.HANDOFF_MANIFEST
        manifest = json.loads(manifest_path.read_bytes())
        manifest["files"].reverse()
        manifest_path.write_bytes(controller.canonical_json_bytes(manifest))

    with pytest.raises(controller.ControllerSealError):
        controller.inspect_handoff(
            source,
            "public-privacy-input",
            staging,
            os.geteuid(),
            f"public-{mutation}",
        )


def test_post_helper_revalidation_detects_persistent_same_uid_replacement(
    tmp_path: Path,
) -> None:
    source = tmp_path / "source"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(source, {"bin/iroha3d": b"reviewed-validator"})
    result = controller.inspect_handoff(
        source, "test-handoff", staging, os.geteuid(), "persistent-attacker"
    )
    stage = Path(str(result["staged_root"]))
    controller._revalidate_staged_roots(
        staging,
        {stage},
        expected_owner=os.geteuid(),
        expected_group=os.getegid(),
    )

    # Model a process left behind under the operation UID. Production stages
    # are root-owned, so this chmod is unavailable there; if ownership or mode
    # regresses, the mandatory post-helper replay still fails closed.
    stage.chmod(0o700)
    (stage / "bin/iroha3d").chmod(0o600)
    (stage / "bin/iroha3d").write_bytes(b"post-helper replacement")
    with pytest.raises(controller.ControllerSealError, match="replaced"):
        controller._revalidate_staged_roots(
            staging,
            {stage},
            expected_owner=os.geteuid(),
            expected_group=os.getegid(),
        )


def test_staging_parent_and_operation_outputs_have_exact_ownership_modes(
    tmp_path: Path,
) -> None:
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    controller._revalidate_staged_roots(
        staging,
        set(),
        expected_owner=os.geteuid(),
        expected_group=os.getegid(),
    )
    staging.chmod(0o731)
    with pytest.raises(controller.ControllerSealError, match="staging parent"):
        controller._revalidate_staged_roots(
            staging,
            set(),
            expected_owner=os.geteuid(),
            expected_group=os.getegid(),
        )

    output = tmp_path / "output"
    output.mkdir(mode=0o700)
    artifact = output / "signed"
    artifact.write_bytes(b"signed-output")
    artifact.chmod(0o600)
    controller._validate_operation_outputs({output}, os.geteuid())
    artifact.chmod(0o620)
    with pytest.raises(controller.ControllerSealError, match="ownership or mode"):
        controller._validate_operation_outputs({output}, os.geteuid())


def test_authority_dispatch_has_fixed_cwd_environment_and_descriptor_policy(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    def fake_run(command, **kwargs):
        captured["command"] = command
        captured.update(kwargs)
        return type("Result", (), {"returncode": 0})()

    monkeypatch.setattr(controller.subprocess, "run", fake_run)
    assert controller._dispatch("check-public", ["--help"]) == 0
    assert captured["cwd"] == controller.CONTROLLER_ROOT
    assert captured["close_fds"] is True
    assert set(captured["env"]) == {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"}
    assert captured["command"] == [
        "/bin/bash",
        str(controller.CONTROLLER_ROOT / controller.BASH_OPERATIONS["check-public"]),
        "--help",
    ]


def test_privacy_rollout_dispatch_injects_only_the_sealed_plan(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    captured: dict[str, object] = {}

    def fake_dispatch(relative, args, run_as, external_tool_identity=None):
        captured.update(
            relative=relative,
            args=list(args),
            run_as=run_as,
            external_tool_identity=external_tool_identity,
        )
        return 0

    monkeypatch.setattr(
        controller,
        "_require_authenticated_rollout_observation_authority",
        lambda: None,
    )
    monkeypatch.setattr(controller, "_dispatch_installed_python", fake_dispatch)
    result = "/authority/privacy-rollout-observation.json"
    assert controller._dispatch(
        "verify-privacy-rollout", ["--result", result], (501, 20)
    ) == 0
    assert captured["relative"] == "scripts/taira_privacy_rollout_contract.py"
    assert captured["args"] == [
        "verify-result",
        "--plan",
        str(
            controller.CONTROLLER_ROOT
            / "configs/soranexus/taira/privacy_rollout_plan_v1.json"
        ),
        "--result",
        result,
    ]


def test_deploy_canary_inputs_are_mandatory_owner_private_and_unskippable(
    tmp_path: Path,
) -> None:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    attestation = _attestation(handoff, trusted, "macos-deploy")
    staging = Path(str(attestation["staging_root"]))
    authority = Path(str(attestation["authority_root"]))
    write_config = staging / "dedicated-canary.toml"
    observation = authority / "privacy-rollout-observation.json"
    write_config.write_bytes(b"chain = \"taira\"\n")
    observation.write_bytes(b"{}\n")
    write_config.chmod(0o400)
    observation.chmod(0o400)
    attestation["trusted_inputs"] = [
        {
            "flag": "--write-config",
            "operation": "check-public",
            "path": str(write_config),
        },
        {
            "flag": "--result",
            "operation": "verify-privacy-rollout",
            "path": str(observation),
        },
    ]
    public_args = [
        "--public-root",
        "https://taira.example",
        *[
            value
            for index in range(1, 5)
            for value in (
                "--validator-root",
                f"taira-validator-{index}=https://validator-{index}.example",
            )
        ],
        "--require-all-validators",
        "--expected-git-sha",
        "a" * 40,
        "--expected-dpn-validator-release-commit",
        "b" * 40,
        "--write-config",
        str(write_config),
    ]

    controller._validate_operation_args("check-public", public_args, attestation)
    controller._validate_operation_args(
        "verify-privacy-rollout", ["--result", str(observation)], attestation
    )
    with pytest.raises(controller.ControllerSealError, match="not allow-listed"):
        controller._validate_operation_args(
            "check-public", [*public_args[:-2], "--skip-write-canary"], attestation
        )
    write_config.chmod(0o600)
    with pytest.raises(controller.ControllerSealError, match="mode-0400"):
        controller._validate_operation_args("check-public", public_args, attestation)


def test_privilege_drop_clears_groups_and_uses_exact_attested_ids(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[str, object]] = []
    state = {"uid": 0, "gid": 0, "groups": [20, 80]}
    monkeypatch.setattr(controller.os, "geteuid", lambda: state["uid"])
    monkeypatch.setattr(controller.os, "getegid", lambda: state["gid"])
    monkeypatch.setattr(controller.os, "getgroups", lambda: list(state["groups"]))
    monkeypatch.setattr(
        controller.os,
        "setgroups",
        lambda value: (calls.append(("groups", list(value))), state.update(groups=list(value))),
    )
    monkeypatch.setattr(
        controller.os,
        "setgid",
        lambda value: (calls.append(("gid", value)), state.update(gid=value)),
    )
    monkeypatch.setattr(
        controller.os,
        "setuid",
        lambda value: (calls.append(("uid", value)), state.update(uid=value)),
    )
    monkeypatch.setattr(controller.os, "umask", lambda value: calls.append(("umask", value)))

    controller._drop_to_attested_user(501, 20)

    assert calls == [("groups", []), ("gid", 20), ("uid", 501), ("umask", 0o077)]


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        (lambda args: [*args, "--evil", "value"], "not allow-listed"),
        (lambda args: args[:-2], "mandatory options"),
        (lambda args: [*args, "--source", args[1]], "not allow-listed"),
        (lambda args: ["--source=/tmp/value", *args[2:]], "not allow-listed"),
        (lambda args: [args[0], args[1] + "\rpoison", *args[2:]], "unsafe"),
        (
            lambda args: [
                args[0],
                args[1] + ("x" * (controller.MAX_OPERATION_ARG_BYTES + 1)),
                *args[2:],
            ],
            "unsafe",
        ),
    ),
)
def test_operation_schema_rejects_smuggling_missing_and_duplicate_flags(
    tmp_path: Path, mutation, message: str
) -> None:
    staging = tmp_path / "staging"
    trusted = tmp_path / "trusted"
    output_parent = tmp_path / "output"
    forbidden = tmp_path / "workspace"
    staging.mkdir(mode=0o711)
    for path in (trusted, output_parent, forbidden):
        path.mkdir(mode=0o700)
    source = trusted / "public"
    source.mkdir(mode=0o700)
    args = _snapshot_args(source, staging / "public-input-123-1", forbidden)
    with pytest.raises(controller.ControllerSealError, match=message):
        controller._validate_operation_args(
            "snapshot-public-privacy",
            mutation(args),
            _attestation(staging, trusted),
        )


def test_removed_publisher_admission_surface_rejects_arbitrary_root_output(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    staging = tmp_path / "staging"
    trusted = tmp_path / "trusted"
    forbidden = tmp_path / "workspace"
    for path in (staging, trusted, forbidden):
        path.mkdir(mode=0o700)
    monkeypatch.setattr(controller.os, "geteuid", lambda: 0)
    monkeypatch.setenv("SUDO_UID", str(os.getuid()))
    with pytest.raises(
        controller.ControllerSealError,
        match="lacks an exact operation identity",
    ):
        controller._validate_operation_args(
            "admit",
            [
                "init-replay-ledger",
                "--output",
                str(Path("/etc").resolve() / "iroha-forged-output"),
            ],
            _attestation(staging, trusted, "macos-publish"),
        )


def test_immutable_composite_output_keeps_controller_identity_contract(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    calls: list[tuple[set[Path], int, int | None]] = []
    monkeypatch.setattr(
        controller,
        "_validate_operation_outputs",
        lambda paths, uid, gid=None: calls.append((paths, uid, gid)),
    )
    attestation: dict[str, object] = {
        "uid": 0,
        "controller_gid": 7,
        "runtime_uid": 41,
        "runtime_gid": 42,
        "runtime_root": "/runtime",
    }
    output = {Path("/handoff/qualification-receipt-1-1")}

    controller._validate_successful_operation_outputs(
        "capture-four-peer",
        output,
        attestation,
        "runtime",
    )
    boi_output = {Path("/handoff/boi-qualified-1-1")}
    controller._validate_successful_operation_outputs(
        "assemble-boi",
        boi_output,
        attestation,
        "authority",
    )
    controller._validate_successful_operation_outputs(
        "extract-privacy",
        {Path("/runtime/extract")},
        attestation,
        "runtime",
    )
    controller._validate_successful_operation_outputs(
        "deploy-reset",
        set(),
        attestation,
        "root",
    )

    assert calls == [
        (output, 0, 7),
        (boi_output, 0, 7),
        ({Path("/runtime/extract")}, 41, 42),
    ]


def test_immutable_authority_handoff_output_requires_attested_root_parent(
    tmp_path: Path,
) -> None:
    staging = tmp_path / "staging"
    trusted = tmp_path / "trusted"
    forbidden = tmp_path / "workspace"
    runner_temp = tmp_path / "runner-temp"
    staging.mkdir(mode=0o711)
    for path in (trusted, forbidden, runner_temp):
        path.mkdir(mode=0o700)
    source = trusted / "public"
    source.mkdir(mode=0o700)
    attestation = _attestation(staging, trusted)
    output = staging / "public-input-123-1"

    staged, outputs = controller._validate_operation_args(
        "snapshot-public-privacy",
        _snapshot_args(source, output, forbidden),
        attestation,
    )
    assert not staged
    assert outputs == {output}

    output.mkdir(mode=0o700)
    with pytest.raises(controller.ControllerSealError, match="immutable authority handoff"):
        controller._validate_operation_args(
            "snapshot-public-privacy",
            _snapshot_args(source, output, forbidden),
            attestation,
        )
    output.rmdir()

    for substituted in (
        runner_temp / output.name,
        staging / "wrong-prefix-123-1",
        staging / "public-input-0-1",
        staging / "public-input-123-0",
    ):
        with pytest.raises(
            controller.ControllerSealError,
            match="immutable authority handoff output",
        ):
            controller._validate_operation_args(
                "snapshot-public-privacy",
                _snapshot_args(source, substituted, forbidden),
                attestation,
            )


def test_controller_cli_rejects_abbreviated_trust_arguments() -> None:
    with pytest.raises(SystemExit):
        controller.parse_args(
            [
                "attest",
                "--expected-launcher",
                "a" * 64,
                "--expected-controller-digest",
                "b" * 64,
                "--expected-version",
                "1",
                "--expected-host-id",
                "host",
                "--expected-installation-id",
                "installation",
                "--expected-uid",
                "0",
                "--source-commit",
                "c" * 40,
                "--platform",
                "linux",
                "--role",
                "linux-authority",
            ]
        )


def _fake_attestation_fixture(tmp_path: Path, role: str = "linux-authority"):
    launcher_payload = b"fixed-controller"
    handoff = tmp_path / "handoff"
    staging = tmp_path / "staging"
    runtime = tmp_path / "runtime"
    authority = tmp_path / "authority"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    staging.mkdir(mode=0o700)
    runtime.mkdir(mode=0o700)
    authority.mkdir(mode=0o700)
    trusted.mkdir(mode=0o700)
    signing_key = trusted / "signing.pub"
    signer = trusted / "signer"
    verifier = trusted / "verifier"
    signing_key.write_bytes(b"public-key")
    signer.write_bytes(b"fixed-signer")
    verifier.write_bytes(b"fixed-verifier")
    signer.chmod(0o500)
    verifier.chmod(0o500)
    staging_uid = os.geteuid()
    staging_gid = os.getegid()
    runtime_uid = staging_uid + 1
    authority_uid = staging_uid + 2
    trust = {
        "authority_gid": staging_gid,
        "authority_root": str(authority.resolve()),
        "authority_uid": authority_uid,
        "handoff_root": str(handoff.resolve()),
        "host_id": "host-a",
        "installation_id": "install-a",
        "platform": "linux",
        "role": role,
        "runtime_gid": staging_gid,
        "runtime_root": str(runtime.resolve()),
        "runtime_uid": runtime_uid,
        "schema": "iroha.taira.release_runner_trust",
        "schema_version": controller.RUNNER_TRUST_SCHEMA_VERSION,
        "staging_root": str(staging.resolve()),
        "staging_gid": staging_gid,
        "staging_uid": staging_uid,
        "trusted_executables": [
            {
                "digest_flag": None,
                "flag": "--external-signer",
                "operation": "finalize-linux",
                "path": str(signer.resolve()),
                "run_as": "authority",
                "sha256": hashlib.sha256(signer.read_bytes()).hexdigest(),
            },
            {
                "digest_flag": "--trusted-release-manifest-verifier-sha256",
                "flag": "--release-manifest-verifier",
                "operation": "finalize-linux",
                "path": str(verifier.resolve()),
                "run_as": "authority",
                "sha256": hashlib.sha256(verifier.read_bytes()).hexdigest(),
            },
        ],
        "trusted_inputs": [
            {
                "flag": "--signing-public-key",
                "operation": "finalize-linux",
                "path": str(signing_key.resolve()),
            }
        ],
        "trusted_values": [],
        "uid": os.geteuid(),
    }
    return launcher_payload, handoff, trusted, controller.canonical_json_bytes(trust)


def _patch_attestation_path_validation(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        controller,
        "_validate_identity_root",
        lambda path, *_args, **_kwargs: path,
    )
    monkeypatch.setattr(
        controller,
        "_validate_handoff_root",
        lambda path, **_kwargs: path,
    )
    monkeypatch.setattr(
        controller,
        "_validate_trusted_input_path",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        controller,
        "_validate_trusted_executable_path",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        controller,
        "_validate_publisher_trusted_input",
        lambda *_args, **_kwargs: None,
    )


@pytest.mark.parametrize(
    ("override", "message"),
    (
        ({"expected_host_id": "host-b"}, "host, installation"),
        ({"expected_installation_id": "install-b"}, "host, installation"),
        ({"expected_uid": "0"}, "execute as root"),
        ({"role": "public-input-authority"}, "host, installation, role"),
    ),
)
def test_attestation_rejects_wrong_host_install_uid_and_role(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    override: dict[str, str],
    message: str,
) -> None:
    launcher, staging, _trusted, trust_payload = _fake_attestation_fixture(tmp_path)
    command_path = tmp_path / "controller"
    trust_path = tmp_path / "trust.json"
    root = tmp_path / "controller-root"
    command_path.write_bytes(launcher)
    trust_path.write_bytes(trust_payload)
    root.mkdir()
    _patch_attestation_path_validation(monkeypatch)

    def fake_root_file(path: Path, _label: str, **_kwargs) -> bytes:
        return launcher if path == command_path else trust_payload

    monkeypatch.setattr(controller, "_require_root_owned_file", fake_root_file)
    monkeypatch.setenv("SUDO_GID", str(os.getegid()))
    monkeypatch.setenv("SUDO_UID", str(os.geteuid()))
    monkeypatch.setattr(
        controller,
        "verify",
        lambda *_args, **_kwargs: {
            "controller_digest": "b" * 64,
            "controller_manifest": str(root / controller.MANIFEST_NAME),
            "controller_root": str(root),
            "platform": "linux",
            "source_commit": "c" * 40,
        },
    )
    values = {
        "expected_launcher_sha256": hashlib.sha256(launcher).hexdigest(),
        "expected_controller_digest": "b" * 64,
        "expected_version": controller.CONTROLLER_VERSION,
        "expected_host_id": "host-a",
        "expected_installation_id": "install-a",
        "expected_uid": str(os.geteuid()),
        "source_commit": "c" * 40,
        "platform_name": "linux",
        "role": "linux-authority",
        "command_path": command_path.resolve(),
        "controller_root": root.resolve(),
        "runner_trust_file": trust_path.resolve(),
        "required_controller_uid": os.geteuid(),
        "required_controller_gid": os.getegid(),
    }
    values.update(override)
    with pytest.raises(controller.ControllerSealError, match=message):
        controller._attest(**values)


@pytest.mark.parametrize(
    ("identity_case", "message"),
    (
        ("missing", "canonical sudo caller identity"),
        ("nonnumeric", "canonical sudo caller identity"),
        ("leading-zero", "canonical sudo caller identity"),
        ("wrong-uid", "differs from the attested release runner"),
        ("wrong-gid", "differs from the attested release runner"),
    ),
)
def test_attestation_binds_exact_sudo_caller_to_staging_identity(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    identity_case: str,
    message: str,
) -> None:
    launcher, _staging, _trusted, trust_payload = _fake_attestation_fixture(tmp_path)
    command_path = tmp_path / "controller"
    trust_path = tmp_path / "trust.json"
    root = tmp_path / "controller-root"
    command_path.write_bytes(launcher)
    trust_path.write_bytes(trust_payload)
    root.mkdir()
    _patch_attestation_path_validation(monkeypatch)

    def fake_root_file(path: Path, _label: str, **_kwargs) -> bytes:
        return launcher if path == command_path else trust_payload

    monkeypatch.setattr(controller, "_require_root_owned_file", fake_root_file)
    monkeypatch.setattr(
        controller,
        "verify",
        lambda *_args, **_kwargs: {
            "controller_digest": "b" * 64,
            "controller_manifest": str(root / controller.MANIFEST_NAME),
            "controller_root": str(root),
            "platform": "linux",
            "source_commit": "c" * 40,
        },
    )
    sudo_uid = str(os.geteuid())
    sudo_gid = str(os.getegid())
    if identity_case == "missing":
        sudo_uid = None
        sudo_gid = None
    elif identity_case == "nonnumeric":
        sudo_uid = "not-a-uid"
    elif identity_case == "leading-zero":
        sudo_uid = f"0{os.geteuid()}"
    elif identity_case == "wrong-uid":
        sudo_uid = str(os.geteuid() + 1)
    else:
        sudo_gid = str(os.getegid() + 1)
    if sudo_uid is None:
        monkeypatch.delenv("SUDO_UID", raising=False)
    else:
        monkeypatch.setenv("SUDO_UID", sudo_uid)
    if sudo_gid is None:
        monkeypatch.delenv("SUDO_GID", raising=False)
    else:
        monkeypatch.setenv("SUDO_GID", sudo_gid)

    with pytest.raises(controller.ControllerSealError, match=message):
        controller._attest(
            expected_launcher_sha256=hashlib.sha256(launcher).hexdigest(),
            expected_controller_digest="b" * 64,
            expected_version=controller.CONTROLLER_VERSION,
            expected_host_id="host-a",
            expected_installation_id="install-a",
            expected_uid=str(os.geteuid()),
            source_commit="c" * 40,
            platform_name="linux",
            role="linux-authority",
            command_path=command_path.resolve(),
            controller_root=root.resolve(),
            runner_trust_file=trust_path.resolve(),
            required_controller_uid=os.geteuid(),
            required_controller_gid=os.getegid(),
        )


def _attest_trust_payload(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    launcher: bytes,
    trust: dict[str, object],
    *,
    sudo_uid: int | None = None,
    sudo_gid: int | None = None,
    platform_name: str = "linux",
    role: str = "linux-authority",
) -> dict[str, object]:
    tmp_path.mkdir(parents=True, exist_ok=True)
    command_path = tmp_path / "controller"
    trust_path = tmp_path / "trust.json"
    root = tmp_path / "controller-root"
    command_path.write_bytes(launcher)
    trust_payload = controller.canonical_json_bytes(trust)
    trust_path.write_bytes(trust_payload)
    root.mkdir()
    _patch_attestation_path_validation(monkeypatch)

    def fake_root_file(path: Path, _label: str, **_kwargs) -> bytes:
        return launcher if path == command_path else trust_payload

    monkeypatch.setattr(controller, "_require_root_owned_file", fake_root_file)
    monkeypatch.setattr(
        controller,
        "verify",
        lambda *_args, **_kwargs: {
            "controller_digest": "b" * 64,
            "controller_manifest": str(root / controller.MANIFEST_NAME),
            "controller_root": str(root),
            "platform": platform_name,
            "source_commit": "c" * 40,
        },
    )
    monkeypatch.setenv(
        "SUDO_UID",
        str(trust["staging_uid"] if sudo_uid is None else sudo_uid),
    )
    monkeypatch.setenv(
        "SUDO_GID",
        str(trust["staging_gid"] if sudo_gid is None else sudo_gid),
    )
    return controller._attest(
        expected_launcher_sha256=hashlib.sha256(launcher).hexdigest(),
        expected_controller_digest="b" * 64,
        expected_version=controller.CONTROLLER_VERSION,
        expected_host_id="host-a",
        expected_installation_id="install-a",
        expected_uid=str(os.geteuid()),
        source_commit="c" * 40,
        platform_name=platform_name,
        role=role,
        command_path=command_path.resolve(),
        controller_root=root.resolve(),
        runner_trust_file=trust_path.resolve(),
        required_controller_uid=os.geteuid(),
        required_controller_gid=os.getegid(),
    )


def _base_trust(tmp_path: Path) -> tuple[bytes, dict[str, object]]:
    launcher, _handoff, _trusted, payload = _fake_attestation_fixture(tmp_path)
    return launcher, json.loads(payload)


def _publisher_trust(tmp_path: Path) -> tuple[bytes, dict[str, object]]:
    launcher, _handoff, trusted, payload = _fake_attestation_fixture(tmp_path)
    trust = json.loads(payload)
    authority = Path(str(trust["authority_root"]))
    registry_config = authority / "registry-config.json"
    registry_config.write_bytes(b'{"auths":{}}\n')
    signing_key = trusted / "publication.pub"
    signing_key.write_bytes(b"k" * 32)
    executables = {
        "--external-signer": trusted / "publication-signer",
        "--oras": trusted / "oras",
        "--release-manifest-verifier": trusted / "publication-verifier",
    }
    for flag, path in executables.items():
        path.write_bytes(flag.encode("ascii"))
        path.chmod(0o555)
    trust.update({"platform": "macos", "role": "macos-publish"})
    trust["trusted_executables"] = [
        {
            "digest_flag": controller.SEALED_EXECUTABLE_DEPENDENCIES[
                "publish-rollout"
            ][flag],
            "flag": flag,
            "operation": "publish-rollout",
            "path": str(path.resolve()),
            "run_as": "authority",
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
        for flag, path in sorted(executables.items())
    ]
    trust["trusted_inputs"] = [
        {
            "flag": "--registry-config",
            "operation": "publish-rollout",
            "path": str(registry_config.resolve()),
        },
        {
            "flag": "--signing-public-key",
            "operation": "publish-rollout",
            "path": str(signing_key.resolve()),
        },
    ]
    trust["trusted_values"] = [
        {
            "flag": "--expected-oras-version",
            "operation": "publish-rollout",
            "value": "1.2.3",
        },
        {
            "flag": "--repository",
            "operation": "publish-rollout",
            "value": "registry.example/taira/release",
        },
        {
            "flag": "--suffix",
            "operation": "publish-rollout",
            "value": "testnet",
        },
        {
            "flag": "--trusted-signing-fingerprint",
            "operation": "publish-rollout",
            "value": hashlib.sha256(signing_key.read_bytes()).hexdigest(),
        },
    ]
    return launcher, trust


def _freeze_handoff(root: Path) -> None:
    for current, directories, files in os.walk(root, topdown=False):
        current_path = Path(current)
        for name in files:
            (current_path / name).chmod(0o444)
        for name in directories:
            (current_path / name).chmod(0o555)
        current_path.chmod(0o555)


def _publisher_operation_fixture(
    tmp_path: Path,
) -> tuple[list[str], dict[str, object], Path]:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    candidate = handoff / "publish-candidate-123-1"
    _write_handoff(candidate, {"candidate.tar.zst": b"candidate"}, "candidate")
    _freeze_handoff(candidate)
    attestation = _attestation(handoff, trusted, "macos-publish")
    authority_root = Path(str(attestation["authority_root"]))
    rollout_inputs = {
        "--rollout-plan": authority_root / "rollout-plan.json",
        "--rollout-result": authority_root / "rollout-result.json",
        "--rollout-authority-envelope": authority_root / "rollout-envelope.json",
        "--rollout-durable-receipt": authority_root / "rollout-receipt.json",
    }
    for flag, path in rollout_inputs.items():
        path.write_bytes((flag + "\n").encode("ascii"))
        path.chmod(0o400)
    attestation["trusted_values"] = [
        {
            "flag": "--expected-oras-version",
            "operation": "publish-rollout",
            "value": "1.2.3",
        },
        {
            "flag": "--repository",
            "operation": "publish-rollout",
            "value": "registry.example/taira/release",
        },
        {
            "flag": "--suffix",
            "operation": "publish-rollout",
            "value": "testnet",
        },
        {
            "flag": "--trusted-signing-fingerprint",
            "operation": "publish-rollout",
            "value": "f" * 64,
        },
    ]
    args = [
        "--candidate-root",
        str(candidate),
        "--expected-source-commit",
        "a" * 40,
        "--expected-dpn-validator-release-commit",
        "b" * 40,
        "--expected-cargo-lock-sha256",
        "c" * 64,
        "--expected-workspace-source-manifest-sha256",
        "d" * 64,
        "--expected-qualification-receipt-id",
        "e" * 64,
        "--repository",
        "registry.example/taira/release",
        "--suffix",
        "testnet",
    ]
    for flag, path in rollout_inputs.items():
        args.extend((flag, str(path)))
    return args, attestation, candidate


def test_runner_trust_schema_v2_attests_and_schema_v1_is_rejected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    launcher, trust = _base_trust(tmp_path)
    result = _attest_trust_payload(tmp_path, monkeypatch, launcher, trust)
    assert result["runtime_uid"] != result["staging_uid"]
    assert result["authority_uid"] != result["staging_uid"]
    assert "trusted_paths" not in result

    legacy = dict(trust)
    legacy["schema_version"] = 1
    legacy_tmp = tmp_path / "legacy"
    legacy_tmp.mkdir()
    with pytest.raises(controller.ControllerSealError, match="host, installation|fields"):
        _attest_trust_payload(legacy_tmp, monkeypatch, launcher, legacy)


def test_publisher_runner_trust_exactly_attests_sealed_tools_inputs_and_literals(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    launcher, trust = _publisher_trust(tmp_path)

    result = _attest_trust_payload(
        tmp_path / "attempt",
        monkeypatch,
        launcher,
        trust,
        platform_name="macos",
        role="macos-publish",
    )

    assert result["trusted_executables"] == trust["trusted_executables"]
    assert result["trusted_inputs"] == trust["trusted_inputs"]
    assert result["trusted_values"] == trust["trusted_values"]
    assert controller.ROLE_OPERATIONS["macos-publish"] == (
        "macos",
        {
            "build-public-soak-candidate",
            "build-public-soak-publication",
            "publish-rollout",
        },
    )


def _public_soak_prerequisite_fixture(
    tmp_path: Path,
) -> tuple[dict[str, object], Path, Path, Path, Path, Path]:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    handoff.chmod(0o711)
    trusted.chmod(0o700)
    candidate = handoff / "publish-candidate-123-1"
    publication = handoff / ("publication-receipt-" + "a" * 64)
    _write_handoff(candidate, {"candidate": b"candidate"}, "candidate")
    _write_handoff(publication, {"publication": b"publication"}, "publication")
    _freeze_handoff(candidate)
    _freeze_handoff(publication)
    attestation = _attestation(handoff, trusted, "macos-publish")
    candidate_prerequisite = handoff / "public-soak-candidate-122-1"
    _write_handoff(
        candidate_prerequisite,
        {"public-soak-prerequisite-v1.json": b"{}\n"},
        "public-soak-candidate-prerequisite",
    )
    _freeze_handoff(candidate_prerequisite)
    candidate_handoff = (
        candidate_prerequisite / "public-soak-prerequisite-v1.json"
    )
    candidate_output = handoff / "public-soak-candidate-123-1"
    publication_output = handoff / "public-soak-publication-123-1"
    return (
        attestation,
        candidate,
        publication,
        candidate_handoff,
        candidate_output,
        publication_output,
    )


def test_public_soak_prerequisite_operations_accept_only_their_exact_paths(
    tmp_path: Path,
) -> None:
    (
        attestation,
        candidate,
        publication,
        candidate_handoff,
        candidate_output,
        publication_output,
    ) = (
        _public_soak_prerequisite_fixture(tmp_path)
    )

    staged, outputs = controller._validate_operation_args(
        "build-public-soak-candidate",
        ["--candidate-root", str(candidate), "--output", str(candidate_output)],
        attestation,
    )
    assert staged == {candidate}
    assert outputs == {candidate_output}

    staged, outputs = controller._validate_operation_args(
        "build-public-soak-publication",
        [
            "--candidate-root",
            str(candidate),
            "--candidate-handoff",
            str(candidate_handoff),
            "--publication-root",
            str(publication),
            "--output",
            str(publication_output),
        ],
        attestation,
    )
    assert staged == {candidate, candidate_handoff.parent, publication}
    assert outputs == {publication_output}

    with pytest.raises(controller.ControllerSealError, match="not allow-listed"):
        controller._validate_operation_args(
            "build-public-soak-candidate",
            [
                "--candidate-root",
                str(candidate),
                "--candidate-handoff",
                str(candidate_handoff),
                "--output",
                str(candidate_output),
            ],
            attestation,
        )
    with pytest.raises(controller.ControllerSealError, match="mandatory options"):
        controller._validate_operation_args(
            "build-public-soak-publication",
            [
                "--candidate-root",
                str(candidate),
                "--output",
                str(publication_output),
            ],
            attestation,
        )


@pytest.mark.parametrize(
    ("operation", "command"),
    (
        ("build-public-soak-candidate", "candidate"),
        ("build-public-soak-publication", "publication"),
    ),
)
def test_public_soak_prerequisite_composite_injects_private_current_attestation(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    operation: str,
    command: str,
) -> None:
    (
        attestation,
        candidate,
        publication,
        candidate_handoff,
        candidate_output,
        publication_output,
    ) = (
        _public_soak_prerequisite_fixture(tmp_path)
    )
    final_output = candidate_output if command == "candidate" else publication_output
    args = ["--candidate-root", str(candidate), "--output", str(final_output)]
    if command == "publication":
        args = [
            "--candidate-root",
            str(candidate),
            "--candidate-handoff",
            str(candidate_handoff),
            "--publication-root",
            str(publication),
            "--output",
            str(final_output),
        ]
    captured_path: Path | None = None

    def dispatch(relative, child_args, run_as, external_tool_identity=None):
        nonlocal captured_path
        assert relative == "scripts/build_taira_public_v2_prerequisite_handoff.py"
        assert child_args[0] == command
        transformed = list(child_args[1 : 1 + len(args)])
        private_output = Path(transformed[transformed.index("--output") + 1])
        assert private_output.parent == Path(child_args[-1]).parent
        assert private_output.name == "public-soak-prerequisite-v1.json"
        transformed[transformed.index("--output") + 1] = str(final_output)
        assert transformed == args
        assert child_args[-2] == "--publisher-controller-attestation"
        captured_path = Path(child_args[-1])
        info = captured_path.stat()
        assert stat.S_IMODE(info.st_mode) == 0o400
        assert json.loads(captured_path.read_bytes()) == attestation
        assert run_as == (os.getuid(), os.getgid())
        assert external_tool_identity is None
        private_output.write_bytes(
            controller.canonical_json_bytes({"kind": command})
        )
        private_output.chmod(0o400)
        return 0

    monkeypatch.setattr(controller, "_dispatch_installed_python", dispatch)
    assert (
        controller._dispatch_public_soak_prerequisite_composite(
            operation, args, attestation
        )
        == 0
    )
    assert captured_path is not None and not captured_path.exists()
    assert final_output.is_dir()
    assert stat.S_IMODE(final_output.stat().st_mode) == 0o555
    assert {
        path.name for path in final_output.iterdir()
    } == {
        controller.HANDOFF_MANIFEST,
        "public-soak-prerequisite-v1.json",
    }


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        ("missing-literal", "exactly cover"),
        ("duplicate-literal", "duplicated or unsorted"),
        ("unsorted-literal", "duplicated or unsorted"),
        ("literal-operation-confusion", "operation or flag"),
        ("literal-flag-confusion", "operation or flag"),
        ("invalid-repository", "repository literal"),
        ("invalid-suffix", "suffix literal"),
        ("invalid-version", "ORAS version"),
        ("invalid-fingerprint", "fingerprint literal"),
        ("missing-publisher-input", "exactly cover"),
        ("missing-publisher-tool", "exactly cover"),
        ("tool-run-as-confusion", "run_as differs"),
        ("tool-digest-flag-confusion", "digest flag"),
    ),
)
def test_publisher_runner_trust_rejects_confusion_and_incomplete_sealing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
    message: str,
) -> None:
    launcher, trust = _publisher_trust(tmp_path)
    values = list(trust["trusted_values"])
    inputs = list(trust["trusted_inputs"])
    executables = list(trust["trusted_executables"])
    if mutation == "missing-literal":
        trust["trusted_values"] = values[:-1]
    elif mutation == "duplicate-literal":
        trust["trusted_values"] = [values[0], values[0], *values[1:]]
    elif mutation == "unsorted-literal":
        trust["trusted_values"] = list(reversed(values))
    elif mutation == "literal-operation-confusion":
        values[0] = {**values[0], "operation": "assemble-candidate"}
        trust["trusted_values"] = values
    elif mutation == "literal-flag-confusion":
        values[0] = {**values[0], "flag": "--registry-config"}
        trust["trusted_values"] = values
    elif mutation == "invalid-repository":
        values[1] = {**values[1], "value": "Registry.EXAMPLE/taira"}
        trust["trusted_values"] = values
    elif mutation == "invalid-suffix":
        values[2] = {**values[2], "value": "../latest"}
        trust["trusted_values"] = values
    elif mutation == "invalid-version":
        values[0] = {**values[0], "value": "latest"}
        trust["trusted_values"] = values
    elif mutation == "invalid-fingerprint":
        values[3] = {**values[3], "value": "F" * 64}
        trust["trusted_values"] = values
    elif mutation == "missing-publisher-input":
        trust["trusted_inputs"] = inputs[:-1]
    elif mutation == "missing-publisher-tool":
        trust["trusted_executables"] = executables[:-1]
    elif mutation == "tool-run-as-confusion":
        executables[0] = {**executables[0], "run_as": "runtime"}
        trust["trusted_executables"] = executables
    else:
        executables[0] = {
            **executables[0],
            "digest_flag": "--trusted-oras-sha256",
        }
        trust["trusted_executables"] = executables

    with pytest.raises(controller.ControllerSealError, match=message):
        _attest_trust_payload(
            tmp_path / "attempt",
            monkeypatch,
            launcher,
            trust,
            platform_name="macos",
            role="macos-publish",
        )


@pytest.mark.parametrize(
    ("mutation", "message"),
    (
        ("same-identity", "pairwise distinct"),
        ("swapped-caller", "sudo caller differs"),
        ("missing-executable", "exactly cover"),
        ("duplicate-executable", "duplicated or unsorted"),
        ("unsorted-executable", "duplicated or unsorted"),
        ("operation-confusion", "operation, flag"),
        ("flag-confusion", "operation, flag"),
        ("run-as-confusion", "operation, flag, digest flag, or run_as"),
        ("missing-input", "exactly cover"),
        ("extra-field", "fields differ"),
    ),
)
def test_runner_trust_v2_rejects_identity_and_record_confusion(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation: str,
    message: str,
) -> None:
    launcher, trust = _base_trust(tmp_path)
    original_staging_uid = int(trust["staging_uid"])
    original_staging_gid = int(trust["staging_gid"])
    executables = list(trust["trusted_executables"])
    inputs = list(trust["trusted_inputs"])
    if mutation == "same-identity":
        trust["runtime_uid"] = trust["staging_uid"]
    elif mutation == "swapped-caller":
        trust["staging_uid"], trust["runtime_uid"] = (
            trust["runtime_uid"],
            trust["staging_uid"],
        )
    elif mutation == "missing-executable":
        trust["trusted_executables"] = executables[:-1]
    elif mutation == "duplicate-executable":
        trust["trusted_executables"] = [executables[0], executables[0], executables[1]]
    elif mutation == "unsorted-executable":
        trust["trusted_executables"] = list(reversed(executables))
    elif mutation == "operation-confusion":
        executables[0] = {**executables[0], "operation": "snapshot-public-privacy"}
        trust["trusted_executables"] = executables
    elif mutation == "flag-confusion":
        executables[0] = {**executables[0], "flag": "--genesis-external-signer"}
        trust["trusted_executables"] = executables
    elif mutation == "run-as-confusion":
        executables[0] = {**executables[0], "run_as": "staging"}
        trust["trusted_executables"] = executables
    elif mutation == "missing-input":
        trust["trusted_inputs"] = inputs[:-1]
    else:
        trust["trusted_paths"] = [str(tmp_path)]
    with pytest.raises(controller.ControllerSealError, match=message):
        _attest_trust_payload(
            tmp_path / "attempt",
            monkeypatch,
            launcher,
            trust,
            sudo_uid=(original_staging_uid if mutation == "swapped-caller" else None),
            sudo_gid=(original_staging_gid if mutation == "swapped-caller" else None),
        )


def test_trusted_executable_requires_root_ancestry_stable_inode_and_digest(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    trusted = Path("/usr/bin/true")
    digest = hashlib.sha256(trusted.read_bytes()).hexdigest()
    controller._validate_trusted_executable_path(trusted, digest)
    with pytest.raises(controller.ControllerSealError, match="digest differs"):
        controller._validate_trusted_executable_path(trusted, "0" * 64)

    substituted = tmp_path / "true"
    substituted.write_bytes(trusted.read_bytes())
    substituted.chmod(0o500)
    with pytest.raises(controller.ControllerSealError, match="root-owned|ancestry"):
        controller._validate_trusted_executable_path(substituted, digest)

    def changed(_rows) -> None:
        raise controller.ControllerSealError("trusted path ancestry changed")

    monkeypatch.setattr(controller, "_revalidate_ancestry", changed)
    with pytest.raises(controller.ControllerSealError, match="ancestry changed"):
        controller._validate_trusted_executable_path(trusted, digest)


def test_kagemusha_release_root_requires_canonical_root_owned_ancestry(
    tmp_path: Path,
) -> None:
    assert controller._validate_root_owned_release_root(Path("/usr")) == Path("/usr")
    with pytest.raises(controller.ControllerSealError, match="filesystem root"):
        controller._validate_root_owned_release_root(Path("/"))
    for noncanonical in (Path("usr"), Path("/usr/../usr")):
        with pytest.raises(controller.ControllerSealError, match="absolute lexical"):
            controller._validate_root_owned_release_root(noncanonical)

    caller_owned = tmp_path / "release-root"
    caller_owned.mkdir(mode=0o755)
    with pytest.raises(
        controller.ControllerSealError, match="root-owned and nonwritable"
    ):
        controller._validate_root_owned_release_root(caller_owned)

    writable_system_root = (
        Path("/private/tmp") if Path("/private/tmp").is_dir() else Path("/tmp")
    )
    with pytest.raises(
        controller.ControllerSealError, match="root-owned and nonwritable"
    ):
        controller._validate_root_owned_release_root(writable_system_root)


@pytest.mark.parametrize(
    "operation_args",
    (
        ["--kagemusha-release-root", "/usr"],
        ["--kagemusha-activation-authority", "genesis-authority"],
    ),
)
def test_prepare_reset_requires_complete_kagemusha_flag_pair(
    tmp_path: Path, operation_args: list[str]
) -> None:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    with pytest.raises(controller.ControllerSealError, match="supplied together"):
        controller._validate_operation_args(
            "prepare-reset",
            operation_args,
            _attestation(handoff, trusted, "macos-qualification"),
        )


def _prepare_reset_controller_case(
    tmp_path: Path, role: str = "macos-qualification",
) -> tuple[list[str], dict[str, object], Path, Path, str]:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    attestation = _attestation(handoff, trusted, role)
    runtime_root = Path(str(attestation["runtime_root"]))
    source_bundle = runtime_root / "source-bundle"
    privacy_release = runtime_root / "privacy-release"
    source_bundle.mkdir(mode=0o700)
    privacy_release.mkdir(mode=0o700)
    controller_root = tmp_path / "installed-controller"
    controller_root.mkdir(mode=0o755)
    controller_manifest = controller_root / controller.MANIFEST_NAME
    controller_manifest.write_bytes(b"{}\n")
    attestation["controller_root"] = str(controller_root)
    attestation["trusted_inputs"] = [
        {
            "flag": "--source-bundle",
            "operation": "prepare-reset",
            "path": str(source_bundle),
        }
    ]
    controller_executable = Path("/usr/bin/false")
    controller_sha256 = hashlib.sha256(
        controller_executable.read_bytes()
    ).hexdigest()
    executable = Path("/usr/bin/true")
    executable_sha256 = hashlib.sha256(executable.read_bytes()).hexdigest()
    attestation["trusted_executables"] = [
        {
            "digest_flag": "--trusted-authenticated-tool-controller-sha256",
            "flag": "--authenticated-tool-controller",
            "operation": "prepare-reset",
            "path": str(controller_executable),
            "run_as": "runtime",
            "sha256": controller_sha256,
        },
        {
            "digest_flag": "--trusted-genesis-external-signer-sha256",
            "flag": "--genesis-external-signer",
            "operation": "prepare-reset",
            "path": str(executable),
            "run_as": "runtime",
            "sha256": executable_sha256,
        },
        {
            "digest_flag": None,
            "flag": "--onboarding-token-hash-tool",
            "operation": "prepare-reset",
            "path": str(executable),
            "run_as": "runtime",
            "sha256": executable_sha256,
        },
    ]
    output = runtime_root / "kagemusha-reset"
    args = [
        "--source-bundle", str(source_bundle),
        "--source-bundle-sha256", "1" * 64,
        "--privacy-release-dir", str(privacy_release),
        "--genesis-external-signer", str(executable),
        "--trusted-genesis-external-signer-sha256", executable_sha256,
        "--authenticated-tool-controller", str(controller_executable),
        "--trusted-authenticated-tool-controller-sha256", controller_sha256,
        "--onboarding-token-hash-tool", str(executable),
        "--irohad-sha256", "2" * 64,
        "--source-commit", "a" * 40,
        "--dpn-validator-release-commit", "b" * 40,
        "--cargo-lock-sha256", "3" * 64,
        "--workspace-source-manifest-sha256", "4" * 64,
        "--controller-manifest", str(controller_manifest),
        "--controller-digest", "5" * 64,
        "--output-bundle", str(output),
        "--kagemusha-release-root", "/usr",
        "--kagemusha-activation-authority", "genesis-authority",
    ]
    return args, attestation, output, controller_executable, controller_sha256


@pytest.mark.parametrize("role", ("macos-qualification", "macos-deploy"))
def test_prepare_reset_accepts_complete_kagemusha_pair_through_controller_schema(
    tmp_path: Path, role: str,
) -> None:
    args, attestation, output, controller_executable, controller_sha256 = (
        _prepare_reset_controller_case(tmp_path, role)
    )

    staged, outputs = controller._validate_operation_args(
        "prepare-reset", args, attestation
    )

    assert not staged
    assert outputs == {output}
    trusted_controller = controller._trusted_executable_for(
        attestation,
        "prepare-reset",
        "--authenticated-tool-controller",
        controller_executable,
    )
    assert trusted_controller == {
        "digest_flag": "--trusted-authenticated-tool-controller-sha256",
        "flag": "--authenticated-tool-controller",
        "operation": "prepare-reset",
        "path": str(controller_executable),
        "run_as": "runtime",
        "sha256": controller_sha256,
    }


@pytest.mark.parametrize("role", ("macos-qualification", "macos-deploy"))
@pytest.mark.parametrize(
    "omitted_flag",
    ("--kagemusha-release-root", "--kagemusha-activation-authority"),
)
def test_prepare_reset_rejects_omitted_kagemusha_pair_member(
    tmp_path: Path, role: str, omitted_flag: str
) -> None:
    args, attestation, _output, _controller_executable, _controller_sha256 = (
        _prepare_reset_controller_case(tmp_path, role)
    )
    index = args.index(omitted_flag)
    del args[index : index + 2]

    with pytest.raises(controller.ControllerSealError, match="supplied together"):
        controller._validate_operation_args("prepare-reset", args, attestation)


@pytest.mark.parametrize("role", ("macos-qualification", "macos-deploy"))
def test_prepare_reset_rejects_substituted_kagemusha_release_root(
    tmp_path: Path, role: str
) -> None:
    args, attestation, _output, _controller_executable, _controller_sha256 = (
        _prepare_reset_controller_case(tmp_path, role)
    )
    substituted = tmp_path / "caller-owned-kagemusha-release"
    substituted.mkdir(mode=0o755)
    args[args.index("--kagemusha-release-root") + 1] = str(substituted)

    with pytest.raises(
        controller.ControllerSealError, match="root-owned and nonwritable"
    ):
        controller._validate_operation_args("prepare-reset", args, attestation)


@pytest.mark.parametrize(
    "omitted_flag",
    (
        "--authenticated-tool-controller",
        "--trusted-authenticated-tool-controller-sha256",
    ),
)
def test_prepare_reset_rejects_omitted_authenticated_controller_pair_member(
    tmp_path: Path, omitted_flag: str
) -> None:
    args, attestation, _output, _controller_executable, _controller_sha256 = (
        _prepare_reset_controller_case(tmp_path)
    )
    index = args.index(omitted_flag)
    del args[index : index + 2]

    with pytest.raises(
        controller.ControllerSealError, match="mandatory options are absent"
    ):
        controller._validate_operation_args("prepare-reset", args, attestation)


@pytest.mark.parametrize(
    ("substituted_flag", "replacement", "message"),
    (
        (
            "--authenticated-tool-controller",
            "/usr/bin/true",
            "lacks one exact trusted executable record",
        ),
        (
            "--trusted-authenticated-tool-controller-sha256",
            "0" * 64,
            "trusted executable digest argument differs from trust",
        ),
    ),
)
def test_prepare_reset_rejects_substituted_authenticated_controller_pair_member(
    tmp_path: Path,
    substituted_flag: str,
    replacement: str,
    message: str,
) -> None:
    args, attestation, _output, _controller_executable, _controller_sha256 = (
        _prepare_reset_controller_case(tmp_path)
    )
    args[args.index(substituted_flag) + 1] = replacement

    with pytest.raises(controller.ControllerSealError, match=message):
        controller._validate_operation_args("prepare-reset", args, attestation)


def _assert_protected_prepare_reset_workflow_pins(workflow: str) -> None:
    contracts = (
        (
            "macos-secret-free-qualification",
            "macos-candidate-authority",
            "QUALIFICATION_KAGEMUSHA_RELEASE_ROOT",
            "TAIRA_QUALIFICATION_KAGEMUSHA_RELEASE_ROOT",
            "QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY",
            "TAIRA_QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY",
        ),
        (
            "macos-deploy",
            "macos-publish",
            "TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT",
            "TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT",
            "TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY",
            "TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY",
        ),
    )
    for (
        job,
        next_job,
        root_name,
        root_variable,
        authority_name,
        authority_variable,
    ) in contracts:
        section = workflow.split(f"  {job}:\n", 1)[1].split(f"  {next_job}:\n", 1)[0]
        assert section.count(
            f"{root_name}: ${{{{ vars.{root_variable} }}}}"
        ) == 1
        assert section.count(
            f"{authority_name}: ${{{{ vars.{authority_variable} }}}}"
        ) == 1
        assert section.count(
            f'if [[ -z "${root_name}" || -z "${authority_name}" ]]; then'
        ) == 1
        assert section.count(f'[[ "${root_name}" == /* ]]') == 1
        assert section.count(
            f'test "$(cd "${root_name}" && pwd -P)" = "${root_name}"'
        ) == 1
        assert section.count(f'--kagemusha-release-root "${root_name}"') == 1
        assert section.count(
            f'--kagemusha-activation-authority "${authority_name}"'
        ) == 1


def test_protected_prepare_reset_workflow_owns_all_authenticated_pins() -> None:
    workflow = (ROOT / ".github/workflows/publish_taira_validator.yml").read_text(
        encoding="utf-8"
    )

    assert workflow.count("prepare-reset --") == 2
    assert workflow.count(
        "TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH: "
        "${{ vars.TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH }}"
    ) == 2
    assert workflow.count(
        "TAIRA_AUTHENTICATED_TOOL_CONTROLLER_SHA256: "
        "${{ vars.TAIRA_AUTHENTICATED_TOOL_CONTROLLER_SHA256 }}"
    ) == 2
    assert workflow.count(
        '--authenticated-tool-controller "$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_PATH"'
    ) == 2
    assert workflow.count(
        "--trusted-authenticated-tool-controller-sha256 "
        '"$TAIRA_AUTHENTICATED_TOOL_CONTROLLER_SHA256"'
    ) == 2
    _assert_protected_prepare_reset_workflow_pins(workflow)


@pytest.mark.parametrize(
    ("original", "replacement"),
    (
        (
            "QUALIFICATION_KAGEMUSHA_RELEASE_ROOT: "
            "${{ vars.TAIRA_QUALIFICATION_KAGEMUSHA_RELEASE_ROOT }}",
            "QUALIFICATION_KAGEMUSHA_RELEASE_ROOT: "
            "${{ vars.SUBSTITUTED_KAGEMUSHA_RELEASE_ROOT }}",
        ),
        (
            "QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY: "
            "${{ vars.TAIRA_QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY }}",
            "QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY: "
            "${{ vars.SUBSTITUTED_KAGEMUSHA_ACTIVATION_AUTHORITY }}",
        ),
        (
            "TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT: "
            "${{ vars.TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT }}",
            "TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT: "
            "${{ vars.SUBSTITUTED_KAGEMUSHA_RELEASE_ROOT }}",
        ),
        (
            "TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY: "
            "${{ vars.TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY }}",
            "TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY: "
            "${{ vars.SUBSTITUTED_KAGEMUSHA_ACTIVATION_AUTHORITY }}",
        ),
        (
            '--kagemusha-release-root "$QUALIFICATION_KAGEMUSHA_RELEASE_ROOT"',
            '--kagemusha-release-root "$SUBSTITUTED_KAGEMUSHA_RELEASE_ROOT"',
        ),
        (
            '--kagemusha-activation-authority '
            '"$QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY"',
            '--kagemusha-activation-authority '
            '"$SUBSTITUTED_KAGEMUSHA_ACTIVATION_AUTHORITY"',
        ),
        (
            '--kagemusha-release-root "$TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT"',
            '--kagemusha-release-root "$SUBSTITUTED_KAGEMUSHA_RELEASE_ROOT"',
        ),
        (
            '--kagemusha-activation-authority '
            '"$TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY"',
            '--kagemusha-activation-authority '
            '"$SUBSTITUTED_KAGEMUSHA_ACTIVATION_AUTHORITY"',
        ),
        (
            'if [[ -z "$QUALIFICATION_KAGEMUSHA_RELEASE_ROOT" || -z '
            '"$QUALIFICATION_KAGEMUSHA_ACTIVATION_AUTHORITY" ]]; then',
            'if [[ -z "$QUALIFICATION_KAGEMUSHA_RELEASE_ROOT" ]]; then',
        ),
        (
            'if [[ -z "$TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT" || -z '
            '"$TAIRA_MACOS_KAGEMUSHA_ACTIVATION_AUTHORITY" ]]; then',
            'if [[ -z "$TAIRA_MACOS_KAGEMUSHA_RELEASE_ROOT" ]]; then',
        ),
    ),
)
def test_protected_prepare_reset_wiring_rejects_omission_or_substitution(
    original: str, replacement: str
) -> None:
    workflow = (ROOT / ".github/workflows/publish_taira_validator.yml").read_text(
        encoding="utf-8"
    )
    assert workflow.count(original) == 1

    for mutation in (
        workflow.replace(original, "", 1),
        workflow.replace(original, replacement, 1),
    ):
        with pytest.raises((AssertionError, IndexError)):
            _assert_protected_prepare_reset_workflow_pins(mutation)


def test_controller_has_no_generic_signing_or_direct_close_surface() -> None:
    all_operations = set(controller.PYTHON_OPERATIONS) | set(controller.BASH_OPERATIONS)
    assert "sign-manifest" not in all_operations
    assert "close-qualification-handoff" not in all_operations
    assert "sign-manifest" not in controller.ROLE_OPERATIONS["macos-publish"][1]
    assert "close-qualification-handoff" not in controller.ROLE_OPERATIONS[
        "macos-qualification"
    ][1]
    for forbidden in ("sign-manifest", "close-qualification-handoff"):
        with pytest.raises(SystemExit):
            controller.parse_args(
                [
                    "run",
                    "--expected-launcher-sha256",
                    "a" * 64,
                    "--expected-controller-digest",
                    "b" * 64,
                    "--expected-version",
                    "1",
                    "--expected-host-id",
                    "host",
                    "--expected-installation-id",
                    "install",
                    "--expected-uid",
                    "0",
                    "--source-commit",
                    "c" * 40,
                    "--platform",
                    "macos",
                    "--role",
                    "macos-publish",
                    forbidden,
                ]
            )


def test_publisher_operation_accepts_only_frozen_candidate_and_sealed_literals(
    tmp_path: Path,
) -> None:
    args, attestation, candidate = _publisher_operation_fixture(tmp_path)

    staged, outputs = controller._validate_operation_args(
        "publish-rollout", args, attestation
    )

    assert staged == {candidate}
    assert not outputs

    suffix_index = args.index("--suffix") + 1
    args[suffix_index] = ""
    for row in attestation["trusted_values"]:  # type: ignore[union-attr]
        if row["flag"] == "--suffix":
            row["value"] = ""
    staged, outputs = controller._validate_operation_args(
        "publish-rollout", args, attestation
    )
    assert staged == {candidate}
    assert not outputs


@pytest.mark.parametrize(
    "flag",
    (
        "--authority-uid",
        "--scratch-parent",
        "--registry-config",
        "--oras",
        "--trusted-oras-sha256",
        "--expected-oras-version",
        "--external-signer",
        "--trusted-external-signer-sha256",
        "--signing-public-key",
        "--trusted-signing-fingerprint",
        "--release-manifest-verifier",
        "--trusted-release-manifest-verifier-sha256",
        "--terminal-handoff",
        "--username",
        "--password",
        "--registry-config-json",
        "--layer",
        "--signature",
    ),
)
def test_publisher_operation_rejects_internal_trust_and_secret_injection(
    tmp_path: Path,
    flag: str,
) -> None:
    args, attestation, _candidate = _publisher_operation_fixture(tmp_path)

    with pytest.raises(controller.ControllerSealError, match="not allow-listed"):
        controller._validate_operation_args(
            "publish-rollout",
            [*args, flag, "/tmp/injected"],
            attestation,
        )


@pytest.mark.parametrize(
    ("flag", "value", "message"),
    (
        ("--repository", "registry.example/other", "differs from sealed trust"),
        ("--suffix", "production", "differs from sealed trust"),
        ("--expected-source-commit", "A" * 40, "installed attestation"),
        (
            "--expected-dpn-validator-release-commit",
            "short",
            "commit field",
        ),
        ("--expected-cargo-lock-sha256", "0" * 63, "digest field"),
        (
            "--expected-workspace-source-manifest-sha256",
            "g" * 64,
            "digest field",
        ),
        ("--expected-qualification-receipt-id", "1" * 65, "digest field"),
    ),
)
def test_publisher_operation_rejects_literal_and_source_identity_confusion(
    tmp_path: Path,
    flag: str,
    value: str,
    message: str,
) -> None:
    args, attestation, _candidate = _publisher_operation_fixture(tmp_path)
    args[args.index(flag) + 1] = value

    with pytest.raises(controller.ControllerSealError, match=message):
        controller._validate_operation_args(
            "publish-rollout", args, attestation
        )


@pytest.mark.parametrize("case", ("wrong-prefix", "nested", "lexical-alias"))
def test_publisher_operation_rejects_noncanonical_candidate_root(
    tmp_path: Path,
    case: str,
) -> None:
    args, attestation, candidate = _publisher_operation_fixture(tmp_path)
    if case == "wrong-prefix":
        replacement = candidate.parent / "candidate-123-1"
        _write_handoff(replacement, {"candidate": b"candidate"}, "candidate")
        _freeze_handoff(replacement)
    elif case == "nested":
        container = candidate.parent / "container"
        container.mkdir(mode=0o700)
        replacement = container / "publish-candidate-123-1"
        _write_handoff(replacement, {"candidate": b"candidate"}, "candidate")
        _freeze_handoff(container)
    else:
        replacement = candidate.parent / ".." / candidate.parent.name / candidate.name
    args[args.index("--candidate-root") + 1] = str(replacement)

    with pytest.raises(controller.ControllerSealError):
        controller._validate_operation_args(
            "publish-rollout", args, attestation
        )


def test_boi_composite_refuses_before_scratch_dispatch_or_candidate_probe(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    handoff = tmp_path / "handoff"
    trusted = tmp_path / "trusted"
    handoff.mkdir(mode=0o711)
    trusted.mkdir(mode=0o700)
    attestation = _attestation(handoff, trusted, "linux-boi-qualification")
    final = handoff / "boi-qualified-123-1"
    args = [
        "--artifact-handoff-root", "/frozen/artifacts",
        "--candidate-archive", "/frozen/candidate.tar.gz",
        "--candidate-authority-dir", "/frozen/authority",
        "--expected-source-commit", "1" * 40,
        "--expected-dpn-validator-release-commit", "2" * 40,
        "--expected-cargo-lock-sha256", "3" * 64,
        "--expected-workspace-source-manifest-sha256", "4" * 64,
        "--expected-receipt-id", "5" * 64,
        "--trusted-signing-fingerprint", "6" * 64,
        "--release-manifest-verifier", "/trusted/verifier",
        "--trusted-release-manifest-verifier-sha256", "7" * 64,
        "--output", str(final),
    ]
    forbidden_calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            forbidden_calls.append(name)
            raise AssertionError(f"BOI barrier reached forbidden operation: {name}")

        return call

    monkeypatch.setattr(controller.tempfile, "mkdtemp", forbidden("mkdtemp"))
    monkeypatch.setattr(controller, "_dispatch", forbidden("dispatch"))
    monkeypatch.setattr(controller, "inspect_handoff", forbidden("inspect"))

    with pytest.raises(
        controller.ControllerSealError,
        match="missing preprovisioned iroha.taira.boi-native-isolation-broker.v1",
    ) as error:
        controller._dispatch_boi_composite(args, attestation)

    assert controller.BOI_QUALIFICATION_RUN_BINDING_CONTRACT in str(error.value)
    assert forbidden_calls == []
    assert not final.exists()


@pytest.mark.parametrize(
    ("operation", "platform_name", "role", "apply", "contracts"),
    (
        (
            "assemble-boi",
            "linux",
            "linux-boi-qualification",
            False,
            (
                controller.BOI_QUALIFICATION_ISOLATION_CONTRACT,
                controller.BOI_QUALIFICATION_RUN_BINDING_CONTRACT,
                controller.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT,
            ),
        ),
        (
            "deploy-reset",
            "macos",
            "macos-deploy",
            False,
            (
                controller.DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT,
                controller.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT,
            ),
        ),
        (
            "deploy-reset",
            "macos",
            "macos-deploy",
            True,
            (
                controller.DEPLOY_AUTHENTICATED_RUN_NONCE_CONTRACT,
                controller.COMPLETE_SOURCE_IDENTITY_ATTESTATION_CONTRACT,
            ),
        ),
    ),
)
def test_installed_issuance_barriers_precede_attestation_paths_and_dispatch(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    operation: str,
    platform_name: str,
    role: str,
    apply: bool,
    contracts: tuple[str, ...],
) -> None:
    calls: list[str] = []

    def forbidden(name: str):
        def call(*_args, **_kwargs):
            calls.append(name)
            raise AssertionError(
                f"installed issuance barrier reached forbidden operation: {name}"
            )

        return call

    for name in (
        "_attest",
        "_validate_operation_args",
        "_dispatch",
        "_dispatch_boi_composite",
        "_revalidate_staged_roots",
        "_revalidate_bound_roots",
    ):
        monkeypatch.setattr(controller, name, forbidden(name))
    monkeypatch.setattr(controller.os, "geteuid", forbidden("geteuid"))
    state = tmp_path / "release-state"
    state.write_bytes(b"unchanged\n")
    argv = [
        "run",
        "--expected-launcher-sha256",
        "a" * 64,
        "--expected-controller-digest",
        "b" * 64,
        "--expected-version",
        "1",
        "--expected-host-id",
        "host-v1",
        "--expected-installation-id",
        "installation-v1",
        "--expected-uid",
        "0",
        "--source-commit",
        "c" * 40,
        "--platform",
        platform_name,
        "--role",
        role,
        operation,
        "--",
        "--bundle",
        str(state),
    ]
    if apply:
        argv.append("--apply")

    assert controller.main(argv) == 1

    error = capsys.readouterr().err
    for contract_name in contracts:
        assert contract_name in error
    assert calls == []
    assert state.read_bytes() == b"unchanged\n"


def _configure_publisher_composite_attestation(
    tmp_path: Path,
    attestation: dict[str, object],
) -> tuple[dict[str, Path], Path, Path]:
    authority = Path(str(attestation["authority_root"]))
    trusted = tmp_path / "trusted"
    registry_config = authority / "registry-config.json"
    registry_config.write_bytes(b'{"auths":{}}\n')
    registry_config.chmod(0o400)
    signing_key = trusted / "publication.pub"
    signing_key.write_bytes(b"k" * 32)
    signing_key.chmod(0o444)
    tools = {
        "--external-signer": trusted / "signer",
        "--oras": trusted / "oras",
        "--release-manifest-verifier": trusted / "verifier",
    }
    for flag, path in tools.items():
        path.write_bytes(flag.encode("ascii"))
        path.chmod(0o555)
    attestation["trusted_inputs"] = [
        {
            "flag": "--registry-config",
            "operation": "publish-rollout",
            "path": str(registry_config),
        },
        {
            "flag": "--signing-public-key",
            "operation": "publish-rollout",
            "path": str(signing_key),
        },
    ]
    attestation["trusted_executables"] = [
        {
            "digest_flag": controller.SEALED_EXECUTABLE_DEPENDENCIES[
                "publish-rollout"
            ][flag],
            "flag": flag,
            "operation": "publish-rollout",
            "path": str(path),
            "run_as": "authority",
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        }
        for flag, path in sorted(tools.items())
    ]
    return tools, registry_config, signing_key


def test_publication_composite_injects_only_sealed_authority_dependencies(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    args, attestation, candidate = _publisher_operation_fixture(tmp_path)
    tools, registry_config, signing_key = (
        _configure_publisher_composite_attestation(tmp_path, attestation)
    )
    calls: list[tuple[str, list[str], object, object]] = []

    def fake_dispatch(
        operation: str,
        child_args,
        run_as=None,
        external_tool_identity=None,
    ) -> int:
        values = list(child_args)
        terminal = Path(values[values.index("--terminal-handoff") + 1])
        terminal.mkdir(mode=0o555)
        calls.append((operation, values, run_as, external_tool_identity))
        return 0

    def fake_close(relative: str, child_args, run_as) -> int:
        values = list(child_args)
        output = Path(str(attestation["handoff_root"])) / (
            "publication-receipt-" + "e" * 64
        )
        output.mkdir(mode=0o555)
        calls.append((relative, values, run_as, None))
        return 0

    monkeypatch.setattr(
        controller,
        "_require_authenticated_rollout_observation_authority",
        lambda: None,
    )
    monkeypatch.setattr(controller, "_dispatch", fake_dispatch)
    monkeypatch.setattr(controller, "_dispatch_installed_python", fake_close)
    monkeypatch.setattr(
        controller, "_validate_operation_outputs", lambda *_args, **_kwargs: None
    )
    monkeypatch.setattr(
        controller,
        "_validate_publisher_trusted_input",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        controller,
        "_validate_trusted_executable_path",
        lambda *_args, **_kwargs: None,
    )

    assert controller._dispatch_publication_composite(args, attestation) == 0

    operation, publisher_args, run_as, external_identity = calls[0]
    assert operation == "publish-rollout"
    assert run_as == (os.getuid(), os.getgid())
    assert external_identity == (os.getuid(), os.getgid())
    _subcommand, publisher_values = controller._operation_option_values(
        "publish-rollout", publisher_args
    )
    assert publisher_values["--candidate-root"] == [str(candidate)]
    assert publisher_values["--registry-config"] == [str(registry_config)]
    assert publisher_values["--signing-public-key"] == [str(signing_key)]
    for flag, path in tools.items():
        assert publisher_values[flag] == [str(path)]
    assert publisher_values["--repository"] == [
        "registry.example/taira/release"
    ]
    assert publisher_values["--suffix"] == ["testnet"]
    assert publisher_values["--expected-oras-version"] == ["1.2.3"]
    assert publisher_values["--trusted-signing-fingerprint"] == ["f" * 64]

    close_relative, close_args, close_run_as, _unused = calls[1]
    assert close_relative == controller.PUBLICATION_CLOSE_HELPER
    assert close_run_as is None
    _subcommand, close_values = controller._operation_option_values(
        "publish-rollout", close_args
    )
    for flag in (
        "--expected-source-commit",
        "--expected-dpn-validator-release-commit",
        "--expected-cargo-lock-sha256",
        "--expected-workspace-source-manifest-sha256",
        "--expected-qualification-receipt-id",
    ):
        assert close_values[flag] == [args[args.index(flag) + 1]]
    assert "--output" not in close_values
    assert not any(
        path.name.startswith(".publish-rollout-")
        for path in Path(str(attestation["authority_root"])).iterdir()
    )


@pytest.mark.parametrize("failure", ("publisher", "close"))
def test_publication_composite_cleans_private_scratch_and_partial_final(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure: str,
) -> None:
    args, attestation, _candidate = _publisher_operation_fixture(tmp_path)
    _configure_publisher_composite_attestation(tmp_path, attestation)
    final = Path(str(attestation["handoff_root"])) / (
        "publication-receipt-" + "e" * 64
    )

    def fake_dispatch(_operation, child_args, *_args) -> int:
        values = list(child_args)
        terminal = Path(values[values.index("--terminal-handoff") + 1])
        terminal.mkdir(mode=0o555)
        return 1 if failure == "publisher" else 0

    def fake_close(_relative, _child_args, _run_as) -> int:
        final.mkdir(mode=0o555)
        return 1

    monkeypatch.setattr(
        controller,
        "_require_authenticated_rollout_observation_authority",
        lambda: None,
    )
    monkeypatch.setattr(controller, "_dispatch", fake_dispatch)
    monkeypatch.setattr(controller, "_dispatch_installed_python", fake_close)
    monkeypatch.setattr(
        controller, "_validate_operation_outputs", lambda *_args, **_kwargs: None
    )
    monkeypatch.setattr(
        controller,
        "_validate_publisher_trusted_input",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        controller,
        "_validate_trusted_executable_path",
        lambda *_args, **_kwargs: None,
    )

    assert controller._dispatch_publication_composite(args, attestation) == 1
    assert not final.exists()
    assert not any(
        path.name.startswith(".publish-rollout-")
        for path in Path(str(attestation["authority_root"])).iterdir()
    )


def test_capture_schema_rejects_caller_receipt_and_runtime_root_injection() -> None:
    for flag in ("--receipt", "--runtime-root"):
        with pytest.raises(controller.ControllerSealError, match="not allow-listed"):
            controller._validate_operation_args(
                "capture-four-peer",
                [flag, "/tmp/fabricated"],
                {
                    "handoff_root": "/tmp",
                    "role": "macos-qualification",
                    "authority_gid": 503,
                    "authority_root": "/authority",
                    "authority_uid": 503,
                    "runtime_gid": 502,
                    "runtime_root": "/runtime",
                    "runtime_uid": 502,
                    "staging_gid": 501,
                    "staging_root": "/staging",
                    "staging_uid": 501,
                    "trusted_executables": [],
                    "trusted_inputs": [],
                    "trusted_values": [],
                    "uid": 0,
                },
            )


def test_capture_composite_replaces_output_and_never_accepts_receipt_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    handoff = tmp_path / "handoff"
    runtime = tmp_path / "runtime"
    handoff.mkdir(mode=0o711)
    runtime.mkdir(mode=0o700)
    identity = tmp_path / "taira-source-identity-v1.json"
    identity.write_bytes(b"identity")
    inputs = {
        name: tmp_path / name
        for name in (
            "exact12.tsv",
            "iroha-core-tests",
            "iroha3d",
            "linux.tar.gz",
            "network-functional",
            "privacy-exact12-action-driver",
        )
    }
    for path in inputs.values():
        path.write_bytes(b"input")
    final_output = handoff / "qualification-receipt-123-1"
    attestation = {
        "controller_gid": os.getegid(),
        "handoff_root": str(handoff),
        "runtime_gid": os.getegid(),
        "runtime_root": str(runtime),
        "runtime_uid": os.geteuid(),
        "uid": os.geteuid(),
    }
    calls: list[tuple[str, list[str], object]] = []

    def fake_dispatch(operation: str, args, run_as=None) -> int:
        values = list(args)
        receipt = Path(values[values.index("--output") + 1])
        assert receipt != final_output
        assert "--runtime-root" in values
        assert "--source-identity" not in values
        receipt.write_bytes(b"controller-owned-receipt")
        calls.append((operation, values, run_as))
        return 0

    def fake_close(relative: str, args, run_as) -> int:
        values = list(args)
        if relative == controller.PRIVACY_CAPTURE_HELPER:
            output = Path(values[values.index("--output-directory") + 1])
            output.mkdir(mode=0o700)
            (output / "evidence.json").write_bytes(b"evidence")
            calls.append((relative, values, run_as))
            return 0
        output = Path(values[values.index("--output") + 1])
        receipt = Path(values[values.index("--receipt") + 1])
        assert receipt.parent.name == "runtime-work"
        privacy = Path(
            values[values.index("--privacy-protocol-evidence-dir") + 1]
        )
        assert privacy.parent == receipt.parent
        output.mkdir(mode=0o700)
        (output / "handoff-inventory-v1.json").write_bytes(b"closed")
        calls.append((relative, values, run_as))
        return 0

    monkeypatch.setattr(controller, "_dispatch", fake_dispatch)
    monkeypatch.setattr(controller, "_dispatch_installed_python", fake_close)
    result = controller._dispatch_capture_composite(
        [
            "--source-identity",
            str(identity),
            "--validator-binary",
            str(inputs["iroha3d"]),
            "--privacy-action-driver",
            str(inputs["privacy-exact12-action-driver"]),
            "--privacy-network-driver",
            str(inputs["network-functional"]),
            "--privacy-jindo-driver",
            str(inputs["iroha-core-tests"]),
            "--linux-archive",
            str(inputs["linux.tar.gz"]),
            "--exact12-matrix",
            str(inputs["exact12.tsv"]),
            "--artifact-handoff-sha256",
            "a" * 64,
            "--output",
            str(final_output),
        ],
        attestation,
    )
    assert result == 0
    assert final_output.is_dir()
    assert calls[0][0] == controller.PRIVACY_CAPTURE_HELPER
    assert calls[0][2] == (os.geteuid(), os.getegid())
    assert calls[1][0] == "capture-four-peer"
    assert calls[1][2] == (os.geteuid(), os.getegid())
    assert calls[2][0] == controller.QUALIFICATION_CLOSE_HELPER
    assert calls[2][2] is None
    assert not any(path.name.startswith(".qualification-capture-") for path in handoff.iterdir())


def test_child_environment_never_forwards_workflow_or_registry_secrets(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("TAIRA_OCI_PASSWORD", "poison")
    monkeypatch.setenv("GITHUB_TOKEN", "poison")
    monkeypatch.setenv("TAIRA_RELEASE_EXTERNAL_SIGNER_PATH", "/poison")
    environment = controller._child_environment()
    assert set(environment) == {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"}
    assert environment["PATH"] == "/usr/bin:/bin:/usr/sbin:/sbin"
    assert "poison" not in environment.values()


def test_root_deploy_child_receives_only_the_sealed_external_tool_identity(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("GITHUB_TOKEN", "poison")
    monkeypatch.setenv("IROHA_TAIRA_EXTERNAL_TOOL_UID", "999")
    captured: dict[str, object] = {}

    def fake_run(command, **kwargs):
        captured["command"] = command
        captured.update(kwargs)
        return type("Result", (), {"returncode": 0})()

    monkeypatch.setattr(controller.subprocess, "run", fake_run)
    assert controller._dispatch_command(
        ["/usr/bin/true"],
        None,
        (41, 42),
    ) == 0

    assert captured["env"] == {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "TMPDIR": "/var/tmp",
        "IROHA_TAIRA_EXTERNAL_TOOL_UID": "41",
        "IROHA_TAIRA_EXTERNAL_TOOL_GID": "42",
    }
    assert captured["preexec_fn"] is None
    assert captured["close_fds"] is True
    assert captured["pass_fds"] == ()


@pytest.mark.parametrize("identity", [(0, 42), (41, 0), (-1, 42), (41, -1)])
def test_child_environment_rejects_nonpositive_external_tool_identity(
    identity: tuple[int, int],
) -> None:
    with pytest.raises(controller.ControllerSealError, match="identity is invalid"):
        controller._child_environment(identity)


def test_descriptor_reader_rejects_fifo_device_and_post_read_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "root"
    root.mkdir(mode=0o700)
    fifo = root / "fifo"
    os.mkfifo(fifo, mode=0o600)
    with pytest.raises(controller.ControllerSealError, match="bounded single-link"):
        controller._read_relative_stable(root, "fifo", 1024)

    device = Path("/dev/null")
    if device.exists() and device.parent.is_dir():
        with pytest.raises(
            controller.ControllerSealError,
            match="bounded single-link",
        ):
            controller._read_relative_stable(device.parent, device.name, 1024)

    fifo.unlink()
    payload = b"descriptor-pinned-payload"
    target = root / "payload"
    target.write_bytes(payload)
    real_read = controller.os.read
    replaced = False

    def replacing_read(descriptor: int, size: int) -> bytes:
        nonlocal replaced
        chunk = real_read(descriptor, size)
        if chunk and not replaced:
            replacement = root / "replacement"
            replacement.write_bytes(payload)
            replacement.replace(target)
            replaced = True
        return chunk

    monkeypatch.setattr(controller.os, "read", replacing_read)
    with pytest.raises(controller.ControllerSealError, match="changed"):
        controller._read_relative_stable(root, "payload", 1024)


@pytest.mark.parametrize("mutation", ("symlink", "hardlink", "fifo", "device"))
def test_operation_output_scan_rejects_aliases_and_special_files(
    tmp_path: Path,
    mutation: str,
) -> None:
    output = tmp_path / "output"
    output.mkdir(mode=0o700)
    target = output / "target"
    if mutation == "symlink":
        outside = tmp_path / "outside"
        outside.write_bytes(b"outside")
        target.symlink_to(outside)
    elif mutation == "hardlink":
        target.write_bytes(b"hard-linked")
        os.link(target, output / "alias")
    elif mutation == "fifo":
        os.mkfifo(target, mode=0o600)
    else:
        output = Path("/dev/null")

    expected_uid = 0 if mutation == "device" else os.geteuid()
    with pytest.raises(controller.ControllerSealError):
        controller._validate_operation_outputs({output}, expected_uid)


def test_operation_output_scan_detects_same_byte_inode_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output = tmp_path / "output"
    output.write_bytes(b"signed-output")
    real_read = controller.os.read
    replaced = False

    def replacing_read(descriptor: int, size: int) -> bytes:
        nonlocal replaced
        chunk = real_read(descriptor, size)
        if chunk and not replaced:
            replacement = tmp_path / "replacement"
            replacement.write_bytes(b"signed-output")
            replacement.replace(output)
            replaced = True
        return chunk

    monkeypatch.setattr(controller.os, "read", replacing_read)
    with pytest.raises(controller.ControllerSealError, match="changed"):
        controller._validate_operation_outputs({output}, os.geteuid())


def test_staged_handoff_rejects_mode_only_mutation(tmp_path: Path) -> None:
    source = tmp_path / "source"
    staging = tmp_path / "staging"
    staging.mkdir(mode=0o711)
    _write_handoff(source, {"bin/iroha3d": b"reviewed-validator"})
    result = controller.inspect_handoff(
        source,
        "test-handoff",
        staging,
        os.geteuid(),
        "mode-only",
    )
    stage = Path(str(result["staged_root"]))
    (stage / "bin/iroha3d").chmod(0o644)
    with pytest.raises(controller.ControllerSealError):
        controller._revalidate_staged_roots(
            staging,
            {stage},
            expected_owner=os.geteuid(),
            expected_group=os.getegid(),
        )


def test_real_dispatch_closes_inheritable_fds_and_fixes_process_context(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    root = tmp_path / "controller-root"
    root.mkdir(mode=0o700)
    probe = root / "probe.py"
    probe.write_text(
        "import json, os, sys\n"
        "output, raw_fd = sys.argv[1], int(sys.argv[2])\n"
        "try:\n"
        "    os.fstat(raw_fd)\n"
        "    inherited = True\n"
        "except OSError:\n"
        "    inherited = False\n"
        "value = {\n"
        "    'cwd': os.getcwd(),\n"
        "    'env': sorted(os.environ),\n"
        "    'fd_inherited': inherited,\n"
        "    'stdin_closed': os.read(0, 1) == b'',\n"
        "}\n"
        "with open(output, 'x', encoding='ascii') as handle:\n"
        "    json.dump(value, handle, sort_keys=True)\n",
        encoding="ascii",
    )
    output = tmp_path / "dispatch.json"
    secret = tmp_path / "secret"
    secret.write_bytes(b"must-not-cross-exec")
    original = os.open(secret, os.O_RDONLY)
    inherited_fd = fcntl.fcntl(original, fcntl.F_DUPFD, 200)
    os.close(original)
    os.set_inheritable(inherited_fd, True)
    monkeypatch.setattr(controller, "CONTROLLER_ROOT", root)
    monkeypatch.setitem(
        controller.PYTHON_OPERATIONS,
        "snapshot-public-privacy",
        probe.name,
    )
    monkeypatch.setenv("TAIRA_OCI_PASSWORD", "must-not-cross-env")
    try:
        assert controller._dispatch(
            "snapshot-public-privacy",
            [str(output), str(inherited_fd)],
        ) == 0
    finally:
        os.close(inherited_fd)

    value = json.loads(output.read_bytes())
    assert value["cwd"] == str(root)
    assert value["fd_inherited"] is False
    assert value["stdin_closed"] is True
    assert set(value["env"]) == {"HOME", "LANG", "LC_ALL", "PATH", "TMPDIR"}
