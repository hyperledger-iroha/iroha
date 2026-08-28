#!/usr/bin/env python3
"""Fail closed on retired developer-only executable surfaces.

This stdlib-only guard authenticates the four deleted Rust preimages through
Git objects, seals the reduced Iroha CLI target manifest, the wave-four Iroha
Core manifest postimage, and the unchanged lockfile, and requires the retained
production replacement markers.  Its mutation tests use in-memory snapshots,
so they cannot modify the checkout they protect.
"""

from __future__ import annotations

import hashlib
import json
import re
import stat
import subprocess
import unittest
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OPENING_COMMIT = "13768a6bed26d978ed159340f3d0f2536e3b626f"
CLI_MANIFEST = "crates/iroha_cli/Cargo.toml"
CORE_MANIFEST = "crates/iroha_core/Cargo.toml"
LOCKFILE = "Cargo.lock"
GUARD_PATH = "scripts/tests/developer_executable_retirement_source_test.py"


@dataclass(frozen=True)
class SourcePin:
    path: str
    blob: str
    sha256: str
    byte_count: int
    line_count: int
    tests: tuple[tuple[str, tuple[str, ...]], ...]
    test_ledger_sha256: str


SOURCE_PINS = (
    SourcePin(
        "crates/iroha_cli/src/bin/direct_asset_transfer.rs",
        "6c1b373bf5bac22dc7f1943fbda89508a424d997",
        "ae9ef13f684beb50d932c174da3179afa89b79f2b9972e4f4eca3143962f93e4",
        4_421,
        134,
        (("fee_payment_requires_explicit_consistent_payer_selection", ("#[test]",)),),
        "ce0b925a5a95747c0da10955bfc4e7d2c6ae0299e3e37f459c762f743be694a1",
    ),
    SourcePin(
        "crates/iroha_cli/src/bin/ivm_contract_call.rs",
        "edf2f11ee99e9cb5e95288b8c22c8cb3291e4e64",
        "162ef1a3e1d9eb038163dde4eca468c7288bf80e44b5b62dec2d9aa8dcca043c",
        19_068,
        502,
        (
            ("payload_digest_hex_hashes_empty_payload_when_absent", ("#[test]",)),
            ("payload_digest_hex_hashes_json_payload_contents", ("#[test]",)),
            (
                "contract_call_metadata_never_duplicates_the_argument_record_as_json",
                ("#[test]",),
            ),
        ),
        "758c601d465efd815b193cb852641a941ee3521dabbf899250d4d8a1b1a95013",
    ),
    SourcePin(
        "crates/iroha_cli/src/bin/split_contract_deploy.rs",
        "ac4e06bb7c56bd07c5b879c3eb53de963d63f41e",
        "eb6b4e7cb6ade4adaf9bba30bc15f9bf29f223a7c3afee1b3c29c0f94f9773d1",
        39_962,
        1_012,
        (
            ("private_key_file_accepts_one_exact_literal_with_terminal_newline", ("#[test]",)),
            (
                "private_key_file_rejects_surrounding_whitespace_without_echoing_secret",
                ("#[test]",),
            ),
            ("fee_payment_file_accepts_canonical_authority_gas_bound", ("#[test]",)),
            ("fee_payment_file_rejects_unknown_compatibility_fields", ("#[test]",)),
            (
                "private_key_file_rejects_group_readable_permissions",
                ("#[cfg(unix)]", "#[test]"),
            ),
            ("clap_surface_does_not_accept_inline_private_keys", ("#[test]",)),
            (
                "split_contract_deploy_fixture_uses_checked_ed25519_key_generation",
                ("#[test]",),
            ),
            ("sign_transaction_checked_helper_verifies", ("#[test]",)),
            (
                "commit_transaction_uses_native_nonce_cas_without_generic_metadata_write",
                ("#[test]",),
            ),
            ("native_upload_plan_rejects_empty_artifact", ("#[test]",)),
            ("native_upload_plan_rejects_noncanonical_code_hash", ("#[test]",)),
            (
                "one_chunk_upload_uploads_and_finalizes_without_reserved_nonce_mutation",
                ("#[test]",),
            ),
            (
                "multi_mib_upload_is_bounded_ordered_and_carries_stable_metadata",
                ("#[test]",),
            ),
            ("emit_sequence_writes_exact_ordered_native_filenames", ("#[test]",)),
        ),
        "37084338e0fca8ec3382c7639b337fbd0c9ceeafd8b201d515bf259a2af1aaa9",
    ),
    SourcePin(
        "crates/iroha_core/examples/bench_dag.rs",
        "6a5c0d9581271534b14de64af775cfc49850fb63",
        "dd0399bc65d69a937aa041f76783f76c2629b8ba60e1471f289afa032a91d9f0",
        43_224,
        1_271,
        (),
        "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
    ),
)
RETIRED_RUST_LINES = 2_919
RETIRED_TESTS = 18
RETIRED_COMPILER_UNITS = 4

OPENING_MANIFEST_BLOB = "c5ed5615017f22d7c5fe0a3d30a9d255724c84c8"
OPENING_MANIFEST_SHA256 = "0a1c9c6910a2e42e8e9945ee11c3850255e249d46fbf29dde13ba198a14c9cdd"
OPENING_MANIFEST_BYTES = 4_501
OPENING_MANIFEST_LINES = 147
HISTORICAL_POST_MANIFEST_SHA256 = (
    "0ace5038cbc52348fe44e1383b5e76f82d45a628d26f66198f6ee43863361a22"
)
HISTORICAL_POST_MANIFEST_BYTES = 4_161
HISTORICAL_POST_MANIFEST_LINES = 132
POST_MANIFEST_BLOB = "a79b0a8bff05782c1245de32f17fd044166ed166"
POST_MANIFEST_SHA256 = "74cb8d7551ff95e773b3be406903bf4c38855f495d81f5ba09408f4c00cedf67"
POST_MANIFEST_BYTES = 4_116
POST_MANIFEST_LINES = 131

OPENING_CORE_MANIFEST_BLOB = "1e80a31dbd28f6650dbd4dd0ff25decd19723024"
OPENING_CORE_MANIFEST_SHA256 = (
    "f30c641f5ad5a0287b72f7c2887ac8e70028c97d020f7b64ab4b4c43e415fa3e"
)
OPENING_CORE_MANIFEST_BYTES = 13_937
OPENING_CORE_MANIFEST_LINES = 375
HISTORICAL_CORE_MANIFEST_BLOB = "ded9b8d998f2d23294dd6a852bd9ac0ba79cb7d7"
HISTORICAL_CORE_MANIFEST_SHA256 = (
    "442489e3927220a2960d7cce218b3aa3729211c8d1502d0afdee842ab2dd0ab7"
)
HISTORICAL_CORE_MANIFEST_BYTES = 13_738
HISTORICAL_CORE_MANIFEST_LINES = 366
CORE_MANIFEST_BLOB = "79b745c6b2780b81b2a4c4abdca608780df501cf"
CORE_MANIFEST_SHA256 = "42e878358198e34aa5eaa4427c0ab9c65d4a3e331bbb09a53215da45cc3e8422"
CORE_MANIFEST_BYTES = 13_411
CORE_MANIFEST_LINES = 362

WAVE_FOUR_CONSOLIDATED_TEST_TABLES = (
    """[[test]]
name = "kaigi_privacy"
path = "tests/kaigi_privacy.rs"
required-features = ["zk-tests"]

""",
    """[[test]]
name = "kagemusha_artifact_v4_streaming"
path = "tests/kagemusha_artifact_v4_streaming.rs"

""",
)

OPENING_LOCK_BLOB = "bf7633694c3f2fdca07de4d99743a09bad2daa12"
OPENING_LOCK_SHA256 = "0ddb3f3938cf32035371317100674cd1601c3cb41232237f7a7d28b3aeab6222"
OPENING_LOCK_BYTES = 315_333
OPENING_LOCK_LINES = 13_758
LOCK_BLOB = "73decc2afc17e8aad2cfe1c4f83a57049d26f2cb"
LOCK_SHA256 = "d5b8bf5efbdc3ce2a8b1c0d2d75e1c5d1a343a072f836cfb76205bc6ea4cf15f"
LOCK_BYTES = 311_234
LOCK_LINES = 13_620

RETIRED_TABLES = (
    """[[bin]]
name = "direct_asset_transfer"
path = "src/bin/direct_asset_transfer.rs"
required-features = ["dev-tools"]

""",
    """[[bin]]
name = "ivm_contract_call"
path = "src/bin/ivm_contract_call.rs"
required-features = ["dev-tools"]

""",
    """[[bin]]
name = "split_contract_deploy"
path = "src/bin/split_contract_deploy.rs"
required-features = ["dev-tools"]

""",
)

EXPECTED_BIN_TABLES = (
    ("iroha", "src/bin/iroha.rs", ("cli",)),
    ("ivm_execution_keygen", "src/bin/ivm_execution_keygen.rs", ("dev-tools",)),
    (
        "account_literal_reencode",
        "src/bin/account_literal_reencode.rs",
        ("dev-tools",),
    ),
    ("gov_instruction", "src/bin/gov_instruction.rs", ("dev-tools",)),
    ("ivm_contract_deploy", "src/bin/ivm_contract_deploy.rs", ("dev-tools",)),
    (
        "taira_fee_sponsor_program",
        "src/bin/taira_fee_sponsor_program.rs",
        ("dev-tools",),
    ),
)

REPLACEMENT_MARKERS = {
    "crates/iroha_cli/src/main_shared.rs": (
        ("fn asset_transfer_instructions(", 1),
        (
            "iroha::data_model::isi::Transfer::asset_quantity(id, args.quantity.clone(), to.clone()),",
            1,
        ),
        ("fn ensure_flag_off_sends_transfer_only()", 1),
    ),
    "crates/iroha_cli/src/contracts.rs": (
        ("pub struct CallArgs {", 1),
        ("impl Run for CallArgs {", 1),
        ("fn resolve_contract_target(", 1),
        ("fn load_contract_payload_value(", 1),
        ("fn load_contract_payload_value_accepts_inline_json()", 1),
        ("fn resolve_contract_target_accepts_contract_alias()", 1),
    ),
    "crates/iroha_cli/src/bin/ivm_contract_deploy.rs": (
        ("fn read_contract_deployment_state(", 1),
        ("fn build_native_upload_transaction_plan(", 1),
        ("fn final_deployment_transaction_is_one_native_atomic_commit()", 1),
        ("fn multi_mib_upload_plan_is_bounded_ordered_and_stable()", 1),
    ),
}

RETIRED_IDENTIFIERS = (
    "direct_asset_transfer",
    "ivm_contract_call",
    "split_contract_deploy",
    "bench_dag",
)

TEST_RE = re.compile(
    r"(?P<attrs>(?:^[ \t]*#\[[^\n]+\]\n)+)"
    r"^[ \t]*(?:pub(?:\([^\n)]*\))?\s+)?(?:async\s+)?fn\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s*\(",
    re.MULTILINE,
)
TABLE_RE = re.compile(
    r"^\[\[(?P<kind>[^\]]+)\]\]\n(?P<body>.*?)(?=^\[\[|^\[[^[]|\Z)",
    re.MULTILINE | re.DOTALL,
)
FIELD_RE = re.compile(r'^([A-Za-z0-9_-]+)\s*=\s*"([^"]*)"$', re.MULTILINE)
FEATURE_RE = re.compile(r'^required-features\s*=\s*\[([^]]*)\]$', re.MULTILINE)


class GuardError(AssertionError):
    """Raised when the retirement source contract changes."""


@dataclass(frozen=True)
class Snapshot:
    files: dict[str, bytes | None]
    consumer_hits: tuple[str, ...]


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _git_blob_id(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def _git(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(
            ["git", *arguments],
            cwd=ROOT,
            check=check,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        raise GuardError(f"git {' '.join(arguments)} failed: {error}") from error


def _git_blob(blob: str) -> bytes:
    return _git("cat-file", "blob", blob).stdout


def _regular_bytes(relative: str) -> bytes:
    path = ROOT / relative
    _require(not path.is_symlink(), f"symlink is not allowed: {relative}")
    try:
        mode = path.stat().st_mode
    except OSError as error:
        raise GuardError(f"cannot stat {relative}: {error}") from error
    _require(stat.S_ISREG(mode), f"not a regular file: {relative}")
    try:
        path.resolve(strict=True).relative_to(ROOT.resolve(strict=True))
    except ValueError as error:
        raise GuardError(f"path escapes repository: {relative}") from error
    return path.read_bytes()


def _test_inventory(source: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    tests = []
    for match in TEST_RE.finditer(source):
        attributes = tuple(line.strip() for line in match.group("attrs").splitlines())
        if "#[test]" in attributes:
            tests.append((match.group("name"), attributes))
    return tuple(tests)


def _test_digest(tests: tuple[tuple[str, tuple[str, ...]], ...]) -> str:
    return _sha256(json.dumps(tests, separators=(",", ":")).encode("utf-8"))


def _authenticate_openings() -> bytes:
    commit_type = _git("cat-file", "-t", OPENING_COMMIT).stdout.strip()
    _require(commit_type == b"commit", "authenticated opening commit is unavailable")
    _require(
        sum(pin.line_count for pin in SOURCE_PINS) == RETIRED_RUST_LINES,
        "retired Rust line ledger changed",
    )
    _require(
        sum(len(pin.tests) for pin in SOURCE_PINS) == RETIRED_TESTS,
        "retired test ledger changed",
    )
    _require(len(SOURCE_PINS) == RETIRED_COMPILER_UNITS, "retired compiler-unit ledger changed")
    for pin in SOURCE_PINS:
        tree_blob = _git("rev-parse", f"{OPENING_COMMIT}:{pin.path}").stdout.decode().strip()
        _require(tree_blob == pin.blob, f"opening tree blob changed: {pin.path}")
        data = _git_blob(pin.blob)
        _require(len(data) == pin.byte_count, f"opening byte count changed: {pin.path}")
        _require(data.count(b"\n") == pin.line_count, f"opening line count changed: {pin.path}")
        _require(_sha256(data) == pin.sha256, f"opening content hash changed: {pin.path}")
        tests = _test_inventory(data.decode("utf-8"))
        _require(tests == pin.tests, f"opening test inventory changed: {pin.path}")
        _require(
            _test_digest(tests) == pin.test_ledger_sha256,
            f"opening test ledger hash changed: {pin.path}",
        )
    tree_manifest = _git("rev-parse", f"{OPENING_COMMIT}:{CLI_MANIFEST}").stdout.decode().strip()
    _require(tree_manifest == OPENING_MANIFEST_BLOB, "opening CLI manifest tree blob changed")
    manifest = _git_blob(OPENING_MANIFEST_BLOB)
    _require(len(manifest) == OPENING_MANIFEST_BYTES, "opening CLI manifest byte count changed")
    _require(
        manifest.count(b"\n") == OPENING_MANIFEST_LINES,
        "opening CLI manifest line count changed",
    )
    _require(_sha256(manifest) == OPENING_MANIFEST_SHA256, "opening CLI manifest hash changed")
    tree_core_manifest = _git(
        "rev-parse", f"{OPENING_COMMIT}:{CORE_MANIFEST}"
    ).stdout.decode().strip()
    _require(
        tree_core_manifest == OPENING_CORE_MANIFEST_BLOB,
        "opening iroha_core manifest tree blob changed",
    )
    core_manifest = _git_blob(OPENING_CORE_MANIFEST_BLOB)
    _require(
        len(core_manifest) == OPENING_CORE_MANIFEST_BYTES,
        "opening iroha_core manifest byte count changed",
    )
    _require(
        core_manifest.count(b"\n") == OPENING_CORE_MANIFEST_LINES,
        "opening iroha_core manifest line count changed",
    )
    _require(
        _sha256(core_manifest) == OPENING_CORE_MANIFEST_SHA256,
        "opening iroha_core manifest hash changed",
    )
    _expected_core_manifest(core_manifest)
    tree_lock = _git("rev-parse", f"{OPENING_COMMIT}:{LOCKFILE}").stdout.decode().strip()
    _require(tree_lock == OPENING_LOCK_BLOB, "opening Cargo.lock tree blob changed")
    lock = _git_blob(OPENING_LOCK_BLOB)
    _require(len(lock) == OPENING_LOCK_BYTES, "opening Cargo.lock byte count changed")
    _require(
        lock.count(b"\n") == OPENING_LOCK_LINES,
        "opening Cargo.lock line count changed",
    )
    _require(_sha256(lock) == OPENING_LOCK_SHA256, "opening Cargo.lock hash changed")
    return manifest


def _expected_manifest(opening: bytes) -> bytes:
    text = opening.decode("utf-8")
    for table in RETIRED_TABLES:
        _require(text.count(table) == 1, "retired target table opening count changed")
        text = text.replace(table, "", 1)
    postimage = text.encode("utf-8")
    _require(
        len(postimage) == HISTORICAL_POST_MANIFEST_BYTES,
        "historical derived manifest byte ledger changed",
    )
    _require(
        postimage.count(b"\n") == HISTORICAL_POST_MANIFEST_LINES,
        "historical derived manifest line ledger changed",
    )
    _require(
        _sha256(postimage) == HISTORICAL_POST_MANIFEST_SHA256,
        "historical derived manifest hash changed",
    )
    return postimage


def _expected_core_manifest(opening: bytes) -> bytes:
    text = opening.decode("utf-8")
    for table in WAVE_FOUR_CONSOLIDATED_TEST_TABLES:
        _require(text.count(table) == 1, "wave-four core target opening count changed")
        text = text.replace(table, "", 1)
    postimage = text.encode("utf-8")
    _require(
        len(postimage) == HISTORICAL_CORE_MANIFEST_BYTES,
        "historical derived core manifest byte ledger changed",
    )
    _require(
        postimage.count(b"\n") == HISTORICAL_CORE_MANIFEST_LINES,
        "historical derived core manifest line ledger changed",
    )
    _require(
        _sha256(postimage) == HISTORICAL_CORE_MANIFEST_SHA256,
        "historical derived core manifest hash changed",
    )
    _require(
        _git_blob_id(postimage) == HISTORICAL_CORE_MANIFEST_BLOB,
        "historical derived core manifest blob changed",
    )
    return postimage


def _bin_tables(manifest: str) -> tuple[tuple[str, str, tuple[str, ...]], ...]:
    rows = []
    for table in TABLE_RE.finditer(manifest):
        if table.group("kind") != "bin":
            continue
        body = table.group("body")
        fields = dict(FIELD_RE.findall(body))
        feature_match = FEATURE_RE.search(body)
        _require(feature_match is not None, "CLI bin table lost required-features")
        features = tuple(re.findall(r'"([^"]+)"', feature_match.group(1)))
        _require(set(fields) == {"name", "path"}, "CLI bin table field set changed")
        rows.append((fields["name"], fields["path"], features))
    return tuple(rows)


def _active_consumer_hits() -> tuple[str, ...]:
    arguments = ["grep", "--untracked", "-n", "-I", "-F"]
    for identifier in RETIRED_IDENTIFIERS:
        arguments.extend(("-e", identifier))
    arguments.extend(
        (
            "--",
            ".",
            ":(exclude)status.md",
            f":(exclude){GUARD_PATH}",
        )
    )
    result = _git(*arguments, check=False)
    _require(result.returncode in (0, 1), "active-consumer scan failed")
    if result.returncode == 1:
        return ()
    return tuple(line for line in result.stdout.decode("utf-8").splitlines() if line)


def _snapshot() -> Snapshot:
    paths = {CLI_MANIFEST, CORE_MANIFEST, LOCKFILE, *REPLACEMENT_MARKERS}
    files: dict[str, bytes | None] = {path: _regular_bytes(path) for path in paths}
    for pin in SOURCE_PINS:
        path = ROOT / pin.path
        if path.exists() or path.is_symlink():
            files[pin.path] = _regular_bytes(pin.path)
        else:
            files[pin.path] = None
    return Snapshot(files=files, consumer_hits=_active_consumer_hits())


def _validate(snapshot: Snapshot, opening_manifest: bytes) -> None:
    for pin in SOURCE_PINS:
        _require(snapshot.files[pin.path] is None, f"retired source resurrected: {pin.path}")

    manifest = snapshot.files[CLI_MANIFEST]
    _require(manifest is not None, "CLI manifest is missing")
    _expected_manifest(opening_manifest)
    _require(len(manifest) == POST_MANIFEST_BYTES, "CLI manifest byte count changed")
    _require(
        manifest.count(b"\n") == POST_MANIFEST_LINES,
        "CLI manifest line count changed",
    )
    _require(_sha256(manifest) == POST_MANIFEST_SHA256, "CLI manifest postimage hash drifted")
    _require(_git_blob_id(manifest) == POST_MANIFEST_BLOB, "CLI manifest postimage blob drifted")
    manifest_text = manifest.decode("utf-8")
    _require(
        len(re.findall(r"^autobins\s*=\s*false$", manifest_text, re.MULTILINE)) == 1,
        "CLI autobins=false contract changed",
    )
    _require(_bin_tables(manifest_text) == EXPECTED_BIN_TABLES, "CLI bin target ledger changed")
    _require(
        manifest_text.count('ivm = { workspace = true }') == 1
        and '"ivm/runtime"' not in manifest_text,
        "CLI IVM feature authority changed",
    )
    for identifier in RETIRED_IDENTIFIERS[:3]:
        _require(identifier not in manifest_text, f"retired CLI target resurrected: {identifier}")

    core_manifest = snapshot.files[CORE_MANIFEST]
    _require(core_manifest is not None, "iroha_core manifest is missing")
    _expected_core_manifest(_git_blob(OPENING_CORE_MANIFEST_BLOB))
    _require(len(core_manifest) == CORE_MANIFEST_BYTES, "iroha_core manifest byte count changed")
    _require(
        core_manifest.count(b"\n") == CORE_MANIFEST_LINES,
        "iroha_core manifest line count changed",
    )
    _require(
        _sha256(core_manifest) == CORE_MANIFEST_SHA256,
        "iroha_core manifest content changed",
    )
    _require(
        _git_blob_id(core_manifest) == CORE_MANIFEST_BLOB,
        "iroha_core manifest Git blob changed",
    )
    core_text = core_manifest.decode("utf-8")
    _require(
        not re.search(r"^autoexamples\s*=\s*false$", core_text, re.MULTILINE),
        "iroha_core autoexample discovery contract changed",
    )
    _require("bench_dag" not in core_text, "retired bench_dag target resurrected in manifest")
    _require(
        core_text.count('ivm = { workspace = true }') == 1
        and 'name = "sccp_security"' in core_text
        and 'required-features = ["iroha-core-tests"]' in core_text
        and "zk-halo2-ipa-poseidon" not in core_text
        and "finality-test-fixtures" not in core_text,
        "iroha_core consolidated target and feature authority changed",
    )

    lock = snapshot.files[LOCKFILE]
    _require(lock is not None, "Cargo.lock is missing")
    _require(len(lock) == LOCK_BYTES, "Cargo.lock byte count changed")
    _require(lock.count(b"\n") == LOCK_LINES, "Cargo.lock line count changed")
    _require(_sha256(lock) == LOCK_SHA256, "Cargo.lock hash changed")
    _require(lock == _git_blob(LOCK_BLOB), "Cargo.lock differs from current authority")

    for path, markers in REPLACEMENT_MARKERS.items():
        data = snapshot.files[path]
        _require(data is not None, f"replacement source is missing: {path}")
        source = data.decode("utf-8")
        for marker, count in markers:
            _require(
                source.count(marker) == count,
                f"replacement marker changed in {path}: {marker}",
            )
    _require(not snapshot.consumer_hits, f"active retired-surface consumer found: {snapshot.consumer_hits}")


def _mutate(snapshot: Snapshot, path: str, data: bytes | None) -> Snapshot:
    files = dict(snapshot.files)
    files[path] = data
    return Snapshot(files=files, consumer_hits=snapshot.consumer_hits)


class DeveloperExecutableRetirementSourceTest(unittest.TestCase):
    """Authenticate the retirement and prove representative mutations fail."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.opening_manifest = _authenticate_openings()
        cls.snapshot = _snapshot()

    def test_retirement_contract(self) -> None:
        _validate(self.snapshot, self.opening_manifest)

    def test_mutation_deleted_file_resurrection_fails(self) -> None:
        mutated = _mutate(self.snapshot, SOURCE_PINS[0].path, b"fn main() {}\n")
        with self.assertRaisesRegex(GuardError, "retired source resurrected"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_deleted_table_resurrection_fails(self) -> None:
        manifest = self.snapshot.files[CLI_MANIFEST]
        assert manifest is not None
        mutated = _mutate(self.snapshot, CLI_MANIFEST, manifest + RETIRED_TABLES[0].encode())
        with self.assertRaisesRegex(GuardError, "manifest"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_manifest_drift_fails(self) -> None:
        manifest = self.snapshot.files[CLI_MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            CLI_MANIFEST,
            manifest.replace(b"description.workspace = true", b"description = \"drift\"", 1),
        )
        with self.assertRaisesRegex(GuardError, "manifest"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_dependency_drift_fails(self) -> None:
        manifest = self.snapshot.files[CLI_MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            CLI_MANIFEST,
            manifest.replace(b"derive_more = { workspace = true }\n", b"", 1),
        )
        with self.assertRaisesRegex(GuardError, "manifest"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_core_autoexamples_comment_fails(self) -> None:
        manifest = self.snapshot.files[CORE_MANIFEST]
        assert manifest is not None
        mutated = _mutate(
            self.snapshot,
            CORE_MANIFEST,
            manifest.replace(
                b"autobins = false\n",
                b"autobins = false\nautoexamples = false # comment\n",
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "iroha_core manifest"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_core_wave_four_target_resurrection_fails(self) -> None:
        manifest = self.snapshot.files[CORE_MANIFEST]
        assert manifest is not None
        anchor = b'[[test]]\nname = "swift_confidential_unshield_redeem"\n'
        mutated = _mutate(
            self.snapshot,
            CORE_MANIFEST,
            manifest.replace(
                anchor,
                WAVE_FOUR_CONSOLIDATED_TEST_TABLES[0].encode("utf-8") + anchor,
                1,
            ),
        )
        with self.assertRaisesRegex(GuardError, "iroha_core manifest"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_active_reference_fails(self) -> None:
        mutated = Snapshot(
            files=dict(self.snapshot.files),
            consumer_hits=("README.md:1:cargo run --example bench_dag",),
        )
        with self.assertRaisesRegex(GuardError, "active retired-surface consumer"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_replacement_marker_fails(self) -> None:
        path = "crates/iroha_cli/src/contracts.rs"
        source = self.snapshot.files[path]
        assert source is not None
        mutated = _mutate(
            self.snapshot,
            path,
            source.replace(b"impl Run for CallArgs {", b"impl Run for RemovedCallArgs {", 1),
        )
        with self.assertRaisesRegex(GuardError, "replacement marker"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_lock_fails(self) -> None:
        lock = self.snapshot.files[LOCKFILE]
        assert lock is not None
        mutated = _mutate(self.snapshot, LOCKFILE, lock + b"# drift\n")
        with self.assertRaisesRegex(GuardError, "Cargo.lock"):
            _validate(mutated, self.opening_manifest)

    def test_mutation_extra_dev_target_fails(self) -> None:
        manifest = self.snapshot.files[CLI_MANIFEST]
        assert manifest is not None
        extra = b'''[[bin]]
name = "extra_dev_target"
path = "src/bin/extra_dev_target.rs"
required-features = ["dev-tools"]

'''
        mutated = _mutate(
            self.snapshot,
            CLI_MANIFEST,
            manifest.replace(b"[dependencies]\n", extra + b"[dependencies]\n", 1),
        )
        with self.assertRaisesRegex(GuardError, "manifest"):
            _validate(mutated, self.opening_manifest)


if __name__ == "__main__":
    unittest.main()
