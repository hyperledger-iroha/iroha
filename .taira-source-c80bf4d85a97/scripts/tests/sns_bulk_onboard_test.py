"""Focused tests for the typed, atomic alias setup planner client."""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

MODULE_PATH = Path(__file__).resolve().parents[1] / "sns_bulk_onboard.py"
SPEC = importlib.util.spec_from_file_location("sns_bulk_onboard", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)  # type: ignore[attr-defined]

PLAN_HASH = (
    "hash:1112131415161718191A1B1C1D1E1F202122232425262728292A2B2C2D2E2F31#011A"
)


def _quote_guard() -> dict[str, object]:
    return {
        "expected_policy_version": 7,
        "expected_payment_asset": "xor#sora",
        "max_amount": "1000000",
        "valid_until_ms": 1_900_000_000_000,
    }


def _ensure(kind: str, name: str) -> dict[str, object]:
    return {
        "intent": {
            "kind": kind,
            "alias": name,
            "owner": "ed0120owner",
        },
        "acquisition": {"term_years": 1},
        "quote_guard": _quote_guard(),
    }


def _setup_intent() -> dict[str, object]:
    return {
        "schema_version": 1,
        "dataspaces": [
            _ensure("Dataspace", "paynet"),
            _ensure("DataspaceAlias", "retail"),
        ],
        "domains": [_ensure("Domain", "banka.paynet")],
        "accounts": [
            _ensure("AccountAlias", "merchant@banka.paynet"),
            _ensure("Account", "auditor@paynet"),
        ],
    }


def _plan(resource_count: int) -> dict[str, object]:
    return {
        "body": {
            "version": 1,
            "authority": "ed0120payer",
            "resources": [
                {"resource": f"resource-{index}", "disposition": "Create"}
                for index in range(resource_count)
            ],
            "instructions": [f"4e525431{index:02x}" for index in range(resource_count)],
            "transaction_count": 1,
            "warnings": [],
            "blockers": [],
        },
        "plan_hash": PLAN_HASH,
    }


def test_request_construction_keeps_one_dependency_ordered_vector() -> None:
    source = _setup_intent()
    before = json.loads(json.dumps(source))

    request = MODULE.build_plan_request(source)

    assert source == before
    assert set(request) == {"schema_version", "intents"}
    assert request["schema_version"] == 1
    assert [entry["intent"]["alias"] for entry in request["intents"]] == [
        "paynet",
        "retail",
        "banka.paynet",
        "merchant@banka.paynet",
        "auditor@paynet",
    ]


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("suffix_id", 0x1001),
        ("payment_proof", {"signature": "fake"}),
        ("settlement_tx", "fake"),
        ("payer", "ed0120payer"),
        ("lease_expiry_ms", 1_900_000_000_000),
        ("private_key", "not-allowed"),
        ("token", "not-allowed"),
    ],
)
def test_legacy_and_secret_intent_fields_are_rejected(field: str, value: object) -> None:
    document = _setup_intent()
    document["accounts"][0]["intent"][field] = value

    with pytest.raises(MODULE.BulkOnboardError) as captured:
        MODULE.build_plan_request(document)

    assert field.replace("_", "") in str(captured.value).replace("_", "")
    assert str(value) not in str(captured.value)


def test_cli_planner_signs_one_complete_vector_and_never_splits(tmp_path: Path) -> None:
    captured: dict[str, object] = {"calls": 0}
    request_path = tmp_path / "request.json"
    plan_path = tmp_path / "plan.json"
    config_path = tmp_path / "client.toml"
    request_body = MODULE.build_plan_request(_setup_intent())
    MODULE.write_plan_file(request_path, request_body)

    def runner(command, **kwargs):
        captured["calls"] = int(captured["calls"]) + 1
        captured["command"] = command
        captured["kwargs"] = kwargs
        plan_path.write_text(json.dumps(_plan(5)), encoding="utf-8")
        return SimpleNamespace(returncode=0, stdout="", stderr="")

    response = MODULE.request_plan_with_cli(
        "/opt/bin/iroha",
        request_path,
        plan_path,
        config_file=config_path,
        runner=runner,
    )

    assert response == _plan(5)
    assert captured["calls"] == 1
    assert captured["command"] == [
        "/opt/bin/iroha",
        "--config",
        str(config_path),
        "app",
        "alias",
        "setup",
        "plan",
        "--intent-file",
        str(request_path),
        "--plan-file",
        str(plan_path),
    ]
    assert len(json.loads(request_path.read_text())["intents"]) == 5


def test_cli_planner_failure_does_not_reflect_subprocess_output(tmp_path: Path) -> None:
    reflected = "token=do-not-log private_key=server-reflection"

    def runner(_command, **_kwargs):
        return SimpleNamespace(returncode=9, stdout="", stderr=reflected)

    with pytest.raises(MODULE.BulkOnboardError) as captured:
        MODULE.request_plan_with_cli(
            "iroha",
            tmp_path / "request.json",
            tmp_path / "plan.json",
            runner=runner,
        )

    assert str(captured.value) == "iroha alias setup plan failed with exit status 9"
    assert "do-not-log" not in str(captured.value)
    assert "server-reflection" not in str(captured.value)


def test_complete_plan_hash_and_atomicity_validation() -> None:
    plan = _plan(5)
    validated = MODULE.validate_plan_response(plan, 5)

    assert validated.body == plan
    assert validated.resource_count == 5
    assert validated.plan_hash == PLAN_HASH


def test_plan_envelopes_and_legacy_sequence_names_are_rejected() -> None:
    plan = _plan(5)
    with pytest.raises(MODULE.BulkOnboardError, match="not an envelope"):
        MODULE.validate_plan_response({"plan": plan}, 5)

    for canonical, legacy in [
        ("resources", "resource_dispositions"),
        ("resources", "dispositions"),
        ("instructions", "framed_instructions"),
        ("instructions", "instruction_frames"),
    ]:
        legacy_plan = _plan(5)
        legacy_plan["body"][legacy] = legacy_plan["body"].pop(canonical)
        with pytest.raises(MODULE.BulkOnboardError, match=f"canonical .*{canonical}"):
            MODULE.validate_plan_response(legacy_plan, 5)

        mixed_plan = _plan(5)
        mixed_plan["body"][legacy] = list(mixed_plan["body"][canonical])
        with pytest.raises(MODULE.BulkOnboardError, match="forbidden legacy"):
            MODULE.validate_plan_response(mixed_plan, 5)


@pytest.mark.parametrize(
    "bad_hash",
    [
        "A5" * 32,
        "hash:" + "a5" * 32 + "#0000",
        "hash:" + "A5" * 32 + "#0000",
        "sha256:" + "A5" * 32,
        "hash:ABC",
    ],
)
def test_plan_hash_format_is_strict(bad_hash: str) -> None:
    plan = _plan(5)
    plan["plan_hash"] = bad_hash

    with pytest.raises(MODULE.BulkOnboardError, match="canonical hash"):
        MODULE.validate_plan_response(plan, 5)


def test_blockers_are_sorted_and_server_details_are_not_reflected() -> None:
    plan = _plan(5)
    plan["body"]["blockers"] = [
        {"code": "alias.z-blocked", "remediation": "token=do-not-log"},
        {"code": "alias.a-blocked", "actual": "private_key=do-not-log"},
    ]

    with pytest.raises(MODULE.BulkOnboardError) as captured:
        MODULE.validate_plan_response(plan, 5)

    assert str(captured.value) == (
        "planner returned blocker(s): alias.a-blocked, alias.z-blocked"
    )


@pytest.mark.parametrize(
    "mutation",
    [
        lambda plan: plan["body"].update(transaction_count=2),
        lambda plan: plan["body"].update(transactions=[{}, {}]),
        lambda plan: plan["body"]["resources"].pop(),
        lambda plan: plan["body"]["instructions"].pop(),
        lambda plan: plan["body"]["resources"][0].update(disposition="Conflict"),
        lambda plan: plan.update(partial_plan=True),
    ],
)
def test_partial_conflicting_or_split_plans_are_rejected(mutation) -> None:
    plan = _plan(5)
    mutation(plan)

    with pytest.raises(MODULE.BulkOnboardError):
        MODULE.validate_plan_response(plan, 5)


def test_plan_file_is_owner_only_and_symlinks_fail_closed(tmp_path: Path) -> None:
    plan_file = tmp_path / "plan.json"
    MODULE.write_plan_file(plan_file, _plan(1))
    assert plan_file.stat().st_mode & 0o777 == 0o600

    target = tmp_path / "target.json"
    target.write_text("{}", encoding="utf-8")
    symlink = tmp_path / "plan-link.json"
    symlink.symlink_to(target)
    with pytest.raises(MODULE.BulkOnboardError, match="symlink"):
        MODULE.write_plan_file(symlink, _plan(1))


def test_apply_command_uses_alias_tree_and_only_file_paths(tmp_path: Path) -> None:
    plan = tmp_path / "setup.plan.json"
    config = tmp_path / "client.toml"

    command = MODULE.build_apply_command(
        "/opt/bin/iroha",
        plan,
        config_file=config,
    )

    assert command == [
        "/opt/bin/iroha",
        "--config",
        str(config),
        "app",
        "alias",
        "setup",
        "apply",
        "--plan-file",
        str(plan),
    ]
    assert "sns" not in command
    assert "register" not in command
    assert "token" not in " ".join(command).lower()


def test_apply_failure_does_not_reflect_subprocess_output(tmp_path: Path) -> None:
    captured: dict[str, object] = {}
    secret = "private_key=do-not-log"

    def runner(command, **kwargs):
        captured["command"] = command
        captured["kwargs"] = kwargs
        return SimpleNamespace(returncode=9, stdout="", stderr=secret)

    with pytest.raises(MODULE.BulkOnboardError) as error:
        MODULE.apply_plan("iroha", tmp_path / "plan.json", runner=runner)

    assert str(error.value) == "iroha alias setup apply failed with exit status 9"
    assert secret not in str(error.value)
    assert captured["kwargs"]["stdin"] == subprocess.DEVNULL
    assert captured["command"][:5] == ["iroha", "app", "alias", "setup", "apply"]


def test_raw_token_argument_is_rejected_without_echo(capsys) -> None:
    raw_secret = "raw-command-line-secret"

    result = MODULE.main(["intent.json", "--token", raw_secret])

    captured = capsys.readouterr()
    assert result == 1
    assert "raw token" in captured.err
    assert raw_secret not in captured.err


def test_default_planning_persists_deterministically_without_applying(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    intent_file = tmp_path / "intent.json"
    intent_file.write_text(json.dumps(_setup_intent()), encoding="utf-8")
    plan_file = tmp_path / "setup.plan.json"
    expected_plan = _plan(5)

    def request_plan(_cli, request_path, _plan_path, *, config_file):
        assert config_file is None
        assert len(json.loads(request_path.read_text())["intents"]) == 5
        return expected_plan

    monkeypatch.setattr(MODULE, "request_plan_with_cli", request_plan)
    monkeypatch.setattr(
        MODULE,
        "apply_plan",
        lambda *_args, **_kwargs: pytest.fail("default planning invoked apply"),
    )

    result = MODULE.main(
        [
            str(intent_file),
            "--plan-file",
            str(plan_file),
        ]
    )

    captured = capsys.readouterr()
    assert result == 0
    assert captured.err == ""
    assert "Planned 5 alias resource(s) atomically" in captured.out
    assert json.loads(plan_file.read_text(encoding="utf-8")) == expected_plan
    first_bytes = plan_file.read_bytes()
    MODULE.write_plan_file(plan_file, expected_plan)
    assert plan_file.read_bytes() == first_bytes


def test_apply_requires_explicit_flag_and_retired_plan_only_is_rejected(
    tmp_path: Path,
    monkeypatch,
    capsys,
) -> None:
    intent_file = tmp_path / "intent.json"
    intent_file.write_text(json.dumps(_setup_intent()), encoding="utf-8")
    plan_file = tmp_path / "setup.plan.json"
    expected_plan = _plan(5)
    applied: list[tuple[str, Path, Path | None]] = []

    monkeypatch.setattr(
        MODULE,
        "request_plan_with_cli",
        lambda *_args, **_kwargs: expected_plan,
    )
    monkeypatch.setattr(
        MODULE,
        "apply_plan",
        lambda cli, plan, *, config_file: applied.append((cli, plan, config_file)),
    )

    result = MODULE.main(
        [
            str(intent_file),
            "--plan-file",
            str(plan_file),
            "--apply",
        ]
    )

    captured = capsys.readouterr()
    assert result == 0
    assert captured.err == ""
    assert "Applied 5 alias resource(s) atomically" in captured.out
    assert applied == [("iroha", plan_file, None)]

    with pytest.raises(SystemExit) as exited:
        MODULE.main(
            [
                str(intent_file),
                "--plan-file",
                str(plan_file),
                "--plan-only",
            ]
        )
    assert exited.value.code == 2
    assert "unsupported command-line argument" in capsys.readouterr().err


def test_redaction_is_bounded_and_removes_common_secret_forms() -> None:
    exact = "T" * 32
    rendered = MODULE.redact_text(
        f"Bearer bearer-value token=token-value private_key=key-value {exact}\nnext",
        [exact],
    )

    assert "bearer-value" not in rendered
    assert "token-value" not in rendered
    assert "key-value" not in rendered
    assert exact not in rendered
    assert "\n" not in rendered
    assert len(rendered) <= 512
