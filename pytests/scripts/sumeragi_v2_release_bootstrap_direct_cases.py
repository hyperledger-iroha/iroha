# Executed lexically in sumeragi_v2_release_bootstrap_test.py; do not import directly.

def test_release_trust_inputs_are_the_only_new_runner_environment_names(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    preexisting_allowlist = {
        "CARGO_HOME",
        "CARGO_NET_GIT_FETCH_WITH_CLI",
        "CARGO_NET_OFFLINE",
        "NIX_SSL_CERT_FILE",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "SSL_CERT_FILE",
    }
    expected_release_environment = (
        set(RELEASE_CONTROL_ENV) | set(FORMAL_REPLAY_ENV)
    )
    assert (
        module._RUNNER_ENV_ALLOWLIST - preexisting_allowlist
        == expected_release_environment
    )
    assert module._RUNNER_ENV_ALLOWLIST == preexisting_allowlist | set(
        expected_release_environment
    )
    assert set(SCALING_TRUST_ENV).isdisjoint(module._RUNNER_ENV_ALLOWLIST)
    for name in SCALING_TRUST_ENV:
        with pytest.raises(module.BootstrapError):
            module._parse_runner_environment([f"{name}=retired"])

    names = tuple(path.name for path in BOOTSTRAP_COMPONENTS)
    assert module._BOOTSTRAP_COMPONENT_FILES == names
    assert set(module._BOOTSTRAP_COMPONENT_SHA256) == set(names)
    assert all(path.is_file() and not path.is_symlink() for path in BOOTSTRAP_COMPONENTS)
    assert {
        path.name: _sha256(path) for path in BOOTSTRAP_COMPONENTS
    } == module._BOOTSTRAP_COMPONENT_SHA256

    copied = tmp_path / BOOTSTRAP.name
    copied.write_text(
        BOOTSTRAP.read_text(encoding="utf-8").replace(
            next(iter(module._BOOTSTRAP_COMPONENT_SHA256.values())),
            "0" * 64,
            1,
        ),
        encoding="utf-8",
    )
    copied.chmod(0o500)
    for component in BOOTSTRAP_COMPONENTS:
        shutil.copy2(component, tmp_path / component.name)
    result = subprocess.run(
        [str(PYTHON), "-I", "-B", "-S", str(copied), "--help"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert result.returncode != 0
    assert b"bootstrap component binding is invalid" in result.stderr


def test_release_runner_waits_for_natural_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = _load_bootstrap_module()
    spawned: list[dict[str, object]] = []
    completed: list[bool] = []

    class FakeProcess:
        def __init__(self, _argv: object, **kwargs: object) -> None:
            spawned.append(kwargs)

        def wait(self) -> int:
            completed.append(True)
            return 23

    monkeypatch.setattr(module.subprocess, "Popen", FakeProcess)
    result = module._run_release_runner(
        tmp_path / "runner",
        (),
        cwd=tmp_path,
        environment={},
        stdout_descriptor=1,
        stderr_descriptor=2,
    )

    assert result.returncode == 23
    assert completed == [True]
    assert len(spawned) == 1
    assert "start_new_session" not in spawned[0]


def test_authenticated_sdk_source_manifest_pruning_is_exact(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    evidence = tmp_path / "evidence"
    evidence.mkdir(mode=0o700)
    manifest = _write(
        evidence / "sdk-dependency-bundle-manifest.json",
        b'{"schema_version":1}\n',
        0o400,
    )
    snapshot = module._read_file(
        manifest,
        "SDK source manifest fixture",
        maximum_bytes=module._MAX_SDK_MANIFEST_BYTES,
    )
    evidence_fd = os.open(evidence, os.O_RDONLY | os.O_DIRECTORY)
    try:
        module._prune_authenticated_sdk_source_manifest(evidence_fd, snapshot)
        module._require_sdk_source_manifest_pruned(evidence_fd, snapshot)
        assert not os.path.lexists(manifest)

        _write(manifest, snapshot.data, snapshot.mode)
        with pytest.raises(
            module.BootstrapError,
            match="survived acknowledgment pruning",
        ):
            module._require_sdk_source_manifest_pruned(evidence_fd, snapshot)
    finally:
        os.close(evidence_fd)


@pytest.mark.parametrize(
    ("timeout_seconds", "maximum_output_bytes", "program", "message"),
    [
        (
            1,
            1024,
            "import time; time.sleep(1.2)",
            "bounded runtime",
        ),
        (
            5,
            32,
            "import sys; "
            "sys.stdout.buffer.write(b'O' * 131072); sys.stdout.flush(); "
            "sys.stderr.buffer.write(b'E' * 131072); sys.stderr.flush()",
            "bounded output limit",
        ),
    ],
)
def test_bounded_helper_finishes_naturally_before_reporting_latched_violation(
    tmp_path: Path,
    timeout_seconds: int,
    maximum_output_bytes: int,
    program: str,
    message: str,
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "natural-completion"
    child = (
        f"{program}; from pathlib import Path; "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )

    with pytest.raises(module.BootstrapError, match=message):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=timeout_seconds,
            maximum_output_bytes=maximum_output_bytes,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def test_bounded_helper_drains_inherited_pipes_until_descendant_finishes(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "descendant-natural-completion"
    descendant = (
        "import time; from pathlib import Path; time.sleep(1.2); "
        f"Path({str(sentinel)!r}).write_text('complete', encoding='utf-8')"
    )
    child = (
        "import subprocess; "
        f"subprocess.Popen([{str(PYTHON)!r}, '-I', '-S', '-c', {descendant!r}])"
    )

    with pytest.raises(module.BootstrapError, match="bounded runtime"):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", child),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=1,
            maximum_output_bytes=1024,
        )

    assert sentinel.read_text(encoding="utf-8") == "complete"


def test_bounded_helper_rejects_nonpositive_timeout_before_spawn(
    tmp_path: Path,
) -> None:
    module = _load_bootstrap_module()
    sentinel = tmp_path / "should-not-be-created"
    with pytest.raises(module.BootstrapError, match="protected command bounds are invalid"):
        module._run_bounded(
            PYTHON,
            ("-I", "-S", "-c", f"open({str(sentinel)!r}, 'w').close()"),
            cwd=tmp_path,
            environment={"PATH": os.defpath},
            timeout_seconds=0,
            maximum_output_bytes=1024,
        )
    assert not sentinel.exists()


def test_scaling_fixture_arguments_bind_canonical_protected_sources(
    release_fixture: Fixture,
) -> None:
    fixture_root = REPO_ROOT / "pytests/scripts/scaling_preflight/fixtures"
    assert release_fixture.scaling_plan.read_bytes() == (fixture_root / "plan.json").read_bytes()
    assert release_fixture.scaling_budget.read_bytes() == (fixture_root / "budget.json").read_bytes()
    assert release_fixture.scaling_handoff_helper.read_bytes() == (
        REPO_ROOT / "scripts/sumeragi_v2_release_scaling_handoff.py"
    ).read_bytes()
    arguments = release_fixture.arguments()
    for flag, path in (
        ("--scaling-plan", release_fixture.scaling_plan),
        ("--scaling-budget", release_fixture.scaling_budget),
        ("--scaling-handoff-helper", release_fixture.scaling_handoff_helper),
    ):
        assert arguments[arguments.index(flag) + 1] == str(path)
        digest_flag = "--expected" + flag[1:] + "-sha256"
        assert arguments[arguments.index(digest_flag) + 1] == _sha256(path)
    assert arguments[arguments.index("--scaling-dependency-source") + 1] == str(
        release_fixture.scaling_dependencies
    )
    assert release_fixture.scaling_dependencies.is_dir()
    assert release_fixture.cargo_home.is_dir()
    assert f"CARGO_HOME={release_fixture.cargo_home}" in arguments


def test_production_runner_fixture_rebinds_approvals_to_actual_handoff_source(
    release_fixture: Fixture,
) -> None:
    before = {name: _sha256(path) for name, path in release_fixture.approvals.items()}
    release_fixture.install_production_runner()
    runner = release_fixture.candidate / "scripts/run_sumeragi_v2_release_gates.sh"
    source = REPO_ROOT / "scripts/run_sumeragi_v2_release_gates.sh"
    assert runner.read_bytes() == source.read_bytes()
    assert b'"$release_python_bin" -I -B -S "$release_scaling_handoff_helper"' in runner.read_bytes()
    assert b'--gate-fd "$release_gate_fd"' in runner.read_bytes()
    assert before != {
        name: _sha256(path) for name, path in release_fixture.approvals.items()
    }


@pytest.mark.parametrize(
    ("name", "label"),
    (
        ("plan", "scaling plan"),
        ("budget", "scaling budget"),
        ("handoff-helper", "scaling handoff helper"),
    ),
)
def test_scaling_protected_input_digest_mismatch_rejects_before_runner(
    release_fixture: Fixture, name: str, label: str,
) -> None:
    arguments = _replace_flag(
        release_fixture.arguments(), f"--expected-scaling-{name}-sha256", "0" * 64
    )
    result = release_fixture.run(arguments)
    assert result.returncode != 0
    assert label in result.stderr
    assert not release_fixture.launch_count.exists()
    assert not release_fixture.evidence.exists()
