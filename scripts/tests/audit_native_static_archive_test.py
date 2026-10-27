"""Exact archive, Cargo provenance, and refusal tests for RustCrypto packaging."""
from __future__ import annotations
import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "audit_native_static_archive.py"
SPEC = importlib.util.spec_from_file_location("audit_native_static_archive", SCRIPT)
AUDITOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDITOR)
MAGIC = b"!<arch>\n"

def raw_member(name: str, payload: bytes, *, bsd: bool = True, timestamp: int = 7) -> bytes:
    """Construct independent standard-ar fixtures, including BSD extended names."""
    name_bytes = name.encode("ascii")
    if bsd:
        header_name = f"#1/{len(name_bytes)}"
        body = name_bytes + payload
    else:
        assert len(name_bytes) <= 15
        header_name = name + "/"
        body = payload
    fields = (
        (header_name, 16),
        (str(timestamp), 12),
        ("12", 6),
        ("34", 6),
        ("100644", 8),
        (str(len(body)), 10),
    )
    header = b"".join(value.encode("ascii").ljust(width, b" ") for value, width in fields)
    assert len(header) == 58
    return header + b"`\n" + body + (b"\n" if len(body) % 2 else b"")



def fixture(tmp_path):
    tmp_path = tmp_path.resolve()
    build = tmp_path / "aarch64-apple-darwin/debug/build"
    build.mkdir(parents=True)
    deps = build.parent / "deps"; deps.mkdir()
    lock = tmp_path / "Cargo.lock"
    packages = []
    rows = []
    for name, version in (("ml-dsa", "0.1.1"), ("ml-kem", "0.3.3")):
        package = {"name": name, "version": version, "source": AUDITOR.REGISTRY, "checksum": "a" * 64}
        packages.append(package)
        path = deps / ("lib" + name.replace("-", "_") + "-abc123.rlib")
        path.write_bytes(MAGIC + raw_member("lib.rmeta", b"metadata") + raw_member(name + ".rcgu.o", b"object"))
        rows.append({"reason": "compiler-artifact", "package_id": AUDITOR.REGISTRY + "#" + name + "@" + version,
                     "target": {"name": name.replace("-", "_")}, "filenames": [str(path)]})
    rows.append({"reason": "build-finished", "success": True})
    lock.write_text("".join("[[package]]\n" + "".join(k + " = " + json.dumps(v) + "\n" for k, v in p.items()) for p in packages))
    messages = tmp_path / "cargo.jsonl"
    messages.write_text("".join(json.dumps(row) + "\n" for row in rows))
    return build, lock, messages, rows


def test_real_archive_format_has_no_rewrite():
    raw = MAGIC + raw_member("__.SYMDEF", b"index") + raw_member("ml_dsa-123.rcgu.o", b"abc") + raw_member("ml_kem-123.rcgu.o", b"def")
    before = hashlib.sha256(raw).hexdigest()
    assert AUDITOR.audit_archive_bytes(raw) == 2
    assert hashlib.sha256(raw).hexdigest() == before


@pytest.mark.parametrize("name", ("same.o", "libpqclean_common.a", "pqcrypto_internals-abc.rcgu.o", "keccak2x.o", "keccak4x.o"))
def test_duplicate_and_retired_objects_are_rejected(name):
    raw = MAGIC + raw_member(name, b"object") * (2 if name == "same.o" else 1)
    with pytest.raises(ValueError): AUDITOR.audit_archive_bytes(raw)


def test_renamed_private_pqclean_symbol_is_rejected():
    with pytest.raises(ValueError, match="retired"):
        AUDITOR.audit_archive_bytes(MAGIC + raw_member("hidden.o", b"symbol\x00PQCLEAN_MLKEM512_CLEAN_crypto_kem_dec"))


@pytest.mark.parametrize("defect", ("empty", "thin", "header", "body", "trailer", "size", "trailing", "gnu", "index_only", "path"))
def test_malformed_archives_are_rejected(defect):
    good = MAGIC + raw_member("object.o", b"payload")
    cases = {"empty": b"", "thin": b"!<thin>\n" + good[8:], "header": MAGIC + b"short",
             "body": good[:-2], "trailer": good[:66] + b"??" + good[68:],
             "size": good[:56] + b"not-a-size" + good[66:], "trailing": good + b"x",
             "gnu": MAGIC + raw_member("/", b"table", bsd=False),
             "index_only": MAGIC + raw_member("__.SYMDEF", b"index"),
             "path": MAGIC + raw_member("../outside.o", b"object")}
    with pytest.raises(ValueError): AUDITOR.audit_archive_bytes(cases[defect])


def test_cargo_providers_bind_lock_messages_and_actual_target_archives(tmp_path):
    build, lock, messages, _ = fixture(tmp_path)
    refs, provenance = AUDITOR.cargo_references(build, "aarch64-apple-darwin", messages, lock)
    assert len(refs) == 4
    assert provenance["cargo_lock_sha256"] == hashlib.sha256(lock.read_bytes()).hexdigest()
    assert set(provenance["providers"]) == {"ml-dsa", "ml-kem"}
    for provider in provenance["providers"].values():
        assert provider["artifact_sha256"] == hashlib.sha256(refs[provider["artifact"]]).hexdigest()


@pytest.mark.parametrize("defect", ("failed", "missing_terminal", "duplicate_terminal", "missing_provider", "duplicate_provider", "wrong_package", "wrong_target", "wrong_path", "malformed_rlib", "retired", "duplicate_json", "symlink"))
def test_cargo_provenance_refuses_substitution(tmp_path, defect):
    build, lock, messages, rows = fixture(tmp_path)
    if defect == "failed": rows[-1]["success"] = False
    elif defect == "missing_terminal": rows.pop()
    elif defect == "duplicate_terminal": rows.append(rows[-1])
    elif defect == "missing_provider": rows.pop(0)
    elif defect == "duplicate_provider": rows.insert(0, rows[0])
    elif defect == "wrong_package": rows[0]["package_id"] = "path+file:///vendor#ml-dsa@0.1.1"
    elif defect == "wrong_target": rows[0]["target"]["name"] = "other"
    elif defect == "wrong_path": rows[0]["filenames"][0] = str(build / "libml_dsa-abc.rlib")
    elif defect == "malformed_rlib": Path(rows[0]["filenames"][0]).write_bytes(b"not archive")
    elif defect == "retired": rows.insert(0, {"reason": "build-script-executed", "package_id": AUDITOR.REGISTRY + "#pqcrypto-internals@0.2.11"})
    elif defect == "symlink":
        path = Path(rows[0]["filenames"][0]); saved = path.with_suffix('.saved'); path.rename(saved); path.symlink_to(saved)
    messages.write_text("".join(json.dumps(row) + "\n" for row in rows))
    if defect == "duplicate_json": messages.write_text(messages.read_text().replace('"success": true', '"success": false, "success": true'))
    with pytest.raises((ValueError, RuntimeError)):
        AUDITOR.cargo_references(build, "aarch64-apple-darwin", messages, lock)


@pytest.mark.parametrize("defect", ("missing", "duplicate", "retired", "checksum", "source"))
def test_lock_provider_is_exact(tmp_path, defect):
    build, lock, messages, _ = fixture(tmp_path)
    raw = lock.read_text()
    if defect == "missing": raw = raw[:raw.rindex('[[package]]')]
    elif defect == "duplicate": raw += raw
    elif defect == "retired": raw += '[[package]]\nname = "pqcrypto-internals"\nversion = "0.2.11"\n'
    elif defect == "checksum": raw = raw.replace('a' * 64, 'not-checksum')
    elif defect == "source": raw = raw.replace(AUDITOR.REGISTRY, 'path+file:///vendor')
    lock.write_text(raw)
    with pytest.raises(ValueError): AUDITOR.cargo_references(build, "aarch64-apple-darwin", messages, lock)


def test_cli_audits_without_mutating_and_refuses_existing_receipt(tmp_path):
    build, lock, messages, _ = fixture(tmp_path)
    library = tmp_path.resolve() / "libbridge.a"
    original = MAGIC + raw_member("bridge.rcgu.o", b"native")
    library.write_bytes(original)
    report = tmp_path.resolve() / "report.json"
    command = [sys.executable, str(SCRIPT), "--library", str(library), "--cargo-build-dir", str(build),
               "--cargo-messages", str(messages), "--cargo-lock", str(lock), "--target", "aarch64-apple-darwin", "--report", str(report)]
    result = subprocess.run(command, capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    record = json.loads(report.read_text())
    assert record["archive_sha256"] == hashlib.sha256(original).hexdigest()
    assert record["schema"] == AUDITOR.SCHEMA
    assert library.read_bytes() == original
    saved = report.read_bytes()
    assert subprocess.run(command, capture_output=True).returncode != 0
    assert report.read_bytes() == saved
