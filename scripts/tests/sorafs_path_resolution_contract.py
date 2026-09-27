"""Keep operator-evidence resolution separate from strict artifact/runtime custody owners.

The shared evidence helper allows absent paths and accumulates diagnostics. Installed SDK
custody must instead reject missing/aliased parents before opening pinned files or subprocesses.
Those existing owners must retain strict resolution; replacing them with the evidence helper
would change their authority boundary. Pure captured namespaces and Node module resolution
are different operations from host filesystem path resolution.
"""
from pathlib import Path
import re


STRICT_CUSTODY_OWNERS = frozenset({
    "scripts/build_sorafs_java_consumer_artifact.py",
    "scripts/build_sorafs_python_consumer_artifact.py",
    "scripts/check_sorafs_production_promotion_bundle.py",
    "scripts/sorafs_python_environment.py",
    "scripts/sorafs_python_package_source.py",
    "scripts/sorafs_python_process.py",
    "scripts/sorafs_python_producer_inputs.py",
    "scripts/sorafs_python_runtime_custody.py",
    "scripts/sorafs_sdk_python_artifact_verifier.py",
})


def custody_owns_resolution(relative: Path, line: str, source: str) -> bool:
    """Recognize only the retained strict/pure/module owners, with no permissive fallback."""
    name = relative.as_posix()
    if "os.path.realpath(" in line:
        return False
    # The original script path is the already permitted import/bootstrap identity; a mixed
    # line must still validate every other resolution instead of hiding it behind __file__.
    checked_line = line.replace("Path(__file__).resolve()", "SCRIPT_BOOTSTRAP_PATH")
    calls = re.findall(r"\.resolve\(([^)]*)\)", checked_line)
    if not calls:
        return False
    if name in STRICT_CUSTODY_OWNERS:
        return all(arguments.strip() == "strict=True" for arguments in calls)
    if name == "scripts/sorafs_javascript_runtime_inputs.py":
        return (
            line.count("namespace.resolve(") == len(calls)
            and "class _Namespace:" in source
            and "def resolve(self, path: str, *, absent_leaf: bool = False)" in source
            and 'require(followed <= MAX_ALIASES, "runtime alias cycle/depth")' in source
            and 'require(absent_leaf and not leaf_alias, "runtime alias target is missing")' in source
        )
    if name == "scripts/sorafs_javascript_child_session.mjs":
        return (
            line.strip() == "const url = import.meta.resolve(specifier);"
            and 'demand(url === pathToFileURL(join(this.#input.installedRoot, "@iroha/iroha-js", expected)).href, "actual installed subject resolution differs");' in source
            and "const module = await import(url);" in source
        )
    return False
