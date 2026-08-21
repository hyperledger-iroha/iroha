from __future__ import annotations

import argparse
import os
from pathlib import Path

import pytest

from scripts import assemble_taira_private_reset_source as assembler
from scripts import prepare_taira_empty_reset_bundle as reset_prepare


def test_direct_cli_help_works_in_isolated_mode() -> None:
    import subprocess
    import sys

    result = subprocess.run(
        [
            sys.executable,
            "-I",
            "-S",
            str(assembler.__file__),
            "--help",
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    assert "--matched-stage-root" in result.stdout


def private_dir(path: Path) -> Path:
    path.mkdir(mode=0o700)
    path.chmod(0o700)
    return path


def private_file(path: Path, body: bytes) -> Path:
    path.write_bytes(body)
    path.chmod(0o600)
    return path


def fixture(tmp_path: Path) -> argparse.Namespace:
    tmp_path.chmod(0o700)
    stage = private_dir(tmp_path / "stage")
    runtime = private_dir(stage / "runtime")
    private_file(runtime / "validator-roster.toml", b"[[validators]]\nslug='test'\n")
    private_file(
        runtime / "validator-secrets.toml",
        b"""
[shared]
account_onboarding_authority = "authority"
account_onboarding_private_key = "private"
account_onboarding_api_token = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
account_onboarding_credential_id = "legacy-dpn-api"
account_onboarding_scope_dataspace = "is2"
sorafs_council_public_keys = ["one", "two", "three"]
sorafs_council_signature_threshold = 2

[[validators]]
slug = "taira-validator-1"
private_key = "one"
""",
    )
    token = private_file(runtime / "dpn-token", b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    static = private_dir(tmp_path / "static")
    for slug in assembler.SLUGS:
        peer = private_dir(static / slug)
        for tree in ("codec", "configs"):
            root = private_dir(peer / tree)
            private_file(root / f"{tree}.bin", f"{slug}-{tree}".encode())
    inspection = assembler.inspect_inputs(stage, static)
    return argparse.Namespace(
        matched_stage_root=stage,
        trusted_stage_pair_sha256=inspection["matched_stage_pair_sha256"],
        dpn_token_file=token,
        static_rendered_root=static,
        trusted_static_inventory_sha256=inspection["static_inventory_sha256"],
        output=tmp_path / "output",
        inspect_only=False,
    )


def test_assembles_exact_source_envelope_and_migrates_credentials(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    args = fixture(tmp_path)
    monkeypatch.setattr(assembler.renderer, "load_roster", lambda *_args, **_kwargs: object())
    receipt = assembler.assemble(args)
    assert set(path.name for path in args.output.iterdir()) == reset_prepare.SOURCE_TOP_LEVEL_NAMES
    with (args.output / "validator-secrets.toml").open("rb") as stream:
        parsed = assembler.tomllib.load(stream)
    shared = parsed["shared"]
    assert "account_onboarding_api_token" not in shared
    assert shared["account_onboarding_credentials"] == [
        {
            "id": "boi-mobile",
            "api_token": "a" * 32,
            "scope_dataspace": "is2",
        },
        {"id": "dpn-api", "api_token": "b" * 32, "scope_dataspace": "dpn"},
    ]
    assert receipt["source_bundle_sha256"] == reset_prepare.source_bundle_sha256(args.output)
    assert receipt["static_file_count"] == 8
    assert receipt["static_directory_count"] == 8
    for root, directories, files in os.walk(args.output):
        assert Path(root).stat().st_mode & 0o777 == 0o700
        for name in files:
            assert (Path(root) / name).stat().st_mode & 0o777 == 0o600


def test_refuses_token_reuse_without_leaving_output(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    args.dpn_token_file.write_bytes(b"a" * 32)
    args.dpn_token_file.chmod(0o600)
    with pytest.raises(assembler.AssemblyError, match="must be distinct"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_retired_namespace_in_consumed_static_tree(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    bad = args.static_rendered_root / assembler.SLUGS[0] / "configs/configs.bin"
    bad.write_bytes(b"retired " + bytes.fromhex("574F4E4445524C414E44") + b" fixture")
    bad.chmod(0o600)
    with pytest.raises(assembler.AssemblyError, match="retired test namespace"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_stage_pair_changed_after_operator_inspection(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    secrets = args.matched_stage_root / "runtime/validator-secrets.toml"
    secrets.write_bytes(secrets.read_bytes() + b"\n# changed stage\n")
    secrets.chmod(0o600)
    with pytest.raises(assembler.AssemblyError, match="trusted roster/secrets digest"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_static_donor_changed_after_operator_inspection(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    donor = args.static_rendered_root / assembler.SLUGS[1] / "codec/codec.bin"
    donor.write_bytes(donor.read_bytes() + b"changed")
    donor.chmod(0o600)
    with pytest.raises(assembler.AssemblyError, match="trusted admitted inventory"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_empty_static_directory_added_after_inspection(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    private_dir(args.static_rendered_root / assembler.SLUGS[2] / "configs/empty")
    with pytest.raises(assembler.AssemblyError, match="trusted admitted inventory"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_retired_namespace_in_static_path(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    encoded = bytes.fromhex("776F6E6465726C616E64").decode("ascii")
    private_dir(args.static_rendered_root / assembler.SLUGS[3] / "codec" / encoded)
    with pytest.raises(assembler.AssemblyError, match="retired test namespace"):
        assembler.assemble(args)
    assert not args.output.exists()


def test_refuses_symlinked_static_input(tmp_path: Path) -> None:
    args = fixture(tmp_path)
    target = args.static_rendered_root / assembler.SLUGS[0] / "codec/codec.bin"
    target.unlink()
    target.symlink_to(args.matched_stage_root / "runtime/validator-roster.toml")
    with pytest.raises(assembler.AssemblyError, match="unsafe static source file"):
        assembler.assemble(args)
    assert not args.output.exists()
