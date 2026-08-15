# Executed lexically in sumeragi_v2_release_bootstrap_test.py; do not import directly.

def test_release_runner_has_no_outer_timeout_or_output_capture(
    release_fixture: Fixture,
) -> None:
    source = BOOTSTRAP.read_text(encoding="utf-8")
    assert "--runner-timeout-seconds" not in source
    assert "_MAX_RUNNER_OUTPUT_BYTES" not in source
    assert "runner = _run_release_runner(" in source
    runner_source = source[
        source.index("def _run_release_runner(") : source.index(
            "def _open_runner_log("
        )
    ]
    assert "subprocess.PIPE" not in runner_source
    assert "selector" not in runner_source
    assert "stdout=stdout_descriptor" in runner_source
    assert "stderr=stderr_descriptor" in runner_source

    release_fixture.install_planned_runner(
        _runner(release_fixture.launch_count, release_fixture.candidate, "slow-success"),
    )
    arguments = _replace_flag(
        release_fixture.arguments(), "--command-timeout-seconds", "20"
    )
    started = time.monotonic()
    result = release_fixture.run(arguments, timeout_seconds=90)

    assert result.returncode == 0, result.stderr
    assert time.monotonic() - started >= 20.5
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert (release_fixture.evidence / "BOOTSTRAP_RELEASE_COMPLETED.json").is_file()


def test_blocked_bootstrap_diagnostics_cannot_backpressure_runner_output(
    release_fixture: Fixture,
) -> None:
    completed = release_fixture.root / "continuous-writer-completed"
    release_fixture.install_planned_runner(
        _continuous_writer_runner(
            release_fixture.launch_count,
            completed,
            chunks=32,
            hold_seconds=0.2,
        ),
    )
    process = subprocess.Popen(
        release_fixture.arguments(),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        env={"PATH": os.environ.get("PATH", "")},
    )
    _wait_for(completed, timeout=60)
    expected_size = int(completed.read_text(encoding="utf-8"))
    assert (release_fixture.evidence / "runner-stdout.log").stat().st_size >= expected_size
    assert (release_fixture.evidence / "runner-stderr.log").stat().st_size >= expected_size
    stdout, stderr = process.communicate(timeout=30)
    assert process.returncode == 37, stderr.decode("utf-8", "replace")
    assert stdout == b""


@pytest.mark.parametrize(
    "action",
    [
        "missing-receipt",
        "receipt-tamper",
        "receipt-wrong-mode",
        "receipt-hardlink",
        "receipt-symlink",
        "receipt-wrong-path",
        "receipt-wrong-schema",
        "receipt-wrong-bootstrap",
        "receipt-wrong-runner",
        "receipt-wrong-identity",
        "receipt-wrong-trust-policy",
        "receipt-mutual-wrong-signer",
        "receipt-missing-cross-tool-evidence",
        "preexisting-postmarker",
    ],
)
def test_success_status_without_exact_authenticated_receipt_fails_closed(
    release_fixture: Fixture, action: str
) -> None:
    release_fixture.install_planned_runner(
        _runner(release_fixture.launch_count, release_fixture.candidate, action),
    )

    result = release_fixture.run()

    if action in {"receipt-hardlink", "receipt-symlink", "receipt-wrong-path"}:
        # The failure publisher refuses to traverse or delete the hostile
        # receipt, but must reclaim the exact owner-private invocation root.
        assert result.returncode == 1, result.stderr
        assert not release_fixture.retained_root.exists()
        assert not release_fixture.evidence.exists()
    elif action in {"receipt-tamper", "receipt-wrong-mode"}:
        assert result.returncode == 2, result.stderr
        assert release_fixture.evidence.is_dir()
        assert {path.name for path in release_fixture.evidence.iterdir()} == {
            "RECEIPT_VALIDATION_FAILED.json",
            "receipt-validator-failure.stdout",
            "receipt-validator-failure.stderr",
        }
        assert not release_fixture.retained_root.exists()
    else:
        assert result.returncode == 2, result.stderr
        assert not release_fixture.evidence.exists()
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"


@pytest.mark.parametrize(
    "field",
    [
        "g_unit_focused_test_inventory",
        "prebuilt_binary_bundle",
        "formal_multilane_apalache_evidence",
        "formal_tlaps_resource_jsonl",
        "formal_tlaps_resource_summary",
        "multilane_scaling_bundle",
        "multilane_scaling_retained_validator",
        "multilane_scaling_trust_anchors",
        "g4p_multilane",
        "g12_cross_dataspace",
    ],
)
def test_terminal_receipt_requires_every_extended_release_field(
    release_fixture: Fixture, field: str
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=f'receipt["evidence"].pop({field!r})',
        ),
    )

    result = release_fixture.run()

    expected_status = 1 if field in {"g4p_multilane", "g12_cross_dataspace"} else 2
    assert result.returncode == expected_status, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()
    if expected_status == 1:
        assert release_fixture.retained_root.is_dir()


@pytest.mark.parametrize(
    "mutation",
    [
        'receipt["evidence"]["g_unit_focused_test_inventory"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g_unit_focused_test_inventory"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_multilane_apalache_evidence"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_multilane_apalache_evidence"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_tlaps_resource_jsonl"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_tlaps_resource_jsonl"]["sha256"] = "0" * 64',
        'receipt["evidence"]["formal_tlaps_resource_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["formal_tlaps_resource_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["size_bytes"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["manifest"]["mode"] = "0500"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["artifact_root"] = str(candidate)',
        'receipt["evidence"]["prebuilt_binary_bundle"]["archive_id"] = "release-prebuilt.bundle.v1:wrong"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["schema_version"] = 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["source_manifest_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["cargo_lock_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["cargo_version_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["rustc_version_sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["host_triple"] = "invalid"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["target_triple"] = "x86_64-unknown-linux-gnu"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["profile"] = "debug"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["bundle_dir"] = str(candidate)',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["tool_archive_id"] = "release-prebuilt.binary.iroha.v1"',
        pytest.param(
            'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["tool_archive_id"] = "release-candidate.runner.v1"',
            id="prebuilt-cargo-transcript-authenticated-tool-substitution",
        ),
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["operation_id"] = "cargo.version.alias"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["cargo"]["size_bytes"] = 0',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["tool_archive_id"] = "release-prebuilt.binary.irohad.v1"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["operation_id"] = "rustc.version.alias"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["version_transcripts"]["rustc"]["size_bytes"] = 65537',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["role"] = "wrong"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["relative_path"] = "release/wrong"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["size_bytes"] += 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["mode"] = "0400"',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["owner_uid"] = os.getuid()',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"][0]["nlink"] = 1',
        'receipt["evidence"]["prebuilt_binary_bundle"]["binaries"].pop()',
        'receipt["evidence"]["multilane_scaling_bundle"]["root"] = str(candidate)',
        'receipt["evidence"]["multilane_scaling_bundle"]["file_count"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["total_size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["relative_path"] = "../escape"',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["mode"] = "0500"',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["owner_uid"] = os.getuid()',
        'receipt["evidence"]["multilane_scaling_bundle"]["files"][0]["nlink"] = 1',
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["relative_path"] '
            '= "missing-scaling-evidence.json"'
        ),
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["path"] '
            '= str(candidate / "payload")'
        ),
        (
            '[record for record in receipt["evidence"]["multilane_scaling_bundle"]["files"] '
            'if record["relative_path"] == "scaling_evidence.json"][0]["sha256"] '
            '= "0" * 64'
        ),
        'receipt["evidence"]["multilane_scaling_bundle"]["directories"].append("missing")',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["mode"] = "0400"',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["owner_uid"] = os.getuid()',
        'receipt["evidence"]["multilane_scaling_retained_validator"]["nlink"] = 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["trial_harness_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["configuration_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["irohad_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["iroha_cli_sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["repository_root"] = str(candidate)',
        (
            'receipt["evidence"]["multilane_scaling_trust_anchors"]'
            '["retained_tooling"][0]["source_path"] = "scripts/wrong.sh"'
        ),
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["role"] = "wrong"',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["size_bytes"] += 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["mode"] = "0400"',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["owner_uid"] = os.getuid()',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"][0]["nlink"] = 1',
        'receipt["evidence"]["multilane_scaling_trust_anchors"]["retained_tooling"].pop()',
        'receipt["evidence"]["g4p_multilane"]["schema_version"] = 2',
        'receipt["evidence"]["g4p_multilane"]["completion"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["completion"]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["completion"]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_summary"]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["size_bytes"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["mode"] = "0500"',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["owner_uid"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"][0]["nlink"] += 1',
        'receipt["evidence"]["g4p_multilane"]["run_logs"].pop()',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_completion"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_summary"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"][0]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["seed_run_logs"].pop()',
        (
            'receipt["evidence"]["g12_cross_dataspace"]'
            '["fault_soak_completion"]["path"] = str(candidate / "payload")'
        ),
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_completion"]["nlink"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["path"] = str(candidate / "payload")',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["sha256"] = "0" * 64',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["size_bytes"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["mode"] = "0500"',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["owner_uid"] += 1',
        'receipt["evidence"]["g12_cross_dataspace"]["fault_soak_log"]["nlink"] += 1',
    ],
)
def test_terminal_receipt_extended_artifact_mutations_fail_closed(
    release_fixture: Fixture, mutation: str
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=mutation,
        ),
    )

    result = release_fixture.run()

    seal_time_path_mutation = any(
        marker in mutation
        for marker in (
            '["g4p_multilane"]["completion"]["path"]',
            '["g12_cross_dataspace"]["seed_completion"]["path"]',
            '["g12_cross_dataspace"]["fault_soak_completion"]["path"]',
        )
    )
    assert result.returncode == (1 if seal_time_path_mutation else 2), result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()
    if seal_time_path_mutation:
        assert release_fixture.retained_root.is_dir()


def _assert_terminal_receipt_mutation_rejected(
    release_fixture: Fixture, mutation: str, *, expected_status: int = 2
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            receipt_mutation_override=mutation,
        ),
    )

    result = release_fixture.run()

    assert result.returncode == expected_status, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()
    if expected_status == 1:
        assert release_fixture.retained_root.is_dir()


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            "evidence_file(g4p_root, 'untracked.log', b'untracked G-4P file\\n')",
            id="g4p",
        ),
        pytest.param(
            "evidence_file(g12_seed_root, 'untracked.log', b'untracked G-12 seed file\\n')",
            id="g12-seed",
        ),
        pytest.param(
            "evidence_file(g12_soak_root, 'untracked.log', b'untracked G-12 soak file\\n')",
            id="g12-soak",
        ),
        pytest.param(
            "evidence_file(scaling_root, 'untracked.log', b'untracked scaling file\\n')",
            id="scaling",
        ),
    ],
)
def test_terminal_receipt_rejects_extra_live_closed_inventory_files(
    release_fixture: Fixture, mutation: str
) -> None:
    _assert_terminal_receipt_mutation_rejected(release_fixture, mutation)


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["files"].reverse()',
            id="unsorted-files",
        ),
        pytest.param(
            'files = receipt["evidence"]["multilane_scaling_bundle"]["files"]\n'
            "files.append(dict(files[-1]))\n"
            'receipt["evidence"]["multilane_scaling_bundle"]["file_count"] += 1\n'
            'receipt["evidence"]["multilane_scaling_bundle"]["total_size_bytes"] += files[-1]["size_bytes"]',
            id="duplicate-files",
        ),
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["directories"].reverse()',
            id="unsorted-directories",
        ),
        pytest.param(
            'directories = receipt["evidence"]["multilane_scaling_bundle"]["directories"]\n'
            "directories.append(directories[-1])",
            id="duplicate-directories",
        ),
        pytest.param(
            'receipt["evidence"]["multilane_scaling_bundle"]["directories"][0] = "../escape"',
            id="directory-traversal",
        ),
    ],
)
def test_terminal_receipt_rejects_duplicate_or_unsorted_scaling_inventory(
    release_fixture: Fixture, mutation: str
) -> None:
    _assert_terminal_receipt_mutation_rejected(release_fixture, mutation)


def test_terminal_receipt_rejects_g12_seed_soak_root_alias(
    release_fixture: Fixture,
) -> None:
    _assert_terminal_receipt_mutation_rejected(
        release_fixture,
        (
            'g12 = receipt["evidence"]["g12_cross_dataspace"]\n'
            'g12["fault_soak_completion"] = dict(g12["seed_completion"])'
        ),
        expected_status=1,
    )


@pytest.mark.parametrize(
    "flag",
    [
        "--expected-bootstrap-sha256",
        "--expected-python-sha256",
        "--expected-git-sha256",
        "--expected-ssh-keygen-sha256",
        "--expected-manifest-helper-sha256",
        "--expected-identity-verifier-sha256",
        "--expected-receipt-validator-sha256",
        "--expected-receipt-validator-support-sha256",
        "--expected-runtime-helper-sha256",
        "--expected-runner-tool-manifest-sha256",
        "--expected-bash-sha256",
        "--expected-ssh-allowed-signers-sha256",
        "--expected-ssh-revocation-sha256",
    ],
)
def test_protected_hash_mismatch_never_launches(
    release_fixture: Fixture, flag: str
) -> None:
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), flag, "0" * 64)
    )
    _assert_never_launched(release_fixture, result)


def test_receipt_validator_support_omission_never_launches(
    release_fixture: Fixture,
) -> None:
    arguments = release_fixture.arguments()
    index = arguments.index("--receipt-validator-support")
    del arguments[index : index + 2]

    result = release_fixture.run(arguments)

    _assert_never_launched(release_fixture, result)
    probe_digest_mismatch = release_fixture.run(
        _replace_flag(
            release_fixture.arguments(),
            "--expected-tool-probe-helper-sha256",
            "0" * 64,
        )
    )
    _assert_never_launched(release_fixture, probe_digest_mismatch)


def test_relative_trusted_path_never_launches(release_fixture: Fixture) -> None:
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), "--git-bin", "trust/git")
    )
    _assert_never_launched(release_fixture, result)


def test_nonisolated_python_startup_never_launches(release_fixture: Fixture) -> None:
    arguments = release_fixture.arguments()
    del arguments[1:3]
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "input_flag",
    [
        "--git-bin",
        "--manifest-helper",
        "--receipt-validator-support",
        "--ssh-allowed-signers",
    ],
)
def test_candidate_contained_trust_input_never_launches(
    release_fixture: Fixture, input_flag: str
) -> None:
    source = {
        "--git-bin": release_fixture.git,
        "--manifest-helper": release_fixture.manifest,
        "--receipt-validator-support": release_fixture.receipt_validator_support,
        "--ssh-allowed-signers": release_fixture.allowed,
    }[input_flag]
    destination = _write(
        release_fixture.candidate / f"untrusted-{source.name}",
        source.read_bytes(),
        stat.S_IMODE(source.stat().st_mode),
    )
    arguments = _replace_flag(release_fixture.arguments(), input_flag, str(destination))
    digest_flag = {
        "--git-bin": "--expected-git-sha256",
        "--manifest-helper": "--expected-manifest-helper-sha256",
        "--receipt-validator-support": (
            "--expected-receipt-validator-support-sha256"
        ),
        "--ssh-allowed-signers": "--expected-ssh-allowed-signers-sha256",
    }[input_flag]
    arguments = _replace_flag(arguments, digest_flag, _sha256(destination))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_bootstrap_copy_inside_candidate_is_rejected(release_fixture: Fixture) -> None:
    copied = _write(
        release_fixture.candidate / "bootstrap.py", BOOTSTRAP.read_bytes(), 0o500
    )
    arguments = release_fixture.arguments()
    arguments[3] = str(copied)
    arguments = _replace_flag(arguments, "--expected-bootstrap-sha256", _sha256(copied))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_symlinked_tool_never_launches(release_fixture: Fixture) -> None:
    link = release_fixture.trust / "git-link"
    link.symlink_to(release_fixture.git)
    result = release_fixture.run(
        _replace_flag(release_fixture.arguments(), "--git-bin", str(link))
    )
    _assert_never_launched(release_fixture, result)


def test_wrong_python_even_with_matching_hash_never_launches(release_fixture: Fixture) -> None:
    fake = _write(release_fixture.trust / "python", "#!/bin/sh\nexit 0\n", 0o500)
    arguments = _replace_flag(release_fixture.arguments(), "--python-bin", str(fake))
    arguments = _replace_flag(arguments, "--expected-python-sha256", _sha256(fake))
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_identity_rejection_never_launches(release_fixture: Fixture) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "reject.py", _identity_verifier(reject=True), 0o500
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "policy",
    [
        "release valid-after=\"20260717Z\" ssh-ed25519 AAAATEST\n",
        "release valid-before=\"20270717Z\" ssh-ed25519 AAAATEST\n",
        "release ssh-ed25519 AAAATEST\nbackup ssh-ed25519 AAAATEST\n",
    ],
)
def test_bootstrap_independently_rejects_nondeterministic_signer_policy(
    release_fixture: Fixture, policy: str
) -> None:
    release_fixture.allowed = _write(
        release_fixture.trust / "nondeterministic-allowed-signers",
        policy,
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "verifier_source",
    [
        _identity_verifier(attestation_schema=1),
        _identity_verifier(attestation_schema=True),
        _identity_verifier(attestation_schema=2.0),
        _identity_verifier(transcript_schema=1),
        _identity_verifier(transcript_schema=True),
        _identity_verifier(transcript_schema=2.0),
        _identity_verifier(bad_evidence_digest=True),
    ],
)
def test_malformed_schema_v2_evidence_never_launches(
    release_fixture: Fixture, verifier_source: str
) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "malformed.py", verifier_source, 0o500
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize("target_name", ["git", "manifest", "allowed"])
def test_trusted_input_toctou_after_verification_never_launches(
    release_fixture: Fixture, target_name: str
) -> None:
    target = getattr(release_fixture, target_name)
    release_fixture.verifier = _write(
        release_fixture.trust / "mutator.py",
        _identity_verifier(mutate_path=target),
        0o500,
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


def test_source_drift_after_verification_never_launches(release_fixture: Fixture) -> None:
    release_fixture.verifier = _write(
        release_fixture.trust / "source-mutator.py",
        _identity_verifier(mutate_candidate=True),
        0o500,
    )
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


@pytest.mark.parametrize(
    "action",
    [
        "source-drift",
        "evidence-tamper",
        "marker-tamper",
        "directory-mode-tamper",
        "receipt-support-archive-omission",
        "receipt-support-archive-substitution",
    ],
)
def test_post_launch_tampering_fails_closed(
    release_fixture: Fixture, action: str
) -> None:
    release_fixture.install_planned_runner(
        _runner(release_fixture.launch_count, release_fixture.candidate, action),
    )
    result = release_fixture.run()
    runner_failure = action in {"marker-tamper", "directory-mode-tamper"}
    assert result.returncode == (1 if runner_failure else 2), result.stderr
    if runner_failure:
        assert "post-run bootstrap validation also failed" in result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_post_launch_trusted_tool_drift_fails_closed(release_fixture: Fixture) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "trusted-drift",
            trusted_mutation=release_fixture.git,
        ),
    )
    result = release_fixture.run()
    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_post_launch_receipt_support_source_drift_fails_closed(
    release_fixture: Fixture,
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "trusted-drift",
            trusted_mutation=release_fixture.receipt_validator_support,
        ),
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_post_launch_sdk_source_manifest_archive_drift_fails_closed(
    release_fixture: Fixture,
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "trusted-drift",
            trusted_mutation=(
                release_fixture.evidence
                / "sdk-dependency-bundle-manifest.json"
            ),
        ),
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert (
        "archived sdk dependency bundle manifest changed during the release bootstrap"
        in result.stderr
    )
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_runner_failure_status_is_preserved_exactly(release_fixture: Fixture) -> None:
    release_fixture.install_planned_runner(
        _runner(release_fixture.launch_count, release_fixture.candidate, "fail"),
    )
    result = release_fixture.run()
    assert result.returncode == 37, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_protected_receipt_validator_failure_blocks_external_completion(
    release_fixture: Fixture,
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "reject-receipt.py",
        "raise SystemExit(72)\n",
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "protected receipt validation failed" in result.stderr.lower()
    assert release_fixture.evidence.is_dir()
    assert {path.name for path in release_fixture.evidence.iterdir()} == {
        "RECEIPT_VALIDATION_FAILED.json",
        "receipt-validator-failure.stdout",
        "receipt-validator-failure.stderr",
    }
    assert not release_fixture.retained_root.exists()
    assert not {
        "BOOTSTRAP_RELEASE_COMPLETED.json",
        "RELEASE_COMPLETED.json",
        "receipt-validation-ack.json",
        "release-retained-inventory.json",
        "release-runner-result.json",
        "sealed-identity.json",
    } & {path.name for path in release_fixture.evidence.iterdir()}


def test_bootstrap_protected_validation_accepts_real_terminal_receipt(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    from pytests.scripts import (
        sumeragi_v2_release_receipt_test as receipt_contract,
    )

    fixture_root = tmp_path / "nested" / "real-receipt"
    fixture_root.mkdir(parents=True)
    evidence: dict[str, object] = receipt_contract.make_evidence(fixture_root)
    writer = receipt_contract.fixture_writer(fixture_root)
    bootstrap_evidence = evidence["bootstrap_evidence_dir"]
    release_root = evidence["release_root"]
    assert isinstance(bootstrap_evidence, Path)
    assert isinstance(release_root, Path)
    release_output = release_root.parent / "output"
    for anchor_key, directory_name in (
        ("corridor_completion", "corridor"),
        ("formal_completion", "formal"),
        ("seed_completion", "seed"),
        ("chaos_completion", "chaos"),
        ("taira_completion", "taira"),
        ("scaling_manifest", "scaling"),
        ("g4p_completion", "g4p"),
        ("g12_seed_completion", "g12-seed"),
        ("g12_soak_completion", "g12-soak"),
    ):
        _relocate_receipt_evidence_root(
            evidence, anchor_key, release_output / directory_name
        )

    marker_path = evidence["bootstrap_completion"]
    scaling_manifest = evidence["scaling_manifest"]
    runner_environment = evidence["bootstrap_runner_environment"]
    assert isinstance(marker_path, Path)
    assert isinstance(scaling_manifest, Path)
    assert isinstance(runner_environment, dict)
    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    runner_environment = dict(runner_environment)
    runner_environment[SCALING_EVIDENCE_ENV] = str(scaling_manifest)
    evidence["bootstrap_runner_environment"] = runner_environment
    marker["runner"]["environment_sha256"] = hashlib.sha256(
        (json.dumps(runner_environment, sort_keys=True, separators=(",", ":")) + "\n").encode()
    ).hexdigest()
    _write(
        marker_path,
        (json.dumps(marker, sort_keys=True, separators=(",", ":")) + "\n").encode(),
        0o400,
    )
    evidence["expected_bootstrap_completion_sha256"] = _sha256(marker_path)

    _rebind_bootstrap_trusted_input(
        evidence,
        label="receipt_validator",
        source=writer,
        archive_name="validate-receipt.py",
        archive_mode=0o400,
    )
    _rebind_bootstrap_trusted_input(
        evidence,
        label="python",
        source=PYTHON,
        archive_name="python3",
        archive_mode=0o500,
    )
    rebound_runner_environment = evidence["bootstrap_runner_environment"]
    assert isinstance(rebound_runner_environment, dict)
    runner_environment = dict(rebound_runner_environment)
    sealed_source = evidence["sealed"]
    bootstrap_identity = evidence["bootstrap_identity"]
    assert isinstance(sealed_source, Path)
    assert isinstance(bootstrap_identity, Path)
    sealed_identity = _write(
        release_root.parent / "sealed-identity.json",
        sealed_source.read_bytes(),
        0o400,
    )
    evidence["candidate"] = bootstrap_identity
    evidence["sealed"] = sealed_identity

    writer_tmp = fixture_root / "writer-tmp"
    writer_tmp.mkdir()
    monkeypatch.setenv("TMPDIR", str(writer_tmp))
    terminal_output = evidence["terminal_output"]
    assert isinstance(terminal_output, Path)
    publication = receipt_contract.run_writer(
        evidence, terminal_output, writer
    )
    assert publication.returncode == 0, publication.stderr
    receipt_before = terminal_output.read_bytes()
    private_sdk_manifest = (
        bootstrap_evidence / "sdk-dependency-bundle-manifest.json"
    )
    assert private_sdk_manifest.is_file()
    invocation_root = release_root.parent
    output_root = invocation_root / "output"
    for path in (
        *(invocation_root / name for name in (
            "runtime", "sdk-inputs", "sdk-work", "target",
        )),
        *(output_root / name for name in (
            "home", "tmp", "cache", "cargo-home",
        )),
    ):
        if path.exists():
            shutil.rmtree(path)
    Path(evidence["runtime_tool_probe_manifest"]).unlink()
    private_sdk_manifest.unlink()
    for log_name in ("runner-stdout.log", "runner-stderr.log"):
        (bootstrap_evidence / log_name).chmod(0o400)

    spec = importlib.util.spec_from_file_location(
        "sumeragi_release_bootstrap_real_receipt", BOOTSTRAP
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)

    def snapshot(path: Path, label: str, maximum_bytes: int) -> object:
        return module._read_file(path, label, maximum_bytes=maximum_bytes)

    marker = json.loads(marker_path.read_text(encoding="utf-8"))
    trusted_inputs = marker["trusted_inputs"]
    archives = {
        "python": snapshot(
            bootstrap_evidence / trusted_inputs["python"]["archive_name"],
            "archived Python",
            module._MAX_TOOL_BYTES,
        ),
        "receipt_validator": snapshot(
            bootstrap_evidence
            / trusted_inputs["receipt_validator"]["archive_name"],
            "archived receipt validator",
            module._MAX_HELPER_BYTES,
        ),
        "receipt_validator_support": snapshot(
            bootstrap_evidence / RECEIPT_VALIDATOR_SUPPORT.name,
            "archived receipt validator support",
            module._MAX_HELPER_BYTES,
        ),
    }
    protected = {
        label: snapshot(
            bootstrap_evidence / trusted_inputs[label]["archive_name"],
            f"protected {label}",
            (
                module._MAX_POLICY_BYTES
                if label in {"allowed_signers", "revocation"}
                else module._MAX_TOOL_BYTES
            ),
        )
        for label in ("git", "ssh_keygen", "allowed_signers", "revocation")
    }
    identity_outputs = {
        "attestation": evidence["bootstrap_attestation"],
        "transcript": evidence["bootstrap_transcript"],
        "raw_commit": evidence["bootstrap_identity_raw_commit"],
        "cargo_lock": evidence["bootstrap_identity_cargo_lock"],
        "allowed": evidence["bootstrap_identity_allowed_signers"],
        "revocation": evidence["bootstrap_identity_revocation"],
        "git": evidence["bootstrap_identity_git"],
        "ssh": evidence["bootstrap_identity_ssh_keygen"],
    }
    assert all(isinstance(path, Path) for path in identity_outputs.values())
    marker_sha256 = _sha256(marker_path)
    environment = runner_environment | {
        "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_sha256,
        "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256": marker_sha256,
    }
    receipt = json.loads(receipt_before)
    validation = module._run_protected_receipt_validator(
        evidence=bootstrap_evidence,
        candidate=Path(evidence["bootstrap_candidate_root"]),
        receipt=receipt,
        receipt_snapshot=snapshot(
            terminal_output,
            "terminal receipt",
            module._MAX_TERMINAL_RECEIPT_BYTES,
        ),
        sealed_identity_snapshot=snapshot(
            sealed_identity, "sealed identity", module._MAX_IDENTITY_BYTES
        ),
        sealed_root=Path(evidence["release_root"]),
        archives=archives,
        protected=protected,
        identity_snapshot=snapshot(
            bootstrap_identity,
            "bootstrap identity",
            module._MAX_IDENTITY_BYTES,
        ),
        identity_outputs=identity_outputs,
        bootstrap_marker=snapshot(
            marker_path, "bootstrap marker", module._MAX_EVIDENCE_BYTES
        ),
        expected_signer_fingerprint=str(evidence["expected_signer_fingerprint"]),
        environment=environment,
        timeout_seconds=30,
    )

    assert validation.returncode == 0
    assert terminal_output.read_bytes() == receipt_before
    assert not (bootstrap_evidence / "__pycache__").exists()


def test_full_bootstrap_succeeds_with_real_terminal_receipt_validator(
    release_fixture: Fixture, monkeypatch: pytest.MonkeyPatch
) -> None:
    from pytests.scripts import (
        sumeragi_v2_release_receipt_test as receipt_contract,
    )

    fixture_root = release_fixture.root / "full-bootstrap-real-receipt"
    fixture_root.mkdir()
    evidence: dict[str, object] = receipt_contract.make_evidence(fixture_root)
    writer = receipt_contract.fixture_writer(fixture_root)
    bootstrap_evidence = evidence["bootstrap_evidence_dir"]
    release_root = evidence["release_root"]
    assert isinstance(bootstrap_evidence, Path)
    assert isinstance(release_root, Path)
    _write(
        release_root
        / "scripts"
        / "copy_sumeragi_v2_release_cargo_cache_validation_ack.py",
        (
            REPO_ROOT
            / "scripts"
            / "copy_sumeragi_v2_release_cargo_cache_validation_ack.py"
        ).read_bytes(),
        0o400,
    )
    release_output = release_root.parent / "output"
    for anchor_key, directory_name in (
        ("corridor_completion", "corridor"),
        ("formal_completion", "formal"),
        ("seed_completion", "seed"),
        ("chaos_completion", "chaos"),
        ("taira_completion", "taira"),
        ("g4p_completion", "g4p"),
        ("g12_seed_completion", "g12-seed"),
        ("g12_soak_completion", "g12-soak"),
    ):
        _relocate_receipt_evidence_root(
            evidence, anchor_key, release_output / directory_name
        )

    # Scaling evidence is intentionally external to the bootstrap evidence
    # tree. Its absolute root and digests are authenticated runner inputs.
    scaling_manifest = evidence["scaling_manifest"]
    assert isinstance(scaling_manifest, Path)
    assert bootstrap_evidence not in scaling_manifest.parents

    _rebind_bootstrap_trusted_input(
        evidence,
        label="receipt_validator",
        source=writer,
        archive_name="validate-receipt.py",
        archive_mode=0o400,
    )
    _rebind_bootstrap_trusted_input(
        evidence,
        label="python",
        source=PYTHON,
        archive_name="python3",
        archive_mode=0o500,
    )
    _rebind_bootstrap_trusted_input(
        evidence,
        label="bash",
        source=release_fixture.bash,
        archive_name="bash",
        archive_mode=0o500,
    )
    sealed_source = evidence["sealed"]
    bootstrap_identity = evidence["bootstrap_identity"]
    assert isinstance(sealed_source, Path)
    assert isinstance(bootstrap_identity, Path)
    sealed_identity = _write(
        release_root.parent / "sealed-identity.json",
        sealed_source.read_bytes(),
        0o400,
    )
    evidence["candidate"] = bootstrap_identity
    evidence["sealed"] = sealed_identity

    writer_tmp = fixture_root / "writer-tmp"
    writer_tmp.mkdir()
    monkeypatch.setenv("TMPDIR", str(writer_tmp))
    terminal_output = evidence["terminal_output"]
    assert isinstance(terminal_output, Path)
    publication = receipt_contract.run_writer(
        evidence, terminal_output, writer
    )
    assert publication.returncode == 0, publication.stderr
    terminal_output.unlink()
    release_root.chmod(0o500)
    protected_source_bytes = {}
    for attribute, evidence_key, filename, mode in (
        ("git", "signature_git", "real-git", 0o500),
        ("ssh", "signature_ssh_keygen", "real-ssh-keygen", 0o500),
        ("allowed", "signature_allowed_signers", "real-allowed-signers", 0o400),
        ("revocation", "signature_revocation", "real-revocation", 0o400),
    ):
        source = evidence[evidence_key]
        assert isinstance(source, Path)
        protected_source_bytes[attribute] = (filename, source.read_bytes(), mode)
    protected_source_bytes["bash"] = (
        "real-bash",
        (bootstrap_evidence / "bash").read_bytes(),
        0o500,
    )
    signature_cargo_lock = evidence["signature_cargo_lock"]
    assert isinstance(signature_cargo_lock, Path)
    signature_cargo_lock_bytes = signature_cargo_lock.read_bytes()
    candidate_identity_json = bootstrap_identity.read_text(
        encoding="utf-8"
    ).strip()
    sealed_identity_json = sealed_source.read_text(encoding="utf-8").strip()

    staged_bootstrap = fixture_root / "prepared-bootstrap"
    shutil.move(str(bootstrap_evidence), staged_bootstrap)
    assert staged_bootstrap.is_dir()
    assert not bootstrap_evidence.exists()
    fixture_root.chmod(0o700)
    release_fixture.evidence = bootstrap_evidence

    release_fixture.manifest = _write(
        release_fixture.trust / "fixed-release-identity.py",
        f'''#!/usr/bin/env python3
import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--root", type=Path, required=True)
parser.add_argument("--release-identity-json", action="store_true", required=True)
args = parser.parse_args()
root = args.root.resolve(strict=True)
if root == Path({str(release_fixture.candidate)!r}):
    print({candidate_identity_json!r})
elif root == Path({str(release_root)!r}):
    print({sealed_identity_json!r})
else:
    raise SystemExit(71)
''',
        0o500,
    )
    release_fixture.verifier = _write(
        release_fixture.trust / "real-identity-verifier.py",
        (
            REPO_ROOT / "scripts" / "verify_sumeragi_v2_release_identity.py"
        ).read_bytes(),
        0o500,
    )
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "real-receipt-validator.py",
        writer.read_bytes(),
        0o500,
    )
    release_fixture.sdk_manifest = _write(
        release_fixture.trust / "real-sdk-dependency-bundle-manifest.json",
        (staged_bootstrap / "sdk-dependency-bundle-manifest.json").read_bytes(),
        0o400,
    )
    release_fixture.tool_probe_helper = _write(
        release_fixture.trust / "real-tool-probe-helper.py",
        (staged_bootstrap / "probe-release-tools.py").read_bytes(),
        0o400,
    )
    for component_name in (
        "write_sumeragi_v2_release_receipt_corridor_log.py",
        "write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "write_sumeragi_v2_release_receipt_gate_evidence.py",
        "write_sumeragi_v2_release_receipt_publication.py",
    ):
        _write(
            release_fixture.trust / component_name,
            (writer.parent / component_name).read_bytes(),
            0o400,
        )
    for attribute, (filename, source_bytes, mode) in protected_source_bytes.items():
        setattr(
            release_fixture,
            attribute,
            _write(release_fixture.trust / filename, source_bytes, mode),
        )
    _write(
        release_fixture.candidate / "Cargo.lock",
        signature_cargo_lock_bytes,
    )
    runner_tool_manifest = json.loads(
        release_fixture.tool_manifest.read_text(encoding="utf-8")
    )
    for name in runner_tool_manifest["tools"]:
        source = staged_bootstrap / "runner-tools" / name
        assert source.is_file() and not source.is_symlink()
        runner_tool_manifest["tools"][name] = {
            "path": str(source.resolve(strict=True)),
            "sha256": _sha256(source),
        }
    _write(
        release_fixture.tool_manifest,
        json.dumps(
            runner_tool_manifest, sort_keys=True, separators=(",", ":")
        )
        + "\n",
        0o400,
    )

    def evidence_path(name: str) -> str:
        path = evidence[name]
        assert isinstance(path, Path)
        return shlex.quote(str(path))

    runner = f'''#!/bin/bash
set -eu
: "${{SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION:?}}"
: "${{SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256:?}}"
count=0
if test -f {shlex.quote(str(release_fixture.launch_count))}; then
    count=$(<{shlex.quote(str(release_fixture.launch_count))})
fi
count=$((count + 1))
printf '%s\\n' "$count" > {shlex.quote(str(release_fixture.launch_count))}
release_runner={shlex.quote(str(release_root.parent))}
receipt_arguments=( \
    --candidate-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
    --sealed-identity "$release_runner/sealed-identity.json" \
    --release-root {shlex.quote(str(release_root))} \
    --bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION" \
    --bootstrap-evidence-dir "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
    --bootstrap-identity "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY" \
    --bootstrap-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
    --bootstrap-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
    --expected-bootstrap-completion-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256" \
    --bootstrap-candidate-root {shlex.quote(str(release_fixture.candidate))} \
    --bootstrap-runner {shlex.quote(str(release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"))} \
    --signature-attestation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_ATTESTATION" \
    --signature-transcript "$SUMERAGI_V2_RELEASE_BOOTSTRAP_IDENTITY_TRANSCRIPT" \
    --signature-raw-commit "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-raw-commit" \
    --signature-cargo-lock "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-Cargo.lock" \
    --signature-allowed-signers "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-allowed-signers" \
    --signature-revocation "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-revocation" \
    --signature-git "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-git" \
    --signature-ssh-keygen "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/identity-ssh-keygen" \
    --expected-git-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_GIT_SHA256" \
    --expected-ssh-keygen-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256" \
    --expected-allowed-signers-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256" \
    --expected-revocation-sha256 "$SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256" \
    --expected-signer-fingerprint "$SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
    --corridor-completion {evidence_path("corridor_completion")} \
    --formal-completion {evidence_path("formal_completion")} \
    --seed-completion {evidence_path("seed_completion")} \
    --chaos-completion {evidence_path("chaos_completion")} \
    --taira-completion {evidence_path("taira_completion")} \
    --g4p-completion {evidence_path("g4p_completion")} \
    --g12-seed-completion {evidence_path("g12_seed_completion")} \
    --g12-fault-soak-completion {evidence_path("g12_soak_completion")} \
    --scaling-evidence-manifest {shlex.quote(str(scaling_manifest))} \
    --sdk-dependency-archive "$release_runner/sdk-dependency-bundle.tar" \
    --sdk-dependency-input-inventory "$release_runner/sdk-dependency-input.json" \
    --sdk-dependency-final-work-inventory "$release_runner/sdk-dependency-work-final.json" \
    --runtime-tool-probe-manifest "$release_runner/runtime-tool-probe-manifest.json" \
    --runtime-tool-probe-result "$release_runner/runtime-tool-probe-result.json" \
    --expected-scaling-trial-harness-sha256 "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
    --expected-scaling-configuration-sha256 "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
    --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
    --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256" \
    --repository-root {shlex.quote(str(release_root))} \
    --output {shlex.quote(str(terminal_output))} \
)
python3 -I -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py" \
    "${{receipt_arguments[@]}}"
source_manifest_sha256="$(python3 -I -S -c 'import json,sys;print(json.load(open(sys.argv[1], encoding="utf-8"))["workspace_source_manifest_sha256"])' "$release_runner/sealed-identity.json")"
python3 -I -S -c 'import os,sys;[os.chmod(path, 0o400) for path in sys.argv[1:]]' \
    "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/runner-stdout.log" \
    "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/runner-stderr.log"
python3 -I -S "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR/validate-receipt.py" \
    "${{receipt_arguments[@]}}" \
    --verify-existing \
    --validation-ack "$release_runner/receipt-validation-ack.json" \
    --source-manifest-sha256 "$source_manifest_sha256"
python3 -I -S "$IROHA_RELEASE_RUNTIME_HELPER" \
    --seal-release-result \
    --invocation-root "$release_runner" \
    --bootstrap-evidence "$SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR" \
    --source-manifest-sha256 "$source_manifest_sha256" \
    --candidate-root {shlex.quote(str(release_fixture.candidate))} \
    --scaling-evidence-manifest {shlex.quote(str(scaling_manifest))} \
    --expected-signer-fingerprint "$SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT" \
    --expected-scaling-trial-harness-sha256 "$IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256" \
    --expected-scaling-configuration-sha256 "$IROHA_RELEASE_SCALING_CONFIGURATION_SHA256" \
    --expected-scaling-irohad-sha256 "$IROHA_RELEASE_SCALING_IROHAD_SHA256" \
    --expected-scaling-iroha-cli-sha256 "$IROHA_RELEASE_SCALING_IROHA_CLI_SHA256"
'''
    release_fixture.install_planned_runner(runner)

    scaling_environment = {
        SCALING_EVIDENCE_ENV: str(scaling_manifest),
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": str(
            evidence["expected_scaling_trial_harness_sha256"]
        ),
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": str(
            evidence["expected_scaling_configuration_sha256"]
        ),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": str(
            evidence["expected_scaling_irohad_sha256"]
        ),
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": str(
            evidence["expected_scaling_iroha_cli_sha256"]
        ),
    }
    arguments = release_fixture.arguments()
    for name, value in scaling_environment.items():
        arguments = _replace_runner_environment(arguments, name, value)
    arguments = _replace_flag(arguments, "--command-timeout-seconds", "20")

    result = release_fixture.run(arguments)

    assert result.returncode == 0, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert (release_fixture.evidence / "BOOTSTRAP_RELEASE_COMPLETED.json").is_file()
    assert not (
        release_fixture.evidence / "sdk-dependency-bundle-manifest.json"
    ).exists()
    marker = json.loads(
        (release_fixture.evidence / "BOOTSTRAP_COMPLETED.json").read_text(
            encoding="utf-8"
        )
    )
    assert "environment_without_self_digest" not in marker["runner"]
    assert re.fullmatch(
        r"[0-9a-f]{64}", marker["runner"]["environment_sha256"]
    )
    receipt = json.loads(terminal_output.read_text(encoding="utf-8"))
    scaling_bundle = receipt["evidence"]["multilane_scaling_bundle"]
    assert "root" not in scaling_bundle
    assert any(
        record["relative_path"] == "scaling_evidence.json"
        for record in scaling_bundle["files"]
    )
    assert release_fixture.evidence not in scaling_manifest.parents


def test_bootstrap_invokes_real_terminal_receipt_validator(
    release_fixture: Fixture,
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "real-receipt-validator.py",
        (REPO_ROOT / "scripts" / "write_sumeragi_v2_release_receipt.py").read_bytes(),
        0o500,
    )
    for component_name in (
        "write_sumeragi_v2_release_receipt_corridor_log.py",
        "write_sumeragi_v2_release_receipt_formal_artifacts.py",
        "write_sumeragi_v2_release_receipt_gate_evidence.py",
        "write_sumeragi_v2_release_receipt_publication.py",
    ):
        _write(
            release_fixture.trust / component_name,
            (REPO_ROOT / "scripts" / component_name).read_bytes(),
            0o400,
        )

    result = release_fixture.run()

    assert result.returncode == 2
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "protected receipt validation failed" in result.stderr
    validator_stderr = (
        release_fixture.evidence / "receipt-validator-failure.stderr"
    ).read_text(encoding="utf-8")
    assert "Sumeragi v2 release receipt error:" in validator_stderr
    assert "the following arguments are required" not in result.stderr
    assert "unrecognized arguments" not in result.stderr
    assert {path.name for path in release_fixture.evidence.iterdir()} == {
        "RECEIPT_VALIDATION_FAILED.json",
        "receipt-validator-failure.stdout",
        "receipt-validator-failure.stderr",
    }


@pytest.mark.parametrize(
    ("binding", "needle", "replacement"),
    [
        pytest.param(
            "g4p-completion",
            'artifact_path("g4p_multilane", "completion")',
            'artifact_path("g12_cross_dataspace", "seed_completion")',
            id="g4p-completion-source",
        ),
        pytest.param(
            "g12-seed-completion",
            'artifact_path("g12_cross_dataspace", "seed_completion")',
            'artifact_path("g4p_multilane", "completion")',
            id="g12-seed-completion-source",
        ),
        pytest.param(
            "g12-fault-soak-completion",
            'artifact_path("g12_cross_dataspace", "fault_soak_completion")',
            'artifact_path("g12_cross_dataspace", "seed_completion")',
            id="g12-fault-soak-completion-source",
        ),
        pytest.param(
            "scaling-evidence-manifest",
            '("path", scaling_manifest_path)',
            '("path", artifact_path("g4p_multilane", "completion"))',
            id="scaling-manifest-source",
        ),
        pytest.param(
            "scaling-trial-harness-digest",
            'scaling_trust["trial_harness_sha256"]',
            'scaling_trust["configuration_sha256"]',
            id="scaling-trial-harness-value",
        ),
        pytest.param(
            "scaling-configuration-digest",
            'scaling_trust["configuration_sha256"]',
            'scaling_trust["trial_harness_sha256"]',
            id="scaling-configuration-value",
        ),
        pytest.param(
            "scaling-irohad-digest",
            'scaling_trust["irohad_sha256"]',
            'scaling_trust["iroha_cli_sha256"]',
            id="scaling-irohad-value",
        ),
        pytest.param(
            "scaling-iroha-cli-digest",
            'scaling_trust["iroha_cli_sha256"]',
            'scaling_trust["irohad_sha256"]',
            id="scaling-iroha-cli-value",
        ),
    ],
)
def test_protected_receipt_validator_extended_value_source_mutations_fail_closed(
    release_fixture: Fixture,
    binding: str,
    needle: str,
    replacement: str,
) -> None:
    source = BOOTSTRAP.read_text(encoding="utf-8")
    assert source.count(needle) == 1
    mutated = _write(
        release_fixture.trust / f"bootstrap-{binding}.py",
        source.replace(needle, replacement, 1),
        0o500,
    )
    for component in BOOTSTRAP_COMPONENTS:
        _write(mutated.parent / component.name, component.read_bytes(), 0o400)
    arguments = release_fixture.arguments()
    arguments[3] = str(mutated)
    arguments = _replace_flag(
        arguments, "--expected-bootstrap-sha256", _sha256(mutated)
    )
    arguments = _replace_flag(arguments, "--command-timeout-seconds", "20")

    result = release_fixture.run(arguments)

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.is_file(), result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "receipt validator normalized option value is not exact" in result.stderr
    assert not release_fixture.evidence.exists()


@pytest.mark.parametrize(
    "mutation",
    [
        pytest.param(
            "target = Path(args.scaling_evidence_manifest)\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late artifact mutation\\n')",
            id="scaling-artifact",
        ),
        pytest.param(
            "target = Path(args.g4p_completion).parent / 'late.log'\n"
            "target.write_bytes(b'late directory mutation\\n')\n"
            "target.chmod(0o400)",
            id="g4p-directory-inventory",
        ),
        pytest.param(
            "target = Path(args.g12_seed_completion).parent / 'seed-00.log'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late G-12 seed-log mutation\\n')",
            id="g12-seed-log",
        ),
        pytest.param(
            "target = Path(args.g12_fault_soak_completion).parent / 'fault-soak.log'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late G-12 soak-log mutation\\n')",
            id="g12-fault-soak-log",
        ),
        pytest.param(
            "target = next(Path(args.release_root).parent.glob(\n"
            "    'output/sumeragi-v2-release/*/programs/*/release/iroha'\n"
            "))\n"
            "target.chmod(0o700)\n"
            "target.write_bytes(b'late prebuilt mutation\\n')",
            id="prebuilt-binary",
        ),
        pytest.param(
            "target = Path(args.formal_completion).parent / 'tlaps_resource.jsonl'\n"
            "target.chmod(0o600)\n"
            "target.write_bytes(b'late formal mutation\\n')",
            id="formal-tlaps-resource",
        ),
    ],
)
def test_protected_validator_cannot_mutate_nested_terminal_evidence(
    release_fixture: Fixture, mutation: str
) -> None:
    release_fixture.receipt_validator = _write(
        release_fixture.trust / "mutating-receipt-validator.py",
        _receipt_validator(mutation),
        0o500,
    )

    result = release_fixture.run()

    assert result.returncode == 2, result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert "changed" in result.stderr
    assert not release_fixture.evidence.exists()


def test_runner_failure_wins_over_post_validation_failure(
    release_fixture: Fixture,
) -> None:
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "fail-and-tamper",
        ),
    )
    result = release_fixture.run()
    assert result.returncode == 37, result.stderr
    assert "post-run bootstrap validation also failed" in result.stderr
    assert release_fixture.launch_count.read_text(encoding="utf-8") == "1\n"
    assert not release_fixture.evidence.exists()


def test_existing_evidence_is_never_overwritten(release_fixture: Fixture) -> None:
    release_fixture.evidence.mkdir(mode=0o700)
    sentinel = _write(release_fixture.evidence / "sentinel", b"keep", 0o400)
    result = release_fixture.run()
    assert result.returncode != 0
    assert sentinel.read_bytes() == b"keep"
    assert not release_fixture.launch_count.exists()


@pytest.mark.parametrize("mode", [0o755, 0o777])
def test_unsafe_evidence_parent_mode_never_launches(
    release_fixture: Fixture, mode: int
) -> None:
    release_fixture.root.chmod(mode)
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)


def test_legacy_runner_path_entry_never_launches(
    release_fixture: Fixture,
) -> None:
    unsafe = release_fixture.root / "unsafe:path"
    unsafe.mkdir(mode=0o700)
    result = release_fixture.run(
        [*release_fixture.arguments(), "--runner-path-entry", str(unsafe)]
    )
    _assert_never_launched(release_fixture, result)


def test_runner_tool_manifest_digest_mismatch_never_launches(
    release_fixture: Fixture,
) -> None:
    manifest = json.loads(release_fixture.tool_manifest.read_text(encoding="utf-8"))
    manifest["tools"]["chmod"]["sha256"] = "0" * 64
    _write(
        release_fixture.tool_manifest,
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)
    assert "protected sha-256" in result.stderr.lower()


def test_runner_tool_manifest_rejects_writable_source_ancestor(
    release_fixture: Fixture,
) -> None:
    writable_directory = release_fixture.root / "writable-tools"
    writable_directory.mkdir(mode=0o700)
    tool = _write(
        writable_directory / "chmod",
        "#!/bin/sh\nexit 0\n",
        0o500,
    )
    writable_directory.chmod(0o770)
    manifest = json.loads(release_fixture.tool_manifest.read_text(encoding="utf-8"))
    manifest["tools"]["chmod"] = {
        "path": str(tool),
        "sha256": _sha256(tool),
    }
    _write(
        release_fixture.tool_manifest,
        json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
        0o400,
    )

    result = release_fixture.run()

    _assert_never_launched(release_fixture, result)
    assert "writable, symlinked, or untrusted ancestor" in result.stderr.lower()
