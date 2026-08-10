from __future__ import annotations

import base64
import functools
import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Any

import pytest

from scripts import snapshot_taira_public_privacy_inputs as snapshotter


REPO_ROOT = Path(snapshotter.__file__).resolve().parents[1]
TAIRA_CONFIG = REPO_ROOT / "configs/soranexus/taira/config.toml"
TAIRA_GENESIS = REPO_ROOT / "configs/soranexus/taira/genesis.json"
TAIRA_PLAN = REPO_ROOT / "configs/soranexus/taira/privacy_bootstrap_plan.json"
TEST_NETWORK_ID = (
    "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
)
FOREIGN_NETWORK_ID = (
    "hash:A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A4A5#E8B5"
)


def _sha256(label: bytes) -> str:
    return hashlib.sha256(label).hexdigest()


def _pretty_json(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            indent=2,
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def _compact_json(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        )
        + "\n"
    ).encode("utf-8")


def _release_config(provider_digest: str) -> bytes:
    lines = [
        line
        for line in TAIRA_CONFIG.read_text(encoding="utf-8").splitlines()
        if not line.lstrip().startswith("#")
    ]
    section = lines.index("[torii.privacy_bootle_lantern_issuer]")
    section_end = next(
        index
        for index in range(section + 1, len(lines))
        if lines[index].startswith("[")
    )
    body = lines[section + 1 : section_end]
    while body and not body[-1]:
        body.pop()
    body = ["enabled = true" if line == "enabled = false" else line for line in body]
    body.extend(
        [
            f'issuer_id_hex = "{snapshotter.ISSUER_ID}"',
            f'policy_id_hex = "{snapshotter.POLICY_ID}"',
            f'runtime_provider_registry_handle = "{snapshotter.PROVIDER_HANDLE}"',
            "runtime_provider_registry_revision = 1",
            f'runtime_provider_registry_policy_digest_hex = "{provider_digest}"',
        ]
    )
    lines = lines[: section + 1] + body + [""] + lines[section_end:]
    normalized: list[str] = []
    for line in lines:
        if line or not normalized or normalized[-1]:
            normalized.append(line)
    while normalized and not normalized[-1]:
        normalized.pop()
    return ("\n".join(normalized) + "\n").encode("utf-8")


@functools.lru_cache(maxsize=1)
def _release_payloads() -> dict[str, bytes]:
    policy_instruction = b"issuer-policy-public-instruction"
    provider_digest = _sha256(b"provider-policy")
    parameter_id = _sha256(b"issuer-parameter-id")
    parameter_digest = _sha256(b"issuer-parameter")
    record_digest = _sha256(b"issuer-record")
    instruction_digest = hashlib.sha256(policy_instruction).hexdigest()
    broker = {
        "schema": "iroha.taira.privacy.bootle-lantern-broker-public.v1",
        "chain_id": snapshotter.CHAIN_ID,
        "network_id": TEST_NETWORK_ID,
        "runtime_provider_handle": snapshotter.PROVIDER_HANDLE,
        "runtime_provider_revision": 1,
        "runtime_provider_policy_digest_hex": provider_digest,
        "issuer_id_hex": snapshotter.ISSUER_ID,
        "policy_id_hex": snapshotter.POLICY_ID,
        "authorization_lifetime_blocks": 300,
        "issuer_parameter_id_hex": parameter_id,
        "issuer_parameter_digest_hex": parameter_digest,
        "policy_record_digest_hex": record_digest,
        "stable_principal_digest_hex": _sha256(b"principal"),
        "issuer_profile_digest_hex": _sha256(b"profile"),
        "broker_contract_digest_hex": _sha256(b"contract"),
        "registration_instruction_norito_hex": policy_instruction.hex(),
        "registration_instruction_norito_sha256": instruction_digest,
        "registration_instruction": {
            "policy": {
                "issuer_id": list(bytes.fromhex(snapshotter.ISSUER_ID)),
                "policy_id": list(bytes.fromhex(snapshotter.POLICY_ID)),
                "epoch": 1,
                "lifecycle": {"state": "active", "value": None},
                "issuer_parameter_id": list(bytes.fromhex(parameter_id)),
                "issuer_parameter_digest": list(bytes.fromhex(parameter_digest)),
                "issuer_public_matrix": {
                    "entries": [
                        {"coefficients": [1] * 64} for _ in range(64)
                    ]
                },
                "required_disclosure_bitmap": 0,
                "allowed_values": [{"values": []} for _ in range(8)],
                "record_digest": list(bytes.fromhex(record_digest)),
            }
        },
    }
    broker_payload = _compact_json(broker)

    plan = json.loads(TAIRA_PLAN.read_bytes())
    plan["network_id"] = TEST_NETWORK_ID
    bootle = plan["bootle_lantern_issuer"]
    bootle["public_export_sha256"] = hashlib.sha256(broker_payload).hexdigest()
    bootle["runtime_provider"]["qualification_policy_digest_hex"] = provider_digest
    bootle["governed_issuer_policy"].update(
        {
            "instruction_norito_sha256": instruction_digest,
            "issuer_parameter_id_hex": parameter_id,
            "issuer_parameter_digest_hex": parameter_digest,
            "record_digest_hex": record_digest,
        }
    )

    genesis = json.loads(TAIRA_GENESIS.read_bytes())
    return {
        "bootle_lantern_broker_public.json": broker_payload,
        "config.toml": _release_config(provider_digest),
        "genesis.json": _pretty_json(genesis),
        "privacy_bootstrap_plan.json": _pretty_json(plan),
    }


def _source(tmp_path: Path) -> Path:
    source = tmp_path / "public-source"
    source.mkdir(mode=0o700, parents=True)
    for name, payload in _release_payloads().items():
        path = source / name
        path.write_bytes(payload)
        path.chmod(0o600)
    return source


def _workspace(tmp_path: Path) -> Path:
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    return workspace


def _assert_rejected_without_output(
    source: Path,
    tmp_path: Path,
    *,
    match: str | None = None,
) -> None:
    output = tmp_path / "handoff"
    with pytest.raises(snapshotter.SnapshotError, match=match):
        snapshotter.snapshot(source, output, _workspace(tmp_path))
    assert not output.exists()


def _load_source_json(source: Path, name: str) -> dict[str, Any]:
    return json.loads((source / name).read_bytes())


def _write_source_json(
    source: Path,
    name: str,
    value: dict[str, Any],
    *,
    broker: bool = False,
    rebind_broker: bool = False,
) -> None:
    payload = _compact_json(value) if broker else _pretty_json(value)
    (source / name).write_bytes(payload)
    if rebind_broker:
        plan = _load_source_json(source, "privacy_bootstrap_plan.json")
        plan["bootle_lantern_issuer"]["public_export_sha256"] = hashlib.sha256(
            payload
        ).hexdigest()
        (source / "privacy_bootstrap_plan.json").write_bytes(_pretty_json(plan))


def _mode(path: Path) -> int:
    return path.stat().st_mode & 0o777


def test_snapshot_emits_one_immutable_exact_four_file_handoff(tmp_path: Path) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    output = tmp_path / "handoff"

    result = snapshotter.snapshot(source, output, workspace)

    assert result["kind"] == "public-privacy-input"
    assert [row["path"] for row in result["files"]] == [
        f"inputs/{name}" for name in sorted(snapshotter.EXPECTED)
    ]
    assert _mode(output) == 0o555
    assert _mode(output / "inputs") == 0o555
    assert _mode(output / snapshotter.HANDOFF_MANIFEST) == 0o444
    for name in snapshotter.EXPECTED:
        assert (output / "inputs" / name).read_bytes() == (source / name).read_bytes()
        assert _mode(output / "inputs" / name) == 0o444
    assert json.loads((output / snapshotter.HANDOFF_MANIFEST).read_bytes()) == result


def test_embedded_projection_fingerprints_match_canonical_templates() -> None:
    assert snapshotter.tomllib is not None
    config = snapshotter.tomllib.loads(TAIRA_CONFIG.read_text(encoding="utf-8"))
    base_config = {
        "root_without_torii": {
            key: value for key, value in config.items() if key != "torii"
        },
        "torii_without_privacy_issuer": {
            key: value
            for key, value in config["torii"].items()
            if key != "privacy_bootle_lantern_issuer"
        },
    }
    assert hashlib.sha256(
        snapshotter._semantic_bytes(base_config, "test config")
    ).hexdigest() == snapshotter.CONFIG_PUBLIC_BASE_SHA256

    genesis = json.loads(TAIRA_GENESIS.read_bytes())
    assert hashlib.sha256(
        snapshotter._semantic_bytes(genesis, "test genesis")
    ).hexdigest() == snapshotter.GENESIS_PUBLIC_BASE_SHA256
    plan = json.loads(TAIRA_PLAN.read_bytes())
    assert tuple(
        (row["index"], row["label"], row["statement_type"])
        for row in plan["privacy_catalog"]["protocols"]
    ) == snapshotter.PROTOCOLS
    assert tuple(plan["privacy_catalog"]["retired_labels"]) == (
        snapshotter.RETIRED_LABELS
    )


@pytest.mark.parametrize(
    ("placeholder", "secret"),
    (
        (
            'private_key_file = "/run/secrets/iroha/taira-validator-private-key"',
            'private_key = "ed0120materialized-validator-key"',
        ),
        (
            "REPLACE_WITH_SORANET_TRANSPORT_PUBLIC_KEY",
            "ed0120materialized-soranet-transport-public-key",
        ),
        (
            'soranet_transport_private_key_file = "/run/secrets/iroha/taira-soranet-transport-private-key"',
            'soranet_transport_private_key = "802620materialized-soranet-transport-private-key"',
        ),
        (
            'private_key_file = "/run/secrets/iroha/taira-kagemusha-commands-private-key"',
            'private_key = "ed0120materialized-kagemusha-key"',
        ),
        (
            "REPLACE_WITH_TAIRA_ONBOARDING_PRIVATE_KEY_FILE",
            "/run/secrets/onboarding.key",
        ),
        (
            "REPLACE_WITH_TAIRA_ONBOARDING_TOKEN_HASH",
            hashlib.sha256(b"real bearer token").hexdigest(),
        ),
        (
            "REPLACE_WITH_TAIRA_FAUCET_PRIVATE_KEY_FILE",
            "/run/secrets/faucet.key",
        ),
        (
            'identity_private_key_file = "/run/secrets/iroha/taira-streaming-identity-private-key"',
            'identity_private_key = "ed0120materialized-streaming-key"',
        ),
    ),
)
def test_snapshot_rejects_every_materialized_secret_class_before_output(
    tmp_path: Path,
    placeholder: str,
    secret: str,
) -> None:
    source = _source(tmp_path)
    config = source / "config.toml"
    text = config.read_text(encoding="utf-8")
    assert text.count(placeholder) == 1
    config.write_text(text.replace(placeholder, secret), encoding="utf-8")

    _assert_rejected_without_output(source, tmp_path, match="materializes")


@pytest.mark.parametrize(
    "mutation",
    (
        "comment-secret",
        "unknown-field",
        "wrong-type",
        "wrong-provider-binding",
    ),
)
def test_snapshot_rejects_noncanonical_or_unbound_config_before_output(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    config = source / "config.toml"
    text = config.read_text(encoding="utf-8")
    if mutation == "comment-secret":
        text += '# private_key = "smuggled-in-comment"\n'
    elif mutation == "unknown-field":
        text += 'unexpected_secret = "opaque"\n'
    elif mutation == "wrong-type":
        text = text.replace("chain_discriminant = 369", 'chain_discriminant = "369"', 1)
    else:
        digest = _sha256(b"unbound-provider-policy")
        text = text.replace(_sha256(b"provider-policy"), digest, 1)
    config.write_text(text, encoding="utf-8")

    _assert_rejected_without_output(source, tmp_path)


@pytest.mark.parametrize(
    "mutation",
    (
        "unknown-top-level",
        "missing-protocol",
        "retired-active-label",
        "wrong-secret-flag-type",
        "rollout-plan-binding",
        "unknown-policy-field",
        "wrong-chain",
        "wrong-network",
    ),
)
def test_snapshot_rejects_plan_inventory_and_type_drift_before_output(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    plan = _load_source_json(source, "privacy_bootstrap_plan.json")
    if mutation == "unknown-top-level":
        plan["operator_private_key"] = "secret"
    elif mutation == "missing-protocol":
        plan["privacy_catalog"]["protocols"].pop()
    elif mutation == "retired-active-label":
        plan["privacy_catalog"]["protocols"][6]["label"] = "sis-with-hints"
    elif mutation == "wrong-secret-flag-type":
        plan["bootle_lantern_issuer"]["secret_material_permitted"] = 0
    elif mutation == "rollout-plan-binding":
        plan["governance_rollout"]["rollout_plan_sha256"] = "11" * 32
    elif mutation == "unknown-policy-field":
        plan["bootle_lantern_issuer"]["governed_issuer_policy"][
            "trapdoor_seed"
        ] = "secret"
    elif mutation == "wrong-chain":
        plan["chain_id"] = "wrong-chain"
    else:
        plan["network_id"] = FOREIGN_NETWORK_ID
    _write_source_json(source, "privacy_bootstrap_plan.json", plan)

    _assert_rejected_without_output(source, tmp_path)


@pytest.mark.parametrize(
    "mutation",
    (
        "unknown-top-level",
        "unknown-policy-field",
        "short-matrix",
        "bad-coefficient-range",
        "boolean-coefficient",
        "short-allowlist",
        "short-public-id",
        "uppercase-instruction-hex",
        "provider-binding",
        "network-binding",
        "export-binding",
    ),
)
def test_snapshot_rejects_broker_shape_and_binding_drift_before_output(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    broker = _load_source_json(source, "bootle_lantern_broker_public.json")
    policy = broker["registration_instruction"]["policy"]
    rebind = True
    if mutation == "unknown-top-level":
        broker["bearer_token"] = "secret"
    elif mutation == "unknown-policy-field":
        policy["falcon_private_key"] = [1, 2, 3]
    elif mutation == "short-matrix":
        policy["issuer_public_matrix"]["entries"].pop()
    elif mutation == "bad-coefficient-range":
        policy["issuer_public_matrix"]["entries"][0]["coefficients"][0] = 12_289
    elif mutation == "boolean-coefficient":
        policy["issuer_public_matrix"]["entries"][0]["coefficients"][0] = True
    elif mutation == "short-allowlist":
        policy["allowed_values"].pop()
    elif mutation == "short-public-id":
        policy["issuer_id"].pop()
    elif mutation == "uppercase-instruction-hex":
        broker["registration_instruction_norito_hex"] = broker[
            "registration_instruction_norito_hex"
        ].upper()
    elif mutation == "provider-binding":
        broker["runtime_provider_policy_digest_hex"] = _sha256(
            b"different provider"
        )
    elif mutation == "network-binding":
        broker["network_id"] = FOREIGN_NETWORK_ID
    else:
        rebind = False
        broker["broker_contract_digest_hex"] = _sha256(b"changed contract")
    _write_source_json(
        source,
        "bootle_lantern_broker_public.json",
        broker,
        broker=True,
        rebind_broker=rebind,
    )

    _assert_rejected_without_output(source, tmp_path)


@pytest.mark.parametrize(
    "mutation",
    (
        "unknown-base-field",
        "encoded-activation",
        "encoded-policy",
        "decoded-activation",
        "decoded-policy",
        "wrong-chain",
        "wrong-discriminant-type",
    ),
)
def test_snapshot_rejects_genesis_shape_and_binding_drift_before_output(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    genesis = _load_source_json(source, "genesis.json")
    instructions = genesis["transactions"][-1]["instructions"]
    if mutation == "unknown-base-field":
        genesis["validator_private_key"] = "secret"
    elif mutation == "encoded-activation":
        instructions.append(base64.b64encode(b"activation").decode("ascii"))
    elif mutation == "encoded-policy":
        instructions.append(base64.b64encode(b"issuer-policy").decode("ascii"))
    elif mutation == "decoded-activation":
        instructions.append({"RegisterPrivacyProtocolActivationV1": {}})
    elif mutation == "decoded-policy":
        instructions.append({"RegisterPrivacyBootleLanternIssuerPolicyV1": {}})
    elif mutation == "wrong-chain":
        genesis["chain"] = "wrong-chain"
    else:
        genesis["chain_discriminant"] = True
    _write_source_json(source, "genesis.json", genesis)

    _assert_rejected_without_output(source, tmp_path)


@pytest.mark.parametrize(
    "mutation",
    (
        "duplicate-plan-key",
        "duplicate-broker-key",
        "nonfinite-plan-number",
        "compact-plan",
        "pretty-broker",
        "unsorted-genesis",
    ),
)
def test_snapshot_rejects_ambiguous_or_noncanonical_json_before_output(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    if mutation == "duplicate-plan-key":
        plan = source / "privacy_bootstrap_plan.json"
        plan.write_bytes(b'{"schema":"first","schema":"second"}\n')
    elif mutation == "duplicate-broker-key":
        broker = source / "bootle_lantern_broker_public.json"
        broker.write_bytes(b'{"schema":"first",' + broker.read_bytes()[1:])
    elif mutation == "nonfinite-plan-number":
        plan = source / "privacy_bootstrap_plan.json"
        plan.write_bytes(
            plan.read_bytes().replace(
                b'"schema_version": 1', b'"schema_version": NaN', 1
            )
        )
    elif mutation == "compact-plan":
        plan = _load_source_json(source, "privacy_bootstrap_plan.json")
        (source / "privacy_bootstrap_plan.json").write_bytes(_compact_json(plan))
    elif mutation == "pretty-broker":
        broker = _load_source_json(source, "bootle_lantern_broker_public.json")
        (source / "bootle_lantern_broker_public.json").write_bytes(
            _pretty_json(broker)
        )
    else:
        genesis = _load_source_json(source, "genesis.json")
        genesis = dict(reversed(tuple(genesis.items())))
        payload = (
            json.dumps(genesis, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
        ).encode("utf-8")
        assert payload != _pretty_json(genesis)
        (source / "genesis.json").write_bytes(payload)

    _assert_rejected_without_output(source, tmp_path)


def test_snapshot_fails_closed_without_toml_parser_before_output(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    monkeypatch.setattr(snapshotter, "tomllib", None)

    _assert_rejected_without_output(source, tmp_path, match="tomllib")


@pytest.mark.parametrize("mutation", ("missing", "extra", "symlink", "hardlink"))
def test_snapshot_rejects_non_exact_or_aliased_inputs(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    target = source / "config.toml"
    if mutation == "missing":
        target.unlink()
    elif mutation == "extra":
        (source / "unexpected").write_bytes(b"unexpected")
    elif mutation == "symlink":
        target.unlink()
        target.symlink_to(source / "genesis.json")
    else:
        os.link(target, source / "config-hardlink")

    output = tmp_path / "handoff"
    with pytest.raises(snapshotter.SnapshotError):
        snapshotter.snapshot(source, output, workspace)
    assert not output.exists()


def test_snapshot_rejects_post_stat_inode_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    real_open = snapshotter.os.open
    replaced = False

    def racing_open(path, flags, *args, **kwargs):
        nonlocal replaced
        if path == "config.toml" and kwargs.get("dir_fd") is not None and not replaced:
            replacement = source / "replacement"
            replacement.write_bytes(b"substituted-after-stat\n")
            replacement.chmod(0o600)
            os.replace(replacement, source / "config.toml")
            replaced = True
        return real_open(path, flags, *args, **kwargs)

    monkeypatch.setattr(snapshotter.os, "open", racing_open)
    output = tmp_path / "handoff"
    with pytest.raises(snapshotter.SnapshotError, match="changed while opening"):
        snapshotter.snapshot(source, output, workspace)
    assert not output.exists()


def test_snapshot_rejects_fifo_and_device_inputs(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    config = source / "config.toml"
    config.unlink()
    os.mkfifo(config, mode=0o600)
    fifo_output = tmp_path / "fifo-output"
    with pytest.raises(snapshotter.SnapshotError, match="identity is unsafe"):
        snapshotter.snapshot(source, fifo_output, workspace)
    assert not fifo_output.exists()

    config.unlink()
    config.write_bytes(b"config\n")
    config.chmod(0o600)
    real_stat = snapshotter.os.stat

    def device_stat(path, *args, **kwargs):
        info = real_stat(path, *args, **kwargs)
        if path == "config.toml" and kwargs.get("dir_fd") is not None:
            values = list(info)
            values[0] = stat.S_IFCHR | 0o600
            return os.stat_result(values)
        return info

    monkeypatch.setattr(snapshotter.os, "stat", device_stat)
    device_output = tmp_path / "device-output"
    with pytest.raises(snapshotter.SnapshotError, match="identity is unsafe"):
        snapshotter.snapshot(source, device_output, workspace)
    assert not device_output.exists()


def test_snapshot_partial_write_never_closes_handoff(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    output = tmp_path / "partial-output"
    monkeypatch.setattr(snapshotter.os, "write", lambda *_args, **_kwargs: 0)

    with pytest.raises(snapshotter.SnapshotError, match="short public privacy"):
        snapshotter.snapshot(source, output, workspace)

    assert output.exists()
    assert not (output / snapshotter.HANDOFF_MANIFEST).exists()
    assert _mode(output) != 0o555


def test_snapshot_rejects_source_inside_checkout_or_symlinked_output_parent(
    tmp_path: Path,
) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    source = _source(workspace)
    output = tmp_path / "handoff"
    with pytest.raises(snapshotter.SnapshotError, match="outside the checkout"):
        snapshotter.snapshot(source, output, workspace)
    assert not output.exists()

    external = tmp_path / "external"
    external.mkdir(mode=0o700)
    alias = tmp_path / "alias"
    alias.symlink_to(external, target_is_directory=True)
    source = _source(tmp_path / "second")
    with pytest.raises(snapshotter.SnapshotError, match="canonical physical"):
        snapshotter.snapshot(source, alias / "handoff", workspace)
    assert not (external / "handoff").exists()


@pytest.mark.parametrize("mutation", ("directory-mode", "file-mode"))
def test_snapshot_requires_exact_owner_private_source_modes(
    tmp_path: Path,
    mutation: str,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    if mutation == "directory-mode":
        source.chmod(0o750)
        message = "exact mode 0700"
    else:
        (source / "config.toml").chmod(0o640)
        message = "identity is unsafe"

    output = tmp_path / "handoff"
    with pytest.raises(snapshotter.SnapshotError, match=message):
        snapshotter.snapshot(source, output, workspace)
    assert not output.exists()


def test_snapshot_rejects_same_uid_output_root_replacement(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    output = tmp_path / "handoff"
    displaced = tmp_path / "displaced-handoff"
    real_fchmod = snapshotter.os.fchmod
    freeze_calls = 0

    def racing_fchmod(descriptor: int, mode: int) -> None:
        nonlocal freeze_calls
        real_fchmod(descriptor, mode)
        if mode == 0o555:
            freeze_calls += 1
            if freeze_calls == 2:
                output.rename(displaced)
                output.mkdir(mode=0o755)

    monkeypatch.setattr(snapshotter.os, "fchmod", racing_fchmod)
    with pytest.raises(snapshotter.SnapshotError, match="handoff root changed"):
        snapshotter.snapshot(source, output, workspace)

    assert displaced.exists()
    assert not (output / snapshotter.HANDOFF_MANIFEST).exists()


def test_snapshot_rejects_output_mutation_before_final_replay(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    source = _source(tmp_path)
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    output = tmp_path / "handoff"
    real_fchmod = snapshotter.os.fchmod
    freeze_calls = 0

    def mutating_fchmod(descriptor: int, mode: int) -> None:
        nonlocal freeze_calls
        real_fchmod(descriptor, mode)
        if mode == 0o555:
            freeze_calls += 1
            if freeze_calls == 2:
                target = output / "inputs/config.toml"
                target.chmod(0o600)
                target.write_bytes(b"same-uid post-write substitution\n")

    monkeypatch.setattr(snapshotter.os, "fchmod", mutating_fchmod)
    with pytest.raises(snapshotter.SnapshotError, match="changed before replay"):
        snapshotter.snapshot(source, output, workspace)


def test_snapshot_refuses_privileged_output_inside_checkout(tmp_path: Path) -> None:
    workspace = tmp_path / "workspace"
    workspace.mkdir(mode=0o700)
    source = _source(tmp_path)

    output = workspace / "handoff"
    with pytest.raises(snapshotter.SnapshotError, match="output.*outside"):
        snapshotter.snapshot(source, output, workspace)
    assert not output.exists()
