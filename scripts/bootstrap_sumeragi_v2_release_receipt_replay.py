# Executed lexically in bootstrap_sumeragi_v2_release.py after exact digest authentication.


def _framework_python_input_records(
    records: Any, declared_count: Any, declared_bytes: Any,
) -> list[dict[str, Any]]:
    """Validate a bounded canonical source inventory before path lookup."""

    if not isinstance(records, list) or len(records) > 250_000:
        raise BootstrapError("framework Python source inventory is not bounded")
    schemas = {
        "directory": {
            "path", "kind", "source_device", "source_inode", "source_mode",
            "destination_device", "destination_inode", "destination_mode",
        },
        "file": {
            "path", "kind", "source_device", "source_inode", "source_mode",
            "destination_device", "destination_inode", "destination_mode",
            "size", "sha256",
        },
        "symlink": {
            "path", "kind", "source_mode", "destination_mode", "target",
        },
    }
    paths: list[str] = []
    file_bytes = 0
    for record in records:
        kind = record.get("kind") if isinstance(record, dict) else None
        if kind not in schemas or set(record) != schemas[kind]:
            raise BootstrapError("framework Python source record is not exact")
        path = record["path"]
        if (
            not isinstance(path, str) or not path or path.startswith("/")
            or PurePosixPath(path).as_posix() != path
            or ".." in PurePosixPath(path).parts
        ):
            raise BootstrapError("framework Python source path is unsafe")
        paths.append(path)
        for key in set(record) & {
            "source_device", "source_inode", "destination_device",
            "destination_inode", "size",
        }:
            if type(record[key]) is not int or record[key] < 0:
                raise BootstrapError("framework Python source integer is invalid")
        for key in set(record) & {"source_mode", "destination_mode"}:
            if not isinstance(record[key], str) or re.fullmatch(
                r"[0-7]{4}", record[key]
            ) is None:
                raise BootstrapError("framework Python source mode is invalid")
            if kind != "symlink" and int(record[key], 8) & 0o022:
                raise BootstrapError("framework Python source mode is unsafe")
        if kind == "file":
            if not isinstance(record["sha256"], str) or _DIGEST_RE.fullmatch(
                record["sha256"]
            ) is None:
                raise BootstrapError("framework Python source digest is invalid")
            file_bytes += record["size"]
        elif kind == "symlink" and (
            not isinstance(record["target"], str) or not record["target"]
        ):
            raise BootstrapError("framework Python source symlink is invalid")
    if (
        paths != sorted(paths) or len(set(paths)) != len(paths)
        or type(declared_count) is not int or declared_count != len(records)
        or type(declared_bytes) is not int or declared_bytes != file_bytes
        or file_bytes > 4 * 1024 * 1024 * 1024
    ):
        raise BootstrapError("framework Python source inventory is not exact")
    return records


def _validate_framework_python_macho_closure(
    value: Any,
    source_by_path: dict[str, dict[str, Any]],
    output_by_path: dict[str, dict[str, Any]],
    framework: str,
) -> None:
    """Validate and bind the complete path-free Mach-O transcript."""

    if not isinstance(value, dict) or set(value) != {
        "format", "schema_version", "source_image_count",
        "external_sources", "transforms", "runtime",
    }:
        raise BootstrapError("framework Python Mach-O closure is malformed")
    runtime = value["runtime"]
    if (
        value["format"]
        != "iroha-sumeragi-v2-framework-python-mach-o-transcript"
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
        or type(value["source_image_count"]) is not int
        or not 3 <= value["source_image_count"] <= 4096
        or not isinstance(value["external_sources"], list)
        or not isinstance(value["transforms"], list)
        or not isinstance(runtime, dict)
        or set(runtime) != {"format", "schema_version", "image_count", "images"}
        or runtime["format"] != "iroha-sumeragi-v2-framework-python-mach-o"
        or type(runtime["schema_version"]) is not int
        or runtime["schema_version"] != 1
        or type(runtime["image_count"]) is not int
        or runtime["image_count"] != value["source_image_count"]
        or not isinstance(runtime["images"], list)
        or len(runtime["images"]) != runtime["image_count"]
    ):
        raise BootstrapError("framework Python Mach-O closure is not exact")

    def safe_path(path: Any) -> bool:
        return (
            isinstance(path, str) and bool(path) and not path.startswith("/")
            and PurePosixPath(path).as_posix() == path
            and ".." not in PurePosixPath(path).parts
        )

    images: list[str] = []
    for image in runtime["images"]:
        if not isinstance(image, dict) or set(image) != {
            "path", "size_bytes", "sha256", "slices",
        }:
            raise BootstrapError("framework Python Mach-O image is malformed")
        output = output_by_path.get(image["path"])
        if (
            not safe_path(image["path"])
            or type(image["size_bytes"]) is not int
            or not 0 < image["size_bytes"] <= 256 * 1024 * 1024
            or not isinstance(image["sha256"], str)
            or _DIGEST_RE.fullmatch(image["sha256"]) is None
            or not isinstance(output, dict)
            or output.get("kind") != "file"
            or (output.get("size"), output.get("sha256"))
            != (image["size_bytes"], image["sha256"])
            or not isinstance(image["slices"], list)
            or not 1 <= len(image["slices"]) <= 64
        ):
            raise BootstrapError("framework Python Mach-O image is not bound")
        for slice_value in image["slices"]:
            if not isinstance(slice_value, dict) or set(slice_value) != {
                "cpu_type", "cpu_subtype", "file_type", "dependencies",
                "id_dylib_sha256", "rpath_sha256", "code_signature",
            }:
                raise BootstrapError("framework Python Mach-O slice is malformed")
            if (
                slice_value["code_signature"] != "embedded"
                or any(
                    type(slice_value[field]) is not int
                    or slice_value[field] < 0
                    for field in ("cpu_type", "cpu_subtype", "file_type")
                )
                or not isinstance(slice_value["dependencies"], list)
            ):
                raise BootstrapError("framework Python Mach-O slice is not exact")
            for field in ("id_dylib_sha256", "rpath_sha256"):
                if not isinstance(slice_value[field], list) or any(
                    not isinstance(digest, str)
                    or _DIGEST_RE.fullmatch(digest) is None
                    for digest in slice_value[field]
                ):
                    raise BootstrapError("framework Python Mach-O digest is malformed")
            for dependency in slice_value["dependencies"]:
                binding = dependency.get("binding") if isinstance(dependency, dict) else None
                keys = (
                    {"command", "binding", "install_name_sha256"}
                    if binding == "system"
                    else {"command", "binding", "install_name", "target"}
                    if binding == "archive"
                    else set()
                )
                if (
                    not isinstance(dependency, dict)
                    or set(dependency) != keys
                    or type(dependency.get("command")) is not int
                    or (
                        binding == "system"
                        and (
                            not isinstance(dependency["install_name_sha256"], str)
                            or _DIGEST_RE.fullmatch(
                                dependency["install_name_sha256"]
                            ) is None
                        )
                    )
                    or (
                        binding == "archive"
                        and (
                            not isinstance(dependency["install_name"], str)
                            or not dependency["install_name"].startswith(
                                ("@loader_path/", "@executable_path/")
                            )
                            or not safe_path(dependency["target"])
                        )
                    )
                ):
                    raise BootstrapError("framework Python dependency is malformed")
        images.append(image["path"])
    if images != sorted(images) or len(images) != len(set(images)):
        raise BootstrapError("framework Python Mach-O image order is not exact")
    image_set = set(images)
    for image in runtime["images"]:
        for slice_value in image["slices"]:
            if any(
                dependency["binding"] == "archive"
                and dependency["target"] not in image_set
                for dependency in slice_value["dependencies"]
            ):
                raise BootstrapError("framework Python dependency target is absent")

    external: dict[str, dict[str, Any]] = {}
    external_paths: list[str] = []
    for record in value["external_sources"]:
        if not isinstance(record, dict) or set(record) != {
            "input_path", "path", "mode", "sha256", "size_bytes",
        }:
            raise BootstrapError("external framework dependency is malformed")
        if (
            not safe_path(record["input_path"])
            or not record["input_path"].startswith("mach-o-dependency-sources/")
            or not safe_path(record["path"])
            or "/iroha-loader-deps/" not in f"/{record['path']}"
            or not isinstance(record["mode"], str)
            or re.fullmatch(r"[0-7]{4}", record["mode"]) is None
            or int(record["mode"], 8) & 0o022
            or not int(record["mode"], 8) & 0o444
            or not isinstance(record["sha256"], str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
            or type(record["size_bytes"]) is not int
            or not 0 < record["size_bytes"] <= 256 * 1024 * 1024
        ):
            raise BootstrapError("external framework dependency is unsafe")
        external[record["path"]] = record
        external_paths.append(record["path"])

    transform_paths: list[str] = []
    for transform in value["transforms"]:
        if not isinstance(transform, dict) or set(transform) != {
            "input_path", "path", "source_mode", "source_sha256",
            "source_size_bytes", "derived_mode", "derived_sha256",
            "derived_size_bytes", "operations", "codesign",
        }:
            raise BootstrapError("framework Python Mach-O transform is malformed")
        source = source_by_path.get(transform["input_path"])
        output = output_by_path.get(transform["path"])
        if (
            not safe_path(transform["input_path"])
            or transform["path"] not in image_set
            or not isinstance(transform["source_mode"], str)
            or re.fullmatch(r"[0-7]{4}", transform["source_mode"]) is None
            or int(transform["source_mode"], 8) & 0o022
            or not int(transform["source_mode"], 8) & 0o444
            or transform["derived_mode"] != "0500"
            or transform["codesign"] != "adhoc"
            or any(
                not isinstance(transform[field], str)
                or _DIGEST_RE.fullmatch(transform[field]) is None
                for field in ("source_sha256", "derived_sha256")
            )
            or any(
                type(transform[field]) is not int
                or not 0 < transform[field] <= 256 * 1024 * 1024
                for field in ("source_size_bytes", "derived_size_bytes")
            )
            or not isinstance(source, dict)
            or source.get("kind") != "file"
            or (source.get("source_mode"), source.get("sha256"), source.get("size"))
            != (
                transform["source_mode"], transform["source_sha256"],
                transform["source_size_bytes"],
            )
            or not isinstance(output, dict)
            or output.get("kind") != "file"
            or (output.get("mode"), output.get("sha256"), output.get("size"))
            != (
                transform["derived_mode"], transform["derived_sha256"],
                transform["derived_size_bytes"],
            )
            or not isinstance(transform["operations"], list)
        ):
            raise BootstrapError("framework Python Mach-O transform is not bound")
        for operation in transform["operations"]:
            if (
                not isinstance(operation, dict)
                or set(operation) != {
                    "operation", "source_install_name_sha256", "replacement",
                }
                or operation["operation"] != "change"
                or not isinstance(operation["source_install_name_sha256"], str)
                or _DIGEST_RE.fullmatch(
                    operation["source_install_name_sha256"]
                ) is None
                or not isinstance(operation["replacement"], str)
                or not operation["replacement"].startswith(
                    ("@loader_path/", "@executable_path/")
                )
            ):
                raise BootstrapError("framework Python Mach-O operation is malformed")
        bound = external.get(transform["path"])
        if bound is not None and (
            transform["input_path"], transform["source_mode"],
            transform["source_sha256"], transform["source_size_bytes"],
        ) != (
            bound["input_path"], bound["mode"], bound["sha256"],
            bound["size_bytes"],
        ):
            raise BootstrapError("external framework dependency binding changed")
        transform_paths.append(transform["path"])
    if (
        external_paths != sorted(external_paths)
        or len(external_paths) != len(external)
        or transform_paths != sorted(transform_paths)
        or len(transform_paths) != len(set(transform_paths))
        or not set(external) <= set(transform_paths)
        or not {
            "bin/python3",
            "Resources/Python.app/Contents/MacOS/Python",
        } <= set(transform_paths)
        or not {
            "bin/python3", framework,
            "Resources/Python.app/Contents/MacOS/Python",
        } <= image_set
    ):
        raise BootstrapError("framework Python Mach-O closure is incomplete")


def _framework_python_marker_record(
    inventory_snapshot: FileSnapshot,
) -> dict[str, Any]:
    """Project the relocated private framework into one path-free marker."""

    inventory = _parse_canonical_json(
        inventory_snapshot, "framework Python runtime inventory",
    )
    required = {
        "format", "schema_version", "runtime_root", "record_count",
        "file_bytes", "records", "source_disclosure", "input_record_count",
        "input_file_bytes", "input_records", "relocation",
    }
    if (
        set(inventory) != required
        or inventory["format"]
        != "iroha-sumeragi-v2-private-framework-python-runtime"
        or type(inventory["schema_version"]) is not int
        or inventory["schema_version"] != 2
        or inventory["source_disclosure"] != "withheld"
        or not isinstance(inventory["records"], list)
        or not isinstance(inventory["input_records"], list)
    ):
        raise BootstrapError(
            "framework Python runtime helper returned the wrong inventory"
        )
    input_records = _framework_python_input_records(
        inventory["input_records"], inventory["input_record_count"],
        inventory["input_file_bytes"],
    )
    sanitized: list[dict[str, Any]] = []
    for record in inventory["records"]:
        if not isinstance(record, dict):
            raise BootstrapError(
                "framework Python runtime inventory member is malformed"
            )
        kind = record.get("kind")
        keys = {
            "directory": {"path", "kind", "device", "inode", "mode"},
            "file": {
                "path", "kind", "device", "inode", "mode", "size", "sha256",
            },
            "symlink": {"path", "kind", "mode", "target"},
        }.get(kind)
        path = record.get("path")
        if (
            keys is None
            or set(record) != keys
            or not isinstance(path, str)
            or not path
            or path.startswith("/")
            or ".." in Path(path).parts
            or Path(path).as_posix() != path
            or not isinstance(record.get("mode"), str)
            or re.fullmatch(r"[0-7]{4}", record["mode"]) is None
            or (kind != "symlink" and int(record["mode"], 8) & 0o022)
        ):
            raise BootstrapError(
                "framework Python runtime inventory member is not exact"
            )
        projected = {
            key: record[key]
            for key in (
                ("path", "kind", "mode")
                if kind == "directory"
                else ("path", "kind", "mode", "size", "sha256")
                if kind == "file"
                else ("path", "kind", "mode", "target")
            )
        }
        if (
            kind == "file"
            and (
                type(projected["size"]) is not int
                or projected["size"] < 0
                or not isinstance(projected["sha256"], str)
                or _DIGEST_RE.fullmatch(projected["sha256"]) is None
            )
        ) or (
            kind == "symlink"
            and (
                not isinstance(projected["target"], str)
                or not projected["target"]
            )
        ):
            raise BootstrapError(
                "framework Python runtime inventory member metadata is invalid"
            )
        sanitized.append(projected)
    sanitized.sort(key=lambda record: record["path"])
    file_bytes = sum(
        record["size"] for record in sanitized if record["kind"] == "file"
    )
    if (
        type(inventory["record_count"]) is not int
        or inventory["record_count"] != len(sanitized)
        or type(inventory["file_bytes"]) is not int
        or inventory["file_bytes"] != file_bytes
    ):
        raise BootstrapError(
            "framework Python runtime inventory accounting is not exact"
        )

    relocation = inventory["relocation"]
    if not isinstance(relocation, dict) or set(relocation) != {
        "format", "schema_version", "framework", "tools", "artifacts",
        "closure",
    }:
        raise BootstrapError("framework Python relocation is malformed")
    framework = relocation["framework"]
    if (
        relocation["format"]
        != "iroha-sumeragi-v2-framework-python-relocation"
        or type(relocation["schema_version"]) is not int
        or relocation["schema_version"] != 2
        or not isinstance(framework, str)
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", framework) is None
        or not isinstance(relocation["tools"], dict)
        or set(relocation["tools"])
        != {"codesign", "install_name_tool", "otool"}
        or not isinstance(relocation["artifacts"], dict)
        or set(relocation["artifacts"]) != {"launcher", "trampoline"}
    ):
        raise BootstrapError("framework Python relocation is not exact")
    for name in ("codesign", "install_name_tool", "otool"):
        tool = relocation["tools"][name]
        if (
            not isinstance(tool, dict)
            or set(tool) != {"path", "mode", "sha256", "size_bytes"}
            or tool["path"] != f"/usr/bin/{name}"
            or not isinstance(tool["mode"], str)
            or re.fullmatch(r"[0-7]{4}", tool["mode"]) is None
            or int(tool["mode"], 8) & 0o022
            or not int(tool["mode"], 8) & 0o111
            or not isinstance(tool["sha256"], str)
            or _DIGEST_RE.fullmatch(tool["sha256"]) is None
            or type(tool["size_bytes"]) is not int
            or not 0 < tool["size_bytes"] <= 64 * 1024 * 1024
        ):
            raise BootstrapError("framework Python relocation tool is not exact")
    output_by_path = {record["path"]: record for record in sanitized}
    input_by_path = {
        record.get("path"): record
        for record in input_records
    }
    _validate_framework_python_macho_closure(
        relocation["closure"], input_by_path, output_by_path, framework,
    )
    transform_by_path = {
        transform["path"]: transform
        for transform in relocation["closure"]["transforms"]
    }
    rewrites = {
        "launcher": (
            "python3", "bin/python3", f"@executable_path/../{framework}",
        ),
        "trampoline": (
            "Resources/Python.app/Contents/MacOS/Python",
            "Resources/Python.app/Contents/MacOS/Python",
            f"@executable_path/../../../../{framework}",
        ),
    }
    for name, (input_path, output_path, rewritten) in rewrites.items():
        artifact = relocation["artifacts"][name]
        source = artifact.get("source") if isinstance(artifact, dict) else None
        derived = artifact.get("derived") if isinstance(artifact, dict) else None
        source_record = input_by_path.get(input_path)
        output_record = output_by_path.get(output_path)
        transform = transform_by_path.get(output_path)
        if (
            not isinstance(artifact, dict)
            or set(artifact) != {"path", "source", "derived"}
            or artifact["path"] != output_path
            or not isinstance(source, dict)
            or set(source) != {
                "mode", "sha256", "size_bytes",
                "framework_dependency_sha256", "dependency_vector_sha256",
            }
            or not isinstance(derived, dict)
            or set(derived) != {
                "mode", "sha256", "size_bytes", "framework_dependency",
                "dependency_vector_sha256", "codesign",
            }
            or not isinstance(source["mode"], str)
            or re.fullmatch(r"[0-7]{4}", source["mode"]) is None
            or int(source["mode"], 8) & 0o022
            or not int(source["mode"], 8) & 0o111
            or type(source["size_bytes"]) is not int
            or not 0 < source["size_bytes"] <= 256 * 1024 * 1024
            or type(derived["size_bytes"]) is not int
            or not 0 < derived["size_bytes"] <= 256 * 1024 * 1024
            or derived["mode"] != "0500"
            or derived["framework_dependency"] != rewritten
            or derived["codesign"] != "adhoc"
            or not isinstance(source_record, dict)
            or (
                source_record.get("source_mode"), source_record.get("sha256"),
                source_record.get("size"),
            )
            != (source["mode"], source["sha256"], source["size_bytes"])
            or not isinstance(output_record, dict)
            or output_record.get("kind") != "file"
            or (
                output_record.get("mode"), output_record.get("sha256"),
                output_record.get("size"),
            )
            != (derived["mode"], derived["sha256"], derived["size_bytes"])
            or not isinstance(transform, dict)
            or transform["input_path"] != input_path
            or (
                transform["source_mode"], transform["source_sha256"],
                transform["source_size_bytes"],
            )
            != (source["mode"], source["sha256"], source["size_bytes"])
            or (
                transform["derived_mode"], transform["derived_sha256"],
                transform["derived_size_bytes"],
            )
            != (derived["mode"], derived["sha256"], derived["size_bytes"])
        ):
            raise BootstrapError(
                "framework Python relocation artifact binding is not exact"
            )
        for digest in (
            source["sha256"], source["framework_dependency_sha256"],
            source["dependency_vector_sha256"], derived["sha256"],
            derived["dependency_vector_sha256"],
        ):
            if not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None:
                raise BootstrapError(
                    "framework Python relocation digest is malformed"
                )
        operation_replacements = [
            operation["replacement"]
            for operation in transform["operations"]
        ]
        already_local = source["framework_dependency_sha256"] == hashlib.sha256(
            rewritten.encode("utf-8", "strict")
        ).hexdigest()
        if (
            operation_replacements not in ([], [rewritten])
            or already_local != (operation_replacements == [])
            or (
                operation_replacements
                and transform["operations"][0][
                    "source_install_name_sha256"
                ]
                != source["framework_dependency_sha256"]
            )
            or (
                operation_replacements
                and source["sha256"] == derived["sha256"]
            )
            or (
                operation_replacements
                and source["dependency_vector_sha256"]
                == derived["dependency_vector_sha256"]
            )
        ):
            raise BootstrapError("framework Python relocation derivation is wrong")
    if output_by_path.get(framework, {}).get("kind") != "file":
        raise BootstrapError("framework Python relocation names the wrong framework")
    return {
        "format": "iroha-sumeragi-v2-framework-python-runtime",
        "schema_version": 2,
        "archive_root": "python-runtime",
        "root_mode": "0500",
        "executable": "bin/python3",
        "inventory": {
            "archive_name": "python-runtime-input.json",
            "mode": f"{inventory_snapshot.mode:04o}",
            "sha256": inventory_snapshot.sha256,
            "size_bytes": inventory_snapshot.size,
        },
        "record_count": len(sanitized),
        "file_bytes": file_bytes,
        "records": sanitized,
        "relocation": relocation,
    }

def _validate_terminal_release_evidence(
    *,
    receipt_evidence: dict[str, Any],
    evidence: Path,
    release_root: Path,
    receipt_identity: dict[str, Any],
    runner_record: dict[str, Any],
    authenticated_environment: dict[str, str],
) -> tuple[list[LargeFileSnapshot], list[DirectorySnapshot]]:
    """Validate and freeze every newly protected terminal-evidence input."""

    artifact_snapshots: list[LargeFileSnapshot] = []
    directory_snapshots: list[DirectorySnapshot] = []
    artifact_paths: dict[Path, str] = {}
    artifact_inodes: dict[tuple[int, int], str] = {}
    directories: dict[Path, DirectorySnapshot] = {}
    directory_inodes: dict[tuple[int, int], str] = {}
    if hashlib.sha256(_canonical_json(authenticated_environment)).hexdigest() != runner_record.get(
        "environment_sha256"
    ):
        raise BootstrapError("terminal runner environment digest is not exact")

    def capture_directory(
        path: Path,
        label: str,
        *,
        containment_root: Path = evidence,
        expected_mode: int | None = None,
    ) -> DirectorySnapshot:
        existing = directories.get(path)
        if existing is not None:
            if expected_mode is not None and existing.mode != expected_mode:
                raise BootstrapError(f"{label} has the wrong mode")
            return existing
        snapshot = _terminal_directory_snapshot(path, label)
        if not _inside(snapshot.path, containment_root):
            raise BootstrapError(f"{label} escaped its authenticated containment root")
        if expected_mode is not None and snapshot.mode != expected_mode:
            raise BootstrapError(f"{label} has the wrong mode")
        inode = (snapshot.device, snapshot.inode)
        alias = directory_inodes.get(inode)
        if alias is not None:
            raise BootstrapError(f"terminal evidence directories alias: {alias} and {label}")
        directory_inodes[inode] = label
        directories[snapshot.path] = snapshot
        directory_snapshots.append(snapshot)
        return snapshot

    def capture_artifact(
        record: Any,
        label: str,
        *,
        full: bool,
        extra_fields: frozenset[str] = frozenset(),
        expected_path: Path | None = None,
        maximum_bytes: int = _MAX_TERMINAL_ARTIFACT_BYTES,
        expected_mode: int | None = None,
        containment_root: Path = evidence,
    ) -> LargeFileSnapshot:
        expected_fields = (
            _TERMINAL_FULL_ARTIFACT_KEYS
            if full
            else _TERMINAL_SIMPLE_ARTIFACT_KEYS
        ) | set(extra_fields)
        record = _require_exact_json_fields(record, expected_fields, label)
        rendered = record["path"]
        digest = record["sha256"]
        if not isinstance(rendered, str) or not isinstance(digest, str):
            raise BootstrapError(f"{label} path or digest is not text")
        _require_digest(digest, f"{label} digest")
        path = _absolute_resolved_existing(Path(rendered), label)
        if not _inside(path, containment_root):
            raise BootstrapError(f"{label} escaped its authenticated containment root")
        if expected_path is not None and path != expected_path:
            raise BootstrapError(f"{label} has the wrong contained path")
        alias = artifact_paths.get(path)
        if alias is not None:
            raise BootstrapError(f"terminal evidence path is multiply carried: {alias} and {label}")
        snapshot = _capture_large_file(path, label)
        if snapshot.size > maximum_bytes:
            raise BootstrapError(f"{label} exceeds its closed size limit")
        if snapshot.sha256 != digest:
            raise BootstrapError(f"{label} changed: digest does not match its bytes")
        if snapshot.owner != os.getuid() or snapshot.nlink != 1:
            raise BootstrapError(f"{label} must be owner-owned and single-link")
        if expected_mode is not None and snapshot.mode != expected_mode:
            raise BootstrapError(f"{label} has the wrong mode")
        if full:
            size = record["size_bytes"]
            owner = record["owner_uid"]
            nlink = record["nlink"]
            if (
                type(size) is not int
                or size < 0
                or type(owner) is not int
                or owner < 0
                or type(nlink) is not int
                or nlink < 1
                or size != snapshot.size
                or _terminal_mode(record["mode"], label) != snapshot.mode
                or owner != snapshot.owner
                or nlink != snapshot.nlink
            ):
                raise BootstrapError(f"{label} metadata does not match its file")
        inode = (snapshot.device, snapshot.inode)
        inode_alias = artifact_inodes.get(inode)
        if inode_alias is not None:
            raise BootstrapError(
                f"terminal evidence files are inode aliases: {inode_alias} and {label}"
            )
        artifact_paths[path] = label
        artifact_inodes[inode] = label
        artifact_snapshots.append(snapshot)
        capture_directory(
            snapshot.path.parent,
            f"{label} parent directory",
            containment_root=containment_root,
        )
        return snapshot

    def capture_archive(
        record: Any,
        label: str,
        *,
        archive_id: str,
        expected_path: Path,
        maximum_bytes: int = _MAX_TERMINAL_ARTIFACT_BYTES,
        expected_mode: int | None = None,
        containment_root: Path = evidence,
        extra_fields: frozenset[str] = frozenset(),
    ) -> LargeFileSnapshot:
        record = _require_exact_json_fields(
            record,
            {"archive_id", "mode", "sha256", "size_bytes"} | set(extra_fields),
            label,
        )
        if record["archive_id"] != archive_id:
            raise BootstrapError(f"{label} has the wrong archive id")
        snapshot = _capture_large_file(expected_path, label)
        if (
            not _inside(snapshot.path, containment_root)
            or snapshot.size > maximum_bytes
            or snapshot.owner != os.getuid()
            or snapshot.nlink != 1
            or record["sha256"] != snapshot.sha256
            or type(record["size_bytes"]) is not int
            or record["size_bytes"] != snapshot.size
            or _terminal_mode(record["mode"], label) != snapshot.mode
            or (expected_mode is not None and snapshot.mode != expected_mode)
        ):
            raise BootstrapError(
                f"{label} changed or its archive metadata is not exact"
            )
        inode = (snapshot.device, snapshot.inode)
        if snapshot.path in artifact_paths or inode in artifact_inodes:
            raise BootstrapError(f"{label} aliases another terminal artifact")
        artifact_paths[snapshot.path] = label
        artifact_inodes[inode] = label
        artifact_snapshots.append(snapshot)
        capture_directory(
            snapshot.path.parent,
            f"{label} parent directory",
            containment_root=containment_root,
        )
        return snapshot

    def require_inventory(
        path: Path,
        expected: set[str],
        label: str,
        *,
        containment_root: Path = evidence,
        expected_mode: int | None = None,
    ) -> None:
        capture_directory(
            path,
            label,
            containment_root=containment_root,
            expected_mode=expected_mode,
        )
        try:
            with os.scandir(path) as iterator:
                entries = list(iterator)
        except OSError as error:
            raise BootstrapError(f"{label} cannot be enumerated") from error
        if {entry.name for entry in entries} != expected:
            raise BootstrapError(
                f"{label} changed or has the wrong closed inventory"
            )
        for entry in entries:
            try:
                metadata = entry.stat(follow_symlinks=False)
            except OSError as error:
                raise BootstrapError(f"{label} entry is unavailable") from error
            if stat.S_ISLNK(metadata.st_mode) or not (
                stat.S_ISREG(metadata.st_mode) or stat.S_ISDIR(metadata.st_mode)
            ):
                raise BootstrapError(f"{label} contains an unsafe entry")

    formal_replay = _require_exact_json_fields(
        receipt_evidence["formal_replay_release"],
        {
            "schema_version",
            "scheme",
            "provider",
            "namespace",
            "principal",
            "signer_fingerprint",
            "source_artifacts",
            "source_receipt",
            "receipt",
            "signature",
            "ssh_keygen",
            "allowed_signers",
            "revocation",
            "attestation",
        },
        "terminal formal replay release",
    )
    replay_environment = {
        "source_receipt": authenticated_environment.get(
            "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT"
        ),
        "release_root": authenticated_environment.get(
            "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT"
        ),
        "signature_sha256": authenticated_environment.get(
            "IROHA_RELEASE_FORMAL_REPLAY_SIGNATURE_SHA256"
        ),
        "principal": authenticated_environment.get(
            "IROHA_RELEASE_FORMAL_REPLAY_SIGNER_PRINCIPAL"
        ),
        "signer_fingerprint": authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_EXPECTED_SIGNER_FINGERPRINT"
        ),
        "ssh_keygen_sha256": authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_KEYGEN_SHA256"
        ),
        "allowed_signers_sha256": authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_ALLOWED_SIGNERS_SHA256"
        ),
        "revocation_sha256": authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_EXPECTED_SSH_REVOCATION_SHA256"
        ),
    }
    if (
        not all(
            isinstance(value, str) and value
            for value in replay_environment.values()
        )
        or type(formal_replay["schema_version"]) is not int
        or not all(
            isinstance(formal_replay[field], str)
            for field in (
                "scheme",
                "provider",
                "namespace",
                "principal",
                "signer_fingerprint",
            )
        )
        or formal_replay["schema_version"] != 1
        or formal_replay["scheme"] != "detached-ssh"
        or formal_replay["provider"] != "openssh-sshsig"
        or formal_replay["namespace"]
        != "iroha-sumeragi-v2-replay-receipt-v1"
        or formal_replay["principal"] != replay_environment["principal"]
        or formal_replay["signer_fingerprint"]
        != replay_environment["signer_fingerprint"]
        or _FINGERPRINT_RE.fullmatch(formal_replay["signer_fingerprint"])
        is None
    ):
        raise BootstrapError(
            "terminal formal replay signing identity is not authenticated"
        )

    source_receipt = capture_artifact(
        formal_replay["source_receipt"],
        "terminal formal replay source receipt",
        full=True,
        maximum_bytes=128 * 1024 * 1024,
        expected_mode=0o600,
        containment_root=Path(str(replay_environment["source_receipt"])).parent,
    )
    if str(source_receipt.path) != replay_environment["source_receipt"]:
        raise BootstrapError(
            "terminal formal replay source receipt is not the authenticated input"
        )
    source_records = formal_replay["source_artifacts"]
    expected_source_names = {
        "01-standalone_sany.stdout",
        "01-standalone_sany.stderr",
        "02-raw_tlc.stdout",
        "02-raw_tlc.stderr",
        "03-normalizer.stdout",
        "03-normalizer.stderr",
    }
    if not isinstance(source_records, list) or len(source_records) != len(
        expected_source_names
    ):
        raise BootstrapError(
            "terminal formal replay source artifact inventory is incomplete"
        )
    source_event_names: list[str] = []
    for index, record in enumerate(source_records):
        snapshot = capture_artifact(
            record,
            f"terminal formal replay source artifact {index}",
            full=True,
            maximum_bytes=256 * 1024 * 1024,
            expected_mode=0o600,
            containment_root=source_receipt.path.parent,
        )
        if snapshot.path.parent != source_receipt.path.parent / "events":
            raise BootstrapError(
                "terminal formal replay source artifact escaped events"
            )
        source_event_names.append(snapshot.path.name)
    if source_event_names != sorted(expected_source_names):
        raise BootstrapError(
            "terminal formal replay source artifacts are not exact and sorted"
        )
    require_inventory(
        source_receipt.path.parent,
        {"receipt.json", "events"},
        "terminal formal replay source root",
        containment_root=source_receipt.path.parent,
        expected_mode=0o700,
    )
    require_inventory(
        source_receipt.path.parent / "events",
        expected_source_names,
        "terminal formal replay source events",
        containment_root=source_receipt.path.parent,
        expected_mode=0o700,
    )

    release_receipt_record = formal_replay["receipt"]
    if not isinstance(release_receipt_record, dict) or not isinstance(
        release_receipt_record.get("path"), str
    ):
        raise BootstrapError("terminal formal replay finalized receipt is malformed")
    replay_release_root = Path(release_receipt_record["path"]).parent
    if str(replay_release_root) != replay_environment["release_root"]:
        raise BootstrapError(
            "terminal formal replay release root is not the authenticated input"
        )
    finalized_specs = (
        ("receipt", "receipt.json", 0o400, 128 * 1024 * 1024),
        ("signature", "receipt.json.sig", 0o400, 16 * 1024 * 1024),
        ("ssh_keygen", "ssh-keygen.release-tool", 0o500, 512 * 1024 * 1024),
        ("allowed_signers", "allowed_signers", 0o400, 16 * 1024 * 1024),
        ("revocation", "revocation.krl", 0o400, 16 * 1024 * 1024),
        (
            "attestation",
            "release-attestation.json",
            0o400,
            128 * 1024 * 1024,
        ),
    )
    finalized: dict[str, LargeFileSnapshot] = {}
    for field, filename, mode, maximum_bytes in finalized_specs:
        finalized[field] = capture_artifact(
            formal_replay[field],
            f"terminal formal replay finalized {field}",
            full=True,
            expected_path=replay_release_root / filename,
            maximum_bytes=maximum_bytes,
            expected_mode=mode,
            containment_root=replay_release_root,
        )
    if (
        finalized["receipt"].sha256 != source_receipt.sha256
        or finalized["receipt"].size != source_receipt.size
        or finalized["signature"].sha256
        != replay_environment["signature_sha256"]
        or finalized["ssh_keygen"].sha256
        != replay_environment["ssh_keygen_sha256"]
        or finalized["allowed_signers"].sha256
        != replay_environment["allowed_signers_sha256"]
        or finalized["revocation"].sha256
        != replay_environment["revocation_sha256"]
    ):
        raise BootstrapError(
            "terminal formal replay bundle differs from authenticated policy"
        )
    require_inventory(
        replay_release_root,
        {filename for _field, filename, _mode, _maximum in finalized_specs},
        "terminal formal replay finalized root",
        containment_root=replay_release_root,
        expected_mode=0o700,
    )

    simple_specs = (
        (
            "g_unit_focused_test_inventory",
            "g-unit-required-tests.tsv",
            "corridor_completion",
            16 * 1024 * 1024,
        ),
        (
            "formal_multilane_apalache_evidence",
            "multilane_apalache_evidence.tsv",
            "formal_completion",
            16 * 1024 * 1024,
        ),
        (
            "formal_tlaps_resource_jsonl",
            "tlaps_resource.jsonl",
            "formal_completion",
            256 * 1024 * 1024,
        ),
        (
            "formal_tlaps_resource_summary",
            "tlaps_resource_summary.json",
            "formal_completion",
            128 * 1024 * 1024,
        ),
    )
    for label, filename, family_completion, maximum_bytes in simple_specs:
        family = receipt_evidence[family_completion]
        if not isinstance(family, dict) or not isinstance(family.get("path"), str):
            raise BootstrapError(f"terminal release evidence {family_completion} is malformed")
        family_path = _absolute_resolved_existing(
            Path(family["path"]), f"terminal receipt {family_completion}"
        )
        capture_artifact(
            receipt_evidence[label],
            f"terminal receipt {label}",
            full=False,
            expected_path=family_path.with_name(filename),
            maximum_bytes=maximum_bytes,
        )
        capture_directory(family_path.parent, f"terminal {label} family directory")

    cargo_cache = _require_exact_json_fields(
        receipt_evidence["cargo_cache_input"],
        {
            "schema_version",
            "inventory",
            "final_inventory",
            "runtime_inventory",
            "runtime_environment_sha256",
            "runtime_directories",
            "cargo_home",
            "source_cargo_home_disclosure",
            "input_root_count",
            "input_record_count",
            "input_file_count",
        },
        "terminal Cargo-cache authentication",
    )
    if (
        type(cargo_cache["schema_version"]) is not int
        or cargo_cache["schema_version"] != 2
        or cargo_cache["source_cargo_home_disclosure"] != "withheld"
        or any(
            type(cargo_cache[name]) is not int or cargo_cache[name] < 0
            for name in ("input_root_count", "input_record_count", "input_file_count")
        )
    ):
        raise BootstrapError("terminal Cargo-cache authentication is malformed")
    cargo_file_specs = (
        (
            "inventory",
            "release-cargo-cache.input-inventory.v1",
            evidence / "cargo-cache-input.json",
        ),
        (
            "final_inventory",
            "release-cargo-cache.final-inventory.v1",
            evidence / "cargo-cache-final.json",
        ),
        (
            "runtime_inventory",
            "release-runtime.inventory.v1",
            evidence.parent / "runtime-input.json",
        ),
    )
    for field, archive_id, expected_path in cargo_file_specs:
        capture_archive(
            cargo_cache[field],
            f"terminal Cargo-cache {field}",
            archive_id=archive_id,
            expected_path=expected_path,
            maximum_bytes=16 * 1024 * 1024,
            expected_mode=_DATA_MODE,
            containment_root=evidence.parent,
        )
    if (
        receipt_evidence["cargo_cache_input_inventory"] != cargo_cache["inventory"]
        or receipt_evidence["cargo_cache_final_inventory"]
        != cargo_cache["final_inventory"]
    ):
        raise BootstrapError("terminal Cargo-cache inventory aliases disagree")
    expected_runtime_environment = {
        "runtime_home_path": str(evidence / "home"),
        "runtime_tmpdir_path": str(evidence / "tmp"),
        "runtime_tmp_path": str(evidence / "tmp"),
        "runtime_temp_path": str(evidence / "tmp"),
        "runtime_cache_path": str(evidence / "cache"),
    }
    if cargo_cache["runtime_environment_sha256"] != hashlib.sha256(
        _canonical_json(expected_runtime_environment)
    ).hexdigest():
        raise BootstrapError("terminal runtime environment digest is not exact")
    runtime_directories = _require_exact_json_fields(
        cargo_cache["runtime_directories"],
        {"cache", "home", "tmp"},
        "terminal runtime directories",
    )
    for name in ("cache", "home", "tmp"):
        if runtime_directories[name] != {
            "archive_id": f"release-runtime.directory.{name}.v1",
            "mode": "0700",
        }:
            raise BootstrapError(
                f"terminal runtime directory {name} authentication is malformed"
            )
    if cargo_cache["cargo_home"] != {
        "archive_id": "release-cargo-cache.home.v1",
        "mode": "0700",
    }:
        raise BootstrapError("terminal Cargo home authentication is malformed")

    sdk = _require_exact_json_fields(
        receipt_evidence["sdk_dependencies"],
        {
            "schema_version", "source_disclosure", "source_manifest_sha256",
            "source_state_sha256", "archive", "input_inventory",
            "final_work_inventory",
        },
        "terminal SDK dependencies",
    )
    if (
        type(sdk["schema_version"]) is not int
        or sdk["schema_version"] != 1
        or sdk["source_disclosure"] != "withheld"
        or not isinstance(sdk["source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(sdk["source_manifest_sha256"]) is None
        or not isinstance(sdk["source_state_sha256"], str)
        or _DIGEST_RE.fullmatch(sdk["source_state_sha256"]) is None
    ):
        raise BootstrapError("terminal SDK dependency identity is malformed")
    try:
        sdk_manifest_record = receipt_evidence["bootstrap"]["trusted_inputs"][
            "sdk_dependency_bundle_manifest"
        ]
    except (KeyError, TypeError) as error:
        raise BootstrapError(
            "terminal SDK dependency manifest authentication is absent"
        ) from error
    if (
        not isinstance(sdk_manifest_record, dict)
        or sdk_manifest_record.get("sha256") != sdk["source_manifest_sha256"]
    ):
        raise BootstrapError(
            "terminal SDK dependency manifest digest is not bootstrap-bound"
        )
    sdk_root = release_root.parent
    sdk_specs = (
        (
            "archive", "release-sdk-dependencies.bundle.v1",
            "sdk-dependency-bundle.tar", _MAX_RETAINED_TOTAL_BYTES,
        ),
        (
            "input_inventory", "release-sdk-dependencies.input-inventory.v1",
            "sdk-dependency-input.json", 256 * 1024 * 1024,
        ),
        (
            "final_work_inventory", "release-sdk-dependencies.work-final.v1",
            "sdk-dependency-work-final.json", 256 * 1024 * 1024,
        ),
    )
    sdk_snapshots: dict[str, LargeFileSnapshot] = {}
    for field, archive_id, archive_name, maximum_bytes in sdk_specs:
        record = _require_exact_json_fields(
            sdk[field],
            {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
            f"terminal SDK {field}",
        )
        if record["archive_name"] != archive_name:
            raise BootstrapError(f"terminal SDK {field} archive name is not exact")
        sdk_snapshots[field] = capture_archive(
            record,
            f"terminal SDK {field}",
            archive_id=archive_id,
            expected_path=sdk_root / archive_name,
            maximum_bytes=maximum_bytes,
            expected_mode=_DATA_MODE,
            containment_root=sdk_root,
            extra_fields=frozenset({"archive_name"}),
        )

    def sdk_document(field: str, label: str) -> dict[str, Any]:
        snapshot = _read_file(
            sdk_snapshots[field].path,
            label,
            maximum_bytes=256 * 1024 * 1024,
        )
        if snapshot.sha256 != sdk_snapshots[field].sha256:
            raise BootstrapError(f"{label} changed before semantic replay")
        return _parse_canonical_json(snapshot, label)

    def sdk_records(
        value: Any, label: str, root_mode: str,
    ) -> tuple[list[dict[str, Any]], int]:
        if not isinstance(value, list) or not value or len(value) > _MAX_RETAINED_RECORDS:
            raise BootstrapError(f"{label} record inventory is not bounded")
        paths: list[str] = []
        by_path: dict[str, dict[str, Any]] = {}
        file_bytes = 0
        for index, record in enumerate(value):
            if not isinstance(record, dict):
                raise BootstrapError(f"{label} record {index} is malformed")
            kind = record.get("kind")
            expected_fields = {
                "directory": {"path", "kind", "mode"},
                "file": {"path", "kind", "mode", "size", "sha256"},
                "symlink": {"path", "kind", "mode", "target"},
            }.get(kind)
            path = record.get("path")
            if (
                expected_fields is None
                or set(record) != expected_fields
                or not isinstance(path, str)
                or (path != "." and (
                    PurePosixPath(path).is_absolute()
                    or PurePosixPath(path).as_posix() != path
                    or not PurePosixPath(path).parts
                    or any(part in {"", ".", ".."} for part in PurePosixPath(path).parts)
                ))
                or not isinstance(record.get("mode"), str)
                or re.fullmatch(r"[0-7]{4}", record["mode"]) is None
            ):
                raise BootstrapError(f"{label} record {index} is not path-free")
            if kind == "file":
                size = record["size"]
                if (
                    type(size) is not int
                    or not 0 <= size <= _MAX_RETAINED_FILE_BYTES
                    or not isinstance(record["sha256"], str)
                    or _DIGEST_RE.fullmatch(record["sha256"]) is None
                ):
                    raise BootstrapError(f"{label} file record is malformed")
                file_bytes += size
                if file_bytes > _MAX_RETAINED_TOTAL_BYTES:
                    raise BootstrapError(f"{label} file bytes exceed their bound")
            elif kind == "symlink":
                target = record["target"]
                if (
                    not isinstance(target, str)
                    or "\0" in target
                    or PurePosixPath(target).is_absolute()
                    or PurePosixPath(target).as_posix() != target
                ):
                    raise BootstrapError(f"{label} symlink target is unsafe")
            paths.append(path)
            by_path[path] = record
        if (
            value[0] != {"path": ".", "kind": "directory", "mode": root_mode}
            or len(set(paths)) != len(paths)
            or paths[1:] != sorted(paths[1:])
        ):
            raise BootstrapError(f"{label} ordering or root is not exact")
        for path, record in by_path.items():
            if path == ".":
                continue
            pure = PurePosixPath(path)
            parent = pure.parent.as_posix()
            if by_path.get(parent, {}).get("kind") != "directory":
                raise BootstrapError(f"{label} member lacks its exact parent")
            if record["kind"] == "symlink":
                parts = list(pure.parent.parts) if parent != "." else []
                for part in PurePosixPath(record["target"]).parts:
                    if part in {"", "."}:
                        continue
                    if part == "..":
                        if not parts:
                            raise BootstrapError(f"{label} symlink escapes its root")
                        parts.pop()
                    else:
                        parts.append(part)
                if ("/".join(parts) or ".") not in by_path:
                    raise BootstrapError(f"{label} symlink target is not inventoried")
        return value, file_bytes

    input_document = _require_exact_json_fields(
        sdk_document("input_inventory", "terminal SDK input inventory"),
        {
            "format", "schema_version", "archive_id", "source_disclosure",
            "source_manifest_sha256", "source_state_sha256", "bindings",
            "archive", "record_count", "file_bytes", "records",
            "work_initial_record_count", "work_initial_file_bytes",
            "work_initial_records",
        },
        "terminal SDK input inventory",
    )
    input_records, input_bytes = sdk_records(
        input_document["records"], "terminal SDK input inventory", "0500"
    )
    initial_records, initial_bytes = sdk_records(
        input_document["work_initial_records"],
        "terminal SDK initial-work inventory", "0700",
    )
    if (
        input_document["format"] != "iroha-sumeragi-v2-sdk-dependency-bundle"
        or input_document["schema_version"] != 1
        or input_document["archive_id"] != "release-sdk-dependencies.bundle.v1"
        or input_document["source_disclosure"] != "withheld"
        or input_document["source_manifest_sha256"] != sdk["source_manifest_sha256"]
        or input_document["source_state_sha256"] != sdk["source_state_sha256"]
        or input_document["archive"] != sdk["archive"]
        or input_document["record_count"] != len(input_records)
        or input_document["file_bytes"] != input_bytes
        or input_document["work_initial_record_count"] != len(initial_records)
        or input_document["work_initial_file_bytes"] != initial_bytes
        or not isinstance(input_document["bindings"], dict)
    ):
        raise BootstrapError("terminal SDK input inventory binding is not exact")
    input_by_path = {str(record["path"]): record for record in input_records}
    bindings = _require_exact_json_fields(
        input_document["bindings"],
        {"node", "openapi_node", "swiftpm", "gradle"},
        "terminal SDK dependency bindings",
    )
    node_binding = _require_exact_json_fields(
        bindings["node"],
        {
            "node_modules_archive_name", "package_lock_archive_name",
            "package_lock_sha256", "installed_lock_sha256",
        },
        "terminal SDK Node binding",
    )
    openapi_node_binding = _require_exact_json_fields(
        bindings["openapi_node"],
        {
            "node_modules_archive_name", "package_lock_archive_name",
            "package_lock_sha256", "installed_lock_sha256",
        },
        "terminal SDK OpenAPI Node binding",
    )
    swift_binding = _require_exact_json_fields(
        bindings["swiftpm"],
        {
            "cache_archive_name", "package_resolved_archive_name",
            "package_resolved_sha256", "resolved_revisions",
        },
        "terminal SDK SwiftPM binding",
    )
    gradle_binding = _require_exact_json_fields(
        bindings["gradle"],
        {
            "distribution_archive_name", "distribution_sha256",
            "distribution_url", "gradle_user_home_archive_name",
            "launcher_archive_name", "wrapper_cache_key", "version",
            "wrapper_properties_sha256",
        },
        "terminal SDK Gradle binding",
    )
    wrapper_digests = _require_exact_json_fields(
        gradle_binding["wrapper_properties_sha256"], {"java", "kotlin"},
        "terminal SDK Gradle wrapper digests",
    )
    gradle_url = (
        "https://services.gradle.org/distributions/gradle-9.3.0-bin.zip"
    )
    gradle_key = "79n14ral3mx1ozqr3csh2u872"
    gradle_launcher = (
        "gradle/gradle-user-home/wrapper/dists/gradle-9.3.0-bin/"
        f"{gradle_key}/gradle-9.3.0/bin/gradle"
    )
    if (
        node_binding["node_modules_archive_name"] != "node/node_modules"
        or node_binding["package_lock_archive_name"] != "node/package-lock.json"
        or openapi_node_binding["node_modules_archive_name"]
        != "openapi/node_modules"
        or openapi_node_binding["package_lock_archive_name"]
        != "openapi/package-lock.json"
        or swift_binding["cache_archive_name"] != "swiftpm/cache"
        or swift_binding["package_resolved_archive_name"]
        != "swiftpm/Package.resolved"
        or gradle_binding["distribution_archive_name"]
        != "gradle/gradle-9.3.0-bin.zip"
        or gradle_binding["distribution_url"] != gradle_url
        or gradle_binding["gradle_user_home_archive_name"]
        != "gradle/gradle-user-home"
        or gradle_binding["launcher_archive_name"] != gradle_launcher
        or gradle_binding["wrapper_cache_key"] != gradle_key
        or gradle_binding["version"] != "9.3.0"
    ):
        raise BootstrapError("terminal SDK path-free bindings are not exact")
    for digest, label in (
        (node_binding["package_lock_sha256"], "terminal SDK package lock"),
        (node_binding["installed_lock_sha256"], "terminal SDK installed lock"),
        (
            openapi_node_binding["package_lock_sha256"],
            "terminal SDK OpenAPI package lock",
        ),
        (
            openapi_node_binding["installed_lock_sha256"],
            "terminal SDK installed OpenAPI lock",
        ),
        (swift_binding["package_resolved_sha256"], "terminal SDK Package.resolved"),
        (gradle_binding["distribution_sha256"], "terminal SDK Gradle distribution"),
        (wrapper_digests["java"], "terminal SDK Java wrapper"),
        (wrapper_digests["kotlin"], "terminal SDK Kotlin wrapper"),
    ):
        _require_digest(digest, label)
    revisions = swift_binding["resolved_revisions"]
    if not isinstance(revisions, list) or not revisions:
        raise BootstrapError("terminal SDK SwiftPM revisions are absent")
    revision_identities: list[str] = []
    for item in revisions:
        item = _require_exact_json_fields(
            item, {"identity", "checkout", "revision", "tree"},
            "terminal SDK SwiftPM revision",
        )
        if (
            not isinstance(item["identity"], str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", item["identity"])
            is None
            or not isinstance(item["checkout"], str)
            or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", item["checkout"])
            is None
            or not isinstance(item["revision"], str)
            or _OBJECT_ID_RE.fullmatch(item["revision"]) is None
            or not isinstance(item["tree"], str)
            or _OBJECT_ID_RE.fullmatch(item["tree"]) is None
        ):
            raise BootstrapError("terminal SDK SwiftPM revision is malformed")
        revision_identities.append(item["identity"])
    if revision_identities != sorted(set(revision_identities)):
        raise BootstrapError("terminal SDK SwiftPM revisions are not unique")
    expected_file_digests = {
        "node/package-lock.json": node_binding["package_lock_sha256"],
        "node/node_modules/.package-lock.json": node_binding["installed_lock_sha256"],
        "openapi/package-lock.json": openapi_node_binding["package_lock_sha256"],
        "openapi/node_modules/.package-lock.json": openapi_node_binding[
            "installed_lock_sha256"
        ],
        "swiftpm/Package.resolved": swift_binding["package_resolved_sha256"],
        "gradle/gradle-9.3.0-bin.zip": gradle_binding["distribution_sha256"],
        "gradle/java-gradle-wrapper.properties": wrapper_digests["java"],
        "gradle/kotlin-gradle-wrapper.properties": wrapper_digests["kotlin"],
    }
    if any(
        input_by_path.get(path, {}).get("kind") != "file"
        or input_by_path[path].get("sha256") != digest
        for path, digest in expected_file_digests.items()
    ) or any(
        input_by_path.get(path, {}).get("kind") != "directory"
        for path in (
            "node/node_modules", "openapi/node_modules", "swiftpm/cache",
            "swiftpm/cache/checkouts",
            "swiftpm/cache/repositories", "gradle/gradle-user-home",
        )
    ) or (
        input_by_path.get(gradle_launcher, {}).get("kind") != "file"
        or int(str(input_by_path[gradle_launcher]["mode"]), 8) & 0o111 == 0
    ):
        raise BootstrapError("terminal SDK bindings do not match retained members")

    private_source_manifest = evidence / "sdk-dependency-bundle-manifest.json"
    if os.path.lexists(private_source_manifest):
        raise BootstrapError(
            "bootstrap-private SDK source manifest survived acknowledgment pruning"
        )
    def retained_source_inventory(prefix: str) -> dict[str, Any]:
        records: list[dict[str, Any]] = []
        for retained in input_records:
            retained_path = str(retained["path"])
            if retained_path != prefix and not retained_path.startswith(prefix + "/"):
                continue
            record = dict(retained)
            record["path"] = (
                "."
                if retained_path == prefix
                else retained_path.removeprefix(prefix + "/")
            )
            if record["kind"] == "directory":
                record["mode"] = "0700"
            elif record["kind"] == "file":
                record["mode"] = (
                    "0700" if int(str(record["mode"]), 8) & 0o111 else "0600"
                )
            records.append(record)
        records.sort(key=lambda item: str(item["path"]))
        payload = json.dumps(
            records, ensure_ascii=True, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
        return {
            "format": "iroha-sumeragi-v2-sdk-dependency-source-inventory",
            "schema_version": 1,
            "record_count": len(records),
            "file_bytes": sum(
                int(record.get("size", 0))
                for record in records
                if record["kind"] == "file"
            ),
            "records_sha256": hashlib.sha256(payload).hexdigest(),
            "records": records,
        }

    source_node = {
        "node_modules_inventory": retained_source_inventory("node/node_modules"),
        "package_lock_sha256": node_binding["package_lock_sha256"],
    }
    source_openapi_node = {
        "node_modules_inventory": retained_source_inventory(
            "openapi/node_modules"
        ),
        "package_lock_sha256": openapi_node_binding["package_lock_sha256"],
    }
    source_swift = {
        "cache_inventory": retained_source_inventory("swiftpm/cache"),
        "package_resolved_sha256": swift_binding["package_resolved_sha256"],
        "resolved_revisions": revisions,
    }
    source_gradle = {
        "distribution_sha256": gradle_binding["distribution_sha256"],
        "distribution_url": gradle_url,
        "gradle_user_home_inventory": retained_source_inventory(
            "gradle/gradle-user-home"
        ),
        "java_wrapper_properties_sha256": wrapper_digests["java"],
        "kotlin_wrapper_properties_sha256": wrapper_digests["kotlin"],
        "version": "9.3.0",
        "wrapper_cache_key": gradle_key,
    }
    if (
        source_node["package_lock_sha256"]
        != node_binding["package_lock_sha256"]
        or source_openapi_node["package_lock_sha256"]
        != openapi_node_binding["package_lock_sha256"]
        or source_swift["package_resolved_sha256"]
        != swift_binding["package_resolved_sha256"]
        or source_swift["resolved_revisions"] != revisions
        or source_gradle["distribution_sha256"]
        != gradle_binding["distribution_sha256"]
        or source_gradle["distribution_url"] != gradle_url
        or source_gradle["java_wrapper_properties_sha256"]
        != wrapper_digests["java"]
        or source_gradle["kotlin_wrapper_properties_sha256"]
        != wrapper_digests["kotlin"]
        or source_gradle["version"] != "9.3.0"
        or source_gradle["wrapper_cache_key"] != gradle_key
    ):
        raise BootstrapError("terminal SDK sanitized bindings disagree with receipt")

    def source_inventory(
        value: Any, label: str,
    ) -> list[dict[str, Any]]:
        inventory = _require_exact_json_fields(
            value,
            {
                "format", "schema_version", "record_count", "file_bytes",
                "records_sha256", "records",
            },
            label,
        )
        raw_records = inventory["records"]
        if (
            not isinstance(raw_records, list)
            or not raw_records
            or not isinstance(raw_records[0], dict)
            or raw_records[0].get("path") != "."
            or raw_records[0].get("kind") != "directory"
            or not isinstance(raw_records[0].get("mode"), str)
        ):
            raise BootstrapError(f"{label} root is malformed")
        records, file_bytes = sdk_records(
            raw_records, label, raw_records[0]["mode"],
        )
        record_payload = json.dumps(
            records, ensure_ascii=True, sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
        if (
            inventory["format"]
            != "iroha-sumeragi-v2-sdk-dependency-source-inventory"
            or type(inventory["schema_version"]) is not int
            or inventory["schema_version"] != 1
            or inventory["record_count"] != len(records)
            or inventory["file_bytes"] != file_bytes
            or inventory["records_sha256"]
            != hashlib.sha256(record_payload).hexdigest()
        ):
            raise BootstrapError(f"{label} accounting is not exact")
        return records

    source_specs = (
        (
            "node/node_modules", source_node["node_modules_inventory"],
            "terminal SDK private Node inventory",
        ),
        (
            "openapi/node_modules",
            source_openapi_node["node_modules_inventory"],
            "terminal SDK private OpenAPI Node inventory",
        ),
        (
            "swiftpm/cache", source_swift["cache_inventory"],
            "terminal SDK private SwiftPM inventory",
        ),
        (
            "gradle/gradle-user-home",
            source_gradle["gradle_user_home_inventory"],
            "terminal SDK private Gradle inventory",
        ),
    )
    source_maps: dict[str, dict[str, dict[str, Any]]] = {}
    for prefix, raw_inventory, label in source_specs:
        records = source_inventory(raw_inventory, label)
        source_maps[prefix] = {str(record["path"]): record for record in records}
        projected: list[dict[str, Any]] = []
        for source_record in records:
            projected_record = dict(source_record)
            relative = str(source_record["path"])
            projected_record["path"] = (
                prefix if relative == "." else f"{prefix}/{relative}"
            )
            if source_record["kind"] == "directory":
                projected_record["mode"] = "0500"
            elif source_record["kind"] == "file":
                projected_record["mode"] = (
                    "0500"
                    if int(str(source_record["mode"]), 8) & 0o111
                    else "0400"
                )
            projected.append(projected_record)
        retained = [
            record for record in input_records
            if record["path"] == prefix
            or str(record["path"]).startswith(prefix + "/")
        ]
        if sorted(projected, key=lambda item: str(item["path"])) != retained:
            raise BootstrapError(f"{label} does not reproduce the retained subtree")
    swift_source_records = source_maps["swiftpm/cache"]
    swift_top = {
        path for path in swift_source_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    checkouts = {str(item["checkout"]) for item in revisions}
    observed_checkouts = {
        path.removeprefix("checkouts/")
        for path, record in swift_source_records.items()
        if path.startswith("checkouts/")
        and "/" not in path.removeprefix("checkouts/")
        and record.get("kind") == "directory"
    }
    if (
        swift_top != {"checkouts", "repositories"}
        or observed_checkouts != checkouts
        or any(
            swift_source_records.get(
                f"checkouts/{checkout}/.git/HEAD", {}
            ).get("kind") != "file"
            for checkout in checkouts
        )
    ):
        raise BootstrapError("terminal SDK SwiftPM source topology is not exact")
    gradle_source_records = source_maps["gradle/gradle-user-home"]
    gradle_top = {
        path for path in gradle_source_records
        if path != "." and PurePosixPath(path).parent.as_posix() == "."
    }
    gradle_cache_root = f"wrapper/dists/gradle-9.3.0-bin/{gradle_key}"
    if (
        gradle_top != {"caches", "wrapper"}
        or gradle_source_records.get("caches/9.3.0", {}).get("kind")
        != "directory"
        or gradle_source_records.get("caches/modules-2", {}).get("kind")
        != "directory"
        or gradle_source_records.get(gradle_cache_root, {}).get("kind")
        != "directory"
        or gradle_source_records.get(
            f"{gradle_cache_root}/gradle-9.3.0-bin.zip.ok", {}
        ).get("kind") != "file"
        or gradle_source_records.get(
            gradle_launcher.removeprefix("gradle/gradle-user-home/"), {}
        ).get("kind") != "file"
    ):
        raise BootstrapError("terminal SDK Gradle source topology is not exact")
    final_document = _require_exact_json_fields(
        sdk_document("final_work_inventory", "terminal SDK final-work inventory"),
        {
            "format", "schema_version", "archive_id",
            "sdk_dependency_inventory_sha256", "record_count", "file_bytes",
            "records",
        },
        "terminal SDK final-work inventory",
    )
    final_records, final_bytes = sdk_records(
        final_document["records"], "terminal SDK final-work inventory", "0700"
    )
    if (
        final_document["format"]
        != "iroha-sumeragi-v2-sdk-dependency-work-final"
        or final_document["schema_version"] != 1
        or final_document["archive_id"]
        != "release-sdk-dependencies.work-final.v1"
        or final_document["sdk_dependency_inventory_sha256"]
        != sdk_snapshots["input_inventory"].sha256
        or final_document["record_count"] != len(final_records)
        or final_document["file_bytes"] != final_bytes
        or final_records != initial_records
        or final_bytes != initial_bytes
    ):
        raise BootstrapError("terminal SDK final-work inventory is not exact")
    expected_members = {
        ("sdk-inputs" if record["path"] == "." else f"sdk-inputs/{record['path']}"): record
        for record in input_records
    }
    control_names = {
        "node/package-lock.json",
        "node/node_modules/.package-lock.json",
        "openapi/package-lock.json",
        "openapi/node_modules/.package-lock.json",
        "swiftpm/Package.resolved",
        "gradle/java-gradle-wrapper.properties",
        "gradle/kotlin-gradle-wrapper.properties",
        *(
            f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
            for item in revisions
        ),
    }
    controls: dict[str, bytes] = {}
    try:
        with tarfile.open(sdk_snapshots["archive"].path, mode="r:") as archive:
            members = archive.getmembers()
            if len(members) != len(expected_members) or {item.name for item in members} != set(expected_members):
                raise BootstrapError("terminal SDK tar inventory is not exact")
            for member in members:
                record = expected_members[member.name]
                kind = record["kind"]
                if (
                    member.uid != 0 or member.gid != 0 or member.mtime != 0
                    or member.mode != int(record["mode"], 8)
                    or (kind == "directory" and not member.isdir())
                    or (kind == "symlink" and (not member.issym() or member.linkname != record["target"]))
                    or (kind == "file" and (not member.isfile() or member.size != record["size"]))
                ):
                    raise BootstrapError("terminal SDK tar member metadata changed")
                if kind == "file":
                    stream = archive.extractfile(member)
                    if stream is None:
                        raise BootstrapError("terminal SDK tar member is unavailable")
                    digest = hashlib.sha256()
                    relative = member.name.removeprefix("sdk-inputs/")
                    captured = bytearray()
                    while block := stream.read(1024 * 1024):
                        digest.update(block)
                        if relative in control_names:
                            captured.extend(block)
                            if len(captured) > 16 * 1024 * 1024:
                                raise BootstrapError(
                                    "terminal SDK control file exceeds its bound"
                                )
                    if digest.hexdigest() != record["sha256"]:
                        raise BootstrapError("terminal SDK tar member digest changed")
                    if relative in control_names:
                        controls[relative] = bytes(captured)
    except (OSError, tarfile.TarError) as error:
        raise BootstrapError("terminal SDK dependency tar is malformed") from error

    def control_json(name: str) -> dict[str, Any]:
        data = controls.get(name)
        if data is None:
            raise BootstrapError(f"terminal SDK control file is absent: {name}")
        try:
            value = json.loads(data)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BootstrapError(f"terminal SDK control file is malformed: {name}") from error
        if not isinstance(value, dict):
            raise BootstrapError(f"terminal SDK control file is not an object: {name}")
        return value

    package_lock = control_json("node/package-lock.json")
    installed_lock = control_json("node/node_modules/.package-lock.json")
    if (
        package_lock.get("lockfileVersion") != 3
        or installed_lock.get("lockfileVersion") != 3
        or (package_lock.get("name"), package_lock.get("version"))
        != (installed_lock.get("name"), installed_lock.get("version"))
        or not isinstance(package_lock.get("packages"), dict)
        or not isinstance(installed_lock.get("packages"), dict)
        or not installed_lock["packages"]
        or any(
            package_lock["packages"].get(name) != value
            for name, value in installed_lock["packages"].items()
        )
    ):
        raise BootstrapError("terminal SDK Node locks do not bind the closure")
    openapi_package_lock = control_json("openapi/package-lock.json")
    openapi_installed_lock = control_json(
        "openapi/node_modules/.package-lock.json"
    )
    openapi_packages = openapi_package_lock.get("packages")
    openapi_installed_packages = openapi_installed_lock.get("packages")
    if (
        openapi_package_lock.get("lockfileVersion") != 3
        or openapi_installed_lock.get("lockfileVersion") != 3
        or (
            openapi_package_lock.get("name"),
            openapi_package_lock.get("version"),
        )
        != (
            openapi_installed_lock.get("name"),
            openapi_installed_lock.get("version"),
        )
        or not isinstance(openapi_packages, dict)
        or not isinstance(openapi_installed_packages, dict)
        or not openapi_installed_packages
        or "" not in openapi_packages
        or openapi_installed_packages
        != {
            name: value
            for name, value in openapi_packages.items()
            if name
        }
    ):
        raise BootstrapError(
            "terminal SDK OpenAPI Node locks do not exactly bind the closure"
        )
    package_resolved = control_json("swiftpm/Package.resolved")
    resolved_pairs = sorted(
        (
            {
                "identity": pin.get("identity"),
                "revision": pin.get("state", {}).get("revision"),
            }
            for pin in package_resolved.get("pins", [])
            if isinstance(pin, dict) and isinstance(pin.get("state"), dict)
        ),
        key=lambda item: str(item["identity"]),
    ) if isinstance(package_resolved.get("pins"), list) else []
    if (
        package_resolved.get("version") != 2
        or resolved_pairs != [
            {"identity": item["identity"], "revision": item["revision"]}
            for item in revisions
        ]
    ):
        raise BootstrapError("terminal SDK Swift revisions are not exact")
    for item in revisions:
        head = controls.get(
            f"swiftpm/cache/checkouts/{item['checkout']}/.git/HEAD"
        )
        try:
            observed_head = head.decode("ascii", "strict").strip()
        except (AttributeError, UnicodeDecodeError) as error:
            raise BootstrapError("terminal SDK Swift checkout HEAD is malformed") from error
        if observed_head != item["revision"]:
            raise BootstrapError("terminal SDK Swift checkout HEAD changed")
    for kind in ("java", "kotlin"):
        try:
            lines = controls[
                f"gradle/{kind}-gradle-wrapper.properties"
            ].decode("utf-8").splitlines()
        except (KeyError, UnicodeDecodeError) as error:
            raise BootstrapError("terminal SDK Gradle wrapper is malformed") from error
        values = dict(
            line.split("=", 1) for line in lines
            if line and not line.startswith("#") and "=" in line
        )
        if values.get("distributionUrl") != gradle_url.replace(":", r"\:", 1):
            raise BootstrapError("terminal SDK Gradle wrapper URL changed")
        checksum = values.get("distributionSha256Sum")
        if checksum is not None and checksum != gradle_binding["distribution_sha256"]:
            raise BootstrapError("terminal SDK Gradle wrapper digest changed")

    runtime_probes = _require_exact_json_fields(
        receipt_evidence["runtime_tool_probes"],
        {
            "format", "schema_version", "host_family",
            "probe_contract_sha256", "tool_count", "result",
        },
        "terminal runtime tool probes",
    )
    if (
        runtime_probes["format"]
        != "iroha-sumeragi-v2-release-tool-functional-probes"
        or type(runtime_probes["schema_version"]) is not int
        or runtime_probes["schema_version"] != 1
        or runtime_probes["host_family"]
        != ("darwin" if sys.platform == "darwin" else "linux")
        or type(runtime_probes["tool_count"]) is not int
        or runtime_probes["tool_count"] != 41
        or not isinstance(runtime_probes["probe_contract_sha256"], str)
        or _DIGEST_RE.fullmatch(runtime_probes["probe_contract_sha256"])
        is None
    ):
        raise BootstrapError("terminal runtime tool probe identity is not exact")
    runtime_probe_record = _require_exact_json_fields(
        runtime_probes["result"],
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        "terminal runtime tool probe result",
    )
    if runtime_probe_record["archive_name"] != "runtime-tool-probe-result.json":
        raise BootstrapError(
            "terminal runtime tool probe result archive name is not exact"
        )
    runtime_probe_snapshot = capture_archive(
        runtime_probe_record,
        "terminal runtime tool probe result",
        archive_id="release-runtime.tool-probes.v1",
        expected_path=release_root.parent / "runtime-tool-probe-result.json",
        maximum_bytes=1024 * 1024,
        expected_mode=_DATA_MODE,
        containment_root=release_root.parent,
        extra_fields=frozenset({"archive_name"}),
    )
    runtime_probe_file = _read_file(
        runtime_probe_snapshot.path,
        "terminal runtime tool probe result",
        maximum_bytes=1024 * 1024,
    )
    if runtime_probe_file.sha256 != runtime_probe_snapshot.sha256:
        raise BootstrapError("terminal runtime tool probe result changed")
    runtime_probe_value = _validate_tool_probe_result(
        _parse_canonical_json(
            runtime_probe_file, "terminal runtime tool probe result"
        ),
        runner_record["tools"],
        archive_id_prefix="release-runtime-tool",
    )
    if any(
        runtime_probes[field] != runtime_probe_value[field]
        for field in (
            "format", "schema_version", "host_family",
            "probe_contract_sha256", "tool_count",
        )
    ):
        raise BootstrapError("terminal runtime tool probe record disagrees")

    prebuilt = _require_exact_json_fields(
        receipt_evidence["prebuilt_binary_bundle"],
        {
            "schema_version",
            "archive_id",
            "manifest",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "cargo_version_sha256",
            "rustc_version_sha256",
            "host_triple",
            "target_triple",
            "profile",
            "version_transcripts",
            "binaries",
        },
        "terminal prebuilt binary bundle",
    )
    if type(prebuilt["schema_version"]) is not int or prebuilt["schema_version"] != 3:
        raise BootstrapError("terminal prebuilt binary bundle has the wrong schema")
    for field in (
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "cargo_version_sha256",
        "rustc_version_sha256",
    ):
        if not isinstance(prebuilt[field], str):
            raise BootstrapError(f"terminal prebuilt {field} is malformed")
        _require_digest(prebuilt[field], f"terminal prebuilt {field}")
    if (
        prebuilt["source_manifest_sha256"]
        != receipt_identity["sealed_source_manifest_sha256"]
        or prebuilt["cargo_lock_sha256"] != receipt_identity["cargo_lock_sha256"]
        or prebuilt["profile"] != "release"
        or not isinstance(prebuilt["host_triple"], str)
        or _PREBUILT_TRIPLE_RE.fullmatch(prebuilt["host_triple"]) is None
        or prebuilt["target_triple"] != prebuilt["host_triple"]
        or not isinstance(prebuilt["archive_id"], str)
        or not prebuilt["archive_id"].startswith("release-prebuilt.bundle.v1:")
    ):
        raise BootstrapError("terminal prebuilt binary bundle identity is not exact")
    release_invocation_root = release_root.parent
    artifact_root = _absolute_resolved_existing(
        release_invocation_root / "output", "terminal release artifact root"
    )
    cargo_target_root = release_invocation_root / "target"
    invocation_id = prebuilt["archive_id"].partition(":")[2]
    if _PREBUILT_INVOCATION_RE.fullmatch(invocation_id) is None:
        raise BootstrapError("terminal prebuilt archive id is malformed")
    prebuilt_root = _absolute_resolved_existing(
        artifact_root
        / "sumeragi-v2-release"
        / receipt_identity["sealed_source_manifest_sha256"]
        / "programs"
        / invocation_id,
        "terminal prebuilt bundle directory",
    )
    if (
        artifact_root != release_invocation_root / "output"
        or cargo_target_root != release_invocation_root / "target"
        or _inside(artifact_root, cargo_target_root)
        or _inside(cargo_target_root, artifact_root)
        or _inside(artifact_root, release_root)
        or _inside(cargo_target_root, release_root)
        or prebuilt_root.parent
        != (
            artifact_root
            / "sumeragi-v2-release"
            / receipt_identity["sealed_source_manifest_sha256"]
            / "programs"
        )
        or _PREBUILT_INVOCATION_RE.fullmatch(prebuilt_root.name) is None
    ):
        raise BootstrapError("terminal prebuilt bundle is outside the sealed invocation root")
    capture_directory(
        artifact_root,
        "terminal release artifact root",
        containment_root=artifact_root,
        expected_mode=_DIRECTORY_MODE,
    )
    prebuilt_manifest_snapshot = capture_archive(
        prebuilt["manifest"],
        "terminal prebuilt manifest",
        archive_id="release-prebuilt.manifest.v2",
        expected_path=prebuilt_root / ".sumeragi-v2-prebuilt-binaries.tsv",
        maximum_bytes=32 * 1024,
        expected_mode=_DATA_MODE,
        containment_root=artifact_root,
    )
    prebuilt_manifest_file = _read_file(
        prebuilt_manifest_snapshot.path,
        "terminal prebuilt manifest contents",
        maximum_bytes=32 * 1024,
    )
    if (
        prebuilt_manifest_file.sha256 != prebuilt_manifest_snapshot.sha256
        or prebuilt_manifest_file.device != prebuilt_manifest_snapshot.device
        or prebuilt_manifest_file.inode != prebuilt_manifest_snapshot.inode
        or prebuilt_manifest_file.mode != prebuilt_manifest_snapshot.mode
        or prebuilt_manifest_file.owner != prebuilt_manifest_snapshot.owner
        or prebuilt_manifest_file.nlink != prebuilt_manifest_snapshot.nlink
        or prebuilt_manifest_file.size != prebuilt_manifest_snapshot.size
        or prebuilt_manifest_file.mtime_ns != prebuilt_manifest_snapshot.mtime_ns
        or prebuilt_manifest_file.ctime_ns != prebuilt_manifest_snapshot.ctime_ns
    ):
        raise BootstrapError("terminal prebuilt manifest changed while it was decoded")
    try:
        manifest_text = prebuilt_manifest_file.data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("terminal prebuilt manifest is not UTF-8") from error
    manifest_lines = manifest_text.splitlines(keepends=True)
    if (
        len(manifest_lines) != len(_PREBUILT_MANIFEST_FIELDS)
        or any(
            not line.endswith("\n")
            or line.endswith("\r\n")
            or line.count("\t") != 1
            for line in manifest_lines
        )
    ):
        raise BootstrapError("terminal prebuilt manifest is not one exact TSV inventory")
    manifest_rows = tuple(line[:-1].split("\t", 1) for line in manifest_lines)
    if tuple(row[0] for row in manifest_rows) != _PREBUILT_MANIFEST_FIELDS:
        raise BootstrapError("terminal prebuilt manifest field order is not exact")
    manifest_fields = dict(manifest_rows)
    if (
        manifest_fields["schema_version"] != "2"
        or manifest_fields["source_manifest_sha256"]
        != prebuilt["source_manifest_sha256"]
        or manifest_fields["cargo_lock_sha256"] != prebuilt["cargo_lock_sha256"]
        or manifest_fields["cargo_version_sha256"]
        != prebuilt["cargo_version_sha256"]
        or manifest_fields["rustc_version_sha256"]
        != prebuilt["rustc_version_sha256"]
        or manifest_fields["host_triple"] != prebuilt["host_triple"]
        or manifest_fields["target_triple"] != prebuilt["target_triple"]
        or manifest_fields["profile"] != prebuilt["profile"]
        or manifest_fields["bundle_dir"] != str(prebuilt_root)
    ):
        raise BootstrapError("terminal prebuilt receipt diverges from its manifest")
    transcripts = _require_exact_json_fields(
        prebuilt["version_transcripts"], {"cargo", "rustc"}, "terminal prebuilt transcripts"
    )
    runner_tools = runner_record.get("tools")
    if not isinstance(runner_tools, dict):
        raise BootstrapError("terminal runner tool inventory is malformed")
    for tool in ("cargo", "rustc"):
        transcript = _require_exact_json_fields(
            transcripts[tool],
            {"operation_id", "tool_archive_id", "sha256", "size_bytes"},
            f"terminal {tool} transcript",
        )
        if (
            transcript["operation_id"] != f"{tool}.version.v1"
            or transcript["tool_archive_id"] != f"release-runner-tool.{tool}.v1"
            or transcript["sha256"] != prebuilt[f"{tool}_version_sha256"]
            or type(transcript["size_bytes"]) is not int
            or not 0 < transcript["size_bytes"] <= 64 * 1024
        ):
            raise BootstrapError(f"terminal {tool} transcript is malformed")
        authenticated_tool = runner_tools.get(tool)
        if not isinstance(authenticated_tool, dict):
            raise BootstrapError(f"terminal runner omits authenticated {tool}")
        authenticated_archive = authenticated_tool.get("archive_name")
        authenticated_sha256 = authenticated_tool.get("sha256")
        if not isinstance(authenticated_archive, str) or not isinstance(
            authenticated_sha256, str
        ):
            raise BootstrapError(f"terminal runner authenticated {tool} is malformed")
        bootstrap_evidence_root = authenticated_environment.get(
            "SUMERAGI_V2_RELEASE_BOOTSTRAP_EVIDENCE_DIR"
        )
        if not isinstance(bootstrap_evidence_root, str):
            raise BootstrapError("terminal runner evidence root is unavailable")
        executable = _absolute_resolved_existing(
            Path(bootstrap_evidence_root) / authenticated_archive,
            f"terminal runner authenticated {tool} executable",
        )
        authenticated_digest = _require_digest(
            authenticated_sha256, f"terminal runner authenticated {tool} digest"
        )
        executable_snapshot = _capture_large_file(
            executable, f"terminal {tool} transcript executable"
        )
        if executable_snapshot.sha256 != authenticated_digest:
            raise BootstrapError(
                f"terminal {tool} transcript executable digest is not authenticated"
            )
        if (
            executable_snapshot.owner != os.getuid()
            or executable_snapshot.nlink != 1
            or executable_snapshot.mode & 0o111 == 0
        ):
            raise BootstrapError(
                f"terminal {tool} transcript executable is not exact and owner-controlled"
            )
    binaries = prebuilt["binaries"]
    if not isinstance(binaries, list) or len(binaries) != len(_PREBUILT_BINARY_SPECS):
        raise BootstrapError("terminal prebuilt binary inventory is incomplete")
    for index, ((role, relative), record) in enumerate(
        zip(_PREBUILT_BINARY_SPECS, binaries)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != role
            or record.get("relative_path") != relative
            or manifest_fields[f"{role}_relative_path"] != relative
            or manifest_fields[f"{role}_sha256"] != record.get("sha256")
            or manifest_fields[f"{role}_size_bytes"]
            != str(record.get("size_bytes"))
            or manifest_fields[f"{role}_mode_octal"] != record.get("mode")
        ):
            raise BootstrapError(f"terminal prebuilt binary {index} identity is not exact")
        capture_archive(
            record,
            f"terminal prebuilt binary {index}",
            archive_id=f"release-prebuilt.binary.{role}.v1",
            extra_fields=frozenset({"role", "relative_path"}),
            expected_path=prebuilt_root.joinpath(*relative.split("/")),
            maximum_bytes=2 * 1024 * 1024 * 1024,
            expected_mode=_TOOL_MODE,
            containment_root=artifact_root,
        )
    require_inventory(
        prebuilt_root,
        {".sumeragi-v2-prebuilt-binaries.tsv", "release", "message-control"},
        "terminal prebuilt invocation directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "release",
        {"iroha3d", "iroha", "kagami"},
        "terminal prebuilt release directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "message-control",
        {"release"},
        "terminal prebuilt message-control directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )
    require_inventory(
        prebuilt_root / "message-control" / "release",
        {"iroha3d"},
        "terminal prebuilt message-control release directory",
        containment_root=artifact_root,
        expected_mode=_TOOL_MODE,
    )

    scaling = _require_exact_json_fields(
        receipt_evidence["multilane_scaling_bundle"],
        {"archive_id", "file_count", "total_size_bytes", "directories", "files"},
        "terminal scaling bundle",
    )
    if scaling["archive_id"] != "release-scaling.bundle.v1":
        raise BootstrapError("terminal scaling bundle archive id is malformed")
    manifest_environment = authenticated_environment.get(
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
    )
    if not isinstance(manifest_environment, str):
        raise BootstrapError("authenticated runner omits scaling manifest")
    scaling_manifest_input = _absolute_resolved_existing(
        Path(manifest_environment), "authenticated scaling manifest"
    )
    scaling_root = _absolute_resolved_existing(
        scaling_manifest_input.parent, "terminal scaling bundle root"
    )
    capture_directory(
        scaling_root,
        "terminal scaling bundle root",
        containment_root=scaling_root,
    )
    scaling_directories = scaling["directories"]
    scaling_files = scaling["files"]
    if (
        not isinstance(scaling_directories, list)
        or len(scaling_directories) > _MAX_SCALING_BUNDLE_DIRECTORY_COUNT
        or not isinstance(scaling_files, list)
        or len(scaling_files) > _MAX_SCALING_BUNDLE_FILE_COUNT
        or type(scaling["file_count"]) is not int
        or scaling["file_count"] != len(scaling_files)
        or type(scaling["total_size_bytes"]) is not int
        or not 0 <= scaling["total_size_bytes"] <= _MAX_SCALING_BUNDLE_TOTAL_BYTES
    ):
        raise BootstrapError("terminal scaling bundle inventory is malformed")
    parsed_directories: list[str] = []
    for index, relative in enumerate(scaling_directories):
        parts = _terminal_relative_path(relative, f"terminal scaling directory {index}")
        parsed_directories.append(relative)
        capture_directory(
            scaling_root.joinpath(*parts),
            f"terminal scaling directory {index}",
            containment_root=scaling_root,
        )
    if parsed_directories != sorted(set(parsed_directories)):
        raise BootstrapError("terminal scaling directories are not sorted and unique")
    parsed_files: list[str] = []
    total_size = 0
    scaling_manifest: Path | None = None
    for index, record in enumerate(scaling_files):
        if not isinstance(record, dict):
            raise BootstrapError(f"terminal scaling file {index} is malformed")
        relative = record.get("relative_path")
        parts = _terminal_relative_path(relative, f"terminal scaling file {index}")
        parsed_files.append(relative)
        snapshot = capture_archive(
            record,
            f"terminal scaling file {index}",
            archive_id="release-scaling.file.v1:" + relative,
            expected_path=scaling_root.joinpath(*parts),
            maximum_bytes=_MAX_SCALING_BUNDLE_FILE_BYTES,
            containment_root=scaling_root,
            extra_fields=frozenset({"relative_path"}),
        )
        total_size += snapshot.size
        if relative == "scaling_evidence.json":
            scaling_manifest = snapshot.path
    if parsed_files != sorted(set(parsed_files)) or total_size != scaling["total_size_bytes"]:
        raise BootstrapError("terminal scaling files are not one exact sorted inventory")
    if scaling_manifest is None:
        raise BootstrapError("terminal scaling bundle omits scaling_evidence.json")
    live_files: list[str] = []
    live_directories: list[str] = []
    for current, names, filenames in os.walk(scaling_root, followlinks=False):
        current_path = Path(current)
        for name in names:
            path = current_path / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
                raise BootstrapError("terminal scaling bundle contains an unsafe directory")
            live_directories.append(path.relative_to(scaling_root).as_posix())
        for name in filenames:
            path = current_path / name
            metadata = path.lstat()
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
                raise BootstrapError("terminal scaling bundle contains an unsafe file")
            live_files.append(path.relative_to(scaling_root).as_posix())
    if sorted(live_directories) != parsed_directories or sorted(live_files) != parsed_files:
        raise BootstrapError("terminal scaling bundle live inventory does not match receipt")

    retained_validator = capture_archive(
        receipt_evidence["multilane_scaling_retained_validator"],
        "terminal retained scaling validator",
        archive_id="release-scaling.retained-validator.v1",
        expected_path=release_root / "scripts" / "nexus" / "validate_multilane_scaling_evidence.py",
        containment_root=release_root,
    )
    trust_anchors = _require_exact_json_fields(
        receipt_evidence["multilane_scaling_trust_anchors"],
        {
            "trial_harness_sha256",
            "configuration_sha256",
            "irohad_sha256",
            "iroha_cli_sha256",
            "retained_tooling",
        },
        "terminal scaling trust anchors",
    )
    for field, environment_name in _SCALING_DIGEST_ENVIRONMENT.items():
        value = authenticated_environment.get(environment_name)
        if not isinstance(value, str):
            raise BootstrapError(f"authenticated runner environment omits {environment_name}")
        _require_digest(value, f"authenticated {environment_name}")
        if trust_anchors[field] != value:
            raise BootstrapError(f"terminal scaling trust anchor {field} is not authenticated")
    if manifest_environment != str(scaling_manifest):
        raise BootstrapError("terminal scaling manifest is not the authenticated runner input")
    retained_tooling = trust_anchors["retained_tooling"]
    if not isinstance(retained_tooling, list) or len(retained_tooling) != len(_SCALING_REQUIRED_TOOLING):
        raise BootstrapError("terminal scaling retained tooling inventory is incomplete")
    for index, ((role, source_path), record) in enumerate(
        zip(_SCALING_REQUIRED_TOOLING, retained_tooling)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != role
        ):
            raise BootstrapError(f"terminal retained scaling tool {index} identity is not exact")
        capture_archive(
            record,
            f"terminal retained scaling tool {index}",
            archive_id=f"release-scaling.retained-tool.{role}.v1",
            extra_fields=frozenset({"role"}),
            expected_path=release_root.joinpath(*source_path.split("/")),
            containment_root=release_root,
        )
    if retained_validator.mode & 0o111 == 0:
        raise BootstrapError("terminal retained scaling validator is not executable")

    g4p = _require_exact_json_fields(
        receipt_evidence["g4p_multilane"],
        {"schema_version", "completion", "run_summary", "run_logs"},
        "terminal G-4P evidence",
    )
    if type(g4p["schema_version"]) is not int or g4p["schema_version"] != 1:
        raise BootstrapError("terminal G-4P evidence has the wrong schema")
    if not isinstance(g4p["completion"], dict) or not isinstance(g4p["completion"].get("path"), str):
        raise BootstrapError("terminal G-4P completion is malformed")
    g4p_root = Path(g4p["completion"]["path"]).parent
    capture_artifact(
        g4p["completion"],
        "terminal G-4P completion",
        full=True,
        expected_path=g4p_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g4p["run_summary"],
        "terminal G-4P run summary",
        full=True,
        expected_path=g4p_root / "runs.tsv",
        maximum_bytes=1024 * 1024,
    )
    g4p_logs = g4p["run_logs"]
    g4p_names = (
        "run-00-nexus_and_streaming.log",
        "run-01-nexus_and_streaming.log",
        "run-02-nexus_and_streaming.log",
        "run-03-native_amx_routing.log",
    )
    if not isinstance(g4p_logs, list) or len(g4p_logs) != len(g4p_names):
        raise BootstrapError("terminal G-4P run-log inventory is incomplete")
    for index, (record, filename) in enumerate(zip(g4p_logs, g4p_names)):
        capture_artifact(
            record,
            f"terminal G-4P run log {index}",
            full=True,
            expected_path=g4p_root / filename,
            maximum_bytes=16 * 1024 * 1024,
        )
    require_inventory(g4p_root, {"COMPLETED.tsv", "runs.tsv", *g4p_names}, "terminal G-4P directory")

    g12 = _require_exact_json_fields(
        receipt_evidence["g12_cross_dataspace"],
        {
            "seed_completion",
            "seed_summary",
            "seed_run_logs",
            "fault_soak_completion",
            "fault_soak_log",
        },
        "terminal G-12 evidence",
    )
    if not isinstance(g12["seed_completion"], dict) or not isinstance(
        g12["seed_completion"].get("path"), str
    ):
        raise BootstrapError("terminal G-12 seed completion is malformed")
    if not isinstance(g12["fault_soak_completion"], dict) or not isinstance(
        g12["fault_soak_completion"].get("path"), str
    ):
        raise BootstrapError("terminal G-12 fault-soak completion is malformed")
    seed_root = Path(g12["seed_completion"]["path"]).parent
    soak_root = Path(g12["fault_soak_completion"]["path"]).parent
    if seed_root == soak_root:
        raise BootstrapError("terminal G-12 seed and soak roots are not distinct")
    capture_artifact(
        g12["seed_completion"],
        "terminal G-12 seed completion",
        full=True,
        expected_path=seed_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g12["seed_summary"],
        "terminal G-12 seed summary",
        full=True,
        expected_path=seed_root / "runs.tsv",
        maximum_bytes=1024 * 1024,
    )
    seed_logs = g12["seed_run_logs"]
    seed_names = tuple(f"seed-{ordinal:02d}.log" for ordinal in range(10))
    if not isinstance(seed_logs, list) or len(seed_logs) != len(seed_names):
        raise BootstrapError("terminal G-12 seed-log inventory is incomplete")
    for index, (record, filename) in enumerate(zip(seed_logs, seed_names)):
        capture_artifact(
            record,
            f"terminal G-12 seed log {index}",
            full=True,
            expected_path=seed_root / filename,
            maximum_bytes=16 * 1024 * 1024,
        )
    capture_artifact(
        g12["fault_soak_completion"],
        "terminal G-12 fault-soak completion",
        full=True,
        expected_path=soak_root / "COMPLETED.tsv",
        maximum_bytes=1024 * 1024,
    )
    capture_artifact(
        g12["fault_soak_log"],
        "terminal G-12 fault-soak log",
        full=True,
        expected_path=soak_root / "fault-soak.log",
        maximum_bytes=16 * 1024 * 1024,
    )
    require_inventory(seed_root, {"COMPLETED.tsv", "runs.tsv", *seed_names}, "terminal G-12 seed directory")
    require_inventory(soak_root, {"COMPLETED.tsv", "fault-soak.log"}, "terminal G-12 soak directory")

    return artifact_snapshots, directory_snapshots


def _retained_release_layout(
    evidence: Path,
    evidence_fd: int,
    *,
    candidate: Path | None = None,
    authenticated_environment: dict[str, str] | None = None,
    expected_receipt: dict[str, Any] | None = None,
) -> tuple[Path, Path, Path, FileSnapshot | None, FileSnapshot | None, FileSnapshot | None]:
    """Authenticate the outer-published retained tree through held directories."""

    result_name = "release-runner-result.json"
    try:
        os.stat(result_name, dir_fd=evidence_fd, follow_symlinks=False)
    except FileNotFoundError:
        release_runner = evidence / "release-runner"
        return (
            release_runner,
            release_runner / "output" / "release" / "RELEASE_COMPLETED.json",
            release_runner / "sealed-identity.json",
            None,
            None,
            None,
        )
    except OSError as error:
        raise BootstrapError("protected outer release result is unavailable") from error
    result_path = evidence / result_name
    result_snapshot = _read_file_at(
        evidence_fd,
        result_name,
        result_path,
        "protected outer release result",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if (
        result_snapshot.mode != _DATA_MODE
        or result_snapshot.owner != os.getuid()
        or result_snapshot.nlink != 1
    ):
        raise BootstrapError("protected outer release result metadata is not exact")
    result = _require_exact_json_fields(
        _parse_canonical_json(result_snapshot, "protected outer release result"),
        {
            "format", "schema_version", "invocation_archive_id",
            "source_archive_id",
            "source_manifest_sha256", "sealed_identity", "receipt", "inventory",
            "receipt_validation",
        },
        "protected outer release result",
    )
    if (
        result["format"] != "iroha-sumeragi-v2-retained-release-evidence"
        or type(result["schema_version"]) is not int
        or result["schema_version"] != 2
        or result["invocation_archive_id"] != "release-retained.invocation.v1"
        or result["source_archive_id"] != "release-retained.source.v1"
        or not isinstance(result["source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(result["source_manifest_sha256"]) is None
    ):
        raise BootstrapError("protected outer release result schema is not exact")

    private_name = "release-runner-private-provenance.json"
    private_path = evidence / private_name
    private_snapshot = _read_file_at(
        evidence_fd,
        private_name,
        private_path,
        "bootstrap-private retained provenance",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if (
        private_snapshot.mode != _DATA_MODE
        or private_snapshot.owner != os.getuid()
        or private_snapshot.nlink != 1
    ):
        raise BootstrapError(
            "bootstrap-private retained provenance metadata is not exact"
        )
    private = _require_exact_json_fields(
        _parse_canonical_json(
            private_snapshot, "bootstrap-private retained provenance"
        ),
        {"format", "schema_version", "invocation_root", "source_root", "artifacts"},
        "bootstrap-private retained provenance",
    )
    if (
        private["format"]
        != "iroha-sumeragi-v2-bootstrap-private-retained-provenance"
        or type(private["schema_version"]) is not int
        or private["schema_version"] != 1
        or not isinstance(private["invocation_root"], str)
        or not isinstance(private["source_root"], str)
    ):
        raise BootstrapError(
            "bootstrap-private retained provenance schema is not exact"
        )
    release_runner = _absolute_resolved_existing(
        Path(private["invocation_root"]), "retained release evidence root"
    )
    source = _absolute_resolved_existing(
        Path(private["source_root"]), "retained sealed source"
    )
    if (
        source != release_runner / "source"
        or _inside(release_runner, evidence)
        or _inside(evidence, release_runner)
    ):
        raise BootstrapError("retained release evidence is not an exact external root")

    root_snapshot = _private_directory_snapshot(
        release_runner, "retained release evidence root"
    )
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        root_fd = os.open(release_runner, directory_flags)
    except OSError as error:
        raise BootstrapError("retained release evidence root could not be held") from error
    root_opened = os.fstat(root_fd)
    if (
        not stat.S_ISDIR(root_opened.st_mode)
        or (root_opened.st_dev, root_opened.st_ino)
        != (root_snapshot.device, root_snapshot.inode)
        or stat.S_IMODE(root_opened.st_mode) != root_snapshot.mode
        or root_opened.st_uid != root_snapshot.owner
    ):
        os.close(root_fd)
        raise BootstrapError("retained release evidence root changed while opened")

    protected_specs = {
        "receipt": (
            "RELEASE_COMPLETED.json",
            _MAX_TERMINAL_RECEIPT_BYTES,
            "output/release/RELEASE_COMPLETED.json",
            "release-terminal.receipt.v1",
        ),
        "sealed_identity": (
            "sealed-identity.json", _MAX_IDENTITY_BYTES, "sealed-identity.json",
            "release-retained.identity.v1",
        ),
        "inventory": (
            "release-retained-inventory.json",
            256 * 1024 * 1024,
            "retained-evidence-inventory.json",
            "release-retained.inventory.v2",
        ),
        "receipt_validation": (
            "receipt-validation-ack.json",
            _MAX_EVIDENCE_BYTES,
            "receipt-validation-ack.json",
            "release-retained.receipt-validation-ack.v3",
        ),
    }
    private_artifacts = _require_exact_json_fields(
        private["artifacts"], set(protected_specs),
        "bootstrap-private retained artifacts",
    )
    protected: dict[str, FileSnapshot] = {}
    binding_records: dict[str, dict[str, Any]] = {}
    try:
        for field, (filename, maximum, relative, archive_id) in protected_specs.items():
            record = _require_exact_json_fields(
                result[field], {"archive_id", "mode", "sha256", "size_bytes"},
                f"retained {field} binding",
            )
            expected_local = release_runner.joinpath(*relative.split("/"))
            expected_protected = evidence / filename
            private_record = _require_exact_json_fields(
                private_artifacts[field], {"path", "protected_path"},
                f"bootstrap-private retained {field}",
            )
            if (
                private_record != {
                    "path": str(expected_local),
                    "protected_path": str(expected_protected),
                }
                or record["archive_id"] != archive_id
                or record["mode"] != "0400"
                or not isinstance(record["sha256"], str)
                or _DIGEST_RE.fullmatch(record["sha256"]) is None
                or type(record["size_bytes"]) is not int
                or record["size_bytes"] < 0
            ):
                raise BootstrapError(f"retained {field} binding is not exact")
            copied = _read_file_at(
                evidence_fd,
                filename,
                expected_protected,
                f"protected retained {field}",
                maximum_bytes=maximum,
            )
            if (
                copied.sha256 != record["sha256"]
                or copied.size != record["size_bytes"]
                or copied.mode != _DATA_MODE
                or copied.owner != os.getuid()
                or copied.nlink != 1
            ):
                raise BootstrapError(f"retained {field} protected copy is not exact")
            protected[field] = copied
            binding_records[field] = record

        inventory_local = _read_file_at(
            root_fd,
            "retained-evidence-inventory.json",
            release_runner / "retained-evidence-inventory.json",
            "retained evidence inventory",
            maximum_bytes=256 * 1024 * 1024,
        )
        if (
            inventory_local.sha256 != protected["inventory"].sha256
            or inventory_local.size != protected["inventory"].size
            or inventory_local.owner != os.getuid()
            or inventory_local.nlink != 1
            or inventory_local.mode != _DATA_MODE
        ):
            raise BootstrapError("retained inventory local and protected copies disagree")
        inventory_snapshot = protected["inventory"]
        inventory = _require_exact_json_fields(
            _parse_canonical_json(inventory_snapshot, "retained release inventory"),
            {
                "format", "schema_version", "invocation_archive_id",
                "source_archive_id",
                "source_manifest_sha256", "record_count", "file_bytes", "records",
            },
            "retained release inventory",
        )
        records = inventory["records"]
        if (
            inventory["format"] != result["format"]
            or type(inventory["schema_version"]) is not int
            or inventory["schema_version"] != 2
            or inventory["invocation_archive_id"]
            != result["invocation_archive_id"]
            or inventory["source_archive_id"] != result["source_archive_id"]
            or inventory["source_manifest_sha256"]
            != result["source_manifest_sha256"]
            or type(records) is not list
            or type(inventory["record_count"]) is not int
            or inventory["record_count"] != len(records)
            or not 0 <= inventory["record_count"] <= _MAX_RETAINED_RECORDS
            or type(inventory["file_bytes"]) is not int
            or not 0 <= inventory["file_bytes"] <= _MAX_RETAINED_TOTAL_BYTES
        ):
            raise BootstrapError("retained release inventory contract is not exact")
        for index, record in enumerate(records):
            if not isinstance(record, dict):
                raise BootstrapError(f"retained release inventory record {index} is not exact")
            kind = record.get("kind")
            expected_keys = (
                {"path", "kind", "mode"}
                if kind == "directory"
                else {"path", "kind", "mode", "size", "sha256"}
                if kind == "file"
                else set()
            )
            relative = record.get("path")
            mode = record.get("mode")
            if (
                set(record) != expected_keys
                or not isinstance(relative, str)
                or not relative
                or relative.startswith("/")
                or any(part in {"", ".", ".."} for part in relative.split("/"))
                or len(relative.encode()) > _MAX_RETAINED_PATH_BYTES
                or len(relative.split("/")) > _MAX_RETAINED_DEPTH
                or not isinstance(mode, str)
                or re.fullmatch(r"[0-7]{4}", mode) is None
                or (
                    kind == "file"
                    and (
                        type(record.get("size")) is not int
                        or not 0 <= record["size"] <= _MAX_RETAINED_FILE_BYTES
                        or not isinstance(record.get("sha256"), str)
                        or _DIGEST_RE.fullmatch(record["sha256"]) is None
                    )
                )
            ):
                raise BootstrapError(f"retained release inventory record {index} is not exact")

        observed: list[dict[str, Any]] = []
        local_files: dict[str, LargeFileSnapshot] = {}
        file_bytes = 0
        record_count = 0
        stable_directory_fields = (
            "st_dev", "st_ino", "st_mode", "st_uid", "st_gid", "st_nlink",
            "st_mtime_ns", "st_ctime_ns",
        )

        def directory_names(descriptor: int, label: str) -> tuple[str, ...]:
            names: list[str] = []
            try:
                with os.scandir(descriptor) as entries:
                    for entry in entries:
                        names.append(entry.name)
                        if len(names) > _MAX_RETAINED_RECORDS:
                            raise BootstrapError(f"{label} contains too many entries")
            except OSError as error:
                raise BootstrapError(f"{label} could not be enumerated") from error
            return tuple(sorted(names))

        def walk(descriptor: int, relative_directory: str) -> None:
            nonlocal file_bytes, record_count
            before = os.fstat(descriptor)
            names = directory_names(
                descriptor, f"retained {relative_directory or '.'}"
            )
            for name in names:
                if not relative_directory and name in {
                    "source", "retained-evidence-inventory.json",
                }:
                    continue
                if name.startswith((".owned-quarantine.", ".owned-quiescent.")):
                    raise BootstrapError("retained release contains a cleanup quarantine")
                relative = name if not relative_directory else f"{relative_directory}/{name}"
                if (
                    len(relative.encode()) > _MAX_RETAINED_PATH_BYTES
                    or len(relative.split("/")) > _MAX_RETAINED_DEPTH
                ):
                    raise BootstrapError("retained release path exceeds its bound")
                try:
                    metadata = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                except OSError as error:
                    raise BootstrapError(f"retained release entry is unavailable: {relative}") from error
                record_count += 1
                if (
                    record_count > _MAX_RETAINED_RECORDS
                    or metadata.st_uid != os.getuid()
                    or stat.S_ISLNK(metadata.st_mode)
                ):
                    raise BootstrapError(f"retained release entry is unsafe: {relative}")
                path = release_runner.joinpath(*relative.split("/"))
                if stat.S_ISDIR(metadata.st_mode):
                    observed.append({
                        "path": relative,
                        "kind": "directory",
                        "mode": f"{stat.S_IMODE(metadata.st_mode):04o}",
                    })
                    try:
                        child = os.open(name, directory_flags, dir_fd=descriptor)
                    except OSError as error:
                        raise BootstrapError(f"retained directory could not be opened: {relative}") from error
                    try:
                        opened = os.fstat(child)
                        if not stat.S_ISDIR(opened.st_mode) or any(
                            getattr(opened, field) != getattr(metadata, field)
                            for field in stable_directory_fields
                        ):
                            raise BootstrapError(f"retained directory changed: {relative}")
                        walk(child, relative)
                    finally:
                        os.close(child)
                    current = os.stat(name, dir_fd=descriptor, follow_symlinks=False)
                    if any(
                        getattr(current, field) != getattr(metadata, field)
                        for field in stable_directory_fields
                    ):
                        raise BootstrapError(f"retained directory changed: {relative}")
                elif stat.S_ISREG(metadata.st_mode):
                    if metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) & 0o022:
                        raise BootstrapError(f"retained release file is unsafe: {relative}")
                    snapshot = _capture_large_file_at(
                        descriptor,
                        name,
                        path,
                        f"retained release entry {relative}",
                        maximum_bytes=_MAX_RETAINED_FILE_BYTES,
                    )
                    file_bytes += snapshot.size
                    if file_bytes > _MAX_RETAINED_TOTAL_BYTES:
                        raise BootstrapError("retained release files exceed their total bound")
                    observed.append({
                        "path": relative,
                        "kind": "file",
                        "mode": f"{snapshot.mode:04o}",
                        "size": snapshot.size,
                        "sha256": snapshot.sha256,
                    })
                    local_files[relative] = snapshot
                else:
                    raise BootstrapError(f"retained release entry is special: {relative}")
            after = os.fstat(descriptor)
            if names != directory_names(
                descriptor, f"retained {relative_directory or '.'}"
            ) or any(
                getattr(after, field) != getattr(before, field)
                for field in stable_directory_fields
            ):
                raise BootstrapError(
                    f"retained directory changed while read: {relative_directory or '.'}"
                )

        walk(root_fd, "")
        if (
            observed != records
            or inventory["file_bytes"] != file_bytes
            or inventory["record_count"] != record_count
            or _private_directory_snapshot(
                release_runner, "retained release evidence root"
            ) != root_snapshot
        ):
            raise BootstrapError("retained release exact inventory changed")
        for field, (_, _, relative, _) in protected_specs.items():
            if field == "inventory":
                local_digest = inventory_local.sha256
                local_size = inventory_local.size
            else:
                local = local_files.get(relative)
                if local is None:
                    raise BootstrapError(f"retained {field} is absent from the exact tree")
                local_digest = local.sha256
                local_size = local.size
            record = binding_records[field]
            if (
                record["sha256"] != local_digest
                or record["size_bytes"] != local_size
            ):
                raise BootstrapError(f"retained {field} local binding changed")
    finally:
        os.close(root_fd)

    ack_snapshot = protected["receipt_validation"]
    ack = _require_exact_json_fields(
        _parse_canonical_json(ack_snapshot, "receipt validation acknowledgment"),
        {"format", "schema_version", "profile", "sealed_source", "receipt", "validator", "invocation", "exit_status", "stdout", "stderr"},
        "receipt validation acknowledgment",
    )
    receipt_record = _require_exact_json_fields(
        ack["receipt"], {"archive_id", "mode", "sha256", "size_bytes"},
        "ack receipt",
    )
    source_record = _require_exact_json_fields(
        ack["sealed_source"], {"archive_id", "manifest_sha256"},
        "ack sealed source",
    )
    validator_record = _require_exact_json_fields(
        ack["validator"],
        {"archive_id", "sha256", "bootstrap_completion_sha256"},
        "ack validator",
    )
    stdout_record = _require_exact_json_fields(
        ack["stdout"], {"sha256", "size_bytes"}, "ack stdout"
    )
    stderr_record = _require_exact_json_fields(
        ack["stderr"], {"sha256", "size_bytes"}, "ack stderr"
    )
    if expected_receipt is None:
        try:
            expected_receipt = json.loads(protected["receipt"].data)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise BootstrapError("protected terminal receipt is malformed") from error
        if not isinstance(expected_receipt, dict):
            raise BootstrapError("protected terminal receipt is malformed")
    invocation_record = ack["invocation"]
    if candidate is None or authenticated_environment is None:
        raise BootstrapError(
            "retained release validation lacks private invocation provenance"
        )
    local_receipt_path = (
        release_runner / "output" / "release" / "RELEASE_COMPLETED.json"
    )
    _validate_validator_invocation(
        invocation_record,
        expected_values=_terminal_validator_invocation_values(
            expected_receipt,
            evidence=evidence,
            candidate=candidate,
            release_runner=release_runner,
            receipt_path=local_receipt_path,
            acknowledgment_path=release_runner / "receipt-validation-ack.json",
            source_manifest_sha256=result["source_manifest_sha256"],
            authenticated_environment=authenticated_environment,
        ),
    )
    expected_stdout = (
        f"Sumeragi v2 aggregate release receipt verified: {local_receipt_path}\n"
    ).encode()
    validator_snapshot = _read_file_at(
        evidence_fd,
        "validate-receipt.py",
        evidence / "validate-receipt.py",
        "archived receipt validator",
        maximum_bytes=_MAX_HELPER_BYTES,
    )
    if (
        ack["format"] != "iroha-sumeragi-v2-receipt-validation-ack"
        or type(ack["schema_version"]) is not int or ack["schema_version"] != 3
        or ack["profile"] != "release"
        or source_record != {
            "archive_id": "release-retained.source.v1",
            "manifest_sha256": result["source_manifest_sha256"],
        }
        or receipt_record != {
            "archive_id": "release-terminal.receipt.v1",
            "mode": f"{protected['receipt'].mode:04o}",
            "sha256": protected["receipt"].sha256,
            "size_bytes": protected["receipt"].size,
        }
        or validator_record["archive_id"]
        != "release-bootstrap.receipt-validator.v1"
        or validator_record["sha256"] != validator_snapshot.sha256
        or not isinstance(validator_record["bootstrap_completion_sha256"], str)
        or _DIGEST_RE.fullmatch(validator_record["bootstrap_completion_sha256"]) is None
        or type(receipt_record["size_bytes"]) is not int
        or type(ack["exit_status"]) is not int or ack["exit_status"] != 0
        or type(stdout_record["size_bytes"]) is not int
        or type(stderr_record["size_bytes"]) is not int
        or stdout_record != {
            "sha256": hashlib.sha256(expected_stdout).hexdigest(),
            "size_bytes": len(expected_stdout),
        }
        or stderr_record != {
            "sha256": hashlib.sha256(b"").hexdigest(),
            "size_bytes": 0,
        }
    ):
        raise BootstrapError("receipt validation acknowledgment contract is not exact")
    _remove_completed_runner_log(
        evidence_fd,
        private_snapshot,
        "bootstrap-private retained provenance",
    )
    try:
        os.stat(private_name, dir_fd=evidence_fd, follow_symlinks=False)
    except FileNotFoundError:
        pass
    except OSError as error:
        raise BootstrapError(
            "bootstrap-private retained provenance cleanup is indeterminate"
        ) from error
    else:
        raise BootstrapError(
            "bootstrap-private retained provenance survived authentication"
        )
    return release_runner, protected["receipt"].path, protected["sealed_identity"].path, result_snapshot, inventory_snapshot, ack_snapshot


def _receipt_validation_failure(
    evidence: Path,
    evidence_fd: int,
    identity: dict[str, Any],
    identity_snapshot: FileSnapshot,
    bootstrap_marker: FileSnapshot,
    protected_validator: FileSnapshot,
) -> tuple[FileSnapshot, dict[str, FileSnapshot]]:
    """Authenticate the bounded failure record published after root cleanup."""

    marker_snapshot = _read_file_at(
        evidence_fd,
        "RECEIPT_VALIDATION_FAILED.json",
        evidence / "RECEIPT_VALIDATION_FAILED.json",
        "receipt validation failure marker",
        maximum_bytes=_MAX_VALIDATOR_FAILURE_MARKER_BYTES,
    )
    marker = _require_exact_json_fields(
        _parse_canonical_json(marker_snapshot, "receipt validation failure marker"),
        {
            "format", "schema_version", "result", "stage", "profile",
            "bootstrap_completion_sha256", "candidate_identity",
            "sealed_source_manifest_sha256", "receipt", "validator", "argv",
            "diagnostics", "invocation_cleanup",
        },
        "receipt validation failure marker",
    )
    candidate = _require_exact_json_fields(
        marker["candidate_identity"], {"sha256", "head_commit", "head_tree"},
        "receipt validation failure candidate identity",
    )
    receipt = _require_exact_json_fields(
        marker["receipt"], {"disclosure", "sha256", "size_bytes"},
        "receipt validation failure receipt",
    )
    validator = _require_exact_json_fields(
        marker["validator"], {"archive_name", "sha256", "exit_status"},
        "receipt validation failure validator",
    )
    argv = _require_exact_json_fields(
        marker["argv"],
        {
            "profile",
            "python_flags",
            "validator",
            "operation",
            "invocation_binding",
        },
        "receipt validation failure argv",
    )
    diagnostics = _require_exact_json_fields(
        marker["diagnostics"], {"stdout", "stderr"},
        "receipt validation failure diagnostics",
    )
    if (
        marker["format"] != "iroha-sumeragi-v2-receipt-validation-failure"
        or type(marker["schema_version"]) is not int
        or marker["schema_version"] != 2
        or marker["result"] != "release-failed"
        or marker["stage"] != "protected-receipt-validation"
        or marker["profile"] != "release"
        or marker["bootstrap_completion_sha256"] != bootstrap_marker.sha256
        or candidate != {
            "sha256": identity_snapshot.sha256,
            "head_commit": identity["head_commit"],
            "head_tree": identity["head_tree"],
        }
        or not isinstance(marker["sealed_source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(marker["sealed_source_manifest_sha256"]) is None
        or receipt.get("disclosure") != "unverified-no-retention"
        or not isinstance(receipt.get("sha256"), str)
        or _DIGEST_RE.fullmatch(receipt["sha256"]) is None
        or type(receipt.get("size_bytes")) is not int
        or receipt["size_bytes"] < 0
        or validator.get("archive_name") != "validate-receipt.py"
        or validator.get("sha256") != protected_validator.sha256
        or type(validator.get("exit_status")) is not int
        or not 1 <= validator["exit_status"] <= 255
        or argv != {
            "profile": "release",
            "python_flags": ["-I", "-S"],
            "validator": "protected:validate-receipt.py",
            "operation": "verify-existing-and-ack",
            "invocation_binding": "not-published-validation-failed",
        }
        or marker["invocation_cleanup"] != "complete"
    ):
        raise BootstrapError("receipt validation failure marker contract is not exact")

    streams: dict[str, FileSnapshot] = {}
    for name in ("stdout", "stderr"):
        record = _require_exact_json_fields(
            diagnostics[name],
            {
                "name", "sha256", "captured_size_bytes", "observed_size_bytes",
                "truncated", "mode",
            },
            f"receipt validator {name} diagnostic",
        )
        expected_name = f"receipt-validator-failure.{name}"
        if (
            record.get("name") != expected_name
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
            or type(record.get("captured_size_bytes")) is not int
            or not 0 <= record["captured_size_bytes"] <= _MAX_VALIDATOR_DIAGNOSTIC_BYTES
            or type(record.get("observed_size_bytes")) is not int
            or record["observed_size_bytes"] < record["captured_size_bytes"]
            or type(record.get("truncated")) is not bool
            or record["truncated"]
            != (record["observed_size_bytes"] > record["captured_size_bytes"])
            or record.get("mode") != "0400"
        ):
            raise BootstrapError(f"receipt validator {name} diagnostic contract is not exact")
        snapshot = _read_file_at(
            evidence_fd,
            expected_name,
            evidence / expected_name,
            f"receipt validator {name} diagnostic",
            maximum_bytes=_MAX_VALIDATOR_DIAGNOSTIC_BYTES,
        )
        if (
            snapshot.sha256 != record["sha256"]
            or snapshot.size != record["captured_size_bytes"]
            or snapshot.mode != _DATA_MODE
            or snapshot.owner != os.getuid()
            or snapshot.nlink != 1
        ):
            raise BootstrapError(f"receipt validator {name} diagnostic changed")
        streams[name] = snapshot
    for name in (
        "BOOTSTRAP_RELEASE_COMPLETED.json", "RELEASE_COMPLETED.json",
        "receipt-validation-ack.json", "release-retained-inventory.json",
        "release-runner-private-provenance.json",
        "release-runner-result.json", "sealed-identity.json",
    ):
        try:
            os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        except FileNotFoundError:
            continue
        except OSError as error:
            raise BootstrapError("could not inspect failure-only evidence") from error
        raise BootstrapError("receipt validation failure retained success evidence")
    return marker_snapshot, streams


def _remove_completed_runner_log(
    evidence_fd: int, snapshot: LargeFileSnapshot, label: str
) -> None:
    """Remove a completed bootstrap-owned runner log at a quiescent boundary."""

    try:
        current = os.stat(snapshot.path.name, dir_fd=evidence_fd, follow_symlinks=False)
    except OSError as error:
        raise BootstrapError(f"{label} became unavailable before cleanup") from error
    if (
        not stat.S_ISREG(current.st_mode)
        or (current.st_dev, current.st_ino) != (snapshot.device, snapshot.inode)
        or current.st_uid != snapshot.owner
        or current.st_nlink != snapshot.nlink
        or current.st_size != snapshot.size
        or stat.S_IMODE(current.st_mode) != snapshot.mode
    ):
        raise BootstrapError(f"{label} changed before cleanup")
    try:
        os.unlink(snapshot.path.name, dir_fd=evidence_fd)
        os.fsync(evidence_fd)
    except OSError as error:
        raise BootstrapError(f"could not remove {label}") from error


def _prune_receipt_validation_failure(
    evidence: Path,
    evidence_fd: int,
    marker: FileSnapshot,
    streams: dict[str, FileSnapshot],
) -> None:
    """Retain only authenticated bounded bootstrap-owned failure diagnostics."""

    retained = {
        marker.path.name: marker,
        **{snapshot.path.name: snapshot for snapshot in streams.values()},
    }
    try:
        with os.scandir(evidence_fd) as entries:
            names = tuple(sorted(entry.name for entry in entries))
    except OSError as error:
        raise BootstrapError("could not enumerate failure-only evidence") from error
    for name in names:
        if name in retained:
            continue
        try:
            metadata = os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        except OSError as error:
            raise BootstrapError("failure-only evidence entry became unavailable") from error
        if metadata.st_uid != os.getuid():
            raise BootstrapError("failure-only cleanup refuses an unowned entry")
        path = evidence / name
        if stat.S_ISDIR(metadata.st_mode) and not stat.S_ISLNK(metadata.st_mode):
            _cleanup(path)
            if os.path.lexists(path):
                raise BootstrapError("failure-only directory cleanup did not complete")
            continue
        if not (stat.S_ISREG(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode)):
            raise BootstrapError("failure-only cleanup refuses a special entry")
        current = os.stat(name, dir_fd=evidence_fd, follow_symlinks=False)
        if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
            raise BootstrapError("failure-only cleanup entry was replaced")
        try:
            os.unlink(name, dir_fd=evidence_fd)
        except OSError as error:
            raise BootstrapError("failure-only evidence could not be pruned") from error
    os.fsync(evidence_fd)
    try:
        observed = set(os.listdir(evidence_fd))
    except OSError as error:
        raise BootstrapError("failure-only retained inventory is unavailable") from error
    if observed != set(retained):
        raise BootstrapError("failure-only retained inventory is not exact")
    for name, snapshot in retained.items():
        _require_unchanged(
            snapshot,
            f"retained failure diagnostic {name}",
            maximum_bytes=max(snapshot.size, 1),
        )


def _validate_terminal_receipt(
    *,
    evidence: Path,
    candidate: Path,
    bootstrap_marker: FileSnapshot,
    bootstrap_sha256: str,
    identity_snapshot: FileSnapshot,
    identity: dict[str, Any],
    runner_snapshot: FileSnapshot,
    runner_record: dict[str, Any],
    approval_record: dict[str, Any],
    protected: dict[str, FileSnapshot],
    identity_attestation: dict[str, Any],
    expected_signer_fingerprint: str,
    authenticated_environment: dict[str, str],
    release_runner: Path | None = None,
    receipt_path: Path | None = None,
) -> tuple[
    FileSnapshot,
    dict[str, Any],
    list[LargeFileSnapshot],
    list[DirectorySnapshot],
]:
    release_runner = release_runner or evidence / "release-runner"
    output = release_runner / "output"
    release = output / "release"
    directories = [
        _private_directory_snapshot(release_runner, "release-runner directory"),
        _private_directory_snapshot(output, "release output directory"),
        _private_directory_snapshot(release, "terminal receipt directory"),
    ]
    receipt_path = receipt_path or release / "RELEASE_COMPLETED.json"
    receipt_snapshot = _read_file(
        receipt_path,
        "terminal release receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    if (
        receipt_snapshot.mode != _DATA_MODE
        or receipt_snapshot.owner != os.getuid()
        or receipt_snapshot.nlink != 1
    ):
        raise BootstrapError(
            "terminal release receipt must be owner-owned, single-link, and mode 0400"
        )
    receipt = _parse_canonical_json(receipt_snapshot, "terminal release receipt")
    _require_exact_json_fields(
        receipt,
        {"schema_version", "protocol", "result", "identity", "authentication", "evidence"},
        "terminal release receipt",
    )
    if (
        type(receipt["schema_version"]) is not int
        or receipt["schema_version"] != 1
        or receipt["protocol"] != "sumeragi-v2"
        or receipt["result"] != "release-complete"
    ):
        raise BootstrapError("terminal release receipt does not record release completion")
    receipt_evidence = _require_exact_json_fields(
        receipt["evidence"], _TERMINAL_EVIDENCE_KEYS, "terminal release evidence"
    )
    bootstrap_evidence = _require_exact_json_fields(
        receipt_evidence["bootstrap"],
        {
            "completion",
            "candidate_identity",
            "runner",
            "candidate_cargo_lock",
            "trusted_inputs",
            "identity_verification",
            "runner_tools",
            "release_approvals",
        },
        "terminal bootstrap evidence",
    )
    if (
        not isinstance(bootstrap_evidence["trusted_inputs"], dict)
        or set(bootstrap_evidence["trusted_inputs"]) != set(protected)
        or not isinstance(bootstrap_evidence["identity_verification"], dict)
        or not isinstance(bootstrap_evidence["runner_tools"], dict)
        or set(bootstrap_evidence["runner_tools"])
        != set(runner_record["tools"])
    ):
        raise BootstrapError("terminal bootstrap evidence inventory is not exact")
    for label in (
        "corridor_completion",
        "formal_completion",
        "formal_verus_evidence",
        "formal_verus_log",
        "formal_cross_tool_evidence",
        "seed_matrix_completion",
        "chaos_completion",
    ):
        record = receipt_evidence[label]
        if (
            not isinstance(record, dict)
            or not isinstance(record.get("path"), str)
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
        ):
            raise BootstrapError(f"terminal release evidence {label} is malformed")

    receipt_identity = _require_exact_json_fields(
        receipt["identity"],
        {
            "head_commit",
            "head_tree",
            "index_tree",
            "cargo_lock_sha256",
            "candidate_source_manifest_sha256",
            "sealed_source_manifest_sha256",
        },
        "terminal release receipt identity",
    )
    expected_identity = {
        "head_commit": identity["head_commit"],
        "head_tree": identity["head_tree"],
        "index_tree": identity["index_tree"],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "candidate_source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
    }
    if any(receipt_identity.get(key) != value for key, value in expected_identity.items()):
        raise BootstrapError("terminal release receipt has the wrong candidate identity")
    if (
        not isinstance(receipt_identity["sealed_source_manifest_sha256"], str)
        or _DIGEST_RE.fullmatch(receipt_identity["sealed_source_manifest_sha256"])
        is None
    ):
        raise BootstrapError("terminal release receipt has an invalid sealed-source digest")
    terminal_artifacts, terminal_evidence_directories = (
        _validate_terminal_release_evidence(
            receipt_evidence=receipt_evidence,
            evidence=output,
            release_root=release_runner / "source",
            receipt_identity=receipt_identity,
            runner_record=runner_record,
            authenticated_environment=authenticated_environment,
        )
    )

    authentication = _require_exact_json_fields(
        receipt["authentication"],
        {"schema_version", "bootstrap", "release_identity"},
        "terminal release authentication",
    )
    if type(authentication["schema_version"]) is not int or authentication["schema_version"] != 2:
        raise BootstrapError("terminal release authentication has the wrong schema version")
    bootstrap = _require_exact_json_fields(
        authentication["bootstrap"],
        {
            "schema_version",
            "completion_sha256",
            "frozen_bootstrap_sha256",
            "candidate_identity_sha256",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "runner",
            "signer_fingerprint",
            "allowed_signers_principal",
            "trusted_input_digests",
            "trusted_input_archives",
            "release_approvals",
        },
        "terminal release bootstrap authentication",
    )
    if (
        type(bootstrap["schema_version"]) is not int
        or bootstrap["schema_version"] != 2
        or bootstrap["completion_sha256"] != bootstrap_marker.sha256
        or bootstrap["frozen_bootstrap_sha256"] != bootstrap_sha256
        or bootstrap["candidate_identity_sha256"] != identity_snapshot.sha256
        or bootstrap["candidate_commit_oid"] != identity["head_commit"]
        or bootstrap["candidate_tree_oid"] != identity["head_tree"]
    ):
        raise BootstrapError("terminal release receipt has the wrong bootstrap binding")
    marker_json = _parse_canonical_json(
        bootstrap_marker, "bootstrap completion marker"
    )
    marker_trusted = marker_json.get("trusted_inputs")
    if (
        not isinstance(marker_trusted, dict)
        or set(marker_trusted) != set(protected)
        or any(
            not isinstance(record, dict)
            or not isinstance(record.get("sha256"), str)
            or _DIGEST_RE.fullmatch(record["sha256"]) is None
            for record in marker_trusted.values()
        )
    ):
        raise BootstrapError(
            "bootstrap marker trusted-input inventory is malformed"
        )
    expected_trusted_digests = {
        label: record["sha256"]
        for label, record in sorted(marker_trusted.items())
    }
    if bootstrap["trusted_input_digests"] != expected_trusted_digests:
        raise BootstrapError("terminal release receipt has wrong trusted-input digests")
    try:
        bootstrap_probe_value = marker_json["trusted_execution_probes"][
            "runner_tool_closure"
        ]["value"]
    except (KeyError, TypeError) as error:
        raise BootstrapError(
            "terminal release omits its tool-probe binding"
        ) from error
    runtime_probe_path = release_runner / "runtime-tool-probe-result.json"
    bootstrap_probe_value = _validate_tool_probe_result(
        bootstrap_probe_value,
        runner_record["tools"],
        archive_id_prefix="release-runner-tool",
    )
    runtime_probe_value = _validate_tool_probe_result(
        _parse_canonical_json(
            _read_file(
                runtime_probe_path,
                "retained runtime tool probe result",
                maximum_bytes=1024 * 1024,
            ),
            "retained runtime tool probe result",
        ),
        runner_record["tools"],
        archive_id_prefix="release-runtime-tool",
    )
    if any(
        runtime_probe_value[field] != bootstrap_probe_value[field]
        for field in (
            "format", "host_family", "probe_contract_sha256",
            "schema_version", "tool_count",
        )
    ) or any(
        {
            key: value
            for key, value in runtime_probe_value["tools"][name].items()
            if key != "archive_id"
        }
        != {
            key: value
            for key, value in bootstrap_probe_value["tools"][name].items()
            if key != "archive_id"
        }
        for name in _REQUIRED_RUNNER_TOOL_NAMES
    ):
        raise BootstrapError(
            "retained runtime tool probes disagree with bootstrap replay"
        )
    expected_trusted_archives = {
        label: {
            key: record[key]
            for key in ("archive_id", "mode", "sha256", "size_bytes")
        }
        for label, record in sorted(marker_trusted.items())
        if isinstance(record, dict)
    }
    if bootstrap["trusted_input_archives"] != expected_trusted_archives:
        raise BootstrapError("terminal release receipt has wrong trusted-input archives")
    expected_release_approval_authentication = {
        "archive_id": _APPROVAL_SET_ARCHIVE_ID,
        "sha256": approval_record["set_attestation"]["sha256"],
        "operation_plan_sha256": approval_record[
            "operation_plan_sha256"
        ],
    }
    if (
        bootstrap["release_approvals"]
        != expected_release_approval_authentication
        or bootstrap_evidence["release_approvals"] != approval_record
        or marker_json.get("release_approvals") != approval_record
    ):
        raise BootstrapError(
            "terminal release receipt has the wrong approval binding"
        )
    if bootstrap["signer_fingerprint"] != expected_signer_fingerprint:
        raise BootstrapError("terminal release receipt has the wrong protected signer")
    if runner_snapshot.path != candidate / "scripts" / "run_sumeragi_v2_release_gates.sh":
        raise BootstrapError("terminal release receipt has the wrong runner root binding")
    receipt_runner = _require_exact_json_fields(
        bootstrap["runner"],
        {
            "archive_id",
            "sha256",
            "mode",
            "invocation",
            "closed_path_resolution",
            "output",
            "tool_directory",
            "tools",
            "environment_sha256",
            "self_digest_environment_variables",
        },
        "terminal release bootstrap runner",
    )
    expected_runner = {
        "archive_id": runner_record["archive_id"],
        "sha256": runner_snapshot.sha256,
        "mode": f"{runner_snapshot.mode:04o}",
        "invocation": runner_record["invocation"],
        "closed_path_resolution": runner_record["closed_path_resolution"],
        "output": runner_record["output"],
        "tool_directory": runner_record["tool_directory"],
        "tools": runner_record["tools"],
        "environment_sha256": runner_record["environment_sha256"],
        "self_digest_environment_variables": runner_record[
            "self_digest_environment_variables"
        ],
    }
    if receipt_runner != expected_runner:
        raise BootstrapError("terminal release receipt has the wrong runner binding")
    release_identity = _require_exact_json_fields(
        authentication["release_identity"],
        {
            "schema_version",
            "signature_format",
            "verification_status",
            "candidate_commit_oid",
            "candidate_tree_oid",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
            "trust_policy",
            "replay",
        },
        "terminal release identity authentication",
    )
    expected_release_identity = {
        "schema_version": 1,
        "signature_format": "ssh",
        "verification_status": "G",
        "candidate_commit_oid": identity["head_commit"],
        "candidate_tree_oid": identity["head_tree"],
        "signer_fingerprint": bootstrap["signer_fingerprint"],
        "allowed_signers_principal": bootstrap["allowed_signers_principal"],
    }
    for field, expected in expected_release_identity.items():
        if (
            field == "schema_version"
            and type(release_identity[field]) is not int
        ) or release_identity[field] != expected:
            raise BootstrapError(
                f"terminal release receipt has the wrong release identity {field}"
            )
    expected_trust_policy = {
        "git_sha256": protected["git"].sha256,
        "ssh_keygen_sha256": protected["ssh_keygen"].sha256,
        "allowed_signers_sha256": protected["allowed_signers"].sha256,
        "revocation_sha256": protected["revocation"].sha256,
        "signer_fingerprint": expected_signer_fingerprint,
    }
    if (
        release_identity["primary_key_fingerprint"] != ""
        or release_identity["trust_policy"] != expected_trust_policy
        or not isinstance(release_identity["replay"], dict)
        or release_identity["replay"].get("performed") is not True
        or release_identity["replay"].get("archive_ids") != _IDENTITY_ARCHIVE_IDS
    ):
        raise BootstrapError("terminal release identity trust evidence is not exact")
    return (
        receipt_snapshot,
        receipt,
        terminal_artifacts,
        [*directories, *terminal_evidence_directories],
    )


def _fsync_file_snapshot(snapshot: FileSnapshot, label: str) -> None:
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(snapshot.path, flags)
    try:
        opened = os.fstat(descriptor)
        if (
            (opened.st_dev, opened.st_ino) != (snapshot.device, snapshot.inode)
            or stat.S_IMODE(opened.st_mode) != snapshot.mode
            or opened.st_uid != snapshot.owner
            or opened.st_nlink != snapshot.nlink
            or opened.st_size != snapshot.size
        ):
            raise BootstrapError(f"{label} changed before fsync")
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _require_unchanged(
        snapshot, label, maximum_bytes=max(snapshot.size, 1), executable=False
    )


def _validate_retained_source(
    *,
    evidence: Path,
    receipt: dict[str, Any],
    candidate_identity: dict[str, Any],
    python: Path,
    manifest_helper: Path,
    environment: dict[str, str],
    timeout_seconds: int,
    release_runner: Path | None = None,
    sealed_identity_path: Path | None = None,
) -> tuple[FileSnapshot, dict[str, Any], DirectorySnapshot]:
    release_runner = release_runner or evidence / "release-runner"
    sealed_root = release_runner / "source"
    sealed_directory = _sealed_directory_snapshot(
        sealed_root, "retained sealed source root"
    )
    sealed_identity_snapshot = _read_file(
        sealed_identity_path or release_runner / "sealed-identity.json",
        "retained sealed identity",
        maximum_bytes=_MAX_IDENTITY_BYTES,
    )
    if (
        sealed_identity_snapshot.mode != _DATA_MODE
        or sealed_identity_snapshot.owner != os.getuid()
        or sealed_identity_snapshot.nlink != 1
    ):
        raise BootstrapError("retained sealed identity metadata is not exact")
    sealed_identity = _load_identity(sealed_identity_snapshot.data)
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if sealed_identity[field] != candidate_identity[field]:
            raise BootstrapError(
                f"retained sealed identity disagrees with candidate {field}"
            )
    receipt_identity = receipt["identity"]
    if receipt_identity["sealed_source_manifest_sha256"] != sealed_identity[
        "workspace_source_manifest_sha256"
    ]:
        raise BootstrapError("terminal receipt does not bind the retained sealed root")
    recomputed_bytes, recomputed_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if (
        recomputed_bytes != sealed_identity_snapshot.data
        or recomputed_identity != sealed_identity
    ):
        raise BootstrapError("retained sealed source does not reproduce its identity")
    _fsync_sealed_tree(sealed_root)
    _fsync_file_snapshot(sealed_identity_snapshot, "retained sealed identity")
    final_bytes, final_identity = _compute_identity(
        python, manifest_helper, sealed_root, environment, timeout_seconds
    )
    if final_bytes != recomputed_bytes or final_identity != recomputed_identity:
        raise BootstrapError("retained sealed source changed during durability closure")
    _require_sealed_directory_unchanged(
        sealed_directory, "retained sealed source root"
    )
    return sealed_identity_snapshot, sealed_identity, sealed_directory


def _receipt_artifact_path(
    receipt: dict[str, Any], label: str, evidence: Path
) -> Path:
    return _receipt_nested_artifact_path(receipt, (label,), evidence)


def _receipt_nested_artifact_path(
    receipt: dict[str, Any], fields: tuple[str | int, ...], containment_root: Path
) -> Path:
    value: Any = receipt.get("evidence")
    rendered_fields: list[str] = []
    for field in fields:
        rendered_fields.append(str(field))
        try:
            value = value[field]
        except (KeyError, IndexError, TypeError) as error:
            raise BootstrapError(
                f"terminal receipt omits {'.'.join(rendered_fields)}"
            ) from error
    record = value
    label = ".".join(rendered_fields)
    if not isinstance(record, dict) or not {"path", "sha256"}.issubset(record):
        raise BootstrapError(f"terminal receipt omits {label}")
    rendered = record["path"]
    digest = record["sha256"]
    if not isinstance(rendered, str) or not isinstance(digest, str):
        raise BootstrapError(f"terminal receipt {label} path is not text")
    _require_digest(digest, f"terminal receipt {label} digest")
    path = _absolute_resolved_existing(Path(rendered), f"terminal receipt {label}")
    if not _inside(path, containment_root):
        raise BootstrapError(
            f"terminal receipt {label} escaped its authenticated containment root"
        )
    snapshot = _capture_large_file(path, f"terminal receipt {label}")
    if snapshot.sha256 != digest:
        raise BootstrapError(f"terminal receipt {label} digest changed")
    return path


def _receipt_scaling_manifest_path(
    receipt: dict[str, Any], authenticated_environment: dict[str, str]
) -> Path:
    bundle = receipt.get("evidence", {}).get("multilane_scaling_bundle")
    if (
        not isinstance(bundle, dict)
        or bundle.get("archive_id") != "release-scaling.bundle.v1"
        or not isinstance(bundle.get("files"), list)
    ):
        raise BootstrapError("terminal receipt omits its scaling bundle")
    matching = [
        record
        for record in bundle["files"]
        if isinstance(record, dict)
        and record.get("relative_path") == "scaling_evidence.json"
    ]
    if len(matching) != 1:
        raise BootstrapError("terminal receipt scaling manifest inventory is not exact")
    rendered = authenticated_environment.get(
        "IROHA_RELEASE_SCALING_EVIDENCE_MANIFEST"
    )
    if not isinstance(rendered, str):
        raise BootstrapError("authenticated runner omits its scaling manifest")
    path = _absolute_resolved_existing(
        Path(rendered), "authenticated scaling manifest"
    )
    snapshot = _capture_large_file(path, "authenticated scaling manifest")
    record = matching[0]
    if (
        record.get("archive_id")
        != "release-scaling.file.v1:scaling_evidence.json"
        or record.get("sha256") != snapshot.sha256
        or record.get("size_bytes") != snapshot.size
        or _terminal_mode(record.get("mode"), "terminal scaling manifest")
        != snapshot.mode
    ):
        raise BootstrapError("terminal scaling manifest authentication is not exact")
    return path


def _run_protected_receipt_validator(
    *,
    evidence: Path,
    candidate: Path,
    receipt: dict[str, Any],
    receipt_snapshot: FileSnapshot,
    sealed_identity_snapshot: FileSnapshot,
    sealed_root: Path,
    archives: dict[str, FileSnapshot],
    protected: dict[str, FileSnapshot],
    identity_snapshot: FileSnapshot,
    identity_outputs: dict[str, Path],
    bootstrap_marker: FileSnapshot,
    expected_signer_fingerprint: str,
    environment: dict[str, str],
    timeout_seconds: int,
) -> CommandResult:
    release_output = sealed_root.parent / "output"
    local_receipt_path = release_output / "release" / "RELEASE_COMPLETED.json"
    local_receipt = _read_file(
        local_receipt_path,
        "retained terminal receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    if (
        local_receipt.data != receipt_snapshot.data
        or local_receipt.sha256 != receipt_snapshot.sha256
        or local_receipt.size != receipt_snapshot.size
        or local_receipt.mode != receipt_snapshot.mode
    ):
        raise BootstrapError(
            "retained and protected terminal receipt copies disagree"
        )

    def release_artifact_root(fields: tuple[str | int, ...]) -> Path:
        value: Any = receipt.get("evidence")
        for field in fields:
            try:
                value = value[field]
            except (KeyError, IndexError, TypeError) as error:
                raise BootstrapError(
                    f"terminal receipt omits {'.'.join(map(str, fields))}"
                ) from error
        if not isinstance(value, dict) or not isinstance(value.get("path"), str):
            raise BootstrapError(
                f"terminal receipt omits {'.'.join(map(str, fields))}"
            )
        path = Path(value["path"])
        if not _inside(path, release_output):
            raise BootstrapError(
                "terminal receipt "
                f"{'.'.join(map(str, fields))} artifact is outside its exact "
                "release output"
            )
        return release_output

    def receipt_artifact(label: str) -> Path:
        return _receipt_artifact_path(
            receipt, label, release_artifact_root((label,))
        )

    def nested_artifact(fields: tuple[str | int, ...]) -> Path:
        return _receipt_nested_artifact_path(
            receipt, fields, release_artifact_root(fields)
        )

    formal_replay = receipt.get("evidence", {}).get("formal_replay_release")
    if not isinstance(formal_replay, dict):
        raise BootstrapError("terminal receipt omits its formal replay release")
    formal_source_rendered = environment.get(
        "IROHA_RELEASE_FORMAL_REPLAY_SOURCE_RECEIPT"
    )
    formal_release_root_rendered = environment.get(
        "IROHA_RELEASE_FORMAL_REPLAY_RELEASE_ROOT"
    )
    if not isinstance(formal_source_rendered, str) or not isinstance(
        formal_release_root_rendered, str
    ):
        raise BootstrapError(
            "authenticated runner omits the formal replay release paths"
        )
    formal_source = _absolute_resolved_existing(
        Path(formal_source_rendered), "authenticated formal replay source receipt"
    )
    formal_release_root = _absolute_resolved_existing(
        Path(formal_release_root_rendered),
        "authenticated formal replay release root",
    )
    formal_source_receipt = _receipt_nested_artifact_path(
        receipt,
        ("formal_replay_release", "source_receipt"),
        formal_source.parent,
    )
    formal_release_receipt = _receipt_nested_artifact_path(
        receipt,
        ("formal_replay_release", "receipt"),
        formal_release_root,
    )
    if (
        formal_source_receipt != formal_source
        or formal_release_receipt != formal_release_root / "receipt.json"
        or not isinstance(formal_replay.get("signature"), dict)
        or not isinstance(formal_replay["signature"].get("sha256"), str)
        or not isinstance(formal_replay.get("principal"), str)
    ):
        raise BootstrapError(
            "terminal formal replay validator inputs are not exact"
        )
    scaling_digests: dict[str, str] = {}
    for field, environment_name in _SCALING_DIGEST_ENVIRONMENT.items():
        value = environment.get(environment_name)
        if not isinstance(value, str):
            raise BootstrapError(
                f"protected receipt validation lacks {environment_name}"
            )
        scaling_digests[field] = _require_digest(
            value, f"protected {environment_name}"
        )
    arguments = [
        "-I",
        "-S",
        str(archives["receipt_validator"].path),
        "--candidate-identity",
        str(identity_snapshot.path),
        "--sealed-identity",
        str(sealed_identity_snapshot.path),
        "--release-root",
        str(sealed_root),
        "--signature-attestation",
        str(identity_outputs["attestation"]),
        "--signature-transcript",
        str(identity_outputs["transcript"]),
        "--signature-raw-commit",
        str(identity_outputs["raw_commit"]),
        "--signature-cargo-lock",
        str(identity_outputs["cargo_lock"]),
        "--signature-allowed-signers",
        str(identity_outputs["allowed"]),
        "--signature-revocation",
        str(identity_outputs["revocation"]),
        "--signature-git",
        str(identity_outputs["git"]),
        "--signature-ssh-keygen",
        str(identity_outputs["ssh"]),
        "--expected-git-sha256",
        protected["git"].sha256,
        "--expected-ssh-keygen-sha256",
        protected["ssh_keygen"].sha256,
        "--expected-allowed-signers-sha256",
        protected["allowed_signers"].sha256,
        "--expected-revocation-sha256",
        protected["revocation"].sha256,
        "--expected-signer-fingerprint",
        expected_signer_fingerprint,
        "--bootstrap-completion",
        str(bootstrap_marker.path),
        "--bootstrap-evidence-dir",
        str(evidence),
        "--bootstrap-identity",
        str(identity_snapshot.path),
        "--bootstrap-attestation",
        str(identity_outputs["attestation"]),
        "--bootstrap-transcript",
        str(identity_outputs["transcript"]),
        "--expected-bootstrap-completion-sha256",
        bootstrap_marker.sha256,
        "--bootstrap-candidate-root",
        str(candidate),
        "--bootstrap-runner",
        str(candidate / "scripts" / "run_sumeragi_v2_release_gates.sh"),
        "--corridor-completion",
        str(receipt_artifact("corridor_completion")),
        "--formal-completion",
        str(receipt_artifact("formal_completion")),
        "--formal-replay-source-receipt",
        str(formal_source_receipt),
        "--formal-replay-release-root",
        str(formal_release_root),
        "--expected-formal-replay-signature-sha256",
        formal_replay["signature"]["sha256"],
        "--formal-replay-principal",
        formal_replay["principal"],
        "--seed-completion",
        str(receipt_artifact("seed_matrix_completion")),
        "--chaos-completion",
        str(receipt_artifact("chaos_completion")),
        "--g4p-completion",
        str(
            nested_artifact(("g4p_multilane", "completion"))
        ),
        "--g12-seed-completion",
        str(
            nested_artifact(("g12_cross_dataspace", "seed_completion"))
        ),
        "--g12-fault-soak-completion",
        str(
            nested_artifact(
                ("g12_cross_dataspace", "fault_soak_completion")
            )
        ),
        "--scaling-evidence-manifest",
        str(_receipt_scaling_manifest_path(receipt, environment)),
        "--sdk-dependency-archive",
        str(sealed_root.parent / "sdk-dependency-bundle.tar"),
        "--sdk-dependency-input-inventory",
        str(sealed_root.parent / "sdk-dependency-input.json"),
        "--sdk-dependency-final-work-inventory",
        str(sealed_root.parent / "sdk-dependency-work-final.json"),
        "--runtime-tool-probe-manifest",
        str(sealed_root.parent / "runtime-tool-probe-manifest.json"),
        "--runtime-tool-probe-result",
        str(sealed_root.parent / "runtime-tool-probe-result.json"),
        "--expected-scaling-trial-harness-sha256",
        scaling_digests["trial_harness_sha256"],
        "--expected-scaling-configuration-sha256",
        scaling_digests["configuration_sha256"],
        "--expected-scaling-irohad-sha256",
        scaling_digests["irohad_sha256"],
        "--expected-scaling-iroha-cli-sha256",
        scaling_digests["iroha_cli_sha256"],
        "--repository-root",
        str(sealed_root),
        "--output",
        str(local_receipt.path),
        "--replay-existing",
    ]
    result = _run_bounded(
        archives["python"].path,
        arguments,
        cwd=sealed_root,
        environment={
            key: value
            for key, value in environment.items()
            if key
            not in {
                "IROHA_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "SUMERAGI_V2_RELEASE_EXPECTED_BOOTSTRAP_COMPLETION_SHA256",
                "PYTHONDONTWRITEBYTECODE",
                "PYTHONHASHSEED",
            }
        },
        timeout_seconds=timeout_seconds,
        maximum_output_bytes=_MAX_HELPER_OUTPUT_BYTES,
    )
    expected_stdout = (
        f"Sumeragi v2 aggregate release receipt replayed: "
        f"{local_receipt.path}\n"
    ).encode()
    if result.returncode != 0 or result.stdout != expected_stdout or result.stderr:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise BootstrapError(
            "protected receipt validator rejected terminal receipt "
            f"with status {result.returncode}: {detail}"
        )
    _require_unchanged(
        local_receipt,
        "protected-validator retained terminal receipt",
        maximum_bytes=_MAX_TERMINAL_RECEIPT_BYTES,
    )
    return result


def _validate_command_record(
    record: Any,
    label: str,
    *,
    require_success: bool,
) -> None:
    expected_keys = {
        "argv",
        "replay_argv",
        "exit_status",
        "stdout_base64",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_base64",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    if not isinstance(record, dict) or set(record) != expected_keys:
        raise BootstrapError(f"identity transcript has invalid {label} evidence")
    for key in ("argv", "replay_argv"):
        value = record[key]
        if not isinstance(value, list) or not value or not all(
            isinstance(argument, str) for argument in value
        ):
            raise BootstrapError(f"identity transcript has invalid {label} {key}")
    exit_status = record["exit_status"]
    if type(exit_status) is not int or exit_status < 0:
        raise BootstrapError(f"identity transcript has invalid {label} exit status")
    if require_success and exit_status != 0:
        raise BootstrapError(f"identity transcript records failed {label}")
    for stream in ("stdout", "stderr"):
        encoded = record[f"{stream}_base64"]
        digest = record[f"{stream}_sha256"]
        size = record[f"{stream}_size_bytes"]
        if not isinstance(encoded, str) or not isinstance(digest, str):
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        if _DIGEST_RE.fullmatch(digest) is None or type(size) is not int or size < 0:
            raise BootstrapError(f"identity transcript has invalid {label} {stream}")
        try:
            decoded = base64.b64decode(encoded, validate=True)
        except (ValueError, binascii.Error) as error:
            raise BootstrapError(
                f"identity transcript has invalid {label} {stream} encoding"
            ) from error
        if len(decoded) != size or hashlib.sha256(decoded).hexdigest() != digest:
            raise BootstrapError(
                f"identity transcript has inconsistent {label} {stream} evidence"
            )


def _validate_sanitized_operation(
    value: Any,
    label: str,
    *,
    operation_id: str,
    exit_status: int,
) -> None:
    expected = {
        "operation_id",
        "exit_status",
        "stdout_sha256",
        "stdout_size_bytes",
        "stderr_sha256",
        "stderr_size_bytes",
    }
    if not isinstance(value, dict) or set(value) != expected:
        raise BootstrapError(
            f"identity transcript has invalid {label} operation"
        )
    if (
        value["operation_id"] != operation_id
        or type(value["exit_status"]) is not int
        or value["exit_status"] != exit_status
    ):
        raise BootstrapError(
            f"identity transcript has the wrong {label} operation binding"
        )
    for stream in ("stdout", "stderr"):
        digest = value[f"{stream}_sha256"]
        size = value[f"{stream}_size_bytes"]
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or type(size) is not int
            or size < 0
            or size > _MAX_HELPER_OUTPUT_BYTES
        ):
            raise BootstrapError(
                f"identity transcript has invalid {label} {stream} metadata"
            )


def _validate_raw_commit(raw: bytes, identity: dict[str, Any]) -> None:
    """Authenticate archived commit bytes against the candidate and trailers."""

    headers, separator, message = raw.partition(b"\n\n")
    if not separator or b"\r" in headers or b"\0" in headers:
        raise BootstrapError("identity raw commit has malformed headers")
    records: list[tuple[bytes, list[bytes]]] = []
    for line in headers.split(b"\n"):
        if line.startswith(b" "):
            if not records:
                raise BootstrapError("identity raw commit has an orphan folded header")
            records[-1][1].append(line[1:])
            continue
        key, marker, field = line.partition(b" ")
        if not marker or not key or any(byte < 0x21 or byte > 0x7E for byte in key):
            raise BootstrapError("identity raw commit has a malformed header")
        records.append((key, [field]))
    trees = [values for key, values in records if key == b"tree"]
    if trees != [[identity["head_tree"].encode("ascii")]]:
        raise BootstrapError("identity raw commit tree does not match the candidate")
    signatures = [values for key, values in records if key.startswith(b"gpgsig")]
    if len(signatures) != 1 or not any(key == b"gpgsig" for key, _ in records):
        raise BootstrapError("identity raw commit must contain exactly one SSH signature")
    signature = b"\n".join(signatures[0])
    lines = signature.split(b"\n")
    if len(lines) < 3 or lines[0] != _SSH_BEGIN or lines[-1] != _SSH_END:
        raise BootstrapError("identity raw commit has invalid SSH signature armor")
    try:
        if not base64.b64decode(b"".join(lines[1:-1]), validate=True):
            raise ValueError
    except (ValueError, binascii.Error) as error:
        raise BootstrapError("identity raw commit has malformed SSH signature data") from error
    if b"\r" in message or b"\0" in message:
        raise BootstrapError("identity raw commit has a malformed LF-only message")
    try:
        text = message.decode("utf-8")
    except UnicodeDecodeError as error:
        raise BootstrapError("identity raw commit message is not UTF-8") from error
    expected = [
        f"{_TRAILER_VERSION}: 1",
        f"{_TRAILER_MANIFEST}: {identity['workspace_source_manifest_sha256']}",
        f"{_TRAILER_LOCK}: {identity['cargo_lock_sha256']}",
    ]
    text_lines = text[:-1].split("\n") if text.endswith("\n") else []
    trailer_keys = {
        _TRAILER_VERSION.casefold(),
        _TRAILER_MANIFEST.casefold(),
        _TRAILER_LOCK.casefold(),
    }
    recognized = [
        index
        for index, line in enumerate(text_lines)
        if ":" in line and line.partition(":")[0].casefold() in trailer_keys
    ]
    terminal = list(range(len(text_lines) - 3, len(text_lines)))
    if (
        len(text_lines) < 5
        or text_lines[-4] != ""
        or not text_lines[-5]
        or text_lines[-3:] != expected
        or recognized != terminal
    ):
        raise BootstrapError("identity raw commit has the wrong release trailer block")
    framed = b"commit " + str(len(raw)).encode("ascii") + b"\0" + raw
    observed_oid = (
        hashlib.sha1(framed, usedforsecurity=False).hexdigest()
        if len(identity["head_commit"]) == 40
        else hashlib.sha256(framed).hexdigest()
    )
    if observed_oid != identity["head_commit"]:
        raise BootstrapError("identity raw commit bytes do not reproduce HEAD")


def _validate_legacy_identity_evidence(
    directory: Path,
    identity: dict[str, Any],
    identity_bytes: bytes,
    expected: dict[str, str],
) -> tuple[dict[str, FileSnapshot], dict[str, Any], dict[str, Any]]:
    attestation = _read_file(
        directory / "identity-attestation.json",
        "identity attestation",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    transcript = _read_file(
        directory / "identity-transcript.json",
        "identity transcript",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    attestation_json = _parse_canonical_json(attestation, "identity attestation")
    transcript_json = _parse_canonical_json(transcript, "identity transcript")
    if attestation.mode != _DATA_MODE or transcript.mode != _DATA_MODE:
        raise BootstrapError("identity attestation and transcript must have exact mode 0400")
    if set(attestation_json) != _ATTESTATION_KEYS:
        raise BootstrapError("identity attestation has the wrong schema")
    if set(transcript_json) != _TRANSCRIPT_KEYS:
        raise BootstrapError("identity transcript has the wrong schema")
    if (
        type(attestation_json.get("schema_version")) is not int
        or attestation_json["schema_version"] != 2
    ):
        raise BootstrapError("identity attestation must use schema version 2")
    if (
        type(transcript_json.get("schema_version")) is not int
        or transcript_json["schema_version"] != 2
    ):
        raise BootstrapError("identity transcript must use schema version 2")
    if attestation_json.get("release_identity") != identity:
        raise BootstrapError("identity attestation does not bind the candidate identity")
    if attestation_json.get("release_identity_sha256") != hashlib.sha256(
        identity_bytes
    ).hexdigest():
        raise BootstrapError("identity attestation has the wrong identity digest")
    verification = attestation_json.get("verification")
    if (
        not isinstance(verification, dict)
        or verification.get("signer_fingerprint") != expected["fingerprint"]
    ):
        raise BootstrapError("identity attestation has the wrong signer fingerprint")
    if verification.get("status") != "G":
        raise BootstrapError("identity attestation is not a good SSH signature")
    if verification.get("primary_key_fingerprint") != "":
        raise BootstrapError("identity attestation is not first-release SSH metadata")
    if not isinstance(verification.get("allowed_signers_principal"), str) or not verification.get(
        "allowed_signers_principal"
    ):
        raise BootstrapError("identity attestation omits its allowed-signers principal")
    tools = attestation_json.get("tools")
    if not isinstance(tools, dict):
        raise BootstrapError("identity attestation omits protected tools")
    for key, digest_key in (("git", "git"), ("ssh_keygen", "ssh")):
        item = tools.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0500":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    policies = attestation_json.get("policies")
    if not isinstance(policies, dict) or policies.get("signature_format") != "ssh":
        raise BootstrapError("identity attestation does not bind SSH policy")
    if policies.get("expected_signer_fingerprint") != expected["fingerprint"]:
        raise BootstrapError("identity attestation has the wrong protected fingerprint")
    for key, digest_key in (("ssh_allowed_signers", "allowed"), ("ssh_revocation", "revocation")):
        item = policies.get(key)
        if not isinstance(item, dict):
            raise BootstrapError(f"identity attestation omits {key}")
        if (
            item.get("observed_sha256") != expected[digest_key]
            or item.get("protected_sha256") != expected[digest_key]
        ):
            raise BootstrapError(f"identity attestation has the wrong {key} digest")
        if item.get("mode") != "0400":
            raise BootstrapError(f"identity attestation has the wrong {key} mode")
        if type(item.get("size_bytes")) is not int or item["size_bytes"] < 0:
            raise BootstrapError(f"identity attestation has invalid {key} size")
    evidence = attestation_json.get("evidence")
    if not isinstance(evidence, dict) or set(evidence) != _EVIDENCE_KEYS:
        raise BootstrapError("identity attestation has the wrong evidence inventory")
    snapshots: dict[str, FileSnapshot] = {
        "identity_attestation": attestation,
        "identity_transcript": transcript,
    }
    expected_archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    seen_names: set[str] = set()
    for label, record in evidence.items():
        if not isinstance(record, dict):
            raise BootstrapError(f"identity evidence record {label} is invalid")
        name = record.get("archive_name")
        if (
            not isinstance(name, str)
            or not name
            or name in {".", ".."}
            or "/" in name
            or name in seen_names
        ):
            raise BootstrapError(f"identity evidence record {label} has an invalid archive name")
        seen_names.add(name)
        if name != expected_archive_names[label]:
            raise BootstrapError(f"identity evidence {label} has the wrong archive name")
        mode_text = record.get("mode")
        expected_mode = _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        if mode_text != f"{expected_mode:04o}":
            raise BootstrapError(f"identity evidence {label} has the wrong protected mode")
        digest = record.get("sha256")
        if digest is None:
            digest = record.get("observed_sha256")
        if not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None:
            raise BootstrapError(f"identity evidence {label} has an invalid digest")
        size = record.get("size_bytes")
        if type(size) is not int or size < 0 or size > _MAX_EVIDENCE_BYTES:
            raise BootstrapError(f"identity evidence {label} has an invalid size")
        snapshot = _read_file(
            directory / name,
            f"identity evidence {label}",
            maximum_bytes=_MAX_EVIDENCE_BYTES,
            executable=expected_mode == _TOOL_MODE,
        )
        if (
            snapshot.mode != expected_mode
            or len(snapshot.data) != size
            or snapshot.sha256 != digest
        ):
            raise BootstrapError(f"identity evidence {label} does not match its attestation")
        snapshots[label] = snapshot
    _validate_allowed_signers_policy(snapshots["ssh_allowed_signers"].data)
    transcript_record = evidence["verify_transcript"]
    transcript_digest = transcript_record.get("sha256")
    if transcript_digest is None:
        transcript_digest = transcript_record.get("observed_sha256")
    if (
        transcript_record.get("archive_name") != transcript.path.name
        or transcript_digest != transcript.sha256
    ):
        raise BootstrapError("identity transcript does not match its attested evidence record")
    if transcript_json.get("candidate_commit_oid") != identity["head_commit"]:
        raise BootstrapError("identity transcript has the wrong candidate commit")
    if transcript_json.get("archive_names") != expected_archive_names:
        raise BootstrapError("identity transcript has the wrong replay archive mapping")
    if transcript_json.get("tools") != tools or transcript_json.get("policies") != policies:
        raise BootstrapError("identity transcript disagrees with the attestation")
    commands = transcript_json.get("commands")
    if not isinstance(commands, dict) or set(commands) != {
        "show_signature_metadata",
        "verify_commit",
    }:
        raise BootstrapError("identity transcript has the wrong command inventory")
    _validate_command_record(
        commands["show_signature_metadata"],
        "show-signature command",
        require_success=True,
    )
    _validate_command_record(
        commands["verify_commit"],
        "verify-commit command",
        require_success=True,
    )
    probes = transcript_json.get("tool_probes")
    if not isinstance(probes, dict) or set(probes) != {"ssh_keygen_usage"}:
        raise BootstrapError("identity transcript has the wrong tool-probe inventory")
    _validate_command_record(
        probes["ssh_keygen_usage"],
        "ssh-keygen probe",
        require_success=False,
    )
    if tools["git"]["size_bytes"] != evidence["git"]["size_bytes"]:
        raise BootstrapError("identity Git size disagrees with its evidence")
    if tools["ssh_keygen"]["size_bytes"] != evidence["ssh_keygen"]["size_bytes"]:
        raise BootstrapError("identity ssh-keygen size disagrees with its evidence")
    if (
        policies["ssh_allowed_signers"]["size_bytes"]
        != evidence["ssh_allowed_signers"]["size_bytes"]
    ):
        raise BootstrapError("allowed-signers size disagrees with its evidence")
    if (
        policies["ssh_revocation"]["size_bytes"]
        != evidence["ssh_revocation"]["size_bytes"]
    ):
        raise BootstrapError("SSH revocation size disagrees with its evidence")
    return snapshots, attestation_json, transcript_json


def _validate_identity_evidence(
    directory: Path,
    identity: dict[str, Any],
    identity_bytes: bytes,
    expected: dict[str, str],
) -> tuple[dict[str, FileSnapshot], dict[str, Any], dict[str, Any]]:
    """Authenticate path-free identity documents against local archive bytes."""

    attestation = _read_file(
        directory / "identity-attestation.json",
        "identity attestation",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    transcript = _read_file(
        directory / "identity-transcript.json",
        "identity transcript",
        maximum_bytes=_MAX_EVIDENCE_BYTES,
    )
    if attestation.mode != _DATA_MODE or transcript.mode != _DATA_MODE:
        raise BootstrapError(
            "identity attestation and transcript must have exact mode 0400"
        )
    attestation_json = _parse_canonical_json(
        attestation, "identity attestation"
    )
    transcript_json = _parse_canonical_json(transcript, "identity transcript")
    if set(attestation_json) != _ATTESTATION_KEYS:
        raise BootstrapError("identity attestation has the wrong schema")
    if set(transcript_json) != _TRANSCRIPT_KEYS:
        raise BootstrapError("identity transcript has the wrong schema")
    if (
        attestation_json["format"] != _IDENTITY_ATTESTATION_FORMAT
        or type(attestation_json["schema_version"]) is not int
        or attestation_json["schema_version"] != 3
        or transcript_json["format"] != _IDENTITY_TRANSCRIPT_FORMAT
        or type(transcript_json["schema_version"]) is not int
        or transcript_json["schema_version"] != 3
    ):
        raise BootstrapError("identity documents must use sanitized schema 3")
    candidate = attestation_json["candidate"]
    if not isinstance(candidate, dict) or candidate != {
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": hashlib.sha256(identity_bytes).hexdigest(),
    }:
        raise BootstrapError("identity attestation does not bind the candidate")

    archive_names = {
        "cargo_lock": "identity-Cargo.lock",
        "git": "identity-git",
        "raw_commit": "identity-raw-commit",
        "ssh_allowed_signers": "identity-allowed-signers",
        "ssh_keygen": "identity-ssh-keygen",
        "ssh_revocation": "identity-revocation",
        "verify_transcript": "identity-transcript.json",
    }
    records = attestation_json["archives"]
    if not isinstance(records, dict) or set(records) != set(_IDENTITY_ARCHIVE_IDS):
        raise BootstrapError("identity attestation has the wrong archive inventory")
    snapshots: dict[str, FileSnapshot] = {
        "identity_attestation": attestation,
        "identity_transcript": transcript,
    }
    for label, archive_id in _IDENTITY_ARCHIVE_IDS.items():
        record = records[label]
        if (
            not isinstance(record, dict)
            or set(record) != {"archive_id", "mode", "sha256", "size_bytes"}
            or record["archive_id"] != archive_id
        ):
            raise BootstrapError(
                f"identity archive {label} has an invalid protected record"
            )
        expected_mode = (
            _TOOL_MODE if label in {"git", "ssh_keygen"} else _DATA_MODE
        )
        if record["mode"] != f"{expected_mode:04o}":
            raise BootstrapError(f"identity archive {label} has the wrong mode")
        digest = record["sha256"]
        size = record["size_bytes"]
        if (
            not isinstance(digest, str)
            or _DIGEST_RE.fullmatch(digest) is None
            or type(size) is not int
            or size < 0
            or size > _MAX_EVIDENCE_BYTES
        ):
            raise BootstrapError(
                f"identity archive {label} has invalid integrity metadata"
            )
        snapshot = (
            transcript
            if label == "verify_transcript"
            else _read_file(
                directory / archive_names[label],
                f"identity archive {label}",
                maximum_bytes=_MAX_EVIDENCE_BYTES,
                executable=expected_mode == _TOOL_MODE,
            )
        )
        if (
            snapshot.mode != expected_mode
            or snapshot.sha256 != digest
            or snapshot.size != size
        ):
            raise BootstrapError(
                f"identity archive {label} does not match authenticated bytes"
            )
        snapshots[label] = snapshot
    for label, digest_key in (
        ("git", "git"),
        ("ssh_keygen", "ssh"),
        ("ssh_allowed_signers", "allowed"),
        ("ssh_revocation", "revocation"),
    ):
        if snapshots[label].sha256 != expected[digest_key]:
            raise BootstrapError(
                f"identity archive {label} has the wrong protected digest"
            )
    if snapshots["cargo_lock"].sha256 != identity["cargo_lock_sha256"]:
        raise BootstrapError("identity Cargo.lock archive has the wrong digest")
    _validate_allowed_signers_policy(snapshots["ssh_allowed_signers"].data)

    if (
        transcript_json["archive_ids"] != _IDENTITY_ARCHIVE_IDS
        or transcript_json["candidate_commit_oid"] != identity["head_commit"]
    ):
        raise BootstrapError("identity transcript has the wrong archive binding")
    operations = transcript_json["operations"]
    if not isinstance(operations, dict) or set(operations) != {
        "show_signature_metadata",
        "verify_commit",
        "ssh_keygen_usage",
    }:
        raise BootstrapError("identity transcript has the wrong operation inventory")
    _validate_sanitized_operation(
        operations["show_signature_metadata"],
        "show-signature",
        operation_id="git.show-signature-metadata.ssh.v1",
        exit_status=0,
    )
    _validate_sanitized_operation(
        operations["verify_commit"],
        "verify-commit",
        operation_id="git.verify-commit.ssh.v1",
        exit_status=0,
    )
    _validate_sanitized_operation(
        operations["ssh_keygen_usage"],
        "ssh-keygen",
        operation_id="ssh-keygen.usage-probe.v1",
        exit_status=1,
    )
    _validate_raw_commit(snapshots["raw_commit"].data, identity)
    return snapshots, attestation_json, transcript_json


def _validate_private_identity_provenance(
    snapshot: FileSnapshot,
    *,
    identity: dict[str, Any],
    identity_snapshot: FileSnapshot,
    candidate: Path,
    private_outputs: dict[str, Path],
    private_snapshots: dict[str, FileSnapshot],
    protected: dict[str, FileSnapshot],
) -> None:
    """Authenticate the path-bearing verifier record before deleting it."""

    value = _parse_canonical_json(
        snapshot, "bootstrap-private identity provenance"
    )
    _require_exact_json_fields(
        value,
        {
            "format",
            "schema_version",
            "candidate",
            "outputs",
            "archive_names",
            "tools",
            "policies",
            "verification",
            "execution",
            "sanitized_transcript",
        },
        "bootstrap-private identity provenance",
    )
    if (
        value["format"] != _IDENTITY_PRIVATE_PROVENANCE_FORMAT
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
    ):
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong schema"
        )
    private_directory = snapshot.path.parent
    expected_candidate = {
        "root_path": str(candidate),
        "identity_source_path": str(identity_snapshot.path),
        "cargo_lock_source_path": str(candidate / "Cargo.lock"),
        "commit_oid": identity["head_commit"],
        "tree_oid": identity["head_tree"],
        "source_manifest_sha256": identity[
            "workspace_source_manifest_sha256"
        ],
        "cargo_lock_sha256": identity["cargo_lock_sha256"],
        "release_identity_sha256": identity_snapshot.sha256,
    }
    if value["candidate"] != expected_candidate:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong candidate"
        )
    expected_outputs = {
        "attestation": str(private_outputs["attestation"]),
        "bootstrap-private provenance": str(private_outputs["provenance"]),
        "verify transcript": str(private_outputs["transcript"]),
        "raw commit": str(private_outputs["raw_commit"]),
        "Cargo.lock archive": str(private_outputs["cargo_lock"]),
        "SSH allowed-signers archive": str(private_outputs["allowed"]),
        "SSH revocation-policy archive": str(private_outputs["revocation"]),
        "Git archive": str(private_outputs["git"]),
        "ssh-keygen archive": str(private_outputs["ssh"]),
    }
    if value["outputs"] != expected_outputs:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong output paths"
        )
    expected_archive_names = {
        "cargo_lock": private_outputs["cargo_lock"].name,
        "git": private_outputs["git"].name,
        "raw_commit": private_outputs["raw_commit"].name,
        "ssh_allowed_signers": private_outputs["allowed"].name,
        "ssh_keygen": private_outputs["ssh"].name,
        "ssh_revocation": private_outputs["revocation"].name,
        "verify_transcript": private_outputs["transcript"].name,
    }
    if value["archive_names"] != expected_archive_names:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong archive names"
        )
    tools = value["tools"]
    if not isinstance(tools, dict) or set(tools) != {"git", "ssh_keygen"}:
        raise BootstrapError(
            "bootstrap-private identity provenance has the wrong tool inventory"
        )
    tool_expectations = {
        "git": ("git", "git", _IDENTITY_ARCHIVE_IDS["git"]),
        "ssh_keygen": ("ssh", "ssh_keygen", _IDENTITY_ARCHIVE_IDS["ssh_keygen"]),
    }
    for label, (snapshot_label, protected_label, archive_id) in tool_expectations.items():
        record = _require_exact_json_fields(
            tools[label],
            {
                "archive_id",
                "archive_path",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_path",
            },
            f"bootstrap-private identity tool {label}",
        )
        archived = private_snapshots[snapshot_label]
        if (
            record["archive_id"] != archive_id
            or record["mode"] != "0500"
            or record["observed_sha256"] != archived.sha256
            or record["protected_sha256"] != protected[protected_label].sha256
            or record["size_bytes"] != archived.size
            or record["source_path"] != str(protected[protected_label].path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity tool {label} binding is not exact"
            )
        archive_path = Path(record["archive_path"])
        if (
            archive_path.parent != private_directory
            or not archive_path.name.startswith(
                "." + expected_archive_names[
                    "git" if label == "git" else "ssh_keygen"
                ] + ".stage."
            )
            or os.path.lexists(archive_path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity tool {label} stage is invalid"
            )

    policies = _require_exact_json_fields(
        value["policies"],
        {
            "expected_signer_fingerprint",
            "signature_format",
            "ssh_allowed_signers",
            "ssh_revocation",
        },
        "bootstrap-private identity policies",
    )
    if (
        policies["expected_signer_fingerprint"]
        != value["verification"].get("signer_fingerprint")
        or policies["signature_format"] != "ssh"
    ):
        raise BootstrapError(
            "bootstrap-private identity signature policy is not exact"
        )
    for label, private_label, protected_label in (
        ("ssh_allowed_signers", "allowed", "allowed_signers"),
        ("ssh_revocation", "revocation", "revocation"),
    ):
        record = _require_exact_json_fields(
            policies[label],
            {
                "archive_id",
                "archive_path",
                "mode",
                "observed_sha256",
                "protected_sha256",
                "size_bytes",
                "source_path",
            },
            f"bootstrap-private identity policy {label}",
        )
        archived = private_snapshots[private_label]
        if (
            record["archive_id"] != _IDENTITY_ARCHIVE_IDS[label]
            or record["mode"] != "0400"
            or record["observed_sha256"] != archived.sha256
            or record["protected_sha256"] != protected[protected_label].sha256
            or record["size_bytes"] != archived.size
            or record["source_path"] != str(protected[protected_label].path)
        ):
            raise BootstrapError(
                f"bootstrap-private identity policy {label} binding is not exact"
            )
    verification = _require_exact_json_fields(
        value["verification"],
        {
            "signature_format",
            "status",
            "signer_fingerprint",
            "primary_key_fingerprint",
            "allowed_signers_principal",
        },
        "bootstrap-private identity verification",
    )
    if (
        verification["signature_format"] != "ssh"
        or verification["status"] != "G"
        or verification["primary_key_fingerprint"] != ""
        or _FINGERPRINT_RE.fullmatch(
            str(verification["signer_fingerprint"])
        )
        is None
        or not isinstance(verification["allowed_signers_principal"], str)
        or not verification["allowed_signers_principal"]
    ):
        raise BootstrapError(
            "bootstrap-private identity verification is not trusted SSH"
        )
    execution = _require_exact_json_fields(
        value["execution"],
        {
            "environment",
            "policy_overrides",
            "replay",
            "commands",
            "tool_probes",
        },
        "bootstrap-private identity execution",
    )
    if (
        not isinstance(execution["environment"], dict)
        or execution["environment"].get("HOME") != str(private_directory)
        or not isinstance(execution["policy_overrides"], list)
    ):
        raise BootstrapError(
            "bootstrap-private identity execution environment is not exact"
        )
    commands = execution["commands"]
    if not isinstance(commands, dict) or set(commands) != {
        "show_signature_metadata",
        "verify_commit",
    }:
        raise BootstrapError(
            "bootstrap-private identity command inventory is not exact"
        )
    _validate_command_record(
        commands["show_signature_metadata"],
        "bootstrap-private show-signature",
        require_success=True,
    )
    _validate_command_record(
        commands["verify_commit"],
        "bootstrap-private verify-commit",
        require_success=True,
    )
    probes = execution["tool_probes"]
    if not isinstance(probes, dict) or set(probes) != {"ssh_keygen_usage"}:
        raise BootstrapError(
            "bootstrap-private identity probe inventory is not exact"
        )
    _validate_command_record(
        probes["ssh_keygen_usage"],
        "bootstrap-private ssh-keygen",
        require_success=False,
    )
    transcript_record = value["sanitized_transcript"]
    expected_transcript_record = {
        "archive_id": _IDENTITY_ARCHIVE_IDS["verify_transcript"],
        "mode": "0400",
        "sha256": private_snapshots["transcript"].sha256,
        "size_bytes": private_snapshots["transcript"].size,
    }
    if transcript_record != expected_transcript_record:
        raise BootstrapError(
            "bootstrap-private provenance does not bind sanitized transcript"
        )


def _artifact_record(label: str, archive: FileSnapshot) -> dict[str, Any]:
    return {
        "archive_id": f"release-bootstrap.{label.replace('_', '-')}.v1",
        "archive_name": archive.path.name,
        "mode": f"{archive.mode:04o}",
        "sha256": archive.sha256,
        "size_bytes": archive.size,
    }


def _load_release_approval_contract(snapshot: FileSnapshot) -> Any:
    """Load one already-authenticated approval component from captured bytes."""

    module_name = "_sumeragi_v2_release_approval_" + snapshot.sha256
    module = types.ModuleType(module_name)
    module.__file__ = str(snapshot.path)
    module.__package__ = ""
    sys.modules[module_name] = module
    try:
        code = compile(snapshot.data, str(snapshot.path), "exec")
        exec(code, module.__dict__)
    except BaseException as error:
        raise BootstrapError(
            "protected release approval contract could not be loaded"
        ) from error
    finally:
        sys.modules.pop(module_name, None)
    required = {
        "APPROVAL_ARCHIVE_IDS",
        "APPROVAL_CLASS_ORDER",
        "APPROVAL_OPERATION_PLAN_SHA256",
        "APPROVAL_SET_ARCHIVE_FORMAT",
        "ReleaseApprovalClass",
        "ReleaseApprovalError",
        "build_release_approval_expectations",
        "load_protected_release_approval_set",
        "sanitized_release_approval_set_archive",
    }
    if any(not hasattr(module, name) for name in required):
        raise BootstrapError("protected release approval contract API is incomplete")
    if tuple(value.value for value in module.APPROVAL_CLASS_ORDER) != _APPROVAL_CLASS_IDS:
        raise BootstrapError("protected release approval class order is not exact")
    return module


def _approval_duration_values(args: argparse.Namespace) -> dict[str, int]:
    return {
        "offline-toolchain-sdk": args.offline_toolchain_sdk_duration_seconds,
        "formal-proof-tools": args.formal_proof_tools_duration_seconds,
        "network-scale-soak": args.network_scale_soak_duration_seconds,
        "final-bootstrap-publication": (
            args.final_bootstrap_publication_duration_seconds
        ),
    }


def _approval_expectations(
    module: Any,
    *,
    identity: dict[str, Any],
    protected_tool_manifest_sha256: str,
    evidence_root_id: str,
    durations: dict[str, int],
) -> dict[Any, Any]:
    if set(durations) != set(_APPROVAL_CLASS_IDS):
        raise BootstrapError("release approval duration inventory is not exact")
    try:
        return module.build_release_approval_expectations(
            candidate_oid=identity["head_commit"],
            candidate_tree=identity["head_tree"],
            protected_tool_manifest_sha256=protected_tool_manifest_sha256,
            evidence_root_id=evidence_root_id,
            offline_toolchain_sdk_duration_seconds=durations[
                "offline-toolchain-sdk"
            ],
            formal_proof_tools_duration_seconds=durations[
                "formal-proof-tools"
            ],
            network_scale_soak_duration_seconds=durations[
                "network-scale-soak"
            ],
            final_bootstrap_publication_duration_seconds=durations[
                "final-bootstrap-publication"
            ],
        )
    except module.ReleaseApprovalError as error:
        raise BootstrapError(f"release approval expectation failed: {error}") from error


def _load_bound_release_approvals(
    module: Any,
    paths: dict[str, Path],
    expectations: dict[Any, Any],
) -> tuple[Any, ...]:
    if set(paths) != set(_APPROVAL_CLASS_IDS):
        raise BootstrapError("release approval path inventory is not exact")
    typed_paths = {
        module.ReleaseApprovalClass(class_id): paths[class_id]
        for class_id in _APPROVAL_CLASS_IDS
    }
    try:
        return module.load_protected_release_approval_set(
            typed_paths,
            expectations=expectations,
            expected_owner_uid=os.getuid(),
        )
    except module.ReleaseApprovalError as error:
        raise BootstrapError(f"protected release approval rejected: {error}") from error


def _approval_archive_record(
    snapshot: FileSnapshot,
    *,
    archive_id: str,
    archive_name: str,
) -> dict[str, Any]:
    if snapshot.path.name != archive_name or snapshot.mode != _DATA_MODE:
        raise BootstrapError("release approval archive metadata is not exact")
    return {
        "archive_id": archive_id,
        "archive_name": archive_name,
        "mode": "0400",
        "sha256": snapshot.sha256,
        "size_bytes": snapshot.size,
    }


def _replay_release_approval_evidence(
    *,
    module: Any,
    approval_paths: dict[str, Path],
    identity: dict[str, Any],
    protected_tool_manifest_sha256: str,
    evidence_root_id: str,
    durations: dict[str, int],
    attestation_snapshots: dict[str, FileSnapshot],
    set_attestation_snapshot: FileSnapshot,
    marker_record: dict[str, Any],
) -> tuple[Any, ...]:
    """Independently replay the four raw approvals and sanitized projections."""

    expectations = _approval_expectations(
        module,
        identity=identity,
        protected_tool_manifest_sha256=protected_tool_manifest_sha256,
        evidence_root_id=evidence_root_id,
        durations=durations,
    )
    approvals = _load_bound_release_approvals(module, approval_paths, expectations)
    expected_plan_digests = {
        approval_class.value: digest
        for approval_class, digest in module.APPROVAL_OPERATION_PLAN_SHA256.items()
    }
    expected_attestations: dict[str, dict[str, Any]] = {}
    for approval in approvals:
        class_id = approval.class_id.value
        sanitized = approval.sanitized_archive()
        snapshot = attestation_snapshots[class_id]
        if snapshot.data != sanitized.canonical_bytes or snapshot.sha256 != sanitized.sha256:
            raise BootstrapError(
                f"sanitized release approval {class_id} changed"
            )
        expected_attestations[class_id] = _approval_archive_record(
            snapshot,
            archive_id=module.APPROVAL_ARCHIVE_IDS[approval.class_id],
            archive_name=_APPROVAL_ATTESTATION_NAMES[class_id],
        )
    set_archive = module.sanitized_release_approval_set_archive(approvals)
    if (
        set_attestation_snapshot.data != set_archive.canonical_bytes
        or set_attestation_snapshot.sha256 != set_archive.sha256
    ):
        raise BootstrapError("sanitized release approval set changed")
    expected_marker = {
        "format": module.APPROVAL_SET_ARCHIVE_FORMAT,
        "schema_version": 1,
        "candidate_oid": identity["head_commit"],
        "candidate_tree": identity["head_tree"],
        "protected_tool_manifest_sha256": protected_tool_manifest_sha256,
        "evidence_root_id": evidence_root_id,
        "expected_duration_seconds": durations,
        "operation_plan_sha256": expected_plan_digests,
        "class_attestations": expected_attestations,
        "set_attestation": _approval_archive_record(
            set_attestation_snapshot,
            archive_id=_APPROVAL_SET_ARCHIVE_ID,
            archive_name=_APPROVAL_SET_ATTESTATION_NAME,
        ),
    }
    if marker_record != expected_marker:
        raise BootstrapError("release approval marker binding is not exact")
    return approvals
