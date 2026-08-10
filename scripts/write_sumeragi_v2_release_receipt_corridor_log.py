# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

_WIRE_RELEASE_INVARIANT_PYTEST_NODES = (
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_binds_current_semantic_sources "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_ledger_weakening "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_semantic_source_mutation"
)


def _sdk_suite_source_manifest(repo_root: Path, suite: str) -> str:
    """Resolve one reviewed SDK suite through its source-bound closure tool."""

    if suite not in _SDK_SOURCE_CLOSURE_SUITES:
        raise ReceiptError(f"unknown SDK source-closure suite {suite!r}")
    resolver = repo_root / _SDK_SOURCE_CLOSURE_RESOLVER
    manifest = repo_root / _SDK_SOURCE_CLOSURE_MANIFEST
    manifest_snapshot = _bounded_evidence_snapshot(
        manifest,
        "SDK source-closure manifest",
        maximum_bytes=_MAX_HELPER_BYTES,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    environment = _closed_replay_environment(repo_root)
    environment.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONHASHSEED": "0",
        }
    )
    status, stdout, stderr = _run_bounded_python_validator(
        resolver,
        [
            "--root",
            str(repo_root),
            "--manifest",
            str(manifest),
            "--suite",
            suite,
            "--manifest-sha256",
        ],
        cwd=repo_root,
        environment=environment,
        name=f"SDK source-closure resolver for {suite}",
        maximum_output_bytes=4096,
        watched_contracts=(manifest_snapshot,),
    )
    if status != 0:
        diagnostic = stderr.decode("utf-8", errors="replace").strip()
        suffix = f": {diagnostic}" if diagnostic else ""
        raise ReceiptError(
            f"SDK source-closure resolver rejected {suite!r}{suffix}"
        )
    if stderr:
        raise ReceiptError(
            f"SDK source-closure resolver for {suite!r} wrote to stderr"
        )
    if re.fullmatch(rb"[0-9a-f]{64}\n", stdout) is None:
        raise ReceiptError(
            f"SDK source-closure resolver for {suite!r} emitted a "
            "noncanonical digest"
        )
    return stdout[:-1].decode("ascii")


def _test_count_from_log(lines: list[str], kind: str, name: str) -> int:
    if kind == "cargo-focus":
        running = [line for line in lines if line == "running 1 test"]
        results = [
            line
            for line in lines
            if re.fullmatch(
                r"test result: ok\. 1 passed; 0 failed; 0 ignored; "
                r"0 measured; [0-9]+ filtered out; finished in .+",
                line,
            )
            is not None
        ]
        if not running or len(running) != len(results):
            raise ReceiptError(
                f"{name} has an ambiguous Cargo transcript for focused tests"
            )
        return len(results)
    if kind.startswith("cargo-"):
        running = [
            match
            for line in lines
            if (match := re.fullmatch(r"running ([0-9]+) tests?", line))
        ]
        results = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
                    r"0 measured; [0-9]+ filtered out; finished in .+",
                    line,
                )
            )
        ]
        if (
            len(running) != 1
            or len(results) != 1
            or running[0].group(1) != results[0].group(1)
        ):
            raise ReceiptError(f"{name} has an ambiguous Cargo transcript")
        return int(results[0].group(1))
    if kind == "pytest":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s", line
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(f"{name} has an ambiguous pytest transcript")
        return int(matches[0].group(1))
    if kind == "node":
        matches = [
            match
            for line in lines
            if (match := re.fullmatch(r"# pass ([0-9]+)", line))
        ]
        if (
            len(matches) != 1
            or lines.count(f"# tests {matches[0].group(1)}") != 1
            or lines.count("# fail 0") != 1
            or lines.count("# cancelled 0") != 1
            or lines.count("# skipped 0") != 1
            or lines.count("# todo 0") != 1
        ):
            raise ReceiptError(f"{name} has an ambiguous Node transcript")
        return int(matches[0].group(1))
    if kind == "native-amx-sdk":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"native-amx-v2-grouped-parity surface=[a-z]+ "
                    r"tests=([0-9]+) fixture_sha256=[0-9a-f]{64} "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous grouped Native AMX V2 SDK transcript"
            )
        return int(matches[0].group(1))
    if kind == "sdk-diagnostics":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"sumeragi-v2-sdk-diagnostics surface=[a-z]+ "
                    r"tests=([0-9]+) "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous Sumeragi v2 SDK diagnostics transcript"
            )
        return int(matches[0].group(1))
    if kind == "command":
        return 0
    raise ReceiptError(f"{name} has unknown leg kind {kind}")


def _prebuilt_artifact_root(
    repo_root: Path, artifact_root: Path, label: str = "release artifact root"
) -> Path:
    """Authenticate one private external root used by a prebuilt bundle."""

    if artifact_root != Path(os.path.abspath(artifact_root)):
        raise ReceiptError(f"{label} must be absolute and normalized")
    try:
        resolved = artifact_root.resolve(strict=True)
        metadata = resolved.lstat()
    except (OSError, RuntimeError) as error:
        raise ReceiptError(f"{label} is unavailable") from error
    source_root = repo_root.resolve(strict=True)
    try:
        roots_overlap = (
            Path(os.path.commonpath((resolved, source_root))) in {resolved, source_root}
        )
    except ValueError:
        roots_overlap = True
    if (
        resolved != artifact_root
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
        or roots_overlap
    ):
        raise ReceiptError(
            f"{label} must be one private owner-owned directory "
            "outside the sealed source"
        )
    return resolved


def _prebuilt_release_roots(
    *,
    repo_root: Path,
    fields: dict[str, str],
    expected_artifact_root: Path,
    expected_cargo_target_root: Path,
) -> tuple[Path, Path]:
    """Bind the corridor root fields to the authenticated bootstrap layout."""

    artifact_root = _prebuilt_artifact_root(
        repo_root, expected_artifact_root
    )
    cargo_target_root = _prebuilt_artifact_root(
        repo_root,
        expected_cargo_target_root,
        "release Cargo target root",
    )
    if fields["artifact_root_path"] != str(artifact_root):
        raise ReceiptError(
            "corridor artifact root is not the exact authenticated release "
            "artifact root"
        )
    if fields["cargo_target_root_path"] != str(cargo_target_root):
        raise ReceiptError(
            "corridor Cargo target root is not the exact authenticated "
            "release Cargo target root"
        )
    try:
        roots_overlap = Path(
            os.path.commonpath((artifact_root, cargo_target_root))
        ) in {artifact_root, cargo_target_root}
    except ValueError:
        roots_overlap = True
    if roots_overlap:
        raise ReceiptError("release artifact and Cargo target roots overlap")
    return artifact_root, cargo_target_root


def _prebuilt_directory(path: Path, name: str) -> Path:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        metadata = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if (
        resolved != path
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o500
        or metadata.st_uid != os.geteuid()
    ):
        raise ReceiptError(
            f"{name} must be an owner-owned resolved non-symlink directory "
            "with exact mode 0500"
        )
    return path
