# Executed lexically in sumeragi_v2_release_bootstrap_test.py; do not import directly.

def test_nonrelocatable_protected_bash_never_launches(
    release_fixture: Fixture,
) -> None:
    protected = _write(
        release_fixture.trust / "protected-shell",
        "#!/bin/sh\nexit 19\n",
        0o500,
    )
    arguments = _replace_flag(
        release_fixture.arguments(), "--bash-bin", str(protected)
    )
    arguments = _replace_flag(
        arguments, "--expected-bash-sha256", _sha256(protected)
    )
    result = release_fixture.run(arguments)
    _assert_never_launched(release_fixture, result)


def test_unapproved_runner_environment_is_rejected(release_fixture: Fixture) -> None:
    result = release_fixture.run(
        [*release_fixture.arguments(), "--runner-environment", "BASH_ENV=/tmp/attack"]
    )
    _assert_never_launched(release_fixture, result)


def test_scaling_evidence_runner_environment_is_authenticated_and_forwarded(
    release_fixture: Fixture,
) -> None:
    scaling_manifest = (
        release_fixture.retained_root
        / "output"
        / "scaling"
        / "scaling_evidence.json"
    )
    observed_environment = release_fixture.root / "observed-scaling-environment"
    release_fixture.install_planned_runner(
        _runner(
            release_fixture.launch_count,
            release_fixture.candidate,
            "success",
            observed_scaling_environment=observed_environment,
        ),
    )

    scaling_environment = {
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA256": "a" * 64,
        SCALING_EVIDENCE_ENV: str(scaling_manifest),
        "IROHA_RELEASE_SCALING_IROHAD_SHA256": "b" * 64,
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256": "c" * 64,
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_SHA256": "d" * 64,
    }
    arguments = [*release_fixture.arguments()]
    for name in SCALING_TRUST_ENV:
        arguments = _replace_runner_environment(
            arguments, name, scaling_environment[name]
        )
    arguments = _replace_flag(arguments, "--command-timeout-seconds", "20")
    result = release_fixture.run(arguments)

    assert result.returncode == 0, result.stderr
    assert dict(
        line.split("=", 1)
        for line in observed_environment.read_text(encoding="utf-8").splitlines()
    ) == scaling_environment
    marker = json.loads(
        (release_fixture.evidence / "BOOTSTRAP_COMPLETED.json").read_text(
            encoding="utf-8"
        )
    )
    assert "environment_without_self_digest" not in marker["runner"]
    assert re.fullmatch(
        r"[0-9a-f]{64}", marker["runner"]["environment_sha256"]
    )


@pytest.mark.parametrize(
    "name",
    [
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST_",
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST_PATH",
        "IROHA_RELEASE_SCALING_IROHAD_SHA256_PATH",
        "IROHA_RELEASE_SCALING_IROHA_CLI_SHA256_",
        "IROHA_RELEASE_SCALING_TRIAL_HARNESS_DIGEST",
        "IROHA_RELEASE_SCALING_CONFIGURATION_SHA512",
        "SUMERAGI_V2_RELEASE_SCALING_EVIDENCE_MANIFEST",
    ],
)
def test_scaling_evidence_runner_environment_lookalikes_are_rejected(
    release_fixture: Fixture,
    name: str,
) -> None:
    result = release_fixture.run(
        [
            *release_fixture.arguments(),
            "--runner-environment",
            f"{name}=/tmp/scaling_evidence.json",
        ]
    )

    _assert_never_launched(release_fixture, result)
    assert "explicitly allowed NAME=VALUE" in result.stderr


def test_candidate_runner_symlink_never_launches(release_fixture: Fixture) -> None:
    runner = release_fixture.candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"
    target = release_fixture.root / "outside-runner"
    shutil.move(runner, target)
    runner.symlink_to(target)
    result = release_fixture.run()
    _assert_never_launched(release_fixture, result)
