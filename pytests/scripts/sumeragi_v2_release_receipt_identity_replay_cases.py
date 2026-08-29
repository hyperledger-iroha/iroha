# Executed lexically in sumeragi_v2_release_receipt_test.py; do not import directly.

@pytest.mark.parametrize(
    ("path", "replacement"),
    [
        (("schema_version",), 2),
        (("format",), "other-attestation"),
        (("candidate", "commit_oid"), "9" * 40),
        (("candidate", "tree_oid"), "8" * 40),
        (("candidate", "source_manifest_sha256"), "0" * 64),
        (("candidate", "cargo_lock_sha256"), "1" * 64),
        (("candidate", "release_identity_sha256"), "2" * 64),
        (("archives", "raw_commit", "archive_id"), "other-commit"),
        (("archives", "raw_commit", "mode"), "0401"),
        (("archives", "raw_commit", "sha256"), "3" * 64),
        (("archives", "raw_commit", "size_bytes"), 0),
        (("archives", "git", "archive_id"), "other-git"),
        (("archives", "git", "mode"), "0501"),
        (("archives", "git", "sha256"), "4" * 64),
        (("archives", "git", "size_bytes"), False),
        (("archives", "ssh_revocation", "archive_id"), "other-revocation"),
        (("archives", "ssh_revocation", "mode"), "0401"),
        (("archives", "ssh_revocation", "sha256"), "5" * 64),
        (("archives", "ssh_revocation", "size_bytes"), False),
    ],
)
def test_receipt_rejects_tampered_signature_attestation_fields(
    tmp_path: Path, path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    mutate_attestation(
        evidence, lambda value: set_nested(value, path, replacement)
    )

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "release" in result.stderr.lower()
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize("artifact_name", ["signature_attestation", "signature_transcript"])
def test_receipt_rejects_noncanonical_signature_json(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    value = json.loads(artifact.read_text(encoding="utf-8"))
    artifact.chmod(0o600)
    artifact.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    artifact.chmod(0o400)
    if artifact_name == "signature_transcript":
        attestation = evidence["signature_attestation"]
        assert isinstance(attestation, Path)
        attestation_value = json.loads(attestation.read_text(encoding="utf-8"))
        attestation_value["archives"]["verify_transcript"] = (
            sanitized_identity_artifact(artifact, 0o400, "verify_transcript")
        )
        rewrite_json(attestation, attestation_value)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "canonical UTF-8 JSON" in result.stderr


def test_receipt_rejects_noncanonical_candidate_identity_bytes(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    candidate = evidence["candidate"]
    assert isinstance(candidate, Path)
    value = json.loads(candidate.read_text(encoding="utf-8"))
    candidate.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "candidate identity is not canonical UTF-8 JSON" in result.stderr


@pytest.mark.parametrize(
    ("mutation", "error_fragment"),
    [
        ("non-ssh", "non-SSH signature format"),
        ("malformed-armor", "malformed SSH signature armor"),
        ("wrong-tree", "tree does not match"),
        ("duplicate-trailer", "exact terminal Sumeragi v2 release trailer block"),
    ],
)
def test_raw_commit_validator_rejects_adversarial_signed_object_shapes(
    tmp_path: Path, mutation: str, error_fragment: str
) -> None:
    evidence = make_evidence(tmp_path)
    raw_path = evidence["signature_raw_commit"]
    candidate_path = evidence["candidate"]
    assert isinstance(raw_path, Path)
    assert isinstance(candidate_path, Path)
    raw = raw_path.read_bytes()
    identity = json.loads(candidate_path.read_text(encoding="utf-8"))
    if mutation == "non-ssh":
        raw = raw.replace(b"SSH SIGNATURE", b"PGP SIGNATURE")
    elif mutation == "malformed-armor":
        raw = re.sub(rb"(?m)^ [A-Za-z0-9+/=]+$", b" ***", raw, count=1)
    elif mutation == "wrong-tree":
        raw = raw.replace(b"tree " + b"2" * 40, b"tree " + b"3" * 40)
    elif mutation == "duplicate-trailer":
        raw = raw.replace(
            b"Sumeragi v2 release fixture\n\n",
            b"Sumeragi v2 release fixture\n"
            b"Sumeragi-V2-Release-Identity-Version: 1\n\n",
        )
    else:
        raise AssertionError(mutation)
    framed = b"commit " + str(len(raw)).encode("ascii") + b"\0" + raw
    identity["head_commit"] = hashlib.sha1(
        framed, usedforsecurity=False
    ).hexdigest()
    symbols = runpy.run_path(str(SCRIPT))

    with pytest.raises(symbols["ReceiptError"], match=error_fragment):
        symbols["_validate_raw_commit"](raw, identity)


def test_allowed_signers_policy_accepts_one_unbounded_active_line() -> None:
    symbols = runpy.run_path(str(SCRIPT))

    symbols["_validate_allowed_signers_policy"](
        b"# release trust root\n\n"
        b"release@example.test ssh-ed25519 AAAAC3NzaFixtureKey\n"
    )


@pytest.mark.parametrize(
    ("policy", "error_fragment"),
    [
        (
            b"first@example.test ssh-ed25519 AAAAC3NzaFirst\n"
            b"second@example.test ssh-ed25519 AAAAC3NzaSecond\n",
            "exactly one active line",
        ),
        (
            b'release@example.test valid-after="20260101Z" '
            b"ssh-ed25519 AAAAC3NzaFixtureKey\n",
            "time-bounded",
        ),
        (
            b'release@example.test valid-before="20270101Z" '
            b"ssh-ed25519 AAAAC3NzaFixtureKey\n",
            "time-bounded",
        ),
    ],
)
def test_allowed_signers_policy_rejects_multiple_or_time_bounded_lines(
    policy: bytes, error_fragment: str
) -> None:
    symbols = runpy.run_path(str(SCRIPT))

    with pytest.raises(symbols["ReceiptError"], match=error_fragment):
        symbols["_validate_allowed_signers_policy"](policy)


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("expected_git_sha256", "0" * 64),
        ("expected_ssh_keygen_sha256", "1" * 64),
        ("expected_allowed_signers_sha256", "2" * 64),
        ("expected_revocation_sha256", "3" * 64),
        ("expected_signer_fingerprint", "SHA256:" + "B" * 43),
    ],
)
def test_receipt_rejects_wrong_out_of_band_signature_policy(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence[field] = replacement

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize(
    "artifact_name",
    [
        "signature_attestation",
        "signature_transcript",
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ],
)
def test_receipt_rejects_signature_archive_mode_drift(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    artifact.chmod(0o600)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact mode" in result.stderr


@pytest.mark.parametrize(
    "artifact_name",
    [
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ],
)
def test_receipt_rejects_signature_archive_content_drift(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    tool = artifact_name in {"signature_git", "signature_ssh_keygen"}
    artifact.chmod(0o700 if tool else 0o600)
    artifact.write_bytes(artifact.read_bytes() + b"tamper\n")
    artifact.chmod(0o500 if tool else 0o400)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


def test_receipt_rejects_nonprivate_signature_archive_directory(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["signature_dir"]
    assert isinstance(directory, Path)
    directory.chmod(0o755)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact mode 0700" in result.stderr


def test_receipt_rejects_signature_archive_wrong_name(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    attestation = evidence["signature_attestation"]
    assert isinstance(attestation, Path)
    renamed = attestation.with_name("attestation.json")
    attestation.rename(renamed)
    evidence["signature_attestation"] = renamed

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "wrong exact name" in result.stderr


def test_receipt_rejects_signature_archives_split_across_directories(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    revocation = evidence["signature_revocation"]
    assert isinstance(revocation, Path)
    other = tmp_path / "other-private"
    other.mkdir(mode=0o700)
    moved = other / revocation.name
    revocation.rename(moved)
    evidence["signature_revocation"] = moved

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "do not share one directory" in result.stderr


def test_receipt_rejects_signature_archive_symlink(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    raw_commit = evidence["signature_raw_commit"]
    assert isinstance(raw_commit, Path)
    real = raw_commit.with_name("raw-commit-real")
    raw_commit.rename(real)
    raw_commit.symlink_to(real.name)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "resolved and non-symlinked" in result.stderr


def test_receipt_rejects_hardlinked_signature_archives(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    allowed = evidence["signature_allowed_signers"]
    revocation = evidence["signature_revocation"]
    assert isinstance(allowed, Path)
    assert isinstance(revocation, Path)
    revocation.unlink()
    os.link(allowed, revocation)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "singly linked" in result.stderr


def test_receipt_rejects_signature_directory_inside_release_root(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    root = evidence["release_root"]
    assert isinstance(root, Path)
    nested = root / "release-identity"
    nested.mkdir(mode=0o700)
    nested.chmod(0o700)
    evidence["signature_dir"] = nested
    for key in (
        "signature_attestation",
        "signature_transcript",
        "signature_raw_commit",
        "signature_cargo_lock",
        "signature_allowed_signers",
        "signature_revocation",
        "signature_git",
        "signature_ssh_keygen",
    ):
        old_path = evidence[key]
        assert isinstance(old_path, Path)
        moved = nested / old_path.name
        old_path.rename(moved)
        evidence[key] = moved

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "sealed release root must be external to the bootstrap archive" in result.stderr


@pytest.mark.parametrize(
    "mutation",
    [
        "schema-version",
        "format",
        "archive-id",
        "candidate-oid",
        "extra-top-level",
        "extra-operation",
        "verify-operation-id",
        "verify-failed",
        "verify-stdout-digest",
        "verify-stderr-size",
        "show-operation-id",
        "show-size",
        "show-digest",
        "show-bad-status",
        "probe-operation-id",
        "probe-status",
        "probe-digest",
    ],
)
def test_receipt_rejects_tampered_signature_transcript(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)

    def apply(value: dict[str, object]) -> None:
        if mutation == "schema-version":
            value["schema_version"] = 2
        elif mutation == "format":
            value["format"] = "other-transcript"
        elif mutation == "archive-id":
            value["archive_ids"]["raw_commit"] = "other-commit"
        elif mutation == "candidate-oid":
            value["candidate_commit_oid"] = "9" * 40
        elif mutation == "extra-top-level":
            value["source_path"] = str(evidence["signature_dir"])
        elif mutation == "extra-operation":
            value["operations"]["verify_commit"]["argv"] = ["git"]
        elif mutation == "verify-operation-id":
            value["operations"]["verify_commit"]["operation_id"] = "other"
        elif mutation == "verify-failed":
            value["operations"]["verify_commit"]["exit_status"] = 1
        elif mutation == "verify-stdout-digest":
            value["operations"]["verify_commit"]["stdout_sha256"] = "6" * 64
        elif mutation == "verify-stderr-size":
            value["operations"]["verify_commit"]["stderr_size_bytes"] += 1
        elif mutation == "show-operation-id":
            value["operations"]["show_signature_metadata"]["operation_id"] = "other"
        elif mutation == "show-size":
            value["operations"]["show_signature_metadata"]["stdout_size_bytes"] += 1
        elif mutation == "show-digest":
            value["operations"]["show_signature_metadata"]["stdout_sha256"] = "7" * 64
        elif mutation == "show-bad-status":
            bad = (
                f"B\0{evidence['expected_signer_fingerprint']}\0\0"
                f"{evidence['signer_principal']}\0\n"
            ).encode()
            value["operations"]["show_signature_metadata"].update(
                sanitized_operation("git.show-signature-metadata.ssh.v1", 0, bad, b"")
            )
        elif mutation == "probe-operation-id":
            value["operations"]["ssh_keygen_usage"]["operation_id"] = "other"
        elif mutation == "probe-status":
            value["operations"]["ssh_keygen_usage"]["exit_status"] = 0
        elif mutation == "probe-digest":
            value["operations"]["ssh_keygen_usage"]["stderr_sha256"] = "8" * 64
        else:
            raise AssertionError(mutation)

    mutate_transcript(evidence, apply)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert not (tmp_path / "receipt.json").exists()


@pytest.mark.parametrize(
    "failure",
    ["verify-failure", "metadata-change", "raw-commit-change", "top-level-change"],
)
def test_receipt_replays_archived_git_and_rejects_runtime_divergence(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure: str,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    git_path = evidence["signature_git"]
    assert isinstance(git_path, Path)
    source = git_path.read_text(encoding="utf-8")
    if failure == "verify-failure":
        source = source.replace(
            "'Good fixture SSH signature' >&2 ;;",
            "'Good fixture SSH signature' >&2; exit 73 ;;",
        )
    elif failure == "metadata-change":
        fingerprint = str(evidence["expected_signer_fingerprint"])
        replacement = "SHA256:" + "B" * 43
        if fingerprint == replacement:
            replacement = "SHA256:" + "C" * 43
        assert fingerprint in source
        source = source.replace(fingerprint, replacement)
    elif failure == "raw-commit-change":
        source = source.replace(
            "Sumeragi v2 release fixture", "Sumeragi v2 changed release fixture"
        )
    elif failure == "top-level-change":
        other_root = tmp_path / "other-root"
        other_root.mkdir()
        source = source.replace(
            "'rev-parse --show-toplevel') pwd -P ;;",
            f"'rev-parse --show-toplevel') printf '%s\\n' '{other_root}' ;;",
        )
    else:
        raise AssertionError(failure)
    rebind_git_archive(evidence, source)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "archived Git" in result.stderr or "signature replay" in result.stderr

    if failure == "verify-failure":
        module = load_writer_module()
        environment = module._closed_replay_environment(tmp_path)
        real_popen = module.subprocess.Popen
        for swap_kind in ("file", "ancestor"):
            swap_root = tmp_path / f"execution-swap-{swap_kind}"
            trusted_directory = swap_root / "trusted"
            trusted_directory.mkdir(parents=True)
            trusted = trusted_directory / "tool"
            trusted.write_text(
                "#!/bin/sh\nprintf 'trusted\\n'\n", encoding="utf-8"
            )
            trusted.chmod(0o500)
            contract = module._bounded_path_contract(
                trusted,
                "fixture trusted executable",
                maximum_bytes=4096,
                allowed_owners={os.geteuid()},
                executable=True,
            )
            if swap_kind == "file":
                malicious = trusted_directory / "malicious"
                malicious.write_text(
                    "#!/bin/sh\nprintf 'malicious\\n'\n", encoding="utf-8"
                )
                malicious.chmod(0o500)
                saved = trusted_directory / "trusted.saved"

                def swapping_popen(
                    *args: object, **kwargs: object
                ) -> subprocess.Popen[bytes]:
                    trusted.rename(saved)
                    malicious.rename(trusted)
                    process = real_popen(*args, **kwargs)
                    process.wait(timeout=5)
                    trusted.rename(malicious)
                    saved.rename(trusted)
                    return process

            else:
                malicious_directory = swap_root / "malicious"
                malicious_directory.mkdir()
                malicious = malicious_directory / "tool"
                malicious.write_text(
                    "#!/bin/sh\nprintf 'malicious\\n'\n", encoding="utf-8"
                )
                malicious.chmod(0o500)
                saved = swap_root / "trusted.saved"

                def swapping_popen(
                    *args: object, **kwargs: object
                ) -> subprocess.Popen[bytes]:
                    trusted_directory.rename(saved)
                    malicious_directory.rename(trusted_directory)
                    process = real_popen(*args, **kwargs)
                    process.wait(timeout=5)
                    trusted_directory.rename(malicious_directory)
                    saved.rename(trusted_directory)
                    return process

            monkeypatch.setattr(module.subprocess, "Popen", swapping_popen)
            with pytest.raises(
                module.ReceiptError,
                match="changed (?:while pinned|during process execution)",
            ):
                module._run_bounded_replay(
                    trusted,
                    [],
                    cwd=tmp_path,
                    environment=environment,
                    name="fixture swapped executable",
                    executable_contract=contract,
                )
            monkeypatch.setattr(module.subprocess, "Popen", real_popen)
def test_receipt_rejects_fully_rebound_cross_policy_allowed_signers(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    allowed = evidence["signature_allowed_signers"]
    transcript_path = evidence["signature_transcript"]
    attestation_path = evidence["signature_attestation"]
    assert isinstance(allowed, Path)
    assert isinstance(transcript_path, Path)
    assert isinstance(attestation_path, Path)
    allowed.chmod(0o600)
    allowed.write_text(
        "attacker@example.test ssh-ed25519 AAAAC3NzaAttacker\n", encoding="utf-8"
    )
    allowed.chmod(0o400)
    forged_digest = sha256(allowed)
    attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    attestation["archives"]["ssh_allowed_signers"] = (
        sanitized_identity_artifact(allowed, 0o400, "ssh_allowed_signers")
    )
    rewrite_json(attestation_path, attestation)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "out-of-band digest" in result.stderr


def test_receipt_accepts_transcript_published_by_chaos_launcher(
    tmp_path: Path,
) -> None:
    release_root = tmp_path / "release"
    release_root.mkdir()
    evidence = make_evidence(release_root)
    chaos_symbols = runpy.run_path(
        str(ROOT_DIR / "pytests" / "scripts" / "sumeragi_v2_chaos_release_test.py")
    )
    launcher_root = tmp_path / "launcher"
    launcher_root.mkdir()
    launcher, env, chaos_evidence = chaos_symbols["_fixture"](
        launcher_root,
        manifest=evidence["sealed_manifest"],
        head=evidence["head"],
        tree=evidence["tree"],
        lock=evidence["lock"],
    )

    launch_result = chaos_symbols["_run"](launcher, env)

    assert launch_result.returncode == 0, launch_result.stderr
    invocations = list(chaos_evidence.glob("invocation.*"))
    assert len(invocations) == 1
    evidence["chaos_completion"] = invocations[0] / "COMPLETED.tsv"
    evidence["chaos_log"] = invocations[0] / "chaos-100k.log"
    writer = fixture_writer(tmp_path / "writer")
    output = terminal_output_path(evidence)

    receipt_result = run_writer(evidence, output, writer)

    assert receipt_result.returncode == 0, receipt_result.stderr
    assert json.loads(output.read_text(encoding="utf-8"))["result"] == "release-complete"


@pytest.mark.parametrize(
    "completion_name",
    [
        "corridor_completion",
        "formal_completion",
        "seed_completion",
    ],
)
def test_receipt_rejects_cross_source_completion(
    tmp_path: Path, completion_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence[completion_name]
    assert isinstance(completion, Path)
    completion.write_text(
        completion.read_text(encoding="utf-8").replace("b" * 64, "c" * 64),
        encoding="utf-8",
    )
    output = tmp_path / "RELEASE_COMPLETED.json"
    output.write_text("previous valid receipt\n", encoding="utf-8")

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert (
        "not bound" in result.stderr
        or "exact release matrix" in result.stderr
        or "exact release preflight" in result.stderr
    )
    assert output.read_text(encoding="utf-8") == "previous valid receipt\n"


@pytest.mark.parametrize(
    ("artifact_name", "error_fragment"),
    [
        ("formal_log", "formal gate log digest mismatch"),
        ("formal_ledger", "formal proof ledger digest mismatch"),
        ("formal_evidence", "formal proof evidence digest mismatch"),
        ("formal_verus_evidence", "formal Verus evidence digest mismatch"),
        ("formal_verus_log", "formal Verus log digest mismatch"),
        (
            "formal_multilane_apalache_evidence",
            "formal multilane Apalache evidence digest mismatch",
        ),
        (
            "formal_production_trace_extraction_evidence",
            "formal production trace-extraction evidence digest mismatch",
        ),
        ("formal_toolchain", "formal toolchain digest mismatch"),
        ("formal_tlaps_resource_jsonl", "TLAPS resource samples digest mismatch"),
        ("formal_tlaps_resource_summary", "TLAPS resource summary digest mismatch"),
        ("formal_verus_tool", "runtime tool probe verus differs from bootstrap closure"),
        ("corridor_summary", "corridor summary digest mismatch"),
        ("corridor_required", "corridor production inventory digest mismatch"),
        ("corridor_g_unit", "corridor G-UNIT inventory digest mismatch"),
        ("corridor_log", "corridor log 0 digest mismatch"),
        (
            "corridor_cargo_tool",
            "runtime tool probe cargo differs from bootstrap closure",
        ),
        ("seed_summary", "summary digest mismatch"),
        ("seed_log", "seed run log 17 digest mismatch"),
        (
            "seed_localnet_manifest_index",
            "seed localnet manifest index digest mismatch",
        ),
        ("seed_localnet_manifest", "seed localnet manifest 17 digest mismatch"),
        (
            "seed_localnet_file",
            "seed localnet manifest 17 does not match retained content",
        ),
        ("chaos_log", "log digest mismatch"),
    ],
)
def test_receipt_rejects_artifact_changed_after_completion(
    tmp_path: Path, artifact_name: str, error_fragment: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    artifact = evidence[artifact_name]
    assert isinstance(artifact, Path)
    original_mode = stat.S_IMODE(artifact.stat().st_mode)
    artifact.chmod(original_mode | stat.S_IWUSR)
    artifact.write_text("tampered after completion\n", encoding="utf-8")
    artifact.chmod(original_mode)
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert error_fragment in result.stderr
    assert not output.exists()


def test_receipt_rejects_candidate_and_sealed_git_identity_mismatch(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    candidate_path = evidence["candidate"]
    assert isinstance(candidate_path, Path)
    candidate = json.loads(candidate_path.read_text(encoding="utf-8"))
    candidate["head_commit"] = "9" * 40
    candidate_path.write_bytes(canonical_json(candidate))
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, writer)

    assert result.returncode == 1
    assert "disagree on head_commit" in result.stderr
    assert not output.exists()


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("head_commit", "9" * 40),
        ("head_tree", "8" * 40),
        ("cargo_lock_sha256", "7" * 64),
    ],
)
def test_receipt_rejects_seed_exact_identity_mismatch(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release matrix" in result.stderr


@pytest.mark.parametrize("field", ["completed_runs", "expected_runs"])
def test_receipt_rejects_stale_four_scenario_seed_count(
    tmp_path: Path, field: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = "128"
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "exact release matrix" in result.stderr


def test_receipt_rejects_legacy_seed_completion_without_localnet_manifests(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["seed_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields = {
        name: value
        for name, value in fields.items()
        if not name.startswith("localnet_manifest")
    }
    fields["schema_version"] = "1"
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed completion fields do not match its completion schema" in result.stderr


def test_receipt_rejects_seed_localnet_manifest_path_escape(
    tmp_path: Path,
) -> None:
    for mutation, expected_error in (
        ("completion", "seed localnet manifest index row 17 is not canonical"),
        ("symlink-parent", "seed localnet manifest 0 escaped its archive"),
    ):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        completion = evidence["seed_completion"]
        assert isinstance(completion, Path)
        if mutation == "completion":
            fields = dict(
                line.split("\t", 1)
                for line in completion.read_text(encoding="utf-8").splitlines()
            )
            fields["localnet_manifest_017_path"] = "../escaped-localnet.tsv"
            write_tsv(completion, fields)
        else:
            manifest = evidence["seed_localnet_manifest"]
            assert isinstance(manifest, Path)
            manifest_directory = manifest.parent
            escaped_directory = case_root / "escaped-localnet-manifests"
            manifest_directory.rename(escaped_directory)
            manifest_directory.symlink_to(escaped_directory, target_is_directory=True)

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert expected_error in result.stderr


def test_receipt_rejects_symlink_in_retained_seed_localnet(tmp_path: Path) -> None:
    for mutation, expected_error in (
        ("entry", "contains a symlink"),
        ("parent", "root must be a resolved real directory"),
    ):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        localnet = evidence["seed_localnet"]
        assert isinstance(localnet, Path)
        if mutation == "entry":
            outside = case_root / "outside-localnet"
            outside.write_text("outside\n", encoding="utf-8")
            (localnet / "escape").symlink_to(outside)
            expected_index = 17
        else:
            localnets_directory = localnet.parent
            escaped_directory = case_root / "escaped-localnets"
            localnets_directory.rename(escaped_directory)
            localnets_directory.symlink_to(escaped_directory, target_is_directory=True)
            expected_index = 0

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert (
            f"seed retained localnet {expected_index} is unsafe or unstable"
            in result.stderr
        )
        assert expected_error in result.stderr


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("cargo_version", "cargo 9.99.9 (forged 2099-01-01)"),
        ("rustc_version", "rustc 9.99.9 (forged 2099-01-01)"),
    ],
)
def test_receipt_rejects_noncanonical_rust_tool_version(
    tmp_path: Path, field: str, replacement: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    completion = evidence["corridor_completion"]
    assert isinstance(completion, Path)
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields[field] = replacement
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "rust-toolchain.toml" in result.stderr

    tool = field.removesuffix("_version")
    fixture_key = f"corridor_{tool}_tool"
    for case_name, mutate_bytes in (
        ("same-bytes-different-path", False),
        ("exact-output-different-bytes", True),
    ):
        case_root = tmp_path / case_name
        case_root.mkdir()
        cross_bound = make_evidence(case_root)
        cross_writer = fixture_writer(case_root)
        source = cross_bound[fixture_key]
        cross_completion = cross_bound["corridor_completion"]
        assert isinstance(source, Path)
        assert isinstance(cross_completion, Path)
        alternate = case_root / f"alternate-{tool}"
        alternate.write_bytes(source.read_bytes())
        if mutate_bytes:
            alternate.write_bytes(
                alternate.read_bytes()
                + b"# distinct executable with the same accepted output\n"
            )
        alternate.chmod(0o500)
        cross_fields = read_tsv_fields(cross_completion)
        cross_fields[f"{tool}_path"] = str(alternate.resolve())
        cross_fields[f"{tool}_sha256"] = sha256(alternate)
        write_tsv(cross_completion, cross_fields)

        cross_result = run_writer(
            cross_bound,
            case_root / "receipt.json",
            cross_writer,
        )

        assert cross_result.returncode == 1
        assert (
            f"corridor {tool} is not the authenticated private runtime tool"
            in cross_result.stderr
        )


def test_receipt_rejects_rehashed_missing_corridor_leg(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    summary.write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "must contain every exact release leg" in result.stderr


def test_receipt_rejects_rehashed_noncanonical_g_unit_inventory(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    inventory = evidence["corridor_g_unit"]
    completion = evidence["corridor_completion"]
    assert isinstance(inventory, Path)
    assert isinstance(completion, Path)
    lines = inventory.read_text(encoding="utf-8").splitlines()
    row = lines[1].split("\t")
    row[2] = "native_amx::tests::forged_g_unit_identity"
    lines[1] = "\t".join(row)
    inventory.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["g_unit_inventory_sha256"] = sha256(inventory)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "corridor G-UNIT inventory row 0 is not canonical" in result.stderr


def test_receipt_rejects_rehashed_g_unit_log_missing_named_test(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    logs = evidence["corridor_logs"]
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(logs, list)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    log = logs[0]
    first_result = next(
        line
        for line in log.read_text(encoding="utf-8").splitlines()
        if line.startswith("test ") and line.endswith(" ... ok")
    )
    log.write_text(
        log.read_text(encoding="utf-8").replace(first_result + "\n", "", 1),
        encoding="utf-8",
    )
    summary_lines = summary.read_text(encoding="utf-8").splitlines()
    row = summary_lines[1].split("\t")
    row[7] = sha256(log)
    summary_lines[1] = "\t".join(row)
    summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "G-UNIT leg g-unit-iroha-core lacks one required passing test" in result.stderr


def test_receipt_rejects_missing_or_altered_source_sealed_full_suite_leg(
    tmp_path: Path,
) -> None:
    for mutation in ("missing", "altered-command"):
        case_root = tmp_path / mutation
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        summary = evidence["corridor_summary"]
        completion = evidence["corridor_completion"]
        assert isinstance(summary, Path)
        assert isinstance(completion, Path)
        lines = summary.read_text(encoding="utf-8").splitlines()
        row_index = next(
            index
            for index, line in enumerate(lines[1:], 1)
            if "\tsource-sealed-workspace-tests\t" in line
        )
        if mutation == "missing":
            del lines[row_index]
        else:
            row = lines[row_index].split("\t")
            row[9] = "cargo test --workspace"
            lines[row_index] = "\t".join(row)
        summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
        fields = dict(
            line.split("\t", 1)
            for line in completion.read_text(encoding="utf-8").splitlines()
        )
        fields["summary_sha256"] = sha256(summary)
        write_tsv(completion, fields)

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert (
            "must contain every exact release leg" in result.stderr
            or "is not the exact release leg" in result.stderr
        )


def test_receipt_rejects_rehashed_malformed_corridor_log(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    log = evidence["corridor_log"]
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    log.write_text("fabricated pass without Cargo semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[1].split("\t")
    row[7] = sha256(log)
    lines[1] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "ambiguous Cargo transcript" in result.stderr


def test_receipt_rejects_sumeragi_diagnostics_rust_log_missing_named_test(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["corridor_summary"]
    completion = evidence["corridor_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    summary_lines = summary.read_text(encoding="utf-8").splitlines()
    row_index = next(
        index
        for index, line in enumerate(summary_lines[1:], 1)
        if "\tsumeragi-diagnostics-rust\t" in line
    )
    row = summary_lines[row_index].split("\t")
    log = summary.parent / row[8]
    log_lines = log.read_text(encoding="utf-8").splitlines()
    named_test_index = next(
        index
        for index, line in enumerate(log_lines)
        if line.startswith("test client::tests::get_sumeragi_")
    )
    del log_lines[named_test_index]
    log.write_text("\n".join(log_lines) + "\n", encoding="utf-8")
    row[7] = sha256(log)
    summary_lines[row_index] = "\t".join(row)
    summary.write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
    completion_fields = read_tsv_fields(completion)
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert (
        "corridor exact Cargo leg sumeragi-diagnostics-rust lacks its named test"
        in result.stderr
    )


def test_receipt_rejects_sumeragi_diagnostics_suite_source_drift(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    release_root = evidence["release_root"]
    assert isinstance(release_root, Path)
    source = release_root / "ci/run_sumeragi_v2_sdk_diagnostics.sh"
    source.write_bytes(source.read_bytes() + b"\n# forged post-harness source drift\n")

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert (
        "corridor Sumeragi v2 SDK diagnostics python leg is not bound to the "
        "exact suite sources" in result.stderr
    )
    _assert_receipt_requires_exact_path_free_openapi_two_mirror_binding(tmp_path)


def _assert_receipt_requires_exact_path_free_openapi_two_mirror_binding(
    tmp_path: Path,
) -> None:
    mutations = (
        ("missing", lambda marker: None),
        ("failed-status", lambda marker: marker.replace("status=success", "status=failed")),
        (
            "candidate-oid",
            lambda marker: re.sub(
                r"candidate_oid=[0-9a-f]{40,64}",
                f"candidate_oid={'9' * 40}",
                marker,
            ),
        ),
        (
            "candidate-tree",
            lambda marker: re.sub(
                r"candidate_tree=[0-9a-f]{40,64}",
                f"candidate_tree={'8' * 40}",
                marker,
            ),
        ),
        ("mirror-count", lambda marker: marker.replace("mirrors=2", "mirrors=3")),
        (
            "artifact-count",
            lambda marker: marker.replace("artifacts=5", "artifacts=6"),
        ),
        (
            "unsigned-policy",
            lambda marker: marker.replace("require_signed=1", "require_signed=0"),
        ),
        ("path-bearing", lambda marker: f"{marker} path=/private/forged"),
        ("duplicate", lambda marker: marker),
        ("ordering", lambda marker: marker),
    )
    for case_name, mutate in mutations:
        case_root = tmp_path / case_name
        case_root.mkdir()
        evidence = make_evidence(case_root)
        writer = fixture_writer(case_root)
        summary = evidence["corridor_summary"]
        completion = evidence["corridor_completion"]
        assert isinstance(summary, Path)
        assert isinstance(completion, Path)
        summary_lines = summary.read_text(encoding="utf-8").splitlines()
        row_index = next(
            index
            for index, line in enumerate(summary_lines[1:], 1)
            if "\tnative-amx-grouped-openapi\t" in line
        )
        row = summary_lines[row_index].split("\t")
        log = summary.parent / row[8]
        log_lines = log.read_text(encoding="utf-8").splitlines()
        marker_index = next(
            index
            for index, line in enumerate(log_lines)
            if line.startswith("openapi-two-mirror-replay ")
        )
        marker = log_lines[marker_index]
        if case_name == "missing":
            del log_lines[marker_index]
        elif case_name == "duplicate":
            log_lines.insert(marker_index, marker)
        elif case_name == "ordering":
            del log_lines[marker_index]
            log_lines.append(marker)
        else:
            replacement = mutate(marker)
            assert isinstance(replacement, str)
            log_lines[marker_index] = replacement
        log.write_text("\n".join(log_lines) + "\n", encoding="utf-8")
        row[7] = sha256(log)
        summary_lines[row_index] = "\t".join(row)
        summary.write_text(
            "\n".join(summary_lines) + "\n", encoding="utf-8"
        )
        completion_fields = read_tsv_fields(completion)
        completion_fields["summary_sha256"] = sha256(summary)
        write_tsv(completion, completion_fields)

        result = run_writer(evidence, case_root / "receipt.json", writer)

        assert result.returncode == 1
        assert (
            "corridor grouped Native AMX V2 openapi leg lacks the exact "
            "path-free two-mirror replay binding" in result.stderr
        )


def test_hand_invoked_writer_rejects_fake_machine_completion_artifacts(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    output = tmp_path / "RELEASE_COMPLETED.json"

    result = run_writer(evidence, output, SCRIPT)

    assert result.returncode == 1
    assert (
        "archived formal ledger has an invalid cross-tool evidence requirement"
        in result.stderr
    )
    assert not output.exists()


def test_receipt_rejects_rehashed_seed_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text("forged success without libtest semantics\n", encoding="utf-8")
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr


def test_receipt_requires_exact_nocapture_seed_diagnostic(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    run_log = evidence["seed_log"]
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(run_log, Path)
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    run_log.write_text(
        run_log.read_text(encoding="utf-8").replace(
            "deterministic network seed = ", "deterministic network seed = wrong-"
        ),
        encoding="utf-8",
    )
    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    row[7] = sha256(run_log)
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing scenario" in result.stderr

    command_root = tmp_path / "hidden-start-retry"
    command_root.mkdir()
    command_evidence = make_evidence(command_root)
    command_writer = fixture_writer(command_root)
    command_summary = command_evidence["seed_summary"]
    command_completion = command_evidence["seed_completion"]
    assert isinstance(command_summary, Path)
    assert isinstance(command_completion, Path)
    command_lines = command_summary.read_text(encoding="utf-8").splitlines()
    command_row = command_lines[18].split("\t")
    command_row[10] = command_row[10].replace(
        "IROHA_TEST_NETWORK_START_ATTEMPTS=1",
        "IROHA_TEST_NETWORK_START_ATTEMPTS=2",
    )
    command_lines[18] = "\t".join(command_row)
    command_summary.write_text(
        "\n".join(command_lines) + "\n", encoding="utf-8"
    )
    command_fields = dict(
        line.split("\t", 1)
        for line in command_completion.read_text(encoding="utf-8").splitlines()
    )
    command_fields["summary_sha256"] = sha256(command_summary)
    write_tsv(command_completion, command_fields)

    command_result = run_writer(
        command_evidence, command_root / "receipt.json", command_writer
    )

    assert command_result.returncode == 1
    assert (
        "seed summary row 17 is not the exact release run" in command_result.stderr
    )


@pytest.mark.parametrize(
    ("pattern", "replacement"),
    (
        (
            r"CARGO_TARGET_DIR=[^ ]+",
            "CARGO_TARGET_DIR=/tmp/escaped-cargo-target",
        ),
        (r"IROHA_TEST_SKIP_BUILD=1", "IROHA_TEST_SKIP_BUILD=0"),
        (r"cargo test --locked --offline", "cargo test --locked"),
        (
            r"IROHA_TEST_TARGET_DIR=[^ ]+",
            "IROHA_TEST_TARGET_DIR=/tmp/escaped-program-target",
        ),
        (
            r"TEST_NETWORK_BIN_IROHAD=[^ ]+",
            "TEST_NETWORK_BIN_IROHAD=/tmp/escaped-iroha3d",
        ),
    ),
)
def test_receipt_rejects_nested_or_unbound_seed_replay(
    tmp_path: Path,
    pattern: str,
    replacement: str,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)

    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    mutated, replacement_count = re.subn(pattern, replacement, row[10], count=1)
    assert replacement_count == 1
    row[10] = mutated
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    completion_fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed summary row 17 is not the exact release run" in result.stderr


def test_receipt_rejects_seed_replay_prebuilt_manifest_drift(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    manifest = evidence["prebuilt_manifest"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    assert isinstance(manifest, Path)

    lines = summary.read_text(encoding="utf-8").splitlines()
    row = lines[18].split("\t")
    expected = f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={sha256(manifest)}"
    mutated, replacement_count = re.subn(
        re.escape(expected),
        f"IROHA_RELEASE_PREBUILT_MANIFEST_SHA256={'0' * 64}",
        row[10],
        count=1,
    )
    assert replacement_count == 1
    row[10] = mutated
    lines[18] = "\t".join(row)
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    completion_fields = read_tsv_fields(completion)
    completion_fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, completion_fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "seed summary row 17 is not the exact release run" in result.stderr


def test_receipt_rejects_rehashed_chaos_log_without_required_semantics(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    chaos_log = evidence["chaos_log"]
    completion = evidence["chaos_completion"]
    assert isinstance(chaos_log, Path)
    assert isinstance(completion, Path)
    chaos_log.write_text("forged 100000-height success\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["log_sha256"] = sha256(chaos_log)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "does not prove its one exact passing release test" in result.stderr

    duplicate_root = tmp_path / "duplicate-marker"
    duplicate_root.mkdir()
    duplicate_evidence = make_evidence(duplicate_root)
    duplicate_writer = fixture_writer(duplicate_root)
    duplicate_log = duplicate_evidence["chaos_log"]
    duplicate_completion = duplicate_evidence["chaos_completion"]
    assert isinstance(duplicate_log, Path)
    assert isinstance(duplicate_completion, Path)
    duplicate_log.write_text(
        duplicate_log.read_text(encoding="utf-8") + CHAOS_MARKER + "\n",
        encoding="utf-8",
    )
    duplicate_fields = dict(
        line.split("\t", 1)
        for line in duplicate_completion.read_text(encoding="utf-8").splitlines()
    )
    duplicate_fields["log_sha256"] = sha256(duplicate_log)
    write_tsv(duplicate_completion, duplicate_fields)

    duplicate_result = run_writer(
        duplicate_evidence, duplicate_root / "receipt.json", duplicate_writer
    )

    assert duplicate_result.returncode == 1
    assert "does not prove its one exact passing release test" in (
        duplicate_result.stderr
    )

    counter_root = tmp_path / "wrong-counter"
    counter_root.mkdir()
    counter_evidence = make_evidence(counter_root)
    counter_writer = fixture_writer(counter_root)
    counter_completion = counter_evidence["chaos_completion"]
    assert isinstance(counter_completion, Path)
    counter_fields = dict(
        line.split("\t", 1)
        for line in counter_completion.read_text(encoding="utf-8").splitlines()
    )
    counter_fields["wal_append_restarts"] = "315"
    write_tsv(counter_completion, counter_fields)

    counter_result = run_writer(
        counter_evidence, counter_root / "receipt.json", counter_writer
    )

    assert counter_result.returncode == 1
    assert "does not match the exact release identity and reducer schedule" in (
        counter_result.stderr
    )


def test_receipt_rejects_seed_summary_row_with_extra_column(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    summary = evidence["seed_summary"]
    completion = evidence["seed_completion"]
    assert isinstance(summary, Path)
    assert isinstance(completion, Path)
    lines = summary.read_text(encoding="utf-8").splitlines()
    lines[1] += "\tforged-extra-column"
    summary.write_text("\n".join(lines) + "\n", encoding="utf-8")
    fields = dict(
        line.split("\t", 1)
        for line in completion.read_text(encoding="utf-8").splitlines()
    )
    fields["summary_sha256"] = sha256(summary)
    write_tsv(completion, fields)

    result = run_writer(evidence, tmp_path / "receipt.json", writer)

    assert result.returncode == 1
    assert "extra or missing columns" in result.stderr
