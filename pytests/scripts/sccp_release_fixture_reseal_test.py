"""Adversarial tests for the public-only SCCP fixture reseal workflow."""

from __future__ import annotations

import base64
import copy
import hashlib
import os
import shutil
import stat
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import sccp_release_common as common  # noqa: E402
import sccp_release_fixture_reseal as reseal  # noqa: E402

REAL_REQUIRE_FORBIDDEN_KEY_REGISTRATION = reseal._require_forbidden_key_registration
REAL_VALIDATE_STAGED_WITH_RUST = reseal._validate_staged_with_rust


def _keypair(label: bytes) -> tuple[bytes, bytes, int]:
    seed = hashlib.sha256(label).digest()
    digest = hashlib.sha512(seed).digest()
    scalar_bytes = bytearray(digest[:32])
    scalar_bytes[0] &= 248
    scalar_bytes[31] &= 63
    scalar_bytes[31] |= 64
    scalar = int.from_bytes(scalar_bytes, "little")
    public = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, scalar))
    return public, digest[32:], scalar


def _sign(keypair: tuple[bytes, bytes, int], message: bytes) -> str:
    public, prefix, scalar = keypair
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % common._ED_L
    encoded_r = common._ed_encode(common._ed_scalar_multiply(common._ED_BASE, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(), "little"
    ) % common._ED_L
    encoded_s = ((nonce + challenge * scalar) % common._ED_L).to_bytes(32, "little")
    signature = encoded_r + encoded_s
    assert common.verify_ed25519(public, signature, message)
    return base64.b64encode(signature).decode("ascii")


def _synthetic_validator_identity(executable_hash: str) -> dict[str, object]:
    identity: dict[str, object] = {
        "protocol_version": 1,
        "crate_name": "iroha_sccp",
        "crate_version": common._workspace_crate_version(),
        "enabled_features": [],
        "build_profile": "release",
        "target_triple": "aarch64-apple-darwin",
        "rustc_version": (
            f"rustc {common._locked_rust_version()} "
            "(0123456789abcdef0123456789abcdef01234567 2026-01-01)"
        ),
        "source_sha256_hex": hashlib.sha256(
            common.RUST_VALIDATOR_SOURCE.read_bytes()
        ).hexdigest(),
        "crate_manifest_sha256_hex": hashlib.sha256(
            common.SCCP_CRATE_MANIFEST.read_bytes()
        ).hexdigest(),
        "build_script_sha256_hex": hashlib.sha256(
            common.SCCP_BUILD_SCRIPT.read_bytes()
        ).hexdigest(),
        "workspace_manifest_sha256_hex": hashlib.sha256(
            common.WORKSPACE_MANIFEST.read_bytes()
        ).hexdigest(),
        "cargo_lock_sha256_hex": hashlib.sha256(common.CARGO_LOCK.read_bytes()).hexdigest(),
        "toolchain_lock_sha256_hex": hashlib.sha256(
            common.RUST_TOOLCHAIN_LOCK.read_bytes()
        ).hexdigest(),
        "executable_sha256_hex": executable_hash,
        "build_identity_hex": "00" * 32,
    }
    identity["build_identity_hex"] = common.validator_build_identity_hex(identity)
    return common._validate_validator_identity(identity)


def _artifact_digests(root: Path) -> dict[str, str]:
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted((root / "artifacts").rglob("*"))
        if path.is_file()
    }


@pytest.fixture
def reseal_fixture(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> dict[str, object]:
    fixture = tmp_path / "fixture"
    shutil.copytree(reseal.FIXTURE_ROOT, fixture)
    session = tmp_path / "session"
    validator = tmp_path / "validator"
    validator.write_bytes(b"synthetic-sccp-validator")
    validator.chmod(validator.stat().st_mode | stat.S_IXUSR)
    validator_hash = hashlib.sha256(validator.read_bytes()).hexdigest()
    identity = _synthetic_validator_identity(validator_hash)
    engineering = _keypair(b"sccp-reseal-engineering")
    security = _keypair(b"sccp-reseal-security")

    monkeypatch.setattr(reseal, "FIXTURE_ROOT", fixture)
    monkeypatch.setattr(
        common,
        "derive_validator_identity",
        lambda _path: (copy.deepcopy(identity), validator_hash),
    )
    monkeypatch.setattr(
        reseal,
        "_require_forbidden_key_registration",
        lambda _keys: None,
    )

    def validate_stage(stage: Path, _validator: Path) -> None:
        policy, _ = common.load_trust_policy(
            stage / reseal.POLICY_NAME, allow_test_policy=True
        )
        evidence, _ = common.load_evidence_file(stage / reseal.EVIDENCE_NAME, policy)
        common.verify_evidence_artifacts(evidence, stage)

    monkeypatch.setattr(reseal, "_validate_staged_with_rust", validate_stage)
    return {
        "root": fixture,
        "session": session,
        "validator": validator,
        "identity": identity,
        "validator_hash": validator_hash,
        "engineering": engineering,
        "security": security,
        "public_keys": (engineering[0].hex(), security[0].hex()),
    }


def _prepare(case: dict[str, object]) -> bytes:
    reseal.prepare(
        validator_path=case["validator"],
        session_dir=case["session"],
        public_keys=case["public_keys"],
    )
    return (case["session"] / reseal.SESSION_FILES["payload"]).read_bytes()


def _finalize(case: dict[str, object], payload: bytes) -> dict[str, object]:
    return reseal.finalize(
        validator_path=case["validator"],
        session_dir=case["session"],
        signatures=(
            _sign(case["engineering"], payload),
            _sign(case["security"], payload),
        ),
    )


def test_prepare_and_finalize_publish_one_complete_generation(
    reseal_fixture: dict[str, object],
) -> None:
    root = reseal_fixture["root"]
    old_policy = (root / reseal.POLICY_NAME).read_bytes()
    old_evidence = (root / reseal.EVIDENCE_NAME).read_bytes()
    artifacts = _artifact_digests(root)

    payload = _prepare(reseal_fixture)
    result = _finalize(reseal_fixture, payload)

    assert result["published"] is True
    assert (root / reseal.POLICY_NAME).read_bytes() != old_policy
    assert (root / reseal.EVIDENCE_NAME).read_bytes() != old_evidence
    assert _artifact_digests(root) == artifacts
    policy, _ = common.load_trust_policy(
        root / reseal.POLICY_NAME, allow_test_policy=True
    )
    evidence, _ = common.load_evidence_file(root / reseal.EVIDENCE_NAME, policy)
    assert common.evidence_signing_payload(evidence) == payload
    assert [entry["public_key_hex"] for entry in policy["roles"]] == list(
        reseal_fixture["public_keys"]
    )
    assert not list(root.parent.glob(".release_evidence_v1.reseal-*"))

    with pytest.raises(common.SccpReleaseError, match="changed after reseal preparation"):
        _finalize(reseal_fixture, payload)


def test_wrong_signature_fails_before_publication(
    reseal_fixture: dict[str, object],
) -> None:
    root = reseal_fixture["root"]
    before = (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )
    payload = _prepare(reseal_fixture)
    wrong = _sign(_keypair(b"wrong-release-role"), payload)
    with pytest.raises(common.SccpReleaseError, match="invalid detached Ed25519 signature"):
        reseal.finalize(
            validator_path=reseal_fixture["validator"],
            session_dir=reseal_fixture["session"],
            signatures=(wrong, _sign(reseal_fixture["security"], payload)),
        )
    assert before == (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )


def test_unsigned_and_complete_evidence_admission_stay_disjoint(
    reseal_fixture: dict[str, object],
) -> None:
    payload = _prepare(reseal_fixture)
    manifest, policy_bytes, unsigned_bytes, observed_payload = reseal._load_session(
        reseal_fixture["session"]
    )
    policy, unsigned = reseal._validate_session_candidate(
        manifest,
        policy_bytes,
        unsigned_bytes,
        observed_payload,
        reseal_fixture["validator"],
    )
    complete = copy.deepcopy(unsigned)
    complete["provenance"] = reseal._provenance(
        policy,
        (
            _sign(reseal_fixture["engineering"], payload),
            _sign(reseal_fixture["security"], payload),
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="unknown provenance"):
        common.validate_test_fixture_evidence_signing_candidate(complete, policy)
    with pytest.raises(common.SccpReleaseError, match="missing provenance"):
        common.validate_evidence(unsigned, policy)

    production_policy = copy.deepcopy(policy)
    production_policy["schema"] = common.TRUST_POLICY_SCHEMA
    production_policy["environment"] = "production"
    with pytest.raises(common.SccpReleaseError, match="restricted to the test fixture"):
        common.validate_test_fixture_evidence_signing_candidate(
            unsigned, production_policy
        )


def test_staged_validation_wires_both_rust_verification_boundaries(
    reseal_fixture: dict[str, object],
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    payload = _prepare(reseal_fixture)
    manifest, policy_bytes, unsigned_bytes, observed_payload = reseal._load_session(
        reseal_fixture["session"]
    )
    policy, unsigned = reseal._validate_session_candidate(
        manifest,
        policy_bytes,
        unsigned_bytes,
        observed_payload,
        reseal_fixture["validator"],
    )
    complete = copy.deepcopy(unsigned)
    complete["provenance"] = reseal._provenance(
        policy,
        (
            _sign(reseal_fixture["engineering"], payload),
            _sign(reseal_fixture["security"], payload),
        ),
    )
    evidence = common.validate_evidence(complete, policy)
    evidence_bytes = common.canonical_json_file_bytes(evidence)
    expected_files = reseal._fixture_inventory(unsigned)
    stage = tmp_path / "stage"
    stage.mkdir()
    stage_fd = reseal._open_direct_directory_fd(
        stage, label="test stage"
    )
    try:
        reseal._copy_fixture_tree(
            stage, stage_fd, expected_files, policy_bytes, evidence_bytes
        )
    finally:
        os.close(stage_fd)
    signature_calls: list[dict[str, object]] = []
    lane_calls: list[tuple[tuple[object, ...], dict[str, object]]] = []

    def verify_signatures(**kwargs: object) -> tuple[dict[str, object], str]:
        signature_calls.append(kwargs)
        return {}, reseal_fixture["validator_hash"]

    def verify_lanes(
        *args: object, **kwargs: object
    ) -> tuple[list[dict[str, object]], str]:
        lane_calls.append((args, kwargs))
        return [], reseal_fixture["validator_hash"]

    monkeypatch.setattr(common, "verify_rust_release_signatures", verify_signatures)
    monkeypatch.setattr(common, "verify_rust_lane_evidence", verify_lanes)
    REAL_VALIDATE_STAGED_WITH_RUST(stage, reseal_fixture["validator"])

    assert len(signature_calls) == 1
    signature_call = signature_calls[0]
    assert signature_call == {
        "trust_policy_path": stage / reseal.POLICY_NAME,
        "trust_policy": policy,
        "trust_policy_bytes": policy_bytes,
        "evidence_path": stage / reseal.EVIDENCE_NAME,
        "evidence": evidence,
        "evidence_bytes": evidence_bytes,
        "validator_path": reseal_fixture["validator"],
        "environment": "test-fixture",
    }
    assert lane_calls == [
        (
            (evidence, stage, reseal_fixture["validator"], policy),
            {
                "trust_policy_path": stage / reseal.POLICY_NAME,
                "evidence_path": stage / reseal.EVIDENCE_NAME,
                "environment": "test-fixture",
            },
        )
    ]


@pytest.mark.parametrize("name", ("policy", "evidence", "payload"))
def test_finalize_rejects_tampered_session_files(
    reseal_fixture: dict[str, object], name: str
) -> None:
    payload = _prepare(reseal_fixture)
    path = reseal_fixture["session"] / reseal.SESSION_FILES[name]
    path.write_bytes(path.read_bytes() + b"x")
    with pytest.raises(common.SccpReleaseError, match="do not match their manifest"):
        _finalize(reseal_fixture, payload)


def test_finalize_rejects_changed_live_fixture(
    reseal_fixture: dict[str, object],
) -> None:
    payload = _prepare(reseal_fixture)
    evidence = reseal_fixture["root"] / reseal.EVIDENCE_NAME
    evidence.write_bytes(evidence.read_bytes() + b" ")
    with pytest.raises(common.SccpReleaseError, match="changed after reseal preparation"):
        _finalize(reseal_fixture, payload)


def test_prepare_rejects_malformed_fixture_before_creating_session(
    reseal_fixture: dict[str, object],
) -> None:
    evidence_path = reseal_fixture["root"] / reseal.EVIDENCE_NAME
    evidence = common.parse_json_bytes(
        evidence_path.read_bytes(),
        label="test evidence",
        maximum=common.MAX_EVIDENCE_BYTES,
    )
    del evidence["artifacts"]
    evidence_path.write_bytes(common.canonical_json_file_bytes(evidence))
    with pytest.raises(common.SccpReleaseError, match="missing artifacts"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=reseal_fixture["session"],
            public_keys=reseal_fixture["public_keys"],
        )
    assert not reseal_fixture["session"].exists()


def test_prepare_and_finalize_refuse_links(
    reseal_fixture: dict[str, object], tmp_path: Path
) -> None:
    existing_session = tmp_path / "existing-session"
    existing_session.mkdir()
    sentinel = existing_session / "sentinel"
    sentinel.write_text("preserve", encoding="utf-8")
    with pytest.raises(common.SccpReleaseError, match="must not already exist"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=existing_session,
            public_keys=reseal_fixture["public_keys"],
        )
    assert sentinel.read_text(encoding="utf-8") == "preserve"

    session_link = tmp_path / "session-link"
    session_link.symlink_to(tmp_path / "missing")
    with pytest.raises(common.SccpReleaseError, match="must not already exist"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=session_link,
            public_keys=reseal_fixture["public_keys"],
        )

    root = reseal_fixture["root"]
    policy_copy = tmp_path / "policy-copy"
    policy_copy.write_bytes((root / reseal.POLICY_NAME).read_bytes())
    (root / reseal.POLICY_NAME).unlink()
    os.link(policy_copy, root / reseal.POLICY_NAME)
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        _prepare(reseal_fixture)


def test_prepare_does_not_remove_directory_won_by_creation_race(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    session = reseal_fixture["session"]
    real_mkdir = os.mkdir

    def win_race(path: Path, mode: int, *, dir_fd: int | None = None) -> None:
        real_mkdir(path, mode, dir_fd=dir_fd)
        (session / "other-owner").write_text("preserve", encoding="utf-8")
        raise FileExistsError

    monkeypatch.setattr(reseal.os, "mkdir", win_race)
    with pytest.raises(common.SccpReleaseError, match="created exclusively"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=session,
            public_keys=reseal_fixture["public_keys"],
        )
    assert (session / "other-owner").read_text(encoding="utf-8") == "preserve"


def test_prepare_never_follows_substituted_session_directory(
    reseal_fixture: dict[str, object],
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    session = reseal_fixture["session"]
    outside = tmp_path / "outside"
    outside.mkdir()
    real_mkdir = os.mkdir

    def substitute(path: Path, mode: int, *, dir_fd: int | None = None) -> None:
        real_mkdir(path, mode, dir_fd=dir_fd)
        os.rename(
            path,
            f"{path}.original",
            src_dir_fd=dir_fd,
            dst_dir_fd=dir_fd,
        )
        os.symlink(outside, path, target_is_directory=True, dir_fd=dir_fd)

    monkeypatch.setattr(reseal.os, "mkdir", substitute)
    with pytest.raises(common.SccpReleaseError, match="could not be opened safely"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=session,
            public_keys=reseal_fixture["public_keys"],
        )
    assert list(outside.iterdir()) == []


def test_finalize_refuses_hard_linked_session_payload(
    reseal_fixture: dict[str, object], tmp_path: Path
) -> None:
    _prepare(reseal_fixture)
    payload = reseal_fixture["session"] / reseal.SESSION_FILES["payload"]
    linked = tmp_path / "linked-payload"
    linked.write_bytes(payload.read_bytes())
    payload.unlink()
    os.link(linked, payload)
    with pytest.raises(common.SccpReleaseError, match="hard-linked"):
        _finalize(reseal_fixture, linked.read_bytes())


def test_finalize_rejects_validator_drift_between_phases(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    payload = _prepare(reseal_fixture)
    changed = copy.deepcopy(reseal_fixture["identity"])
    changed["executable_sha256_hex"] = "ab" * 32
    monkeypatch.setattr(
        common,
        "derive_validator_identity",
        lambda _path: (changed, changed["executable_sha256_hex"]),
    )
    with pytest.raises(common.SccpReleaseError, match="identity changed"):
        _finalize(reseal_fixture, payload)


def test_finalize_refuses_concurrent_publication(
    reseal_fixture: dict[str, object],
) -> None:
    root = reseal_fixture["root"]
    before = (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )
    payload = _prepare(reseal_fixture)
    with reseal._fixture_publication_lock():
        with pytest.raises(common.SccpReleaseError, match="already in progress"):
            _finalize(reseal_fixture, payload)
    assert before == (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )


def test_post_publish_failure_exchanges_the_old_generation_back(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    root = reseal_fixture["root"]
    before = (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )
    payload = _prepare(reseal_fixture)

    def fail_post_publish(*_args: object, **_kwargs: object) -> None:
        raise common.SccpReleaseError("injected post-publication failure")

    monkeypatch.setattr(reseal, "_post_publish_validate", fail_post_publish)
    with pytest.raises(common.SccpReleaseError, match="injected post-publication failure"):
        _finalize(reseal_fixture, payload)
    assert before == (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )


def test_finalize_preserves_artifact_edit_before_publication(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    root = reseal_fixture["root"]
    relative = next(iter(_artifact_digests(root)))
    artifact = root / relative
    concurrent_edit = b"concurrent fixture artifact edit"
    payload = _prepare(reseal_fixture)
    validate_stage = reseal._validate_staged_with_rust

    def validate_then_edit(stage: Path, validator: Path) -> None:
        validate_stage(stage, validator)
        artifact.write_bytes(concurrent_edit)

    monkeypatch.setattr(reseal, "_validate_staged_with_rust", validate_then_edit)
    with pytest.raises(common.SccpReleaseError, match="does not match its signed"):
        _finalize(reseal_fixture, payload)
    assert artifact.read_bytes() == concurrent_edit


def test_finalize_rejects_stage_path_substitution_without_deleting_it(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    root = reseal_fixture["root"]
    before = (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )
    payload = _prepare(reseal_fixture)
    exchange = reseal._atomic_exchange_directories
    replacement: Path | None = None

    def substitute_stage(
        left: Path,
        right: Path,
        *,
        expected_left_identity: tuple[int, int],
        expected_right_identity: tuple[int, int],
    ) -> None:
        nonlocal replacement
        right.rename(right.with_name(right.name + ".original"))
        right.mkdir()
        replacement = right
        (right / "other-owner").write_text("preserve", encoding="utf-8")
        exchange(
            left,
            right,
            expected_left_identity=expected_left_identity,
            expected_right_identity=expected_right_identity,
        )

    monkeypatch.setattr(reseal, "_atomic_exchange_directories", substitute_stage)
    with pytest.raises(common.SccpReleaseError, match="substituted before atomic exchange"):
        _finalize(reseal_fixture, payload)
    assert before == (
        (root / reseal.POLICY_NAME).read_bytes(),
        (root / reseal.EVIDENCE_NAME).read_bytes(),
    )
    assert replacement is not None
    assert (replacement / "other-owner").read_text(encoding="utf-8") == "preserve"


def test_staging_never_follows_substituted_directory(
    reseal_fixture: dict[str, object],
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    payload = _prepare(reseal_fixture)
    outside = tmp_path / "outside-stage"
    outside.mkdir()
    write_exclusive = reseal._write_exclusive_at
    substituted = False

    def substitute_before_write(
        directory_fd: int,
        name: str,
        data: bytes,
        *,
        mode: int = 0o600,
    ) -> None:
        nonlocal substituted
        if not substituted:
            stages = list(
                reseal_fixture["root"].parent.glob(".release_evidence_v1.reseal-*")
            )
            assert len(stages) == 1
            stage = stages[0]
            stage.rename(stage.with_name(stage.name + ".original"))
            stage.symlink_to(outside, target_is_directory=True)
            substituted = True
        write_exclusive(directory_fd, name, data, mode=mode)

    monkeypatch.setattr(reseal, "_write_exclusive_at", substitute_before_write)
    with pytest.raises(common.SccpReleaseError, match="direct non-symlink directory"):
        _finalize(reseal_fixture, payload)
    assert substituted
    assert list(outside.iterdir()) == []


def test_finalize_rolls_back_artifact_edit_discovered_after_exchange(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    root = reseal_fixture["root"]
    relative = next(iter(_artifact_digests(root)))
    concurrent_edit = b"concurrent displaced-generation edit"
    payload = _prepare(reseal_fixture)
    exchange = reseal._atomic_exchange_directories
    exchanges = 0

    def exchange_then_edit(
        left: Path,
        right: Path,
        *,
        expected_left_identity: tuple[int, int],
        expected_right_identity: tuple[int, int],
    ) -> None:
        nonlocal exchanges
        exchange(
            left,
            right,
            expected_left_identity=expected_left_identity,
            expected_right_identity=expected_right_identity,
        )
        exchanges += 1
        if exchanges == 1:
            (right / relative).write_bytes(concurrent_edit)

    monkeypatch.setattr(reseal, "_atomic_exchange_directories", exchange_then_edit)
    with pytest.raises(common.SccpReleaseError, match="does not match its signed"):
        _finalize(reseal_fixture, payload)
    assert exchanges == 2
    assert (root / relative).read_bytes() == concurrent_edit


def test_forbidden_key_registries_must_match_exactly(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    policy, _ = common.load_trust_policy(
        reseal.FIXTURE_ROOT / reseal.POLICY_NAME, allow_test_policy=True
    )
    keys = [entry["public_key_hex"] for entry in policy["roles"]]
    reseal._require_forbidden_key_registration(keys)

    rust = tmp_path / "sccp_release_evidence.rs"
    rust.write_text(
        'const FORBIDDEN_FIXTURE_PUBLIC_KEYS: [&str; 1] = [\n    "'
        + keys[0]
        + '",\n];\n',
        encoding="utf-8",
    )
    monkeypatch.setattr(reseal, "RUST_VALIDATOR_SOURCE", rust)
    with pytest.raises(common.SccpReleaseError, match="do not match exactly"):
        reseal._require_forbidden_key_registration(keys)


def test_prepare_refuses_unregistered_public_keys(
    reseal_fixture: dict[str, object], monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(
        reseal,
        "_require_forbidden_key_registration",
        REAL_REQUIRE_FORBIDDEN_KEY_REGISTRATION,
    )
    with pytest.raises(common.SccpReleaseError, match="registered as fixture-only"):
        reseal.prepare(
            validator_path=reseal_fixture["validator"],
            session_dir=reseal_fixture["session"],
            public_keys=reseal_fixture["public_keys"],
        )


def test_reseal_cli_has_no_signing_key_or_signing_implementation() -> None:
    source = (SCRIPTS / "sccp_release_fixture_reseal.py").read_text(encoding="utf-8")
    assert "private_key" not in source
    assert "FIXTURE_KEY_DOMAIN" not in source
    assert "SigningKey" not in source
    assert "Ed25519PrivateKey" not in source
    assert "--sign" not in source
    assert "-signature-b64" in source
    assert "-public-key-hex" in source


def test_validator_identity_derivation_binds_executable_and_current_inputs(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    executable = b"authenticated-validator"
    executable_hash = hashlib.sha256(executable).hexdigest()
    identity = _synthetic_validator_identity(executable_hash)
    monkeypatch.setattr(common, "_read_validator_executable", lambda _path: executable)
    monkeypatch.setattr(
        common,
        "_invoke_validator_command",
        lambda _path, arguments, expected: (
            common.canonical_json_file_bytes(identity),
            b"",
            0,
            expected,
        )
        if arguments == ("identity",)
        else pytest.fail("unexpected validator command"),
    )
    observed, observed_hash = common.derive_validator_identity(Path("validator"))
    assert observed == identity
    assert observed_hash == executable_hash


def test_validator_identity_derivation_rejects_self_hash_substitution(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    executable = b"authenticated-validator"
    executable_hash = hashlib.sha256(executable).hexdigest()
    identity = _synthetic_validator_identity("ab" * 32)
    monkeypatch.setattr(common, "_read_validator_executable", lambda _path: executable)
    monkeypatch.setattr(
        common,
        "_invoke_validator_command",
        lambda _path, _arguments, expected: (
            common.canonical_json_file_bytes(identity),
            b"",
            0,
            expected,
        ),
    )
    with pytest.raises(common.SccpReleaseError, match="does not bind the selected executable"):
        common.derive_validator_identity(Path("validator"))
    assert executable_hash != identity["executable_sha256_hex"]
