"""Bootstrap archive cases executed by the parent release-receipt suite."""

@pytest.mark.parametrize(
    ("field_path", "replacement"),
    [
        (("schema_version",), True),
        (("schema_version",), 1.0),
        (("trust_boundary", "same_uid_and_trusted_ancestor_owners"), 1),
        (("trusted_inputs", "revocation", "size_bytes"), False),
        (("runner", "size_bytes"), True),
        (("runner", "size_bytes"), 1.0),
        (("runner", "mode"), 0o755),
        (("runner", "path_entries"), ["relative-path"]),
        (
            ("runner", "self_digest_environment_variables"),
            ["SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256"],
        ),
        (("trusted_execution_probes", "bash", "exit_status"), False),
    ],
)
def test_receipt_rejects_bootstrap_marker_schema_and_type_confusion(
    tmp_path: Path, field_path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    mutate_bootstrap_marker(
        evidence,
        lambda value: set_nested(value, field_path, replacement),
    )

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


def test_receipt_rejects_bootstrap_marker_without_exact_external_digest(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence["expected_bootstrap_completion_sha256"] = "0" * 64

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "out-of-band digest" in result.stderr
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize(
    "artifact_name",
    [
        "trusted-bootstrap.py",
        "compute-manifest.py",
        "verify-identity.py",
        "python3",
        "bash",
        "bootstrap-allowed-signers",
        "bootstrap-revocation",
    ],
)
def test_receipt_rejects_tampered_bootstrap_trusted_archives(
    tmp_path: Path, artifact_name: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(directory, Path)
    artifact = directory / artifact_name
    original_mode = artifact.stat().st_mode & 0o7777
    artifact.chmod(0o700 if original_mode == 0o500 else 0o600)
    artifact.write_bytes(artifact.read_bytes() + b"tampered\n")
    artifact.chmod(original_mode)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize("mutation", ["mode", "symlink", "hardlink"])
def test_receipt_rejects_bootstrap_archive_path_and_inode_aliases(
    tmp_path: Path, mutation: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(directory, Path)
    helper = directory / "compute-manifest.py"
    if mutation == "mode":
        helper.chmod(0o600)
    elif mutation == "symlink":
        real = directory / "compute-manifest.real"
        helper.rename(real)
        helper.symlink_to(real.name)
    elif mutation == "hardlink":
        verifier = directory / "verify-identity.py"
        verifier.unlink()
        os.link(helper, verifier)
    else:
        raise AssertionError(mutation)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()
    assert not terminal_output_path(evidence).exists()


@pytest.mark.parametrize(
    ("key", "basename"),
    [
        ("bootstrap_completion", "BOOTSTRAP_COMPLETED.copy.json"),
        ("bootstrap_identity", "candidate-identity.copy.json"),
        ("bootstrap_attestation", "identity-attestation.copy.json"),
        ("bootstrap_transcript", "identity-transcript.copy.json"),
    ],
)
def test_receipt_rejects_bootstrap_cli_path_aliases(
    tmp_path: Path, key: str, basename: str
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    source = evidence[key]
    directory = evidence["bootstrap_evidence_dir"]
    assert isinstance(source, Path)
    assert isinstance(directory, Path)
    alias = directory / basename
    alias.write_bytes(source.read_bytes())
    alias.chmod(source.stat().st_mode & 0o7777)
    evidence[key] = alias

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exact evidence path" in result.stderr


@pytest.mark.parametrize(
    ("field_path", "replacement"),
    [
        (
            (
                "runner",
                "environment_without_self_digest",
                "IROHA_RELEASE_BOOTSTRAP_IDENTITY",
            ),
            "/tmp/aliased-candidate-identity.json",
        ),
        (
            (
                "runner",
                "environment_without_self_digest",
                "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256",
            ),
            "e" * 64,
        ),
        (
            (
                "runner",
                "environment_without_self_digest",
                "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256",
            ),
            "e" * 64,
        ),
        (
            (
                "runner",
                "environment_without_self_digest",
                "IROHA_RELEASE_SCALING_IROHAD_SHA256",
            ),
            "e" * 64,
        ),
        (
            (
                "runner",
                "environment_without_self_digest",
                "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256",
            ),
            "e" * 64,
        ),
        (("runner", "argv", "0"), "/bin/bash"),
        (("runner", "closed_path_resolution", "git"), "/usr/bin/git"),
    ],
)
def test_receipt_rejects_bootstrap_runner_aliases(
    tmp_path: Path, field_path: tuple[str, ...], replacement: object
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)

    def mutate(value: dict[str, object]) -> None:
        if field_path[-1].isdigit():
            current: object = value
            for field in field_path[:-2]:
                assert isinstance(current, dict)
                current = current[field]
            assert isinstance(current, dict)
            sequence = current[field_path[-2]]
            assert isinstance(sequence, list)
            sequence[int(field_path[-1])] = replacement
        else:
            set_nested(value, field_path, replacement)

    mutate_bootstrap_marker(evidence, mutate)

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "bootstrap" in result.stderr.lower()


def test_receipt_requires_distinct_original_and_sealed_candidate_roots(
    tmp_path: Path,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    evidence["bootstrap_candidate_root"] = evidence["release_root"]

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "must be distinct" in result.stderr


def test_receipt_requires_exact_bootstrap_release_source_shape(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    wrong_root = tmp_path / "wrong-sealed-source"
    wrong_root.mkdir()
    source_lock = evidence["signature_cargo_lock"]
    assert isinstance(source_lock, Path)
    (wrong_root / "Cargo.lock").write_bytes(source_lock.read_bytes())
    evidence["release_root"] = wrong_root

    result = run_writer(evidence, terminal_output_path(evidence), writer)

    assert result.returncode == 1
    assert "exact bootstrap release-runner source" in result.stderr


def test_receipt_requires_exact_terminal_output_path(tmp_path: Path) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    wrong_parent = tmp_path / "wrong-output"
    wrong_parent.mkdir(mode=0o700)
    wrong_parent.chmod(0o700)
    wrong_output = wrong_parent / "RELEASE_COMPLETED.json"

    result = run_writer(
        evidence,
        wrong_output,
        writer,
        use_supplied_output=True,
    )

    assert result.returncode == 1
    assert "exact bootstrap release output path" in result.stderr
    assert not wrong_output.exists()
    assert not terminal_output_path(evidence).exists()
