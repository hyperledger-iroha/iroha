"""Tests for the Cargo feature-ownership guard."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = ROOT / "scripts" / "check_cargo_feature_hygiene.py"
SPEC = importlib.util.spec_from_file_location("check_cargo_feature_hygiene", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
FEATURE_HYGIENE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FEATURE_HYGIENE)


def test_repository_feature_hygiene() -> None:
    assert FEATURE_HYGIENE.check_repository(ROOT) == []


def _guarded_document(package: str) -> dict:
    manifest = ROOT / "crates" / package / "Cargo.toml"
    return FEATURE_HYGIENE._load_toml(manifest)


def _guarded_errors(package: str, document: dict) -> list[str]:
    manifest = ROOT / "crates" / package / "Cargo.toml"
    return FEATURE_HYGIENE._check_expected_features(document, manifest)


def test_rejects_unclassified_explicit_feature_omitted_from_default() -> None:
    document = copy.deepcopy(_guarded_document("iroha_core"))
    document["features"]["new-portable-production-capability"] = []

    errors = _guarded_errors("iroha_core", document)

    assert any(
        "Cargo feature `new-portable-production-capability` is unclassified" in error
        for error in errors
    )


def test_rejects_unclassified_implicit_optional_dependency_feature() -> None:
    document = copy.deepcopy(_guarded_document("iroha_core"))
    document["dependencies"]["new_optional_backend"] = {
        "version": "1",
        "optional": True,
    }

    errors = _guarded_errors("iroha_core", document)

    assert any(
        "Cargo feature `new_optional_backend` is unclassified" in error
        for error in errors
    )


def test_rejects_broken_portable_default_closure() -> None:
    document = copy.deepcopy(_guarded_document("iroha_core"))
    document["features"]["default"].remove("simd")

    errors = _guarded_errors("iroha_core", document)

    assert any(
        "portable feature `simd` is not reachable from `default`" in error
        for error in errors
    )


def test_rejects_exact_portable_forwarder_mutation() -> None:
    document = copy.deepcopy(_guarded_document("iroha_core"))
    document["features"]["gost"] = []

    errors = _guarded_errors("iroha_core", document)

    assert any("feature `gost` must be" in error for error in errors)


def test_rejects_unpinned_contextual_shipping_forwarder(monkeypatch) -> None:
    monkeypatch.delitem(
        FEATURE_HYGIENE.EXPECTED_FEATURES["iroha_data_model"],
        "transparent_api",
    )

    errors = _guarded_errors(
        "iroha_data_model", _guarded_document("iroha_data_model")
    )

    assert any(
        "contextual shipping feature `transparent_api` lacks an exact feature pin"
        in error
        for error in errors
    )


def test_rejects_contextual_shipping_forwarder_mutation() -> None:
    document = copy.deepcopy(_guarded_document("iroha_data_model"))
    document["features"]["transparent_api"] = []

    errors = _guarded_errors("iroha_data_model", document)

    assert any("feature `transparent_api` must be" in error for error in errors)


def test_rejects_contextual_shipping_feature_reachable_from_default() -> None:
    document = copy.deepcopy(_guarded_document("iroha_data_model"))
    document["features"]["default"].append("transparent_api")

    errors = _guarded_errors("iroha_data_model", document)

    assert any(
        "contextual shipping feature `transparent_api` is reachable from local "
        "`default`" in error
        for error in errors
    )


def test_rejects_explicit_opt_in_reachable_from_default() -> None:
    document = copy.deepcopy(_guarded_document("iroha_core"))
    document["features"]["default"].append("quic")

    errors = _guarded_errors("iroha_core", document)

    assert any(
        "explicit opt-in feature `quic` is reachable from `default`" in error
        for error in errors
    )


def test_rejects_stale_explicit_opt_in_name(monkeypatch) -> None:
    current = FEATURE_HYGIENE.EXPLICIT_OPT_IN_FEATURES["iroha_core"]
    monkeypatch.setitem(
        FEATURE_HYGIENE.EXPLICIT_OPT_IN_FEATURES,
        "iroha_core",
        tuple(sorted((*current, "retired-opt-in"))),
    )

    errors = _guarded_errors("iroha_core", _guarded_document("iroha_core"))

    assert any(
        "stale explicit opt-in feature `retired-opt-in`" in error for error in errors
    )


def _member_rows(*, implicit_norito_defaults: bool = False) -> list[str]:
    rows = []
    for name in sorted(FEATURE_HYGIENE.FOUNDATIONAL_DEPENDENCIES):
        if implicit_norito_defaults and name == "norito":
            rows.append(
                f'{name} = {{ workspace = true, features = ["json"] }}'
            )
        else:
            rows.append(
                f"{name} = {{ workspace = true, default-features = false }}"
            )
    return rows


def _write_member(root: Path, member: str, rows: list[str]) -> None:
    member_root = root / member
    member_root.mkdir(parents=True)
    (member_root / "Cargo.toml").write_text(
        "\n".join(
            [
                "[package]",
                f'name = "{member_root.name}"',
                'version = "0.1.0"',
                "",
                "[dependencies]",
                *rows,
                "",
            ]
        ),
        encoding="utf-8",
    )


def _write_fixture(
    root: Path,
    *,
    root_features: bool = False,
    member_defaults: bool = False,
    non_default_member_defaults: bool = False,
) -> None:
    dependency_rows = []
    for name in sorted(FEATURE_HYGIENE.FOUNDATIONAL_DEPENDENCIES):
        injected = ', features = ["broad"]' if root_features and name == "norito" else ""
        dependency_rows.append(
            f'{name} = {{ path = "deps/{name}", default-features = false{injected} }}'
        )

    (root / "Cargo.toml").write_text(
        "\n".join(
            [
                "[workspace]",
                'members = ["crates/*", "crates/consumer"]',
                'default-members = ["crates/consumer"]',
                'exclude = ["crates/excluded"]',
                "",
                "[workspace.dependencies]",
                *dependency_rows,
                "",
            ]
        ),
        encoding="utf-8",
    )
    _write_member(
        root,
        "crates/consumer",
        _member_rows(implicit_norito_defaults=member_defaults),
    )
    _write_member(
        root,
        "crates/excluded",
        _member_rows(implicit_norito_defaults=True),
    )
    if non_default_member_defaults:
        _write_member(
            root,
            "crates/tool",
            _member_rows(implicit_norito_defaults=True),
        )


def test_accepts_explicit_workspace_member_feature_ownership(tmp_path: Path) -> None:
    _write_fixture(tmp_path)

    assert FEATURE_HYGIENE.check_repository(tmp_path) == []


def test_workspace_members_expand_globs_deduplicate_and_respect_excludes(
    tmp_path: Path,
) -> None:
    _write_fixture(tmp_path)
    workspace = FEATURE_HYGIENE._load_toml(tmp_path / "Cargo.toml")["workspace"]

    manifests = FEATURE_HYGIENE.workspace_member_manifests(tmp_path, workspace)

    assert [manifest.relative_to(tmp_path).as_posix() for manifest in manifests] == [
        "crates/consumer/Cargo.toml"
    ]


def test_rejects_workspace_feature_injection(tmp_path: Path) -> None:
    _write_fixture(tmp_path, root_features=True)

    errors = FEATURE_HYGIENE.check_repository(tmp_path)

    assert any(
        "workspace dependency `norito` must not inject features" in error
        for error in errors
    )


def test_rejects_implicit_default_features_in_default_member(tmp_path: Path) -> None:
    _write_fixture(tmp_path, member_defaults=True)

    errors = FEATURE_HYGIENE.check_repository(tmp_path)

    assert any(
        "[dependencies] `norito` must set `default-features = false`" in error
        for error in errors
    )


def test_rejects_implicit_defaults_in_non_default_member_with_local_features(
    tmp_path: Path,
) -> None:
    _write_fixture(tmp_path, non_default_member_defaults=True)

    errors = FEATURE_HYGIENE.check_repository(tmp_path)

    assert any(
        "crates/tool/Cargo.toml" in error
        and "[dependencies] `norito` must set `default-features = false`" in error
        for error in errors
    )
