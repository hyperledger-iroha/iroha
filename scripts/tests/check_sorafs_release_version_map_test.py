"""Tests for the SoraFS cross-package release version-map guard."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from scripts import check_sorafs_release_version_map as version_map


def _write_package_sources(root: Path) -> list[dict[str, str]]:
    sources = [
        ("a-cargo", "cargo", "cargo/Cargo.toml", '[package]\nname="a"\nversion="0.1.0"\n'),
        ("b-gradle", "gradle-property", "gradle/gradle.properties", "sdkVersion=0.1.0\n"),
        ("c-msbuild", "msbuild", "dotnet/a.csproj", "<Project><PropertyGroup><Version>0.1.0-preview.1</Version></PropertyGroup></Project>"),
        ("d-npm", "npm", "npm/package.json", '{"name":"a","version":"0.0.3"}'),
        ("e-plain", "plain-semver", "swift/VERSION", "0.1.0\n"),
        ("f-python", "python", "python/pyproject.toml", '[project]\nname="a"\nversion="0.0.1"\n'),
    ]
    versions = {
        "cargo": "0.1.0",
        "gradle-property": "0.1.0",
        "msbuild": "0.1.0-preview.1",
        "npm": "0.0.3",
        "plain-semver": "0.1.0",
        "python": "0.0.1",
    }
    rows: list[dict[str, str]] = []
    for identifier, ecosystem, relative, payload in sources:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(payload, encoding="utf-8")
        row = {
            "id": identifier,
            "ecosystem": ecosystem,
            "path": relative,
            "version": versions[ecosystem],
        }
        if ecosystem == "gradle-property":
            row["version_key"] = "sdkVersion"
        rows.append(row)
    return rows


def _write_map(root: Path, rows: list[dict[str, object]], *, extra: str = "") -> None:
    lines = ['schema_version = 1', 'release_version = "0.1.0"', extra]
    for row in rows:
        lines.extend(
            [
                "[[packages]]",
                f'id = {json.dumps(row["id"])}',
                f'ecosystem = {json.dumps(row["ecosystem"])}',
                f'path = {json.dumps(row["path"])}',
                f'version = {json.dumps(row["version"])}',
            ]
        )
        if "version_key" in row:
            lines.append(f'version_key = {json.dumps(row["version_key"])}')
    target = root / "release/version-map.toml"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("\n".join(lines) + "\n", encoding="utf-8")


def test_read_bytes_no_follow_rejects_symlink_and_non_regular(tmp_path: Path) -> None:
    regular = tmp_path / "regular"
    regular.write_bytes(b"version")
    assert version_map._read_bytes_no_follow(regular) == b"version"

    link = tmp_path / "link"
    link.symlink_to(regular)
    with pytest.raises(OSError):
        version_map._read_bytes_no_follow(link)
    with pytest.raises(ValueError, match="regular file"):
        version_map._read_bytes_no_follow(tmp_path)


def test_json_object_hook_rejects_duplicate_members() -> None:
    assert version_map._json_object_without_duplicate_keys(
        [("name", "sdk"), ("version", "0.1.0")]
    ) == {"name": "sdk", "version": "0.1.0"}
    with pytest.raises(ValueError, match="duplicate key"):
        version_map._json_object_without_duplicate_keys(
            [("version", "0.1.0"), ("version", "9.9.9")]
        )


@pytest.mark.parametrize(
    "relative",
    ["", "../outside", "/absolute", "a/./b", "a\\b", "file://source", "a%2fb"],
)
def test_require_regular_repo_file_rejects_ambiguous_paths(
    tmp_path: Path, relative: str
) -> None:
    with pytest.raises(ValueError):
        version_map._require_regular_repo_file(tmp_path, relative)


def test_require_regular_repo_file_rejects_symlinked_parent(tmp_path: Path) -> None:
    real = tmp_path / "real"
    real.mkdir()
    (real / "file").write_text("x", encoding="utf-8")
    (tmp_path / "alias").symlink_to(real, target_is_directory=True)
    with pytest.raises(ValueError, match="symlinks"):
        version_map._require_regular_repo_file(tmp_path, "alias/file")


def test_read_declared_version_supports_every_mapped_ecosystem(tmp_path: Path) -> None:
    rows = _write_package_sources(tmp_path)
    for row in rows:
        assert (
            version_map._read_declared_version(
                tmp_path / row["path"],
                row["ecosystem"],
                row.get("version_key"),
            )
            == row["version"]
        )
    with pytest.raises(ValueError, match="unsupported"):
        version_map._read_declared_version(tmp_path / rows[0]["path"], "unknown")


def test_validate_version_map_emits_schema_closed_summary(tmp_path: Path) -> None:
    rows = _write_package_sources(tmp_path)
    _write_map(tmp_path, rows)
    summary = version_map.validate_version_map(
        tmp_path,
        required_package_contracts=None,
    )
    assert summary == {
        "schema": version_map.SCHEMA,
        "release_version": "0.1.0",
        "package_count": 6,
        "packages": rows,
    }


@pytest.mark.parametrize(
    ("mutation", "message"),
    [
        (lambda rows: list(reversed(rows)), "sorted"),
        (lambda rows: rows + [dict(rows[-1])], "duplicates an id"),
        (
            lambda rows: [dict(rows[0], ecosystem="unknown"), *rows[1:]],
            "unsupported ecosystem",
        ),
        (
            lambda rows: [dict(rows[0], path="../Cargo.toml"), *rows[1:]],
            "canonical repository-relative",
        ),
        (
            lambda rows: [dict(rows[0], version="01.0.0"), *rows[1:]],
            "invalid version",
        ),
    ],
)
def test_validate_version_map_rejects_adversarial_rows(
    tmp_path: Path, mutation: object, message: str
) -> None:
    rows = _write_package_sources(tmp_path)
    mutated = mutation(rows)  # type: ignore[operator]
    _write_map(tmp_path, mutated)
    with pytest.raises(ValueError, match=message):
        version_map.validate_version_map(
            tmp_path,
            required_package_contracts=None,
        )


def test_validate_version_map_rejects_version_drift_and_extra_fields(tmp_path: Path) -> None:
    rows = _write_package_sources(tmp_path)
    rows[0]["version"] = "0.1.1"
    _write_map(tmp_path, rows)
    with pytest.raises(ValueError, match="does not match"):
        version_map.validate_version_map(
            tmp_path,
            required_package_contracts=None,
        )

    rows[0]["version"] = "0.1.0"
    _write_map(tmp_path, rows, extra='unexpected = "payload"')
    with pytest.raises(ValueError, match="schema-closed"):
        version_map.validate_version_map(
            tmp_path,
            required_package_contracts=None,
        )


def test_main_validates_repository_map(capsys: pytest.CaptureFixture[str]) -> None:
    assert version_map.main([]) == 0
    summary = json.loads(capsys.readouterr().out)
    assert summary["schema"] == version_map.SCHEMA
    assert summary["package_count"] == 13


def test_required_first_release_inventory_rejects_replaced_package(
    tmp_path: Path,
) -> None:
    rows = _write_package_sources(tmp_path)
    required = {
        row["id"]: (
            row["ecosystem"],
            row["path"],
            row.get("version_key"),
        )
        for row in rows
    }
    rows[0]["id"] = "a-replacement"
    _write_map(tmp_path, rows)

    with pytest.raises(ValueError, match="required first-release contract"):
        version_map.validate_version_map(
            tmp_path,
            required_package_contracts=required,
        )


def test_first_release_sdk_versions_are_final_and_swift_is_tag_pinned() -> None:
    summary = version_map.validate_version_map(Path(__file__).resolve().parents[2])
    actual_contracts = {
        row["id"]: (
            row["ecosystem"],
            row["path"],
            row.get("version_key"),
        )
        for row in summary["packages"]
    }
    assert actual_contracts == version_map.REQUIRED_PACKAGE_CONTRACTS
    versions = {row["id"]: row["version"] for row in summary["packages"]}
    assert versions["iroha-sdk-csharp"] == "0.1.0"
    assert versions["iroha-sdk-kotlin"] == "0.1.0"
    assert versions["iroha-android-java"] == "0.1.0"
    assert versions["norito-java"] == "0.1.0"
    assert versions["iroha-swift"] == "0.1.0"
    assert versions["iroha-torii-client"] == "0.0.1"
    assert all(
        "SNAPSHOT" not in version and "preview" not in version
        for version in versions.values()
    )

    podspec = (Path(__file__).resolve().parents[2] / "IrohaSwift/IrohaSwift.podspec").read_text(
        encoding="utf-8"
    )
    assert "s.version          = version" in podspec
    assert ':tag => "iroha-swift-v#{version}"' in podspec
    assert ":branch" not in podspec
    assert "File.symlink?(version_file)" in podspec

    for relative in ("IrohaSwift/README.md", "specs/sdk/swift/index.md"):
        install_doc = (Path(__file__).resolve().parents[2] / relative).read_text(
            encoding="utf-8"
        )
        normalized_install_doc = " ".join(install_doc.split())
        assert 'exact: "0.1.0"' in install_doc
        assert 'branch: "main"' not in install_doc
        assert (
            "not evidence that the tags are already public" in normalized_install_doc
            or "Do not advertise them until" in normalized_install_doc
        )

    for relative in (
        "kotlin/core-jvm/build.gradle.kts",
        "kotlin/client-android/build.gradle.kts",
        "kotlin/offline-wallet-android/build.gradle.kts",
        "java/iroha_android/build.gradle.kts",
        "java/iroha_android/core/build.gradle.kts",
        "java/iroha_android/jvm/build.gradle.kts",
        "java/iroha_android/android/build.gradle.kts",
        "java/iroha_android/samples-android/build.gradle.kts",
        "java/norito_java/build.gradle.kts",
    ):
        source = (Path(__file__).resolve().parents[2] / relative).read_text(
            encoding="utf-8"
        )
        assert "SNAPSHOT" not in source
        assert '.orElse("0.1.0")' in source

    kotlin_readme = (
        Path(__file__).resolve().parents[2] / "kotlin/README.md"
    ).read_text(encoding="utf-8")
    assert "0.1-SNAPSHOT" not in kotlin_readme
    assert kotlin_readme.count(":0.1.0") >= 3


def test_gradle_property_rejects_duplicate_or_interpolated_versions(tmp_path: Path) -> None:
    source = tmp_path / "gradle.properties"
    source.write_text("sdkVersion=0.1.0\nsdkVersion=0.1.1\n", encoding="utf-8")
    with pytest.raises(ValueError, match="exactly once"):
        version_map._read_declared_version(source, "gradle-property", "sdkVersion")

    source.write_text("sdkVersion=${RELEASE_VERSION}\n", encoding="utf-8")
    with pytest.raises(ValueError, match="invalid or missing version"):
        version_map._read_declared_version(source, "gradle-property", "sdkVersion")


def test_npm_rejects_duplicate_version_members(tmp_path: Path) -> None:
    source = tmp_path / "package.json"
    source.write_text(
        '{"version":"0.1.0","version":"9.9.9"}',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="duplicate key"):
        version_map._read_declared_version(source, "npm")


def test_msbuild_rejects_duplicate_versions(tmp_path: Path) -> None:
    source = tmp_path / "package.csproj"
    source.write_text(
        "<Project><PropertyGroup><Version>0.1.0</Version>"
        "<Version>9.9.9</Version></PropertyGroup></Project>",
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="exactly once"):
        version_map._read_declared_version(source, "msbuild")


@pytest.mark.parametrize(
    "payload",
    ["0.1.0\nshadow\n", " 0.1.0\n", "0.1.0 \n", "0.1.0\r\n"],
)
def test_plain_semver_rejects_ambiguous_payloads(
    tmp_path: Path, payload: str
) -> None:
    source = tmp_path / "VERSION"
    source.write_text(payload, encoding="ascii")
    with pytest.raises(ValueError, match="invalid or missing version"):
        version_map._read_declared_version(source, "plain-semver")
