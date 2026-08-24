# Executed lexically in write_sumeragi_v2_release_receipt.py after exact digest authentication.


def _validate_framework_python_input_records(
    records: Any, declared_count: Any, declared_bytes: Any,
) -> list[dict[str, Any]]:
    """Validate one bounded, canonical private source inventory."""

    if not isinstance(records, list) or len(records) > _MAX_FRAMEWORK_RUNTIME_MEMBERS:
        raise ReceiptError("framework Python source inventory is not bounded")
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
            raise ReceiptError("framework Python source record is not exact")
        path = record["path"]
        if (
            not isinstance(path, str) or not path or path.startswith("/")
            or PurePosixPath(path).as_posix() != path
            or ".." in PurePosixPath(path).parts
        ):
            raise ReceiptError("framework Python source path is unsafe")
        paths.append(path)
        for key in set(record) & {
            "source_device", "source_inode", "destination_device",
            "destination_inode", "size",
        }:
            if type(record[key]) is not int or record[key] < 0:
                raise ReceiptError("framework Python source integer is invalid")
        for key in set(record) & {"source_mode", "destination_mode"}:
            if not isinstance(record[key], str) or re.fullmatch(
                r"[0-7]{4}", record[key]
            ) is None:
                raise ReceiptError("framework Python source mode is invalid")
            if kind != "symlink" and int(record[key], 8) & 0o022:
                raise ReceiptError("framework Python source mode is unsafe")
        if kind == "file":
            if not isinstance(record["sha256"], str) or _DIGEST_RE.fullmatch(
                record["sha256"]
            ) is None:
                raise ReceiptError("framework Python source digest is invalid")
            file_bytes += record["size"]
        elif kind == "symlink" and (
            not isinstance(record["target"], str) or not record["target"]
        ):
            raise ReceiptError("framework Python source symlink is invalid")
    if (
        paths != sorted(paths) or len(set(paths)) != len(paths)
        or type(declared_count) is not int or declared_count != len(records)
        or type(declared_bytes) is not int or declared_bytes != file_bytes
        or file_bytes > _MAX_FRAMEWORK_RUNTIME_BYTES
    ):
        raise ReceiptError("framework Python source inventory is not exact")
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
        raise ReceiptError("framework Python Mach-O closure is malformed")
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
        raise ReceiptError("framework Python Mach-O closure is not exact")

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
            raise ReceiptError("framework Python Mach-O image is malformed")
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
            raise ReceiptError("framework Python Mach-O image is not bound")
        for slice_value in image["slices"]:
            if not isinstance(slice_value, dict) or set(slice_value) != {
                "cpu_type", "cpu_subtype", "file_type", "dependencies",
                "id_dylib_sha256", "rpath_sha256", "code_signature",
            }:
                raise ReceiptError("framework Python Mach-O slice is malformed")
            if (
                slice_value["code_signature"] != "embedded"
                or any(
                    type(slice_value[field]) is not int
                    or slice_value[field] < 0
                    for field in ("cpu_type", "cpu_subtype", "file_type")
                )
                or not isinstance(slice_value["dependencies"], list)
            ):
                raise ReceiptError("framework Python Mach-O slice is not exact")
            for field in ("id_dylib_sha256", "rpath_sha256"):
                if not isinstance(slice_value[field], list) or any(
                    not isinstance(digest, str)
                    or _DIGEST_RE.fullmatch(digest) is None
                    for digest in slice_value[field]
                ):
                    raise ReceiptError("framework Python Mach-O digest is malformed")
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
                    raise ReceiptError("framework Python dependency is malformed")
        images.append(image["path"])
    if images != sorted(images) or len(images) != len(set(images)):
        raise ReceiptError("framework Python Mach-O image order is not exact")
    image_set = set(images)
    for image in runtime["images"]:
        for slice_value in image["slices"]:
            if any(
                dependency["binding"] == "archive"
                and dependency["target"] not in image_set
                for dependency in slice_value["dependencies"]
            ):
                raise ReceiptError("framework Python dependency target is absent")

    external: dict[str, dict[str, Any]] = {}
    external_paths: list[str] = []
    for record in value["external_sources"]:
        if not isinstance(record, dict) or set(record) != {
            "input_path", "path", "mode", "sha256", "size_bytes",
        }:
            raise ReceiptError("external framework dependency is malformed")
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
            raise ReceiptError("external framework dependency is unsafe")
        external[record["path"]] = record
        external_paths.append(record["path"])

    transform_paths: list[str] = []
    for transform in value["transforms"]:
        if not isinstance(transform, dict) or set(transform) != {
            "input_path", "path", "source_mode", "source_sha256",
            "source_size_bytes", "derived_mode", "derived_sha256",
            "derived_size_bytes", "operations", "codesign",
        }:
            raise ReceiptError("framework Python Mach-O transform is malformed")
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
            raise ReceiptError("framework Python Mach-O transform is not bound")
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
                raise ReceiptError("framework Python Mach-O operation is malformed")
        bound = external.get(transform["path"])
        if bound is not None and (
            transform["input_path"], transform["source_mode"],
            transform["source_sha256"], transform["source_size_bytes"],
        ) != (
            bound["input_path"], bound["mode"], bound["sha256"],
            bound["size_bytes"],
        ):
            raise ReceiptError("external framework dependency binding changed")
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
        raise ReceiptError("framework Python Mach-O closure is incomplete")


def _validate_framework_python_relocation_evidence(
    public: Any,
    private: Any,
    input_records: Any,
    runtime_records: list[dict[str, Any]],
    input_record_count: Any,
    input_file_bytes: Any,
) -> None:
    """Bind authenticated Mach-O sources/tools to the two derived members."""

    if public != private or not isinstance(public, dict) or set(public) != {
        "format", "schema_version", "framework", "tools", "artifacts",
        "closure",
    }:
        raise ReceiptError("framework Python relocation evidence is malformed")
    framework = public["framework"]
    if (
        public["format"] != "iroha-sumeragi-v2-framework-python-relocation"
        or type(public["schema_version"]) is not int
        or public["schema_version"] != 2
        or not isinstance(framework, str)
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._+-]*", framework) is None
        or not isinstance(public["tools"], dict)
        or set(public["tools"])
        != {"codesign", "install_name_tool", "otool"}
        or not isinstance(public["artifacts"], dict)
        or set(public["artifacts"]) != {"launcher", "trampoline"}
    ):
        raise ReceiptError("framework Python relocation evidence is not exact")
    input_records = _validate_framework_python_input_records(
        input_records, input_record_count, input_file_bytes,
    )
    for name in ("codesign", "install_name_tool", "otool"):
        tool = public["tools"][name]
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
            raise ReceiptError("framework Python relocation tool binding is wrong")
    source_by_path = {
        record.get("path"): record
        for record in input_records
        if isinstance(record, dict)
    }
    derived_by_path = {record["path"]: record for record in runtime_records}
    _validate_framework_python_macho_closure(
        public["closure"], source_by_path, derived_by_path, framework,
    )
    transform_by_path = {
        transform["path"]: transform
        for transform in public["closure"]["transforms"]
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
    for name, (source_path, derived_path, dependency) in rewrites.items():
        artifact = public["artifacts"][name]
        source = artifact.get("source") if isinstance(artifact, dict) else None
        derived = artifact.get("derived") if isinstance(artifact, dict) else None
        source_record = source_by_path.get(source_path)
        derived_record = derived_by_path.get(derived_path)
        transform = transform_by_path.get(derived_path)
        if (
            not isinstance(artifact, dict)
            or set(artifact) != {"path", "source", "derived"}
            or artifact["path"] != derived_path
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
            or derived["framework_dependency"] != dependency
            or derived["codesign"] != "adhoc"
            or not isinstance(source_record, dict)
            or source_record.get("kind") != "file"
            or (
                source_record.get("source_mode"), source_record.get("sha256"),
                source_record.get("size"),
            )
            != (source["mode"], source["sha256"], source["size_bytes"])
            or not isinstance(derived_record, dict)
            or derived_record.get("kind") != "file"
            or (
                derived_record.get("mode"), derived_record.get("sha256"),
                derived_record.get("size"),
            )
            != (derived["mode"], derived["sha256"], derived["size_bytes"])
            or not isinstance(transform, dict)
            or transform["input_path"] != source_path
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
            raise ReceiptError(
                "framework Python relocation artifact binding is wrong"
            )
        digests = (
            source["sha256"], source["framework_dependency_sha256"],
            source["dependency_vector_sha256"], derived["sha256"],
            derived["dependency_vector_sha256"],
        )
        if any(
            not isinstance(digest, str) or _DIGEST_RE.fullmatch(digest) is None
            for digest in digests
        ):
            raise ReceiptError("framework Python relocation digest binding is wrong")
        operation_replacements = [
            operation["replacement"]
            for operation in transform["operations"]
        ]
        already_local = source["framework_dependency_sha256"] == hashlib.sha256(
            dependency.encode("utf-8", "strict")
        ).hexdigest()
        if (
            operation_replacements not in ([], [dependency])
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
            raise ReceiptError("framework Python relocation derivation is wrong")
    if derived_by_path.get(framework, {}).get("kind") != "file":
        raise ReceiptError("framework Python relocation framework is absent")


def _require_pruned_build_roots(release_root: Path) -> None:
    """Require every bootstrap-private disposable build root to be absent."""

    invocation = release_root.parent
    output = invocation / "output"
    for path, label in (
        *((invocation / name, f"release {name} root") for name in (
            "runtime", "sdk-inputs", "sdk-work", "target",
        )),
        *((output / name, f"release {name} root") for name in (
            "home", "tmp", "cache", "cargo-home",
        )),
    ):
        _require_pruned_private_root(path, label)

def build_receipt(
    *,
    candidate_identity_path: Path,
    sealed_identity_path: Path,
    release_root_path: Path,
    signature_attestation_path: Path,
    signature_transcript_path: Path,
    signature_raw_commit_path: Path,
    signature_cargo_lock_path: Path,
    signature_allowed_signers_path: Path,
    signature_revocation_path: Path,
    signature_git_path: Path,
    signature_ssh_keygen_path: Path,
    expected_git_sha256: str,
    expected_ssh_keygen_sha256: str,
    expected_allowed_signers_sha256: str,
    expected_revocation_sha256: str,
    expected_signer_fingerprint: str,
    bootstrap_completion_path: Path,
    bootstrap_evidence_dir_path: Path,
    bootstrap_identity_path: Path,
    bootstrap_attestation_path: Path,
    bootstrap_transcript_path: Path,
    expected_bootstrap_completion_sha256: str,
    bootstrap_candidate_root_path: Path,
    bootstrap_runner_path: Path,
    corridor_completion_path: Path,
    formal_completion_path: Path,
    formal_replay_source_receipt_path: Path,
    formal_replay_release_root_path: Path,
    expected_formal_replay_signature_sha256: str,
    formal_replay_principal: str,
    seed_completion_path: Path,
    chaos_completion_path: Path,
    g4p_completion_path: Path,
    g12_seed_completion_path: Path,
    g12_fault_soak_completion_path: Path,
    scaling_evidence_manifest_path: Path,
    sdk_dependency_archive_path: Path,
    sdk_dependency_input_inventory_path: Path,
    sdk_dependency_final_work_inventory_path: Path,
    runtime_tool_probe_manifest_path: Path,
    runtime_tool_probe_result_path: Path,
    runtime_tool_probe_runtime_available: bool,
    private_build_roots_available: bool,
    bootstrap_private_inputs_available: bool,
    expected_scaling_trial_harness_sha256: str,
    expected_scaling_configuration_sha256: str,
    expected_scaling_irohad_sha256: str,
    expected_scaling_iroha_cli_sha256: str,
    repository_root_path: Path,
    runner_logs_sealed: bool = False,
) -> tuple[
    dict[str, Any],
    PathContract,
    PathContract,
    list[PathContract | DirectoryContract],
]:
    """Validate every completion artifact and return one aggregate receipt."""

    if not private_build_roots_available:
        _require_pruned_build_roots(release_root_path)

    repo_root = repository_root_path.resolve(strict=True)
    if (
        not repository_root_path.is_absolute()
        or Path(os.path.abspath(repository_root_path)) != repository_root_path
        or repo_root != repository_root_path
        or repo_root != release_root_path
        or repo_root.is_symlink()
        or not repo_root.is_dir()
    ):
        raise ReceiptError(
            "repository root must be the exact retained sealed release root"
        )
    candidate_snapshot, candidate = _load_identity(
        candidate_identity_path, "candidate identity"
    )
    sealed_snapshot, sealed = _load_identity(
        sealed_identity_path, "sealed identity"
    )
    for field in ("head_commit", "head_tree", "index_tree", "cargo_lock_sha256"):
        if candidate[field] != sealed[field]:
            raise ReceiptError(f"candidate and sealed identity disagree on {field}")
    if sealed["head_tree"] != sealed["index_tree"]:
        raise ReceiptError("sealed release index tree is not HEAD")
    # All code was compiled and all child evidence was produced after sealing,
    # so child completions bind the sealed permission-aware manifest. The
    # candidate manifest remains independently recorded in the final receipt.
    manifest = sealed["workspace_source_manifest_sha256"]
    expected_scaling_trial_harness_sha256 = _require_digest(
        expected_scaling_trial_harness_sha256,
        "expected scaling trial harness digest",
    )
    expected_scaling_configuration_sha256 = _require_digest(
        expected_scaling_configuration_sha256,
        "expected scaling configuration digest",
    )
    expected_scaling_irohad_sha256 = _require_digest(
        expected_scaling_irohad_sha256,
        "expected scaling irohad digest",
    )
    expected_scaling_iroha_cli_sha256 = _require_digest(
        expected_scaling_iroha_cli_sha256,
        "expected scaling iroha CLI digest",
    )

    release_authentication, signature_archives = _validate_signature_evidence(
        candidate_snapshot=candidate_snapshot,
        candidate=candidate,
        release_root_path=release_root_path,
        signature_attestation_path=signature_attestation_path,
        signature_transcript_path=signature_transcript_path,
        signature_raw_commit_path=signature_raw_commit_path,
        signature_cargo_lock_path=signature_cargo_lock_path,
        signature_allowed_signers_path=signature_allowed_signers_path,
        signature_revocation_path=signature_revocation_path,
        signature_git_path=signature_git_path,
        signature_ssh_keygen_path=signature_ssh_keygen_path,
        expected_git_sha256=expected_git_sha256,
        expected_ssh_keygen_sha256=expected_ssh_keygen_sha256,
        expected_allowed_signers_sha256=expected_allowed_signers_sha256,
        expected_revocation_sha256=expected_revocation_sha256,
        expected_signer_fingerprint=expected_signer_fingerprint,
    )
    (
        bootstrap_authentication,
        bootstrap_evidence,
        bootstrap_runtime_contracts,
    ) = _validate_bootstrap_evidence(
        completion_path=bootstrap_completion_path,
        evidence_dir_path=bootstrap_evidence_dir_path,
        identity_path=bootstrap_identity_path,
        attestation_path=bootstrap_attestation_path,
        transcript_path=bootstrap_transcript_path,
        expected_completion_sha256=expected_bootstrap_completion_sha256,
        candidate_root_path=bootstrap_candidate_root_path,
        runner_path=bootstrap_runner_path,
        release_root_path=release_root_path,
        candidate=candidate,
        candidate_snapshot=candidate_snapshot,
        sealed=sealed,
        expected_signer_fingerprint=expected_signer_fingerprint,
        signature_archives=signature_archives,
        runner_logs_sealed=runner_logs_sealed,
        expected_scaling_manifest_path=scaling_evidence_manifest_path,
        expected_scaling_trial_harness_sha256=(
            expected_scaling_trial_harness_sha256
        ),
        expected_scaling_configuration_sha256=(
            expected_scaling_configuration_sha256
        ),
        expected_scaling_irohad_sha256=expected_scaling_irohad_sha256,
        expected_scaling_iroha_cli_sha256=expected_scaling_iroha_cli_sha256,
        expected_formal_replay_source_receipt_path=(
            formal_replay_source_receipt_path
        ),
        expected_formal_replay_release_root_path=(
            formal_replay_release_root_path
        ),
        expected_formal_replay_signature_sha256=(
            expected_formal_replay_signature_sha256
        ),
        expected_formal_replay_principal=formal_replay_principal,
        bootstrap_private_inputs_available=bootstrap_private_inputs_available,
    )
    sdk_dependencies = _validate_sdk_dependency_evidence(
        archive_path=sdk_dependency_archive_path,
        input_inventory_path=sdk_dependency_input_inventory_path,
        final_work_inventory_path=sdk_dependency_final_work_inventory_path,
        release_root=repo_root,
        source_manifest_path=(
            bootstrap_evidence_dir_path
            / "sdk-dependency-bundle-manifest.json"
        ),
        source_manifest_sha256=bootstrap_authentication[
            "trusted_input_digests"
        ]["sdk_dependency_bundle_manifest"],
        expected_git_sha256=bootstrap_authentication[
            "trusted_input_digests"
        ]["git"],
        bootstrap_private_inputs_available=bootstrap_private_inputs_available,
    )
    runtime_tool_probes, runtime_tool_probe_contracts = (
        _runtime_tool_probe_evidence(
            manifest_path=runtime_tool_probe_manifest_path,
            result_path=runtime_tool_probe_result_path,
            release_root=repo_root,
            bootstrap_authentication=bootstrap_authentication,
            bootstrap_evidence=bootstrap_evidence,
            bootstrap_evidence_root=bootstrap_evidence_dir_path,
            runtime_available=runtime_tool_probe_runtime_available,
        )
    )
    bootstrap_runtime_contracts.extend(runtime_tool_probe_contracts)
    checker_environment = _closed_replay_environment(
        Path(signature_archives["git"]["path"]).parent
    )
    checker_environment.update(
        {
            "PATH": str(Path(signature_archives["git"]["path"]).parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONHASHSEED": "0",
        }
    )

    (
        scaling_bundle,
        retained_scaling_validator,
        scaling_trust_anchors,
    ) = _validate_scaling_evidence(
        manifest_path=scaling_evidence_manifest_path,
        sealed=sealed,
        repo_root=repo_root,
        checker_environment=checker_environment,
        expected_trial_harness_sha256=(
            expected_scaling_trial_harness_sha256
        ),
        expected_configuration_sha256=(
            expected_scaling_configuration_sha256
        ),
        expected_irohad_sha256=expected_scaling_irohad_sha256,
        expected_iroha_cli_sha256=expected_scaling_iroha_cli_sha256,
    )
    corridor_path, corridor_completion = _load_tsv(
        corridor_completion_path, "corridor completion"
    )
    (
        corridor_summary,
        corridor_required,
        corridor_g_unit_inventory,
        corridor_logs,
        prebuilt_binary_bundle,
        cargo_cache_input,
    ) = _corridor_artifacts(
        corridor_path,
        corridor_completion,
        sealed,
        repo_root,
        bootstrap_authentication["runner"]["tools"],
        bootstrap_authentication["trusted_input_digests"],
        expected_artifact_root=(
            release_root_path.parent / "output"
        ),
        expected_cargo_target_root=(
            release_root_path.parent / "target"
        ),
        private_build_roots_available=private_build_roots_available,
    )
    prebuilt_manifest_sha256 = prebuilt_binary_bundle["manifest"]["sha256"]
    prebuilt_invocation_id = prebuilt_binary_bundle["archive_id"].partition(":")[2]
    prebuilt_artifact_root = release_root_path.parent / "output"
    prebuilt_cargo_target_root = release_root_path.parent / "target"
    prebuilt_bundle_dir = (
        prebuilt_artifact_root
        / "sumeragi-v2-release"
        / sealed["workspace_source_manifest_sha256"]
        / "programs"
        / prebuilt_invocation_id
    )
    g4p_evidence = _validate_g4p_evidence(
        completion_path=g4p_completion_path,
        sealed=sealed,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )
    g12_evidence = _validate_g12_evidence(
        seed_completion_path=g12_seed_completion_path,
        fault_soak_completion_path=g12_fault_soak_completion_path,
        sealed=sealed,
        prebuilt_manifest_sha256=prebuilt_manifest_sha256,
    )

    formal_path, formal_completion = _load_tsv(
        formal_completion_path, "formal completion"
    )
    (
        formal_log,
        formal_ledger,
        formal_evidence,
        formal_verus_evidence,
        formal_verus_log,
        formal_multilane_apalache_evidence,
        formal_cross_tool_evidence,
        formal_production_trace_extraction_evidence,
        formal_harness_lock,
        formal_toolchain,
        formal_tlaps_resource_jsonl,
        formal_tlaps_resource_summary,
    ) = _formal_artifacts(
        formal_path,
        formal_completion,
        sealed,
        checker_environment,
        repo_root,
        bootstrap_authentication["runner"]["tools"],
        corridor_completion,
        private_build_roots_available,
    )
    formal_replay_release = _formal_replay_release(
        source_receipt_path=formal_replay_source_receipt_path,
        release_root_path=formal_replay_release_root_path,
        expected_signature_sha256=expected_formal_replay_signature_sha256,
        expected_ssh_keygen_sha256=expected_ssh_keygen_sha256,
        expected_allowed_signers_sha256=expected_allowed_signers_sha256,
        expected_revocation_sha256=expected_revocation_sha256,
        principal=formal_replay_principal,
        expected_signer_fingerprint=expected_signer_fingerprint,
        checker_environment=checker_environment,
        repo_root=repo_root,
    )
    seed_path, seed = _load_tsv(seed_completion_path, "seed completion")
    seed_manifest_fields = {
        "localnet_manifest_count",
        "localnet_manifests_path",
        "localnet_manifests_sha256",
    }
    for index in range(_SEED_RUN_COUNT):
        seed_manifest_fields.add(f"localnet_manifest_{index:03d}_path")
        seed_manifest_fields.add(f"localnet_manifest_{index:03d}_sha256")
    _require_fields(
        seed,
        {
            "schema_version",
            "profile",
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "prebuilt_manifest_sha256",
            "completed_runs",
            "expected_runs",
            "summary_sha256",
        }
        | seed_manifest_fields,
        "seed completion",
    )
    if (
        seed["schema_version"] != "2"
        or seed["profile"] != "release"
        or seed["head_commit"] != sealed["head_commit"]
        or seed["head_tree"] != sealed["head_tree"]
        or seed["source_manifest_sha256"] != manifest
        or seed["cargo_lock_sha256"] != sealed["cargo_lock_sha256"]
        or seed["prebuilt_manifest_sha256"] != prebuilt_manifest_sha256
        or seed["completed_runs"] != str(_SEED_RUN_COUNT)
        or seed["expected_runs"] != str(_SEED_RUN_COUNT)
    ):
        raise ReceiptError("seed completion does not describe the exact release matrix")
    seed_summary = _bounded_evidence_snapshot(
        seed_path.path.with_name("summary.tsv"),
        "seed summary",
        maximum_bytes=_MAX_RELEASE_TSV_BYTES,
    )
    if seed_summary.sha256 != seed["summary_sha256"]:
        raise ReceiptError("seed completion summary digest mismatch")
    seed_run_logs = _seed_run_logs(
        seed_path,
        seed_summary,
        manifest,
        prebuilt_cargo_target_root,
        prebuilt_bundle_dir,
        prebuilt_manifest_sha256,
    )
    seed_localnet_manifest_index, seed_localnet_manifests = (
        _seed_localnet_manifests(seed_path, seed)
    )
    seed_summary_contract = _snapshot_contract(seed_summary)
    del seed_summary

    chaos_path, chaos = _load_tsv(chaos_completion_path, "chaos completion")
    _require_fields(
        chaos,
        {
            "head_commit",
            "head_tree",
            "source_manifest_sha256",
            "cargo_lock_sha256",
            "log_sha256",
        }
        | set(_CHAOS_FIXED_FIELDS),
        "chaos completion",
    )
    expected_chaos = {
        **_CHAOS_FIXED_FIELDS,
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": manifest,
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
    }
    if any(chaos.get(field) != value for field, value in expected_chaos.items()):
        raise ReceiptError(
            "chaos completion does not match the exact release identity and reducer schedule"
        )
    chaos_log = _bounded_evidence_snapshot(
        chaos_path.path.with_name("chaos-100k.log"),
        "chaos log",
        maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
    )
    if chaos_log.sha256 != chaos["log_sha256"]:
        raise ReceiptError("chaos completion log digest mismatch")
    chaos_lines = _decode_lf_text(chaos_log, "chaos log").splitlines()
    chaos_results = [line for line in chaos_lines if line.startswith("test result:")]
    chaos_test_prefix = (
        "test accelerated_100_000_block_chaos_preserves_chain_prefix ... "
    )
    chaos_completion_line = chaos_test_prefix + "ok"
    if (
        chaos_lines.count("running 1 test") != 1
        or len(chaos_results) != 1
        or not re.fullmatch(
            r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
            r"11 filtered out; finished in .+",
            chaos_results[0],
        )
        or sum(chaos_test_prefix in line for line in chaos_lines) != 1
        or chaos_lines.count(chaos_completion_line) != 1
        or chaos_lines.count(_CHAOS_MARKER) != 1
    ):
        raise ReceiptError(
            "chaos log does not prove its one exact passing release test"
        )
    chaos_log_contract = _snapshot_contract(chaos_log)
    del chaos_log

    receipt = {
        "schema_version": 1,
        "protocol": "sumeragi-v2",
        "result": "release-complete",
        "identity": {
            "head_commit": sealed["head_commit"],
            "head_tree": sealed["head_tree"],
            "index_tree": sealed["index_tree"],
            "cargo_lock_sha256": sealed["cargo_lock_sha256"],
            "candidate_source_manifest_sha256": candidate[
                "workspace_source_manifest_sha256"
            ],
            "sealed_source_manifest_sha256": manifest,
        },
        "authentication": {
            "schema_version": 2,
            "bootstrap": bootstrap_authentication,
            "release_identity": release_authentication,
        },
        "evidence": {
            "bootstrap": bootstrap_evidence,
            "release_signature_attestation": signature_archives["attestation"],
            "release_signature_transcript": signature_archives[
                "verify_transcript"
            ],
            "release_signature_raw_commit": signature_archives["raw_commit"],
            "release_signature_cargo_lock": signature_archives["cargo_lock"],
            "release_signature_allowed_signers": signature_archives[
                "ssh_allowed_signers"
            ],
            "release_signature_revocation": signature_archives["ssh_revocation"],
            "release_signature_git": signature_archives["git"],
            "release_signature_ssh_keygen": signature_archives["ssh_keygen"],
            "corridor_completion": _artifact(corridor_path),
            "corridor_summary": _artifact(corridor_summary),
            "corridor_production_inventory": _artifact(corridor_required),
            "g_unit_focused_test_inventory": _artifact(
                corridor_g_unit_inventory
            ),
            "corridor_logs": [_artifact(path) for path in corridor_logs],
            "cargo_cache_input": cargo_cache_input, "cargo_cache_input_inventory": cargo_cache_input["inventory"],
            "cargo_cache_final_inventory": cargo_cache_input["final_inventory"],
            "sdk_dependencies": sdk_dependencies,
            "runtime_tool_probes": runtime_tool_probes,
            "prebuilt_binary_bundle": prebuilt_binary_bundle,
            "formal_completion": _artifact(formal_path),
            "formal_gate_log": _artifact(formal_log),
            "formal_proof_coverage": _artifact(formal_ledger),
            "formal_proof_evidence": _artifact(formal_evidence),
            "formal_verus_evidence": _artifact(formal_verus_evidence),
            "formal_verus_log": _artifact(formal_verus_log),
            "formal_multilane_apalache_evidence": _artifact(
                formal_multilane_apalache_evidence
            ),
            "formal_cross_tool_evidence": _artifact(formal_cross_tool_evidence),
            "formal_production_trace_extraction_evidence": _artifact(
                formal_production_trace_extraction_evidence
            ),
            "formal_harness_lock": _artifact(formal_harness_lock),
            "formal_toolchain": _artifact(formal_toolchain),
            "formal_tlaps_resource_jsonl": _artifact(formal_tlaps_resource_jsonl),
            "formal_tlaps_resource_summary": _artifact(formal_tlaps_resource_summary),
            "formal_replay_release": formal_replay_release,
            "seed_matrix_completion": _artifact(seed_path),
            "seed_matrix_summary": _artifact(seed_summary_contract),
            "seed_matrix_run_logs": [_artifact(path) for path in seed_run_logs],
            "seed_matrix_localnet_manifest_index": _artifact(
                seed_localnet_manifest_index
            ),
            "seed_matrix_localnet_manifests": [
                _artifact(path) for path in seed_localnet_manifests
            ],
            "chaos_completion": _artifact(chaos_path),
            "chaos_log": _artifact(chaos_log_contract),
            "multilane_scaling_bundle": scaling_bundle,
            "multilane_scaling_retained_validator": retained_scaling_validator,
            "multilane_scaling_trust_anchors": scaling_trust_anchors,
            "g4p_multilane": g4p_evidence,
            "g12_cross_dataspace": g12_evidence,
        },
    }
    return (
        receipt,
        _snapshot_contract(candidate_snapshot),
        _snapshot_contract(sealed_snapshot),
        bootstrap_runtime_contracts,
    )


def _iter_artifact_records(value: Any) -> Any:
    if isinstance(value, dict):
        if (
            "path" in value
            and "sha256" in value
            and isinstance(value["path"], str)
            and isinstance(value["sha256"], str)
        ):
            yield value
        for child in value.values():
            yield from _iter_artifact_records(child)
    elif isinstance(value, list):
        for child in value:
            yield from _iter_artifact_records(child)


def _capture_path_contract(
    path: Path,
    name: str,
    *,
    expected_sha256: str | None,
    expected_mode: int | None = None,
    expected_owner: int | None = None,
    expected_nlink: int | None = None,
    expected_size: int | None = None,
) -> PathContract:
    if expected_sha256 is not None:
        _require_digest(expected_sha256, f"{name} digest")
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise ReceiptError(f"{name} must be a resolved regular non-symlink file")
    if expected_mode is not None and stat.S_IMODE(before.st_mode) != expected_mode:
        raise ReceiptError(f"{name} mode changed before receipt publication")
    if expected_owner is not None and before.st_uid != expected_owner:
        raise ReceiptError(f"{name} owner changed before receipt publication")
    if expected_nlink is not None and before.st_nlink != expected_nlink:
        raise ReceiptError(f"{name} link count changed before receipt publication")
    if expected_size is not None and before.st_size != expected_size:
        raise ReceiptError(f"{name} size changed before receipt publication")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISREG(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
            or opened.st_mode != before.st_mode
            or opened.st_uid != before.st_uid
            or opened.st_nlink != before.st_nlink
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        digest = hashlib.sha256()
        size = 0
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            size += len(chunk)
            if size > 4 * 1024 * 1024 * 1024:
                raise ReceiptError(f"{name} exceeds the aggregate evidence size limit")
            digest.update(chunk)
        after = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
            "st_mode",
            "st_uid",
            "st_nlink",
        )
        if any(getattr(after, field) != getattr(opened, field) for field in fields):
            raise ReceiptError(f"{name} changed while it was hashed")
        observed_sha = digest.hexdigest()
        if (
            (expected_sha256 is not None and observed_sha != expected_sha256)
            or size != opened.st_size
        ):
            raise ReceiptError(f"{name} digest changed before receipt publication")
        return PathContract(
            path=path,
            sha256=observed_sha,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            size=opened.st_size,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _snapshot_receipt_inputs(
    receipt: dict[str, Any],
    *,
    candidate_identity: PathContract,
    sealed_identity: PathContract,
    scaling_root: Path,
    bootstrap_evidence_root: Path,
    candidate_root: Path,
    release_root: Path,
    bootstrap_runtime_contracts: list[PathContract | DirectoryContract],
    private_build_roots_available: bool = True,
) -> list[PathContract | DirectoryContract]:
    records = list(_iter_artifact_records(receipt["authentication"])) + list(
        _iter_artifact_records(receipt["evidence"])
    )
    for name, expected in (
        ("candidate identity", candidate_identity),
        ("sealed identity", sealed_identity),
    ):
        contract = _capture_path_contract(
            expected.path,
            name,
            expected_sha256=expected.sha256,
            expected_mode=expected.mode,
            expected_owner=expected.owner,
            expected_nlink=expected.nlink,
            expected_size=expected.size,
        )
        if contract != expected:
            raise ReceiptError(f"{name} changed after semantic validation")
        records.append(_path_contract_artifact(contract))
    by_path: dict[Path, dict[str, Any]] = {}
    for record in records:
        path = Path(record["path"])
        previous = by_path.get(path)
        if previous is not None:
            comparable = {key: value for key, value in record.items() if key != "path"}
            previous_comparable = {
                key: value for key, value in previous.items() if key != "path"
            }
            common = set(comparable) & set(previous_comparable)
            if any(comparable[key] != previous_comparable[key] for key in common):
                raise ReceiptError("aggregate receipt contains conflicting artifact aliases")
            continue
        by_path[path] = record

    scaling_bundle = receipt["evidence"].get("multilane_scaling_bundle")
    if not isinstance(scaling_bundle, dict):
        raise ReceiptError("aggregate receipt lacks its scaling bundle inventory")
    scaling_files_raw = scaling_bundle.get("files")
    scaling_directories_raw = scaling_bundle.get("directories")
    if (
        scaling_bundle.get("archive_id") != "release-scaling.bundle.v1"
        or not isinstance(scaling_files_raw, list)
        or not isinstance(scaling_directories_raw, list)
    ):
        raise ReceiptError("aggregate receipt scaling bundle inventory is malformed")
    expected_scaling_files: list[str] = []
    expected_scaling_size = 0
    for index, record in enumerate(scaling_files_raw):
        if not isinstance(record, dict):
            raise ReceiptError("aggregate receipt scaling file record is malformed")
        relative = record.get("relative_path")
        size = record.get("size_bytes")
        if (
            not isinstance(relative, str)
            or type(size) is not int
            or record.get("archive_id") != "release-scaling.file.v1:" + relative
        ):
            raise ReceiptError(
                f"aggregate receipt scaling file {index} path is malformed"
            )
        expected_scaling_files.append(relative)
        expected_scaling_size += size
    if expected_scaling_files != sorted(expected_scaling_files) or len(
        expected_scaling_files
    ) != len(set(expected_scaling_files)):
        raise ReceiptError(
            "aggregate receipt scaling files are not one deterministic inventory"
        )
    if (
        scaling_bundle.get("file_count") != len(expected_scaling_files)
        or scaling_bundle.get("total_size_bytes") != expected_scaling_size
        or any(not isinstance(item, str) for item in scaling_directories_raw)
        or scaling_directories_raw != sorted(scaling_directories_raw)
    ):
        raise ReceiptError("aggregate receipt scaling bundle accounting is inconsistent")
    current_scaling, current_directories, current_scaling_size = (
        _scan_scaling_bundle(scaling_root)
    )
    if (
        [item[0] for item in current_scaling] != expected_scaling_files
        or current_directories != scaling_directories_raw
        or current_scaling_size != expected_scaling_size
    ):
        raise ReceiptError(
            "scaling evidence bundle inventory changed before receipt publication"
        )
    for relative, path, metadata in current_scaling:
        record = next(
            item for item in scaling_files_raw if item["relative_path"] == relative
        )
        by_path[path] = {
            "path": str(path),
            "sha256": record["sha256"],
            "size_bytes": record["size_bytes"],
            "mode": record["mode"],
            "owner_uid": metadata.st_uid,
            "nlink": metadata.st_nlink,
        }

    prebuilt_bundle = receipt["evidence"].get("prebuilt_binary_bundle")
    if (
        not isinstance(prebuilt_bundle, dict)
        or prebuilt_bundle.get("schema_version") != 3
        or not isinstance(prebuilt_bundle.get("archive_id"), str)
        or not prebuilt_bundle["archive_id"].startswith(
            "release-prebuilt.bundle.v1:"
        )
        or not isinstance(prebuilt_bundle.get("manifest"), dict)
        or not isinstance(prebuilt_bundle.get("binaries"), list)
    ):
        raise ReceiptError("aggregate receipt lacks its prebuilt binary bundle")
    invocation_id = prebuilt_bundle["archive_id"].partition(":")[2]
    if _PREBUILT_INVOCATION_RE.fullmatch(invocation_id) is None:
        raise ReceiptError("aggregate receipt prebuilt bundle id is malformed")
    prebuilt_root = (
        release_root.parent
        / "output"
        / "sumeragi-v2-release"
        / receipt["identity"]["sealed_source_manifest_sha256"]
        / "programs"
        / invocation_id
    )
    prebuilt_manifest = prebuilt_bundle["manifest"]
    prebuilt_binaries = prebuilt_bundle["binaries"]
    if prebuilt_manifest.get("archive_id") != "release-prebuilt.manifest.v2":
        raise ReceiptError("aggregate receipt prebuilt manifest path is malformed")
    manifest_path = prebuilt_root / _PREBUILT_MANIFEST_NAME
    by_path[manifest_path] = {
        "path": str(manifest_path),
        "sha256": prebuilt_manifest["sha256"],
        "size_bytes": prebuilt_manifest["size_bytes"],
        "mode": prebuilt_manifest["mode"],
        "owner_uid": os.geteuid(),
        "nlink": 1,
    }
    expected_binary_paths = [
        (prefix, relative, prebuilt_root.joinpath(*PurePosixPath(relative).parts))
        for prefix, relative in _PREBUILT_BINARY_SPECS
    ]
    if len(prebuilt_binaries) != len(expected_binary_paths):
        raise ReceiptError("aggregate receipt prebuilt binary inventory is incomplete")
    for index, (record, (prefix, relative, path)) in enumerate(
        zip(prebuilt_binaries, expected_binary_paths)
    ):
        if (
            not isinstance(record, dict)
            or record.get("role") != prefix
            or record.get("relative_path") != relative
            or record.get("archive_id")
            != f"release-prebuilt.binary.{prefix}.v1"
        ):
            raise ReceiptError(
                f"aggregate receipt prebuilt binary {index} path is malformed"
            )
        by_path[path] = {
            "path": str(path),
            "sha256": record["sha256"],
            "size_bytes": record["size_bytes"],
            "mode": record["mode"],
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }

    cargo_cache = receipt["evidence"].get("cargo_cache_input")
    if not isinstance(cargo_cache, dict) or set(cargo_cache) != {
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
    }:
        raise ReceiptError("aggregate receipt Cargo-cache authentication is malformed")
    if (
        type(cargo_cache["schema_version"]) is not int
        or cargo_cache["schema_version"] != 2
        or cargo_cache["source_cargo_home_disclosure"] != "withheld"
        or any(
            type(cargo_cache[name]) is not int or cargo_cache[name] < 0
            for name in ("input_root_count", "input_record_count", "input_file_count")
        )
    ):
        raise ReceiptError("aggregate receipt Cargo-cache identity is malformed")
    artifact_root = release_root.parent / "output"
    cargo_file_specs = (
        (
            "inventory",
            "release-cargo-cache.input-inventory.v1",
            artifact_root / "cargo-cache-input.json",
        ),
        (
            "final_inventory",
            "release-cargo-cache.final-inventory.v1",
            artifact_root / "cargo-cache-final.json",
        ),
        (
            "runtime_inventory",
            "release-runtime.inventory.v1",
            release_root.parent / "runtime-input.json",
        ),
    )
    for field, archive_id, path in cargo_file_specs:
        record = cargo_cache[field]
        if (
            not isinstance(record, dict)
            or set(record) != {"archive_id", "mode", "sha256", "size_bytes"}
            or record.get("archive_id") != archive_id
        ):
            raise ReceiptError(f"aggregate receipt Cargo-cache {field} is malformed")
        by_path[path] = {
            "path": str(path),
            "sha256": record["sha256"],
            "size_bytes": record["size_bytes"],
            "mode": record["mode"],
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    if (
        receipt["evidence"].get("cargo_cache_input_inventory")
        != cargo_cache["inventory"]
        or receipt["evidence"].get("cargo_cache_final_inventory")
        != cargo_cache["final_inventory"]
    ):
        raise ReceiptError("aggregate receipt Cargo-cache inventory aliases disagree")
    sdk = _require_exact_json_fields(
        receipt["evidence"].get("sdk_dependencies"),
        {
            "schema_version", "source_disclosure", "source_manifest_sha256",
            "source_state_sha256", "archive", "input_inventory",
            "final_work_inventory",
        },
        "aggregate receipt SDK dependencies",
    )
    if (
        type(sdk["schema_version"]) is not int
        or sdk["schema_version"] != 1
        or sdk["source_disclosure"] != "withheld"
    ):
        raise ReceiptError("aggregate receipt SDK dependency identity is malformed")
    _require_digest(sdk["source_manifest_sha256"], "aggregate SDK source manifest")
    _require_digest(sdk["source_state_sha256"], "aggregate SDK source state")
    sdk_specs = (
        (
            "archive", "release-sdk-dependencies.bundle.v1",
            "sdk-dependency-bundle.tar",
        ),
        (
            "input_inventory", "release-sdk-dependencies.input-inventory.v1",
            "sdk-dependency-input.json",
        ),
        (
            "final_work_inventory", "release-sdk-dependencies.work-final.v1",
            "sdk-dependency-work-final.json",
        ),
    )
    for field, archive_id, archive_name in sdk_specs:
        record = _require_exact_json_fields(
            sdk[field],
            {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
            f"aggregate receipt SDK {field}",
        )
        if (
            record["archive_id"] != archive_id
            or record["archive_name"] != archive_name
            or record["mode"] != "0400"
        ):
            raise ReceiptError(f"aggregate receipt SDK {field} is not exact")
        by_path[release_root.parent / archive_name] = {
            "path": str(release_root.parent / archive_name),
            "sha256": record["sha256"],
            "size_bytes": record["size_bytes"],
            "mode": record["mode"],
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    runtime_probes = _require_exact_json_fields(
        receipt["evidence"].get("runtime_tool_probes"),
        {
            "format", "schema_version", "host_family",
            "probe_contract_sha256", "tool_count", "result",
        },
        "aggregate receipt runtime tool probes",
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
    ):
        raise ReceiptError("aggregate receipt runtime tool probes are malformed")
    _require_digest(
        runtime_probes["probe_contract_sha256"],
        "aggregate runtime tool probe contract digest",
    )
    runtime_result = _require_exact_json_fields(
        runtime_probes["result"],
        {"archive_id", "archive_name", "mode", "sha256", "size_bytes"},
        "aggregate receipt runtime tool probe result",
    )
    if (
        runtime_result["archive_id"] != "release-runtime.tool-probes.v1"
        or runtime_result["archive_name"] != "runtime-tool-probe-result.json"
        or runtime_result["mode"] != "0400"
        or not isinstance(runtime_result["sha256"], str)
        or _DIGEST_RE.fullmatch(runtime_result["sha256"]) is None
        or type(runtime_result["size_bytes"]) is not int
        or runtime_result["size_bytes"] < 0
    ):
        raise ReceiptError(
            "aggregate receipt runtime tool probe result is not exact"
        )
    by_path[release_root.parent / runtime_result["archive_name"]] = {
        "path": str(release_root.parent / runtime_result["archive_name"]),
        "sha256": runtime_result["sha256"],
        "size_bytes": runtime_result["size_bytes"],
        "mode": runtime_result["mode"],
        "owner_uid": os.geteuid(),
        "nlink": 1,
    }
    expected_runtime_environment = {
        "runtime_home_path": str(artifact_root / "home"),
        "runtime_tmpdir_path": str(artifact_root / "tmp"),
        "runtime_tmp_path": str(artifact_root / "tmp"),
        "runtime_temp_path": str(artifact_root / "tmp"),
        "runtime_cache_path": str(artifact_root / "cache"),
    }
    if cargo_cache["runtime_environment_sha256"] != hashlib.sha256(
        _canonical_json(expected_runtime_environment)
    ).hexdigest():
        raise ReceiptError("aggregate receipt runtime environment digest is not exact")
    runtime_directories = cargo_cache["runtime_directories"]
    if not isinstance(runtime_directories, dict) or set(runtime_directories) != {
        "home",
        "tmp",
        "cache",
    }:
        raise ReceiptError("aggregate receipt runtime directory inventory is malformed")
    cargo_cache_directories = {
        artifact_root / "home",
        artifact_root / "tmp",
        artifact_root / "cache",
        artifact_root / "cargo-home",
    }
    for name in ("cache", "home", "tmp"):
        record = runtime_directories[name]
        if record != {
            "archive_id": f"release-runtime.directory.{name}.v1",
            "mode": "0700",
        }:
            raise ReceiptError(
                f"aggregate receipt runtime directory {name} is malformed"
            )
    if cargo_cache["cargo_home"] != {
        "archive_id": "release-cargo-cache.home.v1",
        "mode": "0700",
    }:
        raise ReceiptError("aggregate receipt Cargo home archive is malformed")
    prebuilt_directories = {
        prebuilt_root,
        prebuilt_root / "release",
        prebuilt_root / "message-control",
        prebuilt_root / "message-control" / "release",
    }
    for path, name in (
        (prebuilt_root, "aggregate prebuilt invocation bundle"),
        (prebuilt_root / "release", "aggregate prebuilt release directory"),
        (
            prebuilt_root / "message-control",
            "aggregate prebuilt message-control directory",
        ),
        (
            prebuilt_root / "message-control" / "release",
            "aggregate prebuilt message-control release directory",
        ),
    ):
        _prebuilt_directory(path, name)
    _prebuilt_directory_inventory(
        prebuilt_root,
        {_PREBUILT_MANIFEST_NAME, "release", "message-control"},
        "aggregate prebuilt invocation bundle",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "release",
        {"iroha3d", "iroha", "kagami"},
        "aggregate prebuilt release directory",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "message-control",
        {"release"},
        "aggregate prebuilt message-control directory",
    )
    _prebuilt_directory_inventory(
        prebuilt_root / "message-control" / "release",
        {"iroha3d"},
        "aggregate prebuilt message-control release directory",
    )

    g4p = receipt["evidence"].get("g4p_multilane")
    if not isinstance(g4p, dict) or g4p.get("schema_version") != 1:
        raise ReceiptError("aggregate receipt lacks its G-4P evidence")
    try:
        g4p_root = Path(g4p["completion"]["path"]).parent
    except (KeyError, TypeError) as error:
        raise ReceiptError("aggregate receipt G-4P evidence is malformed") from error

    g12 = receipt["evidence"].get("g12_cross_dataspace")
    if not isinstance(g12, dict):
        raise ReceiptError("aggregate receipt lacks its G-12P evidence")
    try:
        g12_seed_root = Path(g12["seed_completion"]["path"]).parent
        g12_soak_root = Path(g12["fault_soak_completion"]["path"]).parent
    except (KeyError, TypeError) as error:
        raise ReceiptError("aggregate receipt G-12P evidence is malformed") from error

    evidence = receipt["evidence"]
    durability_families = (
        (
            "corridor",
            "corridor_completion",
            (
                "corridor_completion",
                "corridor_summary",
                "corridor_production_inventory",
                "g_unit_focused_test_inventory",
                "corridor_logs",
            ),
        ),
        (
            "formal",
            "formal_completion",
            (
                "formal_completion",
                "formal_gate_log",
                "formal_proof_coverage",
                "formal_proof_evidence",
                "formal_verus_evidence",
                "formal_verus_log",
                "formal_multilane_apalache_evidence",
                "formal_cross_tool_evidence",
                "formal_production_trace_extraction_evidence",
                "formal_harness_lock",
                "formal_toolchain",
                "formal_tlaps_resource_jsonl",
                "formal_tlaps_resource_summary",
            ),
        ),
        (
            "seed",
            "seed_matrix_completion",
            (
                "seed_matrix_completion",
                "seed_matrix_summary",
                "seed_matrix_run_logs",
                "seed_matrix_localnet_manifest_index",
                "seed_matrix_localnet_manifests",
            ),
        ),
        (
            "chaos",
            "chaos_completion",
            ("chaos_completion", "chaos_log"),
        ),
    )
    family_roots: set[Path] = set()
    family_directories: set[Path] = set()
    for family, completion_key, member_keys in durability_families:
        completion_record = evidence.get(completion_key)
        if (
            not isinstance(completion_record, dict)
            or not isinstance(completion_record.get("path"), str)
        ):
            raise ReceiptError(
                f"aggregate receipt {family} completion path is malformed"
            )
        root = Path(completion_record["path"]).parent
        family_roots.add(root)
        family_directories.add(root)
        for member_key in member_keys:
            member = evidence.get(member_key)
            if member is None:
                raise ReceiptError(
                    f"aggregate receipt {family} durability inventory is incomplete"
                )
            records = list(_iter_artifact_records(member))
            if not records:
                raise ReceiptError(
                    f"aggregate receipt {family} durability inventory is malformed"
                )
            for record in records:
                parent = Path(record["path"]).parent
                if parent != root and root not in parent.parents:
                    raise ReceiptError(
                        f"aggregate receipt {family} artifact escaped its evidence root"
                    )
                while True:
                    family_directories.add(parent)
                    if parent == root:
                        break
                    parent = parent.parent

    snapshots: list[PathContract | DirectoryContract] = list(
        bootstrap_runtime_contracts
    )
    inodes: dict[tuple[int, int], Path] = {}
    for index, (path, record) in enumerate(by_path.items()):
        mode_value = record.get("mode")
        mode = _octal_mode(mode_value, f"aggregate evidence {index} mode") if mode_value else None
        owner = record.get("owner_uid", os.geteuid())
        nlink = record.get("nlink", 1)
        size = record.get("size_bytes")
        if type(owner) is not int or type(nlink) is not int or (
            size is not None and type(size) is not int
        ):
            raise ReceiptError("aggregate receipt artifact metadata has non-integer fields")
        snapshot = _capture_path_contract(
            path,
            f"aggregate evidence {index}",
            expected_sha256=record["sha256"],
            expected_mode=mode,
            expected_owner=owner,
            expected_nlink=nlink,
            expected_size=size,
        )
        inode_key = (snapshot.device, snapshot.inode)
        alias = inodes.get(inode_key)
        if alias is not None and alias != path:
            raise ReceiptError("aggregate receipt evidence contains a hard-link alias")
        inodes[inode_key] = path
        snapshots.append(snapshot)
    evidence_root = bootstrap_evidence_root
    directory_paths = {
        evidence_root,
        scaling_root,
        g4p_root,
        g12_seed_root,
        g12_soak_root,
        candidate_root,
        evidence_root / "runner-bin",
        evidence_root / "runner-tools",
        release_root.parent,
        release_root,
    }
    directory_paths.update(family_roots)
    directory_paths.add(artifact_root)
    if private_build_roots_available:
        directory_paths.update(cargo_cache_directories)
    directory_paths.update(family_directories)
    directory_paths.update(prebuilt_directories)
    directory_paths.update(
        scaling_root.joinpath(*PurePosixPath(relative).parts)
        for relative in scaling_directories_raw
    )
    for path in by_path:
        parent = path.parent
        while parent == evidence_root or evidence_root in parent.parents:
            directory_paths.add(parent)
            if parent == evidence_root:
                break
            parent = parent.parent
    for index, path in enumerate(
        sorted(directory_paths, key=lambda item: (-len(item.parts), str(item)))
    ):
        snapshots.append(
            _capture_directory_contract(
                path,
                f"aggregate evidence directory {index}",
            )
        )
    return snapshots


def _capture_directory_contract(path: Path, name: str) -> DirectoryContract:
    if not path.is_absolute() or Path(os.path.abspath(path)) != path:
        raise ReceiptError(f"{name} path must be absolute and normalized")
    try:
        resolved = path.resolve(strict=True)
        before = path.lstat()
    except OSError as error:
        raise ReceiptError(f"{name} is unavailable") from error
    if resolved != path or stat.S_ISLNK(before.st_mode) or not stat.S_ISDIR(before.st_mode):
        raise ReceiptError(f"{name} must be a resolved non-symlink directory")
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ReceiptError(f"{name} could not be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_uid",
            "st_nlink",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if not stat.S_ISDIR(opened.st_mode) or any(
            getattr(opened, field) != getattr(before, field) for field in fields
        ):
            raise ReceiptError(f"{name} changed while it was opened")
        return DirectoryContract(
            path=path,
            device=opened.st_dev,
            inode=opened.st_ino,
            mode=stat.S_IMODE(opened.st_mode),
            owner=opened.st_uid,
            nlink=opened.st_nlink,
            mtime_ns=opened.st_mtime_ns,
            ctime_ns=opened.st_ctime_ns,
        )
    finally:
        os.close(descriptor)


def _revalidate_receipt_inputs(
    snapshots: list[PathContract | DirectoryContract],
    *,
    ignored_directories: frozenset[Path] = frozenset(),
) -> None:
    for index, snapshot in enumerate(snapshots):
        if isinstance(snapshot, DirectoryContract):
            if snapshot.path in ignored_directories:
                continue
            current_directory = _capture_directory_contract(
                snapshot.path, f"aggregate evidence directory {index}"
            )
            if current_directory != snapshot:
                raise ReceiptError(
                    "aggregate evidence directory "
                    f"{index} changed before publication: {snapshot.path}"
                )
            continue
        current = _capture_path_contract(
            snapshot.path,
            f"aggregate evidence {index}",
            expected_sha256=snapshot.sha256,
            expected_mode=snapshot.mode,
            expected_owner=snapshot.owner,
            expected_nlink=snapshot.nlink,
            expected_size=snapshot.size,
        )
        if current != snapshot:
            raise ReceiptError(f"aggregate evidence {index} changed before publication")


def _fsync_receipt_inputs(
    snapshots: list[PathContract | DirectoryContract],
    *,
    ignored_directories: frozenset[Path] = frozenset(),
) -> None:
    """Synchronize every evidence file and then its directories bottom-up."""

    ordered = [
        *[item for item in snapshots if isinstance(item, PathContract)],
        *sorted(
            (
                item
                for item in snapshots
                if isinstance(item, DirectoryContract)
                and item.path not in ignored_directories
            ),
            key=lambda item: (-len(item.path.parts), str(item.path)),
        ),
    ]
    for index, snapshot in enumerate(ordered):
        if isinstance(snapshot, DirectoryContract):
            current = _capture_directory_contract(
                snapshot.path, f"durability directory {index}"
            )
            flags = (
                os.O_RDONLY
                | getattr(os, "O_DIRECTORY", 0)
                | getattr(os, "O_CLOEXEC", 0)
            )
        else:
            current = _capture_path_contract(
                snapshot.path,
                f"durability evidence {index}",
                expected_sha256=snapshot.sha256,
                expected_mode=snapshot.mode,
                expected_owner=snapshot.owner,
                expected_nlink=snapshot.nlink,
                expected_size=snapshot.size,
            )
            flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
        if current != snapshot:
            raise ReceiptError(
                f"durability input {index} changed before fsync: {snapshot.path}"
            )
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        try:
            descriptor = os.open(snapshot.path, flags)
        except OSError as error:
            raise ReceiptError(f"durability input {index} could not be opened") from error
        try:
            opened = os.fstat(descriptor)
            if (
                (opened.st_dev, opened.st_ino)
                != (snapshot.device, snapshot.inode)
                or stat.S_IMODE(opened.st_mode) != snapshot.mode
                or opened.st_uid != snapshot.owner
                or opened.st_nlink != snapshot.nlink
            ):
                raise ReceiptError(f"durability input {index} changed while opened")
            os.fsync(descriptor)
            after = os.fstat(descriptor)
            fields = (
                "st_dev",
                "st_ino",
                "st_mode",
                "st_uid",
                "st_nlink",
                "st_mtime_ns",
                "st_ctime_ns",
            )
            if isinstance(snapshot, PathContract):
                fields += ("st_size",)
            if any(
                getattr(after, field) != getattr(opened, field) for field in fields
            ):
                raise ReceiptError(f"durability input {index} changed during fsync")
        except OSError as error:
            raise ReceiptError(f"durability input {index} fsync failed") from error
        finally:
            os.close(descriptor)
    _revalidate_receipt_inputs(
        snapshots, ignored_directories=ignored_directories
    )


def _existing_receipt_contract(output: Path, data: bytes) -> PathContract:
    return _capture_path_contract(
        output,
        "existing terminal receipt",
        expected_sha256=hashlib.sha256(data).hexdigest(),
        expected_mode=0o400,
        expected_owner=os.geteuid(),
        expected_nlink=1,
        expected_size=len(data),
    )


def _complete_write(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        try:
            written = os.write(descriptor, view)
        except InterruptedError:
            continue
        if written <= 0:
            raise ReceiptError("terminal receipt write made no progress")
        view = view[written:]


def _publish_terminal_receipt(
    output: Path,
    data: bytes,
    *,
    revalidate: Any,
) -> Path:
    if not output.is_absolute() or Path(os.path.abspath(output)) != output:
        raise ReceiptError("terminal receipt path must be absolute and normalized")
    parent, parent_stat = _private_evidence_directory(
        output.parent, "terminal receipt output directory"
    )
    if output.name in {"", ".", ".."} or "/" in output.name or "\0" in output.name:
        raise ReceiptError("terminal receipt output name is invalid")
    for ancestor in (parent, *parent.parents):
        metadata = ancestor.lstat()
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, os.geteuid()}
        ):
            raise ReceiptError("terminal receipt output has an unsafe ancestor")
    if os.path.lexists(output):
        raise ReceiptError("terminal receipt output already exists; overwrite is forbidden")
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        directory_flags |= os.O_NOFOLLOW
    try:
        directory_fd = os.open(parent, directory_flags)
    except OSError as error:
        raise ReceiptError("terminal receipt output directory could not be opened") from error
    staged_name = f".{output.name}.stage.{secrets.token_hex(16)}"
    staged_device = -1
    staged_inode = -1
    committed = False
    try:
        opened_parent = os.fstat(directory_fd)
        if (
            (opened_parent.st_dev, opened_parent.st_ino)
            != (parent_stat.st_dev, parent_stat.st_ino)
            or opened_parent.st_uid != os.geteuid()
            or stat.S_IMODE(opened_parent.st_mode) != 0o700
        ):
            raise ReceiptError("terminal receipt output directory changed while opened")
        flags = (
            os.O_RDWR
            | os.O_CREAT
            | os.O_EXCL
            | getattr(os, "O_CLOEXEC", 0)
        )
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(staged_name, flags, 0o600, dir_fd=directory_fd)
        try:
            staged_open = os.fstat(descriptor)
            staged_device, staged_inode = staged_open.st_dev, staged_open.st_ino
            if not stat.S_ISREG(staged_open.st_mode) or staged_open.st_nlink != 1:
                raise ReceiptError("terminal receipt stage is not one private regular file")
            _complete_write(descriptor, data)
            os.fchmod(descriptor, 0o400)
            os.fsync(descriptor)
            after_write = os.fstat(descriptor)
            if (
                after_write.st_uid != os.geteuid()
                or after_write.st_nlink != 1
                or stat.S_IMODE(after_write.st_mode) != 0o400
                or after_write.st_size != len(data)
            ):
                raise ReceiptError("terminal receipt stage metadata is not exact")
            os.lseek(descriptor, 0, os.SEEK_SET)
            staged_data = bytearray()
            while len(staged_data) < len(data):
                chunk = os.read(descriptor, min(1024 * 1024, len(data) - len(staged_data)))
                if not chunk:
                    break
                staged_data.extend(chunk)
            if bytes(staged_data) != data:
                raise ReceiptError("terminal receipt stage bytes failed verification")
        finally:
            os.close(descriptor)
        revalidate()
        _rename_with_flags = ctypes.CDLL(None, use_errno=True)
        if sys.platform == "darwin":
            rename = _rename_with_flags.renameatx_np; rename_flag = 4
        elif sys.platform.startswith("linux") and hasattr(_rename_with_flags, "renameat2"):
            rename = _rename_with_flags.renameat2; rename_flag = 1
        else:
            raise ReceiptError("terminal receipt no-replace rename is unavailable")
        rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
        rename.restype = ctypes.c_int
        if rename(directory_fd, os.fsencode(staged_name), directory_fd, os.fsencode(output.name), rename_flag) != 0:
            number = ctypes.get_errno()
            try:
                reported_output = os.stat(
                    output.name, dir_fd=directory_fd, follow_symlinks=False
                )
            except FileNotFoundError:
                pass
            else:
                if (
                    stat.S_ISREG(reported_output.st_mode)
                    and reported_output.st_uid == os.geteuid()
                    and reported_output.st_nlink == 1
                    and (reported_output.st_dev, reported_output.st_ino)
                    == (staged_device, staged_inode)
                ):
                    committed = True
            raise OSError(number, os.strerror(number), output.name)
        committed = True
        os.fsync(directory_fd)
        published = os.stat(output.name, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(published.st_mode)
            or (published.st_dev, published.st_ino) != (staged_device, staged_inode)
            or stat.S_IMODE(published.st_mode) != 0o400
            or published.st_nlink != 1
        ):
            raise ReceiptError("terminal receipt changed at publication")
        final = _capture_path_contract(
            output,
            "published terminal receipt",
            expected_sha256=hashlib.sha256(data).hexdigest(),
            expected_mode=0o400,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=len(data),
        )
        if (final.device, final.inode) != (staged_device, staged_inode):
            raise ReceiptError("terminal receipt inode changed after publication")
        revalidate()
        return output
    except BaseException as error:
        if committed and staged_inode >= 0:
            _owned_unlink_name(directory_fd, output.name, staged_device, staged_inode)
        elif staged_inode >= 0:
            _owned_unlink_name(directory_fd, staged_name, staged_device, staged_inode)
        try:
            os.fsync(directory_fd)
        except OSError:
            pass
        if isinstance(error, ReceiptError):
            raise
        if isinstance(error, OSError):
            raise ReceiptError("terminal receipt publication failed closed") from error
        raise
    finally:
        try:
            os.close(directory_fd)
        except OSError:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-identity", type=Path, required=True)
    parser.add_argument("--sealed-identity", type=Path, required=True)
    parser.add_argument("--release-root", type=Path, required=True)
    parser.add_argument("--signature-attestation", type=Path, required=True)
    parser.add_argument("--signature-transcript", type=Path, required=True)
    parser.add_argument("--signature-raw-commit", type=Path, required=True)
    parser.add_argument("--signature-cargo-lock", type=Path, required=True)
    parser.add_argument("--signature-allowed-signers", type=Path, required=True)
    parser.add_argument("--signature-revocation", type=Path, required=True)
    parser.add_argument("--signature-git", type=Path, required=True)
    parser.add_argument("--signature-ssh-keygen", type=Path, required=True)
    parser.add_argument("--expected-git-sha256", required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--expected-revocation-sha256", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--bootstrap-completion", type=Path, required=True)
    parser.add_argument("--bootstrap-evidence-dir", type=Path, required=True)
    parser.add_argument("--bootstrap-identity", type=Path, required=True)
    parser.add_argument("--bootstrap-attestation", type=Path, required=True)
    parser.add_argument("--bootstrap-transcript", type=Path, required=True)
    parser.add_argument(
        "--expected-bootstrap-completion-sha256", required=True
    )
    parser.add_argument("--bootstrap-candidate-root", type=Path, required=True)
    parser.add_argument("--bootstrap-runner", type=Path, required=True)
    parser.add_argument("--corridor-completion", type=Path, required=True)
    parser.add_argument("--formal-completion", type=Path, required=True)
    parser.add_argument(
        "--formal-replay-source-receipt", type=Path, required=True
    )
    parser.add_argument(
        "--formal-replay-release-root", type=Path, required=True
    )
    parser.add_argument(
        "--expected-formal-replay-signature-sha256", required=True
    )
    parser.add_argument("--formal-replay-principal", required=True)
    parser.add_argument("--seed-completion", type=Path, required=True)
    parser.add_argument("--chaos-completion", type=Path, required=True)
    parser.add_argument("--g4p-completion", type=Path, required=True)
    parser.add_argument("--g12-seed-completion", type=Path, required=True)
    parser.add_argument("--g12-fault-soak-completion", type=Path, required=True)
    parser.add_argument("--scaling-evidence-manifest", type=Path, required=True)
    parser.add_argument("--sdk-dependency-archive", type=Path, required=True)
    parser.add_argument(
        "--sdk-dependency-input-inventory", type=Path, required=True
    )
    parser.add_argument(
        "--sdk-dependency-final-work-inventory", type=Path, required=True
    )
    parser.add_argument(
        "--runtime-tool-probe-manifest", type=Path, required=True
    )
    parser.add_argument(
        "--runtime-tool-probe-result", type=Path, required=True
    )
    parser.add_argument(
        "--expected-scaling-trial-harness-sha256", required=True
    )
    parser.add_argument(
        "--expected-scaling-configuration-sha256", required=True
    )
    parser.add_argument("--expected-scaling-irohad-sha256", required=True)
    parser.add_argument("--expected-scaling-iroha-cli-sha256", required=True)
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--verify-existing", action="store_true")
    parser.add_argument("--replay-existing", action="store_true")
    _receipt_validation_ack_arguments(parser)
    args = parser.parse_args()
    try:
        if args.verify_existing and args.replay_existing:
            raise ReceiptError(
                "receipt validation and bootstrap replay operations are exclusive"
            )
        (
            receipt,
            candidate_identity,
            sealed_identity,
            bootstrap_runtime_contracts,
        ) = build_receipt(
            candidate_identity_path=args.candidate_identity,
            sealed_identity_path=args.sealed_identity,
            release_root_path=args.release_root,
            signature_attestation_path=args.signature_attestation,
            signature_transcript_path=args.signature_transcript,
            signature_raw_commit_path=args.signature_raw_commit,
            signature_cargo_lock_path=args.signature_cargo_lock,
            signature_allowed_signers_path=args.signature_allowed_signers,
            signature_revocation_path=args.signature_revocation,
            signature_git_path=args.signature_git,
            signature_ssh_keygen_path=args.signature_ssh_keygen,
            expected_git_sha256=args.expected_git_sha256,
            expected_ssh_keygen_sha256=args.expected_ssh_keygen_sha256,
            expected_allowed_signers_sha256=args.expected_allowed_signers_sha256,
            expected_revocation_sha256=args.expected_revocation_sha256,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
            bootstrap_completion_path=args.bootstrap_completion,
            bootstrap_evidence_dir_path=args.bootstrap_evidence_dir,
            bootstrap_identity_path=args.bootstrap_identity,
            bootstrap_attestation_path=args.bootstrap_attestation,
            bootstrap_transcript_path=args.bootstrap_transcript,
            expected_bootstrap_completion_sha256=(
                args.expected_bootstrap_completion_sha256
            ),
            bootstrap_candidate_root_path=args.bootstrap_candidate_root,
            bootstrap_runner_path=args.bootstrap_runner,
            corridor_completion_path=args.corridor_completion,
            formal_completion_path=args.formal_completion,
            formal_replay_source_receipt_path=(
                args.formal_replay_source_receipt
            ),
            formal_replay_release_root_path=args.formal_replay_release_root,
            expected_formal_replay_signature_sha256=(
                args.expected_formal_replay_signature_sha256
            ),
            formal_replay_principal=args.formal_replay_principal,
            seed_completion_path=args.seed_completion,
            chaos_completion_path=args.chaos_completion,
            g4p_completion_path=args.g4p_completion,
            g12_seed_completion_path=args.g12_seed_completion,
            g12_fault_soak_completion_path=args.g12_fault_soak_completion,
            scaling_evidence_manifest_path=args.scaling_evidence_manifest,
            sdk_dependency_archive_path=args.sdk_dependency_archive,
            sdk_dependency_input_inventory_path=(
                args.sdk_dependency_input_inventory
            ),
            sdk_dependency_final_work_inventory_path=(
                args.sdk_dependency_final_work_inventory
            ),
            runtime_tool_probe_manifest_path=args.runtime_tool_probe_manifest,
            runtime_tool_probe_result_path=args.runtime_tool_probe_result,
            runtime_tool_probe_runtime_available=(not args.replay_existing),
            private_build_roots_available=(not args.replay_existing),
            bootstrap_private_inputs_available=(not args.replay_existing),
            expected_scaling_trial_harness_sha256=(
                args.expected_scaling_trial_harness_sha256
            ),
            expected_scaling_configuration_sha256=(
                args.expected_scaling_configuration_sha256
            ),
            expected_scaling_irohad_sha256=(
                args.expected_scaling_irohad_sha256
            ),
            expected_scaling_iroha_cli_sha256=(
                args.expected_scaling_iroha_cli_sha256
            ),
            repository_root_path=args.repository_root,
            runner_logs_sealed=(args.verify_existing or args.replay_existing),
        )
        snapshots = _snapshot_receipt_inputs(
            receipt,
            candidate_identity=candidate_identity,
            sealed_identity=sealed_identity,
            scaling_root=args.scaling_evidence_manifest.parent,
            bootstrap_evidence_root=args.bootstrap_evidence_dir,
            candidate_root=args.bootstrap_candidate_root,
            release_root=args.release_root,
            bootstrap_runtime_contracts=bootstrap_runtime_contracts,
            private_build_roots_available=(not args.replay_existing),
        )
        expected_output = (
            args.release_root.parent
            / "output"
            / "release"
            / "RELEASE_COMPLETED.json"
        )
        if args.output != expected_output:
            raise ReceiptError(
                "terminal receipt is not the exact bootstrap release output path"
            )
        receipt_bytes = _canonical_json(receipt)
        if args.verify_existing or args.replay_existing:
            terminal = _existing_receipt_contract(args.output, receipt_bytes)
            verification_snapshots = [*snapshots, terminal]
            _fsync_receipt_inputs(verification_snapshots)
            _revalidate_receipt_inputs(verification_snapshots)
            if args.replay_existing:
                _require_pruned_build_roots(args.release_root)
            if args.verify_existing:
                _receipt_validation_ack(args, verification_snapshots)
            elif any((args.validation_ack, args.source_manifest_sha256)):
                raise ReceiptError("receipt replay rejects acknowledgment inputs")
        else:
            if any((args.validation_ack, args.source_manifest_sha256)):
                raise ReceiptError("receipt publication rejects ack inputs")
            _fsync_receipt_inputs(snapshots)
            mutable_directory = frozenset({args.output.parent})
            _publish_terminal_receipt(
                args.output,
                receipt_bytes,
                revalidate=lambda: _revalidate_receipt_inputs(
                    snapshots, ignored_directories=mutable_directory
                ),
            )
    except (OSError, ReceiptError) as error:
        print(f"Sumeragi v2 release receipt error: {error}", file=sys.stderr)
        return 1
    action = (
        "replayed"
        if args.replay_existing
        else "verified"
        if args.verify_existing
        else "published"
    )
    print(
        f"Sumeragi v2 aggregate release receipt {action}: {args.output}"
    )
    return 0
