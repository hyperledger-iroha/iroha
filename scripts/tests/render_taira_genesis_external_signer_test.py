from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess

import pytest

from scripts import render_taira_genesis_external_signer as signer


PUBLIC_KEY = "ed0120" + "AB" * 32


def private_file(path: Path, payload: bytes, mode: int) -> Path:
    path.write_bytes(payload)
    path.chmod(mode)
    return path


def render_fixture(tmp_path: Path, kagami_body: bytes | None = None) -> tuple[Path, Path, Path]:
    tmp_path.chmod(0o700)
    kagami = private_file(
        tmp_path / "kagami",
        kagami_body
        or b"#!/bin/sh\nset -eu\n"
        b"while [ $# -gt 0 ]; do\n"
        b"case \"$1\" in\n"
        b"--bound-manifest-out) bound=$2; shift 2;;\n"
        b"--out-file) signed=$2; shift 2;;\n"
        b"--expected-hash-out) expected=$2; shift 2;;\n"
        b"*) shift;;\n"
        b"esac\ndone\n"
        b"printf '{\"bound\":true}\\n' >\"$bound\"\n"
        b"printf signed >\"$signed\"\n"
        b"printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\n' >\"$expected\"\n",
        0o700,
    )
    key = private_file(tmp_path / "genesis-private-key", b"private\n", 0o600)
    output = tmp_path / "genesis-external-signer"
    receipt = signer.render(
        signer.parser().parse_args(
            [
                "--kagami",
                str(kagami),
                "--private-key",
                str(key),
                "--expected-public-key",
                PUBLIC_KEY,
                "--output",
                str(output),
            ]
        )
    )
    assert receipt == {
        "expected_public_key": PUBLIC_KEY,
        "kagami_sha256": hashlib.sha256(kagami.read_bytes()).hexdigest(),
        "signer_sha256": hashlib.sha256(output.read_bytes()).hexdigest(),
    }
    assert output.stat().st_mode & 0o777 == 0o700
    return output, kagami, key


def test_rendered_signer_maps_fixed_protocol_without_exposing_key(tmp_path: Path) -> None:
    output, _, key = render_fixture(tmp_path)
    unsigned = private_file(tmp_path / "unsigned.json", b"{}\n", 0o600)
    config = private_file(tmp_path / "config.toml", b"chain='taira'\n", 0o600)
    signed = tmp_path / "genesis.signed.nrt"
    expected = tmp_path / "genesis.expected_hash"
    completed = subprocess.run(
        [
            str(output),
            "--unsigned-genesis",
            str(unsigned),
            "--peer-config",
            str(config),
            "--bound-manifest-out",
            str(unsigned),
            "--signed-genesis-out",
            str(signed),
            "--expected-hash-out",
            str(expected),
        ],
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    assert signed.read_bytes() == b"signed"
    assert expected.read_text() == "a" * 64 + "\n"
    assert str(key) not in completed.stdout + completed.stderr


def test_render_refuses_mutable_inputs_and_existing_output(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    kagami = private_file(tmp_path / "kagami", b"#!/bin/sh\n", 0o722)
    key = private_file(tmp_path / "key", b"secret\n", 0o600)
    args = signer.parser().parse_args(
        ["--kagami", str(kagami), "--private-key", str(key),
         "--expected-public-key", PUBLIC_KEY, "--output", str(tmp_path / "signer")]
    )
    with pytest.raises(signer.SignerRenderError, match="executable"):
        signer.render(args)

    kagami.chmod(0o700)
    (tmp_path / "signer").write_text("occupied")
    with pytest.raises(signer.SignerRenderError, match="already exists"):
        signer.render(args)


def test_render_refuses_replaceable_input_ancestry(tmp_path: Path) -> None:
    tmp_path.chmod(0o700)
    unsafe = tmp_path / "unsafe"
    unsafe.mkdir(mode=0o770)
    unsafe.chmod(0o770)
    kagami = private_file(unsafe / "kagami", b"#!/bin/sh\n", 0o700)
    key_parent = tmp_path / "keys"
    key_parent.mkdir(mode=0o700)
    key = private_file(key_parent / "key", b"secret\n", 0o600)
    args = signer.parser().parse_args(
        ["--kagami", str(kagami), "--private-key", str(key),
         "--expected-public-key", PUBLIC_KEY, "--output", str(tmp_path / "signer")]
    )
    with pytest.raises(signer.SignerRenderError, match="replaceable ancestry"):
        signer.render(args)


def test_rendered_signer_executes_a_private_verified_snapshot(tmp_path: Path) -> None:
    marker_body = (
        b"#!/bin/sh\nset -eu\n"
        b"self=$0\n"
        b"while [ $# -gt 0 ]; do\n"
        b"case \"$1\" in\n"
        b"--bound-manifest-out) bound=$2; shift 2;;\n"
        b"--out-file) signed=$2; shift 2;;\n"
        b"--expected-hash-out) expected=$2; shift 2;;\n"
        b"--private-key-file) key=$2; shift 2;;\n"
        b"*) shift;;\n"
        b"esac\ndone\n"
        b"printf '{\"executable\":\"%s\",\"key\":\"%s\"}\\n' \"$self\" \"$key\" >\"$bound\"\n"
        b"printf signed >\"$signed\"\n"
        b"printf 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\\n' >\"$expected\"\n"
    )
    output, kagami, key = render_fixture(tmp_path, marker_body)
    unsigned = private_file(tmp_path / "unsigned.json", b"{}\n", 0o600)
    config = private_file(tmp_path / "config.toml", b"chain='taira'\n", 0o600)
    completed = subprocess.run(
        [str(output), "--unsigned-genesis", str(unsigned), "--peer-config", str(config),
         "--bound-manifest-out", str(unsigned), "--signed-genesis-out", str(tmp_path / "signed"),
         "--expected-hash-out", str(tmp_path / "hash")],
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    receipt = json.loads(unsigned.read_text())
    executed = receipt["executable"]
    assert executed != str(kagami)
    assert "/.taira-genesis-sign-" in executed
    assert receipt["key"] == str(key)
    assert not Path(executed).exists()
    assert list(tmp_path.glob(".taira-genesis-sign-*")) == []


def test_rendered_signer_detects_kagami_replacement(tmp_path: Path) -> None:
    output, kagami, _ = render_fixture(tmp_path)
    kagami.write_bytes(b"#!/bin/sh\nexit 0\n")
    kagami.chmod(0o700)
    completed = subprocess.run([str(output), "--help"], capture_output=True, text=True)
    # argparse help exits before custody access; one real invocation must enforce the digest.
    assert completed.returncode == 0
    unsigned = private_file(tmp_path / "unsigned.json", b"{}\n", 0o600)
    config = private_file(tmp_path / "config.toml", b"chain='taira'\n", 0o600)
    completed = subprocess.run(
        [str(output), "--unsigned-genesis", str(unsigned), "--peer-config", str(config),
         "--bound-manifest-out", str(unsigned), "--signed-genesis-out", str(tmp_path / "signed"),
         "--expected-hash-out", str(tmp_path / "hash")],
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 70
    assert "SHA-256 mismatch" in completed.stderr
