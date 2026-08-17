# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

import ctypes
import errno

_WIRE_RELEASE_INVARIANT_PYTEST_NODES = (
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_binds_current_semantic_sources "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_ledger_weakening "
    "pytests/scripts/sumeragi_v2_multilane_wire_release_invariant_test.py::"
    "test_wire_release_invariant_rejects_semantic_source_mutation"
)

_CARGO_CACHE_INPUT_FORMAT = "iroha-sumeragi-v2-cargo-cache-input"
_CARGO_CACHE_FINAL_FORMAT = "iroha-sumeragi-v2-cargo-cache-final"
_CARGO_CACHE_SOURCE_READ_SEMANTICS = (
    "read-only; host filesystem may update access time"
)
_MAX_CARGO_CACHE_INPUT_INVENTORY_BYTES = 256 * 1024 * 1024
_MAX_CARGO_CACHE_INPUT_RECORDS = 250_000
_MAX_CARGO_CACHE_INPUT_FILE_BYTES = 4 * 1024 * 1024 * 1024
_MAX_CARGO_CACHE_INPUT_TOTAL_BYTES = 64 * 1024 * 1024 * 1024
_MAX_CARGO_CACHE_DEPTH = 128
_MAX_CARGO_CACHE_PATH_BYTES = 4096
_RECEIPT_VALIDATION_OPTION_ORDER = (
    "--candidate-identity",
    "--sealed-identity",
    "--release-root",
    "--bootstrap-completion",
    "--bootstrap-evidence-dir",
    "--bootstrap-identity",
    "--bootstrap-attestation",
    "--bootstrap-transcript",
    "--expected-bootstrap-completion-sha256",
    "--bootstrap-candidate-root",
    "--bootstrap-runner",
    "--signature-attestation",
    "--signature-transcript",
    "--signature-raw-commit",
    "--signature-cargo-lock",
    "--signature-allowed-signers",
    "--signature-revocation",
    "--signature-git",
    "--signature-ssh-keygen",
    "--expected-git-sha256",
    "--expected-ssh-keygen-sha256",
    "--expected-allowed-signers-sha256",
    "--expected-revocation-sha256",
    "--expected-signer-fingerprint",
    "--corridor-completion",
    "--formal-completion",
    "--seed-completion",
    "--chaos-completion",
    "--taira-completion",
    "--g4p-completion",
    "--g12-seed-completion",
    "--g12-fault-soak-completion",
    "--scaling-evidence-manifest",
    "--sdk-dependency-archive",
    "--sdk-dependency-input-inventory",
    "--sdk-dependency-final-work-inventory",
    "--runtime-tool-probe-manifest",
    "--runtime-tool-probe-result",
    "--expected-scaling-trial-harness-sha256",
    "--expected-scaling-configuration-sha256",
    "--expected-scaling-irohad-sha256",
    "--expected-scaling-iroha-cli-sha256",
    "--repository-root",
    "--output",
    "--verify-existing",
    "--validation-ack",
    "--source-manifest-sha256",
)
_RECEIPT_VALIDATION_PATH_OPTIONS = frozenset(
    {
        "--candidate-identity",
        "--sealed-identity",
        "--release-root",
        "--bootstrap-completion",
        "--bootstrap-evidence-dir",
        "--bootstrap-identity",
        "--bootstrap-attestation",
        "--bootstrap-transcript",
        "--bootstrap-candidate-root",
        "--bootstrap-runner",
        "--signature-attestation",
        "--signature-transcript",
        "--signature-raw-commit",
        "--signature-cargo-lock",
        "--signature-allowed-signers",
        "--signature-revocation",
        "--signature-git",
        "--signature-ssh-keygen",
        "--corridor-completion",
        "--formal-completion",
        "--seed-completion",
        "--chaos-completion",
        "--taira-completion",
        "--g4p-completion",
        "--g12-seed-completion",
        "--g12-fault-soak-completion",
        "--scaling-evidence-manifest",
        "--sdk-dependency-archive",
        "--sdk-dependency-input-inventory",
        "--sdk-dependency-final-work-inventory",
        "--runtime-tool-probe-manifest",
        "--runtime-tool-probe-result",
        "--repository-root",
        "--output",
        "--validation-ack",
    }
)


def _receipt_validation_invocation_value_sha256(
    kind: str, value: str | bool,
) -> str:
    payload = json.dumps(
        {"kind": kind, "value": value},
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def _receipt_validation_invocation_binding(arguments: list[str]) -> dict[str, Any]:
    """Bind the exact ordered validator invocation without disclosing values."""

    bindings: list[dict[str, str]] = []
    cursor = 0
    for expected_name in _RECEIPT_VALIDATION_OPTION_ORDER:
        if cursor >= len(arguments) or arguments[cursor] != expected_name:
            raise ReceiptError("receipt validation argument order is not exact")
        cursor += 1
        if expected_name == "--verify-existing":
            kind = "flag"
            normalized: str | bool = True
        else:
            if cursor >= len(arguments):
                raise ReceiptError("receipt validation argument value is absent")
            raw_value = arguments[cursor]
            cursor += 1
            if expected_name in _RECEIPT_VALIDATION_PATH_OPTIONS:
                kind = "path"
                normalized = os.path.abspath(os.path.normpath(raw_value))
                if raw_value != normalized:
                    raise ReceiptError(
                        "receipt validation path argument is not canonical"
                    )
            else:
                kind = "text"
                normalized = raw_value
        bindings.append(
            {
                "name": expected_name,
                "value_kind": kind,
                "normalized_value_sha256": (
                    _receipt_validation_invocation_value_sha256(kind, normalized)
                ),
            }
        )
    if cursor != len(arguments):
        raise ReceiptError("receipt validation invocation has trailing arguments")
    invocation = {
        "profile": "release",
        "operation": "verify-existing-and-ack",
        "python_flags": ["-I", "-S"],
        "validator": "protected:validate-receipt.py",
        "ordered_options": bindings,
    }
    payload = json.dumps(
        invocation,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return {
        **invocation,
        "invocation_sha256": hashlib.sha256(payload).hexdigest(),
    }


def _cargo_cache_relative_path(value: Any, name: str) -> PurePosixPath:
    if not isinstance(value, str):
        raise ReceiptError(f"{name} path is not text")
    relative = PurePosixPath(value)
    if (
        relative.is_absolute()
        or value != relative.as_posix()
        or not relative.parts
        or any(part in {"", ".", ".."} for part in relative.parts)
        or relative.parts[0] not in {"registry", "git"}
    ):
        raise ReceiptError(f"{name} path is not one canonical cache-relative path")
    return relative


def _cargo_cache_final_relative_path(value: Any, name: str) -> PurePosixPath:
    if not isinstance(value, str):
        raise ReceiptError(f"{name} path is not text")
    relative = PurePosixPath(value)
    if (
        relative.is_absolute()
        or value != relative.as_posix()
        or not relative.parts
        or any(part in {"", ".", ".."} for part in relative.parts)
        or len(relative.parts) > _MAX_CARGO_CACHE_DEPTH
        or len(value.encode("utf-8")) > _MAX_CARGO_CACHE_PATH_BYTES
    ):
        raise ReceiptError(f"{name} path is not one canonical cache-relative path")
    return relative


def _cargo_cache_octal_mode(value: Any, name: str) -> int:
    if not isinstance(value, str) or re.fullmatch(r"[0-7]{4}", value) is None:
        raise ReceiptError(f"{name} mode is not canonical octal")
    return int(value, 8)


def _cargo_cache_integer(value: Any, name: str) -> int:
    if type(value) is not int or value < 0:
        raise ReceiptError(f"{name} is not one nonnegative integer")
    return value


_CARGO_CACHE_DIRECTORY_FLAGS = (
    os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    | getattr(os, "O_NOFOLLOW", 0)
)
_CARGO_CACHE_STABLE_FIELDS = (
    "st_dev", "st_ino", "st_mode", "st_uid", "st_nlink", "st_size",
    "st_mtime_ns", "st_ctime_ns",
)


def _cargo_cache_unchanged(before: os.stat_result, after: os.stat_result) -> bool:
    return all(getattr(before, field) == getattr(after, field) for field in _CARGO_CACHE_STABLE_FIELDS)


def _cargo_cache_names(descriptor: int, budget: dict[str, int]) -> tuple[str, ...]:
    names: list[str] = []
    try:
        with os.scandir(descriptor) as scanned:
            for entry in scanned:
                names.append(entry.name)
                if len(names) > _MAX_CARGO_CACHE_INPUT_RECORDS:
                    raise ReceiptError("corridor Cargo home exceeds its structural limits")
    except OSError as error:
        raise ReceiptError("corridor Cargo home could not be enumerated") from error
    return tuple(sorted(names))


def _cargo_cache_stat(parent_fd: int, name: str) -> os.stat_result:
    try:
        return os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except OSError as error:
        raise ReceiptError("corridor Cargo home changed during traversal") from error


def _cargo_cache_open_regular(
    parent_fd: int, leaf: str, name: str, before: os.stat_result,
    budget: dict[str, int],
) -> tuple[os.stat_result, str]:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(leaf, flags, dir_fd=parent_fd)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if opened.st_size > _MAX_CARGO_CACHE_INPUT_FILE_BYTES:
            raise ReceiptError(f"{name} exceeds the cache file size limit")
        budget["bytes"] += opened.st_size
        if budget["bytes"] > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
            raise ReceiptError("corridor Cargo home exceeds its total byte limit")
        digest = hashlib.sha256()
        total = 0
        while block := os.read(descriptor, 1024 * 1024):
            digest.update(block)
            total += len(block)
            if total > _MAX_CARGO_CACHE_INPUT_FILE_BYTES or budget["bytes"] - opened.st_size + total > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
                raise ReceiptError(f"{name} grew beyond the cache byte limits")
        after = _cargo_cache_stat(parent_fd, leaf)
        if (
            not stat.S_ISREG(opened.st_mode)
            or not _cargo_cache_unchanged(before, opened)
            or not _cargo_cache_unchanged(opened, after)
            or total != opened.st_size
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        return opened, digest.hexdigest()
    finally:
        os.close(descriptor)


def _cargo_cache_tree(cargo_home: Path) -> dict[str, tuple[Any, ...]]:
    entries: dict[str, tuple[Any, ...]] = {}
    budget = {"records": 0, "bytes": 0}
    try:
        root_before = cargo_home.lstat()
        root_fd = os.open(cargo_home, _CARGO_CACHE_DIRECTORY_FLAGS)
    except OSError as error:
        raise ReceiptError("corridor Cargo home could not be opened safely") from error
    if not _cargo_cache_unchanged(root_before, os.fstat(root_fd)):
        os.close(root_fd)
        raise ReceiptError("corridor Cargo home changed while opened")

    def target_parts(parent: tuple[str, ...], target: str) -> tuple[str, ...]:
        rendered = PurePosixPath(target)
        if rendered.is_absolute():
            raise ReceiptError("corridor Cargo home contains an external symlink escape")
        parts = list(parent)
        for part in rendered.parts:
            if part in {"", "."}:
                continue
            if part == "..":
                if not parts:
                    raise ReceiptError("corridor Cargo home contains an external symlink escape")
                parts.pop()
            else:
                parts.append(part)
        if not parts:
            raise ReceiptError("corridor Cargo home symlink target is unavailable")
        return tuple(parts)

    def require_target(parts: tuple[str, ...]) -> None:
        descriptor = os.dup(root_fd)
        try:
            for index, part in enumerate(parts):
                metadata = _cargo_cache_stat(descriptor, part)
                if stat.S_ISLNK(metadata.st_mode):
                    raise ReceiptError("corridor Cargo home symlink target has a symlink component")
                if index + 1 == len(parts):
                    if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)):
                        raise ReceiptError("corridor Cargo home symlink target is special")
                    return
                if not stat.S_ISDIR(metadata.st_mode):
                    raise ReceiptError("corridor Cargo home symlink target is unavailable")
                child = os.open(part, _CARGO_CACHE_DIRECTORY_FLAGS, dir_fd=descriptor)
                if not _cargo_cache_unchanged(metadata, os.fstat(child)):
                    os.close(child)
                    raise ReceiptError("corridor Cargo home symlink target changed")
                os.close(descriptor)
                descriptor = child
        finally:
            os.close(descriptor)

    def visit(directory_fd: int, relative_parts: tuple[str, ...]) -> None:
        before = os.fstat(directory_fd)
        if not stat.S_ISDIR(before.st_mode) or before.st_uid != os.geteuid() or stat.S_IMODE(before.st_mode) & 0o022:
            raise ReceiptError("corridor Cargo home contains an unsafe directory")
        names = _cargo_cache_names(directory_fd, budget)
        for child_name in names:
            relative = (*relative_parts, child_name)
            relative_text = PurePosixPath(*relative).as_posix()
            budget["records"] += 1
            if budget["records"] > _MAX_CARGO_CACHE_INPUT_RECORDS or len(relative) > _MAX_CARGO_CACHE_DEPTH or len(relative_text.encode("utf-8")) > _MAX_CARGO_CACHE_PATH_BYTES:
                raise ReceiptError("corridor Cargo home exceeds its structural limits")
            metadata = _cargo_cache_stat(directory_fd, child_name)
            mode = stat.S_IMODE(metadata.st_mode)
            if metadata.st_uid != os.geteuid() or (not stat.S_ISLNK(metadata.st_mode) and mode & 0o022):
                raise ReceiptError("corridor Cargo home contains a non-private entry")
            if stat.S_ISDIR(metadata.st_mode):
                entries[relative_text] = ("directory", metadata)
                child_fd = os.open(child_name, _CARGO_CACHE_DIRECTORY_FLAGS, dir_fd=directory_fd)
                try:
                    if not _cargo_cache_unchanged(metadata, os.fstat(child_fd)):
                        raise ReceiptError("corridor Cargo home directory changed while opened")
                    visit(child_fd, relative)
                finally:
                    os.close(child_fd)
                if not _cargo_cache_unchanged(metadata, _cargo_cache_stat(directory_fd, child_name)):
                    raise ReceiptError("corridor Cargo home changed during traversal")
            elif stat.S_ISREG(metadata.st_mode):
                opened, digest = _cargo_cache_open_regular(directory_fd, child_name, f"corridor Cargo cache file {relative_text}", metadata, budget)
                if opened.st_nlink != 1:
                    raise ReceiptError("corridor Cargo home contains a hard-link escape")
                entries[relative_text] = ("file", opened, digest)
            elif stat.S_ISLNK(metadata.st_mode):
                try:
                    target = os.readlink(child_name, dir_fd=directory_fd)
                    after = _cargo_cache_stat(directory_fd, child_name)
                except OSError as error:
                    raise ReceiptError("corridor Cargo home contains an unreadable symlink") from error
                if not _cargo_cache_unchanged(metadata, after) or os.readlink(child_name, dir_fd=directory_fd) != target:
                    raise ReceiptError("corridor Cargo home contains an unsafe symlink")
                require_target(target_parts(relative_parts, target))
                entries[relative_text] = ("symlink", metadata, target)
            else:
                raise ReceiptError("corridor Cargo home contains a forbidden special file")
        if _cargo_cache_names(directory_fd, budget) != names or not _cargo_cache_unchanged(before, os.fstat(directory_fd)):
            raise ReceiptError("corridor Cargo home changed during traversal")

    try:
        visit(root_fd, ())
        if not _cargo_cache_unchanged(root_before, cargo_home.lstat()):
            raise ReceiptError("corridor Cargo home root was replaced during traversal")
        return entries
    finally:
        os.close(root_fd)


def _validate_cargo_cache_input(
    fields: dict[str, str], *, artifact_root: Path,
    private_build_roots_available: bool = True,
) -> dict[str, Any]:
    cargo_home = artifact_root / "cargo-home"
    inventory_path = artifact_root / "cargo-cache-input.json"
    final_inventory_path = artifact_root / "cargo-cache-final.json"
    if (
        Path(fields["cargo_home_path"]) != cargo_home
        or Path(fields["cargo_cache_input_inventory_path"]) != inventory_path
        or Path(fields["cargo_cache_final_inventory_path"]) != final_inventory_path
    ):
        raise ReceiptError("corridor Cargo cache paths are not the exact private output paths")
    if private_build_roots_available:
        _private_evidence_directory(cargo_home, "corridor Cargo home")
    else:
        _require_pruned_private_root(cargo_home, "corridor Cargo home")
    expected_runtime = {
        "runtime_home_path": artifact_root / "home",
        "runtime_tmpdir_path": artifact_root / "tmp",
        "runtime_tmp_path": artifact_root / "tmp",
        "runtime_temp_path": artifact_root / "tmp",
        "runtime_cache_path": artifact_root / "cache",
    }
    if any(Path(fields[name]) != path for name, path in expected_runtime.items()):
        raise ReceiptError("corridor runtime environment escaped its private roots")
    runtime_directories = {}
    for name, path in {"home": artifact_root / "home", "tmp": artifact_root / "tmp", "cache": artifact_root / "cache"}.items():
        if private_build_roots_available:
            _, metadata = _private_evidence_directory(path, f"corridor runtime {name}")
            mode = format(stat.S_IMODE(metadata.st_mode), "04o")
            owner_uid = metadata.st_uid
        else:
            _require_pruned_private_root(path, f"corridor runtime {name}")
            mode = "0700"
            owner_uid = os.geteuid()
        runtime_directories[name] = {
            "path": str(path),
            "mode": mode,
            "owner_uid": owner_uid,
        }
    inventory = _bounded_evidence_snapshot(
        inventory_path,
        "Cargo cache input inventory",
        maximum_bytes=_MAX_CARGO_CACHE_INPUT_INVENTORY_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    expected_digest = _require_digest(
        fields["cargo_cache_input_inventory_sha256"],
        "Cargo cache input inventory digest",
    )
    if inventory.sha256 != expected_digest:
        raise ReceiptError("Cargo cache input inventory digest mismatch")
    try:
        document = json.loads(_decode_lf_text(inventory, "Cargo cache input inventory"))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ReceiptError("Cargo cache input inventory is malformed JSON") from error
    if not isinstance(document, dict) or _canonical_json(document) != inventory.data:
        raise ReceiptError("Cargo cache input inventory is not canonical JSON")
    if set(document) != {
        "format",
        "schema_version",
        "source_cargo_home_disclosure",
        "source_read_semantics",
        "cargo_home_path",
        "roots",
        "input_record_count",
        "input_file_bytes",
        "records",
    }:
        raise ReceiptError("Cargo cache input inventory schema is not exact")
    if (
        document["format"] != _CARGO_CACHE_INPUT_FORMAT
        or type(document["schema_version"]) is not int
        or document["schema_version"] != 1
        or document["source_read_semantics"]
        != _CARGO_CACHE_SOURCE_READ_SEMANTICS
        or document["source_cargo_home_disclosure"] != "withheld"
        or document["cargo_home_path"] != str(cargo_home)
    ):
        raise ReceiptError("Cargo cache input inventory identity is not exact")
    roots = document["roots"]
    records = document["records"]
    declared_record_count = _cargo_cache_integer(
        document["input_record_count"], "Cargo cache input record count"
    )
    if (
        not isinstance(roots, list)
        or roots not in ([], ["registry"], ["git"], ["registry", "git"])
        or not isinstance(records, list)
        or len(records) > _MAX_CARGO_CACHE_INPUT_RECORDS
        or declared_record_count != len(records)
    ):
        raise ReceiptError("Cargo cache input inventory cardinality is not bounded")
    declared_file_bytes = _cargo_cache_integer(
        document["input_file_bytes"], "Cargo cache input byte count"
    )
    if declared_file_bytes > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
        raise ReceiptError("Cargo cache input inventory exceeds its total byte limit")
    final_inventory = _bounded_evidence_snapshot(
        final_inventory_path,
        "Cargo cache final inventory",
        maximum_bytes=_MAX_CARGO_CACHE_INPUT_INVENTORY_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    final_digest = _require_digest(
        fields["cargo_cache_final_inventory_sha256"],
        "Cargo cache final inventory digest",
    )
    if final_inventory.sha256 != final_digest:
        raise ReceiptError("Cargo cache final inventory digest mismatch")
    runtime_inventory = _bounded_evidence_snapshot(
        Path(fields["runtime_inventory_path"]), "private runtime inventory",
        maximum_bytes=_MAX_CARGO_CACHE_INPUT_INVENTORY_BYTES,
        expected_mode=0o400, allowed_owners={os.geteuid()}, require_single_link=True,
    )
    if (
        runtime_inventory.sha256 != _require_digest(fields["runtime_inventory_sha256"], "private runtime inventory digest")
        or runtime_inventory.path.parent != artifact_root.parent
    ):
        raise ReceiptError("private runtime inventory binding is not exact")
    try:
        runtime_document = json.loads(_decode_lf_text(runtime_inventory, "private runtime inventory"))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ReceiptError("private runtime inventory is malformed JSON") from error
    runtime_keys = {"format", "schema_version", "runtime_root", "record_count", "file_bytes", "records", "source_disclosure", "input_record_count", "input_file_bytes", "input_records"}
    framework_runtime = "framework_python_relocation" in runtime_document
    if framework_runtime:
        runtime_keys.add("framework_python_relocation")
    if not isinstance(runtime_document, dict) or set(runtime_document) != runtime_keys or _canonical_json(runtime_document) != runtime_inventory.data or runtime_document["format"] != "iroha-sumeragi-v2-private-runtime" or type(runtime_document["schema_version"]) is not int or runtime_document["schema_version"] != (2 if framework_runtime else 1) or runtime_document["source_disclosure"] != "withheld" or runtime_document["runtime_root"] != str(artifact_root.parent / "runtime"):
        raise ReceiptError("private runtime inventory schema is not exact")
    for prefix in ("", "input_"):
        records_name = f"{prefix}records"
        count_name = f"{prefix}record_count"
        bytes_name = f"{prefix}file_bytes"
        runtime_records = runtime_document[records_name]
        if not isinstance(runtime_records, list) or len(runtime_records) > _MAX_CARGO_CACHE_INPUT_RECORDS or type(runtime_document[count_name]) is not int or runtime_document[count_name] != len(runtime_records) or type(runtime_document[bytes_name]) is not int or runtime_document[bytes_name] < 0 or runtime_document[bytes_name] > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
            raise ReceiptError("private runtime inventory accounting is not exact")
        previous_runtime_path = ""
        observed_runtime_bytes = 0
        for index, record in enumerate(runtime_records):
            label = f"private runtime {prefix}record {index}"
            if not isinstance(record, dict) or not isinstance(record.get("path"), str):
                raise ReceiptError(f"{label} is not exact")
            relative = _cargo_cache_final_relative_path(record["path"], label).as_posix()
            if relative <= previous_runtime_path:
                raise ReceiptError("private runtime records are not uniquely sorted")
            previous_runtime_path = relative
            kind = record.get("kind")
            common = {"path", "kind", "mode"} if not prefix else {"path", "kind", "source_mode", "destination_mode"}
            identities = ({"device", "inode"} if not prefix else {"source_device", "source_inode", "destination_device", "destination_inode"}) if kind in {"directory", "file"} else set()
            expected_record_keys = common | identities | ({"size", "sha256"} if kind == "file" else {"target"} if kind == "symlink" else set())
            if kind not in {"directory", "file", "symlink"} or set(record) != expected_record_keys:
                raise ReceiptError(f"{label} metadata is not exact")
            for key in identities | ({"size"} if kind == "file" else set()):
                _cargo_cache_integer(record[key], label)
            output_mode = _cargo_cache_octal_mode(
                record["destination_mode" if prefix else "mode"], label,
            )
            if framework_runtime and kind != "symlink" and output_mode & 0o022:
                raise ReceiptError("framework Python runtime mode is unsafe")
            if prefix:
                source_mode = _cargo_cache_octal_mode(record["source_mode"], label)
                if framework_runtime and kind != "symlink" and source_mode & 0o022:
                    raise ReceiptError("framework Python source mode is unsafe")
            if kind == "file":
                _require_digest(record["sha256"], f"{label} digest")
                observed_runtime_bytes += record["size"]
            elif kind == "symlink" and not isinstance(record["target"], str):
                raise ReceiptError(f"{label} target is not text")
        if observed_runtime_bytes != runtime_document[bytes_name]:
            raise ReceiptError("private runtime byte accounting is not exact")
    if framework_runtime:
        _validate_framework_python_relocation_evidence(
            runtime_document["framework_python_relocation"],
            runtime_document["framework_python_relocation"],
            runtime_document["input_records"], runtime_document["records"],
            runtime_document["input_record_count"],
            runtime_document["input_file_bytes"],
        )
    try:
        final_document = json.loads(
            _decode_lf_text(final_inventory, "Cargo cache final inventory")
        )
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ReceiptError("Cargo cache final inventory is malformed JSON") from error
    if (
        not isinstance(final_document, dict)
        or _canonical_json(final_document) != final_inventory.data
        or set(final_document)
        != {"format", "schema_version", "cargo_home_path", "record_count", "file_bytes", "records"}
        or final_document["format"] != _CARGO_CACHE_FINAL_FORMAT
        or type(final_document["schema_version"]) is not int
        or final_document["schema_version"] != 1
        or final_document["cargo_home_path"] != str(cargo_home)
        or not isinstance(final_document["records"], list)
        or len(final_document["records"]) > _MAX_CARGO_CACHE_INPUT_RECORDS
    ):
        raise ReceiptError("Cargo cache final inventory schema is not exact")
    final_record_count = _cargo_cache_integer(
        final_document["record_count"], "Cargo cache final record count"
    )
    final_file_bytes = _cargo_cache_integer(
        final_document["file_bytes"], "Cargo cache final byte count"
    )
    if final_file_bytes > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
        raise ReceiptError("Cargo cache final inventory exceeds its total byte limit")
    previous_final_path = ""
    declared_final_bytes = 0
    for index, record in enumerate(final_document["records"]):
        name = f"Cargo cache final record {index}"
        if not isinstance(record, dict):
            raise ReceiptError(f"{name} is not an object")
        relative_text = _cargo_cache_final_relative_path(record.get("path"), name).as_posix()
        if relative_text <= previous_final_path:
            raise ReceiptError("Cargo cache final records are not uniquely sorted")
        previous_final_path = relative_text
        kind = record.get("kind")
        expected_keys = {
            "directory": {"path", "kind", "mode", "device", "inode"},
            "file": {"path", "kind", "mode", "device", "inode", "size", "sha256"},
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        if expected_keys is None or set(record) != expected_keys:
            raise ReceiptError(f"{name} metadata is not exact")
        _cargo_cache_octal_mode(record["mode"], name)
        if kind in {"directory", "file"}:
            _cargo_cache_integer(record["device"], name)
            _cargo_cache_integer(record["inode"], name)
        if kind == "file":
            _cargo_cache_integer(record["size"], name)
            _require_digest(record["sha256"], f"{name} digest")
            declared_final_bytes += record["size"]
        elif kind == "symlink" and not isinstance(record["target"], str):
            raise ReceiptError(f"{name} symlink target is not text")
    if (
        final_record_count != len(final_document["records"])
        or final_file_bytes != declared_final_bytes
    ):
        raise ReceiptError("Cargo cache final inventory accounting is not exact")
    current_entries = (
        _cargo_cache_tree(cargo_home) if private_build_roots_available else {}
    )
    if private_build_roots_available and any(
        name in current_entries for name in ("config", "config.toml")
    ):
        raise ReceiptError("corridor Cargo home contains external configuration")
    final_records: list[dict[str, Any]] = []
    observed_final_bytes = 0
    for relative_text, current in sorted(current_entries.items()):
        metadata = current[1]
        record: dict[str, Any] = {
            "path": relative_text,
            "kind": current[0],
            "mode": format(stat.S_IMODE(metadata.st_mode), "04o"),
        }
        if current[0] in {"directory", "file"}:
            record.update({"device": metadata.st_dev, "inode": metadata.st_ino})
        if current[0] == "file":
            record.update({"size": metadata.st_size, "sha256": current[2]})
            observed_final_bytes += metadata.st_size
        elif current[0] == "symlink":
            record["target"] = current[2]
        final_records.append(record)
    if private_build_roots_available and (
        final_document["records"] != final_records
        or final_record_count != len(final_records)
        or final_file_bytes != observed_final_bytes
    ):
        raise ReceiptError("Cargo cache final inventory does not match the exact final tree")
    previous_path = ""
    seen_paths: set[str] = set()
    input_file_count = 0
    input_file_bytes = 0
    for index, record in enumerate(records):
        name = f"Cargo cache input record {index}"
        if not isinstance(record, dict):
            raise ReceiptError(f"{name} is not an object")
        relative = _cargo_cache_relative_path(record.get("path"), name)
        relative_text = relative.as_posix()
        if relative_text <= previous_path or relative_text in seen_paths:
            raise ReceiptError("Cargo cache input records are not uniquely sorted")
        previous_path = relative_text
        seen_paths.add(relative_text)
        if relative.parts[0] not in roots:
            raise ReceiptError(f"{name} is outside the declared cache roots")
        current = current_entries.get(relative_text)
        if private_build_roots_available and current is None:
            raise ReceiptError(f"{name} is absent from the private Cargo home")
        kind = record.get("kind")
        if kind == "directory":
            expected_keys = {
                "path",
                "kind",
                "source_device",
                "source_inode",
                "source_mode",
                "destination_device",
                "destination_inode",
                "destination_mode",
            }
        elif kind == "file":
            expected_keys = {
                "path",
                "kind",
                "source_device",
                "source_inode",
                "source_mode",
                "destination_device",
                "destination_inode",
                "destination_mode",
                "size",
                "sha256",
            }
        elif kind == "symlink":
            expected_keys = {
                "path",
                "kind",
                "source_mode",
                "destination_mode",
                "target",
            }
        else:
            raise ReceiptError(f"{name} has an unknown entry kind")
        if set(record) != expected_keys or (
            private_build_roots_available and current[0] != kind
        ):
            raise ReceiptError(f"{name} metadata is not exact")
        source_mode = _cargo_cache_octal_mode(record["source_mode"], name)
        destination_mode = _cargo_cache_octal_mode(record["destination_mode"], name)
        del source_mode
        metadata = current[1] if private_build_roots_available else None
        if private_build_roots_available and stat.S_IMODE(metadata.st_mode) != destination_mode:
            raise ReceiptError(f"{name} mode does not match the private copy")
        if kind in {"directory", "file"}:
            source_identity = (
                _cargo_cache_integer(record["source_device"], name),
                _cargo_cache_integer(record["source_inode"], name),
            )
            destination_identity = (
                _cargo_cache_integer(record["destination_device"], name),
                _cargo_cache_integer(record["destination_inode"], name),
            )
            if private_build_roots_available and (
                metadata.st_dev, metadata.st_ino
            ) != destination_identity:
                raise ReceiptError(f"{name} inode does not match the private copy")
            if kind == "file" and destination_identity == source_identity:
                raise ReceiptError(f"{name} shares a regular-file inode with its source")
        if kind == "file":
            input_file_count += 1
            size = _cargo_cache_integer(record["size"], name)
            digest = _require_digest(record["sha256"], f"{name} digest")
            if size > _MAX_CARGO_CACHE_INPUT_FILE_BYTES:
                raise ReceiptError(f"{name} exceeds the cache input file size limit")
            input_file_bytes += size
            if input_file_bytes > _MAX_CARGO_CACHE_INPUT_TOTAL_BYTES:
                raise ReceiptError("Cargo cache input exceeds its total byte limit")
            if private_build_roots_available and (
                metadata.st_size != size or current[2] != digest
            ):
                raise ReceiptError(f"{name} content does not match the private copy")
        elif kind == "symlink":
            target = record["target"]
            if not isinstance(target, str) or (
                private_build_roots_available and current[2] != target
            ):
                raise ReceiptError(f"{name} symlink target does not match the private copy")
    for root in roots:
        root_record = next(
            (
                record
                for record in records
                if record.get("path") == root and record.get("kind") == "directory"
            ),
            None,
        )
        if root_record is None:
            raise ReceiptError("Cargo cache input inventory omits a declared root")
    if input_file_bytes != declared_file_bytes:
        raise ReceiptError("Cargo cache input byte count does not match its records")
    def sanitized_file(
        snapshot: EvidenceSnapshot | PathContract, archive_id: str
    ) -> dict[str, Any]:
        return {
            "archive_id": archive_id,
            "mode": f"{snapshot.mode:04o}",
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
        }

    return {
        "schema_version": 2,
        "inventory": sanitized_file(
            inventory, "release-cargo-cache.input-inventory.v1"
        ),
        "final_inventory": sanitized_file(
            final_inventory, "release-cargo-cache.final-inventory.v1"
        ),
        "runtime_inventory": sanitized_file(
            runtime_inventory, "release-runtime.inventory.v1"
        ),
        "runtime_environment_sha256": hashlib.sha256(
            _canonical_json({name: fields[name] for name in expected_runtime})
        ).hexdigest(),
        "runtime_directories": {
            name: {
                "archive_id": f"release-runtime.directory.{name}.v1",
                "mode": record["mode"],
            }
            for name, record in sorted(runtime_directories.items())
        },
        "cargo_home": {
            "archive_id": "release-cargo-cache.home.v1",
            "mode": (
                f"{stat.S_IMODE(cargo_home.lstat().st_mode):04o}"
                if private_build_roots_available else "0700"
            ),
        },
        "source_cargo_home_disclosure": "withheld",
        "input_root_count": len(roots),
        "input_record_count": len(records),
        "input_file_count": input_file_count,
    }


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


def _require_pruned_private_root(path: Path, label: str) -> None:
    """Require one disposable private build root to remain absent."""

    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise ReceiptError(f"{label} pruning is indeterminate") from error
    raise ReceiptError(f"{label} survived retained-release pruning")


def _prebuilt_release_roots(
    *,
    repo_root: Path,
    fields: dict[str, str],
    expected_artifact_root: Path,
    expected_cargo_target_root: Path,
    private_build_roots_available: bool = True,
) -> tuple[Path, Path]:
    """Bind the corridor root fields to the authenticated bootstrap layout."""

    artifact_root = _prebuilt_artifact_root(
        repo_root, expected_artifact_root
    )
    cargo_target_root = expected_cargo_target_root
    if private_build_roots_available:
        cargo_target_root = _prebuilt_artifact_root(
            repo_root,
            expected_cargo_target_root,
            "release Cargo target root",
        )
    else:
        _require_pruned_private_root(
            cargo_target_root, "release Cargo target root"
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


def _publish_receipt_validation_ack(
    *, ack_path: Path, receipt_path: Path, release_root: Path,
    source_manifest_sha256: str, bootstrap_completion_sha256: str,
    revalidate: Any,
) -> None:
    """Publish the archived validator's canonical, no-clobber success ack."""

    invocation_root = release_root.parent
    validator_path = Path(__file__).resolve(strict=True)
    if (
        ack_path != invocation_root / "receipt-validation-ack.json"
        or invocation_root.resolve(strict=True) != invocation_root
        or stat.S_IMODE(invocation_root.lstat().st_mode) != 0o700
        or invocation_root.lstat().st_uid != os.geteuid()
        or _DIGEST_RE.fullmatch(source_manifest_sha256) is None
        or _DIGEST_RE.fullmatch(bootstrap_completion_sha256) is None
    ):
        raise ReceiptError("receipt validation acknowledgment target is not exact")
    receipt = _bounded_evidence_snapshot(
        receipt_path, "validated aggregate receipt", maximum_bytes=_MAX_RELEASE_JSON_BYTES,
    )
    receipt_value = _decode_canonical_json(receipt.data, "validated aggregate receipt")
    if receipt_value.get("identity", {}).get("sealed_source_manifest_sha256") != source_manifest_sha256:
        raise ReceiptError("receipt acknowledgment source digest disagrees with the receipt")
    validator = _bounded_evidence_snapshot(
        validator_path, "archived receipt validator", maximum_bytes=16 * 1024 * 1024,
    )
    stdout = f"Sumeragi v2 aggregate release receipt verified: {receipt_path}\n".encode()
    if sys.flags.isolated != 1 or sys.flags.no_site != 1:
        raise ReceiptError("receipt validation Python flags are not exact")
    invocation_binding = _receipt_validation_invocation_binding(sys.argv[1:])
    value = {
        "format": "iroha-sumeragi-v2-receipt-validation-ack",
        "schema_version": 3,
        "profile": "release",
        "sealed_source": {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": source_manifest_sha256,
        },
        "receipt": {
            "archive_id": "release-terminal.receipt.v1",
            "mode": f"{receipt.mode:04o}",
            "sha256": receipt.sha256,
            "size_bytes": receipt.size,
        },
        "validator": {
            "archive_id": "release-bootstrap.receipt-validator.v1",
            "sha256": validator.sha256,
            "bootstrap_completion_sha256": bootstrap_completion_sha256,
        },
        "invocation": invocation_binding,
        "exit_status": 0,
        "stdout": {
            "sha256": hashlib.sha256(stdout).hexdigest(),
            "size_bytes": len(stdout),
        },
        "stderr": {
            "sha256": hashlib.sha256(b"").hexdigest(),
            "size_bytes": 0,
        },
    }
    _publish_terminal_receipt(ack_path, _canonical_json(value), revalidate=revalidate)


def _receipt_validation_ack_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--validation-ack", type=Path)
    parser.add_argument("--source-manifest-sha256")


def _receipt_validation_ack(
    args: argparse.Namespace,
    snapshots: list[PathContract | DirectoryContract],
) -> None:
    if args.validation_ack is None or args.source_manifest_sha256 is None:
        raise ReceiptError("receipt verification lacks its acknowledgment binding")
    mutable_directory = frozenset({args.validation_ack.parent})
    _publish_receipt_validation_ack(
        ack_path=args.validation_ack,
        receipt_path=args.output,
        release_root=args.release_root,
        source_manifest_sha256=args.source_manifest_sha256,
        bootstrap_completion_sha256=args.expected_bootstrap_completion_sha256,
        revalidate=lambda: _revalidate_receipt_inputs(
            snapshots, ignored_directories=mutable_directory,
        ),
    )


def _owned_unlink_name(
    directory_fd: int, name: str, device: int, inode: int,
) -> bool:
    """Atomically quarantine one exact regular-file inode without unlink races."""

    def renamed(source: str, destination: str, flags: int) -> None:
        library = ctypes.CDLL(None, use_errno=True)
        if sys.platform == "darwin":
            function = library.renameatx_np
        elif sys.platform.startswith("linux") and hasattr(library, "renameat2"):
            function = library.renameat2
        else:
            raise OSError(errno.ENOTSUP, "atomic flagged rename unavailable")
        function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        function.restype = ctypes.c_int
        if function(directory_fd, os.fsencode(source), directory_fd, os.fsencode(destination), flags) != 0:
            number = ctypes.get_errno()
            raise OSError(number, os.strerror(number), destination)

    identity = (device, inode)
    quarantine = f".unlink-quarantine.{secrets.token_hex(16)}"
    try:
        observed = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if not stat.S_ISREG(observed.st_mode) or observed.st_uid != os.geteuid() or observed.st_nlink != 1 or (observed.st_dev, observed.st_ino) != identity:
            return False
        renamed(name, quarantine, 4 if sys.platform == "darwin" else 1)
        moved = os.stat(quarantine, dir_fd=directory_fd, follow_symlinks=False)
        if (moved.st_dev, moved.st_ino) != identity:
            renamed(quarantine, name, 4 if sys.platform == "darwin" else 1)
            return False
        try:
            os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise ReceiptError("owned unlink replacement retained after quarantine")
        os.unlink(quarantine, dir_fd=directory_fd)
        try:
            os.stat(quarantine, dir_fd=directory_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            raise ReceiptError("owned unlink quarantine retained after unlink")
        return True
    except FileNotFoundError:
        return False
    except OSError as error:
        raise ReceiptError("atomic owned unlink failed closed") from error


def _corridor_legs(
    cargo_path: str = "${IROHA_RELEASE_CARGO_BIN}",
) -> list[tuple[str, str, int, str]]:
    legs = [
        (
            leg_id,
            "cargo-focus",
            count,
            _g_unit_leg_command(array_name, package, cargo_target),
        )
        for array_name, leg_id, package, count, cargo_target in _G_UNIT_GROUPS
    ]
    legs.extend(
        (
            (
                leg_id,
                "cargo-module",
                count,
                _production_module_command(module),
            )
            for leg_id, module, count in _PRODUCTION_MODULES
        )
    )
    legs.append(
        (
            "status-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_STATUS_TEST} -- --test-threads=1",
        )
    )
    legs.append(
        (
            "lane-certificate-rust",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p iroha_data_model --lib "
            f"{_DATA_LANE_CERTIFICATE_TEST} -- --exact --test-threads=1",
        )
    )
    legs.extend(
        (
            (
                "source-sealed-workspace-build",
                "command",
                0,
                f"{cargo_path} build -j1 --locked --offline --workspace",
            ),
            (
                "source-sealed-workspace-tests",
                "command",
                0,
                f"{cargo_path} test -j1 --locked --offline --workspace",
            ),
            (
                "source-sealed-irohad-tests",
                "command",
                0,
                f"{cargo_path} test -j1 --locked --offline -p irohad "
                "--lib --features test-network-message-control",
            ),
            (
                "source-sealed-workspace-clippy",
                "command",
                0,
                f"{cargo_path} clippy -j1 --locked --offline --workspace "
                "--all-targets -- -D warnings",
            ),
            (
                "source-sealed-workspace-format",
                "command",
                0,
                f"{cargo_path} fmt --all -- --check",
            ),
            (
                "source-sealed-legacy-codec-guard",
                "command",
                0,
                "bash scripts/check_no_legacy_codec.sh",
            ),
        )
    )
    legs.extend(
        (
            f"taira-contract-{index}",
            "cargo-exact",
            1,
            "cargo test --locked --offline -p integration_tests "
            "--test consensus_and_da "
            f"{test} -- --exact --test-threads=1",
        )
        for index, test in enumerate(_TAIRA_CONTRACT_TESTS)
    )
    legs.append(
        (
            "cross-sdk-rust",
            "cargo-exact",
            2,
            "cargo test --locked --offline -p iroha_data_model --test "
            "iroha_data_model_group_02 sumeragi_v2_cross_sdk_fixtures:: "
            "-- --test-threads=1",
        )
    )
    legs.append(
        (
            "native-amx-rust-fixture-check",
            "command",
            0,
            "regenerate Native AMX Rust fixture authority twice into disjoint "
            "private roots and byte-authenticate both outputs",
        )
    )
    legs.extend(
        (
            f"native-amx-grouped-{surface}",
            "native-amx-sdk",
            count,
            f"bash {_NATIVE_AMX_GROUPED_PARITY_HARNESS} {surface}",
        )
        for surface, count in _NATIVE_AMX_GROUPED_PARITY_SUITES
    )
    legs.append(
        (
            "sumeragi-diagnostics-rust",
            "cargo-exact",
            len(_RUST_SDK_DIAGNOSTICS_TESTS),
            "cargo test --locked --offline -p iroha --lib "
            "client::tests::get_sumeragi_ -- --test-threads=1",
        )
    )
    legs.extend(
        (
            f"sumeragi-diagnostics-{surface}",
            "sdk-diagnostics",
            count,
            f"bash {_SUMERAGI_SDK_DIAGNOSTICS_HARNESS} {surface}",
        )
        for surface, count in _SUMERAGI_SDK_DIAGNOSTICS_SUITES
    )
    legs.extend(
        (
            (
                "preflight-source-seal",
                "pytest",
                78,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/workspace_source_manifest_test.py "
                "pytests/scripts/seal_workspace_source_test.py",
            ),
            (
                "preflight-seed-launcher",
                "pytest",
                14,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_runs_every_exact_scenario_with_one_start_attempt "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_prior_invocation_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_release_profile_uses_32_seeds_per_scenario "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_zero_test_and_preserves_evidence "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_ambiguous_test_summary "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_preserves_cargo_failure_through_tee "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_bundle_tampering_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_marker_durability_failure_is_not_terminal "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_parent_source_manifest_mismatch "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_source_drift_before_completion "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_concurrent_writer_without_clobbering "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_refuses_uninspected_stale_lock "
                "pytests/scripts/sumeragi_v2_seed_matrix_test.py::"
                "test_mocked_seed_matrix_rejects_unsafe_retained_localnet_entries",
            ),
            (
                "preflight-chaos-launcher",
                "pytest",
                5,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_chaos_release_test.py",
            ),
            (
                "preflight-release-identity",
                "pytest",
                75,
                "SUMERAGI_V2_TEST_RELOCATABLE_SSH_KEYGEN_BIN="
                "$IROHA_RELEASE_SSH_KEYGEN_BIN PYTHONDONTWRITEBYTECODE=1 "
                "PYTHONHASHSEED=0 python3 -m pytest -q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_identity_signature_test.py",
            ),
            (
                "preflight-release-bootstrap",
                "pytest",
                258,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_test.py "
                "pytests/scripts/sumeragi_v2_release_bootstrap_cancellation_test.py",
            ),
            (
                "preflight-release-bootstrap-validator",
                "pytest",
                44,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_bootstrap_validator_test.py",
            ),
            (
                "preflight-release-receipt",
                "pytest",
                368,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_release_receipt_test.py "
                "pytests/scripts/sumeragi_v2_release_receipt_components_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_test.py "
                "pytests/scripts/sumeragi_v2_prebuilt_bundle_shell_test.py "
                "pytests/scripts/sumeragi_v2_release_process_policy_test.py",
            ),
            (
                "preflight-multilane-scaling",
                "pytest",
                52,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "scripts/tests/validate_multilane_scaling_evidence_test.py "
                "scripts/tests/run_multilane_scaling_gate_test.py",
            ),
            (
                "preflight-proof-fidelity",
                "pytest",
                5513,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/sumeragi_v2_proof_ledger_test.py "
                "pytests/scripts/sumeragi_v2_verus_evidence_test.py "
                "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py "
                "pytests/scripts/sumeragi_v2_reviewed_rust_source_test.py "
                "pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py "
                "pytests/scripts/sumeragi_v2_multilane_passive_recovery_contract_test.py "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_legacy_layout_only_claim "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_state_order_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
                "test_inflight_composed_contract_rejects_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_terminal_tail_test.py::"
                "test_inflight_composed_contract_rejects_missing_direct_release_action "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_layout_contract_rejects_action_inventory_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_composed_contract_rejects_per_key_prefix_skip_weakening "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_tla_snapshot_nonstutter_mapping "
                "pytests/scripts/sumeragi_v2_multilane_models_tail_test.py::"
                "test_inflight_composed_contract_rejects_verus_snapshot_stutter_proof_removal "
                "pytests/scripts/sumeragi_v2_multilane_models_test.py::"
                "test_inflight_layout_contract_rejects_membership_only_lane_authorship "
                + _WIRE_RELEASE_INVARIANT_PYTEST_NODES,
            ),
            (
                "preflight-formal-launcher",
                "pytest",
                27,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider pytests/scripts/sumeragi_v2_formal_release_test.py",
            ),
            (
                "preflight-taira-soak",
                "pytest",
                43,
                "PYTHONDONTWRITEBYTECODE=1 PYTHONHASHSEED=0 python3 -m pytest "
                "-q -p no:cacheprovider "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_pins_complete_profile_and_runs_exactly_one_test "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_inventory "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_zero_test_execution_output "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_bundle_tampering_before_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_symlinked_marker_temp_without_completion "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_marker_durability_failure_is_not_terminal "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_profile_override_arguments_before_cargo "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_rejects_a_concurrent_source_bound_soak "
                "pytests/scripts/taira_v2_soak_test.py::"
                "test_launcher_does_not_promote_provisional_evidence_when_validation_fails "
                "pytests/scripts/taira_v2_soak_evidence_test.py",
            ),
        )
    )
    return legs
