"""Run the original wheel fixture harness and captured-byte parity without Cargo.

Fixtures remain owned by ci/privacy_sdk_cargo_lockfile_test.sh. This focused test
extracts and executes that exact inline Python harness unchanged in a fresh
isolated child, then reuses its actual helper objects and archives. No native SDK
binary executes; the original harness owns its explicitly inert loader fixture.
"""
from __future__ import annotations

import ast
from dataclasses import FrozenInstanceError
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import stat
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "ci/verify_privacy_python_wheel.py"
SHELL_HARNESS = ROOT / "ci/privacy_sdk_cargo_lockfile_test.sh"
_HARNESS_HEADER = "\n".join((
    'VERIFIER_FIXTURE_ROOT="${TEST_ROOT}/wheel-verifier"',
    '"${TEST_PYTHON}" -I -B - \\',
    '  "${SOURCE_ROOT}/ci/verify_privacy_python_wheel.py" \\',
    '  "${VERIFIER_FIXTURE_ROOT}" <<\'PY\'',
)) + "\n"


def extract_original_harness(source: bytes) -> str:
    """Select only the exact maintained wheel-verifier heredoc, without edits."""
    text = source.decode("utf-8", "strict")
    if text.count(_HARNESS_HEADER) != 1:
        raise AssertionError("expected one exact source-owned wheel harness")
    body, separator, _rest = text.split(_HARNESS_HEADER, 1)[1].partition("\nPY\n")
    if not separator:
        raise AssertionError("wheel harness has no exact terminating delimiter")
    ast.parse(body)
    return body + "\n"


def run_byte_controls(original: dict) -> int:
    """Reuse the original fixture owners, including their real ZIP/RECORD builders."""
    verifier = original["verifier"]
    fixture_root = original["fixture_root"]
    write_wheel = original["write_wheel"]
    with_record = original["with_record"]
    valid_entries = original["valid_entries"]
    sdk_entries = original["sdk_entries"]
    streamed_wheel_path = original["streamed_wheel_path"]
    seal = original["seal"]
    expect_failure = original["expect_failure"]
    hidden_gap_path = original["hidden_gap_path"]
    package_bytes = original["package_bytes"]
    record_hash = original["record_hash"]
    member = original["member"]
    sdk_dist = original["sdk_dist"]
    byte_path_parity_checks = 0

    def assert_byte_path_parity(path, *, owner=None, suffixes=(".abi3.so",)):
        """Exercise the same original fixture through both canonical entries."""
        nonlocal byte_path_parity_checks
        owner = verifier.NATIVE_OWNER if owner is None else owner
        raw = path.read_bytes()
        parsed = authenticated = None
        byte_error = path_error = None
        try:
            parsed = verifier.parse_wheel_bytes(raw, extension_suffixes=suffixes, owner=owner)
        except verifier.VerificationError as error:
            byte_error = error
        try:
            authenticated = verifier.preflight_wheel(
                path, seal(path), extension_suffixes=suffixes, owner=owner
            )
        except verifier.VerificationError as error:
            path_error = error
        byte_path_parity_checks += 1
        if (byte_error is None) != (path_error is None):
            raise AssertionError("captured-byte and sealed-path wheel acceptance differs")
        if byte_error is not None:
            if str(byte_error) != str(path_error):
                raise AssertionError(f"captured-byte/path diagnostic differs: {byte_error!s} / {path_error!s}")
            raise path_error
        if type(parsed) is not verifier.WheelArchive or type(authenticated) is not verifier.WheelPreflight:
            raise AssertionError("content layout acquired or lost original file authority")
        if hasattr(parsed, "path") or hasattr(parsed, "seal"):
            raise AssertionError("captured bytes must not fabricate a filesystem owner")
        if any(getattr(authenticated, field) != value for field, value in vars(parsed).items()):
            raise AssertionError("captured-byte layout differs from its original file preflight")
        if authenticated.path != path or authenticated.seal.render() != seal(path):
            raise AssertionError("file preflight did not retain its original path/seal")
        return authenticated

    # Revisit every actual archive left by the unchanged original harness.
    # Its original expected-outcome assertions already ran; here both entries
    # must still agree on the same concrete bytes, whether valid or adversarial.
    for candidate in sorted(fixture_root.glob("*.whl")):
        try:
            assert_byte_path_parity(candidate)
        except verifier.VerificationError:
            pass

    # Captured-byte consumers use exactly the file preflight's parser, without
    # inventing a path, seal, install owner or process-qualification result. Reuse
    # the original archive/RECORD helpers and run both entries on every new control.

    byte_fixture = write_wheel(fixture_root / "byte-owner-native.whl", valid_entries())
    byte_raw = byte_fixture.read_bytes()
    byte_sdk_fixture = write_wheel(fixture_root / "byte-owner-sdk.whl", sdk_entries)
    assert_byte_path_parity(byte_fixture)
    assert_byte_path_parity(byte_sdk_fixture, owner=verifier.SDK_OWNER)
    assert_byte_path_parity(streamed_wheel_path)
    content = verifier.parse_wheel_bytes(byte_raw, extension_suffixes=(".abi3.so",))
    try:
        content.metadata_version = "substituted"
    except FrozenInstanceError:
        pass
    else:
        raise AssertionError("captured wheel layout must be immutable")

    # No file operation or import may be needed to parse already captured bytes.
    original_file_reader = verifier._read_stable_regular_file
    original_importer = verifier.importlib.util.module_from_spec
    original_path_open = Path.open
    original_os_open = os.open

    def forbidden_byte_parser_io(*_args, **_kwargs):
        raise AssertionError("byte parser attempted filesystem or import authority")

    try:
        verifier._read_stable_regular_file = forbidden_byte_parser_io
        verifier.importlib.util.module_from_spec = forbidden_byte_parser_io
        Path.open = forbidden_byte_parser_io
        os.open = forbidden_byte_parser_io
        if verifier.parse_wheel_bytes(byte_raw, extension_suffixes=(".abi3.so",)) != content:
            raise AssertionError("immutable captured content changed")
    finally:
        verifier._read_stable_regular_file = original_file_reader
        verifier.importlib.util.module_from_spec = original_importer
        Path.open = original_path_open
        os.open = original_os_open

    class BytesAlias(bytes):
        pass

    for invalid_bytes in (bytearray(byte_raw), memoryview(byte_raw), BytesAlias(byte_raw), None, "wheel"):
        expect_failure("exact immutable bytes", lambda: verifier.parse_wheel_bytes(invalid_bytes))

    # The existing path owner must still authenticate its seal before delegating.
    original_parser = verifier.parse_wheel_bytes
    parser_calls = []
    def observed_parser(payload, **kwargs):
        parser_calls.append(payload)
        return original_parser(payload, **kwargs)
    try:
        verifier.parse_wheel_bytes = observed_parser
        verifier.preflight_wheel(byte_fixture, seal(byte_fixture), extension_suffixes=(".abi3.so",))
        if parser_calls != [byte_raw]:
            raise AssertionError("file entry must delegate once with the actual captured bytes")
        parser_calls.clear()
        expect_failure("shell-provided seal", lambda: verifier.preflight_wheel(byte_fixture, "0" * 64 + seal(byte_fixture)[64:]))
        if parser_calls:
            raise AssertionError("wrong file seal reached the content parser")
    finally:
        verifier.parse_wheel_bytes = original_parser

    for label, raw, fragment in (
        ("empty", b"", "byte size bound"),
        ("arbitrary", b"not an archive", "valid bounded ZIP archive"),
        ("truncated", byte_raw[:-1], "valid bounded ZIP archive"),
        ("appended", byte_raw + b"unreferenced", "canonical ZIP records"),
        ("prepended", b"unreferenced" + byte_raw, "prepended payload"),
    ):
        candidate = fixture_root / f"byte-{label}.whl"
        candidate.write_bytes(raw)
        expect_failure(fragment, lambda: assert_byte_path_parity(candidate))
    expect_failure("local records do not contiguously cover", lambda: assert_byte_path_parity(hidden_gap_path))

    # RECORD mutations remain structurally valid ZIPs so each reaches the named
    # coverage/hash assertion rather than failing an unrelated archive check.
    record_name = "iroha_native-0.0.0.dist-info/RECORD"
    record_payload = next(payload for info, payload in valid_entries() if info.filename == record_name)
    record_lines = record_payload.splitlines(keepends=True)
    for label, replacement, fragment in (
        ("missing-row", b"".join(record_lines[1:]), "cover every installed file"),
        ("duplicate-row", record_payload + record_lines[0], "duplicate member row"),
        ("wrong-hash", record_payload.replace(record_hash(package_bytes).encode(), record_hash(b"substituted").encode()), "hash or size"),
        ("noncanonical-size", record_payload.replace(f",{len(package_bytes)}\n".encode(), f",0{len(package_bytes)}\n".encode(), 1), "hash or size"),
        ("self-hash", record_payload.replace(f"{record_name},,\n".encode(), f"{record_name},{record_hash(b'')},0\n".encode()), "own hash and size"),
        ("foreign-row", record_payload + b"foreign.py,,\n", "unexpected member row"),
    ):
        entries = [(info, replacement if info.filename == record_name else payload) for info, payload in valid_entries()]
        candidate = write_wheel(fixture_root / f"byte-record-{label}.whl", entries)
        expect_failure(fragment, lambda: assert_byte_path_parity(candidate))

    expect_failure("package initializer", lambda: assert_byte_path_parity(byte_fixture, owner=verifier.SDK_OWNER))
    expect_failure("package initializer", lambda: assert_byte_path_parity(byte_sdk_fixture, owner=verifier.NATIVE_OWNER))
    foreign_owner = verifier.WheelOwner("foreign", "foreign", False)
    expect_failure("fixed native or SDK", lambda: assert_byte_path_parity(byte_fixture, owner=foreign_owner))
    for suffixes in ((), ("",), (".dylib",), ("../evil.so",), ("\\evil.pyd",)):
        expect_failure("invalid extension suffixes", lambda: assert_byte_path_parity(byte_fixture, suffixes=suffixes))
    expect_failure("current-platform native module", lambda: assert_byte_path_parity(byte_fixture, suffixes=(".pyd",)))

    for name, fragment in (
        ("iroha_native/_crypto.pyd", "non-current native module"),
        ("iroha_native/_crypto.py", "reserved native Python source path"),
        ("iroha_native/nested/_crypto.abi3.so", "non-current native module"),
        ("iroha_native/duplicated.PYD", "non-current native module"),
    ):
        candidate = write_wheel(fixture_root / "byte-foreign-extension.whl", with_record(valid_entries()[:-1] + [member(name)]))
        expect_failure(fragment, lambda: assert_byte_path_parity(candidate))

    # SDK ownership checks reach the same sole parser too; regenerate RECORD
    # after each metadata/native mutation so an unrelated digest does not reject it.
    for original, changed, fragment in (
        (b"Requires-Dist: iroha-native==0.0.0", b"Requires-Dist: iroha-native>=0.0.0", "exactly its matching native owner"),
        (b"Name: iroha-python", b"Name: foreign", "different distribution"),
        (b"Root-Is-Purelib: true", b"Root-Is-Purelib: false", "Root-Is-Purelib: true"),
        (b"Tag: py3-none-any", b"Tag: cp312-abi3-any", "platform-independent Python tags"),
    ):
        entries = [(info, payload.replace(original, changed)) for info, payload in sdk_entries if not info.filename.endswith("/RECORD")]
        candidate = write_wheel(fixture_root / "byte-sdk-owner.whl", with_record(entries, f"{sdk_dist}/RECORD"))
        expect_failure(fragment, lambda: assert_byte_path_parity(candidate, owner=verifier.SDK_OWNER))
    candidate = write_wheel(fixture_root / "byte-sdk-native.whl", with_record([entry for entry in sdk_entries if not entry[0].filename.endswith("/RECORD")] + [member("iroha_python/_crypto.abi3.so")], f"{sdk_dist}/RECORD"))
    expect_failure("must not contain a native module", lambda: assert_byte_path_parity(candidate, owner=verifier.SDK_OWNER))

    # Explicit package-root directories have only one PurePosixPath component.
    # They must parse normally while reserved direct child directories still refuse.
    for owner, entries, dist in ((verifier.NATIVE_OWNER, valid_entries(), "iroha_native-0.0.0.dist-info"),
                                  (verifier.SDK_OWNER, sdk_entries, sdk_dist)):
        originals = [entry for entry in entries if not entry[0].filename.endswith("/RECORD")]
        directory = member(owner.package + "/", b"", stat.S_IFDIR | 0o755)
        candidate = write_wheel(fixture_root / (owner.package + "-root-directory.whl"),
                                with_record(originals + [directory], dist + "/RECORD"))
        assert_byte_path_parity(candidate, owner=owner)
        reserved = member(owner.package + "/_crypto/", b"", stat.S_IFDIR | 0o755)
        candidate = write_wheel(fixture_root / (owner.package + "-reserved-directory.whl"),
                                with_record(originals + [directory, reserved], dist + "/RECORD"))
        expect_failure("reserved native Python source path", lambda: assert_byte_path_parity(candidate, owner=owner))

    # Exercise exact inclusive limits with small genuine archives; lower only one
    # test-local bound at a time and prove the same original passes at the boundary
    # and refuses one step below. Do not allocate enormous boundary payloads.
    with zipfile.ZipFile(io.BytesIO(byte_raw)) as archive:
        infos = archive.infolist()
        exact_bounds = (
            ("MAX_WHEEL_BYTES", len(byte_raw), "size bound"),
            ("MAX_ARCHIVE_MEMBERS", len(infos), "members"),
            ("MAX_MEMBER_BYTES", max(info.file_size for info in infos), "archive size bound"),
            ("MAX_TOTAL_UNCOMPRESSED_BYTES", sum(info.file_size for info in infos), "uncompressed-size"),
            ("MAX_MEMBER_NAME_BYTES", max(len(info.filename.encode("utf-8")) for info in infos), "path-length"),
            ("MAX_PATH_COMPONENTS", max(len(info.filename.split("/")) for info in infos), "path-depth"),
            ("MAX_COMPRESSION_RATIO", 1, "compression-ratio"),
        )
    for name, exact, fragment in exact_bounds:
        previous = getattr(verifier, name)
        try:
            setattr(verifier, name, exact)
            assert_byte_path_parity(byte_fixture)
            setattr(verifier, name, exact - 1)
            expect_failure(fragment, lambda: assert_byte_path_parity(byte_fixture))
        finally:
            setattr(verifier, name, previous)

    return byte_path_parity_checks

def _identity(path: Path) -> dict[str, object]:
    raw = path.read_bytes()
    return {"sha256": hashlib.sha256(raw).hexdigest(), "size": len(raw)}


def run_isolated(fixture_root: Path) -> dict:
    """Execute unchanged canonical harness source and bind the precise inputs."""
    sources = {"verifier": VERIFIER, "harness": SHELL_HARNESS, "controls": Path(__file__).resolve()}
    identities = {name: _identity(path) for name, path in sources.items()}
    source = SHELL_HARNESS.read_bytes()
    if hashlib.sha256(source).hexdigest() != identities["harness"]["sha256"]:
        raise AssertionError("harness changed before extraction")
    original = {"__name__": "__main__", "__file__": str(SHELL_HARNESS)}
    sys.argv = [str(SHELL_HARNESS), str(VERIFIER), str(fixture_root)]
    body = extract_original_harness(source)
    exec(compile(body, str(SHELL_HARNESS) + ":wheel-verifier-heredoc", "exec"), original)
    pairs = run_byte_controls(original)
    if {name: _identity(path) for name, path in sources.items()} != identities:
        raise AssertionError("source changed during wheel controls")
    return {"scope": "fixture-parser-controls-only", "byte_path_pairs": pairs,
            "original_harness_sha256": hashlib.sha256(body.encode()).hexdigest(),
            "sources": identities, "python": {"path": str(Path(sys.executable).resolve()),
            "identity": _identity(Path(sys.executable).resolve()), "version": sys.version}}


def test_original_wheel_harness_and_captured_bytes(tmp_path: Path) -> None:
    """Preserve every original assertion and add exact byte/path refusal parity."""
    result = subprocess.run(
        [sys.executable, "-I", "-B", str(Path(__file__).resolve()), "--run", str(tmp_path / "wheels")],
        capture_output=True, text=True, timeout=120, check=False,
    )
    (tmp_path / "controls.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "two-wheel bounded archive, installed-origin, loader, missing-owner and tamper checks passed" in result.stdout
    lines = [line.removeprefix("WHEEL_BYTE_OWNER_RESULT=") for line in result.stdout.splitlines() if line.startswith("WHEEL_BYTE_OWNER_RESULT=")]
    assert len(lines) == 1
    report = json.loads(lines[0])
    assert report["scope"] == "fixture-parser-controls-only"
    assert report["sources"] == {"verifier": _identity(VERIFIER), "harness": _identity(SHELL_HARNESS), "controls": _identity(Path(__file__).resolve())}
    assert report["original_harness_sha256"] == hashlib.sha256(extract_original_harness(SHELL_HARNESS.read_bytes()).encode()).hexdigest()
    assert report["byte_path_pairs"] == 80
    assert report["python"] == {"path": str(Path(sys.executable).resolve()), "identity": _identity(Path(sys.executable).resolve()), "version": sys.version}
    print(lines[0])


def test_harness_extraction_requires_its_exact_unique_source_owner() -> None:
    """Missing, ambiguous and unterminated source blocks cannot become a pass."""
    original = SHELL_HARNESS.read_bytes()
    body = extract_original_harness(original)
    assert body.startswith("import base64\n")
    assert "def valid_entries(" in body and "def with_record(" in body
    for mutation in (b"", original + _HARNESS_HEADER.encode(), _HARNESS_HEADER.encode() + b"pass\n"):
        try:
            extract_original_harness(mutation)
        except AssertionError:
            pass
        else:
            raise AssertionError("invalid original harness owner was accepted")


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] != "--run":
        raise SystemExit("usage: python_wheel_byte_owner_test.py --run FRESH_FIXTURE_DIRECTORY")
    print("WHEEL_BYTE_OWNER_RESULT=" + json.dumps(run_isolated(Path(sys.argv[2])), sort_keys=True))
