"""Exact isolated staging controls; these tests confer no source-removal qualification."""

import hashlib
import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).parents[1] / "runtime_source_removal_candidate.py"
SPEC = importlib.util.spec_from_file_location("runtime_source_removal_candidate", SCRIPT)
candidate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(candidate)


def fixture(tmp_path):
    source = tmp_path / "original"
    source.mkdir()
    files = {"handler.rs": b"fn retired() {}\n", "lib.rs": b"mod handler;\nfn reader() {}\n"}
    for name, data in files.items():
        (source / name).write_bytes(data)
    request = {
        "inputs": sorted(files),
        "pinned": {name: candidate.fingerprint(data, (source / name).stat()) for name, data in files.items()},
        "reviewed": {name: hashlib.sha256(data).hexdigest() for name, data in files.items()},
        "omit": ["handler.rs"],
        "edits": [{"path": "lib.rs", "before": "mod handler;\n", "after": ""}],
    }
    return source, tmp_path / "candidate", files, request


def test_exact_isolated_omission_keeps_originals_and_retained_reader(tmp_path):
    source, output, files, request = fixture(tmp_path)
    candidate.stage(source, output, request)
    candidate.verify(source, output, request)
    assert not (output / "handler.rs").exists()
    assert (output / "lib.rs").read_bytes() == b"fn reader() {}\n"
    assert {name: (source / name).read_bytes() for name in files} == files
    assert (source / "lib.rs").stat().st_ino != (output / "lib.rs").stat().st_ino


def test_reappearing_handler_or_changed_reader_refuses_after_staging(tmp_path):
    source, output, _, request = fixture(tmp_path)
    candidate.stage(source, output, request)
    (output / "handler.rs").write_text("fn retired() {}")
    with pytest.raises(candidate.CandidateError, match="membership"):
        candidate.verify(source, output, request)
    (output / "handler.rs").unlink()
    (output / "lib.rs").write_text("fn another_reader() {}")
    with pytest.raises(candidate.CandidateError, match="candidate source changed"):
        candidate.verify(source, output, request)


@pytest.mark.parametrize("name", ["handler.rs", "lib.rs"])
def test_stale_handler_or_unreviewed_hook_edit_refuses_before_creating_output(tmp_path, name):
    source, output, _, request = fixture(tmp_path)
    with (source / name).open("ab") as stream:
        stream.write(b"// unreviewed\n")
    with pytest.raises(candidate.CandidateError, match="reviewed preimage"):
        candidate.stage(source, output, request)
    assert not output.exists()


def test_ambiguous_hook_refuses_and_removes_only_new_candidate(tmp_path):
    source, output, files, request = fixture(tmp_path)
    request["edits"][0]["before"] = "unknown hook"
    with pytest.raises(candidate.CandidateError, match="absent or ambiguous"):
        candidate.stage(source, output, request)
    assert not output.exists()
    assert {name: (source / name).read_bytes() for name in files} == files


@pytest.mark.parametrize("name", [".", "../escape", "/absolute", "target/cached.rs", ".git/config", "a//b", "C:/escape", "a\\escape"])
def test_invalid_input_names_refuse(tmp_path, name):
    source, output, _, request = fixture(tmp_path)
    request["inputs"] = sorted([*request["inputs"], name])
    with pytest.raises(candidate.CandidateError, match="noncanonical"):
        candidate.stage(source, output, request)
    assert not output.exists()


def test_existing_or_original_subtree_cannot_be_a_candidate(tmp_path):
    source, output, _, request = fixture(tmp_path)
    output.mkdir()
    with pytest.raises(candidate.CandidateError, match="already exists"):
        candidate.stage(source, output, request)
    with pytest.raises(candidate.CandidateError, match="outside"):
        candidate.stage(source, source / "candidate", request)


def test_linked_input_cannot_reenter_original_handler(tmp_path):
    source, output, _, request = fixture(tmp_path)
    (source / "reader.rs").symlink_to(source / "handler.rs")
    request["inputs"].append("reader.rs")
    request["inputs"].sort()
    request["pinned"]["reader.rs"] = {}
    with pytest.raises(candidate.CandidateError, match="regular file"):
        candidate.stage(source, output, request)
    assert not output.exists()


def test_reviewed_preimage_changed_between_initial_check_and_copy_refuses(tmp_path, monkeypatch):
    source, output, _, request = fixture(tmp_path)
    original = candidate._file
    reads = 0
    def racing(root, name):
        nonlocal reads
        reads += 1
        if reads == 3:
            (source / "handler.rs").write_bytes(b"fn changed() {}\n")
        return original(root, name)
    monkeypatch.setattr(candidate, "_file", racing)
    with pytest.raises(candidate.CandidateError, match="original build input changed"):
        candidate.stage(source, output, request)
    assert not output.exists()


def test_changed_original_and_candidate_cannot_repin_each_other(tmp_path):
    source, output, _, request = fixture(tmp_path)
    candidate.stage(source, output, request)
    (source / "lib.rs").write_bytes(b"mod handler;\nfn changed() {}\n")
    (output / "lib.rs").write_bytes(b"fn changed() {}\n")
    with pytest.raises(candidate.CandidateError, match="original build input changed"):
        candidate.verify(source, output, request)


def test_read_is_bounded_even_when_file_grows_after_stat(tmp_path, monkeypatch):
    source, _, _, _ = fixture(tmp_path)
    original = Path.open
    monkeypatch.setattr(candidate, "MAX_FILE_BYTES", 64)
    def growing(path, *args, **kwargs):
        if path == source / "handler.rs" and args == ("rb",):
            with original(path, "ab") as stream:
                stream.write(b"x" * 100)
        return original(path, *args, **kwargs)
    monkeypatch.setattr(Path, "open", growing)
    with pytest.raises(candidate.CandidateError, match="while reading"):
        candidate._file(source, "handler.rs")


def test_equal_byte_replacement_loses_original_identity(tmp_path):
    source, output, files, request = fixture(tmp_path)
    candidate.stage(source, output, request)
    replacement = source / "replacement"
    replacement.write_bytes(files["lib.rs"])
    replacement.replace(source / "lib.rs")
    with pytest.raises(candidate.CandidateError, match="original build input changed"):
        candidate.verify(source, output, request)


@pytest.mark.parametrize("name", ["sydneycreds", "scripts/sydneycreds.py", "scripts/sydney_credentials.py", "crates/runtime/private_key.pem", "defaults/.env"])
def test_legacy_and_runtime_credentials_are_never_opened(tmp_path, monkeypatch, name):
    def forbidden(*args, **kwargs):
        pytest.fail("credential path must refuse before any filesystem read")
    monkeypatch.setattr(Path, "lstat", forbidden)
    with pytest.raises(candidate.CandidateError, match="credential"):
        candidate._file(tmp_path, name)


def test_capture_selects_explicit_workspace_subtrees_without_reading_loose_secrets(tmp_path, monkeypatch):
    names = ["sydneycreds", "wallet/private.key", "scripts/sydneycreds.py", "docs/history/target/ignored.rs",
             "crates/example/src/secret.rs", "xtask/Cargo.toml", ".oss-scanner/build-features/Cargo.toml",
             "mochi/mochi-core/Cargo.toml", "experiments/kagemusha-spartan/Cargo.toml"]
    expected = names[4:]
    for name in expected:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"source")
    class Result:
        stdout = b"\0".join(name.encode() for name in names) + b"\0"
    monkeypatch.setattr(candidate.subprocess, "run", lambda *args, **kwargs: Result())
    assert list(candidate.capture(tmp_path)) == sorted(expected)


def test_reviewed_compiler_diagnostic_asset_does_not_admit_adjacent_credentials(tmp_path):
    name = "crates/kotodama_lang/src/assets/diagnostics_v1/secret_reject_cases_v1.tsv"
    path = tmp_path / name
    path.parent.mkdir(parents=True)
    path.write_bytes(b"static compiler diagnostic source\n")
    assert candidate._file(tmp_path, name)[0] == b"static compiler diagnostic source\n"
    for adjacent in ("secret_reject_cases_v2.tsv", "private_key.tsv", "secret_reject_cases_v1.tsv.key"):
        with pytest.raises(candidate.CandidateError, match="credential"):
            candidate._file(tmp_path, str(Path(name).with_name(adjacent)))


def test_exact_public_build_fixtures_do_not_admit_adjacent_assets(tmp_path, monkeypatch):
    expected = [
        "artifacts/poseidon/constants.ron",
        "codec/rans/tables/rans_seed0.toml",
        "contracts/evm/sccp/test/sccp_v1_fixtures.test.js",
        "javascript/iroha_js/test/fixtures/race-v1-codec.json",
        "javascript/iroha_js/test/fixtures/retail_fee_codec_v1.json",
        "specs/contract_state_value_proof_v1.md",
        "specs/runtime_baseline_catalog.json",
        "specs/runtime_baseline_instruction_layouts.tsv",
        "specs/runtime_ironwood_catalog.json",
        "specs/state_table_inventory.json",
        "specs/sumeragi.md",
        "specs/zk_operation_inventory.json",
    ]
    storage = "specs/runtime_baseline_storage_layouts.tsv"
    names = expected + ["artifacts/poseidon/unreviewed.ron", "codec/rans/tables/other.toml",
                        "javascript/iroha_js/test/fixtures/other.json", "specs/other.json"]
    for name in expected:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"public fixture")
    class Result:
        @property
        def stdout(self):
            return b"\0".join(name.encode() for name in names) + b"\0"
    monkeypatch.setattr(candidate.subprocess, "run", lambda *args, **kwargs: Result())
    assert list(candidate.capture(tmp_path)) == sorted(expected)
    # Capture never fabricates the optional fixture. Once the actual test creates
    # it and Git's name inventory includes it, the candidate carries those bytes.
    assert not (tmp_path / storage).exists()
    (tmp_path / storage).write_bytes(b"actual first capture")
    names.append(storage)
    captured = candidate.capture(tmp_path)
    assert list(captured) == sorted([*expected, storage])
    assert captured[storage]["sha256"] == hashlib.sha256(b"actual first capture").hexdigest()


@pytest.mark.parametrize("name", sorted(candidate.REVIEWED_SOURCE_DATA))
def test_reviewed_static_source_data_never_authorizes_an_adjacent_name(tmp_path, name):
    path = tmp_path / name
    path.parent.mkdir(parents=True)
    path.write_bytes(b"reviewed static public fixture")
    assert candidate._file(tmp_path, name)[0] == b"reviewed static public fixture"
    with pytest.raises(candidate.CandidateError, match="credential"):
        candidate._file(tmp_path, name + ".unreviewed")
    with pytest.raises(candidate.CandidateError, match="credential"):
        candidate._file(tmp_path, str(path.relative_to(tmp_path).with_name("private_key.txt")))
