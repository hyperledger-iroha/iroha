"""Exact-byte trust and malformed-input tests for Apple pqcrypto archive normalization."""

from __future__ import annotations

import importlib.util
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "normalize_pqcrypto_archive.py"
SPEC = importlib.util.spec_from_file_location("normalize_pqcrypto_archive", SCRIPT)
assert SPEC and SPEC.loader
NORMALIZER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = NORMALIZER
SPEC.loader.exec_module(NORMALIZER)

MAGIC = b"!<arch>\n"
COMMON_NAMES = tuple(
    f"0123456789abcdef-{name}.o"
    for name in ("aes", "fips202", "sha2", "nistseedexpander", "sp800-185")
)
ARCH_NAMES = {
    "libkeccak2x.a": (
        "fedcba9876543210-fips202x2.o",
        "fedcba9876543210-feat.o",
    ),
    "libkeccak4x.a": ("fedcba9876543210-KeccakP-1600-times4-SIMD256.o",),
}


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


def payload(name: str) -> bytes:
    return b"object-bytes\x00" + name.encode("ascii") + b"\xff"


def reference_fixture(architecture: str = "libkeccak2x.a"):
    names = COMMON_NAMES + ARCH_NAMES[architecture]
    refs = {
        "libpqclean_common.a": MAGIC
        + raw_member("__.SYMDEF SORTED", b"trusted common symbols")
        + b"".join(raw_member(name, payload(name)) for name in COMMON_NAMES),
        architecture: MAGIC
        + b"".join(raw_member(name, payload(name)) for name in ARCH_NAMES[architecture]),
    }
    objects = [raw_member(name, payload(name)) for name in names]
    return refs, names, objects


@pytest.mark.parametrize("architecture", ARCH_NAMES)
def test_removes_only_second_trusted_objects_and_preserves_other_members(architecture):
    refs, names, objects = reference_fixture(architecture)
    unique_before = raw_member("first.o", b"untouched\x00odd", bsd=False, timestamp=11)
    unique_after = raw_member("unrelated-long-object-name.o", b"\x00\xfftail", timestamp=29)
    symbol_index = raw_member("__.SYMDEF SORTED", b"old offsets")
    repeated = [raw_member(name, payload(name), timestamp=99) for name in names]
    original = MAGIC + symbol_index + unique_before + b"".join(objects + repeated) + unique_after

    normalized, removed = NORMALIZER.normalize_archive_bytes(original, refs)

    assert normalized == MAGIC + unique_before + b"".join(objects) + unique_after
    assert len(removed) == len(names)
    assert set(removed) == set(names)
    assert NORMALIZER.normalize_archive_bytes(normalized, refs) == (normalized, [])


@pytest.mark.parametrize("architecture", ARCH_NAMES)
def test_already_normalized_archive_including_symbol_index_is_byte_unchanged(architecture):
    refs, _, objects = reference_fixture(architecture)
    original = MAGIC + raw_member("__.SYMDEF", b"keep these offsets", bsd=False) + b"".join(objects)
    assert NORMALIZER.normalize_archive_bytes(original, refs) == (original, [])


def test_identical_unknown_duplicate_is_rejected():
    refs, _, objects = reference_fixture()
    unrelated = raw_member("other.o", b"identical", bsd=False)
    with pytest.raises(ValueError, match="unknown duplicate"):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects) + unrelated * 2, refs)


@pytest.mark.parametrize("changed_copy", ["first", "second", "both"])
def test_trusted_name_never_authorizes_conflicting_payloads(changed_copy):
    refs, names, objects = reference_fixture()
    first = list(objects)
    if changed_copy in ("first", "both"):
        first[0] = raw_member(names[0], b"different object")
    repeated_payload = b"different object" if changed_copy in ("second", "both") else payload(names[0])
    original = MAGIC + b"".join(first) + raw_member(names[0], repeated_payload)
    with pytest.raises(ValueError):
        NORMALIZER.normalize_archive_bytes(original, refs)


def test_three_identical_trusted_copies_are_rejected():
    refs, _, objects = reference_fixture()
    with pytest.raises(ValueError, match="more than twice"):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects) + objects[0] * 2, refs)


def test_reference_payload_must_authenticate_both_input_copies():
    refs, names, objects = reference_fixture()
    refs["libpqclean_common.a"] = MAGIC + b"".join(
        raw_member(name, b"wrong reference" if name == names[0] else payload(name))
        for name in COMMON_NAMES
    )
    with pytest.raises(ValueError):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects) + objects[0], refs)


@pytest.mark.parametrize("defect", ["missing", "extra", "duplicate", "wrong_prefix"])
def test_reference_archive_inventory_is_exact(defect):
    refs, _, objects = reference_fixture()
    common = [raw_member(name, payload(name)) for name in COMMON_NAMES]
    if defect == "missing":
        common.pop()
    elif defect == "extra":
        common.append(raw_member("unexpected.o", b"extra", bsd=False))
    elif defect == "duplicate":
        common.append(common[0])
    else:
        common[0] = raw_member("wrong-aes.o", payload(COMMON_NAMES[0]))
    refs["libpqclean_common.a"] = MAGIC + b"".join(common)
    with pytest.raises(ValueError):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects) + objects[0], refs)


@pytest.mark.parametrize("defect", ["missing_common", "missing_architecture", "unknown", "both_architectures"])
def test_reference_archive_names_are_closed(defect):
    refs, _, objects = reference_fixture()
    if defect == "missing_common":
        del refs["libpqclean_common.a"]
    elif defect == "missing_architecture":
        del refs["libkeccak2x.a"]
    elif defect == "unknown":
        refs["other.a"] = refs["libkeccak2x.a"]
    else:
        other, _, _ = reference_fixture("libkeccak4x.a")
        refs["libkeccak4x.a"] = other["libkeccak4x.a"]
    with pytest.raises(ValueError):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects), refs)


def test_every_trusted_member_must_be_present_in_input():
    refs, _, objects = reference_fixture()
    with pytest.raises(ValueError):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects[:-1]), refs)


@pytest.mark.parametrize("defect", ["empty", "thin", "truncated_header", "truncated_body", "bad_trailer", "bad_size", "trailing_byte", "gnu_names"])
def test_malformed_input_archives_fail_closed(defect):
    refs, _, objects = reference_fixture()
    good = MAGIC + b"".join(objects)
    cases = {
        "empty": b"",
        "thin": b"!<thin>\n" + good[len(MAGIC):],
        "truncated_header": MAGIC + b"short",
        "truncated_body": good[:-2],
        "bad_trailer": good[:66] + b"??" + good[68:],
        "bad_size": good[:56] + b"not-a-size" + good[66:],
        "trailing_byte": good + b"x",
        "gnu_names": MAGIC + raw_member("/", b"unsupported GNU names", bsd=False) + b"".join(objects),
    }
    with pytest.raises(ValueError, match="archive"):
        NORMALIZER.normalize_archive_bytes(cases[defect], refs)


def test_malformed_reference_archive_fails_even_when_input_needs_no_normalization():
    refs, _, objects = reference_fixture()
    refs["libkeccak2x.a"] = b"not an archive"
    with pytest.raises(ValueError, match="archive"):
        NORMALIZER.normalize_archive_bytes(MAGIC + b"".join(objects), refs)


def cargo_fixture(build_dir: Path, suffix: str = "01234567"):
    refs, names, objects = reference_fixture()
    candidate = build_dir / f"pqcrypto-internals-{suffix}"
    out_dir = candidate / "out"
    out_dir.mkdir(parents=True)
    for name, data in refs.items():
        (out_dir / name).write_bytes(data)
    output = candidate / "output"
    output.write_text(
        f"cargo:rustc-link-search=native={out_dir.resolve()}\n"
        "cargo:rustc-link-lib=static=pqclean_common\n"
        "cargo:rustc-link-lib=static=keccak2x\n",
        encoding="utf-8",
    )
    return refs, names, objects, output


def test_cargo_reference_provenance_binds_recorded_search_path_and_file_hashes(tmp_path):
    refs, _, _, output = cargo_fixture(tmp_path)
    actual, provenance = NORMALIZER.cargo_references(tmp_path, "aarch64-apple-ios")
    assert actual == refs
    assert provenance["cargo_build_output"] == str(output.resolve())
    assert provenance["cargo_build_output_sha256"] == hashlib.sha256(output.read_bytes()).hexdigest()
    for name, data in refs.items():
        assert provenance[name] == hashlib.sha256(data).hexdigest()


def test_two_matching_cargo_outputs_are_rejected(tmp_path):
    cargo_fixture(tmp_path, "0123")
    cargo_fixture(tmp_path, "abcd")
    with pytest.raises(ValueError, match="exactly one"):
        NORMALIZER.cargo_references(tmp_path, "aarch64-apple-ios")


@pytest.mark.parametrize("defect", ["different_search_path", "missing_common_link", "wrong_architecture_link"])
def test_cargo_reference_requires_its_exact_recorded_link_directives(tmp_path, defect):
    _, _, _, output = cargo_fixture(tmp_path)
    text = output.read_text()
    if defect == "different_search_path":
        text = text.replace(str(output.parent / "out"), str(tmp_path / "other"))
    elif defect == "missing_common_link":
        text = text.replace("cargo:rustc-link-lib=static=pqclean_common\n", "")
    else:
        text = text.replace("static=keccak2x", "static=keccak4x")
    output.write_text(text)
    with pytest.raises(ValueError, match="exactly one"):
        NORMALIZER.cargo_references(tmp_path, "aarch64-apple-ios")


def test_cargo_reference_rejects_a_malformed_archive_inventory(tmp_path):
    _, _, _, output = cargo_fixture(tmp_path)
    (output.parent / "out" / "libpqclean_common.a").write_bytes(
        MAGIC + raw_member(COMMON_NAMES[0], payload(COMMON_NAMES[0]))
    )
    with pytest.raises(ValueError, match="reference"):
        NORMALIZER.cargo_references(tmp_path, "aarch64-apple-ios")


def test_read_regular_rejects_symbolic_provenance_files(tmp_path):
    source = tmp_path / "original.a"
    source.write_bytes(MAGIC)
    alias = tmp_path / "alias.a"
    alias.symlink_to(source)
    assert NORMALIZER.read_regular(source) == MAGIC
    with pytest.raises(ValueError, match="regular file"):
        NORMALIZER.read_regular(alias)


@pytest.mark.parametrize("version", ["0.2.10", "0.2.12"])
def test_cli_rejects_any_other_locked_pqcrypto_version_without_writing(tmp_path, version):
    _, _, objects, _ = cargo_fixture(tmp_path / "build")
    original = MAGIC + b"".join(objects) + objects[0]
    library = tmp_path / "library.a"
    library.write_bytes(original)
    lock = tmp_path / "Cargo.lock"
    lock.write_text(f'[[package]]\nname = "pqcrypto-internals"\nversion = "{version}"\n')
    report = tmp_path / "report.json"
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--library", str(library),
         "--cargo-build-dir", str(tmp_path / "build"), "--target", "aarch64-apple-ios",
         "--cargo-lock", str(lock), "--report", str(report)],
        capture_output=True, text=True, check=False, timeout=10,
    )
    assert result.returncode == 1
    assert "locked pqcrypto-internals 0.2.11" in result.stderr
    assert library.read_bytes() == original
    assert not report.exists()


def test_cli_records_the_exact_authenticated_input_and_output(tmp_path):
    refs, names, objects, _ = cargo_fixture(tmp_path / "build")
    original = MAGIC + b"".join(objects) + objects[0]
    library = tmp_path / "library.a"
    library.write_bytes(original)
    lock = tmp_path / "Cargo.lock"
    lock.write_text('[[package]]\nname = "pqcrypto-internals"\nversion = "0.2.11"\n')
    report = tmp_path / "report.json"
    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--library", str(library),
         "--cargo-build-dir", str(tmp_path / "build"), "--target", "aarch64-apple-ios",
         "--cargo-lock", str(lock), "--report", str(report)],
        capture_output=True, text=True, check=False, timeout=10,
    )
    assert result.returncode == 0, result.stderr
    assert library.read_bytes() == MAGIC + b"".join(objects)
    record = json.loads(report.read_text())
    assert record["input_sha256"] == hashlib.sha256(original).hexdigest()
    assert record["unindexed_output_sha256"] == hashlib.sha256(library.read_bytes()).hexdigest()
    assert record["removed_identical_members"] == [names[0]]
    for name, data in refs.items():
        assert record["references"][name] == hashlib.sha256(data).hexdigest()
