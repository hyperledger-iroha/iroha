"""Static cross-platform contracts for the authenticated-tool controller."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTROLLER = (
    ROOT
    / "crates"
    / "iroha_kagami"
    / "src"
    / "bin"
    / "iroha_authenticated_tool_controller.rs"
)
CHECK = ROOT / "ci" / "check_authenticated_tool_controller.sh"
FULL_BACKEND_CFG = '#[cfg(any(target_os = "macos", test))]'
UNSUPPORTED_HOST_CFG = '#[cfg(all(not(target_os = "macos"), not(test)))]'


def controller_source() -> str:
    """Read the exact controller source used by the Cargo binary target."""

    return CONTROLLER.read_text(encoding="utf-8")


def test_production_non_macos_binary_excludes_full_backend_modules() -> None:
    """Unsupported production hosts compile only same-name rejecting stubs."""

    source = controller_source()
    for module, relative in (
        (
            "kagemusha_promotion_publisher",
            "kagemusha_promotion_publisher.rs",
        ),
        ("kagemusha_python_launcher", "kagemusha_python_launcher.rs"),
    ):
        assert (
            f'{FULL_BACKEND_CFG}\n'
            f'#[path = "iroha_authenticated_tool_controller/{relative}"]\n'
            f"mod {module};"
        ) in source
        assert (
            f"{UNSUPPORTED_HOST_CFG}\n"
            f"mod {module} {{"
        ) in source


def test_non_macos_entrypoint_is_a_fail_closed_dispatcher() -> None:
    """Every reviewed subcommand remains present and rejects without a backend."""

    source = controller_source()
    assert f"{FULL_BACKEND_CFG}\nfn entrypoint(" in source
    assert f"{UNSUPPORTED_HOST_CFG}\nfn entrypoint(" in source
    for subcommand in (
        "run-v1",
        "qualify-host-v1",
        "qualification-probe-v1",
        "launch-kagemusha-readiness-v1",
        "launch-kagemusha-sealed-builder-v1",
        "promote-kagemusha-release-v4",
    ):
        assert f'Some("{subcommand}")' in source
    for diagnostic in (
        "Linux isolation is unavailable: a qualified Landlock, seccomp, and delegated-cgroup backend is required",
        "host qualification is unavailable without the macOS backend",
        "qualification probe is unavailable without the macOS backend",
        "Kagemusha native Python launch is available only on a qualified macOS host",
        "Kagemusha promotion publication requires a qualified macOS host",
    ):
        assert diagnostic in source


def test_controller_lint_gate_remains_strict() -> None:
    """The platform boundary must not be implemented by suppressing warnings."""

    source = controller_source()
    check = CHECK.read_text(encoding="utf-8")
    assert "-D warnings" in check
    assert "-D unsafe-code" in check
    for suppression in ("allow(warnings)", "allow(dead_code)", "allow(unused_imports)"):
        assert suppression not in source
