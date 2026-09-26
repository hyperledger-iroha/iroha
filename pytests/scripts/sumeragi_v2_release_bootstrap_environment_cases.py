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


@pytest.mark.parametrize("name", SCALING_TRUST_ENV)
def test_retired_scaling_runner_environment_is_rejected(
    release_fixture: Fixture, name: str,
) -> None:
    result = release_fixture.run(
        [*release_fixture.arguments(), "--runner-environment", f"{name}=retired"]
    )
    _assert_never_launched(release_fixture, result)
    assert "explicitly allowed NAME=VALUE" in result.stderr


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
