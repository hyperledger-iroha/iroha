"""Security contract for the protected Kagemusha V4 promotion workflow."""

import ast
import hashlib
from pathlib import Path
import re
import subprocess
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
PROMOTION_WORKFLOW = ROOT / ".github/workflows/promote_kagemusha_v4.yml"
READINESS_GATE = ROOT / "ci/check_kagemusha_production_readiness.sh"
READINESS_SOURCE_CONTRACT = (
    ROOT / "ci/check_kagemusha_production_readiness_source_contract.py"
)
KAGEMUSHA_RUNBOOK = ROOT / "specs/offline_kagemusha.md"
KAGEMUSHA_KAGAMI = ROOT / "crates/iroha_kagami/src/kagemusha.rs"
NATIVE_PYTHON_LAUNCHER = (
    ROOT
    / "crates/iroha_kagami/src/bin/iroha_authenticated_tool_controller"
    / "kagemusha_python_launcher.rs"
)
FIXED_CONTROLLER_PATH = (
    "/Library/SORA/Kagemusha/bin/iroha_authenticated_tool_controller"
)
EXPECTED_REPOSITORY = "hyperledger-iroha/iroha"
EXPECTED_WORKFLOW_PATH = ".github/workflows/promote_kagemusha_v4.yml"
PROMOTION_ID_DOMAIN = "iroha.kagemusha.github-promotion-run.v1"
CATALOG_REVALIDATION_RECEIPT_ROOT = "/Library/SORA/Kagemusha/catalog-revalidation"


def _job_block(workflow: str, name: str) -> str:
    """Return one exact top-level GitHub Actions job block."""

    jobs = workflow.split("\njobs:\n", 1)
    if len(jobs) != 2:
        return ""
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
        jobs[1],
    )
    return "" if match is None else match.group(0)


def _named_step_block(job: str, name: str) -> str:
    """Return one named step rather than matching markers in another step."""

    match = re.search(
        rf"(?ms)^      - name: {re.escape(name)}\n.*?"
        r"(?=^      - (?:name|uses): |\Z)",
        job,
    )
    return "" if match is None else match.group(0)


def _run_script(step: str) -> str:
    """Decode the literal run block of one workflow step."""

    marker = "        run: |\n"
    if step.count(marker) != 1:
        return ""
    body = step.split(marker, 1)[1]
    lines: list[str] = []
    for line in body.splitlines():
        if not line.startswith("          "):
            break
        lines.append(line[10:])
    return "\n".join(lines) + ("\n" if lines else "")


def _command_substitution(script: str, variable: str) -> str:
    """Return the body of one multiline shell assignment command substitution."""

    match = re.search(
        rf'(?ms)^{re.escape(variable)}="\$\(\n(.*?)^\)"$',
        script,
    )
    return "" if match is None else match.group(1)


def _shell_control_stack_before(script: str, target_line: str) -> list[str]:
    """Track structured shell blocks and return those enclosing a target command."""

    stack: list[str] = []
    for raw_line in script.splitlines():
        line = raw_line.strip()
        if raw_line == target_line:
            return stack
        if not line or line.startswith("#"):
            continue
        if re.match(r"^[A-Za-z_][A-Za-z0-9_]*\(\) \{$", line):
            stack.append("function")
            continue
        if re.match(r"^if\b", line):
            stack.append("if")
            continue
        if re.match(r"^(?:for|while|until)\b", line):
            stack.append("loop")
            continue
        if re.match(r"^case\b", line):
            stack.append("case")
            continue
        if line.endswith("|| {") or line == "{":
            stack.append("brace")
            continue
        if line == "fi":
            if not stack or stack.pop() != "if":
                return ["malformed"]
            continue
        if line == "done":
            if not stack or stack.pop() != "loop":
                return ["malformed"]
            continue
        if line == "esac":
            if not stack or stack.pop() != "case":
                return ["malformed"]
            continue
        if line in {"}", "};"}:
            if not stack or stack.pop() not in {"brace", "function"}:
                return ["malformed"]
    return ["target-missing"]


def _executable_gate_trace(script: str) -> int:
    """Execute Bash control flow while suppressing every protected side effect.

    Bash's ``extdebug`` mode lets a nonzero DEBUG trap skip the next simple
    command. Successful explicit termination and ``false`` execute normally so
    an early exit or a false enclosing branch still prevents the launch trace.
    """

    harness = f"""\
shopt -s extdebug
__kagemusha_trace_command() {{
  case "$BASH_COMMAND" in
    *launch-kagemusha-readiness-v1*)
      /usr/bin/printf '%s\\n' kagemusha-native-gate-reached
      return 1
      ;;
    'exit'|'exit 0'|'return'|'return 0'|false|'/usr/bin/false')
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}}
__kagemusha_trace_script() {{
  trap __kagemusha_trace_command DEBUG
{script}
}}
__kagemusha_trace_script
"""
    completed = subprocess.run(
        ["/bin/bash", "--noprofile", "--norc"],
        input=harness,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    return completed.stdout.splitlines().count("kagemusha-native-gate-reached")


def _has_potential_successful_termination(script: str) -> bool:
    """Reject a pre-gate exit/return unless it is an explicit failure status."""

    commands = re.compile(
        r"(?:^|[;&|]\s*)"
        r"(?:(?:builtin|command)\s+)?(?:exit|return)"
        r"(?:\s+([^;&|#]+?))?\s*(?=;|&&|\|\||#|$)",
        re.MULTILINE,
    )
    for match in commands.finditer(script):
        status = (match.group(1) or "").strip()
        if not status.isdecimal() or not 1 <= int(status, 10) <= 255:
            return True
    return False


def _embedded_gate_python(gate: str) -> str:
    """Return the exact Python program embedded in the readiness gate."""

    return gate.split("<<'PY'\n", 1)[1].rsplit("\nPY\n", 1)[0]


def _load_embedded_function(gate: str, name: str) -> object:
    """Compile one embedded gate function for focused executable tests."""

    embedded = _embedded_gate_python(gate)
    parsed = ast.parse(embedded)
    function = next(
        node
        for node in parsed.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
        and node.name == name
    )
    source = ast.get_source_segment(embedded, function)
    assert source is not None
    namespace: dict[str, object] = {"Path": Path}
    exec("from __future__ import annotations\n" + source, namespace)
    return namespace[name]


def _validate_promotion_workflow(workflow: str) -> list[str]:
    """Return deterministic errors for weakened protected-promotion semantics."""

    errors: list[str] = []
    if not workflow.startswith(
        "name: Authorize Kagemusha V4 mobile production builds\n"
    ):
        errors.append("workflow must disclose its mobile authorization boundary")
    production = _job_block(workflow, "production-promotion")
    if not production:
        return ["protected production-promotion job is missing"]
    if "name: Qualify and authorize the Apple production build" not in production:
        errors.append("protected Apple authorization job is mislabeled")

    checkout = _named_step_block(
        production, "Check out the exact workflow commit for identity binding"
    )
    protected = _named_step_block(
        production, "Install, qualify, and run the protected readiness gate"
    )
    if not checkout:
        errors.append("protected job must check out its exact workflow commit")
    else:
        for marker in (
            "uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
            "ref: ${{ github.workflow_sha }}",
            "persist-credentials: false",
        ):
            if marker not in checkout:
                errors.append(f"protected checkout identity is incomplete: {marker}")
    if not protected:
        return errors + ["protected gate step is missing"]

    fixed_environment = (
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN: "
        f"{FIXED_CONTROLLER_PATH}"
    )
    if fixed_environment not in protected or (
        "vars.KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN" in protected
    ):
        errors.append("controller destination must be the fixed reviewed path")

    identity_markers = (
        "PROMOTION_GITHUB_EVENT_NAME: ${{ github.event_name }}",
        "PROMOTION_GITHUB_REF: ${{ github.ref }}",
        "PROMOTION_GITHUB_REF_PROTECTED: ${{ github.ref_protected }}",
        "PROMOTION_GITHUB_REPOSITORY: ${{ github.repository }}",
        "PROMOTION_GITHUB_RUN_ATTEMPT: ${{ github.run_attempt }}",
        "PROMOTION_GITHUB_RUN_ID: ${{ github.run_id }}",
        "PROMOTION_GITHUB_SHA: ${{ github.sha }}",
        "PROMOTION_GITHUB_WORKFLOW_REF: ${{ github.workflow_ref }}",
        "PROMOTION_GITHUB_WORKFLOW_SHA: ${{ github.workflow_sha }}",
        "PROMOTION_GITHUB_WORKSPACE: ${{ github.workspace }}",
    )
    for marker in identity_markers:
        if marker not in protected:
            errors.append(f"GitHub run identity is incomplete: {marker}")

    script = _run_script(protected)
    if not script:
        return errors + ["protected gate script is not one literal run block"]
    if re.search(r"(?<![/A-Za-z0-9_])sudo(?=\s)", script):
        errors.append("privileged commands must use exact /usr/bin/sudo")
    safe_directory = '-c safe.directory="$reviewed_checkout"'
    reviewed_head = _command_substitution(script, "reviewed_checkout_head")
    workflow_head = _command_substitution(script, "workflow_checkout_head")
    if (
        script.count(safe_directory) != 1
        or safe_directory not in reviewed_head
        or safe_directory in workflow_head
        or "safe.directory=*" in script
    ):
        errors.append(
            "reviewed checkout must use one exact command-scoped safe.directory"
        )
    syntax = subprocess.run(
        ["/bin/bash", "-n"],
        input=script,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if syntax.returncode != 0:
        errors.append(f"protected gate script is not valid Bash: {syntax.stderr.strip()}")

    required_script_markers = (
        'test "$PROMOTION_GITHUB_EVENT_NAME" = workflow_dispatch',
        f'test "$PROMOTION_GITHUB_REPOSITORY" = {EXPECTED_REPOSITORY}',
        'test "$PROMOTION_GITHUB_REF_PROTECTED" = true',
        f'"{EXPECTED_REPOSITORY}/{EXPECTED_WORKFLOW_PATH}@$PROMOTION_GITHUB_REF"',
        '[[ "$PROMOTION_GITHUB_RUN_ID" =~ ^[1-9][0-9]*$ ]]',
        '[[ "$PROMOTION_GITHUB_RUN_ATTEMPT" =~ ^[1-9][0-9]*$ ]]',
        'test "$PROMOTION_GITHUB_WORKFLOW_SHA" = "$PROMOTION_GITHUB_SHA"',
        'test "$workflow_checkout_head" = "$PROMOTION_GITHUB_SHA"',
        'rev-parse --verify \'HEAD^{commit}\'',
        'test "$reviewed_checkout_head" = "$PROMOTION_GITHUB_SHA"',
        '-c safe.directory="$reviewed_checkout"',
        f'readonly promotion_identity_domain="{PROMOTION_ID_DOMAIN}"',
        (
            'KAGEMUSHA_V4_PROMOTION_ID="$(\n'
            "  /usr/bin/printf '%s\\0%s\\0%s\\0%s\\0%s\\0%s\\0' \\\n"
            '    "$promotion_identity_domain" \\\n'
            '    "$PROMOTION_GITHUB_REPOSITORY" \\\n'
            '    "$PROMOTION_GITHUB_WORKFLOW_REF" \\\n'
            '    "$PROMOTION_GITHUB_WORKFLOW_SHA" \\\n'
            '    "$PROMOTION_GITHUB_RUN_ID" \\\n'
            '    "$PROMOTION_GITHUB_RUN_ATTEMPT" \\\n'
            "    | /usr/bin/shasum -a 256 | /usr/bin/awk '{print $1}'\n"
            ')"'
        ),
        '[[ "$KAGEMUSHA_V4_PROMOTION_ID" =~ ^[0-9a-f]{64}$ ]]',
        'readonly KAGEMUSHA_V4_PROMOTION_ID',
        f'readonly catalog_revalidation_receipt_root="{CATALOG_REVALIDATION_RECEIPT_ROOT}"',
        (
            'KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT='
            '"$catalog_revalidation_receipt_root/$KAGEMUSHA_V4_PROMOTION_ID.json"'
        ),
        'readonly controller_parent="/Library/SORA/Kagemusha/bin"',
        'readonly controller_name="iroha_authenticated_tool_controller"',
        (
            'test "$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN" = \\\n'
            '  "$controller_install_path"'
        ),
    )
    for marker in required_script_markers:
        if marker not in script:
            errors.append(f"protected gate binding is missing: {marker}")

    gate_launch_line = "/usr/bin/sudo -n /usr/bin/env -i \\\n"
    launch_offsets = [
        match.start()
        for match in re.finditer(re.escape(gate_launch_line), script)
        if "launch-kagemusha-readiness-v1" in script[match.start() :]
    ]
    if len(launch_offsets) != 1:
        errors.append("protected gate must have one top-level native launch")
    else:
        launch_offset = launch_offsets[0]
        # The earlier qualification command is embedded in an assignment. The gate
        # launch is the sole env-clear command beginning at column zero.
        if script[launch_offset :].count("launch-kagemusha-readiness-v1") != 1:
            errors.append("protected gate native launch is not unique")
        if _shell_control_stack_before(script, gate_launch_line.rstrip("\n")):
            errors.append("protected gate native launch is conditionally unreachable")
        prefix = script[:launch_offset]
        if _has_potential_successful_termination(prefix):
            errors.append("successful shell termination precedes the protected gate")
        if re.search(r"(?m)^\s*exec\s+(?:/usr/bin/)?(?:true|:)(?:\s|$)", prefix):
            errors.append("successful exec termination precedes the protected gate")
        final_argument = (
            '--python-sha256 "$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256"\n'
        )
        if not script.endswith(final_argument):
            errors.append("protected gate launch must be the terminal shell command")
    return errors


def test_production_job_is_distinct_from_the_untrusted_controller_build() -> None:
    """Production inputs must never enter the job that builds handoff bytes."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    build, promotion = source.split("  production-promotion:\n", 1)
    assert "environment:" not in build
    assert "kagemusha-untrusted-build" in build
    assert "environment: kagemusha-v4-production" in promotion
    assert "name: Qualify and authorize the Apple production build" in promotion
    assert "needs: controller-build" in promotion
    assert "kagemusha-production" in promotion
    assert "cancel-in-progress: false" in source


def test_mobile_authorizations_use_distinct_protected_platform_lanes() -> None:
    """Apple evidence must never stand in for Android qualification."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    apple = _job_block(source, "production-promotion")
    android = _job_block(source, "android-production-qualification")
    assert apple and android
    assert "environment: kagemusha-v4-production" in apple
    assert "environment: kagemusha-v4-android-production" in android
    assert "needs: production-promotion" in android
    assert "--platform apple" in apple
    assert "--platform android" not in apple
    assert "--platform android" in android
    assert "--platform apple" not in android
    assert "check_android_device_lab_slot.py" in android
    assert "--require-kagemusha-production-evidence" in android
    assert "--require-kagemusha-standard-matrix" in android
    assert source.count("uses: actions/attest@") == 2
    assert "kagemusha-mobile-apple-production-authorization-" in apple
    assert "kagemusha-mobile-android-production-authorization-" in android
    assert "mobile_release_manifest_sha256" in source
    assert "--artifact-manifest-sha256" in apple
    assert "--artifact-manifest-sha256" in android
    assert "--release-verification-report" in apple
    assert "--release-verification-report" in android


def test_protected_workflow_semantics_are_fail_closed() -> None:
    """The reviewed workflow must satisfy its parsed job and shell control-flow contract."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    assert _validate_promotion_workflow(source) == []


def test_executable_control_flow_reaches_the_native_gate() -> None:
    """A side-effect-free Bash trace must reach the real top-level launch."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    protected = _named_step_block(
        _job_block(source, "production-promotion"),
        "Install, qualify, and run the protected readiness gate",
    )
    script = _run_script(protected)
    assert _executable_gate_trace(script) == 1

    launch = "/usr/bin/sudo -n /usr/bin/env -i \\\n"
    assert script.count(launch) == 1
    assert _executable_gate_trace(script.replace(launch, "exit 0\n" + launch, 1)) == 0

    terminal = (
        '--python-sha256 "$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256"\n'
    )
    assert script.count(terminal) == 1
    unreachable = script.replace(launch, "if false; then\n" + launch, 1).replace(
        terminal, terminal + "fi\n", 1
    )
    assert _executable_gate_trace(unreachable) == 0


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "name: Authorize Kagemusha V4 mobile production builds",
            "name: Publish unauthenticated mobile builds",
            "must disclose its mobile authorization boundary",
        ),
        (
            "          /usr/bin/sudo -n /bin/test ! -d ",
            "          sudo -n /bin/test ! -d ",
            "privileged commands must use exact /usr/bin/sudo",
        ),
        (
            '              /usr/bin/git -c safe.directory="$reviewed_checkout" \\',
            '              /usr/bin/git -c safe.directory="*" \\',
            "reviewed checkout must use one exact command-scoped safe.directory",
        ),
        (
            "          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "          exit 0\n          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "successful shell termination precedes the protected gate",
        ),
        (
            "          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "          command exit \"$((0))\"\n          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "successful shell termination precedes the protected gate",
        ),
        (
            "          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "          exit 256\n          /usr/bin/sudo -n /usr/bin/env -i \\\n",
            "successful shell termination precedes the protected gate",
        ),
        (
            "          ref: ${{ github.workflow_sha }}",
            "          ref: ${{ github.sha }}",
            "protected checkout identity is incomplete",
        ),
        (
            "          PROMOTION_GITHUB_WORKFLOW_SHA: ${{ github.workflow_sha }}",
            "          PROMOTION_GITHUB_WORKFLOW_SHA: ${{ vars.PROMOTION_WORKFLOW_SHA }}",
            "GitHub run identity is incomplete",
        ),
        (
            "          PROMOTION_GITHUB_RUN_ID: ${{ github.run_id }}",
            "          PROMOTION_GITHUB_RUN_ID: ${{ vars.PROMOTION_RUN_ID }}",
            "GitHub run identity is incomplete",
        ),
        (
            "          PROMOTION_GITHUB_WORKSPACE: ${{ github.workspace }}",
            "          PROMOTION_GITHUB_WORKSPACE: ${{ vars.PROMOTION_WORKSPACE }}",
            "GitHub run identity is incomplete",
        ),
        (
            '          test "$PROMOTION_GITHUB_WORKFLOW_SHA" = "$PROMOTION_GITHUB_SHA"',
            '          test "$PROMOTION_GITHUB_WORKFLOW_SHA" = "$PROMOTION_GITHUB_WORKFLOW_SHA"',
            "protected gate binding is missing",
        ),
        (
            '          test "$reviewed_checkout_head" = "$PROMOTION_GITHUB_SHA"',
            '          test "$reviewed_checkout_head" = "$reviewed_checkout_head"',
            "protected gate binding is missing",
        ),
        (
            f"          KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN: {FIXED_CONTROLLER_PATH}",
            (
                "          KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN: "
                "${{ vars.KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN }}"
            ),
            "controller destination must be the fixed reviewed path",
        ),
        (
            '          readonly controller_parent="/Library/SORA/Kagemusha/bin"',
            '          readonly controller_parent="/private/tmp"',
            "protected gate binding is missing",
        ),
        (
            (
                "          readonly catalog_revalidation_receipt_root="
                f'"{CATALOG_REVALIDATION_RECEIPT_ROOT}"'
            ),
            '          readonly catalog_revalidation_receipt_root="/private/tmp"',
            "protected gate binding is missing",
        ),
        (
            '              "$PROMOTION_GITHUB_RUN_ATTEMPT" \\',
            '              "$PROMOTION_GITHUB_RUN_ID" \\',
            "protected gate binding is missing",
        ),
        (
            '              "$PROMOTION_GITHUB_WORKFLOW_SHA" \\',
            '              "$PROMOTION_GITHUB_SHA" \\',
            "protected gate binding is missing",
        ),
    ),
)
def test_protected_workflow_rejects_identity_and_control_flow_mutations(
    old: str, new: str, expected_error: str
) -> None:
    """Hostile source mutations must be detected before protected execution."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    assert old in source
    errors = _validate_promotion_workflow(source.replace(old, new, 1))
    assert any(expected_error in error for error in errors), errors


def test_protected_workflow_rejects_a_conditionally_unreachable_gate() -> None:
    """A syntactically present launch inside a false branch is not a gate."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    launch = "          /usr/bin/sudo -n /usr/bin/env -i \\\n"
    terminal = (
        "              --python-sha256 "
        '"$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256"\n'
    )
    assert source.count(launch) == 1
    assert source.count(terminal) == 1
    mutated = source.replace(
        launch, "          if false; then\n" + launch, 1
    ).replace(terminal, terminal + "          fi\n", 1)
    errors = _validate_promotion_workflow(mutated)
    assert any("conditionally unreachable" in error for error in errors), errors


def test_poisoned_path_cannot_select_the_workflow_privilege_boundary(
    tmp_path: Path,
) -> None:
    """Executable shell resolution must distinguish ambient from exact sudo."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    protected = _named_step_block(
        _job_block(source, "production-promotion"),
        "Install, qualify, and run the protected readiness gate",
    )
    script = _run_script(protected)
    sudo_tokens = re.findall(r"(?m)(?<!\S)(?:/usr/bin/)?sudo(?=\s+-n)", script)
    assert sudo_tokens
    assert set(sudo_tokens) == {"/usr/bin/sudo"}

    poison = tmp_path / "poison"
    poison.mkdir()
    fake_sudo = poison / "sudo"
    fake_sudo.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
    fake_sudo.chmod(0o755)
    completed = subprocess.run(
        ["/bin/bash", "--noprofile", "--norc"],
        input=(
            'test "$(command -v sudo)" = "$POISON/sudo"\n'
            'test "$(command -v /usr/bin/sudo)" = /usr/bin/sudo\n'
        ),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env={"PATH": f"{poison}:/usr/bin:/bin", "POISON": str(poison)},
        check=False,
    )
    assert completed.returncode == 0, completed.stderr


def test_exact_reviewed_checkout_safe_directory_handles_different_owner(
    tmp_path: Path,
) -> None:
    """The reviewed root checkout is trusted narrowly, never by wildcard."""

    repository = tmp_path / "reviewed"
    initialized = subprocess.run(
        ["/usr/bin/git", "init", "--quiet", str(repository)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    assert initialized.returncode == 0, initialized.stderr
    environment = {
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_TEST_ASSUME_DIFFERENT_OWNER": "1",
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    denied = subprocess.run(
        ["/usr/bin/git", "-C", str(repository), "rev-parse", "--show-toplevel"],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
        check=False,
    )
    assert denied.returncode == 128
    assert "dubious ownership" in denied.stderr

    allowed = subprocess.run(
        [
            "/usr/bin/git",
            "-c",
            f"safe.directory={repository}",
            "-C",
            str(repository),
            "rev-parse",
            "--show-toplevel",
        ],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=environment,
        check=False,
    )
    assert allowed.returncode == 0, allowed.stderr
    assert allowed.stdout.strip() == str(repository)

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    protected = _run_script(
        _named_step_block(
            _job_block(source, "production-promotion"),
            "Install, qualify, and run the protected readiness gate",
        )
    )
    workflow_head = _command_substitution(protected, "workflow_checkout_head")
    reviewed_head = _command_substitution(protected, "reviewed_checkout_head")
    assert "safe.directory" not in workflow_head
    assert '-c safe.directory="$reviewed_checkout"' in reviewed_head
    assert "safe.directory=*" not in protected


def test_runbook_carries_native_runtime_and_sealed_report_inputs() -> None:
    """Both operator examples must show the five native-build pins previously omitted."""

    runbook = KAGEMUSHA_RUNBOOK.read_text(encoding="utf-8")
    for name in (
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD",
        "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH",
        "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256",
    ):
        assert runbook.count(f"{name}=") == 2
    assert runbook.count(
        f"KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN={FIXED_CONTROLLER_PATH}"
    ) == 2
    assert "separate protected Apple and Android" in runbook
    assert "Mobile build authorization is not\nprotocol activation" in runbook
    assert "publisher independently verifies both receipts" in runbook
    assert PROMOTION_ID_DOMAIN in runbook
    assert f"{CATALOG_REVALIDATION_RECEIPT_ROOT}/<promotion-id>.json" in runbook
    assert re.search(
        r"After dispatch\s+and before approving the\s+protected environment", runbook
    )
    assert "invokes `revalidate-catalog`" in runbook
    assert "exact seventeen-file pre-promotion candidate" in runbook
    assert "full exact\neighteen-file promoted-release verifier" in runbook
    assert "The workflow\ndoes not publish the Kagemusha promotion record" in runbook
    assert "`promote-kagemusha-release-v4` subcommand" in runbook
    assert "generic writable-file mode pre-creates outputs" in runbook


def test_kagami_promotion_source_is_verify_publish_verify() -> None:
    """The local publisher must reach both exact-inventory verification states."""

    source = KAGEMUSHA_KAGAMI.read_text(encoding="utf-8")
    helper = source.split("fn verify_publish_verify_release_v4", 1)[1].split(
        "\nfn validate_artifacts_sequentially", 1
    )[0]
    candidate = helper.index("verify(ReleaseInventoryStateV4::Candidate)?")
    publish = helper.index("publish(&candidate)?")
    promoted = helper.index("verify(ReleaseInventoryStateV4::Promoted)")
    assert candidate < publish < promoted
    assert "Self::Candidate => 17" in source
    assert "Self::Promoted => 18" in source
    assert "supplied.as_os_str() != expected.as_os_str()" in source


def test_production_job_installs_and_qualifies_the_exact_controller_image() -> None:
    """The protected job must byte-pin, root-install, and attack its controller."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    promotion = source.split("  production-promotion:\n", 1)[1]
    install = promotion.index(
        "/usr/bin/install -o root -g wheel -m 0555 \"$controller_source\""
    )
    publish = promotion.index(
        "/bin/mv -f -- \"$controller_install_candidate\" "
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"'
    )
    post_digest = promotion.index(
        'shasum -a 256 "$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN"',
        publish,
    )
    qualify = promotion.index(
        '"$KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN" qualify-host-v1',
        post_digest,
    )
    gate_snapshot = promotion.index(
        'gate_snapshot="$gate_launch_dir/check_kagemusha_production_readiness.sh"',
        qualify,
    )
    gate = promotion.index("launch-kagemusha-readiness-v1", gate_snapshot)
    assert install < publish < post_digest < qualify < gate
    assert gate_snapshot < gate
    native_call = promotion[gate:]
    assert '--gate-snapshot "$gate_snapshot"' in native_call
    assert '--gate-source "$KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH"' in native_call
    assert (
        '--expected-macos-build '
        '"$KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD"'
        in native_call
    )
    assert (
        '--python-runtime-tree-sha256 '
        '"$KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256"'
        in native_call
    )
    assert "exec /bin/bash /dev/fd/10 promotion" not in promotion
    assert 'controller_uid" != 0' in promotion
    assert 'controller_gid" != 0' in promotion
    assert 'controller_mode" != 555' in promotion
    assert "controller handoff inventory is not exact" in promotion
    assert "require_root_custodied_directory_chain" in promotion
    assert "/bin/ls -lde" in promotion
    assert "/usr/bin/xattr" in promotion
    assert "/usr/bin/python3" not in promotion


@pytest.mark.skipif(
    not Path("/usr/bin/shasum").is_file(),
    reason="protected macOS launcher requires /usr/bin/shasum",
)
def test_gate_hash_and_execution_use_independent_authenticated_descriptors() -> None:
    """A pathname replacement cannot substitute bytes after all FDs are open."""

    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        gate = root / "gate.sh"
        hostile = root / "hostile.sh"
        gate.write_text(
            "#!/bin/bash\n"
            "set -euo pipefail\n"
            'test "${BASH_SOURCE[0]}" = /dev/fd/10\n'
            'observed="$(/usr/bin/shasum -a 256 /dev/fd/8 | '
            "/usr/bin/awk '{print $1}')\"\n"
            'test "$observed" = "$1"\n'
            "echo trusted-gate\n",
            encoding="utf-8",
        )
        hostile.write_text("#!/bin/bash\necho hostile-gate\n", encoding="utf-8")
        digest = hashlib.sha256(gate.read_bytes()).hexdigest()
        launcher = r'''
set -euo pipefail
gate="$1"
hostile="$2"
expected="$3"
exec 8<"$gate"
exec 9<"$gate"
exec 10<"$gate"
observed="$(/usr/bin/shasum -a 256 <&9 | /usr/bin/awk '{print $1}')"
exec 9<&-
test "$observed" = "$expected"
/bin/mv "$gate" "$gate.authenticated"
/bin/mv "$hostile" "$gate"
exec /bin/bash /dev/fd/10 "$expected"
'''
        completed = subprocess.run(
            ["/bin/bash", "-c", launcher, "descriptor-launch", str(gate), str(hostile), digest],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            close_fds=True,
            text=True,
        )
        assert completed.returncode == 0, completed.stderr
        assert completed.stdout == "trusted-gate\n"


def test_promotion_forwards_every_required_external_pin_under_env_clear() -> None:
    """Missing operator trust records must make the protected gate fail closed."""

    source = PROMOTION_WORKFLOW.read_text(encoding="utf-8")
    promotion = source.split("  production-promotion:\n", 1)[1]
    required = (
        "KAGEMUSHA_PRODUCTION_READINESS_GATE_PATH",
        "KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_ROOT",
        "KAGEMUSHA_PRODUCTION_READINESS_PYTHON_RUNTIME_TREE_SHA256",
        "KAGEMUSHA_PRODUCTION_READINESS_EXPECTED_MACOS_BUILD",
        "KAGEMUSHA_V4_RELEASE_POLICY_PATH",
        "KAGEMUSHA_V4_ARTIFACT_ROOT",
        "KAGEMUSHA_V4_KAGAMI_BIN",
        "KAGEMUSHA_V4_KAGAMI_SHA256",
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN",
        "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_SHA256",
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE",
        "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH",
        "KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256",
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION",
        "KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_AUTHORIZATION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_SIGNATURE_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_ALLOWED_SIGNERS_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_CONTROLLER_REVOCATION_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_EXECUTION_POLICY_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_RAW_UNIT_GRAPH_SHA256",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH",
        "KAGEMUSHA_BUILD_SOURCE_SEAL_NORMALIZED_UNIT_GRAPH_SHA256",
        "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_PATH",
        "KAGEMUSHA_V4_SEALED_CANDIDATE_BUILD_REPORT_SHA256",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_PRODUCTION_POLICY",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_KEY_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_FRESHNESS_TRUSTED_PUBLIC_KEY",
        "KAGEMUSHA_V4_PROMOTION_ID",
        "KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT",
    )
    launcher = NATIVE_PYTHON_LAUNCHER.read_text(encoding="utf-8")
    native_inventory = launcher.split(
        "const READINESS_EXTERNAL_ENVIRONMENT_NAMES: &[&str] = &[", 1
    )[1].split("];", 1)[0]
    native_names = tuple(
        line.strip().removesuffix(",").strip('"')
        for line in native_inventory.splitlines()
        if line.strip().startswith('"')
    )
    assert native_names == required
    native_command_environment = promotion.split(
        "/usr/bin/sudo -n /usr/bin/env -i", 1
    )[1].split("launch-kagemusha-readiness-v1", 1)[0]
    workflow_names = tuple(
        line.strip().split("=", 1)[0]
        for line in native_command_environment.splitlines()
        if line.strip().startswith("KAGEMUSHA_")
    )
    assert workflow_names == required
    for name in required:
        if name == "KAGEMUSHA_AUTHENTICATED_TOOL_CONTROLLER_BIN":
            assert f"{name}: {FIXED_CONTROLLER_PATH}" in promotion
            assert f"vars.{name}" not in promotion
        elif name in {
            "KAGEMUSHA_V4_PROMOTION_ID",
            "KAGEMUSHA_IOS_DEVICE_EVIDENCE_CATALOG_REVALIDATION_RECEIPT",
        }:
            assert f"vars.{name}" not in promotion
            assert f"readonly {name}" in promotion
        else:
            assert f"{name}: ${{{{ vars.{name} }}}}" in promotion
        assert f'{name}="${name}"' in promotion
    assert "/usr/bin/sudo -n /usr/bin/env -i" in promotion
    assert "KAGEMUSHA_PRODUCTION_READINESS_ROOT" not in promotion


def test_native_readiness_launcher_rejects_ambient_extensions_and_clears_child_env() -> None:
    """Unknown Kagemusha names must not cross the native pre-exec boundary."""

    source = NATIVE_PYTHON_LAUNCHER.read_text(encoding="utf-8")
    readiness = source.split("fn launch_readiness_macos", 1)[1].split(
        "fn launch_builder_macos", 1
    )[0]
    validator = source.split("fn validate_readiness_environment", 1)[1].split(
        "fn exit_status", 1
    )[0]
    assert "READINESS_EXTERNAL_ENVIRONMENT_NAMES" in source
    assert "variable inventory is not exact" in source
    assert '!name.starts_with("KAGEMUSHA_")' not in validator
    assert ".env_clear()" in readiness
    assert ".envs(readiness_environment)" in readiness
    assert '.arg("/dev/fd/10")' in readiness


def test_native_builder_executes_pinned_descriptor_and_cleanup_pins_pgid() -> None:
    """Path replacement and descendant escape controls remain in the native TCB."""

    source = NATIVE_PYTHON_LAUNCHER.read_text(encoding="utf-8")
    builder = source.split("fn launch_builder_macos", 1)[1].split(
        "fn native_launch_json", 1
    )[0]
    captured = source.split("pub(super) fn run_captured", 1)[1].split(
        "pub(super) fn high_descriptor", 1
    )[0]
    assert 'let builder_fd = high_descriptor(builder.file_mut(), 71)?;' in builder
    assert '.arg("/dev/fd/12")' in builder
    assert ".arg(&launch.builder)" not in builder
    assert "WNOWAIT" in captured
    observe = captured.index("leader_exited_nowait")
    sweep = captured.index("sweep_pinned_process_group", observe)
    reap = captured.index(".wait()", sweep)
    assert observe < sweep < reap
    assert ".try_wait()" not in captured
    assert "ensure_empty_process_group(process_group)?" in captured[reap:]


def test_promotion_cross_binds_native_v2_report_to_live_trust_pins() -> None:
    """A structurally valid forged V2 envelope must not select another TCB."""

    gate = READINESS_GATE.read_text(encoding="utf-8")
    promotion = gate.rsplit("def promotion_errors()", 1)[1]
    report = promotion.split(
        "sealed_build_report_identity = validate_sealed_build_report", 1
    )[1]
    launch_call = report.split("validate_native_build_launch_binding(", 1)[1]
    for binding in (
        "tool_controller_sha256",
        "trusted_python_sha256",
        "trusted_python_runtime_tree_sha256",
        "trusted_native_macos_build",
        "trusted_native_os_tcb_sha256",
    ):
        assert binding in launch_call
    assert "validate_native_builder_entrypoint_binding(" in report
    assert "sealed_build_report_identity, helper_bytes" in report


def test_python_runtime_is_a_full_preimport_tree_closure() -> None:
    """Interpreter-only pins cannot authenticate imported standard-library code."""

    gate = (ROOT / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    shell = gate.split("<<'PY'", 1)[0]
    tree_check = shell.index("promotion_root_tree_sha256()")
    version_probe = shell.index("sys.version_info >= (3, 10)")
    interpreter = shell.index('"${PYTHON_BIN}" -I -S - "${ROOT_DIR}"')
    assert tree_check < version_probe < interpreter
    assert "/private/var/db/iroha-kagemusha-python-runtime-v1" in shell
    assert '"${PYTHON_BIN}" != "${PYTHON_RUNTIME_ROOT}/bin/python3"' in shell
    assert "/usr/bin/find -s" in shell
    assert "-print0" in shell
    assert "has unbound extended attributes" in shell
    assert "contains a symbolic link" in shell
    assert "contains a group/world-writable entry" in shell
    assert "differs from its trusted tree SHA-256" in shell
    assert "changed before interpreter execution" in shell
    assert "changed during the production gate" in gate
    embedded = gate.split("<<'PY'\n", 1)[1].rsplit("\nPY\n", 1)[0]
    assert "trusted_python_runtime_root" in embedded
    assert "promotion Python import path escapes the sealed runtime tree" in embedded
    assert "imported promotion Python module" in embedded
    assert "MACOS_LIBC.flistxattr" in embedded


def test_source_contract_provider_executes_only_authenticated_exact_bytes() -> None:
    """Promotion must never reopen the static provider after authenticating it."""

    gate = READINESS_GATE.read_text(encoding="utf-8")
    provider_auth = gate.split(
        "def pin_authenticated_reviewed_source_file(", 1
    )[1].split("def authenticated_verifier_exited_without_reaping(", 1)[0]
    custody = provider_auth.index("require_production_root_custody(descriptor, label)")
    descriptor_read = provider_auth.index("payload = read_pinned_descriptor(")
    closure_auth = provider_auth.index(
        "authenticate_reviewed_source_file(relative, payload, source_commit, maximum_bytes)"
    )
    retain = provider_auth.index(
        "retained_pins.append((path, descriptor, fingerprint, label))"
    )
    assert custody < descriptor_read < closure_auth < retain

    promotion = gate.rsplit("def promotion_errors()", 1)[1].split(
        "source_contract_errors: list[str] = []", 1
    )[0]
    assert "for relative in READINESS_SOURCE_PROVIDERS:" in promotion
    assert (
        "authenticated_readiness_source_contract_bytes[relative] = (\n"
        "                pin_authenticated_reviewed_source_file("
    ) in promotion

    dispatch = gate.rsplit("source_contract_errors: list[str] = []", 1)[1].split(
        "errors = source_contract_errors", 1
    )[0]
    promotion_branch, candidate_branch = dispatch.split("\nelse:\n", 1)
    assert "read_bytes" not in promotion_branch
    assert (
        "source_contract_bytes = authenticated_readiness_source_contract_bytes"
        in promotion_branch
    )
    assert dispatch.count("(root / relative).read_bytes()") == 1
    assert "payload = (root / relative).read_bytes()" in candidate_branch
    assert "code = compile(\n            primary_bytes," in dispatch
    assert "exec(code, source_contract_context, source_contract_context)" in dispatch
    assert READINESS_SOURCE_CONTRACT.stat().st_size <= 140 * 1024


def test_ios_verifier_accepts_the_complete_configured_authority_tuple(
    tmp_path: Path,
) -> None:
    """Exercise the valid eight-field configuration before any evidence I/O."""

    gate = READINESS_GATE.read_text(encoding="utf-8")
    verifier = _load_embedded_function(gate, "verify_ios_evidence")
    assert callable(verifier)
    missing_release = tmp_path / ("1" * 64)
    configuration = (
        tmp_path,
        "lab-key",
        tmp_path / "lab.pub",
        tmp_path / "policy.json",
        "freshness-key",
        tmp_path / "freshness.pub",
        "2" * 64,
        tmp_path / "catalog-revalidation.json",
    )

    candidate_sha256, binding, error = verifier(
        missing_release,
        configuration,
        None,
        b"",
        tmp_path / "trusted-lab.pub",
        tmp_path / "trusted-policy.json",
        tmp_path / "trusted-freshness.pub",
        [],
        [],
        tmp_path,
    )

    assert candidate_sha256 is None
    assert binding is None
    assert error == (
        f"{missing_release.name}: physical-iOS evidence must use "
        f"{tmp_path}/<manifest-sha256>/raw"
    )


def test_promotion_gate_reconstructs_the_projection_from_every_signed_input() -> None:
    """A pinned hand-authored projection must not bypass signed graph reconstruction."""

    gate = (ROOT / "ci/check_kagemusha_production_readiness.sh").read_text(
        encoding="utf-8"
    )
    promotion = gate.rsplit("def promotion_errors()", 1)[1]
    source_identity = promotion.index("source_identity = parsed_identity")
    snapshot = promotion.index("snapshot_private_python_package(")
    verify = promotion.index("run_kagemusha_source_projection_snapshot.py")
    artifact_scan = promotion.index("for path in artifact_root.iterdir()")
    assert source_identity < snapshot < verify < artifact_scan
    assert '"scripts/profile_cargo_build.py"' in gate
    assert '"scripts/build_kagemusha_v4_candidate_bundle.py"' in gate
    assert (
        '"-I",\n                "-S",\n'
        '                str(trusted_source_projection_launcher)'
        in promotion
    )
    assert "str(producer_package_root)" in promotion[verify:artifact_scan]
    for argument in (
        "--authorization",
        "--controller-signature",
        "--controller-allowed-signers",
        "--controller-revocation",
        "--execution-policy",
        "--raw-unit-graph",
        "--unit-graph",
        "--projection",
    ):
        assert argument in promotion
    assert "source-projection reconstruction receipt differs from the signed inputs" in promotion
    assert "manifest build tools differ from the authenticated execution policy" in promotion
