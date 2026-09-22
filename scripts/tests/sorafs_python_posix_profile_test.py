"""POSIX ABI3 wheel admission is independent of the artifact verifier's host."""
from __future__ import annotations

from copy import copy
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from sorafs_python_producer_inputs import POSIX_EXTENSION_SUFFIXES, verifier
from sorafs_python_dependency_install_test import harness


@pytest.mark.parametrize("native_name", ("_crypto.abi3.so", "_crypto.so", "_crypto.cpython-312-darwin.so", "_crypto.pyd"))
def test_original_native_name_uses_explicit_posix_abi3_profile(harness, tmp_path, monkeypatch, native_name):
    entries = []
    for info, raw in harness["valid_entries"]():
        if info.filename.endswith("/RECORD"):
            continue
        info = copy(info)
        if info.filename == "iroha_native/_crypto.abi3.so":
            info.filename = info.orig_filename = "iroha_native/" + native_name
        entries.append((info, raw))
    entries = harness["with_record"](entries, "iroha_native-0.0.1.dist-info/RECORD")
    path = harness["write_wheel"](tmp_path / "native.whl", entries)
    monkeypatch.setattr(verifier.importlib.machinery, "EXTENSION_SUFFIXES", [".pyd"])
    if native_name == "_crypto.abi3.so":
        parsed = verifier.parse_wheel_bytes(path.read_bytes(), extension_suffixes=POSIX_EXTENSION_SUFFIXES)
        assert parsed.native_member == "iroha_native/" + native_name
    else:
        with pytest.raises(verifier.VerificationError):
            verifier.parse_wheel_bytes(path.read_bytes(), extension_suffixes=POSIX_EXTENSION_SUFFIXES)
